//! Le garde-fou : qui entre, ce qu'il a le droit de faire, et quels en-tetes
//! il recoit.
//!
//! Deux couches, dans cet ordre d'application (la derniere declaree est la plus
//! exterieure) :
//!
//! 1. [`entetes_securite`] pose les en-tetes HTTP, **avant** tout le reste :
//!    une redirection vers la page de connexion part elle aussi avec sa CSP ;
//! 2. [`garde`] verifie la session, le mode lecture seule et les droits du
//!    module, puis etablit le contexte d'entite et d'utilisateur.
//!
//! # Pourquoi le contexte est pose ici
//!
//! Les ecrans lisent `entite::courante()` et `utilisateur::courant()`. Ces deux
//! valeurs viennent de la session, donc de ce fichier : un ecran ne peut pas
//! les deviner, et c'est voulu — une piece comptable doit porter l'entite et
//! l'auteur reels, pas ceux qu'un formulaire a bien voulu declarer.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};

use crate::*;

pub(crate) fn politique(app: &App, req: &axum::extract::Request) -> db::securite::Config {
    use std::time::{Duration, Instant};

    // L'entite vient de la session, pas de `entite::courante()` : ce middleware
    // est pose en couche *exterieure* a `garde`, donc avant que le contexte
    // d'entite ne soit etabli. Lire `courante()` ici renverrait toujours 1, et
    // une politique reglee sur l'entite 7 resterait invisible.
    let entity = crate::auth::lire_cookie(req.headers())
        .and_then(|j| {
            app.sessions
                .lock()
                .ok()
                .and_then(|s| s.get(&j).map(|s| s.entity))
        })
        .unwrap_or(1);

    // Le verrou ne doit couvrir **que** la lecture du cache. La version
    // precedente le tenait pendant la requete MySQL bloquante, sur chaque
    // requete HTTP : avec deux societes connectees, le cache mono-entite ne
    // prenait jamais et tout le serveur se serialisait derriere ce verrou.
    if let Ok(garde) = cache_politique().lock() {
        if let Some((quand, cache_entity, config)) = garde.as_ref() {
            if *cache_entity == entity && quand.elapsed() < Duration::from_secs(30) {
                return config.clone();
            }
        }
    }

    let config = db::securite::config(&app.pool, entity).unwrap_or_default();
    if let Ok(mut garde) = cache_politique().lock() {
        *garde = Some((Instant::now(), entity, config.clone()));
    }
    config
}

/// Le cache lui-même, pour pouvoir l'oublier.
fn cache_politique(
) -> &'static std::sync::Mutex<Option<(std::time::Instant, i64, db::securite::Config)>> {
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<Option<(std::time::Instant, i64, db::securite::Config)>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Oublie la politique en cache.
///
/// Appelé après un enregistrement depuis l'écran : sans cela, un réglage
/// prendrait effet jusqu'à trente secondes plus tard, et l'utilisateur
/// conclurait que le formulaire ne marche pas.
pub(crate) fn oublier_politique() {
    if let Ok(mut g) = cache_politique().lock() {
        *g = None;
    }
}

/// L'adresse de l'instance, pour l'en-tête CORS.
///
/// Déduite de l'en-tête `Host` : c'est la seule source qui ne se trompe pas
/// derrière un reverse proxy.
fn origine(req: &axum::extract::Request) -> String {
    req.headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|h| format!("http://{h}"))
        .unwrap_or_else(|| "http://localhost".to_string())
}

/// Les en-têtes de sécurité, sur chaque réponse.
///
/// Posé en couche extérieure, donc il couvre aussi la page de connexion et les
/// redirections. Une requête sans session n'a pas d'entité : elle reçoit la
/// politique de l'entité 1, qui est celle d'une installation neuve.
pub(crate) async fn entetes_securite(
    State(app): State<App>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let origine = origine(&req);
    let config = politique(&app, &req);
    let mut reponse = next.run(req).await;
    let entetes = reponse.headers_mut();
    for (nom, valeur) in db::securite::entetes(&config, &origine) {
        if let (Ok(nom), Ok(valeur)) = (
            axum::http::HeaderName::try_from(nom),
            axum::http::HeaderValue::try_from(valeur),
        ) {
            entetes.insert(nom, valeur);
        }
    }
    reponse
}

