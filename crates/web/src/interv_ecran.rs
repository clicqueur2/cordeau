//! Ecran des interventions : le bon de travail.
//!
//! Dolibarr a `fichinter.php`. C'est la piece de l'artisan : « je suis passe
//! chez le client le 5, voila ce que j'ai fait, ca a pris deux heures ». Elle se
//! distingue du devis par sa simplicite : pas de prix, pas de totaux, une duree
//! et une description.
//!
//! La duree est ecrite **en minutes** dans toute la base, en-tete comme lignes.
//! L'ecran l'affiche en heures et minutes, parce que « 150 » ne se lit pas.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::fichinter::{self, NewFichinter};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// Une duree en minutes, ecrite en heures et minutes.
///
/// `150` devient `2 h 30`, `45` devient `45 min`. Une duree absente n'est pas
/// zero : c'est une duree qui n'a pas ete saisie, et l'ecran le dit.
fn duree(minutes: i64) -> String {
    if minutes <= 0 {
        return "—".to_string();
    }
    let heures = minutes / 60;
    let reste = minutes % 60;
    match (heures, reste) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m:02}"),
    }
}

/// Des minutes portees par un `Decimal`, ramenees a l'entier.
///
/// La colonne est un `double` en base : la valeur peut arriver en `150.0`.
/// Tronquer est le bon choix — une demi-minute n'a pas de sens sur un bon de
/// travail, et arrondir au superieur gonflerait le temps facture.
fn minutes(d: rust_decimal::Decimal) -> i64 {
    d.trunc().to_string().parse::<i64>().unwrap_or(0)
}

/// La liste.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let interventions = fichinter::list(&app.pool, entity, 200, 0)?;

    Ok(ui::page(
        "interventions",
        "interventions",
        "Interventions",
        html! {
            h1 { (crate::langue::tr("Interventions")) }
            p .sous-titre {
                (crate::langue::tr("Le bon de travail : qui, quand, ce qui a été fait, et combien de temps ça a pris."))
            }

            @if interventions.is_empty() {
                (ui::vide(
                    "Aucune intervention. Elle se crée depuis un tiers, avec sa date de passage.",
                    None,
                ))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Client")) }
                            th { (crate::langue::tr("Passage")) }
                            th { (crate::langue::tr("Durée")) }
                            th { (crate::langue::tr("État")) }
                        }
                    }
                    tbody {
                        @for i in &interventions {
                            @let minutes = i.duree
                                .map(minutes)
                                .unwrap_or(0);
                            tr {
                                td .ref {
                                    a href=(format!("/interventions/{}", i.rowid)) { (i.ref_.clone()) }
                                }
                                td { (i.tiers.clone().unwrap_or_else(|| "—".to_string())) }
                                td .ref { (i.datei.clone().unwrap_or_else(|| "—".to_string())) }
                                td .ref { (duree(minutes)) }
                                td { (pastille(i.statut)) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// La fiche d'une intervention, avec ses actions.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let i = fichinter::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("intervention {id} introuvable")))?;
    let lignes = fichinter::lines(&app.pool, entity, id)?;
    let totale = fichinter::duree_totale(&app.pool, entity, id)?;

    Ok(ui::page(
        "interventions",
        "interventions",
        &i.ref_,
        html! {
            h1 { (i.ref_.clone()) }
            p .sous-titre {
                (i.tiers.clone().unwrap_or_else(|| "—".to_string()))
                @if let Some(r) = &i.ref_client {
                    " · " (crate::langue::tr("référence client")) " " (r.clone())
                }
            }

            table {
                tbody {
                    tr {
                        th { (crate::langue::tr("État")) }
                        td { (pastille(i.statut)) }
                    }
                    tr {
                        th { (crate::langue::tr("Passage")) }
                        td .ref { (i.datei.clone().unwrap_or_else(|| "—".to_string())) }
                    }
                    tr {
                        th { (crate::langue::tr("Durée totale")) }
                        td { (duree(minutes(totale))) }
                    }
                    @if let Some(d) = &i.datee {
                        tr {
                            th { (crate::langue::tr("Clôturée le")) }
                            td .ref { (d.clone()) }
                        }
                    }
                }
            }

            @if let Some(d) = i.description.clone().filter(|d| !d.trim().is_empty()) {
                h2 { (crate::langue::tr("Ce qui a été fait")) }
                p { (d) }
            }

            h2 { (crate::langue::tr("Lignes")) }
            @if lignes.is_empty() {
                (ui::vide("Aucune ligne : l'intervention se raconte par sa description.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Date")) }
                            th { (crate::langue::tr("Ce qui a été fait")) }
                            th { (crate::langue::tr("Durée")) }
                        }
                    }
                    tbody {
                        @for l in &lignes {
                            tr {
                                td .ref { (l.date.clone().unwrap_or_else(|| "—".to_string())) }
                                td {
                                    (l.description.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td .ref { (duree(l.duree)) }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Actions")) }
            @if i.statut == fichinter::STATUT_BROUILLON {
                form method="post" action=(format!("/interventions/{}/valider", i.rowid)) {
                    button type="submit" { (crate::langue::tr("Valider")) }
                }
            } @else if i.statut == fichinter::STATUT_VALIDE {
                form method="post" action=(format!("/interventions/{}/cloturer", i.rowid)) {
                    label for="date_fin" { (crate::langue::tr("Date de fin")) } " "
                    input type="date" name="date_fin" id="date_fin" required;
                    " "
                    button type="submit" { (crate::langue::tr("Clôturer")) }
                }
            } @else {
                p class="aide" {
                    (crate::langue::tr("Cette intervention est clôturée."))
                }
            }
        },
    ))
}

/// Cree une intervention depuis la fiche du tiers.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let fk_soc = lire("fk_soc")
        .parse::<i64>()
        .map_err(|_| Erreur("tiers manquant".into()))?;
    let datei = lire("datei");
    if datei.is_empty() {
        return Err(Erreur("la date de passage est obligatoire".into()));
    }

    let mut new = NewFichinter::new(fk_soc);
    new.datei = datei;
    new.description = lire("description");
    new.ref_client = Some(lire("ref_client")).filter(|s| !s.is_empty());

    let id = fichinter::create(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to(&format!("/interventions/{id}")))
}

/// Valide une intervention.
pub async fn valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    fichinter::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/interventions/{id}")))
}

/// Cloture une intervention.
pub async fn cloturer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let date_fin = f
        .get("date_fin")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("date de fin manquante".into()))?;
    fichinter::cloturer(&app.pool, entite::courante(), id, &date_fin)?;
    Ok(Redirect::to(&format!("/interventions/{id}")))
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        fichinter::STATUT_CLOTURE => ("p-payee", "clôturée"),
        fichinter::STATUT_VALIDE => ("p-partielle", "validée"),
        _ => ("p-brouillon", "brouillon"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_duree_se_lit_en_heures_et_minutes() {
        assert_eq!(duree(150), "2 h 30");
        assert_eq!(duree(120), "2 h");
        assert_eq!(duree(45), "45 min");
        // Zero et negatif ne s'affichent pas « 0 min » : une duree non saisie
        // n'est pas une duree nulle.
        assert_eq!(duree(0), "—");
        assert_eq!(duree(-5), "—");
    }
}
