//! Ecran de la base de connaissances : ce qu'on garde d'un probleme resolu.
//!
//! Dolibarr a `knowledgemanagement`. Une fiche est une **question** et sa
//! reponse : pas un article, pas une documentation. Elle sert la fois suivante,
//! quand le meme probleme revient chez un autre client.
//!
//! # Une recherche qui cherche vraiment
//!
//! La recherche passe par `db::connaissances::rechercher`, qui echappe les
//! jokers de `LIKE`. Sans cela, taper `%` ramenerait toute la base — l'erreur la
//! plus banale du genre, et la plus invisible. L'ecran n'a rien a faire de
//! particulier : la garantie est dans le module.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use axum::Form;
use db::connaissances::{self, NewFiche};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La liste, avec recherche.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let terme = q.get("q").map(|s| s.trim().to_string()).unwrap_or_default();

    let fiches = if terme.is_empty() {
        connaissances::list(&app.pool, entity, 200, 0)?
    } else {
        connaissances::rechercher(&app.pool, entity, &terme, 200)?
    };

    Ok(ui::page(
        "connaissances",
        "connaissances",
        "Base de connaissances",
        html! {
            h1 { (crate::langue::tr("Base de connaissances")) }
            p .sous-titre {
                (crate::langue::tr("Ce qu'on garde d'un problème résolu, pour la fois suivante. Une question, une réponse."))
            }

            h2 { (crate::langue::tr("Chercher")) }
            form method="get" action="/connaissances" {
                input type="search" name="q" value=(terme) placeholder=(crate::langue::tr("un mot de la question ou de la réponse"))
                     .champ-large;
                " "
                button type="submit" class="petit" { (crate::langue::tr("Chercher")) }
            }

            h2 { (crate::langue::tr("Nouvelle fiche")) }
            form method="post" action="/connaissances" {
                label for="question" { (crate::langue::tr("Question")) }
                textarea name="question" id="question" rows="2" required
                         style="width:100%;font-size:14px"
                         placeholder=(crate::langue::tr("Comment relancer une facture impayée ?")) {}
                label for="answer" { (crate::langue::tr("Réponse")) }
                textarea name="answer" id="answer" rows="4"
                        .code.pleine {}
                div .actions {
                    button type="submit" { (crate::langue::tr("Créer la fiche")) }
                }
            }

            h2 { (crate::langue::tr("Fiches")) }
            @if fiches.is_empty() {
                @if terme.is_empty() {
                    (ui::vide("Aucune fiche. La première sert à la deuxième fois.", None))
                } @else {
                    (ui::vide(
                        "Aucune fiche ne correspond. Essayez un mot de la réponse, pas seulement de la question.",
                        Some(("/connaissances", "Voir toutes les fiches")),
                    ))
                }
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Question")) }
                            th { (crate::langue::tr("Ticket")) }
                            th { (crate::langue::tr("Langue")) }
                            th { (crate::langue::tr("État")) }
                        }
                    }
                    tbody {
                        @for f in &fiches {
                            tr {
                                td .ref {
                                    a href=(format!("/connaissances/{}", f.rowid)) { (f.ref_.clone()) }
                                }
                                td { (f.question.clone()) }
                                td .ref {
                                    (f.ticket_ref.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td .ref {
                                    (f.langue.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td {
                                    @if f.est_publiee() {
                                        span class="pastille p-payee" { (crate::langue::tr("publiée")) }
                                    } @else {
                                        span class="pastille p-brouillon" { (crate::langue::tr("brouillon")) }
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

/// La fiche complete.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let f = connaissances::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("fiche {id} introuvable")))?;

    Ok(ui::page(
        "connaissances",
        "connaissances",
        &f.ref_,
        html! {
            h1 { (f.question.clone()) }
            p .sous-titre {
                (f.ref_.clone())
                @if let Some(l) = &f.langue {
                    " · " (l.clone())
                }
                @if let Some(t) = &f.ticket_ref {
                    " · " (crate::langue::tr("né du ticket")) " " (t.clone())
                }
            }

            @if let Some(url) = &f.url {
                p {
                    a href=(url) { (url) }
                }
            }

            h2 { (crate::langue::tr("Réponse")) }
            @if let Some(r) = f.reponse.clone().filter(|r| !r.trim().is_empty()) {
                pre style="background:#fbfaf8;border:1px solid #e6e2dc;padding:14px;font-size:13.5px;white-space:pre-wrap;margin:0" {
                    (r)
                }
            } @else {
                (ui::vide("Cette fiche n'a pas encore de réponse.", None))
            }

            h2 { (crate::langue::tr("Actions")) }
            @if f.est_publiee() {
                form method="post" action=(format!("/connaissances/{}/depublier", f.rowid)) {
                    button type="submit" class="petit secondaire" {
                        (crate::langue::tr("Dépublier"))
                    }
                }
            } @else {
                form method="post" action=(format!("/connaissances/{}/publier", f.rowid)) {
                    button type="submit" { (crate::langue::tr("Publier")) }
                }
            }
        },
    ))
}

/// Cree une fiche.
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

    let new = NewFiche {
        question: lire("question"),
        answer: Some(lire("answer")).filter(|s| !s.is_empty()),
        url: None,
        lang: Some("fr_FR".to_string()),
        fk_ticket: None,
        fk_c_ticket_category: None,
        fk_user_creat: auteur,
    };
    let id = connaissances::create(&app.pool, entity, &new)?;
    Ok(Redirect::to(&format!("/connaissances/{id}")))
}

/// Publie une fiche.
pub async fn publier(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    connaissances::publier(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/connaissances/{id}")))
}

/// Depublie une fiche.
pub async fn depublier(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    connaissances::depublier(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/connaissances/{id}")))
}
