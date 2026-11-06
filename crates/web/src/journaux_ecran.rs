//! Ecran des journaux : qui a fait quoi, et ce que la base garde en trace.
//!
//! Dolibarr a deux tables de trace, et elles ne repondent pas a la meme
//! question :
//!
//! - `llx_events` : **qui s'est connecte, quand, depuis quelle adresse**. Le
//!   noyau l'ecrit a chaque connexion, echec de connexion, changement de mot de
//!   passe et bascule d'entite. C'est la table qu'on ouvre quand on se demande
//!   « qui a touche a ca » ;
//! - `llx_blockedlog` : le **registre certifie**. Chaque document comptable y
//!   laisse une empreinte chainee, pour qu'une suppression se voie. Il reste
//!   vide tant qu'aucun document n'est certifie, et ce n'est pas une anomalie.
//!
//! L'ecran ne fait que lire. Un journal qu'on peut retoucher ne prouve rien.

use std::collections::HashMap;

use axum::extract::{Query, State};
use db::events::{self, Evenement};
use maud::html;

use crate::{entite, ui, App, Res};

/// Le nombre de lignes montrees sans filtre.
const LIMITE: u32 = 200;

/// Lit les evenements, filtres par type quand on le demande.
///
/// Le filtre se fait ici et non en base : `llx_events` reste petite (une ligne
/// par connexion), et une requete parametree par type pour chaque valeur du
/// menu serait plus de code que de trier deux cents lignes en memoire.
fn filtrer(evenements: Vec<Evenement>, type_: Option<&str>) -> Vec<Evenement> {
    match type_ {
        Some(t) if !t.trim().is_empty() && t != "tous" => evenements
            .into_iter()
            .filter(|e| e.type_ == t.trim())
            .collect(),
        _ => evenements,
    }
}

/// La page.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let choisi = q.get("type").map(|s| s.as_str());
    let types = events::par_type(&app.pool, entity)?;
    let total = events::total(&app.pool, entity)?;
    let evenements = filtrer(events::recents(&app.pool, entity, LIMITE)?, choisi);

    Ok(ui::page(
        "parametres",
        "journaux",
        "Journaux",
        html! {
            h1 { (crate::langue::tr("Journaux")) }
            p .sous-titre {
                (crate::langue::tr("Qui s'est connecté, quand, depuis quelle adresse. Le noyau consigne chaque connexion, chaque échec et chaque changement de mot de passe."))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Événements"),
                    &total.to_string(),
                    Some(&crate::langue::tr("en base, tout confondu")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Types distincts"),
                    &types.len().to_string(),
                    Some(&crate::langue::tr("connexions, sécurité, comptabilité")),
                    false,
                ))
            }

            h2 { (crate::langue::tr("Filtrer")) }
            p style="margin:0 0 14px" {
                a href="/parametres/journaux" class="petit secondaire" {
                    (crate::langue::tr("Tous"))
                }
                @for (t, n) in &types {
                    " "
                    a href=(format!("/parametres/journaux?type={}", t)) class="petit secondaire" {
                        (t) " (" (n) ")"
                    }
                }
            }

            h2 { (crate::langue::tr("Derniers événements")) }
            @if evenements.is_empty() {
                (ui::vide(
                    "Aucun événement pour ce filtre. Une base neuve n'en a aucun : la première ligne apparaît à la première connexion.",
                    None,
                ))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Date")) }
                            th { (crate::langue::tr("Type")) }
                            th { (crate::langue::tr("Compte")) }
                            th { (crate::langue::tr("Ce qui s'est passé")) }
                            th { (crate::langue::tr("Adresse")) }
                        }
                    }
                    tbody {
                        @for e in &evenements {
                            tr {
                                td .ref { (e.date.clone()) }
                                td {
                                    span class=(format!("pastille {}", classe_type(&e.type_))) {
                                        (e.type_.clone())
                                    }
                                }
                                td { (e.login.clone().unwrap_or_else(|| "—".into())) }
                                td { (e.description.clone()) }
                                td .ref { (e.ip.clone()) }
                            }
                        }
                    }
                }
                p.muet.petite.haut {
                    (crate::langue::tr("Les 200 derniers événements. Le journal complet reste en base ; la tâche purge le nettoie au-delà du délai configuré."))
                }
            }

            h2 { (crate::langue::tr("Registre certifié")) }
            p class="aide" {
                (crate::langue::tr("llx_blockedlog est le registre à empreintes chaînées : chaque document certifié y laisse une signature, pour qu'une suppression se voie. Il reste vide tant qu'aucun document n'est certifié — ce n'est pas une anomalie."))
            }
        },
    ))
}

/// La couleur d'une pastille selon la famille d'evenement.
///
/// Un echec de connexion doit se voir avant les autres : c'est le seul cas qui
/// peut annoncer une tentative d'intrusion.
fn classe_type(type_: &str) -> &'static str {
    if type_.contains("FAILED") {
        "p-impayee"
    } else if type_.starts_with("SECURITY_") {
        "p-partielle"
    } else {
        "p-payee"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evenement(type_: &str) -> Evenement {
        Evenement {
            rowid: 1,
            type_: type_.to_string(),
            date: "2026-11-05 10:00:00".into(),
            login: Some("maxime".into()),
            description: "connexion".into(),
            ip: "127.0.0.1".into(),
            agent: None,
        }
    }

    #[test]
    fn le_filtre_garde_le_type_demande() {
        let tous = vec![
            evenement("USER_LOGIN"),
            evenement("USER_LOGIN_FAILED"),
            evenement("USER_LOGIN"),
        ];
        let gardes = filtrer(tous.clone(), Some("USER_LOGIN"));
        assert_eq!(gardes.len(), 2);
        assert!(gardes.iter().all(|e| e.type_ == "USER_LOGIN"));
    }

    #[test]
    fn sans_filtre_tout_passe() {
        let tous = vec![
            evenement("USER_LOGIN"),
            evenement("SECURITY_MODULE_ENABLED"),
        ];
        assert_eq!(filtrer(tous.clone(), None).len(), 2);
        assert_eq!(filtrer(tous.clone(), Some("")).len(), 2);
        // « tous » est le nom du lien dans l'ecran, pas un type.
        assert_eq!(filtrer(tous, Some("tous")).len(), 2);
    }

    #[test]
    fn un_echec_de_connexion_se_distingue() {
        assert_eq!(classe_type("USER_LOGIN_FAILED"), "p-impayee");
        assert_eq!(classe_type("SECURITY_ENTITY_SWITCH"), "p-partielle");
        assert_eq!(classe_type("USER_LOGIN"), "p-payee");
    }
}
