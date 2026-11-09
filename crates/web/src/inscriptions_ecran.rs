//! Inscriptions a un evenement : qui vient, et qui est venu.
//!
//! Dolibarr tient `llx_eventorganization_registration`. Une inscription se
//! confirme, s'annule, puis se pointe le jour J — trois gestes distincts, et
//! c'est le pointage qui fait foi, pas la confirmation.
//!
//! L'evenement lui-meme vit dans l'agenda (`llx_actioncomm`) : l'ecran le lit,
//! il ne le cree pas.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use axum::Form;
use db::evenement::{self, NouvelleInscription};
use maud::html;

use crate::*;

pub(crate) async fn inscriptions(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let liste = evenement::list(p, entity, 300)?;
    let evenements = evenement::evenements(p, entity, 200).unwrap_or_default();

    Ok(ui::page(
        "inscriptions",
        "liste",
        "Inscriptions",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Inscriptions aux événements")) }
            p .sous-titre {
                (crate::langue::tr("Une inscription se confirme, s'annule, puis se pointe le jour J. C'est le pointage qui fait foi."))
            }

            h2 {(crate::langue::tr("Nouvelle inscription")) }
            @if evenements.is_empty() {
                (ui::vide("Aucun événement à venir dans l'agenda.", Some(("/agenda", "Voir l'agenda"))))
            } @else {
                form method="post" action="/inscriptions" {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Événement")) }
                                td {
                                    select name="evenement" required.pleine {
                                        @for e in &evenements {
                                            option value=(e.0) { (e.1.clone()) }
                                        }
                                    }
                                }
                            }
                            tr { th {(crate::langue::tr("Email")) } td { input type="email" name="email" required .pleine; } }
                            tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" .pleine; } }
                            tr { th {(crate::langue::tr("Prénom")) } td { input type="text" name="prenom" .pleine; } }
                            tr { th {(crate::langue::tr("Société")) } td { input type="text" name="societe" .pleine; } }
                        }
                    }
                    div .actions { button type="submit" {(crate::langue::tr("Inscrire")) } }
                }
            }

            h2 {(crate::langue::tr("Inscriptions")) }
            @if liste.is_empty() {
                (ui::vide("Aucune inscription.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Événement")) }
                            th {(crate::langue::tr("Nom")) }
                            th {(crate::langue::tr("Email")) }
                            th { "État" }
                            th { "" }
                        }
                    }
                    tbody {
                        @for i in &liste {
                            tr {
                                td .ref { (i.evenement_label.clone().unwrap_or_default()) }
                                td { (i.lastname.clone().unwrap_or_default()) " " (i.firstname.clone().unwrap_or_default()) }
                                td .ref { (i.email.clone().unwrap_or_default()) }
                                td { (evenement::libelle_statut(i.statut)) }
                                td {
                                    @if i.statut == 0 {
                                        form method="post" action={ "/inscriptions/" (i.rowid) "/confirmer" } {
                                            button type="submit" class="petit" {(crate::langue::tr("Confirmer")) }
                                        }
                                    } @else if i.statut != 9 {
                                        form method="post" action={ "/inscriptions/" (i.rowid) "/present" } {
                                            button type="submit" class="petit" {(crate::langue::tr("Marquer présent")) }
                                        }
                                    }
                                }
                                td {
                                    form method="post" action={ "/inscriptions/" (i.rowid) "/annuler" } {
                                        button type="submit" class="petit secondaire" {(crate::langue::tr("Annuler")) }
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

pub(crate) async fn inscrire(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let inscription = NouvelleInscription {
        fk_soc: None,
        fk_actioncomm: champ("evenement")
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| Erreur("événement obligatoire".into()))?,
        email: champ("email").ok_or_else(|| Erreur("email obligatoire".into()))?,
        firstname: champ("prenom"),
        lastname: champ("nom"),
        company: champ("societe"),
        date_subscription: None,
        fk_country: None,
        note: None,
    };
    evenement::inscrire(&app.pool, entite::courante(), &inscription)?;
    Ok(Redirect::to("/inscriptions?ok=inscrit"))
}

pub(crate) async fn inscrire_confirmer(
    State(app): State<App>,
    Path(id): Path<i64>,
) -> Res<Redirect> {
    evenement::confirmer(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/inscriptions?ok=confirme"))
}

pub(crate) async fn inscrire_annuler(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    evenement::annuler(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/inscriptions?ok=annule"))
}

pub(crate) async fn inscrire_present(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    evenement::marquer_present(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/inscriptions?ok=present"))
}
