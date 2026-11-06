//! Ecran de planification : les taches qui tournent sans personne.

use std::collections::HashMap;

use axum::extract::State;
use axum::response::Redirect;
use axum::Form;
use db::planification::{self, TACHES};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// Le nombre de secondes rendu lisible : « tous les jours », « toutes les semaines ».
fn periode(secondes: i64) -> &'static str {
    match secondes {
        s if s >= 2_592_000 => "tous les mois",
        s if s >= 604_800 => "toutes les semaines",
        s if s >= 86_400 => "tous les jours",
        s if s >= 3_600 => "toutes les heures",
        _ => "souvent",
    }
}

/// La page : ce que le binaire sait faire, et ce qui est programme.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let travaux = planification::travaux(&app.pool, entity)?;

    Ok(ui::page(
        "parametres",
        "planification",
        "Planification",
        html! {
            h1 { (crate::langue::tr("Planification")) }
            p .sous-titre {
                (crate::langue::tr("Le noyau n'a aucun démon : un service qui tourne en permanence pour trois tâches est un service de plus à surveiller. Les tâches se programment dans llx_cronjob, et c'est le cron de Dolibarr qui les lance."))
            }

            h2 { (crate::langue::tr("Tâches disponibles")) }
            table {
                thead {
                    tr {
                        th { (crate::langue::tr("Tâche")) }
                        th { (crate::langue::tr("Ce qu'elle fait")) }
                        th { (crate::langue::tr("Commande")) }
                        th { (crate::langue::tr("Fréquence")) }
                        th { "" }
                    }
                }
                tbody {
                    @for t in TACHES {
                        tr {
                            td { (crate::langue::tr(t.libelle)) }
                            td class="aide" { (crate::langue::tr(t.description)) }
                            td .ref { (t.commande) }
                            td { (crate::langue::tr(periode(t.frequence))) }
                            td {
                                form method="post" action="/parametres/planification" {
                                    input type="hidden" name="code" value=(t.code);
                                    button type="submit" class="petit" { (crate::langue::tr("Programmer")) }
                                }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Travaux programmés")) }
            @if travaux.is_empty() {
                (ui::vide(
                    "Aucune tâche programmée. Le cron de Dolibarr ne lancera rien.",
                    None,
                ))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Libellé")) }
                            th { (crate::langue::tr("Commande")) }
                            th { (crate::langue::tr("Dernière")) }
                            th { (crate::langue::tr("Prochaine")) }
                            th { "État" }
                            th { "" }
                        }
                    }
                    tbody {
                        @for t in &travaux {
                            tr {
                                td { (t.libelle.clone()) }
                                td .ref { (t.commande.clone()) }
                                td .ref { (t.derniere.clone().unwrap_or_else(|| "—".into())) }
                                td .ref { (t.prochaine.clone().unwrap_or_else(|| "—".into())) }
                                td {
                                    @if t.actif {
                                        span .pastille.p-payee { "actif" }
                                    } @else {
                                        span .pastille.p-brouillon { "inactif" }
                                    }
                                    @if let Some(r) = &t.resultat {
                                        span.muet.petite { " " (r.clone()) }
                                    }
                                }
                                td {
                                    @if t.actif {
                                        form method="post" action="/parametres/planification/desactiver" {
                                            input type="hidden" name="rowid" value=(t.rowid);
                                            button type="submit" class="petit secondaire" {
                                                (crate::langue::tr("Désactiver"))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            p style="color:#78736c;font-size:13.5px;margin-top:16px" {
                (crate::langue::tr("Sans cron Dolibarr, la même commande se programme dans le planificateur du système (tâche planifiée Windows, cron Unix). L'écran donne la ligne exacte : rien à deviner."))
            }
        },
    ))
}

/// Programme une tache.
pub async fn programmer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let code = f
        .get("code")
        .map(|s| s.trim().to_string())
        .ok_or_else(|| Erreur("tâche manquante".into()))?;
    let commande = f.get("commande").map(|s| s.as_str());

    planification::programmer(&app.pool, entite::courante(), &code, commande)?;
    Ok(Redirect::to("/parametres/planification"))
}

/// Desactive un travail.
pub async fn desactiver(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let rowid = f
        .get("rowid")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("travail manquant".into()))?;

    planification::desactiver(&app.pool, entite::courante(), rowid)?;
    Ok(Redirect::to("/parametres/planification"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_frequence_se_lit_en_francais() {
        assert_eq!(periode(86_400), "tous les jours");
        assert_eq!(periode(604_800), "toutes les semaines");
        assert_eq!(periode(3_600), "toutes les heures");
        assert_eq!(periode(10), "souvent");
    }
}
