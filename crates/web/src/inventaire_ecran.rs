//! Ecran des inventaires : ce qu'on devrait avoir, ce qu'on a vraiment.
//!
//! Dolibarr a `inventory.php`. Le seul chiffre qui decide est l'**ecart** :
//! constate moins theorique. Un inventaire qui n'affiche que les deux quantites
//! oblige a faire la soustraction de tete, et c'est comme ca qu'on laisse passer
//! une rupture de stock.
//!
//! # Le stock ne bouge pas
//!
//! Valider un inventaire dans Dolibarr cree d'ordinaire des mouvements de
//! regularisation. Ici la piece est tracee, mais le stock reste intact : ecrire
//! le stock sans le module de mouvements complet produirait des quantites
//! fantomes qu'aucune ligne d'historique n'explique. L'ecran le dit, et un test
//! le verifie.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::inventaire::{self, NewInventaire, NewLigneInventaire};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste et le formulaire de saisie.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let inventaires = inventaire::list(&app.pool, entity, 200, 0)?;
    let entrepots = db::stock::entrepots(&app.pool, entity).unwrap_or_default();
    let produits = db::product::list(&app.pool, entity, 500, 0).unwrap_or_default();

    Ok(ui::page(
        "inventaires",
        "inventaires",
        "Inventaires",
        html! {
            h1 { (crate::langue::tr("Inventaires")) }
            p .sous-titre {
                (crate::langue::tr("Le comptage d'un entrepôt. Le seul chiffre qui décide est l'écart : constaté moins théorique."))
            }

            h2 { (crate::langue::tr("Nouvel inventaire")) }
            @if entrepots.is_empty() || produits.is_empty() {
                (ui::vide(
                    "Il faut au moins un entrepôt et un produit pour compter.",
                    Some(("/stock", "Voir le stock")),
                ))
            } @else {
                form method="post" action="/inventaires" {
                    label for="title" { (crate::langue::tr("Titre")) } " "
                    input type="text" name="title" id="title" required style="min-width:240px"
                          placeholder=(crate::langue::tr("Comptage de fin d'année"));
                    " "
                    label for="date_inventory" { (crate::langue::tr("Date du comptage")) } " "
                    input type="date" name="date_inventory" id="date_inventory" required;
                    br;
                    label for="fk_warehouse" { (crate::langue::tr("Entrepôt")) } " "
                    select name="fk_warehouse" id="fk_warehouse" {
                        @for e in &entrepots {
                            option value=(e.rowid) { (e.ref_.clone()) }
                        }
                    }
                    " "
                    label for="fk_product" { (crate::langue::tr("Produit")) } " "
                    select name="fk_product" id="fk_product" style="min-width:260px" {
                        @for p in &produits {
                            option value=(p.rowid) { (p.label.clone()) }
                        }
                    }
                    br;
                    label for="qty_stock" { (crate::langue::tr("Quantité théorique")) } " "
                    input type="text" name="qty_stock" id="qty_stock" required style="width:110px";
                    " "
                    label for="qty_viewed" { (crate::langue::tr("Quantité constatée")) } " "
                    input type="text" name="qty_viewed" id="qty_viewed" required style="width:110px";
                    " "
                    button type="submit" { (crate::langue::tr("Créer le comptage")) }
                }
                p style="color:#78736c;font-size:13px;margin:8px 0 0" {
                    (crate::langue::tr("L'écart ne se saisit pas : il se calcule. Un écart saisi à la main est un écart qu'on peut faire dire n'importe quoi."))
                }
            }

            h2 { (crate::langue::tr("Comptages")) }
            @if inventaires.is_empty() {
                (ui::vide("Aucun inventaire enregistré.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Titre")) }
                            th { (crate::langue::tr("Comptage")) }
                            th { (crate::langue::tr("Lignes")) }
                            th { (crate::langue::tr("Écart")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for i in &inventaires {
                            tr {
                                td .ref {
                                    a href=(format!("/inventaires/{}", i.rowid)) { (i.ref_.clone()) }
                                }
                                td { (i.title.clone()) }
                                td .ref {
                                    (i.date_inventory.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td { (i.nb_lignes) }
                                td .num { (ecart(i.ecart_total)) }
                                td { (pastille(i.status)) }
                                td {
                                    a href=(format!("/inventaires/{}", i.rowid)) class="petit secondaire" {
                                        (crate::langue::tr("Ouvrir"))
                                    }
                                }
                            }
                        }
                    }
                }
            }

            p style="color:#78736c;font-size:13px;margin-top:16px" {
                (crate::langue::tr("Valider un inventaire ne modifie pas le stock : le mouvement de régularisation viendra avec le module dédié, pour que chaque quantité ait une origine traçable."))
            }
        },
    ))
}

