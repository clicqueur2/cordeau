//! Ecran des categories : les etiquettes, et ce qu'elles marquent.
//!
//! Une categorie appartient a une famille : une categorie de clients ne peut pas
//! etiqueter un produit. L'ecran le montre en imposant la famille avant tout.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Form;
use db::categorie::{self, Famille};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La page, pour une famille.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let famille = q
        .get("famille")
        .and_then(|s| Famille::depuis_code(s))
        .unwrap_or(Famille::Client);
    let listes = categorie::categories(&app.pool, entity, famille)?;
    let ok = q.get("ok").map(|s| s.as_str());

    Ok(ui::page(
        "parametres",
        "categories",
        "Catégories",
        html! {
            h1 { (crate::langue::tr("Catégories")) }
            p .sous-titre {
                (crate::langue::tr("Une étiquette appartient à une famille : une catégorie de clients ne peut pas marquer un produit."))
            }

            @if ok == Some("creee") { (ui::flash("Catégorie créée.")) }
            @if ok == Some("affectee") { (ui::flash("Objet étiqueté.")) }
            @if ok == Some("retiree") { (ui::flash("Étiquette retirée.")) }

            form method="get" action="/categories" {
                div .actions {
                    select name="famille" aria-label="Famille" onchange="this.form.submit()" {
                        @for f in Famille::TOUTES {
                            option value=(f.code()) selected[f == famille] { (crate::langue::tr(f.libelle())) }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr(famille.libelle())) " — " (listes.len().to_string()) " catégorie(s)" }

            @if listes.is_empty() {
                (ui::vide("Aucune catégorie dans cette famille.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Libellé")) }
                            th { (crate::langue::tr("Description")) }
                            th .num { (crate::langue::tr("Objets")) }
                            th { (crate::langue::tr("Type")) }
                        }
                    }
                    tbody {
                        @for c in &listes {
                            tr {
                                td {
                                    @if let Some(couleur) = &c.couleur {
                                        span style=(format!("display:inline-block;width:9px;height:9px;background:{couleur};margin-right:7px;vertical-align:middle")) {}
                                    }
                                    (c.libelle.clone())
                                }
                                td { (c.description.clone()) }
                                td .num { (c.objets.to_string()) }
                                td .ref { (famille.type_id().to_string()) }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Créer une catégorie")) }
            form method="post" action="/categories/creer" {
                input type="hidden" name="famille" value=(famille.code());
                div .actions {
                    input type="text" name="libelle" placeholder=(crate::langue::tr("Libellé")) size="22" required;
                    input type="text" name="description" placeholder=(crate::langue::tr("Description")) size="28";
                    input type="color" name="couleur" value="#8a5a2b" aria-label=(crate::langue::tr("Couleur"));
                    button type="submit" class="petit" { (crate::langue::tr("Créer")) }
                }
            }

            h2 { (crate::langue::tr("Étiqueter un objet")) }
            @if listes.is_empty() {
                (ui::vide("Créez d'abord une catégorie.", None))
            } @else {
                form method="post" action="/categories/affecter" {
                    input type="hidden" name="famille" value=(famille.code());
                    div .actions {
                        select name="categorie" aria-label=(crate::langue::tr("Catégorie")) required {
                            @for c in &listes {
                                option value=(c.rowid) { (c.libelle.clone()) }
                            }
                        }
                        input type="number" name="objet" placeholder=(crate::langue::tr("N° de l'objet")) size="8" required;
                        button type="submit" class="petit" { (crate::langue::tr("Étiqueter")) }
                    }
                    p class="aide" {
                        (crate::langue::tr("Le numéro est le rowid de l'objet dans sa table Dolibarr."))
                        " " code { (famille.table()) }
                    }
                }

                h2 { (crate::langue::tr("Retirer une étiquette")) }
                form method="post" action="/categories/retirer" {
                    input type="hidden" name="famille" value=(famille.code());
                    div .actions {
                        select name="categorie" aria-label=(crate::langue::tr("Catégorie")) required {
                            @for c in &listes {
                                option value=(c.rowid) { (c.libelle.clone()) }
                            }
                        }
                        input type="number" name="objet" placeholder=(crate::langue::tr("N° de l'objet")) size="8" required;
                        button type="submit" class="petit secondaire" { (crate::langue::tr("Retirer")) }
                    }
                }
            }
        },
    ))
}

fn famille_de(f: &HashMap<String, String>) -> Result<Famille, Erreur> {
    f.get("famille")
        .and_then(|s| Famille::depuis_code(s))
        .ok_or_else(|| Erreur("famille inconnue".into()))
}

/// Cree une categorie.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let famille = famille_de(&f)?;
    let libelle = f.get("libelle").map(|s| s.trim()).unwrap_or("");
    let description = f.get("description").map(|s| s.trim()).unwrap_or("");
    let couleur = f.get("couleur").map(|s| s.as_str());

    categorie::creer(
        &app.pool,
        entite::courante(),
        famille,
        libelle,
        description,
        couleur,
        0,
    )?;
    Ok(Redirect::to(&format!(
        "/categories?famille={}&ok=creee",
        famille.code()
    )))
}

/// Etiquette un objet.
pub async fn affecter(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let famille = famille_de(&f)?;
    let cat = f
        .get("categorie")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("catégorie manquante".into()))?;
    let objet = f
        .get("objet")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| Erreur("numéro d'objet invalide".into()))?;

    categorie::affecter(&app.pool, entite::courante(), famille, cat, objet)?;
    Ok(Redirect::to(&format!(
        "/categories?famille={}&ok=affectee",
        famille.code()
    )))
}

/// Retire une etiquette.
pub async fn retirer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let famille = famille_de(&f)?;
    let cat = f
        .get("categorie")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("catégorie manquante".into()))?;
    let objet = f
        .get("objet")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| Erreur("numéro d'objet invalide".into()))?;

    categorie::retirer(&app.pool, famille, cat, objet)?;
    Ok(Redirect::to(&format!(
        "/categories?famille={}&ok=retiree",
        famille.code()
    )))
}
