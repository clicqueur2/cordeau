//! Reglages generaux : coordonnees de la societe, envoi d'e-mail d'essai.
//!
//! Ce sont les constantes `MAIN_*` de Dolibarr. Elles sont lues par le noyau
//! mais aussi par le Dolibarr PHP, s'il tourne sur la meme base : une valeur
//! fausse ici se voit des deux cotes.
use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use maud::{html, Markup};

use super::section;
use crate::{message, Erreur, Res};

/// Ecran de reglage generique : une section = une entree du menu.
pub async fn reglages(
    State(app): State<crate::App>,
    Path(cle): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let s = section(&cle).ok_or_else(|| Erreur("reglage inconnu".into()))?;
    let mut valeurs = Vec::new();
    for c in s.champs {
        let v = db::company::get_const(&app.pool, c.cle)?
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| c.defaut.to_string());
        valeurs.push(v);
    }

    Ok(crate::ui::page(
        "parametres",
        s.cle,
        s.titre,
        html! {
            (message(q.get("ok")))
            h1 { (s.titre) }
            p .sous-titre { (s.intro) }
            form method="post" action={ "/parametres/reglage/" (s.cle) } {
                table {
                    tbody {
                        @for (i, c) in s.champs.iter().enumerate() {
                            tr {
                                th { (c.libelle) }
                                td {
                                    @if c.genre == "oui_non" {
                                        select name=(c.cle) {
                                            option value="0" selected[valeurs[i] != "1"] {(crate::langue::tr("Non")) }
                                            option value="1" selected[valeurs[i] == "1"] {(crate::langue::tr("Oui")) }
                                        }
                                    } @else if c.genre == "nombre" {
                                        input type="number" name=(c.cle) value=(valeurs[i]) style="width:130px";
                                    } @else if c.genre == "textarea" {
                                        textarea name=(c.cle) rows="4" style="width:100%;font:inherit;padding:8px 10px;border:1px solid #e6e2dc" {
                                            (valeurs[i])
                                        }
                                    } @else {
                                        input type="text" name=(c.cle) value=(valeurs[i]) class="pleine";
                                    }
                                    p style="color:#78736c;font-size:12px;margin:6px 0 0" { "constante " code { (c.cle) } }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Enregistrer")) } }
            }
            @if s.cle == "emails" {
                h2 {(crate::langue::tr("Envoyer un essai")) }
                p .sous-titre {
                    "Le message part vraiment, par le serveur ci-dessus. Une erreur SMTP s'affiche "
                    "telle quelle : c'est elle qui dit quoi corriger."
                }
                (message(q.get("ok")))
                @if let Some(e) = q.get("erreur") {
                    p .erreur { (e) }
                }
                form method="post" action="/parametres/emails/essai" {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Destinataire")) }
                                td {
                                    input type="email" name="destinataire" required
                                          value=(q.get("destinataire").cloned().unwrap_or_default())
                                          class="pleine";
                                }
                            }
                        }
                    }
                    div .actions { button type="submit" {(crate::langue::tr("Envoyer l'essai")) } }
                }
            }
        },
    ))
}

/// Envoie un e-mail d'essai par le serveur SMTP configure.
///
/// C'est le seul endroit qui parle au reseau pour du courrier : l'appel est
/// bloquant, donc execute hors du fil asynchrone. Une erreur SMTP remonte son
/// code et sa phrase, telles quelles — « 535 5.7.8 Authentication failed »
/// vaut mieux qu'un « l'envoi a echoue ».
pub async fn email_essai(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let destinataire = f
        .get("destinataire")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("destinataire obligatoire".into()))?;

    let const_ou = |cle: &str| {
        db::company::get_const(&app.pool, cle)
            .ok()
            .flatten()
            .unwrap_or_default()
    };
    let hote = const_ou("DOLIBARR_RS_SMTP_HOST");
    if hote.trim().is_empty() {
        return Ok(Redirect::to(
            "/parametres/reglage/emails?erreur=Serveur%20SMTP%20non%20renseigne",
        ));
    }
    let port = const_ou("DOLIBARR_RS_SMTP_PORT")
        .parse::<u16>()
        .unwrap_or(587);
    let mut mot_de_passe = const_ou("DOLIBARR_RS_SMTP_MOTDEPASSE");
    // La variable d'environnement l'emporte : un secret n'a pas a trainer en
    // base quand le serveur peut le fournir.
    if let Ok(v) = std::env::var("DOLIBARR_SMTP_PASSWORD") {
        if !v.is_empty() {
            mot_de_passe = v;
        }
    }

    let config = mail::Config {
        hote: hote.trim().to_string(),
        port,
        utilisateur: const_ou("DOLIBARR_RS_SMTP_UTILISATEUR").trim().to_string(),
        mot_de_passe,
        expediteur: const_ou("DOLIBARR_RS_EMAIL_EXPEDITEUR").trim().to_string(),
        nom_expediteur: const_ou("MAIN_INFO_SOCIETE_NOM").trim().to_string(),
        securite: match const_ou("DOLIBARR_RS_SMTP_SECURITE").trim() {
            "aucune" => mail::Securite::Aucune,
            "directe" => mail::Securite::Directe,
            "starttls" => mail::Securite::Starttls,
            // Par defaut : 465 direct, tout le reste en STARTTLS.
            _ => mail::Config::securite_pour(port),
        },
        delai: 20,
    };

    let message = mail::Message::texte(
        &config.expediteur,
        &[destinataire.as_str()],
        "Essai d'envoi - noyau Dolibarr en Rust",
        "Ce message a ete envoye par le noyau.

Si vous le lisez, le serveur SMTP, le compte et le mot de passe sont bons.

--\nNoyau Dolibarr en Rust",
    );

    let resultat = tokio::task::spawn_blocking(move || mail::envoyer(&config, &message)).await;

    match resultat {
        Ok(Ok(())) => Ok(Redirect::to("/parametres/reglage/emails?ok=essai_envoye")),
        Ok(Err(e)) => Ok(Redirect::to(&format!(
            "/parametres/reglage/emails?erreur={}&destinataire={}",
            encode(&e.to_string()),
            encode(&destinataire)
        ))),
        Err(e) => Ok(Redirect::to(&format!(
            "/parametres/reglage/emails?erreur={}",
            encode(&format!("tache interrompue : {e}"))
        ))),
    }
}

/// Encode une valeur pour un parametre d'URL.
fn encode(s: &str) -> String {
    let mut sortie = String::new();
    for octet in s.bytes() {
        match octet {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                sortie.push(octet as char)
            }
            b' ' => sortie.push('+'),
            autre => sortie.push_str(&format!("%{autre:02X}")),
        }
    }
    sortie
}

pub async fn reglages_enregistrer(
    State(app): State<crate::App>,
    Path(cle): Path<String>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let s = section(&cle).ok_or_else(|| Erreur("reglage inconnu".into()))?;
    for c in s.champs {
        let valeur = f.get(c.cle).cloned().unwrap_or_default();
        db::company::set_const(&app.pool, c.cle, valeur.trim())?;
    }
    Ok(Redirect::to(&format!(
        "/parametres/reglage/{cle}?ok=reglage"
    )))
}
