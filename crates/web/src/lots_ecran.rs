//! Ecran des lots et series : lister, creer, et voir ce qu'il reste.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Form;
use db::lot::{self, NouveauLot};
use maud::html;

use crate::*;

pub(crate) async fn lots(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let liste = lot::list(p, entity, 500)?;
    let produits = db::product::list(p, entity, 500, 0)?;
    let jour = aujourdhui(p)?;

    // Combien expirent cette annee : c'est la seule chose qu'un lot demande
    // vraiment, et elle doit se voir sans ouvrir chaque fiche.
    let fin_annee = format!("{}-12-31", &jour[..4]);
    let bientot: Vec<_> = liste
        .iter()
        .filter(|l| {
            l.eatby
                .as_deref()
                .map(|d| d >= jour.as_str() && d <= fin_annee.as_str())
                .unwrap_or(false)
        })
        .collect();

    Ok(ui::page(
        "produits",
        "lots",
        "Lots et séries",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Lots et séries")) }
            p .sous-titre {
                (crate::langue::tr("Un lot porte un numéro et ses dates. Le stock d'un lot, lui, est la somme de ses mouvements — on ne le recopie pas."))
            }

            div .kpis {
                div .kpi {
                    div .titre {(crate::langue::tr("Lots")) }
                    div .valeur { (liste.len()) }
                }
                div .kpi.alerte {
                    div .titre {(crate::langue::tr("À consommer cette année")) }
                    div .valeur { (bientot.len()) }
                }
            }

            h2 {(crate::langue::tr("Nouveau lot")) }
            @if produits.is_empty() {
                (ui::vide("Aucun produit : un lot trace un produit.", Some(("/produits/nouveau", "Créer un produit"))))
            } @else {
                form method="post" action="/lots" {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Produit")) }
                                td {
                                    select name="produit" required.pleine {
                                        @for pr in &produits {
                                            option value=(pr.rowid) { (pr.ref_.clone()) " — " (pr.label.clone()) }
                                        }
                                    }
                                }
                            }
                            tr { th {(crate::langue::tr("Numéro de lot")) } td { input type="text" name="batch" required.pleine placeholder="L2026-045"; } }
                            tr { th {(crate::langue::tr("À consommer avant")) } td { input type="date" name="eatby" .champ-moyen; } }
                            tr { th {(crate::langue::tr("À vendre avant")) } td { input type="date" name="sellby" .champ-moyen; } }
                            tr { th {(crate::langue::tr("Note")) } td { input type="text" name="note" .pleine; } }
                        }
                    }
                    div .actions { button type="submit" {(crate::langue::tr("Créer le lot")) } }
                }
            }

            h2 {(crate::langue::tr("Lots enregistrés")) }
            @if liste.is_empty() {
                (ui::vide("Aucun lot.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Produit")) }
                            th {(crate::langue::tr("Lot")) }
                            th {(crate::langue::tr("À consommer avant")) }
                            th {(crate::langue::tr("À vendre avant")) }
                            th {(crate::langue::tr("Créé le")) }
                        }
                    }
                    tbody {
                        @for l in &liste {
                            tr {
                                td .ref { a href={ "/produits/" } { "..." } }
                                td .ref { (l.batch.clone().unwrap_or_default()) }
                                td .ref {
                                    @if l.eatby.as_deref().map(|d| d < jour.as_str()).unwrap_or(false) {
                                        span .pastille.p-retard { (app::fmt_date_fr(l.eatby.as_deref())) }
                                    } @else {
                                        (app::fmt_date_fr(l.eatby.as_deref()))
                                    }
                                }
                                td .ref { (app::fmt_date_fr(l.sellby.as_deref())) }
                                td .ref { (app::fmt_date_fr(l.datec.as_deref())) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn lot_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let nouveau = NouveauLot {
        fk_product: champ("produit")
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| Erreur("produit obligatoire".into()))?,
        batch: champ("batch").ok_or_else(|| Erreur("numéro de lot obligatoire".into()))?,
        eatby: champ("eatby"),
        sellby: champ("sellby"),
        note_public: champ("note"),
    };
    lot::creer(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to("/lots?ok=cree"))
}
