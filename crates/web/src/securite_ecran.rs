//! Ecran de securite HTTP : les en-tetes que le serveur envoie vraiment.
//!
//! L'ecran montre le resultat, pas la saisie : a droite du formulaire, la liste
//! exacte des en-tetes qui partiront avec la prochaine reponse. Un reglage qu'on
//! ne voit pas applique ne se verifie jamais.

use std::collections::HashMap;

use axum::extract::State;
use axum::response::Redirect;
use axum::Form;
use db::securite::{self, Config};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La page.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let config = securite::config(&app.pool, entity)?;
    let envoyes = securite::entetes(&config, "https://votre-instance");

    Ok(ui::page(
        "parametres",
        "securite",
        "Sécurité",
        html! {
            h1 { (crate::langue::tr("Sécurité")) }
            p .sous-titre {
                (crate::langue::tr("Les en-têtes que le navigateur reçoit à chaque réponse. Un formulaire de facturation sans eux fonctionne — il fonctionne même trop bien."))
            }

            form method="post" action="/parametres/securite" {
                h2 { (crate::langue::tr("Politique de sécurité du contenu")) }
                table {
                    tbody {
                        tr {
                            th { "Content-Security-Policy" }
                            td {
                                textarea name="csp" rows="2" style="width:100%;font-family:ui-monospace,monospace;font-size:13px"
                                         placeholder="default-src 'self'; img-src * data:" {
                                    (config.csp)
                                }
                                div style="color:#78736c;font-size:13px" {
                                    (crate::langue::tr("Vide : aucun en-tête. Une CSP trop stricte casse le premier module qui charge une police distante — elle se saisit, elle ne se devine pas."))
                                }
                            }
                        }
                        tr {
                            th { "Content-Security-Policy-Report-Only" }
                            td {
                                textarea name="csp_rapport" rows="2" style="width:100%;font-family:ui-monospace,monospace;font-size:13px" {
                                    (config.csp_rapport)
                                }
                                div style="color:#78736c;font-size:13px" {
                                    (crate::langue::tr("Signale les violations sans rien bloquer. À utiliser avant d'activer la CSP."))
                                }
                            }
                        }
                    }
                }

                h2 { (crate::langue::tr("Transport et permissions")) }
                table {
                    tbody {
                        tr {
                            th { "Strict-Transport-Security" }
                            td {
                                input type="text" name="hsts" value=(config.hsts) style="width:100%" placeholder="max-age=31536000";
                                div style="color:#78736c;font-size:13px" {
                                    (crate::langue::tr("À ne poser qu'en HTTPS : un HSTS sur un site encore en HTTP le rend inaccessible pour des mois."))
                                }
                            }
                        }
                        tr {
                            th { "Permissions-Policy" }
                            td {
                                input type="text" name="permissions" value=(config.permissions) style="width:100%"
                                       placeholder="camera=(), microphone=(), geolocation=()";
                            }
                        }
                        tr {
                            th { "Referrer-Policy" }
                            td {
                                input type="text" name="referrer" value=(config.referrer) style="width:100%" placeholder="same-origin";
                                div style="color:#78736c;font-size:13px" {
                                    (crate::langue::tr("same-origin par défaut : le navigateur n'annonce pas d'où vient le visiteur sur les liens sortants."))
                                }
                            }
                        }
                        tr {
                            th { (crate::langue::tr("Appels d'un autre site")) }
                            td {
                                input type="checkbox" name="cors" value="1" checked[config.cors];
                                span style="color:#78736c;font-size:13px" {
                                    " " (crate::langue::tr("Autorise les appels d'API à votre instance, avec Vary: Origin. Jamais de wildcard."))
                                }
                            }
                        }
                    }
                }

                div .actions {
                    button type="submit" { (crate::langue::tr("Enregistrer")) }
                }
            }

            h2 { (crate::langue::tr("Ce qui part avec chaque réponse")) }
            table {
                thead {
                    tr {
                        th { (crate::langue::tr("En-tête")) }
                        th { (crate::langue::tr("Valeur")) }
                    }
                }
                tbody {
                    @for (nom, valeur) in &envoyes {
                        tr {
                            td .ref { (nom.clone()) }
                            td .ref { (valeur.clone()) }
                        }
                    }
                }
            }
            p style="color:#78736c;font-size:13px;margin-top:12px" {
                (crate::langue::tr("X-Content-Type-Options: nosniff est toujours posé : il empêche le navigateur de deviner un type de contenu, ce qui transforme un fichier déposé par un client en script exécutable."))
            }
        },
    ))
}

/// Enregistre la politique.
pub async fn enregistrer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let config = Config {
        csp: lire("csp"),
        csp_rapport: lire("csp_rapport"),
        hsts: lire("hsts"),
        permissions: lire("permissions"),
        referrer: lire("referrer"),
        cors: f.contains_key("cors"),
    };
    securite::enregistrer(&app.pool, entite::courante(), &config)
        .map_err(|e| Erreur(e.to_string()))?;
    // Le cache de la politique est oublie : sinon les en-tetes changeraient
    // jusqu'a trente secondes plus tard, et l'ecran semblerait sans effet.
    crate::garde::oublier_politique();

    Ok(Redirect::to("/parametres/securite"))
}
