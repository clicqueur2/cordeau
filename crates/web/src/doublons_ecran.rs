//! Doublons de tiers : on montre, on propose, on fusionne sur demande.
//!
//! Un groupe n'est pas une preuve : « M. Charco » et « Dupont » peuvent etre
//! deux personnes. L'ecran ne fusionne donc jamais tout seul — il propose une
//! cible, et c'est le clic qui decide.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Form;
use maud::html;

use crate::*;

pub(crate) async fn doublons(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let groupes = db::doublons::groupes(p, entite::courante())?;
    let fiches: usize = groupes.iter().map(|g| g.membres.len()).sum();

    Ok(ui::page(
        "tiers",
        "doublons",
        "Doublons",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Doublons de tiers")) }
            p .sous-titre {
                (crate::langue::tr("Trois clés, de la plus forte à la plus faible : le SIRET, l'e-mail, puis le nom et la ville. Un groupe est une proposition, pas une preuve."))
            }
            p .aide.bloc {
                (crate::langue::tr("Les groupes sont classés par confiance : « certaine » quand le SIRET est identique, « forte » quand l'e-mail l'est, « à vérifier » quand seul le nom et la ville concordent. Deux noms identiques dans deux villes différentes ne sont pas proposés."))
            }

            @if groupes.is_empty() {
                (ui::vide("Aucun doublon probable. Vos fiches sont propres.", None))
            } @else {
                div .kpis {
                    div .kpi.fort {
                        div .titre {(crate::langue::tr("Groupes")) }
                        div .valeur { (groupes.len()) }
                    }
                    div .kpi {
                        div .titre {(crate::langue::tr("Fiches concernées")) }
                        div .valeur { (fiches) }
                    }
                }

                @for g in &groupes {
                    section .widget {
                        div .widget-entete {
                            h2 {
                                span class=(format!("pastille {}", g.confiance.classe())) {
                                    (g.confiance.libelle())
                                }
                                " " (g.membres.len()) " fiches"
                            }
                            span .widget-sous { (g.raison) " — " code { (g.cle.clone()) } }
                        }
                        table .tableau-serre {
                            thead {
                                tr {
                                    th {(crate::langue::tr("Fiche")) }
                                    th {(crate::langue::tr("Ville")) }
                                    th {(crate::langue::tr("E-mail")) }
                                    th {(crate::langue::tr("SIRET")) }
                                    th { "État" }
                                }
                            }
                            tbody {
                                @for m in &g.membres {
                                    tr {
                                        td .ref { a href={ "/tiers/" (m.rowid) } { (m.nom.clone()) } }
                                        td { (m.town.clone().unwrap_or_default()) }
                                        td .ref { (m.email.clone().unwrap_or_default()) }
                                        td .ref { (m.siret.clone().unwrap_or_default()) }
                                        td {
                                            @if m.statut == 0 {
                                                span .pastille.p-brouillon {(crate::langue::tr("désactivée")) }
                                            } @else {
                                                span .pastille.p-payee {(crate::langue::tr("active")) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        @let ids = g.membres.iter().map(|m| m.rowid.to_string()).collect::<Vec<_>>().join(",");
                        form method="post" action="/tiers/fusionner" .actions {
                            input type="hidden" name="sources" value=(ids);
                            label .etiquette {
                                (crate::langue::tr("garder"))
                                select name="cible" {
                                    @for m in &g.membres {
                                        option value=(m.rowid) { (m.nom.clone()) " (" (m.rowid) ")" }
                                    }
                                }
                            }
                            button type="submit" class="secondaire" {
                                (crate::langue::tr("Fusionner les autres dans celle-ci"))
                            }
                        }
                        p .aide.petite {
                            (crate::langue::tr("La fiche absorbée n'est pas supprimée : elle est désactivée, et son nom dit dans quelle fiche elle est partie."))
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn fusionner(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let cible: i64 = f
        .get("cible")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| Erreur("fiche à garder manquante".into()))?;
    let sources: Vec<i64> = f
        .get("sources")
        .map(|v| {
            v.split(',')
                .filter_map(|s| s.trim().parse::<i64>().ok())
                .filter(|id| *id != cible)
                .collect()
        })
        .unwrap_or_default();
    if sources.is_empty() {
        return Err(Erreur("aucune fiche à fusionner".into()));
    }
    for source in &sources {
        db::doublons::fusionner(&app.pool, entite::courante(), *source, cible)?;
    }
    Ok(Redirect::to("/tiers/doublons?ok=fusion"))
}
