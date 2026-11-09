//! Ressources : le materiel, les salles, les vehicules qu'on affecte a un
//! chantier.
//!
//! Dolibarr tient `llx_resource` et `llx_element_resources`. Une ressource n'est
//! pas un produit : elle ne se vend pas, elle s'affecte. C'est ce qui permet de
//! savoir qu'une nacelle est prise la semaine prochaine — et donc qu'elle ne
//! l'est pas ailleurs.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use axum::Form;
use db::ressource::{self, NewRessource};
use maud::html;
use rust_decimal::Decimal;

use crate::*;

pub(crate) async fn ressources(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let liste = ressource::list(p, entity, 300, 0)?;

    Ok(ui::page(
        "ressources",
        "liste",
        "Ressources",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Ressources")) }
            p .sous-titre {
                (crate::langue::tr("Le matériel, les salles, les véhicules : ce qui s'affecte à un chantier au lieu de se vendre."))
            }

            h2 {(crate::langue::tr("Nouvelle ressource")) }
            form method="post" action="/ressources" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Réf.")) } td { input type="text" name="ref" required.champ-moyen placeholder="NACELLE-01"; } }
                        tr { th {(crate::langue::tr("Désignation")) } td { input type="text" name="description" .pleine; } }
                        tr {
                            th {(crate::langue::tr("Type")) }
                            td {
                                input type="text" name="type" .pleine
                                      placeholder="MATERIEL, SALLE, VEHICULE…";
                            }
                        }
                        tr { th {(crate::langue::tr("Ville")) } td { input type="text" name="ville" .pleine; } }
                        tr { th {(crate::langue::tr("Téléphone")) } td { input type="text" name="tel" .pleine; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer")) } }
            }

            h2 {(crate::langue::tr("Ressources enregistrées")) }
            @if liste.is_empty() {
                (ui::vide("Aucune ressource.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Réf.")) }
                            th {(crate::langue::tr("Désignation")) }
                            th {(crate::langue::tr("Type")) }
                            th {(crate::langue::tr("Ville")) }
                            th { "État" }
                        }
                    }
                    tbody {
                        @for r in &liste {
                            tr {
                                td .ref { a href={ "/ressources/" (r.rowid) } { (r.ref_.clone()) } }
                                td { (r.description.clone().unwrap_or_default()) }
                                td { (r.type_label.clone().unwrap_or_default()) }
                                td { (r.town.clone().unwrap_or_default()) }
                                td {
                                    @if r.busy {
                                        span .pastille.p-validee {(crate::langue::tr("affectée")) }
                                    } @else {
                                        span .pastille.p-payee {(crate::langue::tr("libre")) }
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

pub(crate) async fn ressource_fiche(State(app): State<App>, Path(id): Path<i64>) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let r = ressource::get(p, entity, id)?
        .ok_or_else(|| Erreur(format!("ressource {id} introuvable")))?;
    let chantiers = db::projet::list(p, entity, 200, 0)?;
    let liens = ressource::combien_de_lies(p, entity, id).unwrap_or(0);

    Ok(ui::page(
        "ressources",
        "liste",
        &r.ref_,
        html! {
            h1 { (r.ref_.clone()) }
            p .sous-titre {
                (r.description.clone().unwrap_or_default())
                @if let Some(v) = &r.type_label { " — " (v) }
            }

            h2 {(crate::langue::tr("Affecter à un chantier")) }
            @if chantiers.is_empty() {
                (ui::vide("Aucun chantier ouvert.", Some(("/chantiers", "Voir les chantiers"))))
            } @else {
                form method="post" action={ "/ressources/" (id) "/affecter" } .actions {
                    select name="projet" required {
                        @for c in &chantiers {
                            option value=(c.rowid) { (c.ref_.clone()) " — " (c.title.clone()) }
                        }
                    }
                    input type="text" name="duree" placeholder="durée (h)".champ-court;
                    button type="submit" {(crate::langue::tr("Affecter")) }
                }
            }

            h2 {(crate::langue::tr("Affectations")) }
            @if liens == 0 {
                (ui::vide("Aucune affectation : la ressource est libre.", None))
            } @else {
                p.sous-titre {
                    (liens) " " (pluriel(liens as usize, "affectation", "affectations"))
                }
            }
        },
    ))
}

pub(crate) async fn ressource_creer(
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
    let nouveau = NewRessource {
        ref_: champ("ref").ok_or_else(|| Erreur("référence obligatoire".into()))?,
        asset_number: None,
        description: champ("description"),
        fk_code_type_resource: champ("type"),
        address: None,
        zip: None,
        town: champ("ville"),
        phone: champ("tel"),
        email: None,
        url: None,
        max_users: None,
        fk_user_author: auteur,
    };
    let id = ressource::creer(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to(&format!("/ressources/{id}?ok=cree")))
}

pub(crate) async fn ressource_affecter(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let projet: i64 = f
        .get("projet")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| Erreur("chantier obligatoire".into()))?;
    let duree = f
        .get("duree")
        .map(|v| v.trim().replace(',', "."))
        .and_then(|v| v.parse::<Decimal>().ok());
    ressource::attacher(&app.pool, entite::courante(), id, projet, "projet", duree)?;
    Ok(Redirect::to(&format!("/ressources/{id}?ok=affecte")))
}