pub(crate) async fn garde(
    State(app): State<App>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let chemin = req.uri().path().to_string();
    if chemin == "/connexion" || chemin == "/installation" || chemin.starts_with("/public") {
        return next.run(req).await;
    }
    let courant = crate::auth::lire_cookie(req.headers()).and_then(|j| {
        // `auth::sessions` tolère l'empoisonnement du verrou : un panic survenu
        // sous verrou ne doit pas transformer le middleware — donc chaque
        // requête — en 500 définitif.
        let s = crate::auth::sessions(&app);
        s.get(&j)
            .filter(|s| s.expire > SystemTime::now())
            .map(|s| Courant {
                login: s.login.clone(),
                admin: s.admin,
            })
    });

    let Some(courant) = courant else {
        return Redirect::to("/connexion").into_response();
    };

    // Mode lecture seule : quelqu'un qui pointe ce noyau sur sa vraie base ne
    // doit pas pouvoir la modifier par accident. Une seule porte a fermer :
    // toutes les ecritures passent par une requete non-GET.
    if std::env::var("DOLIBARR_LECTURE_SEULE").is_ok() {
        let methode = req.method();
        if methode != axum::http::Method::GET && methode != axum::http::Method::HEAD {
            return (
                StatusCode::FORBIDDEN,
                "Mode lecture seule : aucune ecriture n'est acceptee sur cette installation.",
            )
                .into_response();
        }
    }

    // La configuration est reservee aux administrateurs.
    if chemin.starts_with("/parametres") && !courant.admin {
        return Redirect::to("/compte?refus=1").into_response();
    }

    // La barre de menus appartient a l'entite : on la lit a la premiere requete
    // de chaque entite, pas au demarrage ou l'on ne sait pas encore laquelle.
    crate::menu::charger_depuis_base(
        &app.pool,
        entite_de_session(&app, req.headers()).unwrap_or(1),
    );

    // Droits par ecran : un utilisateur non administrateur doit avoir le droit
    // du module. Le droit est cherche une fois par requete, pas par lien.
    if !courant.admin {
        // Un module declaratif peut porter son propre droit : il passe avant
        // la carte du noyau, plus precise sur le chemin.
        // Lire et ecrire ne sont pas le meme droit : une methode qui modifie
        // exige le droit d'ecriture du module, pas celui de lecture.
        let ecriture =
            req.method() != axum::http::Method::GET && req.method() != axum::http::Method::HEAD;
        let exiges: Vec<i64> = match manifeste::droit_du_chemin(&chemin) {
            Some(id) => vec![id],
            None if ecriture => db::droit::droits_ecriture_du_chemin(&chemin).to_vec(),
            None => db::droit::droits_du_chemin(&chemin).to_vec(),
        };
        // Une ecriture sans droit mappe n'est pas « libre » : c'est un trou.
        // On refuse, en le disant, plutot que d'ouvrir par omission — et on
        // nomme le chemin, sinon personne ne saura quoi corriger.
        let sans_droit_connu =
            ecriture && exiges.is_empty() && !crate::garde::autorise_sans_droit(&chemin);
        if sans_droit_connu {
            crate::journal_erreurs::enregistrer(
                &chemin,
                &format!(
                    "écriture sans droit mappé (utilisateur {}) : le module doit \
                     déclarer un droit, sinon chaque action est refusée",
                    courant.login
                ),
            );
            return (
                axum::http::StatusCode::FORBIDDEN,
                ui::page(
                    "accueil",
                    "",
                    "Acces refuse",
                    maud::html! {
                        h1 {(crate::langue::tr("Accès refusé")) }
                        p .sous-titre {
                            "Cet écran n'est associé à aucun droit d'écriture : "
                            "l'application refuse la modification par précaution. "
                            "Un administrateur doit déclarer le droit du module."
                        }
                        a .bouton href="/" {(crate::langue::tr("Retour au tableau de bord")) }
                    },
                ),
            )
                .into_response();
        }
        if !exiges.is_empty() {
            // Echec ferme : si la base ne repond pas, on ne sait pas si le
            // droit existe, et « ne pas savoir » ne vaut pas « autoriser ».
            let autorise = db::user::par_login(&app.pool, &courant.login)
                .ok()
                .flatten()
                .map(|u| {
                    db::groupe::droits_effectifs(&app.pool, entite::courante(), u.rowid)
                        .map(|d| exiges.iter().any(|id| d.contains(id)))
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if !autorise {
                return (
                    axum::http::StatusCode::FORBIDDEN,
                    ui::page(
                        "accueil",
                        "",
                        "Acces refuse",
                        maud::html! {
                            h1 {(crate::langue::tr("Accès refusé")) }
                            p .sous-titre {
                                "Votre compte n'a pas le droit d'ouvrir cet écran. "
                                "Un administrateur peut l'accorder dans Paramètres → Utilisateurs."
                            }
                            a .bouton href="/" {(crate::langue::tr("Retour au tableau de bord")) }
                        },
                    ),
                )
                    .into_response();
            }
        }
    }

    let mut req = req;
    req.extensions_mut().insert(courant.clone());
    // Toute la requete travaille dans l'entite de la session, et dans le
    // contexte de la personne connectee : un ecran qui ecrit une piece doit
    // pouvoir dire qui l'a creee sans le deviner.
    let entite_courante = entite_de_session(&app, req.headers()).unwrap_or(1);

    // Le style d'affichage voyage avec la personne, pas avec le navigateur :
    // deux postes, une seule apparence. Une lecture par requete sur une table
    // de preference (cle primaire) ne se sent pas.
    let style = db::user::par_login(&app.pool, &courant.login)
        .ok()
        .flatten()
        .and_then(|u| {
            db::preferences::lire_avec_defaut(&app.pool, entite_courante, u.rowid, "cordeau.style")
                .ok()
                .flatten()
        })
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "systeme".to_string());

    entite::avec(
        entite_courante,
        utilisateur::avec_style(courant.login.clone(), style, next.run(req)),
    )
    .await
}

/// Les ecritures qui ne demandent aucun droit de module.
///
/// Ce sont les gestes de la personne sur son propre compte, la deconnexion, et
/// les deux portes d'entree (connexion, installation) — toutes deja bornees
/// ailleurs : la connexion a son frein, l'installation n'accepte qu'une base
/// vierge, et changer d'entite ne fait que deplacer sa propre session.
fn autorise_sans_droit(chemin: &str) -> bool {
    const LIBRES: &[&str] = &[
        "/compte",
        "/style",
        "/entite",
        "/deconnexion",
        "/connexion",
        "/installation",
        "/recherche",
        "/public",
        "/api",
    ];
    LIBRES.iter().any(|p| chemin.starts_with(p))
}

/// Entite portee par le cookie de session, 1 si inconnue.
fn entite_de_session(app: &App, headers: &axum::http::HeaderMap) -> Option<i64> {
    let j = crate::auth::lire_cookie(headers)?;
    let s = crate::auth::sessions(app);
    s.get(&j).map(|x| x.entity)
}
