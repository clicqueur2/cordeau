//! Ecran des notifications : ce qui reclame une decision, et ou ca part.
//!
//! Aucun intermediaire : le courriel part du SMTP configure, le webhook est une
//! requete POST vers l'adresse fournie. L'ecran montre le message tel qu'il
//! partira, avant qu'il ne parte.

use std::collections::HashMap;

use axum::extract::State;
use axum::response::Redirect;
use axum::Form;
use db::notifications::{self, Config};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La page : configuration, et apercu du message.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let config = notifications::config(&app.pool, entity)?;
    let jour = db::aujourdhui(&app.pool)?;
    let alertes = notifications::alertes(&app.pool, entity, &jour)?;
    let resume = notifications::resume(&alertes);

    Ok(ui::page(
        "parametres",
        "notifications",
        "Notifications",
        html! {
            h1 { (crate::langue::tr("Notifications")) }
            p .sous-titre {
                (crate::langue::tr("Ce qui réclame une décision, envoyé par courriel et par webhook. Aucun intermédiaire : ni n8n, ni service tiers."))
            }

            h2 { (crate::langue::tr("Canaux")) }
            form method="post" action="/parametres/notifications" {
                table {
                    tbody {
                        tr {
                            th { (crate::langue::tr("Courriel")) }
                            td { input type="email" name="email" value=(config.email) class="pleine"
                                       placeholder="contact@exemple.fr"; }
                        }
                        tr {
                            th { (crate::langue::tr("Webhook")) }
                            td { input type="url" name="webhook" value=(config.webhook) class="pleine"
                                       placeholder="https://…"; }
                        }
                        tr {
                            th { (crate::langue::tr("Délai de retard")) }
                            td {
                                input type="number" name="jours" min="0" value=(config.jours_retard)
                                      class="champ-court";
                                span class="aide" {
                                    " " (crate::langue::tr("jours avant d'alerter — 5 par défaut, le temps qu'un virement arrive."))
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" { (crate::langue::tr("Enregistrer")) }
                }
                p class="aide" {
                    (crate::langue::tr("Le webhook reçoit du JSON : un champ text (ce que lit Telegram) et un tableau alertes (ce que lira un service maison). Rien n'est supposé du destinataire."))
                }
            }

            h2 { (crate::langue::tr("Ce qui partirait maintenant")) }
            @if config.aucun_canal() {
                (ui::echec("Aucun canal configuré : rien ne partirait. Renseignez un courriel ou un webhook."))
            }
            pre .panneau.note {
                (resume)
            }

            h2 { (crate::langue::tr("Quand ça part")) }
            p class="aide" {
                (crate::langue::tr("La tâche notifier est dans Planification. Une fois programmée, elle est lancée par le cron de Dolibarr, ou par le planificateur du système avec la commande cordeau tache notifier."))
                " "
                a href="/parametres/planification" { (crate::langue::tr("Voir la planification")) }
            }
        },
    ))
}

/// Enregistre la configuration.
pub async fn enregistrer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let email = f
        .get("email")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let webhook = f
        .get("webhook")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let jours = f
        .get("jours")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(5);

    let config = Config {
        email,
        webhook,
        jours_retard: jours,
    };
    notifications::enregistrer(&app.pool, entite::courante(), &config)
        .map_err(|e| Erreur(e.to_string()))?;

    Ok(Redirect::to("/parametres/notifications"))
}
