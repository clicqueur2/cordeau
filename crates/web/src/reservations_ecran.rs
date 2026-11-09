//! Reservations : un calendrier, des plages d'ouverture, des creneaux.
//!
//! Dolibarr tient `llx_bookcal_calendar` et `llx_bookcal_availabilities`. Le
//! calcul des creneaux libres est dans `db::reservation` : il decoupe la plage
//! du jour en tranches de la duree demandee et retire celles qui chevauchent une
//! reservation existante. L'ecran ne recalcule rien, il affiche.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use axum::Form;
use db::reservation::{self, NouveauCalendrier, NouvellePlage};
use maud::html;

use crate::*;

pub(crate) async fn reservations(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    // Une base sans le module BookCal n'a pas la table : on le dit, plutot que
    // de laisser remonter une erreur SQL que personne ne comprend.
    if !reservation::installe(p)? {
        return Ok(ui::page(
            "reservations",
            "liste",
            "Réservations",
            html! {
                h1 {(crate::langue::tr("Réservations")) }
                (ui::vide(
                    "Le module BookCal n'est pas installé sur cette base : il n'y a pas de calendrier à afficher.",
                    Some(("/parametres", "Voir les modules")),
                ))
                p .aide.petite {
                    (crate::langue::tr("La table llx_bookcal_calendar est créée par Dolibarr à l'activation du module. Rien n'est cassé : il n'y a simplement pas de calendrier."))
                }
            },
        ));
    }
    let calendriers = reservation::calendriers(p, entity, 100)?;
    let clients = db::party::list(p, entity, 300, 0)?;

    Ok(ui::page(
        "reservations",
        "liste",
        "Réservations",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Réservations")) }
            p .sous-titre {
                (crate::langue::tr("Un calendrier, des plages d'ouverture, et les créneaux libres se calculent — ils ne se saisissent pas."))
            }

            h2 {(crate::langue::tr("Nouveau calendrier")) }
            form method="post" action="/reservations" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Libellé")) } td { input type="text" name="label" required .pleine placeholder="Atelier, salle, nacelle…"; } }
                        tr { th {(crate::langue::tr("Description")) } td { input type="text" name="description" .pleine; } }
                        tr {
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" .pleine {
                                    option value="" {(crate::langue::tr("— aucun —")) }
                                    @for c in &clients {
                                        option value=(c.rowid) { (c.nom.clone()) }
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le calendrier")) } }
            }

            h2 {(crate::langue::tr("Calendriers")) }
            @if calendriers.is_empty() {
                (ui::vide("Aucun calendrier.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Libellé")) } th {(crate::langue::tr("Description")) } th { "" } } }
                    tbody {
                        @for c in &calendriers {
                            tr {
                                td .ref { (c.reference.clone()) }
                                td { a href={ "/reservations/" (c.rowid) } { (c.libelle.clone()) } }
                                td { (c.description.clone().unwrap_or_default()) }
                                td {
                                    a .bouton.secondaire.petit href={ "/reservations/" (c.rowid) } {(crate::langue::tr("Ouvrir")) }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn reservation_calendrier(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let calendrier = reservation::calendriers(p, entity, 200)?
        .into_iter()
        .find(|c| c.rowid == id)
        .ok_or_else(|| Erreur(format!("calendrier {id} introuvable")))?;
    let plages = reservation::plages(p, entity, id)?;
    let jour = aujourdhui(p)?;
    let prises = reservation::reservations(p, entity, id, &jour)?;
    let libres = reservation::creneaux_libres(p, entity, id, &jour, 60).unwrap_or_default();

    Ok(ui::page(
        "reservations",
        "liste",
        &calendrier.libelle,
        html! {
            (message(q.get("ok")))
            h1 { (calendrier.libelle.clone()) }
            p .sous-titre { (calendrier.reference.clone()) }

            h2 {(crate::langue::tr("Plages d'ouverture")) }
            @if plages.is_empty() {
                (ui::vide("Aucune plage : sans ouverture, aucun créneau ne peut être proposé.", None))
            } @else {
                table {
                    tbody {
                        @for pl in &plages {
                            tr {
                                td .ref { (pl.jour_debut.clone()) " → " (pl.jour_fin.clone()) }
                                td .num { (pl.heure_debut.clone()) " – " (pl.heure_fin.clone()) }
                            }
                        }
                    }
                }
            }
            form method="post" action={ "/reservations/" (id) "/plage" } .actions {
                input type="date" name="debut" required.champ-moyen;
                input type="date" name="fin" required.champ-moyen;
                input type="time" name="heure_debut" value="08:00";
                input type="time" name="heure_fin" value="18:00";
                button type="submit" class="petit" {(crate::langue::tr("Ajouter la plage")) }
            }

            h2 {(crate::langue::tr("Créneaux libres aujourd'hui")) }
            @if libres.is_empty() {
                (ui::vide("Aucun créneau libre d'une heure aujourd'hui.", None))
            } @else {
                p {
                    @for (i, h) in libres.iter().enumerate() {
                        @if i > 0 { " · " }
                        strong { (h) }
                    }
                }
            }

            h2 {(crate::langue::tr("Réserver")) }
            @if libres.is_empty() {
                p.aide {(crate::langue::tr("Aucun créneau à réserver sur ce jour. Choisissez-en un dans la liste ci-dessus.")) }
            } @else {
                form method="post" action={ "/reservations/" (id) "/reserver" } .actions {
                    input type="hidden" name="jour" value=(jour.clone());
                    select name="heure" required {
                        @for h in &libres {
                            option value=(h) { (h) }
                        }
                    }
                    input type="text" name="duree" value="60" .champ-court;
                    input type="text" name="note" .champ-moyen placeholder="objet";
                    button type="submit" {(crate::langue::tr("Réserver")) }
                }
            }

            h2 {(crate::langue::tr("Réservations")) }
            @if prises.is_empty() {
                (ui::vide("Aucune réservation.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Début")) } th {(crate::langue::tr("Fin")) } th {(crate::langue::tr("Objet")) } } }
                    tbody {
                        @for r in &prises {
                            tr {
                                td .ref { (r.debut.clone()) }
                                td .ref { (r.fin.clone().unwrap_or_default()) }
                                td { (r.libelle.clone()) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn calendrier_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let login = utilisateur::courant().ok_or_else(|| Erreur("session sans utilisateur".into()))?;
    let auteur = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;
    let id = reservation::creer_calendrier(
        &app.pool,
        entite::courante(),
        &NouveauCalendrier {
            label: &champ("label").ok_or_else(|| Erreur("libellé obligatoire".into()))?,
            description: champ("description").as_deref(),
            fk_soc: champ("soc").and_then(|v| v.parse().ok()),
            fk_user_creat: auteur,
            note_public: None,
        },
    )?;
    Ok(Redirect::to(&format!("/reservations/{id}?ok=cree")))
}

pub(crate) async fn plage_creer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| f.get(c).map(|s| s.trim().to_string()).unwrap_or_default();
    let login = utilisateur::courant().ok_or_else(|| Erreur("session sans utilisateur".into()))?;
    let auteur = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;
    reservation::creer_plage(
        &app.pool,
        entite::courante(),
        &NouvellePlage {
            fk_bookcal_calendar: id,
            jour_debut: &champ("debut"),
            jour_fin: &champ("fin"),
            heure_debut: &champ("heure_debut"),
            heure_fin: &champ("heure_fin"),
            fk_user_creat: auteur,
        },
    )?;
    Ok(Redirect::to(&format!("/reservations/{id}?ok=plage")))
}

pub(crate) async fn reserver(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| f.get(c).map(|s| s.trim().to_string()).unwrap_or_default();
    let duree: i64 = champ("duree").parse().unwrap_or(60);
    reservation::reserver(
        &app.pool,
        entite::courante(),
        id,
        &champ("jour"),
        &champ("heure"),
        duree,
        None,
        &champ("note"),
    )?;
    Ok(Redirect::to(&format!("/reservations/{id}?ok=reserve")))
}
