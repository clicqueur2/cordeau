//! Ecran des transferts de stock : faire passer de la marchandise d'un
//! entrepôt à un autre.
//!
//! Dolibarr a `stocktransfer`. Deux gestes, dans cet ordre : on **expédie**
//! (la marchandise quitte la source), puis on **reçoit** (elle arrive à
//! destination). Entre les deux, elle est en route — ni ici ni là.
//!
//! # Le stock ne bouge pas
//!
//! Un transfert réellement exécuté décrémente la source et incrémente la
//! destination, en deux mouvements. Ici la pièce est tracée, mais aucun
//! mouvement n'est écrit : écrire une moitié de transfert (la sortie sans
//! l'entrée) créerait un stock faux et impossible à expliquer. Ce module est un
//! **registre de transferts**, pas un exécutant — et l'écran le dit.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::transfert_stock::{self, NewLigneTransfert, NewTransfert};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste et le formulaire.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let transferts = transfert_stock::list(&app.pool, entity, 200, 0)?;
    let entrepots = db::stock::entrepots(&app.pool, entity).unwrap_or_default();
    let produits = db::product::list(&app.pool, entity, 500, 0).unwrap_or_default();

    Ok(ui::page(
        "transferts",
        "transferts",
        "Transferts de stock",
        html! {
            h1 { (crate::langue::tr("Transferts de stock")) }
            p .sous-titre {
                (crate::langue::tr("Faire passer de la marchandise d'un entrepôt à un autre : expédition, puis réception. Entre les deux, elle est en route."))
            }

            h2 { (crate::langue::tr("Nouveau transfert")) }
            @if entrepots.len() < 2 || produits.is_empty() {
                (ui::vide(
                    "Il faut au moins deux entrepôts et un produit.",
                    Some(("/stock", "Voir le stock")),
                ))
            } @else {
                form method="post" action="/transferts" {
                    label for="label" { (crate::langue::tr("Libellé")) } " "
                    input type="text" name="label" id="label" style="min-width:220px"
                          placeholder=(crate::langue::tr("Déplacement vers le chantier"));
                    br;
                    label for="source" { (crate::langue::tr("De")) } " "
                    select name="fk_warehouse_source" id="source" {
                        @for e in &entrepots {
                            option value=(e.rowid) { (e.ref_.clone()) }
                        }
                    }
                    " "
                    label for="destination" { (crate::langue::tr("Vers")) } " "
                    select name="fk_warehouse_destination" id="destination" {
                        @for e in &entrepots {
                            option value=(e.rowid) { (e.ref_.clone()) }
                        }
                    }
                    br;
                    label for="fk_product" { (crate::langue::tr("Produit")) } " "
                    select name="fk_product" id="fk_product" style="min-width:260px" {
                        @for p in &produits {
                            option value=(p.rowid) { (p.label.clone()) }
                        }
                    }
                    " "
                    label for="qty" { (crate::langue::tr("Quantité")) } " "
                    input type="text" name="qty" id="qty" required style="width:100px";
                    " "
                    button type="submit" { (crate::langue::tr("Créer le bon")) }
                }
                p style="color:#78736c;font-size:13px;margin:8px 0 0" {
                    (crate::langue::tr("Un transfert vers le même entrepôt est refusé : ce n'est pas un transfert."))
                }
            }

            h2 { (crate::langue::tr("Bons de transfert")) }
            @if transferts.is_empty() {
                (ui::vide("Aucun transfert enregistré.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Libellé")) }
                            th { (crate::langue::tr("Trajet")) }
                            th { (crate::langue::tr("Quantité")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for t in &transferts {
                            tr {
                                td .ref {
                                    a href=(format!("/transferts/{}", t.rowid)) { (t.ref_.clone()) }
                                }
                                td { (t.label.clone().unwrap_or_else(|| "—".to_string())) }
                                td .ref {
                                    (t.entrepot_source.clone().unwrap_or_else(|| "?".to_string()))
                                    " → "
                                    (t.entrepot_destination.clone().unwrap_or_else(|| "?".to_string()))
                                }
                                td .num { (t.quantite_totale.normalize().to_string()) }
                                td { (pastille(t.status)) }
                                td {
                                    a href=(format!("/transferts/{}", t.rowid)) class="petit secondaire" {
                                        (crate::langue::tr("Ouvrir"))
                                    }
                                }
                            }
                        }
                    }
                }
            }

            p style="color:#78736c;font-size:13px;margin-top:16px" {
                (crate::langue::tr("Ce registre ne déplace pas le stock : un transfert écrit à moitié (la sortie sans l'entrée) laisserait une quantité fantôme. Le mouvement viendra avec le module dédié."))
            }
        },
    ))
}

