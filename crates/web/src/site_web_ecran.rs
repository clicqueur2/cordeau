//! Ecran du site web : les sites et leurs pages.
//!
//! Dolibarr a `website.php`. Un site porte des pages, et l'adresse d'une page
//! est son identifiant public : deux pages ne peuvent pas partager la meme
//! adresse dans un meme site. C'est le seul index que le module impose, et il
//! est legitime — une adresse qui change casse les liens deja envoyes.
//!
//! # Ce que cet ecran ne fait pas
//!
//! Il n'imprime pas de HTML : le rendu d'un site est un sujet a part entiere
//! (gabarits, menus, feuilles de style). Ici on tient le **contenu** — titre,
//! description, corps — et on decide ce qui est en ligne.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::site_web::{self, NewPage, NewSite};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La liste des sites.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let sites = site_web::sites(&app.pool, entity, 100)?;

    Ok(ui::page(
        "sites",
        "sites",
        "Site web",
        html! {
            h1 { (crate::langue::tr("Site web")) }
            p .sous-titre {
                (crate::langue::tr("Les sites et leurs pages. L'adresse d'une page est son identifiant public : elle ne se partage pas dans un même site."))
            }

            h2 { (crate::langue::tr("Nouveau site")) }
            form method="post" action="/sites" {
                label for="ref" { (crate::langue::tr("Référence")) } " "
                input type="text" name="ref" id="ref" required style="width:220px"
                      placeholder="maconnerie-architecture";
                " "
                label for="lang" { (crate::langue::tr("Langue")) } " "
                input type="text" name="lang" id="lang" value="fr_FR" class="champ-court";
                " "
                label for="virtualhost" { (crate::langue::tr("Domaine")) } " "
                input type="text" name="virtualhost" id="virtualhost" style="width:220px";
                " "
                button type="submit" { (crate::langue::tr("Créer")) }
            }
            p class="aide petite sous-champ" {
                (crate::langue::tr("La référence est le nom interne du site. Elle doit être unique : deux sites de même nom rendraient les journaux illisibles."))
            }

            h2 { (crate::langue::tr("Sites")) }
            @if sites.is_empty() {
                (ui::vide("Aucun site enregistré.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Domaine")) }
                            th { (crate::langue::tr("Langue")) }
                            th { (crate::langue::tr("Pages")) }
                            th { (crate::langue::tr("Vues")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for s in &sites {
                            tr {
                                td .ref {
                                    a href=(format!("/sites/{}", s.rowid)) { (s.ref_.clone()) }
                                }
                                td .ref {
                                    (s.virtualhost.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td .ref {
                                    (s.langue.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td .num { (s.pages) }
                                td .num { (s.vues) }
                                td {
                                    @if s.est_en_ligne() {
                                        span class="pastille p-payee" { (crate::langue::tr("en ligne")) }
                                    } @else {
                                        span class="pastille p-brouillon" { (crate::langue::tr("hors ligne")) }
                                    }
                                }
                                td {
                                    a href=(format!("/sites/{}", s.rowid)) class="petit secondaire" {
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

/// Les pages d'un site.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let pages = site_web::pages(&app.pool, entity, id)?;
    let site = site_web::sites(&app.pool, entity, 500)?
        .into_iter()
        .find(|s| s.rowid == id)
        .ok_or_else(|| Erreur(format!("site {id} introuvable")))?;

    Ok(ui::page(
        "sites",
        "sites",
        &site.ref_,
        html! {
            h1 { (site.ref_.clone()) }
            p .sous-titre {
                @if let Some(d) = &site.description {
                    (d.clone()) " · "
                }
                (crate::langue::tr("pages")) " : " (pages.len())
            }

            h2 { (crate::langue::tr("Nouvelle page")) }
            form method="post" action=(format!("/sites/{}/pages", site.rowid)) {
                label for="pageurl" { (crate::langue::tr("Adresse")) } " "
                span class="muet" { "/" }
                input type="text" name="pageurl" id="pageurl" required style="width:220px"
                      placeholder="contact";
                " "
                label for="title" { (crate::langue::tr("Titre")) } " "
                input type="text" name="title" id="title" style="width:260px";
                br;
                label for="description" { (crate::langue::tr("Description")) } " "
                input type="text" name="description" id="description" style="width:420px";
                br;
                label for="content" { (crate::langue::tr("Contenu")) }
                textarea name="content" id="content" rows="5" required
                         style="width:100%;font-family:ui-monospace,monospace;font-size:13px" {}
                div .actions {
                    button type="submit" { (crate::langue::tr("Créer la page")) }
                }
                p style="color:#78736c;font-size:13px;margin:0" {
                    (crate::langue::tr("Une page naît hors ligne. Elle ne devient publique que lorsque vous la publiez."))
                }
            }

            h2 { (crate::langue::tr("Pages")) }
            @if pages.is_empty() {
                (ui::vide("Ce site n'a aucune page.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Adresse")) }
                            th { (crate::langue::tr("Titre")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for p in &pages {
                            tr {
                                td .ref { "/" (p.adresse.clone()) }
                                td {
                                    (p.titre.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td {
                                    @if p.est_en_ligne() {
                                        span class="pastille p-payee" { (crate::langue::tr("en ligne")) }
                                    } @else {
                                        span class="pastille p-brouillon" { (crate::langue::tr("hors ligne")) }
                                    }
                                }
                                td {
                                    @if p.est_en_ligne() {
                                        form method="post" action=(format!("/pages/{}/depublier", p.rowid)) class="en-ligne" {
                                            button type="submit" class="petit secondaire" {
                                                (crate::langue::tr("Dépublier"))
                                            }
                                        }
                                    } @else {
                                        form method="post" action=(format!("/pages/{}/publier", p.rowid)) class="en-ligne" {
                                            button type="submit" class="petit" {
                                                (crate::langue::tr("Publier"))
                                            }
                                        }
                                    }
                                    " "
                                    form method="post" action=(format!("/pages/{}/supprimer", p.rowid)) class="en-ligne" {
                                        button type="submit" class="petit secondaire" {
                                            (crate::langue::tr("Supprimer"))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Supprimer le site")) }
            @if pages.is_empty() {
                form method="post" action=(format!("/sites/{}/supprimer", site.rowid)) {
                    button type="submit" class="petit secondaire" {
                        (crate::langue::tr("Supprimer ce site vide"))
                    }
                }
            } @else {
                p class="aide" {
                    (crate::langue::tr("Ce site porte des pages : il ne se supprime pas. Vider un site de vingt pages sur un clic n'est pas une récupération possible — supprimez les pages une par une si c'est bien ce que vous voulez."))
                }
            }
        },
    ))
}

/// Cree un site.
pub async fn creer_site(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let new = NewSite {
        ref_: lire("ref"),
        description: None,
        lang: Some(lire("lang")).filter(|s| !s.is_empty()),
        virtualhost: Some(lire("virtualhost")).filter(|s| !s.is_empty()),
    };
    let id = site_web::creer_site(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to(&format!("/sites/{id}")))
}

/// Cree une page dans un site.
pub async fn creer_page(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let new = NewPage {
        fk_website: id,
        pageurl: lire("pageurl"),
        title: Some(lire("title")).filter(|s| !s.is_empty()),
        description: Some(lire("description")).filter(|s| !s.is_empty()),
        keywords: None,
        lang: Some("fr_FR".to_string()),
        content: lire("content"),
        // Hors ligne a la creation : une page en chantier ne doit pas etre
        // publique parce qu'un formulaire a ete valide.
        status: site_web::STATUT_HORS_LIGNE,
    };
    site_web::creer_page(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to(&format!("/sites/{id}")))
}

/// Publie une page.
pub async fn publier(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let entity = entite::courante();
    let p = site_web::page(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("page {id} introuvable")))?;
    site_web::publier_page(&app.pool, entity, id)?;
    Ok(Redirect::to(&format!("/sites/{}", p.fk_website)))
}

/// Depublie une page.
pub async fn depublier(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let entity = entite::courante();
    let p = site_web::page(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("page {id} introuvable")))?;
    site_web::depublier_page(&app.pool, entity, id)?;
    Ok(Redirect::to(&format!("/sites/{}", p.fk_website)))
}

/// Supprime une page.
pub async fn supprimer_page(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let entity = entite::courante();
    let p = site_web::page(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("page {id} introuvable")))?;
    site_web::supprimer_page(&app.pool, entity, id)?;
    Ok(Redirect::to(&format!("/sites/{}", p.fk_website)))
}

/// Supprime un site vide.
pub async fn supprimer_site(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    site_web::supprimer_site(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/sites"))
}
