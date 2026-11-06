//! Stock : niveaux par entrepot, mouvements, entrepots.
//!
//! Le stock d'un produit n'est pas une colonne : c'est la somme de ses
//! mouvements. Un ecran qui afficherait un chiffre stocke mentirait des que
//! deux mouvements se contredisent.
use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use maud::{html, Markup};

use crate::{message, Erreur, Res};

/// Stock : niveaux par produit et entrepot, derniers mouvements.
pub async fn stock(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let niveaux = db::stock::niveaux(&app.pool, crate::entite::courante())?;
    let mouvements = db::stock::mouvements(&app.pool, crate::entite::courante(), 50)?;
    let entrepots = db::stock::entrepots(&app.pool, crate::entite::courante())?;
    let produits = db::product::list(&app.pool, crate::entite::courante(), 200, 0)?;

    Ok(crate::ui::page(
        "produits",
        "stock",
        "Stock",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Stock")) }
            p .sous-titre {(crate::langue::tr("Le niveau affiche est la somme des mouvements, jamais une valeur saisie a part."))
            }

            h2 {(crate::langue::tr("Niveaux")) }
            @if niveaux.is_empty() {
                (crate::ui::vide("Aucun stock enregistre.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Produit")) } th {(crate::langue::tr("Designation")) } th {(crate::langue::tr("Entrepot")) } th .num {(crate::langue::tr("Quantite")) } } }
                    tbody {
                        @for n in &niveaux {
                            tr {
                                td .ref { (n.ref_produit) }
                                td { (n.label_produit) }
                                td .ref { (n.entrepot) }
                                td .num { (app::fmt_qte(Some(n.quantite))) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouveau mouvement")) }
            @if entrepots.is_empty() {
                (crate::ui::vide("Creez d'abord un entrepot.", None))
                form method="post" action="/stock/entrepot" .actions {
                    input type="text" name="ref" placeholder="Reference de l'entrepot" required;
                    input type="text" name="lieu" placeholder="Lieu";
                    button type="submit" {(crate::langue::tr("Creer l'entrepot")) }
                }
            } @else {
                form method="post" action="/stock/mouvement" {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Produit")) }
                                td {
                                    select name="produit" required class="pleine" {
                                        @for p in &produits {
                                            option value=(p.rowid) { (p.ref_) " - " (p.label) }
                                        }
                                    }
                                }
                            }
                            tr {
                                th {(crate::langue::tr("Entrepot")) }
                                td {
                                    select name="entrepot" required {
                                        @for e in &entrepots {
                                            option value=(e.rowid) { (e.ref_) }
                                        }
                                    }
                                }
                            }
                            tr { th {(crate::langue::tr("Quantite")) } td { input type="number" name="quantite" step="0.01" min="0.01" value="1" class="champ-court"; } }
                            tr {
                                th {(crate::langue::tr("Sens")) }
                                td {
                                    select name="sens" {
                                        option value="0" {(crate::langue::tr("Entree")) }
                                        option value="1" {(crate::langue::tr("Sortie")) }
                                    }
                                }
                            }
                            tr { th {(crate::langue::tr("Libelle")) } td { input type="text" name="label" class="pleine"; } }
                        }
                    }
                    div .actions { button type="submit" {(crate::langue::tr("Enregistrer le mouvement")) } }
                }
                form method="post" action="/stock/entrepot" .actions {
                    input type="text" name="ref" placeholder="Nouvel entrepot" required;
                    input type="text" name="lieu" placeholder="Lieu";
                    button type="submit" class="secondaire" {(crate::langue::tr("Creer l'entrepot")) }
                }
            }

            h2 {(crate::langue::tr("Derniers mouvements")) }
            @if mouvements.is_empty() {
                (crate::ui::vide("Aucun mouvement.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Produit")) } th {(crate::langue::tr("Entrepot")) } th {(crate::langue::tr("Sens")) } th .num {(crate::langue::tr("Quantite")) } th {(crate::langue::tr("Libelle")) } } }
                    tbody {
                        @for m in &mouvements {
                            tr {
                                td .ref { (app::fmt_date_fr(m.date.as_deref())) }
                                td .ref { (m.ref_produit) }
                                td .ref { (m.entrepot) }
                                td {
                                    @if m.type_ == 0 { "entree" } @else { "sortie" }
                                }
                                td .num { (app::fmt_qte(Some(m.quantite))) }
                                td { (m.label.clone().unwrap_or_default()) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub async fn stock_mouvement(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| f.get(c).map(|s| s.trim().to_string());
    let produit = champ("produit")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("produit obligatoire".into()))?;
    let entrepot = champ("entrepot")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("entrepot obligatoire".into()))?;
    let quantite = champ("quantite")
        .map(|v| v.replace(',', "."))
        .and_then(|v| v.parse::<rust_decimal::Decimal>().ok())
        .ok_or_else(|| Erreur("quantite invalide".into()))?;
    let sens: i16 = champ("sens").and_then(|v| v.parse().ok()).unwrap_or(0);
    let label = champ("label").filter(|s| !s.is_empty());
    db::stock::mouvement(
        &app.pool,
        crate::entite::courante(),
        produit,
        entrepot,
        quantite,
        sens,
        label.as_deref(),
    )?;
    Ok(Redirect::to("/stock?ok=mouvement"))
}

pub async fn stock_entrepot(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let reference = champ("ref").ok_or_else(|| Erreur("reference obligatoire".into()))?;
    let lieu = champ("lieu");
    db::stock::creer_entrepot(
        &app.pool,
        crate::entite::courante(),
        &reference,
        lieu.as_deref(),
    )?;
    Ok(Redirect::to("/stock?ok=entrepot"))
}