/// La fiche d'un transfert.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let t = transfert_stock::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("transfert {id} introuvable")))?;
    let lignes = transfert_stock::lines(&app.pool, entity, id)?;

    Ok(ui::page(
        "transferts",
        "transferts",
        &t.ref_,
        html! {
            h1 { (t.ref_.clone()) }
            p .sous-titre {
                (t.entrepot_source.clone().unwrap_or_else(|| "?".to_string()))
                " → "
                (t.entrepot_destination.clone().unwrap_or_else(|| "?".to_string()))
                @if let Some(l) = &t.label {
                    " · " (l.clone())
                }
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Quantité"),
                    &t.quantite_totale.normalize().to_string(),
                    Some(&crate::langue::tr("toutes lignes confondues")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("État"),
                    &libelle(t.status),
                    None,
                    false,
                ))
            }

            h2 { (crate::langue::tr("Lignes")) }
            @if lignes.is_empty() {
                (ui::vide("Ce bon n'a aucune ligne.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Produit")) }
                            th { (crate::langue::tr("Lot")) }
                            th { (crate::langue::tr("Quantité")) }
                        }
                    }
                    tbody {
                        @for l in &lignes {
                            tr {
                                td { (nom_produit(l)) }
                                td .ref { (l.batch.clone().unwrap_or_else(|| "—".to_string())) }
                                td .num { (l.qty.normalize().to_string()) }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Actions")) }
            @match t.status {
                0 => {
                    form method="post" action=(format!("/transferts/{}/expedier", t.rowid)) {
                        label for="date" { (crate::langue::tr("Départ")) } " "
                        input type="date" name="date" id="date" required;
                        " "
                        button type="submit" { (crate::langue::tr("Expédier")) }
                    }
                }
                1 => {
                    form method="post" action=(format!("/transferts/{}/recevoir", t.rowid)) {
                        label for="date" { (crate::langue::tr("Arrivée")) } " "
                        input type="date" name="date" id="date" required;
                        " "
                        button type="submit" { (crate::langue::tr("Réceptionner")) }
                    }
                    p style="color:#78736c;font-size:13.5px" {
                        (crate::langue::tr("La marchandise est en route. Elle n'est ni dans l'entrepôt de départ, ni dans celui d'arrivée."))
                    }
                }
                _ => {
                    p style="color:#78736c;font-size:13.5px" {
                        (crate::langue::tr("Ce transfert est reçu. Le stock n'a pas été mouvementé : c'est le module de stock qui le fera."))
                    }
                }
            }
        },
    ))
}

/// Cree un transfert avec sa premiere ligne.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();
    let nombre = |cle: &str| -> Result<i64, Erreur> {
        lire(cle)
            .parse::<i64>()
            .map_err(|_| Erreur(format!("{cle} manquant")))
    };

    let login = crate::utilisateur::courant()
        .ok_or_else(|| Erreur("aucun utilisateur en session".into()))?;
    let entity = entite::courante();
    let auteur = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;

    let produit = nombre("fk_product")?;
    let qty: Decimal = lire("qty")
        .parse()
        .map_err(|_| Erreur("quantité invalide".into()))?;

    let mut new = NewTransfert::new(
        nombre("fk_warehouse_source")?,
        nombre("fk_warehouse_destination")?,
        auteur,
    );
    new.label = Some(lire("label")).filter(|s| !s.is_empty());
    new.lines = vec![NewLigneTransfert::new(produit, qty)];

    let id = transfert_stock::create(&app.pool, entity, &new)?;
    Ok(Redirect::to(&format!("/transferts/{id}")))
}

/// Marque le depart.
pub async fn expedier(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let date = f
        .get("date")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("date de départ manquante".into()))?;
    transfert_stock::expedier(&app.pool, entite::courante(), id, &date)?;
    Ok(Redirect::to(&format!("/transferts/{id}")))
}

/// Marque l'arrivee.
pub async fn recevoir(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let date = f
        .get("date")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("date d'arrivée manquante".into()))?;
    transfert_stock::recevoir(&app.pool, entite::courante(), id, &date)?;
    Ok(Redirect::to(&format!("/transferts/{id}")))
}

/// Le nom d'un produit : son libelle, sinon sa reference, sinon son numero.
///
/// Trois niveaux parce qu'une ligne peut venir d'une suppression de fiche
/// produit : dans ce cas il reste au moins un identifiant a montrer, et un
/// transfert sans designation ne se verifie pas.
fn nom_produit(l: &transfert_stock::LigneTransfert) -> String {
    if !l.label_produit.trim().is_empty() {
        l.label_produit.clone()
    } else if !l.ref_produit.trim().is_empty() {
        l.ref_produit.clone()
    } else {
        format!("produit {}", l.fk_product)
    }
}

/// Le libelle d'un etat.
fn libelle(status: i16) -> String {
    match status {
        transfert_stock::STATUT_EXPEDIE => "expédié",
        transfert_stock::STATUT_RECU => "reçu",
        _ => "brouillon",
    }
    .to_string()
}

/// La pastille d'etat.
fn pastille(status: i16) -> maud::Markup {
    let classe = match status {
        transfert_stock::STATUT_RECU => "p-payee",
        transfert_stock::STATUT_EXPEDIE => "p-partielle",
        _ => "p-brouillon",
    };
    html! { span class=(format!("pastille {classe}")) { (libelle(status)) } }
}