/// La fiche d'un comptage.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let i = inventaire::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("inventaire {id} introuvable")))?;
    let lignes = inventaire::lines(&app.pool, entity, id)?;

    Ok(ui::page(
        "inventaires",
        "inventaires",
        &i.ref_,
        html! {
            h1 { (i.title.clone()) }
            p .sous-titre {
                (i.ref_.clone()) " · "
                (i.date_inventory.clone().unwrap_or_else(|| "—".to_string()))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Écart total"),
                    &ecart(i.ecart_total),
                    Some(&crate::langue::tr("constaté moins théorique")),
                    i.ecart_total < Decimal::ZERO,
                ))
                (ui::carte(
                    &crate::langue::tr("Lignes"),
                    &lignes.len().to_string(),
                    None,
                    false,
                ))
            }

            h2 { (crate::langue::tr("Lignes")) }
            @if lignes.is_empty() {
                (ui::vide("Ce comptage n'a aucune ligne.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Produit")) }
                            th { (crate::langue::tr("Lot")) }
                            th { (crate::langue::tr("Théorique")) }
                            th { (crate::langue::tr("Constaté")) }
                            th { (crate::langue::tr("Écart")) }
                        }
                    }
                    tbody {
                        @for l in &lignes {
                            tr {
                                td { (produit_lisible(&l.fk_product)) }
                                td .ref { (l.batch.clone().unwrap_or_else(|| "—".to_string())) }
                                td .num { (l.qty_stock.normalize().to_string()) }
                                td .num { (l.qty_viewed.normalize().to_string()) }
                                td .num { (ecart(l.qty_regulated)) }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Actions")) }
            @if i.status == inventaire::STATUT_BROUILLON {
                form method="post" action=(format!("/inventaires/{}/valider", i.rowid)) {
                    button type="submit" { (crate::langue::tr("Valider le comptage")) }
                }
                p style="color:#78736c;font-size:13.5px" {
                    (crate::langue::tr("Valider arrête le comptage. Le stock n'est pas modifié : l'écart est constaté, pas appliqué."))
                }
            } @else {
                p style="color:#78736c;font-size:13.5px" {
                    (crate::langue::tr("Ce comptage est validé."))
                }
            }
        },
    ))
}

/// Cree un comptage avec sa premiere ligne.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();
    let decimal = |cle: &str| -> Result<Decimal, Erreur> {
        lire(cle)
            .parse::<Decimal>()
            .map_err(|_| Erreur(format!("{cle} : quantité invalide")))
    };

    let entrepot = lire("fk_warehouse")
        .parse::<i64>()
        .map_err(|_| Erreur("entrepôt manquant".into()))?;
    let produit = lire("fk_product")
        .parse::<i64>()
        .map_err(|_| Erreur("produit manquant".into()))?;

    let mut new = NewInventaire::new(&lire("title"));
    new.fk_warehouse = Some(entrepot);
    new.fk_product = Some(produit);
    new.date_inventory = Some(lire("date_inventory")).filter(|s| !s.is_empty());
    new.lines = vec![NewLigneInventaire::new(
        entrepot,
        produit,
        decimal("qty_stock")?,
        decimal("qty_viewed")?,
    )];

    let id = inventaire::create(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to(&format!("/inventaires/{id}")))
}

/// Valide un comptage.
pub async fn valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    inventaire::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/inventaires/{id}")))
}

/// Un ecart, signe, en francais : `+2`, `−3`, `0`.
///
/// Le signe compte plus que la valeur : un manque et un exces ne se corrigent
/// pas de la meme facon, et sans signe les deux se ressemblent.
fn ecart(valeur: Decimal) -> String {
    let v = valeur.normalize();
    if v > Decimal::ZERO {
        format!("+{v}")
    } else if v < Decimal::ZERO {
        format!("\u{2212}{}", v.abs())
    } else {
        "0".to_string()
    }
}

/// Le nom d'un produit, sans requete : la ligne ne porte que son identifiant.
fn produit_lisible(fk: &i64) -> String {
    format!("produit {fk}")
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        inventaire::STATUT_VALIDE => ("p-payee", "validé"),
        _ => ("p-brouillon", "brouillon"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_ecart_porte_son_signe() {
        // Le signe compte plus que la valeur : un manque et un exces ne se
        // corrigent pas de la meme facon.
        assert_eq!(ecart(Decimal::new(25, 1)), "+2.5");
        assert_eq!(ecart(Decimal::new(-30, 1)), "\u{2212}3");
        assert_eq!(ecart(Decimal::ZERO), "0");
    }
}
