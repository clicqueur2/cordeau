//! Declinaisons produit : taille, couleur, finition.
//!
//! Dolibarr tient `llx_product_attribute` (l'attribut), `_value` (les valeurs) et
//! `_combination` (le produit fils qui porte la combinaison). Un produit pere ne
//! se vend pas : ce sont ses fils qui partent, avec leur stock et leur prix.
//!
//! L'ecran fait les trois : creer un attribut, y ajouter des valeurs, puis
//! engendrer les combinaisons d'un produit.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use axum::Form;
use db::variante::{self, NouvelAttribut, NouvelleValeur};
use maud::html;

use crate::*;

pub(crate) async fn variantes(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let attributs = variante::attributs(p, entity)?;
    let mut detail = Vec::new();
    for a in &attributs {
        detail.push((a.clone(), variante::valeurs(p, a.rowid)?));
    }

    Ok(ui::page(
        "produits",
        "variantes",
        "Déclinaisons",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Déclinaisons produit")) }
            p .sous-titre {
                (crate::langue::tr("Un attribut (taille, couleur) porte des valeurs ; le produit père engendre ses fils."))
            }

            h2 {(crate::langue::tr("Nouvel attribut")) }
            form method="post" action="/variantes" .actions {
                input type="text" name="ref" required .champ-court placeholder="TAILLE";
                input type="text" name="label" required .champ-moyen placeholder="Taille";
                button type="submit" {(crate::langue::tr("Créer l'attribut")) }
            }

            h2 {(crate::langue::tr("Attributs et valeurs")) }
            @if detail.is_empty() {
                (ui::vide("Aucun attribut : un produit sans déclinaison n'en a pas besoin.", None))
            } @else {
                @for (a, valeurs) in &detail {
                    h2 .espace-dessus { (a.label.clone()) " " span.aide { "(" (a.ref_.clone()) ")" } }
                    @if valeurs.is_empty() {
                        p.aide {(crate::langue::tr("Aucune valeur.")) }
                    } @else {
                        table {
                            tbody {
                                tr {
                                    @for v in valeurs {
                                        td .ref { (v.value.clone()) }
                                    }
                                }
                            }
                        }
                    }
                    form method="post" action={ "/variantes/" (a.rowid) "/valeur" } .actions {
                        input type="text" name="ref" required .champ-court placeholder="S";
                        input type="text" name="value" required .champ-moyen placeholder="Small";
                        button type="submit" class="petit" {(crate::langue::tr("Ajouter la valeur")) }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn variante_attribut(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    variante::creer_attribut(
        &app.pool,
        entite::courante(),
        &NouvelAttribut {
            ref_: champ("ref").ok_or_else(|| Erreur("référence obligatoire".into()))?,
            label: champ("label").ok_or_else(|| Erreur("libellé obligatoire".into()))?,
            position: 0,
        },
    )?;
    Ok(Redirect::to("/variantes?ok=attribut"))
}

pub(crate) async fn variante_valeur(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    variante::creer_valeur(
        &app.pool,
        entite::courante(),
        &NouvelleValeur {
            fk_attribut: id,
            ref_: champ("ref").ok_or_else(|| Erreur("référence obligatoire".into()))?,
            value: champ("value").ok_or_else(|| Erreur("valeur obligatoire".into()))?,
            position: 0,
        },
    )?;
    Ok(Redirect::to("/variantes?ok=valeur"))
}

/// Les combinaisons d'un produit, et l'ajout d'une combinaison.
pub(crate) async fn produit_variantes(State(app): State<App>, Path(id): Path<i64>) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let produit = db::product::get(p, entity, id)?
        .ok_or_else(|| Erreur(format!("produit {id} introuvable")))?;
    let combinaisons = variante::combinaisons(p, entity, id)?;
    let attributs = variante::attributs(p, entity)?;
    let mut valeurs = Vec::new();
    for a in &attributs {
        for v in variante::valeurs(p, a.rowid)? {
            valeurs.push((a.label.clone(), v));
        }
    }

    Ok(ui::page(
        "produits",
        "variantes",
        &produit.label,
        html! {
            h1 { "Déclinaisons de " (produit.label.clone()) }
            p .sous-titre { (produit.ref_.clone()) }

            h2 {(crate::langue::tr("Combinaisons")) }
            @if combinaisons.is_empty() {
                (ui::vide("Aucune combinaison : ce produit n'est pas décliné.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Valeurs")) } th .num {(crate::langue::tr("Écart de prix")) } th { "" } } }
                    tbody {
                        @for c in &combinaisons {
                            tr {
                                td {
                                    @for (i, v) in c.valeurs.iter().enumerate() {
                                        @if i > 0 { " · " }
                                        (v.value.clone())
                                    }
                                }
                                td .num { (ui::euros(c.variation_prix)) }
                                td {
                                    form method="post" action={ "/produits/" (id) "/variantes/" (c.rowid) "/supprimer" } {
                                        button type="submit" class="petit secondaire" {(crate::langue::tr("Supprimer")) }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouvelle combinaison")) }
            @if valeurs.is_empty() {
                (ui::vide("Aucune valeur d'attribut : créez d'abord une taille ou une couleur.", Some(("/variantes", "Gérer les attributs"))))
            } @else {
                form method="post" action={ "/produits/" (id) "/variantes" } {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Valeurs")) }
                                td {
                                    @for (attribut, v) in &valeurs {
                                        label .etiquette {
                                            input type="checkbox" name={"v_" (v.rowid)} value="1";
                                            " " (attribut.clone()) " : " (v.value.clone())
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div .actions { button type="submit" {(crate::langue::tr("Engendrer la combinaison")) } }
                }
            }
        },
    ))
}

pub(crate) async fn produit_variante_creer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let mut ids: Vec<i64> = f
        .keys()
        .filter_map(|k| k.strip_prefix("v_").and_then(|n| n.parse().ok()))
        .collect();
    ids.sort_unstable();
    if ids.is_empty() {
        return Err(Erreur("choisissez au moins une valeur".into()));
    }
    variante::creer_combinaison(&app.pool, entite::courante(), id, &ids, None)?;
    Ok(Redirect::to(&format!(
        "/produits/{id}/variantes?ok=combinaison"
    )))
}

pub(crate) async fn produit_variante_supprimer(
    State(app): State<App>,
    Path((id, combinaison)): Path<(i64, i64)>,
) -> Res<Redirect> {
    variante::supprimer_combinaison(&app.pool, entite::courante(), combinaison)?;
    Ok(Redirect::to(&format!(
        "/produits/{id}/variantes?ok=supprime"
    )))
}
