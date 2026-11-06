//! Droits et groupes.
//!
//! Un droit est une ligne de `llx_user_rights`, posee par un groupe ou
//! directement. Le noyau relit les droits **a chaque requete** : retirer un
//! droit prend effet immediatement, sans reconnexion — c'est ce qui rend une
//! revocation utile.
use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use maud::{html, Markup};

use crate::{message, Erreur, Res};

/// Droits d'acces d'un utilisateur, un par ecran.
pub async fn utilisateur_droits(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let u =
        db::user::get(&app.pool, id)?.ok_or_else(|| Erreur("utilisateur introuvable".into()))?;
    let attribues = db::droit::droits(&app.pool, id)?;

    Ok(crate::ui::page(
        "parametres",
        "utilisateurs",
        &format!("Droits de {}", u.login),
        html! {
            (message(q.get("ok")))
            h1 { "Droits de " (u.login) }
            p .sous-titre {(crate::langue::tr("Un administrateur a tous les droits. Pour les autres comptes, chaque case ouvre un ecran."))
            }
            form method="post" action={ "/parametres/utilisateurs/" (id) "/droits" } {
                table {
                    thead { tr { th {(crate::langue::tr("Ecran")) } th {(crate::langue::tr("Autorise")) } } }
                    tbody {
                        @for d in db::droit::DEFINITIONS {
                            tr {
                                td { (d.libelle) }
                                td {
                                    input type="checkbox" name=(format!("d{}", d.id)) value="1"
                                          checked[attribues.contains(&d.id)];
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer les droits")) }
                    a .bouton.secondaire href="/parametres/utilisateurs" {(crate::langue::tr("Retour")) }
                }
            }

            h2 {(crate::langue::tr("Droits hérités des groupes")) }
            @let mes_groupes = db::groupe::groupes_de(&app.pool, crate::entite::courante(), id)?;
            @if mes_groupes.is_empty() {
                p {(crate::langue::tr("Ce compte n'appartient à aucun groupe.")) }
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Groupe")) } th {(crate::langue::tr("Droits portés")) } } }
                    tbody {
                        @for g in &mes_groupes {
                            @let droits_groupe = db::groupe::droits(&app.pool, g.rowid)?;
                            tr {
                                td { a href={ "/parametres/groupes/" (g.rowid) } { (g.nom) } }
                                td {
                                    @for d in db::droit::DEFINITIONS.iter().filter(|d| droits_groupe.contains(&d.id)) {
                                        (d.libelle) " · "
                                    }
                                }
                            }
                        }
                    }
                }
                p class="aide petite" {(crate::langue::tr("Les droits ci-dessus sont accordés par le groupe : pour les retirer, il faut modifier le groupe."))
                }
            }
        },
    ))
}

pub async fn utilisateur_droits_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    for d in db::droit::DEFINITIONS {
        let actif = f.contains_key(&format!("d{}", d.id));
        db::droit::definir(&app.pool, crate::entite::courante(), id, d.id, actif)?;
    }
    Ok(Redirect::to(&format!(
        "/parametres/utilisateurs/{id}/droits?ok=droits"
    )))
}

/// Liste des groupes et creation.
pub async fn groupes(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let liste = db::groupe::list(&app.pool, crate::entite::courante())?;
    Ok(crate::ui::page(
        "parametres",
        "groupes",
        "Groupes",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Groupes")) }
            p .sous-titre {(crate::langue::tr("Un groupe porte des droits ; ses membres en heritent. Un utilisateur peut cumuler plusieurs groupes."))
            }
            @if liste.is_empty() {
                (crate::ui::vide("Aucun groupe.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Groupe")) } th {(crate::langue::tr("Note")) } th { "" } } }
                    tbody {
                        @for g in &liste {
                            tr {
                                td { a href={ "/parametres/groupes/" (g.rowid) } { (g.nom) } }
                                td { (g.note.clone().unwrap_or_default()) }
                                td { a .bouton.petit.secondaire href={ "/parametres/groupes/" (g.rowid) } {(crate::langue::tr("Ouvrir")) } }
                            }
                        }
                    }
                }
            }
            h2 {(crate::langue::tr("Nouveau groupe")) }
            form method="post" action="/parametres/groupes" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" required class="pleine"; } }
                        tr { th {(crate::langue::tr("Note")) } td { input type="text" name="note" class="pleine"; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Creer le groupe")) } }
            }
        },
    ))
}

pub async fn groupe_creer(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let nom = f
        .get("nom")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("nom du groupe obligatoire".into()))?;
    let note = f
        .get("note")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    db::groupe::creer(&app.pool, crate::entite::courante(), &nom, note.as_deref())?;
    Ok(Redirect::to("/parametres/groupes?ok=cree"))
}

/// Fiche d'un groupe : membres et droits.
pub async fn groupe_fiche(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let g = db::groupe::get(&app.pool, crate::entite::courante(), id)?
        .ok_or_else(|| Erreur("groupe introuvable".into()))?;
    let membres = db::groupe::membres(&app.pool, crate::entite::courante(), id)?;
    let droits = db::groupe::droits(&app.pool, id)?;
    let utilisateurs = db::user::list(&app.pool, crate::entite::courante())?;

    Ok(crate::ui::page(
        "parametres",
        "groupes",
        &g.nom,
        html! {
            (message(q.get("ok")))
            h1 { "Groupe " (g.nom) }
            p .sous-titre {(crate::langue::tr("Les membres heritent des droits coches ci-dessous.")) }

            h2 {(crate::langue::tr("Membres")) }
            form method="post" action={ "/parametres/groupes/" (id) "/membres" } {
                table {
                    thead { tr { th {(crate::langue::tr("Utilisateur")) } th {(crate::langue::tr("Membre")) } } }
                    tbody {
                        @for u in &utilisateurs {
                            tr {
                                td { (u.login) }
                                td {
                                    input type="checkbox" name=(format!("u{}", u.rowid)) value="1"
                                          checked[membres.iter().any(|(mid, _)| *mid == u.rowid)];
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Enregistrer les membres")) } }
            }

            h2 {(crate::langue::tr("Droits du groupe")) }
            form method="post" action={ "/parametres/groupes/" (id) "/droits" } {
                table {
                    thead { tr { th {(crate::langue::tr("Ecran")) } th {(crate::langue::tr("Autorise")) } } }
                    tbody {
                        @for d in db::droit::DEFINITIONS {
                            tr {
                                td { (d.libelle) }
                                td {
                                    input type="checkbox" name=(format!("d{}", d.id)) value="1"
                                          checked[droits.contains(&d.id)];
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer les droits du groupe")) }
                    a .bouton.secondaire href="/parametres/groupes" {(crate::langue::tr("Retour")) }
                }
            }
        },
    ))
}

pub async fn groupe_membres_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    for u in db::user::list(&app.pool, crate::entite::courante())? {
        let membre = f.contains_key(&format!("u{}", u.rowid));
        db::groupe::definir_membre(&app.pool, crate::entite::courante(), id, u.rowid, membre)?;
    }
    Ok(Redirect::to(&format!(
        "/parametres/groupes/{id}?ok=membres"
    )))
}

pub async fn groupe_droits_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    for d in db::droit::DEFINITIONS {
        let actif = f.contains_key(&format!("d{}", d.id));
        db::groupe::definir_droit(&app.pool, crate::entite::courante(), id, d.id, actif)?;
    }
    Ok(Redirect::to(&format!("/parametres/groupes/{id}?ok=droits")))
}
