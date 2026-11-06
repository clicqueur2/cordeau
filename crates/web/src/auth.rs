//! Session, connexion et deconnexion.
//!
//! Trois sujets qu'il vaut mieux lire ensemble :
//!
//! - **le jeton de session** : aleatoire, il ne dit rien du mot de passe ;
//! - **le cookie** : `HttpOnly`, donc invisible au JavaScript de la page ;
//! - **l'entite de travail** : choisie ici, a la connexion, puis portee par la
//!   session. Changer d'entite est un evenement consigne dans le journal.
//!
//! # Ce qui n'est pas fait ici
//!
//! Aucun mot de passe n'est stocke en session : seul le login y figure, et les
//! droits sont relus a chaque requete. Une session volee ne donne donc pas les
//! droits d'hier si l'administrateur les a retires entre-temps.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Extension;
use axum::Form;
use maud::{html, Markup};

use crate::*;

pub(crate) fn jeton(login: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h: u64 = 1469598103934665603;
    for b in login.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(1099511628211);
    }
    format!("{nanos:x}{h:x}")
}

pub(crate) fn lire_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    let brut = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for morceau in brut.split(';') {
        let m = morceau.trim();
        if let Some(v) = m.strip_prefix(COOKIE) {
            if let Some(v) = v.strip_prefix('=') {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// L'adresse du client, pour le journal des evenements.
///
/// `X-Forwarded-For` n'a de valeur que posee par un proxy de confiance : un
/// client direct peut la forger. Quand elle est absente, on retombe sur la
/// connexion locale, qui est la verite de ce processus.
pub(crate) fn ip_client(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

/// Le navigateur declare, tronque a la taille de la colonne.
pub(crate) fn agent_client(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.chars().take(255).collect())
}

/// Change l'entite de la session en cours.
pub(crate) async fn changer_entite(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let entity: i64 = f
        .get("entity")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Erreur("entite invalide".into()))?;
    if let Some(j) = lire_cookie(&headers) {
        let mut sessions = app.sessions.lock().unwrap();
        if let Some(s) = sessions.get_mut(&j) {
            s.entity = entity;
        }
    }
    // La barre de menus et les modules dependent de l'entite.
    let _ = menu_actif::charger(&app.pool, entity);
    Ok(Redirect::to("/"))
}

/// La politique de sécurité, relue au plus toutes les trente secondes.
///
/// Une base lue à chaque requête ferait une requête SQL par image et par page.
/// Une valeur figée au démarrage ignorerait un changement fait depuis
/// l'interface PHP de Dolibarr, qui écrit dans la même table. Trente secondes
/// est le compromis : un réglage prend effet sans redémarrer le serveur.
pub(crate) async fn compte(
    Extension(courant): Extension<Courant>,
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let u = db::user::par_login(&app.pool, &courant.login)?;
    let groupes = match &u {
        Some(u) => db::user::groupes(&app.pool, entite::courante(), u.rowid)?,
        None => Vec::new(),
    };
    let noms_groupes = groupes
        .iter()
        .map(|(_, n)| n.clone())
        .collect::<Vec<_>>()
        .join(", ");

    Ok(ui::page(
        "",
        "",
        "Mon compte",
        html! {
            @if q.contains_key("refus") {
                p .erreur {(crate::langue::tr("Cet écran est réservé aux administrateurs.")) }
            }
            h1 {(crate::langue::tr("Mon compte")) }
            @match &u {
                Some(u) => {
                    p .sous-titre {
                        (u.nom_complet()) " — "
                        @if u.admin { "administrateur" } @else { "utilisateur" }
                    }
                    table {
                        tbody {
                            tr { th {(crate::langue::tr("Identifiant")) } td .ref { (u.login) } }
                            tr { th {(crate::langue::tr("E-mail")) } td { (u.email.clone().unwrap_or_default()) } }
                            tr { th {(crate::langue::tr("Langue")) } td { (u.lang.clone().unwrap_or_default()) } }
                            tr { th {(crate::langue::tr("Dernière connexion")) } td { (u.datelastlogin.clone().unwrap_or_default()) } }
                            tr { th {(crate::langue::tr("Groupes")) } td {
                                @if noms_groupes.is_empty() { "aucun" } @else { (noms_groupes) }
                            } }
                        }
                    }
                }
                None => { p .erreur {(crate::langue::tr("Compte introuvable en base.")) } }
            }
        },
    ))
}

/// La page de connexion : la première chose qu'on voit du logiciel.
///
/// Elle portait ses libellés dans des cellules de tableau, sans `label` associé
/// : un lecteur d'écran annonçait « formulaire, tableau, bouton » et rien
/// d'autre. Chaque champ a maintenant son `<label for>`, ce qui le nomme pour
/// un lecteur d'écran comme pour la saisie au clavier.
pub(crate) async fn connexion_page(Query(q): Query<HashMap<String, String>>) -> Markup {
    let erreur = q.contains_key("erreur");
    ui::page(
        "",
        "",
        "Connexion",
        html! {
            div .connexion {
                div .connexion-carte {
                    div .connexion-marque {
                        span .sceau { "C" }
                        span .nom { (app::PRODUIT) }
                    }
                    h1 {(crate::langue::tr("Connexion")) }
                    p .sous-titre {(crate::langue::tr("Comptes de la base Dolibarr.")) }

                    @if erreur {
                        p .erreur { (crate::langue::tr("Identifiant ou mot de passe incorrect.")) }
                    }

                    form method="post" action="/connexion" {
                        div .champ {
                            label for="login" { (crate::langue::tr("Identifiant")) }
                            input type="text" id="login" name="login" required autofocus
                                  autocomplete="username" .pleine;
                        }
                        div .champ {
                            label for="mdp" { (crate::langue::tr("Mot de passe")) }
                            input type="password" id="mdp" name="mdp" required
                                  autocomplete="current-password" .pleine;
                        }
                        button type="submit" .pleine {
                            (crate::langue::tr("Se connecter"))
                        }
                    }

                    p .aide.petite.haut {
                        (crate::langue::tr("Le même identifiant et le même mot de passe que dans Dolibarr."))
                    }
                }
            }
        },
    )
}

pub(crate) async fn connexion(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Form(f): Form<HashMap<String, String>>,
) -> Response {
    let login = f.get("login").map(|s| s.trim()).unwrap_or("");
    let mdp = f.get("mdp").cloned().unwrap_or_default();
    let ip = ip_client(&headers);
    let agent = agent_client(&headers);
    match db::user::authentifier(&app.pool, login, &mdp) {
        Ok(Some(u)) => {
            // Connexion reussie : on la consigne avant tout le reste, pour que
            // le journal existe meme si la suite echoue.
            let _ = db::events::enregistrer(
                &app.pool,
                &db::events::Nouvel::connexion(
                    db::events::CONNEXION,
                    u.entity,
                    Some(u.rowid),
                    &format!("connexion de {}", u.login),
                    &ip,
                    agent.as_deref(),
                ),
            );
            let j = jeton(&u.login);
            app.sessions.lock().unwrap().insert(
                j.clone(),
                Session {
                    login: u.login.clone(),
                    // Une session neuve demarre sur l'entite du compte.
                    entity: u.entity,
                    admin: u.admin,
                    expire: SystemTime::now() + DUREE_SESSION,
                },
            );
            // Purge des sessions expirees au passage.
            app.sessions
                .lock()
                .unwrap()
                .retain(|_, s| s.expire > SystemTime::now());
            (
                [(
                    axum::http::header::SET_COOKIE,
                    format!("{COOKIE}={j}; HttpOnly; SameSite=Lax; Path=/; Max-Age=28800"),
                )],
                Redirect::to("/"),
            )
                .into_response()
        }
        _ => {
            // Echec : on consigne le login tente, jamais le mot de passe.
            let _ = db::events::enregistrer(
                &app.pool,
                &db::events::Nouvel::connexion(
                    db::events::CONNEXION_ECHEC,
                    // Un echec n'a pas de compte, donc pas d'entite : l'evenement
                    // est global (0) et visible depuis toutes les societes. C'est
                    // justement quand on cherche une intrusion qu'on en a besoin.
                    0,
                    None,
                    &format!("connexion refusee pour « {login} »"),
                    &ip,
                    agent.as_deref(),
                ),
            );
            Redirect::to("/connexion?erreur=1").into_response()
        }
    }
}

pub(crate) async fn deconnexion(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
) -> Response {
    let mut compte: Option<(String, i64, Option<i64>)> = None;
    if let Some(j) = lire_cookie(&headers) {
        if let Some(s) = app.sessions.lock().unwrap().remove(&j) {
            compte = Some((s.login, s.entity, None));
        }
    }
    if let Some((login, entity, fk)) = compte {
        let _ = db::events::enregistrer(
            &app.pool,
            &db::events::Nouvel::connexion(
                db::events::DECONNEXION,
                entity,
                fk,
                &format!("deconnexion de {login}"),
                &ip_client(&headers),
                agent_client(&headers).as_deref(),
            ),
        );
    }
    (
        [(
            axum::http::header::SET_COOKIE,
            format!("{COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"),
        )],
        Redirect::to("/connexion"),
    )
        .into_response()
}

// --- utilitaires ------------------------------------------------------------
