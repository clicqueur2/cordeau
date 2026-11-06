//! Ecran des marges : ce que rapporte reellement une affaire.
//!
//! Dolibarr n'a **aucune table** de marge : son module recalcule a la volee la
//! difference entre le prix de vente et le prix d'achat des lignes de facture.
//! Ici on garde une piece : un libelle, des lignes, et un taux qu'on peut
//! relire dans six mois.
//!
//! # Deux vocabulaires, et ce n'est pas un detail
//!
//! Dolibarr distingue le **taux de marge** (marge / prix d'achat) du **taux de
//! marque** (marge / prix de vente). Les deux s'affichent, parce qu'un commercant
//! qui confond les deux annonce 50 % la ou il gagne 33 %.
//!
//! # Le taux global ne s'obtient pas en moyennant
//!
//! Une ligne a 10 EUR ne pese pas comme une ligne a 10 000 EUR. Le taux global se
//! calcule donc sur les totaux, jamais comme une moyenne des taux de ligne.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::marge::{self, NouvelleLigneMarge, NouvelleMarge};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste et le formulaire.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let affaires = marge::list(&app.pool, entity, 200, 0)?;
    let clients = db::party::list(&app.pool, entity, 500, 0)?;
    let produits = db::product::list(&app.pool, entity, 500, 0).unwrap_or_default();

    Ok(ui::page(
        "marges",
        "marges",
        "Marges",
        html! {
            h1 { (crate::langue::tr("Marges")) }
            p .sous-titre {
                (crate::langue::tr("Ce que rapporte réellement une affaire : le prix de vente face au prix d'achat. Dolibarr n'en garde aucune trace en base — ici, la pièce reste."))
            }

            h2 { (crate::langue::tr("Nouvelle affaire")) }
            @if clients.is_empty() {
                (ui::vide(
                    "Aucun tiers enregistré : une marge se mesure sur une affaire avec quelqu'un.",
                    Some(("/tiers", "Voir les tiers")),
                ))
            } @else if produits.is_empty() {
                (ui::vide(
                    "Aucun produit : une ligne de marge compare un prix de vente à un prix d'achat.",
                    Some(("/produits", "Voir les produits")),
                ))
            } @else {
                form method="post" action="/marges" {
                    label for="libelle" { (crate::langue::tr("Libellé")) } " "
                    input type="text" name="libelle" id="libelle" required style="min-width:240px"
                          placeholder=(crate::langue::tr("Terrasse chez M. Dupont"));
                    " "
                    label for="fk_soc" { (crate::langue::tr("Client")) } " "
                    select name="fk_soc" id="fk_soc" class="champ-large" {
                        @for c in &clients {
                            option value=(c.rowid) { (c.nom.clone()) }
                        }
                    }
                    br;
                    label for="designation" { (crate::langue::tr("Ligne")) } " "
                    input type="text" name="designation" id="designation" required class="champ-large";
                    " "
                    label for="quantite" { (crate::langue::tr("Qté")) } " "
                    input type="text" name="quantite" id="quantite" value="1" style="width:80px" required;
                    " "
                    label for="prix_vente_ht" { (crate::langue::tr("Vente HT")) } " "
                    input type="text" name="prix_vente_ht" id="prix_vente_ht" required class="champ-court";
                    " "
                    label for="prix_achat_ht" { (crate::langue::tr("Achat HT")) } " "
                    input type="text" name="prix_achat_ht" id="prix_achat_ht" required class="champ-court";
                    " "
                    button type="submit" { (crate::langue::tr("Créer")) }
                }
                p class="aide petite sous-champ" {
                    (crate::langue::tr("Le premier produit sert à créer l'affaire ; les lignes suivantes s'ajoutent sur sa fiche."))
                }
            }

            h2 { (crate::langue::tr("Affaires")) }
            @if affaires.is_empty() {
                (ui::vide("Aucune affaire chiffrée.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Libellé")) }
                            th { (crate::langue::tr("Date")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for a in &affaires {
                            tr {
                                td { a href=(format!("/marges/{}", a.rowid)) { (a.libelle.clone()) } }
                                td .ref {
                                    (a.date_marge.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td { (pastille(a.statut)) }
                                td {
                                    a href=(format!("/marges/{}", a.rowid)) class="petit secondaire" {
                                        (crate::langue::tr("Ouvrir"))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// La fiche d'une affaire, avec ses lignes et ses taux.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let a = marge::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("affaire {id} introuvable")))?;
    let lignes = marge::lignes(&app.pool, entity, id)?;
    let (vente, achat, taux) = marge::totaux(&app.pool, entity, id)?;

    Ok(ui::page(
        "marges",
        "marges",
        &a.libelle,
        html! {
            h1 { (a.libelle.clone()) }
            p .sous-titre {
                @if let Some(r) = &a.ref_ {
                    (r.clone()) " · "
                }
                (a.date_marge.clone().unwrap_or_else(|| "—".to_string()))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Vente HT"),
                    &ui::euros(vente),
                    Some(&crate::langue::tr("ce qu'on facture")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Achat HT"),
                    &ui::euros(achat),
                    Some(&crate::langue::tr("ce que ça coûte")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Taux de marque"),
                    &taux.map(|t| format!("{} %", t.round_dp(1))).unwrap_or_else(|| "n/a".to_string()),
                    Some(&crate::langue::tr("marge rapportée à la vente — pas une moyenne des lignes")),
                    taux.map(|t| t < Decimal::ZERO).unwrap_or(false),
                ))
            }

            h2 { (crate::langue::tr("Lignes")) }
            @if lignes.is_empty() {
                (ui::vide("Aucune ligne.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Désignation")) }
                            th { (crate::langue::tr("Qté")) }
                            th { (crate::langue::tr("Vente HT")) }
                            th { (crate::langue::tr("Achat HT")) }
                            th { (crate::langue::tr("Marge")) }
                            th { (crate::langue::tr("Taux de marge")) }
                        }
                    }
                    tbody {
                        @for l in &lignes {
                            @let t = marge::taux_marge_dolibarr(l);
                            tr {
                                td { (l.designation.clone()) }
                                td .num { (l.quantite.normalize().to_string()) }
                                td .num { (ui::euros(l.prix_vente_ht)) }
                                td .num { (ui::euros(l.prix_achat_ht)) }
                                td .num { (ui::euros(marge::marge_ht(l))) }
                                td .num {
                                    (t.map(|v| format!("{} %", v.round_dp(1)))
                                        .unwrap_or_else(|| "n/a".to_string()))
                                }
                            }
                        }
                    }
                }
                p class="aide petite haut" {
                    (crate::langue::tr("Le taux de marge se rapporte au prix d'achat, le taux de marque au prix de vente. Un achat nul rend le taux de marge incalculable : il s'affiche n/a, jamais zéro."))
                }
            }

            h2 { (crate::langue::tr("Actions")) }
            @if a.statut == marge::STATUT_BROUILLON {
                form method="post" action=(format!("/marges/{}/ligne", a.rowid)) {
                    label for="designation" { (crate::langue::tr("Ligne")) } " "
                    input type="text" name="designation" id="designation" required style="width:240px";
                    " "
                    label for="quantite" { (crate::langue::tr("Qté")) } " "
                    input type="text" name="quantite" id="quantite" value="1" style="width:70px" required;
                    " "
                    label for="prix_vente_ht" { (crate::langue::tr("Vente")) } " "
                    input type="text" name="prix_vente_ht" id="prix_vente_ht" required style="width:100px";
                    " "
                    label for="prix_achat_ht" { (crate::langue::tr("Achat")) } " "
                    input type="text" name="prix_achat_ht" id="prix_achat_ht" required style="width:100px";
                    " "
                    button type="submit" class="petit" { (crate::langue::tr("Ajouter")) }
                }
                form method="post" action=(format!("/marges/{}/valider", a.rowid)) class="haut" {
                    button type="submit" { (crate::langue::tr("Valider l'affaire")) }
                }
                p class="aide" {
                    (crate::langue::tr("Valider fige les lignes : une marge validée est un constat, pas un brouillon."))
                }
            } @else {
                p class="aide" {
                    (crate::langue::tr("Cette affaire est validée. Ses lignes ne bougent plus."))
                }
            }
        },
    ))
}

/// Cree une affaire avec sa premiere ligne.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let login = crate::utilisateur::courant()
        .ok_or_else(|| Erreur("aucun utilisateur en session".into()))?;
    let entity = entite::courante();
    let auteur = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;

    let new = NouvelleMarge {
        libelle: lire("libelle"),
        ref_: None,
        fk_soc: lire("fk_soc").parse::<i64>().ok(),
        fk_projet: None,
        date_marge: None,
        fk_user_author: auteur,
    };
    let id = marge::creer(&app.pool, entity, &new)?;

    marge::ajouter_ligne(&app.pool, entity, id, &ligne_depuis(&f))?;
    Ok(Redirect::to(&format!("/marges/{id}")))
}

/// Ajoute une ligne a une affaire.
pub async fn ajouter_ligne(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    marge::ajouter_ligne(&app.pool, entite::courante(), id, &ligne_depuis(&f))?;
    Ok(Redirect::to(&format!("/marges/{id}")))
}

/// Valide une affaire.
pub async fn valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    marge::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/marges/{id}")))
}

/// Une ligne lue dans un formulaire.
fn ligne_depuis(f: &HashMap<String, String>) -> NouvelleLigneMarge {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();
    let decimal = |cle: &str| lire(cle).parse::<Decimal>().unwrap_or_default();

    NouvelleLigneMarge {
        fk_product: None,
        designation: lire("designation"),
        quantite: decimal("quantite"),
        prix_vente_ht: decimal("prix_vente_ht"),
        prix_achat_ht: decimal("prix_achat_ht"),
    }
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        marge::STATUT_VALIDE => ("p-payee", "validée"),
        _ => ("p-brouillon", "brouillon"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_ligne_illisible_ne_fait_pas_tomber_l_ecran() {
        // Un champ vide vaut zero : c'est le module qui refuse ensuite, avec un
        // message. Laisser paniquer l'ecran sur un `parse` serait pire.
        let f = std::collections::HashMap::new();
        let l = ligne_depuis(&f);
        assert_eq!(l.quantite, Decimal::ZERO);
        assert!(l.designation.is_empty());
    }
}
