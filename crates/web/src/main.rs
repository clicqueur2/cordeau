//! Interface web : lecture et actions sur la base Dolibarr.
//!
//! Rendu cote serveur (maud), sans JavaScript. Les actions sont des `POST`
//! suivis d'une redirection ; le message de confirmation passe par `?ok=`.

mod actions_masse;
mod analytique_ecran;
mod api;
mod campagnes;
mod categories_ecran;
mod cloture_ecran;
mod courriel;
mod devises_ecran;
mod entite;
mod extrafields_ecran;
pub(crate) mod icones;
mod journal_erreurs;
mod langue;
mod lettrage_ecran;
mod livre_tenu;
mod manifeste;
mod menu;
mod menu_actif;
mod mrp_ecran;
mod pages;
mod plugin;
mod pos_ecran;
mod recur_ecran;
mod sepa_ecran;
mod stats_ecran;
pub(crate) mod ui;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Extension;
use axum::{Form, Router};
use db::Pool;
use maud::{html, Markup};
use rust_decimal::Decimal;

// L'entite courante vient de la session : voir `entite::courante()`.

#[derive(Clone, Debug)]
struct Session {
    login: String,
    /// Entite (societe) dans laquelle travaille la session.
    entity: i64,
    admin: bool,
    expire: SystemTime,
}

/// Utilisateur de la requete courante, depose par le garde-fou.
#[derive(Clone, Debug)]
struct Courant {
    login: String,
    admin: bool,
}

const DUREE_SESSION: Duration = Duration::from_secs(8 * 3600);
const COOKIE: &str = "dolibarr_sess";

#[derive(Clone)]
struct App {
    pool: Arc<Pool>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

pub struct Erreur(pub String);

impl IntoResponse for Erreur {
    fn into_response(self) -> Response {
        // Toute erreur servie au navigateur laisse une trace.
        crate::journal_erreurs::enregistrer("erreur", &self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            ui::page(
                "",
                "",
                "Erreur",
                html! {
                    h1 {(crate::langue::tr("Cette action n'a pas abouti")) }
                    p .erreur { (self.0) }
                    p { a href="/" {(crate::langue::tr("Retour au tableau de bord")) } }
                },
            ),
        )
            .into_response()
    }
}

impl From<Box<dyn std::error::Error>> for Erreur {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        Erreur(e.to_string())
    }
}

impl From<String> for Erreur {
    fn from(e: String) -> Self {
        Erreur(e)
    }
}

pub type Res<T> = Result<T, Erreur>;

#[tokio::main]
async fn main() {
    let dsn = std::env::var("DOLIBARR_DSN")
        .unwrap_or_else(|_| "mysql://root:root@127.0.0.1:3307/dolibarr".to_string());
    let pool = db::pool(&dsn).expect("connexion MySQL impossible");
    // Liste des entites (societes) pour le selecteur du bandeau.
    if let Err(e) = entite::charger(&pool) {
        eprintln!("liste des entites indisponible : {e}");
    }

    // Modules actives : la barre de menus en depend. On seme d'abord les
    // modules portes, puis on charge : l'ordre compte, sinon la barre est vide.
    if let Err(e) = db::module::semer_modules_portes(&pool, entite::courante()) {
        eprintln!("semis des modules impossible : {e}");
    }
    if let Err(e) = menu_actif::charger(&pool, entite::courante()) {
        eprintln!("chargement des modules impossibles : {e}");
    }

    // Definitions de droits : idempotent, une fois par demarrage.
    // Modules declaratifs : un fichier JSON suffit, sans recompiler.
    {
        let dossier = std::env::var("DOLIBARR_MODULES")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("modules"));
        match manifeste::charger(&pool, entite::courante(), &dossier) {
            Ok(0) => {}
            Ok(n) => println!(
                "{n} module(s) declare(s) charge(s) depuis {}",
                dossier.display()
            ),
            Err(e) => eprintln!("modules declaratifs : {e}"),
        }
    }

    if let Err(e) = db::droit::assurer_definitions(&pool, entite::courante()) {
        eprintln!("definitions de droits impossibles : {e}");
    }

    // Cle d'API : l'environnement prime, sinon on lit la constante Dolibarr.
    if std::env::var("DOLIBARR_API_KEY").is_err() {
        if let Ok(Some(cle)) = db::company::get_const(&pool, "DOLIBARR_API_KEY") {
            if !cle.trim().is_empty() {
                std::env::set_var("DOLIBARR_API_KEY", cle.trim());
            }
        }
    }

    let app = App {
        pool: Arc::new(pool),
        sessions: Arc::new(Mutex::new(HashMap::new())),
    };

    let pages = Router::new()
        .route(
            "/installation",
            get(pages::installation_page).post(pages::installation_creer),
        )
        .route("/connexion", get(connexion_page).post(connexion))
        .route("/deconnexion", post(deconnexion))
        .route("/compte", get(compte))
        .route("/entite", post(changer_entite))
        .route("/", get(accueil))
        .route("/recherche", get(recherche))
        .route("/tiers", get(tiers))
        .route("/tiers/:id", get(tiers_fiche))
        .route("/produits", get(produits))
        .route("/caisse", get(pos_ecran::page))
        .route("/caisse/ajouter", post(pos_ecran::ajouter))
        .route("/caisse/vider", post(pos_ecran::vider))
        .route("/caisse/encaisser", post(pos_ecran::encaisser))
        .route("/caisse/sceller", post(pos_ecran::sceller))
        .route("/caisse/archive", get(pos_ecran::archive))
        .route("/produits/:id", get(produit_fiche))
        .route(
            "/produits/:id/modifier",
            axum::routing::post(produit_enregistrer),
        )
        .route("/devis", get(devis))
        .route("/devis/actions", post(actions_masse::devis))
        .route("/devis/:id", get(devis_fiche))
        .route("/devis/:id/pdf", get(devis_pdf))
        .route("/devis/:id/facturer", post(devis_facturer))
        .route("/devis/:id/situation", post(devis_situation))
        .route("/devis/:id/commander", post(devis_commander))
        .route("/commandes", get(commandes))
        .route("/commandes/:id", get(commande_fiche))
        .route("/commandes/:id/valider", post(commande_valider))
        .route("/commandes/:id/facturer", post(commande_facturer))
        .route("/commandes/:id/acompte", post(commande_acompte))
        .route("/commandes/:id/pdf", get(commande_pdf))
        .route("/factures", get(factures))
        .route("/factures/actions", post(actions_masse::factures))
        .route("/devises", get(devises_ecran::page))
        .route("/mrp", get(mrp_ecran::page))
        .route("/categories", get(categories_ecran::page))
        .route("/categories/creer", post(categories_ecran::creer))
        .route("/categories/affecter", post(categories_ecran::affecter))
        .route("/categories/retirer", post(categories_ecran::retirer))
        .route("/statistiques", get(stats_ecran::statistiques))
        .route("/echeancier", get(stats_ecran::echeancier))
        .route("/devises/figer", post(devises_ecran::figer))
        .route("/factures/:id", get(factures_fiche))
        .route("/factures/:id/pdf", get(factures_pdf))
        .route("/factures/:id/valider", post(factures_valider))
        .route("/factures/:id/envoyer", post(courriel::facture))
        .route("/factures/:id/recurrence", post(factures_recurrence))
        .route("/factures/:id/reglement", post(factures_reglement))
        .route("/factures/:id/avoir", post(factures_avoir))
        .route(
            "/factures/nouveau",
            get(factures_nouveau).post(factures_creer),
        )
        .route("/relances", get(relances))
        .route("/chantiers", get(chantiers).post(chantier_creer))
        .route("/chantiers/:id", get(chantier_fiche))
        .route("/chantiers/:id/tache", post(chantier_tache_creer))
        .route(
            "/chantiers/:id/modifier",
            get(pages::chantier_modifier).post(pages::chantier_enregistrer),
        )
        .route(
            "/achats/fournisseur/:id/modifier",
            get(pages::fournisseur_fiche).post(pages::fournisseur_enregistrer),
        )
        .route(
            "/banques/:id/modifier",
            get(pages::compte_fiche).post(pages::compte_enregistrer),
        )
        .route("/achats", get(achats))
        .route(
            "/achats/facture",
            get(achat_facture_nouvelle).post(achat_facture_creer),
        )
        .route("/achats/facture/:id/valider", post(achat_facture_valider))
        .route("/achats/fournisseur", post(achat_fournisseur_creer))
        .route("/tickets", get(pages::tickets).post(pages::ticket_creer))
        .route(
            "/tickets/:id/fermer",
            axum::routing::post(pages::ticket_fermer),
        )
        .route("/liste/:code", get(pages::liste_simple_page))
        .route(
            "/liste/:code/nouveau",
            get(pages::liste_saisie_form).post(pages::liste_saisie_creer),
        )
        .route(
            "/liste/:code/fiche/:id",
            get(pages::fiche_saisie).post(pages::fiche_saisie_enregistrer),
        )
        .route(
            "/liste/:code/fiche/:id/supprimer",
            post(pages::fiche_saisie_supprimer),
        )
        .route("/contacts", get(pages::contacts).post(pages::contact_creer))
        .route(
            "/contacts/:id/modifier",
            get(pages::contact_fiche).post(pages::contact_enregistrer),
        )
        .route("/tickets/:id", get(pages::ticket_fiche))
        .route(
            "/tickets/:id/statut",
            axum::routing::post(pages::ticket_statut),
        )
        .route("/stock", get(pages::stock))
        .route("/expeditions", get(pages::expeditions))
        .route(
            "/stock/mouvement",
            axum::routing::post(pages::stock_mouvement),
        )
        .route(
            "/stock/entrepot",
            axum::routing::post(pages::stock_entrepot),
        )
        .route("/banques", get(banques).post(banque_creer))
        .route("/banques/rapprocher", post(banque_rapprocher))
        .route("/documents", get(documents))
        .route("/agenda", get(agenda))
        .route("/reglements", get(pages::reglements))
        .route("/avoirs", get(pages::avoirs))
        .route("/contrats", get(pages::contrats))
        .route("/parametres/journal", get(pages::journal))
        .route(
            "/parametres/module/:code",
            get(pages::options_module).post(pages::options_module_enregistrer),
        )
        .route(
            "/parametres/groupes",
            get(pages::groupes).post(pages::groupe_creer),
        )
        .route("/parametres/groupes/:id", get(pages::groupe_fiche))
        .route(
            "/parametres/groupes/:id/membres",
            axum::routing::post(pages::groupe_membres_enregistrer),
        )
        .route(
            "/parametres/groupes/:id/droits",
            axum::routing::post(pages::groupe_droits_enregistrer),
        )
        .route(
            "/parametres/utilisateurs/:id/droits",
            get(pages::utilisateur_droits).post(pages::utilisateur_droits_enregistrer),
        )
        .route("/achats/sepa", get(sepa_ecran::page))
        .route("/achats/sepa/fichier", get(sepa_ecran::fichier))
        .route("/campagnes", get(campagnes::page))
        .route("/campagnes/envoyer", post(campagnes::envoyer))
        .route("/recurrences", get(recur_ecran::page))
        .route("/recurrences/generer", post(recur_ecran::generer))
        .route("/parametres/greffons", get(pages::greffons_page))
        .route("/parametres/tables", get(pages::tables_page))
        .route("/parametres/tables/:nom", get(pages::table_page))
        .route("/parametres/evenements", get(pages::evenements_page))
        .route("/parametres/extrafields", get(extrafields_ecran::liste))
        .route(
            "/parametres/extrafields/:elementtype",
            get(extrafields_ecran::cible),
        )
        .route(
            "/extrafields/:elementtype/:id",
            get(extrafields_ecran::objet).post(extrafields_ecran::enregistrer),
        )
        .route("/parametres/emails/essai", post(pages::email_essai))
        .route(
            "/parametres/reglage/:cle",
            get(pages::reglages).post(pages::reglages_enregistrer),
        )
        .route("/comptabilite", get(comptabilite))
        .route("/comptabilite/grand-livre", get(grand_livre))
        .route("/comptabilite/lettrage", get(lettrage_ecran::page))
        .route(
            "/comptabilite/lettrage/:compte",
            post(lettrage_ecran::lettrer),
        )
        .route("/comptabilite/livre-tenu", get(livre_tenu::page))
        .route("/comptabilite/fec", get(fec_page).post(fec_generer))
        .route("/comptabilite/analytique", get(analytique_ecran::page))
        .route(
            "/comptabilite/analytique/axe",
            post(analytique_ecran::creer_axe),
        )
        .route(
            "/comptabilite/analytique/compte",
            post(analytique_ecran::creer_compte),
        )
        .route(
            "/comptabilite/analytique/affecter",
            post(analytique_ecran::affecter),
        )
        .route(
            "/comptabilite/cloture",
            get(cloture_ecran::page).post(cloture_ecran::cloturer),
        )
        .route("/comptabilite/journal", get(pages::journal_comptable))
        .route(
            "/commandes-fournisseurs",
            get(pages::commandes_fournisseurs),
        )
        .route("/agenda/nouveau", axum::routing::post(pages::agenda_creer))
        .route("/export/fec.txt", get(export_fec))
        .route("/parametres", get(parametres))
        .route("/parametres/societe", post(parametres_societe))
        .route("/parametres/cgv-modele", post(parametres_cgv_modele))
        .route("/parametres/dictionnaires", get(parametres_dictionnaires))
        .route(
            "/parametres/conditions-reglement",
            post(condition_reglement_creer),
        )
        .route(
            "/parametres/conditions-reglement/desactiver",
            post(condition_reglement_desactiver),
        )
        .route(
            "/parametres/modules",
            get(parametres_modules).post(parametres_module_basculer),
        )
        .route("/parametres/base", get(parametres_base))
        .route("/parametres/utilisateurs", get(utilisateurs))
        .route("/parametres/utilisateurs/nouveau", post(utilisateurs_creer))
        .route(
            "/parametres/utilisateurs/:id/basculer",
            post(utilisateurs_basculer),
        )
        .route("/parametres/import", get(import_page).post(import_tiers))
        .route("/parametres/import/produits", post(import_produits))
        .route("/parametres/import/factures", post(import_factures))
        .route("/export/tiers.csv", get(export_tiers))
        .route("/export/ventes.csv", get(export_ventes))
        .route("/export/achats.csv", get(export_achats))
        .route("/export/tva.csv", get(export_tva))
        .route("/export/grand-livre.csv", get(export_grand_livre))
        .route("/export/produits.csv", get(export_produits))
        .route(
            "/parametres/sauvegarde",
            get(sauvegarde_page).post(sauvegarder_maintenant),
        )
        .route("/tiers/nouveau", get(tiers_nouveau).post(tiers_creer))
        .route(
            "/produits/nouveau",
            get(produits_nouveau).post(produits_creer),
        )
        .route("/devis/nouveau", get(devis_nouveau).post(devis_creer))
        .route(
            "/devis/:id/modifier",
            get(devis_modifier).post(devis_enregistrer),
        )
        .route("/devis/:id/valider", post(devis_valider))
        .route(
            "/tiers/:id/modifier",
            get(tiers_modifier).post(tiers_enregistrer),
        )
        .layer(axum::middleware::from_fn_with_state(app.clone(), garde));

    // L'API a sa propre authentification (en-tete DOLAPIKEY) : elle ne passe
    // pas par la session du navigateur.
    let routes = pages
        .merge(Router::new().nest("/api", api::routes()))
        .with_state(app);

    let ecoute =
        std::env::var("DOLIBARR_WEB_ADDR").unwrap_or_else(|_| "127.0.0.1:8090".to_string());
    let listener = tokio::net::TcpListener::bind(&ecoute)
        .await
        .expect("port indisponible");
    println!("dolibarr-web sur http://{ecoute}");
    axum::serve(listener, routes).await.expect("serveur");
}

// --- session ----------------------------------------------------------------

fn jeton(login: &str) -> String {
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

fn lire_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
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
async fn changer_entite(
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

async fn garde(
    State(app): State<App>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let chemin = req.uri().path().to_string();
    if chemin == "/connexion" || chemin == "/installation" || chemin.starts_with("/public") {
        return next.run(req).await;
    }
    let courant = lire_cookie(req.headers()).and_then(|j| {
        let s = app.sessions.lock().unwrap();
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
        if !exiges.is_empty() {
            let autorise = db::user::par_login(&app.pool, &courant.login)
                .ok()
                .flatten()
                .map(|u| {
                    db::groupe::droits_effectifs(&app.pool, entite::courante(), u.rowid)
                        .map(|d| exiges.iter().any(|id| d.contains(id)))
                        .unwrap_or(true)
                })
                .unwrap_or(true);
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
    // Toute la requete travaille dans l'entite de la session.
    let entite_courante = entite_de_session(&app, req.headers()).unwrap_or(1);
    entite::avec(entite_courante, next.run(req)).await
}

/// Entite portee par le cookie de session, 1 si inconnue.
fn entite_de_session(app: &App, headers: &axum::http::HeaderMap) -> Option<i64> {
    let j = lire_cookie(headers)?;
    let s = app.sessions.lock().unwrap();
    s.get(&j).map(|x| x.entity)
}

async fn compte(
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

async fn connexion_page(Query(q): Query<HashMap<String, String>>) -> Markup {
    ui::page(
        "",
        "",
        "Connexion",
        html! {
            h1 {(crate::langue::tr("Connexion")) }
            @if q.contains_key("erreur") {
                p .erreur {(crate::langue::tr("Identifiant ou mot de passe incorrect.")) }
            }
            p .sous-titre {(crate::langue::tr("Comptes de la base Dolibarr.")) }
            form method="post" action="/connexion" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Identifiant")) } td { input type="text" name="login" required autofocus style="width:240px"; } }
                        tr { th {(crate::langue::tr("Mot de passe")) } td { input type="password" name="mdp" required style="width:240px"; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Se connecter")) } }
            }
        },
    )
}

async fn connexion(
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

async fn deconnexion(State(app): State<App>, headers: axum::http::HeaderMap) -> Response {
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

fn aujourdhui(pool: &Pool) -> Result<String, Erreur> {
    Ok(db::aujourdhui(pool)?)
}

fn en_retard(echeance: Option<&str>, statut: i16, paye: i16, jour: &str) -> bool {
    statut >= db::invoice::STATUT_VALIDEE
        && paye == 0
        && echeance.map(|d| d < jour).unwrap_or(false)
}

pub fn nom_tiers(pool: &Pool, fk: Option<i64>) -> Res<String> {
    match fk {
        None => Ok(String::new()),
        Some(id) => Ok(db::party::get(pool, entite::courante(), id)?
            .map(|p| p.nom)
            .unwrap_or_else(|| format!("(tiers {id} absent)"))),
    }
}

fn pluriel(n: usize, singulier: &str, pluriel: &str) -> String {
    if n > 1 {
        format!("{n} {pluriel}")
    } else {
        format!("{n} {singulier}")
    }
}

pub(crate) fn tracer(
    app: &App,
    fk_soc: Option<i64>,
    elementtype: &str,
    fk_element: i64,
    label: &str,
) {
    if let Ok(mut conn) = app.pool.get_conn() {
        let _ = db::journal::noter(
            &mut conn,
            entite::courante(),
            fk_soc,
            elementtype,
            fk_element,
            label,
        );
    }
}

pub fn message(ok: Option<&String>) -> Markup {
    match ok.map(|s| s.as_str()) {
        Some("reglement") => ui::flash("Règlement enregistré."),
        Some("validee") => ui::flash("Facture validée."),
        Some("envoyee") => ui::flash("Facture envoyée par e-mail, PDF joint."),
        Some("serie") => ui::flash("Série de factures créée : elle sera émise à l'échéance."),
        Some("echec-adresse") => ui::echec("Adresse e-mail invalide : rien n'a été envoyé."),
        Some("echec-pdf") => {
            ui::echec("Aucun PDF dans la GED pour cette facture : validez-la d'abord.")
        }
        Some("echec-config") => {
            ui::echec("Serveur SMTP non configuré : voir Paramètres > E-mails.")
        }
        Some("echec-envoi") => {
            ui::echec("L'envoi a échoué : le motif est dans le journal des erreurs.")
        }
        Some("facture") => ui::flash("Devis transformé en facture."),
        Some("supprime") => ui::flash("Supprimé."),
        Some("societe") => ui::flash("Paramètres enregistrés."),
        Some("enregistre") => ui::flash("Modifications enregistrées."),
        Some("valide") => ui::flash("Devis validé."),
        Some("compte") => ui::flash("Compte mis à jour."),
        Some("avoir") => ui::flash("Avoir créé : la facture d'origine est annulée comptablement."),
        _ => html! {},
    }
}

// --- tableau de bord --------------------------------------------------------

async fn accueil(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let jour = aujourdhui(p)?;

    let chiffres = db::tableau::chiffres(p, entity)?;
    let retard = db::tableau::en_retard(p, entity, &jour, 8)?;
    let sans_reponse = db::tableau::devis_sans_reponse(p, entity, &jour, 15, 6)?;
    let stock = db::tableau::stock_bas(p, entity)?;
    let total_retard = db::tableau::total_retard(&retard);

    let factures = db::invoice::list(p, entity, 6, 0)?;
    let recentes: Vec<_> = factures
        .iter()
        .filter(|f| f.fk_statut >= db::invoice::STATUT_VALIDEE)
        .collect();

    // Le nombre d'alertes en tete : c'est ce qui dit s'il y a quelque chose a
    // faire avant de regarder autre chose.
    let alertes = retard.len() + sans_reponse.len() + stock.len();

    Ok(ui::page(
        "accueil",
        "",
        "Tableau de bord",
        html! {
            h1 { (crate::langue::tr("Tableau de bord")) }
            p .sous-titre { (crate::langue::tr("Exercice en cours")) " — " (jour.clone()) }

            @if alertes > 0 {
                h2 { (crate::langue::tr("À traiter aujourd'hui")) }
                div .cartes {
                    (ui::carte(
                        "Factures en retard",
                        &retard.len().to_string(),
                        Some(&ui::euros(total_retard)),
                        !retard.is_empty(),
                    ))
                    (ui::carte(
                        "Devis sans réponse",
                        &sans_reponse.len().to_string(),
                        Some("depuis plus de 15 jours"),
                        !sans_reponse.is_empty(),
                    ))
                    (ui::carte(
                        "Stock sous le seuil",
                        &stock.len().to_string(),
                        Some("produits à réapprovisionner"),
                        !stock.is_empty(),
                    ))
                }
            } @else {
                (ui::flash("Rien en attente : aucune facture en retard, aucun devis oublié, aucun stock bas."))
            }

            h2 { (crate::langue::tr("Chiffres")) }
            div .cartes {
                (ui::carte(
                    "Facturé",
                    &ui::euros(chiffres.facture),
                    Some(&pluriel(chiffres.nb_factures as usize, "facture validée", "factures validées")),
                    false,
                ))
                (ui::carte("Encaissé", &ui::euros(chiffres.encaisse), None, false))
                (ui::carte(
                    "Reste à encaisser",
                    &ui::euros(chiffres.reste),
                    Some("encours client"),
                    chiffres.reste > Decimal::ZERO,
                ))
            }

            @if !retard.is_empty() {
                h2 { (crate::langue::tr("Factures en retard")) }
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Réf.")) }
                            th { (crate::langue::tr("Client")) }
                            th { (crate::langue::tr("Échéance")) }
                            th .num { (crate::langue::tr("Retard")) }
                            th .num { (crate::langue::tr("Reste dû")) }
                        }
                    }
                    tbody {
                        @for f in &retard {
                            tr {
                                td .ref { a href={ "/factures/" (f.rowid) } { (f.reference.clone()) } }
                                td { (f.tiers.clone()) }
                                td .ref { (f.echeance.clone()) }
                                td .num {
                                    span .pastille.p-retard { (f.jours.to_string()) " j" }
                                }
                                td .num { (ui::euros(f.reste)) }
                            }
                        }
                    }
                }
            }

            @if !sans_reponse.is_empty() {
                h2 { (crate::langue::tr("Devis sans réponse")) }
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Réf.")) }
                            th { (crate::langue::tr("Client")) }
                            th { (crate::langue::tr("Envoyé le")) }
                            th .num { (crate::langue::tr("Depuis")) }
                            th .num { (crate::langue::tr("TTC")) }
                        }
                    }
                    tbody {
                        @for d in &sans_reponse {
                            tr {
                                td .ref { a href={ "/devis/" (d.rowid) } { (d.reference.clone()) } }
                                td { (d.tiers.clone()) }
                                td .ref { (d.date.clone()) }
                                td .num { (d.jours.to_string()) " j" }
                                td .num { (ui::euros(d.montant)) }
                            }
                        }
                    }
                }
            }

            @if !stock.is_empty() {
                h2 { (crate::langue::tr("Stock sous le seuil")) }
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Réf.")) }
                            th { (crate::langue::tr("Produit")) }
                            th .num { (crate::langue::tr("Stock")) }
                            th .num { (crate::langue::tr("Seuil")) }
                        }
                    }
                    tbody {
                        @for s in &stock {
                            tr {
                                td .ref { (s.reference.clone()) }
                                td { (s.libelle.clone()) }
                                td .num { (s.stock.normalize().to_string()) }
                                td .num { (s.seuil.normalize().to_string()) }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Dernières factures")) }
            @if recentes.is_empty() {
                (ui::vide("Aucune facture pour l'instant.", Some(("/devis", "Partir d'un devis"))))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Réf.")) }
                            th { (crate::langue::tr("Client")) }
                            th { (crate::langue::tr("Échéance")) }
                            th .num { (crate::langue::tr("TTC")) }
                            th .num { (crate::langue::tr("Reste dû")) }
                            th { (crate::langue::tr("Statut")) }
                        }
                    }
                    tbody {
                        @for f in &recentes {
                            tr {
                                td .ref { a href={ "/factures/" (f.rowid) } { (f.ref_.clone()) } }
                                td { (nom_tiers(p, f.fk_soc)?) }
                                td { (f.date_lim_reglement.clone().unwrap_or_default()) }
                                td .num { (ui::euros(f.total_ttc.unwrap_or_default())) }
                                td .num { (ui::euros(db::payment::balance(p, entity, f.rowid)?.reste)) }
                                td { (ui::pastille(f.fk_statut, f.paye, en_retard(f.date_lim_reglement.as_deref(), f.fk_statut, f.paye, &jour))) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn recherche(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let terme = q
        .get("q")
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    let vide = terme.is_empty();

    // Un seul mot suffit a chercher : le nom du tiers pour une facture, la
    // reference pour une piece, le libelle pour un produit. Une recherche qui
    // ne regarde qu'un champ par objet ne trouve que ce qu'on tape deja.
    let tiers: Vec<_> = db::party::list(p, entite::courante(), 500, 0)?
        .into_iter()
        .filter(|t| {
            vide || t.nom.to_lowercase().contains(&terme)
                || t.code_client
                    .as_deref()
                    .map(|c| c.to_lowercase().contains(&terme))
                    .unwrap_or(false)
                || t.email
                    .as_deref()
                    .map(|c| c.to_lowercase().contains(&terme))
                    .unwrap_or(false)
        })
        .collect();

    let produits: Vec<_> = db::product::list(p, entite::courante(), 500, 0)?
        .into_iter()
        .filter(|x| {
            vide || x.ref_.to_lowercase().contains(&terme)
                || x.label.to_lowercase().contains(&terme)
        })
        .collect();

    let devis: Vec<_> = db::proposal::list(p, entite::courante(), 200, 0)?
        .into_iter()
        .filter(|d| vide || d.ref_.to_lowercase().contains(&terme))
        .collect();

    let factures: Vec<_> = db::invoice::list(p, entite::courante(), 200, 0)?
        .into_iter()
        .filter(|x| vide || x.ref_.to_lowercase().contains(&terme))
        .collect();

    let commandes: Vec<_> = db::commande::list(p, entite::courante(), 200, 0)?
        .into_iter()
        .filter(|c| vide || c.ref_.to_lowercase().contains(&terme))
        .collect();

    let projets: Vec<_> = db::projet::list(p, entite::courante(), 200, 0)?
        .into_iter()
        .filter(|x| {
            vide || x.title.to_lowercase().contains(&terme)
                || x.ref_.to_lowercase().contains(&terme)
        })
        .collect();

    // La recherche porte aussi sur le **nom du client** : chercher « Dupont »
    // doit sortir ses factures, pas seulement sa fiche.
    let par_client: Vec<i64> = tiers.iter().map(|t| t.rowid).collect();
    let devis: Vec<_> = if par_client.is_empty() {
        devis
    } else {
        db::proposal::list(p, entite::courante(), 200, 0)?
            .into_iter()
            .filter(|d| {
                (vide || d.ref_.to_lowercase().contains(&terme))
                    || d.fk_soc.map(|s| par_client.contains(&s)).unwrap_or(false)
            })
            .collect()
    };
    let factures: Vec<_> = if par_client.is_empty() {
        factures
    } else {
        db::invoice::list(p, entite::courante(), 200, 0)?
            .into_iter()
            .filter(|x| {
                (vide || x.ref_.to_lowercase().contains(&terme))
                    || x.fk_soc.map(|s| par_client.contains(&s)).unwrap_or(false)
            })
            .collect()
    };

    let total = tiers.len()
        + produits.len()
        + devis.len()
        + factures.len()
        + commandes.len()
        + projets.len();

    Ok(ui::page(
        "",
        "",
        "Recherche",
        html! {
            h1 {(crate::langue::tr("Recherche")) }
            form method="get" action="/recherche" {
                div .actions {
                    input type="search" name="q" value=(terme) placeholder="Nom, référence, e-mail…"
                          style="width:320px" aria-label="Recherche" autofocus;
                    button type="submit" {(crate::langue::tr("Chercher")) }
                }
            }
            p .sous-titre {
                @if vide {
                    "Tapez un nom, une référence, un e-mail : la recherche porte sur les tiers, "
                    "produits, devis, factures, commandes et chantiers."
                } @else {
                    (total.to_string()) " résultat(s) pour « " (terme) " »"
                }
            }

            @if !vide && total == 0 {
                (ui::vide("Rien ne correspond. Essayez le nom du client plutôt que sa référence, ou l'inverse.", None))
            }

            @if !tiers.is_empty() {
                h2 { "Tiers — " (tiers.len().to_string()) }
                table { tbody { @for t in &tiers {
                    tr {
                        td { a href={ "/tiers/" (t.rowid) } { (t.nom) } }
                        td .ref { (t.code_client.clone().unwrap_or_default()) }
                        td { (t.email.clone().unwrap_or_default()) }
                    }
                } } }
            }

            @if !produits.is_empty() {
                h2 { "Produits et services — " (produits.len().to_string()) }
                table { tbody { @for x in &produits {
                    tr {
                        td .ref { (x.ref_) }
                        td { (x.label) }
                    }
                } } }
            }

            @if !devis.is_empty() {
                h2 { "Devis — " (devis.len().to_string()) }
                table { tbody { @for d in &devis {
                    tr {
                        td .ref { a href={ "/devis/" (d.rowid) } { (d.ref_) } }
                        td { (nom_tiers(p, d.fk_soc)?) }
                        td .num { (ui::euros(d.total_ttc.unwrap_or_default())) }
                    }
                } } }
            }

            @if !commandes.is_empty() {
                h2 { "Commandes — " (commandes.len().to_string()) }
                table { tbody { @for c in &commandes {
                    tr {
                        td .ref { (c.ref_) }
                        td { (nom_tiers(p, Some(c.fk_soc))?) }
                        td .num { (ui::euros(c.total_ttc.unwrap_or_default())) }
                    }
                } } }
            }

            @if !factures.is_empty() {
                h2 { "Factures — " (factures.len().to_string()) }
                table { tbody { @for x in &factures {
                    tr {
                        td .ref { a href={ "/factures/" (x.rowid) } { (x.ref_) } }
                        td { (nom_tiers(p, x.fk_soc)?) }
                        td .num { (ui::euros(x.total_ttc.unwrap_or_default())) }
                    }
                } } }
            }

            @if !projets.is_empty() {
                h2 { "Chantiers — " (projets.len().to_string()) }
                table { tbody { @for x in &projets {
                    tr {
                        td .ref { (x.ref_) }
                        td { a href={ "/chantiers/" (x.rowid) } { (x.title) } }
                    }
                } } }
            }
        },
    ))
}

// --- tiers ------------------------------------------------------------------

async fn tiers(State(app): State<App>, Query(q): Query<HashMap<String, String>>) -> Res<Markup> {
    let filtre = q.get("q").cloned().unwrap_or_default().to_lowercase();
    let type_ = q.get("type").cloned().unwrap_or_default();
    // `type` vient d'une liste fermee : la condition SQL est choisie ici, jamais
    // construite a partir du texte saisi.
    let listes: Vec<_> = db::divers::tiers_filtres(&app.pool, entite::courante(), &type_, 500)?
        .into_iter()
        .filter(|p| filtre.is_empty() || p.nom.to_lowercase().contains(&filtre))
        .collect();
    let titre = match type_.as_str() {
        "prospect" => "Prospects",
        "fournisseur" => "Fournisseurs",
        "client" => "Clients",
        _ => "Tiers",
    };
    let onglet = match type_.as_str() {
        "prospect" => "prospects",
        "fournisseur" => "fournisseurs",
        _ => "liste",
    };

    Ok(ui::page(
        "tiers",
        onglet,
        titre,
        html! {
            h1 { (titre) }
            p .sous-titre { (listes.len()) " fiche(s)" }
            form method="get" action="/tiers" .actions {
                input type="text" name="q" placeholder="Nom du client" value=(filtre);
                @if !type_.is_empty() {
                    input type="hidden" name="type" value=(type_);
                }
                button type="submit" {(crate::langue::tr("Chercher")) }
            }
            @if listes.is_empty() {
                (ui::vide("Aucun tiers ne correspond.", Some(("/tiers", "Voir tous les tiers"))))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Nom")) } th {(crate::langue::tr("Ville")) } th {(crate::langue::tr("Type")) } } }
                    tbody {
                        @for p in &listes {
                            tr {
                                td .ref { a href={ "/tiers/" (p.rowid) } { (p.code_client.clone().unwrap_or_default()) } }
                                td { a href={ "/tiers/" (p.rowid) } { (p.nom) } }
                                td { (p.town.clone().unwrap_or_default()) }
                                td { (match p.client { 1 => "client", 2 => "prospect", 3 => "client et prospect", _ => "—" }) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn tiers_fiche(State(app): State<App>, Path(id): Path<i64>) -> Res<Markup> {
    let p = &app.pool;
    let t = db::party::get(p, entite::courante(), id)?.ok_or(Erreur("Tiers introuvable".into()))?;
    let jour = aujourdhui(p)?;
    let devis: Vec<_> = db::proposal::list(p, entite::courante(), 500, 0)?
        .into_iter()
        .filter(|d| d.fk_soc == Some(id))
        .collect();
    let factures: Vec<_> = db::invoice::list(p, entite::courante(), 500, 0)?
        .into_iter()
        .filter(|f| f.fk_soc == Some(id))
        .collect();
    let evts_tiers = db::journal::pour_tiers(p, entite::courante(), id, 20)?;

    Ok(ui::page(
        "tiers",
        "liste",
        &t.nom,
        html! {
            h1 { (t.nom) }
            p .sous-titre { "Code client " (t.code_client.clone().unwrap_or_default()) }
            div .actions {
                a .bouton.secondaire href={ "/tiers/" (id) "/modifier" } {(crate::langue::tr("Modifier la fiche")) }
            }

            div .fiche {
                div {
                    h2 style="margin-top:0" {(crate::langue::tr("Coordonnées")) }
                    dl {
                        dt {(crate::langue::tr("Adresse")) }
                        dd { (t.address.clone().unwrap_or_default()) }
                        dt {(crate::langue::tr("Ville")) }
                        dd { (format!("{} {}", t.zip.clone().unwrap_or_default(), t.town.clone().unwrap_or_default()).trim()) }
                        dt {(crate::langue::tr("Email")) }
                        dd { (t.email.clone().unwrap_or_default()) }
                        dt {(crate::langue::tr("Téléphone")) }
                        dd { (t.phone.clone().unwrap_or_default()) }
                    }
                }
                div {
                    h2 style="margin-top:0" {(crate::langue::tr("Identification")) }
                    dl {
                        dt {(crate::langue::tr("SIRET")) }
                        dd { (t.siret.clone().unwrap_or_default()) }
                        dt {(crate::langue::tr("TVA intracommunautaire")) }
                        dd { (t.tva_intra.clone().unwrap_or_default()) }
                    }
                }
            }

            @if !evts_tiers.is_empty() {
                h2 {(crate::langue::tr("Historique")) }
                table {
                    tbody {
                        @for e in &evts_tiers {
                            tr {
                                td .ref { (e.datep.clone().unwrap_or_default()) }
                                td { (e.label) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Devis")) }
            @if devis.is_empty() {
                (ui::vide("Aucun devis pour ce client.", None))
            } @else {
                table {
                    tbody {
                        @for d in &devis {
                            tr {
                                td .ref { a href={ "/devis/" (d.rowid) } { (d.ref_) } }
                                td .num { (ui::euros(d.total_ttc.unwrap_or_default())) }
                                td { (ui::pastille(d.fk_statut, 0, false)) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Factures")) }
            @if factures.is_empty() {
                (ui::vide("Aucune facture pour ce client.", None))
            } @else {
                table {
                    tbody {
                        @for f in &factures {
                            tr {
                                td .ref { a href={ "/factures/" (f.rowid) } { (f.ref_) } }
                                td .num { (ui::euros(f.total_ttc.unwrap_or_default())) }
                                td .num { (ui::euros(db::payment::balance(p, entite::courante(), f.rowid)?.reste)) }
                                td { (ui::pastille(f.fk_statut, f.paye, en_retard(f.date_lim_reglement.as_deref(), f.fk_statut, f.paye, &jour))) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

// --- produits ---------------------------------------------------------------

async fn produits(State(app): State<App>) -> Res<Markup> {
    let listes = db::product::list(&app.pool, entite::courante(), 500, 0)?;
    Ok(ui::page(
        "produits",
        "liste",
        "Produits",
        html! {
            h1 {(crate::langue::tr("Produits et services")) }
            p .sous-titre { (listes.len()) " référence(s)" }
            @if listes.is_empty() {
                (ui::vide("Aucun produit.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Libellé")) } th .num {(crate::langue::tr("PU HT")) } th {(crate::langue::tr("Type")) } th {(crate::langue::tr("Unité")) } } }
                    tbody {
                        @for pr in &listes {
                            tr {
                                td .ref { (pr.ref_) }
                                td { (pr.label) }
                                td .num { (ui::euros(pr.price.unwrap_or_default())) }
                                td { (if pr.est_service() { "service" } else { "produit" }) }
                                td { (pr.fk_unit.map(|u| u.to_string()).unwrap_or_default()) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

// --- devis ------------------------------------------------------------------

async fn devis(State(app): State<App>, Query(q): Query<HashMap<String, String>>) -> Res<Markup> {
    let p = &app.pool;
    let statut = q.get("statut").cloned().unwrap_or_default();
    let filtre = q
        .get("q")
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();

    let tous = db::proposal::list(p, entite::courante(), 200, 0)?;
    let total = tous.len();
    let listes: Vec<_> = tous
        .into_iter()
        .filter(|d| {
            let par_statut = match statut.as_str() {
                "brouillon" => d.fk_statut == 0,
                "valide" => d.fk_statut == 1,
                "signe" => d.fk_statut == 2,
                "perdu" => d.fk_statut == 3,
                _ => true,
            };
            let par_texte = filtre.is_empty() || d.ref_.to_lowercase().contains(&filtre);
            par_statut && par_texte
        })
        .collect();

    Ok(ui::page(
        "devis",
        "liste",
        "Devis",
        html! {
            h1 {(crate::langue::tr("Devis")) }
            form method="get" action="/devis" {
                div .actions {
                    select name="statut" aria-label="Statut" {
                        option value="" selected[statut.is_empty()] {(crate::langue::tr("Tous")) }
                        option value="brouillon" selected[statut == "brouillon"] {(crate::langue::tr("Brouillons")) }
                        option value="valide" selected[statut == "valide"] {(crate::langue::tr("Validés")) }
                        option value="signe" selected[statut == "signe"] {(crate::langue::tr("Signés")) }
                        option value="perdu" selected[statut == "perdu"] {(crate::langue::tr("Perdus")) }
                    }
                    input type="search" name="q" value=(filtre) placeholder="Référence…" aria-label="Recherche";
                    button type="submit" class="petit" {(crate::langue::tr("Filtrer")) }
                    @if !statut.is_empty() || !filtre.is_empty() {
                        a .bouton.secondaire.petit href="/devis" {(crate::langue::tr("Tout voir")) }
                    }
                }
            }
            p .sous-titre {
                (listes.len()) " devis"
                @if listes.len() != total {
                    " sur " (total.to_string())
                }
            }
            // Le formulaire est declare a part et les cases s'y rattachent par
            // leur attribut `form` : un formulaire ne peut pas entourer une
            // partie de tableau, et on ne va pas imbriquer des tables.
            form id="actions-devis" method="post" action="/devis/actions" {}
            div .actions {
                button type="submit" form="actions-devis" name="action" value="valider" {(crate::langue::tr("Valider la sélection")) }
                span style="color:#78736c;font-size:13.5px" {(crate::langue::tr("Seuls les devis en brouillon sont cochables."))
                }
            }
            @if listes.is_empty() {
                (ui::vide("Aucun devis.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Client")) } th .num {(crate::langue::tr("HT")) } th .num {(crate::langue::tr("TTC")) } th {(crate::langue::tr("Statut")) } th { "" } } }
                    tbody {
                        @for d in &listes {
                            tr {
                                td .ref {
                                    @if d.fk_statut == 0 {
                                        input type="checkbox" name={"id_" (d.rowid)}
                                              value="1" form="actions-devis"
                                              aria-label="sélectionner ce devis";
                                        " "
                                    }
                                    a href={ "/devis/" (d.rowid) } { (d.ref_) }
                                }
                                td { (nom_tiers(p, d.fk_soc)?) }
                                td .num { (ui::euros(d.total_ht.unwrap_or_default())) }
                                td .num { (ui::euros(d.total_ttc.unwrap_or_default())) }
                                td { (ui::pastille(d.fk_statut, 0, false)) }
                                td { a .bouton.petit.secondaire href={ "/devis/" (d.rowid) } {(crate::langue::tr("Ouvrir")) } }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn devis_fiche(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let d =
        db::proposal::get(p, entite::courante(), id)?.ok_or(Erreur("Devis introuvable".into()))?;
    let lignes = db::proposal::lines(p, entite::courante(), id)?;
    let reglement =
        db::reglement::condition(p, entite::courante(), d.fk_cond_reglement.unwrap_or(0))
            .ok()
            .flatten()
            .map(|c| c.libelle_fr())
            .filter(|l| !l.trim().is_empty());
    let situations = db::situation::situations(p, entite::courante(), id)?;
    let total_devis = db::situation::total_devis(p, entite::courante(), id)?;
    let deja_facture: Decimal = situations.iter().map(|s| s.total_ttc).sum();
    let reste_a_facturer = total_devis - deja_facture;

    Ok(ui::page(
        "devis",
        "liste",
        &d.ref_,
        html! {
            (message(q.get("ok")))
            h1 { "Devis " (d.ref_) " " (ui::pastille(d.fk_statut, 0, false)) }
            p .sous-titre { "Client : " strong { (nom_tiers(p, d.fk_soc)?) }
                @if let Some(r) = &reglement { " — règlement " (r) }
                @if let Some(a) = d.deposit_percent { " — acompte " (app::fmt_qte(Some(a))) " %" } }

            (table_lignes(
                lignes.iter().map(|l| (
                    l.is_section(),
                    l.label.clone().unwrap_or_default(),
                    app::fmt_dec(l.qty),
                    l.subprice.unwrap_or_default(),
                    app::fmt_dec(l.tva_tx),
                    l.total_ht.unwrap_or_default(),
                )),
                d.total_ht.unwrap_or_default(),
                d.total_tva.unwrap_or_default(),
                d.total_ttc.unwrap_or_default(),
                None,
            ))

            div .actions {
                a .bouton.secondaire href={ "/devis/" (id) "/pdf" } {(crate::langue::tr("Télécharger le PDF")) }
                @if d.fk_statut == 0 {
                    a .bouton.secondaire href={ "/devis/" (id) "/modifier" } {(crate::langue::tr("Modifier")) }
                    form method="post" action={ "/devis/" (id) "/valider" } {
                        button type="submit" {(crate::langue::tr("Valider le devis")) }
                    }
                }
                @if situations.is_empty() {
                    form method="post" action={ "/devis/" (id) "/facturer" } {
                        button type="submit" {(crate::langue::tr("Facturer ce devis")) }
                    }
                    form method="post" action={ "/devis/" (id) "/commander" } {
                        button type="submit" class="secondaire" {(crate::langue::tr("Créer la commande")) }
                    }
                } @else {
                    p style="color:#78736c;font-size:13px" {(crate::langue::tr("Ce devis est facturé par situations : la facture globale est désactivée pour éviter la double facturation."))
                    }
                }
            }

            h2 {(crate::langue::tr("Facturation par situations")) }
            p .sous-titre {
                "Le devis se facture par tranches. La somme des situations égale le devis ; "
                "la dernière prend le reste au centime."
            }
            @if situations.is_empty() {
                (ui::vide("Aucune situation facturée.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("N°")) } th {(crate::langue::tr("Facture")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("Part")) } th .num {(crate::langue::tr("TTC")) } th { "" } } }
                    tbody {
                        @for s in &situations {
                            tr {
                                td .ref { (s.counter) }
                                td .ref { a href={ "/factures/" (s.rowid) } { (s.ref_) } }
                                td .ref { (s.datef.clone().unwrap_or_default()) }
                                td .num { (app::fmt_qte(Some(s.pourcentage))) " %" }
                                td .num { (ui::euros(s.total_ttc)) }
                                td {
                                    @if s.finale {
                                        span .pastille.p-payee { "solde" }
                                    }
                                }
                            }
                        }
                    }
                    tfoot {
                        tr {
                            td colspan="4" .num {(crate::langue::tr("Reste à facturer")) }
                            td .num .total-fort { (ui::euros(reste_a_facturer)) }
                            td {}
                        }
                    }
                }
            }
            form method="post" action={ "/devis/" (id) "/situation" } .actions {
                input type="number" name="pct" step="0.01" min="0.01" max="100"
                      placeholder="%" style="width:110px" aria-label="Pourcentage" required;
                input type="date" name="date" aria-label="Date de la situation";
                button type="submit" {(crate::langue::tr("Créer la facture de situation")) }
            }
        },
    ))
}

fn table_lignes(
    lignes: impl Iterator<Item = (bool, String, String, Decimal, String, Decimal)>,
    total_ht: Decimal,
    total_tva: Decimal,
    total_ttc: Decimal,
    reste: Option<Decimal>,
) -> Markup {
    html! {
        table {
            thead { tr {
                th {(crate::langue::tr("Désignation")) } th .num {(crate::langue::tr("Qté")) } th .num {(crate::langue::tr("PU HT")) }
                th .num {(crate::langue::tr("TVA")) } th .num {(crate::langue::tr("Total HT")) }
            } }
            tbody {
                @for (section, label, qty, pu, tva, total) in lignes {
                    @if section {
                        tr { td colspan="5" style="background:#f6f4f0;font-weight:600" { (label) } }
                    } @else {
                        tr {
                            td { (label) }
                            td .num { (qty) }
                            td .num { (ui::euros(pu)) }
                            td .num { (tva) " %" }
                            td .num { (ui::euros(total)) }
                        }
                    }
                }
            }
            tfoot {
                tr { td colspan="4" .num {(crate::langue::tr("Total HT")) } td .num { (ui::euros(total_ht)) } }
                tr { td colspan="4" .num {(crate::langue::tr("TVA")) } td .num { (ui::euros(total_tva)) } }
                tr { td colspan="4" .num {(crate::langue::tr("Total TTC")) } td .num { (ui::euros(total_ttc)) } }
                @if let Some(r) = reste {
                    tr { td colspan="4" .num {(crate::langue::tr("Reste dû")) } td .num .total-fort { (ui::euros(r)) } }
                }
            }
        }
    }
}

async fn devis_pdf(State(app): State<App>, Path(id): Path<i64>) -> Res<Response> {
    let rendu = app::devis_pdf(&app.pool, entite::courante(), id)?;
    Ok(pdf_response(&rendu.bytes, &format!("devis-{id}.pdf")))
}

async fn devis_situation(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let pct: Decimal = f
        .get("pct")
        .map(|v| v.replace(',', "."))
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Erreur("pourcentage de la situation invalide".into()))?;
    let date = f
        .get("date")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let facture = db::situation::creer(&app.pool, entite::courante(), id, pct, date.as_deref())?;
    Ok(Redirect::to(&format!("/factures/{facture}?ok=situation")))
}

async fn devis_facturer(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let nouveau = db::invoice::create_from_proposal(&app.pool, entite::courante(), id)?;
    if let Some(d) = db::proposal::get(&app.pool, entite::courante(), id)? {
        tracer(
            &app,
            d.fk_soc,
            "propal",
            id,
            &format!("Devis {} facturé", d.ref_),
        );
    }
    Ok(Redirect::to(&format!("/factures/{nouveau}?ok=facture")))
}

// --- factures ---------------------------------------------------------------

async fn factures(State(app): State<App>, Query(q): Query<HashMap<String, String>>) -> Res<Markup> {
    let p = &app.pool;
    let jour = aujourdhui(p)?;
    let statut = q.get("statut").cloned().unwrap_or_default();
    let filtre = q
        .get("q")
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();

    let toutes = db::invoice::list(p, entite::courante(), 200, 0)?;
    let total = toutes.len();
    let listes: Vec<_> = toutes
        .into_iter()
        .filter(|f| {
            // Le statut vient d'une liste fermee : aucune valeur saisie n'arrive
            // jusqu'ici, donc pas de condition composee a partir du texte.
            let par_statut = match statut.as_str() {
                "brouillon" => f.fk_statut == 0,
                "validee" => f.fk_statut == 1 && f.paye == 0,
                "payee" => f.paye == 1,
                _ => true,
            };
            let par_texte = filtre.is_empty() || f.ref_.to_lowercase().contains(&filtre);
            par_statut && par_texte
        })
        .collect();

    Ok(ui::page(
        "factures",
        "liste",
        "Factures",
        html! {
            h1 {(crate::langue::tr("Factures")) }
            form method="get" action="/factures" {
                div .actions {
                    select name="statut" aria-label="Statut" {
                        option value="" selected[statut.is_empty()] {(crate::langue::tr("Toutes")) }
                        option value="brouillon" selected[statut == "brouillon"] {(crate::langue::tr("Brouillons")) }
                        option value="validee" selected[statut == "validee"] {(crate::langue::tr("Validées, non payées")) }
                        option value="payee" selected[statut == "payee"] {(crate::langue::tr("Payées")) }
                    }
                    input type="search" name="q" value=(filtre) placeholder="Référence…" aria-label="Recherche";
                    button type="submit" class="petit" {(crate::langue::tr("Filtrer")) }
                    @if !statut.is_empty() || !filtre.is_empty() {
                        a .bouton.secondaire.petit href="/factures" {(crate::langue::tr("Tout voir")) }
                    }
                }
            }
            p .sous-titre {
                (listes.len()) " facture(s)"
                @if listes.len() != total {
                    " sur " (total.to_string())
                }
            }
            form id="actions-factures" method="post" action="/factures/actions" {}
            div .actions {
                button type="submit" form="actions-factures" name="action" value="valider" {(crate::langue::tr("Valider la sélection")) }
                span style="color:#78736c;font-size:13.5px" {(crate::langue::tr("Seules les factures en brouillon sont cochables."))
                }
            }
            @if listes.is_empty() {
                (ui::vide("Aucune facture.", Some(("/devis", "Partir d'un devis"))))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Client")) } th {(crate::langue::tr("Échéance")) } th .num {(crate::langue::tr("TTC")) } th .num {(crate::langue::tr("Reste dû")) } th {(crate::langue::tr("Statut")) } } }
                    tbody {
                        @for f in &listes {
                            tr {
                                td .ref {
                                    @if f.fk_statut == 0 {
                                        input type="checkbox" name={"id_" (f.rowid)}
                                              value="1" form="actions-factures"
                                              aria-label="sélectionner cette facture";
                                        " "
                                    }
                                    a href={ "/factures/" (f.rowid) } { (f.ref_) }
                                }
                                td { (nom_tiers(p, f.fk_soc)?) }
                                td { (f.date_lim_reglement.clone().unwrap_or_default()) }
                                td .num { (ui::euros(f.total_ttc.unwrap_or_default())) }
                                td .num { (ui::euros(db::payment::balance(p, entite::courante(), f.rowid)?.reste)) }
                                td { (ui::pastille(f.fk_statut, f.paye, en_retard(f.date_lim_reglement.as_deref(), f.fk_statut, f.paye, &jour))) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn factures_fiche(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let jour = aujourdhui(p)?;
    let f =
        db::invoice::get(p, entite::courante(), id)?.ok_or(Erreur("Facture introuvable".into()))?;
    let lignes = db::invoice::lines(p, entite::courante(), id)?;
    let solde = db::payment::balance(p, entite::courante(), id)?;
    let reglements = db::payment::list_for_invoice(p, entite::courante(), id)?;
    let docs = db::ged::pour_element(p, entite::courante(), "facture", id)?;
    let evts = db::journal::pour_element(p, entite::courante(), "facture", id)?;
    let reglement =
        db::reglement::condition(p, entite::courante(), f.fk_cond_reglement.unwrap_or(0))
            .ok()
            .flatten()
            .map(|c| c.libelle_fr())
            .filter(|l| !l.trim().is_empty());

    // Le courriel propose : l'adresse du tiers, un objet qui porte la reference,
    // et un corps court. L'utilisateur garde la main, mais il n'a rien a taper.
    let courriel_tiers = match f.fk_soc {
        Some(soc) => db::party::get(p, entite::courante(), soc)?
            .and_then(|t| t.email)
            .unwrap_or_default(),
        None => String::new(),
    };
    let courriel_sujet = format!("Votre facture {} — Pierre Angulaire", f.ref_);
    let courriel_corps = format!(
        "Bonjour,\n\nVeuillez trouver ci-joint la facture {} d'un montant de {} €.\n\n\
         Bien cordialement,\n",
        f.ref_,
        f.total_ttc.unwrap_or_default().round_dp(2)
    );

    Ok(ui::page(
        "factures",
        "liste",
        &f.ref_,
        html! {
            (message(q.get("ok")))
            h1 { (if f.type_ == 2 { "Avoir " } else if f.type_ == db::invoice::TYPE_SITUATION { "Facture de situation " } else { "Facture " }) (f.ref_) " " (ui::pastille(f.fk_statut, f.paye, en_retard(f.date_lim_reglement.as_deref(), f.fk_statut, f.paye, &jour))) }
            p .sous-titre { "Client : " strong { (nom_tiers(p, f.fk_soc)?) }
                @if let Some(e) = &f.date_lim_reglement { " — échéance " (e) }
                @if let Some(r) = &reglement { " — règlement " (r) } }

            (table_lignes(
                lignes.iter().map(|l| (
                    l.is_section(),
                    l.label.clone().unwrap_or_default(),
                    app::fmt_dec(l.qty),
                    l.subprice.unwrap_or_default(),
                    app::fmt_dec(l.tva_tx),
                    l.total_ht.unwrap_or_default(),
                )),
                f.total_ht.unwrap_or_default(),
                f.total_tva.unwrap_or_default(),
                f.total_ttc.unwrap_or_default(),
                Some(solde.reste),
            ))

            div .actions {
                a .bouton.secondaire href={ "/factures/" (id) "/pdf" } {(crate::langue::tr("Télécharger le PDF")) }
                @if f.est_brouillon() {
                    form method="post" action={ "/factures/" (id) "/valider" } {
                        button type="submit" {(crate::langue::tr("Valider la facture")) }
                    }
                }
                @if f.fk_statut >= 1 && f.type_ != 2 {
                    form method="post" action={ "/factures/" (id) "/avoir" } {
                        button type="submit" class="secondaire" {(crate::langue::tr("Créer un avoir")) }
                    }
                }
            }

            @if !f.est_brouillon() {
                h2 {(crate::langue::tr("Envoyer par e-mail")) }
                form method="post" action={ "/factures/" (id) "/envoyer" } {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Destinataire")) }
                                td {
                                    input type="email" name="destinataire" value=(courriel_tiers)
                                          required style="width:300px";
                                }
                            }
                            tr {
                                th {(crate::langue::tr("Objet")) }
                                td { input type="text" name="sujet" value=(courriel_sujet) style="width:100%"; }
                            }
                            tr {
                                th {(crate::langue::tr("Message")) }
                                td { textarea name="corps" rows="5" style="width:100%" { (courriel_corps) } }
                            }
                        }
                    }
                    div .actions {
                        button type="submit" {(crate::langue::tr("Envoyer la facture")) }
                        span style="color:#78736c;font-size:13.5px" {(crate::langue::tr("Le PDF archivé part en pièce jointe. L'envoi est tracé."))
                        }
                    }
                }
            }

            @if !f.est_brouillon() {
                h2 {(crate::langue::tr("Facture récurrente")) }
                form method="post" action={ "/factures/" (id) "/recurrence" } {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Périodicité")) }
                                td {
                                    select name="periode" aria-label="Périodicité" {
                                        option value="mois" {(crate::langue::tr("Tous les mois")) }
                                        option value="trimestre" {(crate::langue::tr("Tous les trimestres")) }
                                        option value="semaine" {(crate::langue::tr("Toutes les semaines")) }
                                        option value="an" {(crate::langue::tr("Tous les ans")) }
                                    }
                                }
                                th {(crate::langue::tr("Prochaine échéance")) }
                                td { input type="date" name="prochaine" required; }
                                th {(crate::langue::tr("Nombre")) }
                                td {
                                    input type="number" name="maximum" min="0" value="0"
                                          title="0 = sans fin";
                                }
                            }
                        }
                    }
                    div .actions {
                        button type="submit" {(crate::langue::tr("Créer la série")) }
                        span style="color:#78736c;font-size:13.5px" {(crate::langue::tr("La série est une copie figée : modifier cette facture ne la change pas."))
                        }
                    }
                }
            }

            @if !solde.soldee && !f.est_brouillon() {
                h2 {(crate::langue::tr("Enregistrer un règlement")) }
                form method="post" action={ "/factures/" (id) "/reglement" } .actions {
                    input type="number" name="montant" step="0.01" min="0.01"
                          value=(solde.reste.to_string()) style="width:150px" aria-label="Montant";
                    select name="mode" aria-label="Mode de règlement" {
                        option value="virement" {(crate::langue::tr("Virement")) }
                        option value="cheque" {(crate::langue::tr("Chèque")) }
                        option value="especes" {(crate::langue::tr("Espèces")) }
                        option value="carte" {(crate::langue::tr("Carte")) }
                    }
                    button type="submit" {(crate::langue::tr("Encaisser")) }
                }
            }

            @if !docs.is_empty() {
                h2 {(crate::langue::tr("Documents archivés")) }
                table {
                    tbody {
                        @for (dossier, nom) in &docs {
                            tr {
                                td .ref { (nom) }
                                td { (dossier) }
                            }
                        }
                    }
                }
            }

            @if !evts.is_empty() {
                h2 {(crate::langue::tr("Historique")) }
                table {
                    tbody {
                        @for e in &evts {
                            tr {
                                td .ref { (e.datep.clone().unwrap_or_default()) }
                                td { (e.label) }
                            }
                        }
                    }
                }
            }

            @if !reglements.is_empty() {
                h2 {(crate::langue::tr("Règlements")) }
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("Montant")) } } }
                    tbody {
                        @for r in &reglements {
                            tr {
                                td .ref { (r.ref_) }
                                td { (r.datep.clone().unwrap_or_default()) }
                                td .num { (ui::euros(r.amount.unwrap_or_default())) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn factures_pdf(State(app): State<App>, Path(id): Path<i64>) -> Res<Response> {
    let rendu = app::facture_pdf(&app.pool, entite::courante(), id)?;
    Ok(pdf_response(&rendu.bytes, &format!("facture-{id}.pdf")))
}

async fn factures_avoir(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let avoir = db::invoice::create_avoir(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/factures/{avoir}?ok=avoir")))
}

async fn factures_valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    db::invoice::validate(&app.pool, entite::courante(), id)?;
    if let Some(f) = db::invoice::get(&app.pool, entite::courante(), id)? {
        tracer(
            &app,
            f.fk_soc,
            "facture",
            id,
            &format!("Facture {} validée", f.ref_),
        );
        // Le PDF part dans la GED : un document non archivé n'est pas une pièce.
        let _ = app::archiver_facture(&app.pool, entite::courante(), id);
    }
    Ok(Redirect::to(&format!("/factures/{id}?ok=validee")))
}

#[derive(serde::Deserialize)]
struct FormulaireReglement {
    montant: String,
    mode: String,
}

async fn factures_reglement(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(form): Form<FormulaireReglement>,
) -> Res<Redirect> {
    let montant = form
        .montant
        .replace(',', ".")
        .trim_matches('"')
        .parse::<Decimal>()
        .map_err(|_| Erreur("montant invalide".into()))?;
    let mode = match form.mode.as_str() {
        "virement" => db::payment::MODE_VIREMENT,
        "cheque" => db::payment::MODE_CHEQUE,
        "especes" => db::payment::MODE_ESPECES,
        "carte" => db::payment::MODE_CARTE,
        _ => db::payment::MODE_VIREMENT,
    };
    db::payment::create(
        &app.pool,
        entite::courante(),
        &db::payment::NewPayment {
            fk_facture: id,
            montant,
            fk_paiement: mode,
            ..Default::default()
        },
    )?;
    if let Some(f) = db::invoice::get(&app.pool, entite::courante(), id)? {
        tracer(
            &app,
            f.fk_soc,
            "facture",
            id,
            &format!(
                "Règlement de {} sur la facture {}",
                app::fmt_dec(Some(montant)),
                f.ref_
            ),
        );
    }
    Ok(Redirect::to(&format!("/factures/{id}?ok=reglement")))
}

// --- paramètres et dictionnaires --------------------------------------------

async fn chantiers(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let liste = db::projet::list(p, entite::courante(), 500, 0)?;
    let clients = db::party::list(p, entite::courante(), 500, 0)?;

    Ok(ui::page(
        "chantiers",
        "liste",
        "Chantiers",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Chantiers")) }
            p .sous-titre {(crate::langue::tr("Un chantier porte son budget et ses tâches : c'est le projet Dolibarr (llx_projet)."))
            }

            @if liste.is_empty() {
                (ui::vide("Aucun chantier.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Chantier")) } th {(crate::langue::tr("Client")) } th {(crate::langue::tr("Début")) } th {(crate::langue::tr("Fin")) } th .num {(crate::langue::tr("Budget")) } th { "État" } } }
                    tbody {
                        @for c in &liste {
                            tr {
                                td .ref { a href={ "/chantiers/" (c.rowid) } { (c.ref_) } }
                                td { a href={ "/chantiers/" (c.rowid) } { (c.title) } }
                                td { (nom_tiers(p, c.fk_soc)?) }
                                td .ref { (c.date_start.clone().unwrap_or_default()) }
                                td .ref { (c.date_end.clone().unwrap_or_default()) }
                                td .num { (ui::euros(c.budget.unwrap_or_default())) }
                                td {
                                    @if c.est_ouvert() {
                                        span .pastille.p-validee { "ouvert" }
                                    } @else {
                                        span .pastille.p-payee { "clos" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouveau chantier")) }
            form method="post" action="/chantiers" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" style="width:100%" {
                                    option value="" {(crate::langue::tr("— chantier interne —")) }
                                    @for c in &clients {
                                        option value=(c.rowid) { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Intitulé")) } td { input type="text" name="title" required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Début")) } td { input type="date" name="debut" style="width:170px"; } }
                        tr { th {(crate::langue::tr("Fin prévue")) } td { input type="date" name="fin" style="width:170px"; } }
                        tr { th {(crate::langue::tr("Budget HT")) } td { input type="number" name="budget" step="0.01" min="0" style="width:150px"; } }
                        tr { th {(crate::langue::tr("Description")) } td { input type="text" name="description" style="width:100%"; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le chantier")) } }
            }
        },
    ))
}

async fn chantier_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let titre = champ("title").ok_or_else(|| Erreur("intitulé du chantier obligatoire".into()))?;
    let mut nouveau = db::projet::NewProjet::new(&titre);
    nouveau.fk_soc = champ("soc").and_then(|v| v.parse().ok());
    nouveau.date_start = champ("debut");
    nouveau.date_end = champ("fin");
    nouveau.description = champ("description");
    nouveau.budget = f
        .get("budget")
        .map(|v| v.replace(',', "."))
        .and_then(|v| v.parse().ok())
        .unwrap_or_default();
    let id = db::projet::create(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to(&format!("/chantiers/{id}?ok=cree")))
}

async fn chantier_fiche(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let c =
        db::projet::get(p, entite::courante(), id)?.ok_or(Erreur("Chantier introuvable".into()))?;
    let taches = db::projet::taches(p, entite::courante(), id)?;

    Ok(ui::page(
        "chantiers",
        "liste",
        &c.ref_,
        html! {
            (message(q.get("ok")))
            h1 { (c.title) " " (if c.est_ouvert() { ui::pastille(1, 0, false) } else { ui::pastille(2, 1, false) }) }
            p .sous-titre {
                (c.ref_) " — client : " strong { (nom_tiers(p, c.fk_soc)?) }
                @if let Some(d) = &c.date_start { " — du " (d) }
                @if let Some(d) = &c.date_end { " au " (d) }
            }

            div .cartes {
                (ui::carte("Budget HT", &ui::euros(c.budget.unwrap_or_default()), None, false))
                (ui::carte("Tâches", &taches.len().to_string(), None, false))
            }

            @if let Some(d) = &c.description {
                p { (d) }
            }

            h2 {(crate::langue::tr("Tâches")) }
            @if taches.is_empty() {
                (ui::vide("Aucune tâche.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Tâche")) } th {(crate::langue::tr("Début")) } th {(crate::langue::tr("Fin")) } th .num {(crate::langue::tr("Avancement")) } } }
                    tbody {
                        @for t in &taches {
                            tr {
                                td .ref { (t.ref_.clone().unwrap_or_default()) }
                                td { (t.label) }
                                td .ref { (t.date_start.clone().unwrap_or_default()) }
                                td .ref { (t.date_end.clone().unwrap_or_default()) }
                                td .num { (t.progress) " %" }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouvelle tâche")) }
            form method="post" action={ "/chantiers/" (id) "/tache" } {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Libellé")) } td { input type="text" name="label" required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Début")) } td { input type="date" name="debut" style="width:170px"; } }
                        tr { th {(crate::langue::tr("Fin")) } td { input type="date" name="fin" style="width:170px"; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Ajouter la tâche")) } }
            }
        },
    ))
}

async fn chantier_tache_creer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let label = champ("label").ok_or_else(|| Erreur("libellé de la tâche obligatoire".into()))?;
    db::projet::creer_tache(
        &app.pool,
        entite::courante(),
        id,
        &label,
        champ("debut").as_deref(),
        champ("fin").as_deref(),
    )?;
    Ok(Redirect::to(&format!("/chantiers/{id}?ok=tache")))
}

async fn achats(State(app): State<App>, Query(q): Query<HashMap<String, String>>) -> Res<Markup> {
    let p = &app.pool;
    let liste = db::achat::fournisseurs(p, entite::courante(), 500, 0)?;
    let factures = db::achat::factures_fournisseur(p, entite::courante(), 100, 0)?;
    let paiements = db::divers::paiements_fournisseurs(p, entite::courante(), 50)?;
    let total = db::achat::total_achats(p, entite::courante())?;

    Ok(ui::page(
        "achats",
        "fournisseurs",
        "Achats",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Achats")) }
            p .sous-titre {(crate::langue::tr("Fournisseurs et factures fournisseurs (llx_societe fournisseur à 1, llx_facture_fourn)."))
            }

            div .cartes {
                (ui::carte("Achats (TTC)", &ui::euros(total), None, false))
                (ui::carte("Fournisseurs", &liste.len().to_string(), None, false))
            }

            h2 {(crate::langue::tr("Fournisseurs")) }
            @if liste.is_empty() {
                (ui::vide("Aucun fournisseur.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Nom")) } th {(crate::langue::tr("Ville")) } th {(crate::langue::tr("SIRET")) } th {(crate::langue::tr("E-mail")) } } }
                    tbody {
                        @for f in &liste {
                            tr {
                                td .ref { (f.code_fournisseur.clone().unwrap_or_default()) }
                                td { (f.nom) }
                                td { (f.town.clone().unwrap_or_default()) }
                                td .ref { (f.siret.clone().unwrap_or_default()) }
                                td { (f.email.clone().unwrap_or_default()) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouveau fournisseur")) }
            form method="post" action="/achats/fournisseur" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Code fournisseur")) } td { input type="text" name="code" placeholder="généré si vide" style="width:220px"; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le fournisseur")) } }
            }

            @if !paiements.is_empty() {
                h2 {(crate::langue::tr("Paiements fournisseurs")) }
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Mode")) } th .num {(crate::langue::tr("Montant")) } } }
                    tbody {
                        @for (ref_, date, montant, mode) in &paiements {
                            tr {
                                td .ref { (ref_) }
                                td .ref { (app::fmt_date_fr(date.as_deref())) }
                                td { (mode.clone().unwrap_or_default()) }
                                td .num { (ui::euros(*montant)) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Factures fournisseurs")) }
            div .actions {
                a .bouton href="/achats/facture" {(crate::langue::tr("Saisir une facture fournisseur")) }
            }
            @if factures.is_empty() {
                (ui::vide("Aucune facture fournisseur.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Chez le fournisseur")) } th {(crate::langue::tr("Fournisseur")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("TTC")) } th { "État" } th { "" } } }
                    tbody {
                        @for f in &factures {
                            tr {
                                td .ref { (f.ref_) }
                                td .ref { (f.ref_supplier.clone().unwrap_or_default()) }
                                td { (f.fournisseur.clone().unwrap_or_default()) }
                                td .ref { (f.datef.clone().unwrap_or_default()) }
                                td .num { (ui::euros(f.total_ttc.unwrap_or_default())) }
                                td { (ui::pastille(f.fk_statut, if f.paye { 1 } else { 0 }, false)) }
                                td {
                                    @if f.est_brouillon() {
                                        form method="post" action={ "/achats/facture/" (f.rowid) "/valider" } {
                                            button type="submit" class="secondaire petit" {(crate::langue::tr("Valider")) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn achat_facture_nouvelle(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let fournisseurs = db::achat::fournisseurs(&app.pool, entite::courante(), 500, 0)?;
    let tva = db::dict::taux_tva(&app.pool, entite::courante(), 1)?;

    Ok(ui::page(
        "achats",
        "facture",
        "Nouvelle facture fournisseur",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Nouvelle facture fournisseur")) }
            p .sous-titre {(crate::langue::tr("Saisie d'un achat reçu. La facture est créée en brouillon.")) }
            form method="post" action="/achats/facture" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Fournisseur")) }
                            td {
                                select name="soc" required style="width:100%" {
                                    @for f in &fournisseurs {
                                        option value=(f.rowid) { (f.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Référence chez le fournisseur")) } td { input type="text" name="ref_supplier" style="width:100%"; } }
                        tr { th {(crate::langue::tr("Date")) } td { input type="date" name="datef" style="width:170px"; } }
                        tr { th {(crate::langue::tr("Note interne")) } td { input type="text" name="note" style="width:100%"; } }
                    }
                }
                h2 {(crate::langue::tr("Lignes")) }
                table {
                    thead { tr { th {(crate::langue::tr("Désignation")) } th .num {(crate::langue::tr("Qté")) } th .num {(crate::langue::tr("PU HT")) } th {(crate::langue::tr("TVA")) } } }
                    tbody {
                        @for i in 1..=5 {
                            tr {
                                td { input type="text" name=(format!("l{i}_lib")) style="width:100%"; }
                                td { input type="number" name=(format!("l{i}_qte")) step="0.01" min="0" value="1" style="width:90px"; }
                                td { input type="number" name=(format!("l{i}_pu")) step="0.01" min="0" style="width:120px"; }
                                td {
                                    select name=(format!("l{i}_tva")) {
                                        @for t in &tva {
                                            option value=(t.taux) { (app::fmt_qte(Some(t.taux))) " %" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Créer la facture fournisseur")) }
                    a .bouton.secondaire href="/achats" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

async fn achat_facture_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let fk_soc = f
        .get("soc")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("fournisseur invalide".into()))?;
    let mut lignes = Vec::new();
    for i in 1..=5 {
        let lib = f.get(&format!("l{i}_lib")).map(|s| s.trim()).unwrap_or("");
        if lib.is_empty() {
            continue;
        }
        let qte: Decimal = f
            .get(&format!("l{i}_qte"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let pu: Decimal = f
            .get(&format!("l{i}_pu"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let tva: Decimal = f
            .get(&format!("l{i}_tva"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        lignes.push(db::achat_ecriture::LigneAchat::new(lib, qte, pu, tva));
    }
    if lignes.is_empty() {
        return Err(Erreur("au moins une ligne est nécessaire".into()));
    }
    let mut nouvelle = db::achat_ecriture::NouvelleFactureAchat::new(fk_soc);
    nouvelle.ref_supplier = f
        .get("ref_supplier")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    nouvelle.datef = f
        .get("datef")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    nouvelle.note = f
        .get("note")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    nouvelle.lines = lignes;
    db::achat_ecriture::creer(&app.pool, entite::courante(), &nouvelle)?;
    Ok(Redirect::to("/achats?ok=facture"))
}

async fn achat_facture_valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    db::achat_ecriture::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/achats?ok=facture-validee"))
}

async fn achat_fournisseur_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let nom = f
        .get("nom")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("nom du fournisseur obligatoire".into()))?;
    let code = f
        .get("code")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    db::achat::creer_fournisseur(&app.pool, entite::courante(), &nom, code.as_deref())?;
    Ok(Redirect::to("/achats?ok=fournisseur"))
}

async fn banques(State(app): State<App>, Query(q): Query<HashMap<String, String>>) -> Res<Markup> {
    let p = &app.pool;
    let liste = db::banque::comptes(p, entite::courante())?;
    let ecritures = db::banque::ecritures(p, entite::courante(), 40)?;

    let mut soldes = Vec::with_capacity(liste.len());
    for c in &liste {
        soldes.push(db::banque::solde(p, entite::courante(), c.rowid)?);
    }
    let nom_compte = |id: Option<i64>| -> String {
        id.and_then(|i| liste.iter().find(|c| c.rowid == i))
            .map(|c| format!("{} — {}", c.ref_, c.label))
            .unwrap_or_else(|| "—".to_string())
    };

    Ok(ui::page(
        "banques",
        "comptes",
        "Banques",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Banques")) }
            p .sous-titre {
                "Le solde se déduit des écritures : il n'est jamais stocké, comme dans Dolibarr. "
                "Les règlements de factures écrivent déjà leur ligne bancaire."
            }

            h2 {(crate::langue::tr("Comptes")) }
            @if liste.is_empty() {
                (ui::vide("Aucun compte bancaire.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Libellé")) } th {(crate::langue::tr("Banque")) } th {(crate::langue::tr("IBAN")) } th .num {(crate::langue::tr("Solde")) } th { "État" } } }
                    tbody {
                        @for (i, c) in liste.iter().enumerate() {
                            tr {
                                td .ref { (c.ref_) }
                                td { (c.label) }
                                td { (c.banque.clone().unwrap_or_default()) }
                                td .ref { (c.iban.clone().unwrap_or_default()) }
                                td .num { (ui::euros(soldes[i])) }
                                td {
                                    @if c.clos {
                                        span .pastille.p-brouillon { "clos" }
                                    } @else {
                                        span .pastille.p-payee { "ouvert" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouveau compte")) }
            form method="post" action="/banques" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Référence")) } td { input type="text" name="ref" required style="width:140px"; } }
                        tr { th {(crate::langue::tr("Libellé")) } td { input type="text" name="label" required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Banque")) } td { input type="text" name="banque" style="width:100%"; } }
                        tr { th {(crate::langue::tr("IBAN")) } td { input type="text" name="iban" style="width:100%"; } }
                        tr { th {(crate::langue::tr("BIC")) } td { input type="text" name="bic" style="width:260px"; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le compte")) } }
            }

            h2 {(crate::langue::tr("Dernières écritures")) }
            @if ecritures.is_empty() {
                (ui::vide("Aucune écriture. Encaisser une facture en crée une.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Libellé")) } th {(crate::langue::tr("Compte")) } th .num {(crate::langue::tr("Montant")) } th {(crate::langue::tr("Rapprochement")) } } }
                    tbody {
                        @for e in &ecritures {
                            tr {
                                td .ref { (e.dateo.clone().unwrap_or_default()) }
                                td { (e.label.clone().unwrap_or_default()) }
                                td { (nom_compte(e.fk_account)) }
                                td .num { (ui::euros(e.montant)) }
                                td {
                                    form method="post" action="/banques/rapprocher" {
                                        input type="hidden" name="id" value=(e.rowid);
                                        input type="hidden" name="fait" value=(if e.rapproche { "0" } else { "1" });
                                        button type="submit" class="secondaire petit" {
                                            (if e.rapproche { "Rapproché" } else { "Rapprocher" })
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn banque_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let reference = champ("ref").ok_or_else(|| Erreur("référence du compte obligatoire".into()))?;
    let label = champ("label").ok_or_else(|| Erreur("libellé du compte obligatoire".into()))?;
    db::banque::creer_compte(
        &app.pool,
        entite::courante(),
        &reference,
        &label,
        champ("banque").as_deref(),
        champ("iban").as_deref(),
        champ("bic").as_deref(),
    )?;
    Ok(Redirect::to("/banques?ok=compte"))
}

async fn banque_rapprocher(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let id = f
        .get("id")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("écriture non précisée".into()))?;
    let fait = f.get("fait").map(|v| v == "1").unwrap_or(true);
    db::banque::rapprocher(&app.pool, entite::courante(), id, fait)?;
    Ok(Redirect::to("/banques?ok=rapproche"))
}

async fn documents(State(app): State<App>) -> Res<Markup> {
    let docs = db::ged::tous(&app.pool, entite::courante(), 200)?;
    Ok(ui::page(
        "documents",
        "ged",
        "Documents",
        html! {
            h1 {(crate::langue::tr("Documents archivés")) }
            p .sous-titre {
                "Les PDF de devis et de factures sont déposés ici à chaque édition, "
                "rangés comme dans Dolibarr (documents/<module>/<réf>/)."
            }
            @if docs.is_empty() {
                (ui::vide("Aucun document archivé.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Libellé")) } th {(crate::langue::tr("Dossier")) } th {(crate::langue::tr("Fichier")) } } }
                    tbody {
                        @for d in &docs {
                            tr {
                                td .ref { (d.date_c.clone().unwrap_or_default()) }
                                td { (d.label) }
                                td .ref { (d.filepath) }
                                td .ref { (d.filename) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn agenda(State(app): State<App>) -> Res<Markup> {
    let evenements = db::journal::recents(&app.pool, entite::courante(), 200)?;
    Ok(ui::page(
        "agenda",
        "evenements",
        "Agenda",
        html! {
            h1 {(crate::langue::tr("Agenda")) }
            p .sous-titre {
                "Les événements sont écrits dans le journal Dolibarr (llx_actioncomm) : "
                "validation de devis, relances, règlements."
            }
            h2 {(crate::langue::tr("Nouvel événement")) }
            form method="post" action="/agenda/nouveau" .actions {
                input type="text" name="label" placeholder="Objet du rendez-vous" required style="width:320px";
                input type="date" name="date" aria-label="Date";
                button type="submit" {(crate::langue::tr("Ajouter")) }
            }

            @if evenements.is_empty() {
                (ui::vide("Aucun événement.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Événement")) } th {(crate::langue::tr("Objet")) } } }
                    tbody {
                        @for e in &evenements {
                            tr {
                                td .ref { (e.datep.clone().unwrap_or_default()) }
                                td { (e.label) }
                                td {
                                    @if let (Some(t), Some(id)) = (&e.elementtype, e.fk_element) {
                                        @if t == "facture" {
                                            a href={ "/factures/" (id) } { "Facture " (id) }
                                        } @else if t == "propal" {
                                            a href={ "/devis/" (id) } { "Devis " (id) }
                                        } @else {
                                            (t) " " (id)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn parametres(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let mut s = db::company::get(&app.pool, entite::courante())?;
    let mut retour: Option<String> = None;

    // Meme recette que pour les tiers : l'annuaire de l'Etat pre-remplit la fiche.
    if let Some(num) = q.get("siret").filter(|v| !v.trim().is_empty()) {
        match annuaire::par_siret(num) {
            Ok(Some(e)) => {
                s.nom = e.nom;
                s.adresse = Some(e.adresse).filter(|a| !a.is_empty());
                s.code_postal = Some(e.code_postal).filter(|a| !a.is_empty());
                s.ville = Some(e.commune).filter(|a| !a.is_empty());
                s.siret = Some(e.siret);
                retour = Some("Fiche completee depuis l'INSEE.".to_string());
            }
            Ok(None) => retour = Some("Aucune entreprise trouvee.".into()),
            Err(e) => retour = Some(format!("Annuaire indisponible : {e}")),
        }
    }
    if let Some(a) = q.get("adresse").filter(|v| !v.trim().is_empty()) {
        match annuaire::adresses(a, 1) {
            Ok(liste) if !liste.is_empty() => {
                let adr = &liste[0];
                s.adresse = Some(adr.numero_voie.clone());
                s.code_postal = Some(adr.code_postal.clone());
                s.ville = Some(adr.commune.clone());
                retour = Some(format!("Adresse retenue : {}", adr.libelle));
            }
            Ok(_) => retour = Some("Aucune adresse trouvee.".into()),
            Err(e) => retour = Some(format!("Base Adresse indisponible : {e}")),
        }
    }

    Ok(ui::page(
        "parametres",
        "societe",
        "Paramètres — société",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Société émettrice")) }
            p .sous-titre {(crate::langue::tr("Ces informations alimentent l'en-tête et les mentions de tous les documents.")) }

            @if let Some(m) = &retour {
                p .flash { (m) }
            }

            div .actions {
                form method="get" action="/parametres" {
                    input type="text" name="siret" placeholder="SIRET ou SIREN" style="width:190px";
                    button type="submit" {(crate::langue::tr("Chercher l'entreprise")) }
                }
                form method="get" action="/parametres" {
                    input type="text" name="adresse" placeholder="12 rue des Peupliers, Saint-Cyr" style="width:330px";
                    button type="submit" class="secondaire" {(crate::langue::tr("Chercher l'adresse")) }
                }
            }

            form method="post" action="/parametres/societe" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Denomination")) } td { input type="text" name="nom" value=(s.nom) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Forme juridique")) } td { input type="text" name="forme" value=(s.forme_juridique.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Capital")) } td { input type="text" name="capital" value=(s.capital.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("RCS")) } td { input type="text" name="rcs" value=(s.rcs.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Adresse")) } td { input type="text" name="adresse" value=(s.adresse.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Code postal")) } td { input type="text" name="cp" value=(s.code_postal.clone().unwrap_or_default()) style="width:120px"; } }
                        tr { th {(crate::langue::tr("Ville")) } td { input type="text" name="ville" value=(s.ville.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("SIRET")) } td { input type="text" name="siret" value=(s.siret.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("TVA intracommunautaire")) } td { input type="text" name="tva_intra" value=(s.tva_intra.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Email")) } td { input type="text" name="email" value=(s.email.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("IBAN")) } td { input type="text" name="iban" value=(s.iban.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("BIC")) } td { input type="text" name="bic" value=(s.bic.clone().unwrap_or_default()) style="width:260px"; } }
                        tr { th {(crate::langue::tr("Assurance décennale (assureur)")) } td { input type="text" name="assurance_assureur" value=(s.assurance_assureur.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Assurance (n° de police)")) } td { input type="text" name="assurance_police" value=(s.assurance_police.clone().unwrap_or_default()) style="width:260px"; } }
                        tr { th {(crate::langue::tr("Assurance (couverture)")) } td { input type="text" name="assurance_couverture" value=(s.assurance_couverture.clone().unwrap_or_default()) style="width:100%"; } }
                        tr {
                            th {(crate::langue::tr("TVA taux réduit (travaux)")) }
                            td {
                                select name="tva_taux_reduit" {
                                    option value="0" selected[!s.tva_taux_reduit] {(crate::langue::tr("Non")) }
                                    option value="1" selected[s.tva_taux_reduit] {(crate::langue::tr("Oui — attestation 279-0 bis imprimée")) }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("TVA sur les encaissements")) }
                            td {
                                select name="tva_encaissements" {
                                    option value="0" selected[!s.tva_sur_encaissements] {(crate::langue::tr("Non — TVA sur les débits")) }
                                    option value="1" selected[s.tva_sur_encaissements] {(crate::langue::tr("Oui — TVA exigible sur les encaissements")) }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Conditions générales de vente")) }
                            td {
                                textarea name="cgv" rows="6" style="width:100%;font:inherit;padding:8px 10px;border:1px solid #e6e2dc" {
                                    (s.cgv.clone().unwrap_or_default())
                                }
                                p style="color:#78736c;font-size:12.5px;margin:6px 0 0" {(crate::langue::tr("Imprimées en bas de chaque devis et facture. Une ligne fait un paragraphe."))
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Delai de paiement (jours)")) } td { input type="number" name="delai" value=(s.delai_paiement_jours) style="width:110px"; } }
                        tr {
                            th {(crate::langue::tr("Regime de TVA")) }
                            td {
                                select name="franchise" {
                                    option value="0" selected[!s.franchise_en_base] {(crate::langue::tr("Assujetti")) }
                                    option value="1" selected[s.franchise_en_base] {(crate::langue::tr("Franchise en base (art. 293 B)")) }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Enregistrer")) } }
            }
            form method="post" action="/parametres/cgv-modele" .actions {
                button type="submit" class="secondaire" {(crate::langue::tr("Insérer un modèle de CGV")) }
            }
        },
    ))
}

/// Modules / applications : catalogue du cœur, activation par constante
/// Dolibarr (`MAIN_MODULE_*` dans `llx_const`).
async fn parametres_modules(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let actifs = db::module::actifs(&app.pool, entite::courante())?;
    let familles: [(&str, &str); 11] = [
        ("base", "Base"),
        ("crm", "Commercial et relation client"),
        ("financial", "Financier"),
        ("products", "Produits et stocks"),
        ("projects", "Projets et chantiers"),
        ("srm", "Achats et fournisseurs"),
        ("hr", "Ressources humaines"),
        ("ecm", "Documents"),
        ("portal", "Portails et points de vente"),
        ("interface", "Interfaces et connecteurs"),
        ("technic", "Technique"),
    ];

    Ok(ui::page(
        "parametres",
        "modules",
        "Modules et applications",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Modules et applications")) }
            p .sous-titre {
                "L'activation s'écrit dans les constantes Dolibarr ("
                code { "llx_const" }
                "), pas dans une table propre : Dolibarr PHP lit le même état."
            }

            @for (cle, libelle) in familles {
                @let liste: Vec<_> = db::module::CATALOGUE.iter().filter(|m| m.famille == cle).collect();
                @if !liste.is_empty() {
                    h2 { (libelle) }
                    table {
                        thead { tr { th {(crate::langue::tr("Module")) } th {(crate::langue::tr("Nom technique")) } th {(crate::langue::tr("Statut")) } th { "" } } }
                        tbody {
                            @for m in liste {
                                @let actif = actifs.contains(&m.code.to_uppercase());
                                tr {
                                    td {
                                        strong { (m.nom) }
                                        @if m.porte {
                                            " "
                                            span .pastille.p-payee style="font-size:11px" {(crate::langue::tr("porté")) }
                                        }
                                    }
                                    td .ref { (m.code) }
                                    td {
                                        @if actif {
                                            span .pastille.p-validee {(crate::langue::tr("activé")) }
                                        } @else {
                                            span .pastille.p-brouillon { "inactif" }
                                        }
                                    }
                                    td .num {
                                        @if let Some(ecran) = plugin::par_code(m.code).and_then(|x| x.ecran) {
                                            a .bouton.petit.secondaire href=(ecran) style="margin-right:6px" {(crate::langue::tr("Ouvrir")) }
                                        }
                                        @if actif {
                                            @if plugin::par_code(m.code).is_some() {
                                                a .bouton.petit.secondaire href={ "/parametres/module/" (m.code) } style="margin-right:6px" title="Options du module" {(crate::langue::tr("Options")) }
                                            }
                                        }
                                        form method="post" action="/parametres/modules" style="display:inline" {
                                            input type="hidden" name="code" value=(m.code);
                                            input type="hidden" name="actif" value=(if actif { "0" } else { "1" });
                                            button type="submit" class="secondaire petit" {
                                                (if actif { "Désactiver" } else { "Activer" })
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn parametres_module_basculer(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let code = f
        .get("code")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("module non précisé".into()))?;
    let actif = f.get("actif").map(|v| v == "1").unwrap_or(false);
    db::module::definir_actif(&app.pool, entite::courante(), &code, actif)?;
    menu_actif::charger(&app.pool, entite::courante())?;
    // Un module qui s'active change ce que l'ERP sait faire : cela se consigne.
    let _ = db::events::enregistrer(
        &app.pool,
        &db::events::Nouvel {
            type_: if actif {
                db::events::MODULE_ACTIVE
            } else {
                db::events::MODULE_DESACTIVE
            },
            entity: entite::courante(),
            fk_user: None,
            description: &format!(
                "module {code} {}",
                if actif { "active" } else { "desactive" }
            ),
            ip: &ip_client(&headers),
            agent: agent_client(&headers).as_deref(),
            fk_object: None,
            methode: None,
        },
    );
    Ok(Redirect::to("/parametres/modules?ok=module"))
}

/// Insère un modèle de CGV dans les paramètres, pour ne pas partir d'une page
/// blanche. Le texte reste modifiable ensuite.
async fn parametres_cgv_modele(State(app): State<App>) -> Res<Redirect> {
    let mut s = db::company::get(&app.pool, entite::courante())?;
    s.cgv = Some(
        "Devis valable 30 jours ; la signature vaut acceptation des présentes conditions.\n\
         Paiement par virement sur le compte indiqué ci-dessous.\n\
         Les ouvrages sont garantis selon les garanties légales : parfait achèvement, \
         bon fonctionnement et garantie décennale.\n\
         Toute réclamation doit être adressée par écrit dans les huit jours suivant la réception."
            .to_string(),
    );
    db::company::save(&app.pool, &s)?;
    Ok(Redirect::to("/parametres?ok=cgv"))
}

async fn comptabilite(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let ventes = db::compta::ventes_par_mois(p, entite::courante(), 24)?;
    let achats = db::compta::achats_par_mois(p, entite::courante(), 24)?;
    let (collectee, deductible, solde) = db::compta::solde_tva(p, entite::courante())?;
    let ca_ht: Decimal = ventes.iter().map(|m| m.total_ht).sum();
    let achats_ht: Decimal = achats.iter().map(|m| m.total_ht).sum();

    Ok(ui::page(
        "comptabilite",
        "journaux",
        "Comptabilité",
        html! {
            h1 {(crate::langue::tr("Comptabilité")) }
            p .sous-titre {
                "Synthèse bâtie sur les factures validées, pas sur un grand livre : "
                "Dolibarr tient le grand livre dans llx_accounting_bookkeeping, qui demande "
                "une saisie par journal."
            }

            div .cartes {
                (ui::carte("Chiffre d'affaires HT", &ui::euros(ca_ht), Some(&pluriel(ventes.len(), "mois", "mois")), false))
                (ui::carte("Achats HT", &ui::euros(achats_ht), None, false))
                (ui::carte("TVA collectée", &ui::euros(collectee), None, false))
                (ui::carte("TVA déductible", &ui::euros(deductible), None, false))
                (ui::carte(
                    if solde >= Decimal::ZERO { "TVA à payer" } else { "Crédit de TVA" },
                    &ui::euros(solde.abs()),
                    Some("indicatif : à vérifier avant déclaration"),
                    solde > Decimal::ZERO,
                ))
            }

            div .actions {
                a .bouton.secondaire href="/comptabilite/grand-livre" {(crate::langue::tr("Grand livre et balance")) }
                a .bouton.secondaire href="/comptabilite/fec" {(crate::langue::tr("Comptabilisation et FEC")) }
            }

            h2 {(crate::langue::tr("Exports comptables")) }
            div .actions {
                form method="get" action="/export/ventes.csv" {
                    input type="date" name="debut" aria-label="Début";
                    input type="date" name="fin" aria-label="Fin";
                    button type="submit" {(crate::langue::tr("Journal des ventes (CSV)")) }
                }
                form method="get" action="/export/achats.csv" {
                    input type="date" name="debut" aria-label="Début";
                    input type="date" name="fin" aria-label="Fin";
                    button type="submit" class="secondaire" {(crate::langue::tr("Journal des achats (CSV)")) }
                }
                form method="get" action="/export/tva.csv" {
                    input type="date" name="debut" aria-label="Début";
                    input type="date" name="fin" aria-label="Fin";
                    button type="submit" class="secondaire" {(crate::langue::tr("TVA par taux (CSV)")) }
                }
            }

            h2 {(crate::langue::tr("Ventes par mois")) }
            @if ventes.is_empty() {
                (ui::vide("Aucune facture validée.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Mois")) } th .num {(crate::langue::tr("Documents")) } th .num {(crate::langue::tr("HT")) } th .num {(crate::langue::tr("TVA")) } th .num {(crate::langue::tr("TTC")) } } }
                    tbody {
                        @for m in &ventes {
                            tr {
                                td .ref { (m.periode) }
                                td .num { (m.documents) }
                                td .num { (ui::euros(m.total_ht)) }
                                td .num { (ui::euros(m.total_tva)) }
                                td .num { (ui::euros(m.total_ttc)) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Achats par mois")) }
            @if achats.is_empty() {
                (ui::vide("Aucune facture fournisseur validée.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Mois")) } th .num {(crate::langue::tr("Documents")) } th .num {(crate::langue::tr("HT")) } th .num {(crate::langue::tr("TVA")) } th .num {(crate::langue::tr("TTC")) } } }
                    tbody {
                        @for m in &achats {
                            tr {
                                td .ref { (m.periode) }
                                td .num { (m.documents) }
                                td .num { (ui::euros(m.total_ht)) }
                                td .num { (ui::euros(m.total_tva)) }
                                td .num { (ui::euros(m.total_ttc)) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn condition_reglement_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let code = champ("code").ok_or_else(|| Erreur("code de la condition obligatoire".into()))?;
    let libelle =
        champ("libelle").ok_or_else(|| Erreur("libellé de la condition obligatoire".into()))?;
    let nbjour: i64 = champ("nbjour").and_then(|v| v.parse().ok()).unwrap_or(0);
    let fin_de_mois = f.get("fin_de_mois").map(|v| v == "1").unwrap_or(false);
    db::reglement::creer_condition(
        &app.pool,
        entite::courante(),
        &code,
        &libelle,
        nbjour,
        fin_de_mois,
    )?;
    Ok(Redirect::to("/parametres/dictionnaires?ok=condition"))
}

async fn condition_reglement_desactiver(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let id = f
        .get("id")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("condition non précisée".into()))?;
    db::reglement::desactiver_condition(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/parametres/dictionnaires?ok=condition"))
}

async fn commandes(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let liste = db::commande::list(p, entite::courante(), 200, 0)?;

    Ok(ui::page(
        "commandes",
        "liste",
        "Commandes",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Commandes clients")) }
            p .sous-titre {(crate::langue::tr("Le maillon entre le devis signé et la facture.")) }
            @if liste.is_empty() {
                (ui::vide("Aucune commande. Elle se crée depuis un devis validé.", Some(("/devis", "Voir les devis"))))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Client")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("TTC")) } th { "État" } } }
                    tbody {
                        @for c in &liste {
                            tr {
                                td .ref { a href={ "/commandes/" (c.rowid) } { (c.ref_) } }
                                td { (nom_tiers(p, Some(c.fk_soc))?) }
                                td .ref { (app::fmt_date_fr(c.date_commande.as_deref())) }
                                td .num { (ui::euros(c.total_ttc.unwrap_or_default())) }
                                td {
                                    @match c.fk_statut {
                                        0 => { span .pastille.p-brouillon { "brouillon" } }
                                        1 => { span .pastille.p-validee {(crate::langue::tr("validée")) } }
                                        2 => { span .pastille.p-payee {(crate::langue::tr("livrée")) } }
                                        _ => { span .pastille.p-brouillon {(crate::langue::tr("annulée")) } }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn commande_fiche(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let c = db::commande::get(p, entite::courante(), id)?
        .ok_or(Erreur("Commande introuvable".into()))?;
    let lignes = db::commande::lines(p, entite::courante(), id)?;
    let acompte_existe = db::invoice::acompte_existe(&app.pool, entite::courante(), &c.ref_)?;

    Ok(ui::page(
        "commandes",
        "liste",
        &c.ref_,
        html! {
            (message(q.get("ok")))
            h1 { "Commande " (c.ref_) " " (ui::pastille(c.fk_statut, 0, false)) }
            p .sous-titre {
                "Client : " strong { (nom_tiers(p, Some(c.fk_soc))?) }
                @if let Some(d) = &c.date_commande { " — du " (app::fmt_date_fr(Some(d))) }
            }

            (table_lignes(
                lignes.iter().map(|l| (
                    l.is_section(),
                    l.label.clone().unwrap_or_default(),
                    app::fmt_qte(l.qty),
                    l.subprice.unwrap_or_default(),
                    app::fmt_qte(l.tva_tx),
                    l.total_ht.unwrap_or_default(),
                )),
                c.total_ht.unwrap_or_default(),
                c.total_tva.unwrap_or_default(),
                c.total_ttc.unwrap_or_default(),
                None,
            ))

            div .actions {
                @if c.est_brouillon() {
                    form method="post" action={ "/commandes/" (id) "/valider" } {
                        button type="submit" {(crate::langue::tr("Valider la commande")) }
                    }
                } @else {
                    form method="post" action={ "/commandes/" (id) "/facturer" } {
                        button type="submit" {(crate::langue::tr("Facturer la commande")) }
                    }
                }
                @if c.deposit_percent.unwrap_or_default() > rust_decimal::Decimal::ZERO && !acompte_existe {
                    form method="post" action={ "/commandes/" (id) "/acompte" } {
                        button type="submit" {
                            "Créer l'acompte (" (app::fmt_qte(c.deposit_percent)) " %)"
                        }
                    }
                }
                a .bouton.secondaire href={ "/commandes/" (id) "/pdf" } {(crate::langue::tr("Télécharger le PDF")) }
                a .bouton.secondaire href="/commandes" {(crate::langue::tr("Retour")) }
            }
        },
    ))
}

async fn commande_pdf(State(app): State<App>, Path(id): Path<i64>) -> Res<Response> {
    let rendu = app::commande_pdf(&app.pool, entite::courante(), id)?;
    Ok(pdf_response(&rendu.bytes, &format!("commande-{id}.pdf")))
}

async fn commande_facturer(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let facture = db::invoice::create_from_commande(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/factures/{facture}?ok=facturee")))
}

/// Cree la facture d'acompte d'une commande (type 3, une ligne par taux).
async fn commande_acompte(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let facture = db::invoice::create_deposit_from_commande(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/factures/{facture}?ok=acompte")))
}

async fn commande_valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    db::commande::validate(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/commandes/{id}?ok=validee")))
}

async fn devis_commander(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let commande = db::commande::create_from_proposal(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/commandes/{commande}?ok=cree")))
}

async fn parametres_dictionnaires(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let unites = db::dict::unites(p)?;
    let tva = db::dict::taux_tva(p, entite::courante(), 1)?;
    let modes = db::dict::modes_paiement(p)?;
    let conditions = db::reglement::conditions(p, entite::courante())?;
    let recensement = db::dict::recensement(p)?;

    Ok(ui::page(
        "parametres",
        "dictionnaires",
        "Paramètres — dictionnaires",
        html! {
            h1 {(crate::langue::tr("Dictionnaires")) }
            p .sous-titre {(crate::langue::tr("Lus directement dans la base Dolibarr, jamais recopiés. Ajouter une entrée dans Dolibarr la rend disponible ici.")) }

            h2 {(crate::langue::tr("Taux de TVA")) }
            table {
                thead { tr { th {(crate::langue::tr("Taux")) } th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Note")) } } }
                tbody {
                    @for t in &tva {
                        tr { td .num { (app::fmt_dec(Some(t.taux))) " %" } td { (t.code) } td { (t.libelle) } }
                    }
                }
            }

            h2 {(crate::langue::tr("Conditions de règlement")) }
            table {
                thead { tr { th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Libellé client")) } th .num {(crate::langue::tr("Délai")) } th { "" } } }
                tbody {
                    @for c in &conditions {
                        tr {
                            td .ref { (c.code) }
                            td { (c.libelle_fr()) }
                            td .num { (c.nbjour) " j" }
                            td {
                                form method="post" action="/parametres/conditions-reglement/desactiver" {
                                    input type="hidden" name="id" value=(c.rowid);
                                    button type="submit" class="secondaire petit" {(crate::langue::tr("Désactiver")) }
                                }
                            }
                        }
                    }
                }
            }

            h3 {(crate::langue::tr("Nouvelle condition")) }
            form method="post" action="/parametres/conditions-reglement" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Code")) } td { input type="text" name="code" required placeholder="RECEPTION" style="width:180px"; } }
                        tr { th {(crate::langue::tr("Libellé client")) } td { input type="text" name="libelle" required placeholder="Règlement à réception de facture, par virement" style="width:100%"; } }
                        tr { th {(crate::langue::tr("Délai (jours)")) } td { input type="number" name="nbjour" min="0" value="0" style="width:110px"; } }
                        tr {
                            th {(crate::langue::tr("Fin de mois")) }
                            td {
                                select name="fin_de_mois" {
                                    option value="0" {(crate::langue::tr("Non")) }
                                    option value="1" {(crate::langue::tr("Oui — dernier jour du mois")) }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer la condition")) } }
            }

            h2 {(crate::langue::tr("Modes de paiement")) }
            table {
                thead { tr { th {(crate::langue::tr("Id")) } th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Libellé")) } } }
                tbody {
                    @for m in &modes { tr { td .num { (m.id) } td .ref { (m.code) } td { (m.libelle) } } }
                }
            }

            h2 {(crate::langue::tr("Unités de mesure")) }
            table {
                thead { tr { th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Libellé")) } } }
                tbody {
                    @for u in &unites { tr { td .ref { (u.code) } td { (u.libelle) } } }
                }
            }

            h2 {(crate::langue::tr("Toutes les tables de dictionnaire")) }
            p .sous-titre { (recensement.len()) " tables llx_c_*" }
            table {
                thead { tr { th {(crate::langue::tr("Table")) } th .num {(crate::langue::tr("Lignes")) } } }
                tbody {
                    @for (t, n) in &recensement {
                        tr { td .ref { (t) } td .num { (n) } }
                    }
                }
            }
        },
    ))
}

async fn parametres_base(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let i = db::infos_base(p)?;
    let modules = db::modules_actifs(p)?;
    let s = db::company::get(p, entite::courante())?;
    let manquants = legal::champs_manquants(
        legal::DocType::Facture,
        &app::societe_legale(&s),
        &legal::Client {
            nom: "client".into(),
            ..Default::default()
        },
    );

    Ok(ui::page(
        "parametres",
        "base",
        "Paramètres — base de données",
        html! {
            h1 {(crate::langue::tr("Base de données")) }
            p .sous-titre {(crate::langue::tr("Connexion et état du schéma Dolibarr")) }

            table {
                tbody {
                    tr { th {(crate::langue::tr("Serveur")) } td { (i.version_serveur) } }
                    tr { th {(crate::langue::tr("Hôte")) } td { (i.hote) ":" (i.port) } }
                    tr { th {(crate::langue::tr("Base")) } td { (i.base) } }
                    tr { th {(crate::langue::tr("Tables")) } td .num { (i.tables) } }
                    tr { th {(crate::langue::tr("Colonnes")) } td .num { (i.colonnes) } }
                    tr { th {(crate::langue::tr("Schéma de référence")) } td {(crate::langue::tr("Dolibarr 24.0.1")) } }
                    tr {
                        th {(crate::langue::tr("Conformité de la société")) }
                        td {
                            @if manquants.is_empty() {
                                span .pastille.p-payee {(crate::langue::tr("complète")) }
                            } @else {
                                span .pastille.p-retard { (manquants.join(", ")) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Modules Dolibarr activés")) }
            @if modules.is_empty() {
                (ui::vide("Aucun module activé dans llx_const.", None))
            } @else {
                p { (modules.join(" · ")) }
            }
        },
    ))
}

async fn parametres_societe(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let mut s = db::company::get(&app.pool, entite::courante())?;
    let lire = |cle: &str| f.get(cle).map(|v| v.trim().to_string());
    s.nom = lire("nom").unwrap_or(s.nom);
    s.forme_juridique = lire("forme");
    s.capital = lire("capital");
    s.rcs = lire("rcs");
    s.adresse = lire("adresse");
    s.code_postal = lire("cp");
    s.ville = lire("ville");
    s.siret = lire("siret");
    s.tva_intra = lire("tva_intra");
    s.email = lire("email");
    // Champs qui etaient affiches mais jamais enregistres : IBAN, BIC,
    // assurance et CGV. Un formulaire qui ne sauvegarde pas est un piege.
    s.iban = lire("iban");
    s.bic = lire("bic");
    s.assurance_assureur = lire("assurance_assureur");
    s.assurance_police = lire("assurance_police");
    s.assurance_couverture = lire("assurance_couverture");
    s.cgv = lire("cgv");
    s.tva_sur_encaissements = f
        .get("tva_encaissements")
        .map(|v| v == "1")
        .unwrap_or(false);
    s.tva_taux_reduit = f.get("tva_taux_reduit").map(|v| v == "1").unwrap_or(false);
    s.franchise_en_base = f.get("franchise").map(|v| v == "1").unwrap_or(false);
    s.delai_paiement_jours = f
        .get("delai")
        .and_then(|v| v.parse().ok())
        .unwrap_or(s.delai_paiement_jours);
    db::company::save(&app.pool, &s)?;
    Ok(Redirect::to("/parametres?ok=societe"))
}

// --- creation ---------------------------------------------------------------

async fn tiers_nouveau(
    State(_app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let mut nom = String::new();
    let mut adresse = String::new();
    let mut cp = String::new();
    let mut ville = String::new();
    let mut siret = String::new();
    let mut retour: Option<String> = None;

    // Remplissage depuis l'annuaire de l'Etat, sans JavaScript : deux GET.
    if let Some(s) = q.get("siret").filter(|s| !s.trim().is_empty()) {
        match annuaire::par_siret(s) {
            Ok(Some(e)) => {
                nom = e.nom.clone();
                adresse = e.adresse.clone();
                cp = e.code_postal.clone();
                ville = e.commune.clone();
                siret = e.siret.clone();
                retour = Some(format!("Fiche remplie depuis l'INSEE : {}", e.nom));
            }
            Ok(None) => retour = Some("Aucune entreprise trouvee pour ce numero.".into()),
            Err(e) => retour = Some(format!("Annuaire indisponible : {e}")),
        }
    }
    if let Some(a) = q.get("adresse").filter(|s| !s.trim().is_empty()) {
        match annuaire::adresses(a, 5) {
            Ok(liste) if !liste.is_empty() => {
                let adr = &liste[0];
                adresse = adr.numero_voie.clone();
                cp = adr.code_postal.clone();
                ville = adr.commune.clone();
                retour = Some(format!("Adresse retenue : {}", adr.libelle));
            }
            Ok(_) => retour = Some("Aucune adresse trouvee.".into()),
            Err(e) => retour = Some(format!("Base Adresse indisponible : {e}")),
        }
    }

    Ok(ui::page(
        "tiers",
        "nouveau",
        "Nouveau tiers",
        html! {
            h1 {(crate::langue::tr("Nouveau tiers")) }
            @if let Some(m) = &retour {
                p .flash { (m) }
            }

            h2 {(crate::langue::tr("Remplir automatiquement")) }
            div .actions {
                form method="get" action="/tiers/nouveau" {
                    input type="text" name="siret" placeholder="SIRET ou SIREN" style="width:190px";
                    button type="submit" {(crate::langue::tr("Chercher l'entreprise")) }
                }
                form method="get" action="/tiers/nouveau" {
                    input type="text" name="adresse" placeholder="12 rue des Peupliers, Saint-Cyr" style="width:330px";
                    button type="submit" class="secondaire" {(crate::langue::tr("Chercher l'adresse")) }
                }
            }

            h2 {(crate::langue::tr("Fiche")) }
            form method="post" action="/tiers/nouveau" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" required value=(nom) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Adresse")) } td { input type="text" name="adresse" value=(adresse) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Code postal")) } td { input type="text" name="cp" value=(cp) style="width:120px"; } }
                        tr { th {(crate::langue::tr("Ville")) } td { input type="text" name="ville" value=(ville) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Email")) } td { input type="email" name="email" style="width:100%"; } }
                        tr { th {(crate::langue::tr("Telephone")) } td { input type="text" name="tel" style="width:100%"; } }
                        tr { th {(crate::langue::tr("SIRET")) } td { input type="text" name="siret" value=(siret) style="width:100%"; } }
                        tr { th {(crate::langue::tr("TVA intracommunautaire")) } td { input type="text" name="tva_intra" style="width:100%"; } }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Creer le tiers")) }
                    a .bouton.secondaire href="/tiers" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

async fn tiers_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |cle: &str| {
        f.get(cle)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let mut nouveau = db::party::NewParty::client(f.get("nom").map(|s| s.trim()).unwrap_or(""));
    nouveau.address = champ("adresse");
    nouveau.zip = champ("cp");
    nouveau.town = champ("ville");
    nouveau.email = champ("email");
    nouveau.phone = champ("tel");
    nouveau.siret = champ("siret");
    nouveau.tva_intra = champ("tva_intra");
    let id = db::party::create(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to(&format!("/tiers/{id}")))
}

/// Fiche d'un produit : consultation et edition des champs courants.
async fn produit_fiche(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = db::product::get(&app.pool, entite::courante(), id)?
        .ok_or_else(|| Erreur("produit introuvable".into()))?;
    let tva = db::dict::taux_tva(&app.pool, entite::courante(), 1)?;

    Ok(ui::page(
        "produits",
        "liste",
        &p.ref_,
        html! {
            (message(q.get("ok")))
            h1 { (p.label) " " }
            p .sous-titre { "Référence " code { (p.ref_) } " — la référence ne change jamais : elle sert de clé dans les documents déjà émis." }
            form method="post" action={ "/produits/" (id) "/modifier" } {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Désignation")) } td { input type="text" name="label" value=(p.label) required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Prix HT")) } td { input type="number" name="prix" step="0.01" min="0" value=(app::fmt_dec(p.price)) style="width:140px"; } }
                        tr {
                            th {(crate::langue::tr("TVA")) }
                            td {
                                select name="tva" {
                                    @for t in &tva {
                                        option value=(t.taux) selected[p.tva_tx == Some(t.taux)] { (app::fmt_qte(Some(t.taux))) " %" }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Description")) } td { input type="text" name="description" value=(p.description.clone().unwrap_or_default()) style="width:100%"; } }
                        tr {
                            th {(crate::langue::tr("En vente")) }
                            td {
                                select name="vente" {
                                    option value="1" selected[p.on_sell()] {(crate::langue::tr("Oui")) }
                                    option value="0" selected[!p.on_sell()] {(crate::langue::tr("Non (masqué)")) }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton.secondaire href="/produits" {(crate::langue::tr("Retour")) }
                }
            }
            p .sous-titre { "Prix TTC calculé : " strong { (app::fmt_eur(p.price_ttc)) } }
        },
    ))
}

async fn produit_enregistrer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let label = champ("label").ok_or_else(|| Erreur("désignation obligatoire".into()))?;
    let prix: Decimal = champ("prix")
        .and_then(|v| v.replace(',', ".").parse().ok())
        .unwrap_or_default();
    let tva: Decimal = champ("tva")
        .and_then(|v| v.replace(',', ".").parse().ok())
        .unwrap_or_default();
    let description = champ("description");
    let vente = f.get("vente").map(|v| v == "1").unwrap_or(true);
    db::product::update(
        &app.pool,
        entite::courante(),
        id,
        &label,
        prix,
        tva,
        description.as_deref(),
        vente,
    )?;
    Ok(Redirect::to(&format!("/produits/{id}?ok=enregistre")))
}

async fn produits_nouveau(State(app): State<App>) -> Res<Markup> {
    let unites = db::dict::unites(&app.pool)?;
    let tva = db::dict::taux_tva(&app.pool, entite::courante(), 1)?;
    Ok(ui::page(
        "produits",
        "nouveau",
        "Nouveau produit",
        html! {
            h1 {(crate::langue::tr("Nouveau produit")) }
            form method="post" action="/produits/nouveau" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Reference")) } td { input type="text" name="ref" required style="width:220px"; } }
                        tr { th {(crate::langue::tr("Libelle")) } td { input type="text" name="label" required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Prix HT")) } td { input type="number" name="prix" step="0.01" min="0" style="width:140px"; } }
                        tr {
                            th {(crate::langue::tr("Taux de TVA")) }
                            td {
                                select name="tva" {
                                    @for t in &tva {
                                        option value=(t.taux) { (app::fmt_dec(Some(t.taux))) " %" }
                                    }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Unite")) }
                            td {
                                select name="unite" {
                                    option value="" { "aucune" }
                                    @for u in &unites {
                                        option value=(u.code) { (u.libelle) " (" (u.code) ")" }
                                    }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Nature")) }
                            td {
                                select name="nature" {
                                    option value="produit" {(crate::langue::tr("Produit")) }
                                    option value="service" {(crate::langue::tr("Service")) }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Creer le produit")) }
                    a .bouton.secondaire href="/produits" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

async fn produits_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let reference = f.get("ref").map(|s| s.trim()).unwrap_or("");
    let label = f.get("label").map(|s| s.trim()).unwrap_or("");
    let mut nouveau = db::product::NewProduct::new(reference, label);
    nouveau.price = f.get("prix").and_then(|v| v.replace(',', ".").parse().ok());
    nouveau.tva_tx = f.get("tva").and_then(|v| v.replace(',', ".").parse().ok());
    nouveau.fk_unit = match f.get("unite").map(|s| s.as_str()) {
        Some(c) if !c.is_empty() => db::product::unit_id(&app.pool, c)?,
        _ => None,
    };
    if f.get("nature").map(|s| s == "service").unwrap_or(false) {
        nouveau.fk_product_type = db::product::TYPE_SERVICE;
    }
    db::product::create(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to("/produits"))
}

async fn devis_nouveau(State(app): State<App>) -> Res<Markup> {
    let clients = db::party::list(&app.pool, entite::courante(), 500, 0)?;
    let tva = db::dict::taux_tva(&app.pool, entite::courante(), 1)?;
    let conditions = db::reglement::conditions(&app.pool, entite::courante())?;
    Ok(ui::page(
        "devis",
        "nouveau",
        "Nouveau devis",
        html! {
            h1 {(crate::langue::tr("Nouveau devis")) }
            form method="post" action="/devis/nouveau" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" required style="width:100%" {
                                    @for c in &clients {
                                        option value=(c.rowid) { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Note interne")) } td { input type="text" name="note" style="width:100%"; } }
                        tr {
                            th {(crate::langue::tr("Règlement")) }
                            td {
                                select name="cond" style="width:100%" {
                                    @for c in &conditions {
                                        option value=(c.rowid) { (c.libelle_fr()) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Acompte (%)")) } td { input type="number" name="acompte" step="0.01" min="0" max="100" style="width:110px" placeholder="0"; } }
                        tr { th {(crate::langue::tr("Valable jusqu'au")) } td { input type="date" name="validite" style="width:170px"; } }
                    }
                }
                h2 {(crate::langue::tr("Lignes")) }
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Designation")) }
                            th .num {(crate::langue::tr("Qte")) }
                            th .num {(crate::langue::tr("PU HT")) }
                            th {(crate::langue::tr("TVA")) }
                        }
                    }
                    tbody {
                        @for i in 1..=4 {
                            tr {
                                td { input type="text" name=(format!("l{i}_lib")) style="width:100%"; }
                                td { input type="number" name=(format!("l{i}_qte")) step="0.01" min="0" value="1" style="width:90px"; }
                                td { input type="number" name=(format!("l{i}_pu")) step="0.01" min="0" style="width:120px"; }
                                td {
                                    select name=(format!("l{i}_tva")) {
                                        @for t in &tva {
                                            option value=(t.taux) { (app::fmt_dec(Some(t.taux))) " %" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Creer le devis")) }
                    a .bouton.secondaire href="/devis" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

async fn devis_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let fk_soc = f
        .get("soc")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("client invalide".into()))?;

    let mut lignes = Vec::new();
    for i in 1..=4 {
        let lib = f.get(&format!("l{i}_lib")).map(|s| s.trim()).unwrap_or("");
        if lib.is_empty() {
            continue;
        }
        let qte: Decimal = f
            .get(&format!("l{i}_qte"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let pu: Decimal = f
            .get(&format!("l{i}_pu"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let tva: Decimal = f
            .get(&format!("l{i}_tva"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        lignes.push(db::proposal::NewProposalLine::new(lib, qte, pu, tva));
    }
    if lignes.is_empty() {
        return Err(Erreur("au moins une ligne est necessaire".into()));
    }

    let mut nouveau = db::proposal::NewProposal::new(fk_soc);
    nouveau.note_private = f.get("note").filter(|s| !s.trim().is_empty()).cloned();
    nouveau.fk_cond_reglement = f.get("cond").and_then(|v| v.parse().ok());
    nouveau.fin_validite = f
        .get("validite")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    nouveau.deposit_percent = f
        .get("acompte")
        .filter(|s| !s.trim().is_empty())
        .and_then(|v| v.replace(',', ".").parse().ok());
    nouveau.lines = lignes;
    let id = db::proposal::create(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to(&format!("/devis/{id}")))
}
// --- edition -----------------------------------------------------------------

async fn tiers_modifier(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let mut t = db::party::get(&app.pool, entite::courante(), id)?
        .ok_or(Erreur("Tiers introuvable".into()))?;
    let mut retour: Option<String> = None;

    // Même recette que la création : deux GET qui pré-remplissent la fiche.
    if let Some(s) = q.get("siret").filter(|s| !s.trim().is_empty()) {
        match annuaire::par_siret(s) {
            Ok(Some(e)) => {
                t.nom = e.nom;
                t.address = Some(e.adresse).filter(|a| !a.is_empty());
                t.zip = Some(e.code_postal).filter(|a| !a.is_empty());
                t.town = Some(e.commune).filter(|a| !a.is_empty());
                t.siret = Some(e.siret);
                retour = Some("Fiche complétée depuis l'INSEE.".to_string());
            }
            Ok(None) => retour = Some("Aucune entreprise trouvée.".into()),
            Err(e) => retour = Some(format!("Annuaire indisponible : {e}")),
        }
    }
    if let Some(a) = q.get("adresse").filter(|s| !s.trim().is_empty()) {
        match annuaire::adresses(a, 1) {
            Ok(liste) if !liste.is_empty() => {
                let adr = &liste[0];
                t.address = Some(adr.numero_voie.clone());
                t.zip = Some(adr.code_postal.clone());
                t.town = Some(adr.commune.clone());
                retour = Some(format!("Adresse retenue : {}", adr.libelle));
            }
            Ok(_) => retour = Some("Aucune adresse trouvée.".into()),
            Err(e) => retour = Some(format!("Base Adresse indisponible : {e}")),
        }
    }

    Ok(ui::page(
        "tiers",
        "liste",
        &format!("Modifier {}", t.nom),
        html! {
            h1 { "Modifier " (t.nom) }
            p .sous-titre { "Code client " (t.code_client.clone().unwrap_or_default()) }
            @if let Some(m) = &retour {
                p .flash { (m) }
            }

            h2 {(crate::langue::tr("Remplir automatiquement")) }
            div .actions {
                form method="get" action={ "/tiers/" (id) "/modifier" } {
                    input type="text" name="siret" placeholder="SIRET ou SIREN" style="width:190px";
                    button type="submit" {(crate::langue::tr("Chercher l'entreprise")) }
                }
                form method="get" action={ "/tiers/" (id) "/modifier" } {
                    input type="text" name="adresse" placeholder="12 rue des Peupliers, Saint-Cyr" style="width:330px";
                    button type="submit" class="secondaire" {(crate::langue::tr("Chercher l'adresse")) }
                }
            }

            form method="post" action={ "/tiers/" (id) "/modifier" } {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" required value=(t.nom) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Adresse")) } td { input type="text" name="adresse" value=(t.address.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Code postal")) } td { input type="text" name="cp" value=(t.zip.clone().unwrap_or_default()) style="width:120px"; } }
                        tr { th {(crate::langue::tr("Ville")) } td { input type="text" name="ville" value=(t.town.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Email")) } td { input type="email" name="email" value=(t.email.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Telephone")) } td { input type="text" name="tel" value=(t.phone.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("SIRET")) } td { input type="text" name="siret" value=(t.siret.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("TVA intracommunautaire")) } td { input type="text" name="tva_intra" value=(t.tva_intra.clone().unwrap_or_default()) style="width:100%"; } }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton.secondaire href={ "/tiers/" (id) } {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

async fn tiers_enregistrer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |cle: &str| {
        f.get(cle)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let mut modif = db::party::NewParty::client(f.get("nom").map(|s| s.trim()).unwrap_or(""));
    modif.address = champ("adresse");
    modif.zip = champ("cp");
    modif.town = champ("ville");
    modif.email = champ("email");
    modif.phone = champ("tel");
    modif.siret = champ("siret");
    modif.tva_intra = champ("tva_intra");
    db::party::update(&app.pool, entite::courante(), id, &modif)?;
    Ok(Redirect::to(&format!("/tiers/{id}")))
}

const NB_LIGNES_DEVIS: usize = 10;

async fn devis_modifier(State(app): State<App>, Path(id): Path<i64>) -> Res<Markup> {
    let p = &app.pool;
    let d =
        db::proposal::get(p, entite::courante(), id)?.ok_or(Erreur("Devis introuvable".into()))?;
    if d.fk_statut != 0 {
        return Err(Erreur(
            "Seul un devis en brouillon peut etre modifie.".into(),
        ));
    }
    let lignes = db::proposal::lines(p, entite::courante(), id)?;
    let clients = db::party::list(p, entite::courante(), 500, 0)?;
    let tva = db::dict::taux_tva(p, entite::courante(), 1)?;
    let conditions = db::reglement::conditions(p, entite::courante())?;

    // Lignes pre-remplies : (section, libelle, qte, pu, tva).
    let mut prefill: Vec<(String, String, String, String, String)> = Vec::new();
    let mut section_courante = String::new();
    for l in &lignes {
        if l.is_section() {
            section_courante = l.label.clone().unwrap_or_default();
            prefill.push((
                section_courante.clone(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ));
        } else {
            prefill.push((
                section_courante.clone(),
                l.label.clone().unwrap_or_default(),
                app::fmt_dec(l.qty),
                app::fmt_dec(l.subprice),
                app::fmt_dec(l.tva_tx),
            ));
        }
    }
    while prefill.len() < NB_LIGNES_DEVIS {
        prefill.push((
            String::new(),
            String::new(),
            "1".into(),
            String::new(),
            String::new(),
        ));
    }

    Ok(ui::page(
        "devis",
        "liste",
        &format!("Modifier {}", d.ref_),
        html! {
            h1 { "Modifier le devis " (d.ref_) }
            p .sous-titre {(crate::langue::tr("Les lignes sont remplacees en bloc. Une ligne sans libelle mais avec un nom de section ouvre une section."))
            }
            form method="post" action={ "/devis/" (id) "/modifier" } {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" required style="width:100%" {
                                    @for c in &clients {
                                        option value=(c.rowid) selected[d.fk_soc == Some(c.rowid)] { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Note interne")) } td { input type="text" name="note" value=(d.note_private.clone().unwrap_or_default()) style="width:100%"; } }
                        tr {
                            th {(crate::langue::tr("Règlement")) }
                            td {
                                select name="cond" style="width:100%" {
                                    @for c in &conditions {
                                        option value=(c.rowid) selected[d.fk_cond_reglement == Some(c.rowid)] { (c.libelle_fr()) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Acompte (%)")) } td { input type="number" name="acompte" step="0.01" min="0" max="100" value=(app::fmt_dec(d.deposit_percent)) style="width:110px"; } }
                        tr { th {(crate::langue::tr("Valable jusqu'au")) } td { input type="date" name="validite" value=(d.fin_validite.clone().unwrap_or_default()) style="width:170px"; } }
                    }
                }
                h2 {(crate::langue::tr("Lignes")) }
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Section")) }
                            th {(crate::langue::tr("Designation")) }
                            th .num {(crate::langue::tr("Qte")) }
                            th .num {(crate::langue::tr("PU HT")) }
                            th {(crate::langue::tr("TVA")) }
                        }
                    }
                    tbody {
                        @for (i, (sec, lib, qte, pu, tva_l)) in prefill.iter().enumerate() {
                            tr {
                                td { input type="text" name=(format!("r{}_sec", i + 1)) value=(sec) style="width:130px"; }
                                td { input type="text" name=(format!("r{}_lib", i + 1)) value=(lib) style="width:100%"; }
                                td { input type="number" name=(format!("r{}_qte", i + 1)) step="0.01" min="0" value=(qte) style="width:80px"; }
                                td { input type="number" name=(format!("r{}_pu", i + 1)) step="0.01" min="0" value=(pu) style="width:110px"; }
                                td {
                                    select name=(format!("r{}_tva", i + 1)) {
                                        @for t in &tva {
                                            option value=(t.taux) selected[tva_l == &app::fmt_dec(Some(t.taux))] {
                                                (app::fmt_dec(Some(t.taux))) " %"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton.secondaire href={ "/devis/" (id) } {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

async fn devis_enregistrer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let fk_soc = f
        .get("soc")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("client invalide".into()))?;

    let mut lignes = Vec::new();
    for i in 1..=NB_LIGNES_DEVIS {
        let lib = f.get(&format!("r{i}_lib")).map(|s| s.trim()).unwrap_or("");
        if lib.is_empty() {
            continue;
        }
        let section = f
            .get(&format!("r{i}_sec"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let qte: Decimal = f
            .get(&format!("r{i}_qte"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let pu: Decimal = f
            .get(&format!("r{i}_pu"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let tva: Decimal = f
            .get(&format!("r{i}_tva"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let mut ligne = db::proposal::NewProposalLine::new(lib, qte, pu, tva);
        ligne.section = section;
        lignes.push(ligne);
    }
    if lignes.is_empty() {
        return Err(Erreur("au moins une ligne est necessaire".into()));
    }

    let mut modif = db::proposal::NewProposal::new(fk_soc);
    modif.note_private = f.get("note").filter(|s| !s.trim().is_empty()).cloned();
    modif.fk_cond_reglement = f.get("cond").and_then(|v| v.parse().ok());
    modif.fin_validite = f
        .get("validite")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    modif.deposit_percent = f
        .get("acompte")
        .filter(|s| !s.trim().is_empty())
        .and_then(|v| v.replace(',', ".").parse().ok());
    modif.lines = lignes;
    db::proposal::update(&app.pool, entite::courante(), id, &modif)?;
    Ok(Redirect::to(&format!("/devis/{id}?ok=enregistre")))
}

async fn devis_valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    db::proposal::validate(&app.pool, entite::courante(), id)?;
    if let Some(d) = db::proposal::get(&app.pool, entite::courante(), id)? {
        tracer(
            &app,
            d.fk_soc,
            elementtype_devis(),
            id,
            &format!("Devis {} validé", d.ref_),
        );
        let _ = app::archiver_devis(&app.pool, entite::courante(), id);
    }
    Ok(Redirect::to(&format!("/devis/{id}?ok=valide")))
}

/// Cree une serie de factures recurrentes depuis une facture validee.
async fn factures_recurrence(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let entity = entite::courante();
    let champ = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let prochaine = champ("prochaine");
    let frequence: i32 = champ("frequence").parse().unwrap_or(1);
    let maximum: i32 = champ("maximum").parse().unwrap_or(0);
    // La periodicite vient d'une liste fermee : on choisit l'unite ici, on ne la
    // compose jamais a partir du texte saisi.
    let (freq, unite) = match champ("periode").as_str() {
        "semaine" => (1, "w"),
        "mois" => (frequence.max(1), "m"),
        "trimestre" => (3, "m"),
        "an" => (1, "y"),
        autre => return Err(Erreur(format!("periodicite inconnue : « {autre} »"))),
    };

    let id_serie = db::recurrence::depuis_facture(
        &app.pool,
        entity,
        id,
        "",
        &prochaine,
        (freq, unite),
        maximum,
    )?;

    tracer(
        &app,
        None,
        "facture_rec",
        id_serie,
        &format!("Série de factures créée depuis la facture {id}"),
    );

    Ok(Redirect::to(&format!("/factures/{id}?ok=serie")))
}

fn elementtype_devis() -> &'static str {
    "propal"
}

// --- facture directe et relances --------------------------------------------

async fn factures_nouveau(State(app): State<App>) -> Res<Markup> {
    let clients = db::party::list(&app.pool, entite::courante(), 500, 0)?;
    let tva = db::dict::taux_tva(&app.pool, entite::courante(), 1)?;
    let conditions = db::reglement::conditions(&app.pool, entite::courante())?;
    Ok(ui::page(
        "factures",
        "nouveau",
        "Nouvelle facture",
        html! {
            h1 {(crate::langue::tr("Nouvelle facture")) }
            p .sous-titre {(crate::langue::tr("Sans devis préalable. La facture est créée en brouillon.")) }
            form method="post" action="/factures/nouveau" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" required style="width:100%" {
                                    @for c in &clients {
                                        option value=(c.rowid) { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Bon de commande")) } td { input type="text" name="ref_client" style="width:100%"; } }
                        tr {
                            th {(crate::langue::tr("Règlement")) }
                            td {
                                select name="cond" style="width:100%" {
                                    @for c in &conditions {
                                        option value=(c.rowid) { (c.libelle_fr()) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Date de la prestation")) } td { input type="date" name="date_prestation" style="width:170px"; } }
                        tr { th {(crate::langue::tr("Note interne")) } td { input type="text" name="note" style="width:100%"; } }
                    } }
                h2 {(crate::langue::tr("Lignes")) }
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Designation")) }
                            th .num {(crate::langue::tr("Qte")) }
                            th .num {(crate::langue::tr("PU HT")) }
                            th {(crate::langue::tr("TVA")) }
                        }
                    }
                    tbody {
                        @for i in 1..=6 {
                            tr {
                                td { input type="text" name=(format!("l{i}_lib")) style="width:100%"; }
                                td { input type="number" name=(format!("l{i}_qte")) step="0.01" min="0" value="1" style="width:90px"; }
                                td { input type="number" name=(format!("l{i}_pu")) step="0.01" min="0" style="width:120px"; }
                                td {
                                    select name=(format!("l{i}_tva")) {
                                        @for t in &tva {
                                            option value=(t.taux) { (app::fmt_dec(Some(t.taux))) " %" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Créer la facture")) }
                    a .bouton.secondaire href="/factures" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

async fn factures_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let fk_soc = f
        .get("soc")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("client invalide".into()))?;

    let mut lignes = Vec::new();
    for i in 1..=6 {
        let lib = f.get(&format!("l{i}_lib")).map(|s| s.trim()).unwrap_or("");
        if lib.is_empty() {
            continue;
        }
        let qte: Decimal = f
            .get(&format!("l{i}_qte"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let pu: Decimal = f
            .get(&format!("l{i}_pu"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let tva: Decimal = f
            .get(&format!("l{i}_tva"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        lignes.push(db::invoice::NewInvoiceLine::new(lib, qte, pu, tva));
    }
    if lignes.is_empty() {
        return Err(Erreur("au moins une ligne est nécessaire".into()));
    }

    let mut nouvelle = db::invoice::NewInvoice::new(fk_soc);
    nouvelle.note_private = f.get("note").filter(|s| !s.trim().is_empty()).cloned();
    nouvelle.fk_cond_reglement = f.get("cond").and_then(|v| v.parse().ok());
    nouvelle.ref_client = f
        .get("ref_client")
        .filter(|s| !s.trim().is_empty())
        .cloned();
    nouvelle.date_prestation = f
        .get("date_prestation")
        .filter(|s| !s.trim().is_empty())
        .cloned();
    nouvelle.lines = lignes;
    let id = db::invoice::create(&app.pool, entite::courante(), &nouvelle)?;
    Ok(Redirect::to(&format!("/factures/{id}")))
}

async fn relances(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let listes = db::relance::en_retard(p, entite::courante())?;
    let montant = db::relance::total(&listes);

    Ok(ui::page(
        "relances",
        "liste",
        "Relances",
        html! {
            h1 {(crate::langue::tr("Relances")) }
            p .sous-titre {
                (listes.len()) " facture(s) échue(s) — " (ui::euros(montant)) " à relancer"
            }
            @if listes.is_empty() {
                (ui::vide("Aucune facture en retard. Rien à relancer.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Facture")) }
                            th {(crate::langue::tr("Client")) }
                            th {(crate::langue::tr("Échéance")) }
                            th .num {(crate::langue::tr("Retard")) }
                            th .num {(crate::langue::tr("Reste dû")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for r in &listes {
                            tr {
                                td .ref { a href={ "/factures/" (r.fk_facture) } { (r.reference) } }
                                td { (r.client) }
                                td { (r.echeance.clone().unwrap_or_default()) }
                                td .num { (r.jours_retard) " j" }
                                td .num { (ui::euros(r.reste)) }
                                td {
                                    @if let Some(mail) = &r.email {
                                        a .bouton.petit.secondaire
                                          href={ "mailto:" (mail) "?subject=Relance facture " (r.reference)
                                                 "&body=" (urlencode(&db::relance::texte(r))) }
                                          {(crate::langue::tr("Préparer le mail")) }
                                    } @else {
                                        span .vide {(crate::langue::tr("pas d'e-mail")) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for o in s.bytes() {
        match o {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(o as char)
            }
            _ => out.push_str(&format!("%{o:02X}")),
        }
    }
    out
}

// --- utilisateurs -----------------------------------------------------------

async fn utilisateurs(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let listes = db::user::list(&app.pool, entite::courante())?;
    Ok(ui::page(
        "parametres",
        "utilisateurs",
        "Utilisateurs",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Utilisateurs")) }
            p .sous-titre {(crate::langue::tr("Comptes de la base Dolibarr : les mots de passe créés ici fonctionnent aussi dans Dolibarr.")) }

            @if listes.is_empty() {
                (ui::vide("Aucun compte.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Identifiant")) }
                            th {(crate::langue::tr("Nom")) }
                            th {(crate::langue::tr("E-mail")) }
                            th {(crate::langue::tr("Rôle")) }
                            th { "État" }
                            th { "" }
                        }
                    }
                    tbody {
                        @for u in &listes {
                            tr {
                                td .ref { (u.login) }
                                td { (u.nom_complet()) }
                                td { (u.email.clone().unwrap_or_default()) }
                                td { (if u.admin { "administrateur" } else { "utilisateur" }) }
                                td {
                                    @if u.actif() {
                                        span .pastille.p-payee { "actif" }
                                    } @else {
                                        span .pastille.p-brouillon {(crate::langue::tr("désactivé")) }
                                    }
                                }
                                td {
                                    @if !u.admin {
                                        a .bouton.petit.secondaire href={ "/parametres/utilisateurs/" (u.rowid) "/droits" } {(crate::langue::tr("Droits")) }
                                    }
                                    form method="post" action={ "/parametres/utilisateurs/" (u.rowid) "/basculer" } {
                                        button type="submit" class="petit secondaire" {
                                            (if u.actif() { "Désactiver" } else { "Activer" })
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouveau compte")) }
            form method="post" action="/parametres/utilisateurs/nouveau" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Identifiant")) } td { input type="text" name="login" required style="width:220px"; } }
                        tr { th {(crate::langue::tr("Mot de passe")) } td { input type="text" name="mdp" required minlength="8" style="width:220px"; } }
                        tr { th {(crate::langue::tr("Prénom")) } td { input type="text" name="prenom" style="width:100%"; } }
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" style="width:100%"; } }
                        tr { th {(crate::langue::tr("E-mail")) } td { input type="email" name="email" style="width:100%"; } }
                        tr {
                            th {(crate::langue::tr("Rôle")) }
                            td {
                                select name="admin" {
                                    option value="0" {(crate::langue::tr("Utilisateur")) }
                                    option value="1" {(crate::langue::tr("Administrateur")) }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le compte")) } }
            }
        },
    ))
}

async fn utilisateurs_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let nouveau = db::user::NouvelUtilisateur {
        login: f.get("login").map(|s| s.trim()).unwrap_or("").to_string(),
        mot_de_passe: f.get("mdp").cloned().unwrap_or_default(),
        nom: f.get("nom").cloned().unwrap_or_default(),
        prenom: f.get("prenom").cloned().unwrap_or_default(),
        email: f.get("email").filter(|s| !s.trim().is_empty()).cloned(),
        admin: f.get("admin").map(|v| v == "1").unwrap_or(false),
    };
    db::user::creer(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to("/parametres/utilisateurs?ok=compte"))
}

async fn utilisateurs_basculer(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    if let Some(u) = db::user::get(&app.pool, id)? {
        db::user::activer(&app.pool, id, !u.actif())?;
    }
    Ok(Redirect::to("/parametres/utilisateurs?ok=compte"))
}

// --- import / export --------------------------------------------------------

async fn import_page(Query(q): Query<HashMap<String, String>>) -> Res<Markup> {
    Ok(ui::page(
        "parametres",
        "import",
        "Import / Export",
        html! {
            @if let Some(n) = q.get("crees") {
                p .flash { "Import terminé : " (n) " tiers créés." }
            }
            h1 {(crate::langue::tr("Import / Export")) }
            p .sous-titre {(crate::langue::tr("CSV séparé par « ; » ou par « , », avec une ligne d'entête. Les colonnes sont reconnues par leur nom.")) }

            h2 {(crate::langue::tr("Importer des tiers")) }
            p { "Colonnes reconnues : " code { "nom" } " (obligatoire), " code { "adresse" } ", " code { "cp" } ", " code { "ville" } ", " code { "email" } ", " code { "tel" } ", " code { "siret" } ", " code { "tva" } "." }
            form method="post" action="/parametres/import" {
                textarea name="csv" rows="10" style="width:100%;font-family:ui-monospace,monospace;padding:10px;border:1px solid #e2e2e2" placeholder="nom;ville;email&#10;SARL Exemple;Charly;contact@exemple.fr"
                { "" }
                div .actions {
                    button type="submit" {(crate::langue::tr("Importer")) }
                }
            }

            h2 {(crate::langue::tr("Importer des produits")) }
            p { "Colonnes reconnues : " code { "ref" } " (obligatoire), " code { "libelle" } ", " code { "prix" } ", " code { "tva" } ", " code { "type" } " (produit ou service), " code { "unite" } "." }
            form method="post" action="/parametres/import/produits" {
                textarea name="csv" rows="6" style="width:100%;font-family:ui-monospace,monospace;padding:10px;border:1px solid #e2e2e2" placeholder="ref;libelle;prix;tva;type&#10;BOR-030;Bordure granit 30 cm;125.50;20;produit" { "" }
                div .actions { button type="submit" {(crate::langue::tr("Importer")) } }
            }

            h2 {(crate::langue::tr("Importer des factures")) }
            p { "Une ligne = une ligne de facture, regroupée par client et référence. Colonnes : " code { "client" } " (obligatoire), " code { "facture" } ", " code { "date" } ", " code { "echeance" } ", " code { "libelle" } ", " code { "qte" } ", " code { "pu" } ", " code { "tva" } "." }
            p .vide {(crate::langue::tr("Les factures importées sont créées puis validées. Le client est créé s'il n'existe pas. La numérotation Dolibarr est attribuée.")) }
            form method="post" action="/parametres/import/factures" {
                textarea name="csv" rows="6" style="width:100%;font-family:ui-monospace,monospace;padding:10px;border:1px solid #e2e2e2" placeholder="client;facture;date;libelle;qte;pu;tva&#10;SARL Exemple;F2024-001;2024-03-12;Travaux;1;1200;20" { "" }
                div .actions { button type="submit" {(crate::langue::tr("Importer")) } }
            }

            h2 {(crate::langue::tr("Exporter")) }
            table {
                tbody {
                    tr { td { a .bouton.secondaire href="/export/tiers.csv" {(crate::langue::tr("Tiers (CSV)")) } } td {(crate::langue::tr("code client, nom, adresse, CP, ville, e-mail, téléphone, SIRET, TVA")) } }
                    tr { td { a .bouton.secondaire href="/export/produits.csv" {(crate::langue::tr("Produits (CSV)")) } } td {(crate::langue::tr("référence, libellé, prix HT, taux de TVA, type")) } }
                }
            }
        },
    ))
}

async fn import_tiers(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Markup> {
    let csv = f.get("csv").cloned().unwrap_or_default();
    if csv.trim().is_empty() {
        return Err(Erreur("aucun contenu à importer".into()));
    }
    let rapport = db::impex::importer_tiers(&app.pool, entite::courante(), &csv)?;
    Ok(ui::page(
        "parametres",
        "import",
        "Import",
        html! {
            h1 {(crate::langue::tr("Résultat de l'import")) }
            div .cartes {
                (ui::carte("Créés", &rapport.crees.to_string(), None, false))
                (ui::carte("Ignorés", &rapport.ignores.to_string(), Some("déjà présents ou sans nom"), false))
                (ui::carte("Erreurs", &rapport.erreurs.len().to_string(), None, !rapport.erreurs.is_empty()))
            }
            @if !rapport.erreurs.is_empty() {
                h2 {(crate::langue::tr("Erreurs")) }
                table {
                    tbody {
                        @for e in &rapport.erreurs { tr { td { (e) } } }
                    }
                }
            }
            div .actions {
                a .bouton href="/tiers" {(crate::langue::tr("Voir les tiers")) }
                a .bouton.secondaire href="/parametres/import" {(crate::langue::tr("Importer un autre fichier")) }
            }
        },
    ))
}

async fn import_produits(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Markup> {
    let csv = f.get("csv").cloned().unwrap_or_default();
    if csv.trim().is_empty() {
        return Err(Erreur("aucun contenu à importer".into()));
    }
    let rapport = db::impex::importer_produits(&app.pool, entite::courante(), &csv)?;
    Ok(ui::page(
        "parametres",
        "import",
        "Import produits",
        html! {
            h1 {(crate::langue::tr("Résultat de l'import produits")) }
            div .cartes {
                (ui::carte("Créés", &rapport.crees.to_string(), None, false))
                (ui::carte("Ignorés", &rapport.ignores.to_string(), Some("déjà présents ou sans référence"), false))
                (ui::carte("Erreurs", &rapport.erreurs.len().to_string(), None, !rapport.erreurs.is_empty()))
            }
            @if !rapport.erreurs.is_empty() {
                h2 {(crate::langue::tr("Erreurs")) }
                table { tbody { @for e in &rapport.erreurs { tr { td { (e) } } } } }
            }
            div .actions {
                a .bouton href="/produits" {(crate::langue::tr("Voir les produits")) }
                a .bouton.secondaire href="/parametres/import" {(crate::langue::tr("Retour")) }
            }
        },
    ))
}

async fn import_factures(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Markup> {
    let csv = f.get("csv").cloned().unwrap_or_default();
    if csv.trim().is_empty() {
        return Err(Erreur("aucun contenu à importer".into()));
    }
    let rapport = db::impex::importer_factures(&app.pool, entite::courante(), &csv)?;
    Ok(ui::page(
        "parametres",
        "import",
        "Import factures",
        html! {
            h1 {(crate::langue::tr("Résultat de l'import factures")) }
            div .cartes {
                (ui::carte("Créées", &rapport.crees.to_string(), Some("validées automatiquement"), false))
                (ui::carte("Ignorées", &rapport.ignores.to_string(), None, false))
                (ui::carte("Erreurs", &rapport.erreurs.len().to_string(), None, !rapport.erreurs.is_empty()))
            }
            @if !rapport.erreurs.is_empty() {
                h2 {(crate::langue::tr("Erreurs")) }
                table { tbody { @for e in &rapport.erreurs { tr { td { (e) } } } } }
            }
            div .actions {
                a .bouton href="/factures" {(crate::langue::tr("Voir les factures")) }
                a .bouton.secondaire href="/parametres/import" {(crate::langue::tr("Retour")) }
            }
        },
    ))
}

/// Nombre en ecriture comptable francaise : `1234,56` (virgule, pas de
/// separateur de milliers : Excel les interprete mal en CSV).
fn csv_nombre(v: rust_decimal::Decimal) -> String {
    format!("{:.2}", v).replace('.', ",")
}

fn csv_date(iso: &str) -> String {
    app::fmt_date_fr(Some(iso))
}

/// Echappe un champ pour un CSV a point-virgule.
fn csv_champ(v: &str) -> String {
    let propre = v.replace(';', ",").replace(['\r', '\n'], " ");
    format!("\"{}\"", propre.replace('"', "\"\""))
}

/// Journal des ventes au format attendu par un cabinet comptable :
/// point-virgule, virgule decimale, dates jj/mm/aaaa, BOM UTF-8.
async fn grand_livre(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let (debut, fin) = (
        q.get("debut").cloned().unwrap_or_default(),
        q.get("fin").cloned().unwrap_or_default(),
    );
    let ecritures = db::grand_livre::ecritures(
        &app.pool,
        entite::courante(),
        Some(debut.as_str()).filter(|d| !d.is_empty()),
        Some(fin.as_str()).filter(|d| !d.is_empty()),
    )?;
    let balance = db::grand_livre::balance(
        &app.pool,
        entite::courante(),
        Some(debut.as_str()).filter(|d| !d.is_empty()),
        Some(fin.as_str()).filter(|f| !f.is_empty()),
    )?;
    let total_debit: Decimal = balance.iter().map(|b| b.debit).sum();
    let total_credit: Decimal = balance.iter().map(|b| b.credit).sum();

    Ok(ui::page(
        "comptabilite",
        "journaux",
        "Grand livre",
        html! {
            h1 {(crate::langue::tr("Grand livre")) }
            p .sous-titre {
                "Écritures dérivées des documents (plan comptable général simplifié). "
                "Ce n'est pas une comptabilité tenue : c'est une proposition à valider par l'expert-comptable."
            }
            form method="get" action="/comptabilite/grand-livre" .actions {
                input type="date" name="debut" value=(debut) aria-label="Début";
                input type="date" name="fin" value=(fin) aria-label="Fin";
                button type="submit" {(crate::langue::tr("Filtrer")) }
                a .bouton.secondaire href={ "/export/grand-livre.csv?debut=" (debut) "&fin=" (fin) } {(crate::langue::tr("Exporter le grand livre (CSV)")) }
            }

            h2 {(crate::langue::tr("Balance")) }
            @if balance.is_empty() {
                (ui::vide("Aucune écriture sur la période.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Compte")) } th {(crate::langue::tr("Libellé")) } th .num {(crate::langue::tr("Débit")) } th .num {(crate::langue::tr("Crédit")) } th .num {(crate::langue::tr("Solde")) } } }
                    tbody {
                        @for b in &balance {
                            tr {
                                td .ref { (b.compte) }
                                td { (b.libelle) }
                                td .num { (ui::euros(b.debit)) }
                                td .num { (ui::euros(b.credit)) }
                                td .num { (ui::euros(b.solde())) }
                            }
                        }
                    }
                    tfoot {
                        tr {
                            td colspan="2" .num {(crate::langue::tr("Totaux")) }
                            td .num { (ui::euros(total_debit)) }
                            td .num { (ui::euros(total_credit)) }
                            td .num { (ui::euros(total_debit - total_credit)) }
                        }
                    }
                }
            }

            h2 { "Écritures" }
            @if ecritures.is_empty() {
                (ui::vide("Aucune écriture.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Journal")) } th {(crate::langue::tr("Compte")) } th {(crate::langue::tr("Libellé")) } th .num {(crate::langue::tr("Débit")) } th .num {(crate::langue::tr("Crédit")) } } }
                    tbody {
                        @for e in &ecritures {
                            tr {
                                td .ref { (app::fmt_date_fr(Some(&e.date))) }
                                td .ref { (e.journal) }
                                td .ref { (e.compte) " " (e.libelle_compte) }
                                td { (e.libelle) }
                                td .num { (ui::euros(e.debit)) }
                                td .num { (ui::euros(e.credit)) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

async fn fec_page(State(app): State<App>, Query(q): Query<HashMap<String, String>>) -> Res<Markup> {
    let (debut, fin) = (
        q.get("debut").cloned().unwrap_or_default(),
        q.get("fin").cloned().unwrap_or_default(),
    );
    let nombre = db::fec::compter(
        &app.pool,
        entite::courante(),
        Some(debut.as_str()).filter(|d| !d.is_empty()),
        Some(fin.as_str()).filter(|d| !d.is_empty()),
    )?;

    Ok(ui::page(
        "comptabilite",
        "journaux",
        "Comptabilisation et FEC",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Comptabilisation et FEC")) }
            p .sous-titre {
                "Les écritures sont générées depuis les documents valides et écrites dans "
                code { "llx_accounting_bookkeeping" } ". L'export suit le format FEC (art. A47 A-1 du LPF)."
            }

            div .actions {
                form method="post" action="/comptabilite/fec" {
                    input type="date" name="debut" value=(debut) aria-label="Début";
                    input type="date" name="fin" value=(fin) aria-label="Fin";
                    button type="submit" {(crate::langue::tr("Générer les écritures")) }
                }
                @if nombre > 0 {
                    a .bouton.secondaire href={ "/export/fec.txt?debut=" (debut) "&fin=" (fin) } {(crate::langue::tr("Exporter le FEC")) }
                }
            }

            table {
                tbody {
                    tr { th {(crate::langue::tr("Écritures en base")) } td .num { (nombre) } }
                    tr { th {(crate::langue::tr("Journaux")) } td {(crate::langue::tr("VE (ventes) · AC (achats) · BQ (banque)")) } }
                }
            }

            p style="color:#78736c;font-size:13px" {
                "Générer remplace les écritures VE/AC/BQ de la période. Une écriture saisie "
                "dans un autre journal n'est jamais touchée."
            }
        },
    ))
}

async fn fec_generer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let debut = f
        .get("debut")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let fin = f
        .get("fin")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    db::fec::generer(
        &app.pool,
        entite::courante(),
        debut.as_deref(),
        fin.as_deref(),
    )?;
    Ok(Redirect::to(&format!(
        "/comptabilite/fec?ok=genere&debut={}&fin={}",
        debut.unwrap_or_default(),
        fin.unwrap_or_default()
    )))
}

async fn export_fec(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Response> {
    let debut = q
        .get("debut")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "1970-01-01".to_string());
    let fin = q
        .get("fin")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "2099-12-31".to_string());
    let contenu = db::fec::export(&app.pool, entite::courante(), &debut, &fin)?;
    Ok(csv_response(contenu, "FEC.txt"))
}

async fn export_grand_livre(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Response> {
    let ecritures = db::grand_livre::ecritures(
        &app.pool,
        entite::courante(),
        q.get("debut").map(String::as_str),
        q.get("fin").map(String::as_str),
    )?;
    let mut csv =
        String::from("\u{feff}Date;Journal;Compte;Libelle compte;Libelle;Reference;Debit;Credit\n");
    for e in &ecritures {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{}\n",
            csv_date(&e.date),
            e.journal,
            e.compte,
            csv_champ(e.libelle_compte),
            csv_champ(&e.libelle),
            csv_champ(&e.reference),
            csv_nombre(e.debit),
            csv_nombre(e.credit),
        ));
    }
    Ok(csv_response(csv, "grand-livre.csv"))
}

async fn export_ventes(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Response> {
    let lignes = db::compta::journal_ventes(
        &app.pool,
        entite::courante(),
        q.get("debut").map(String::as_str),
        q.get("fin").map(String::as_str),
    )?;
    let mut csv = String::from("\u{feff}Date;Reference;Type;Client;Total HT;TVA;Total TTC\n");
    for l in &lignes {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{}\n",
            csv_date(&l.date),
            csv_champ(&l.reference),
            csv_champ(&l.type_document),
            csv_champ(&l.tiers),
            csv_nombre(l.total_ht),
            csv_nombre(l.total_tva),
            csv_nombre(l.total_ttc),
        ));
    }
    Ok(csv_response(csv, "journal-ventes.csv"))
}

async fn export_achats(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Response> {
    let lignes = db::compta::journal_achats(
        &app.pool,
        entite::courante(),
        q.get("debut").map(String::as_str),
        q.get("fin").map(String::as_str),
    )?;
    let mut csv = String::from(
        "\u{feff}Date;Reference;Reference fournisseur;Fournisseur;Total HT;TVA;Total TTC\n",
    );
    for l in &lignes {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{}\n",
            csv_date(&l.date),
            csv_champ(&l.reference),
            csv_champ(&l.complement),
            csv_champ(&l.tiers),
            csv_nombre(l.total_ht),
            csv_nombre(l.total_tva),
            csv_nombre(l.total_ttc),
        ));
    }
    Ok(csv_response(csv, "journal-achats.csv"))
}

async fn export_tva(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Response> {
    let lignes = db::compta::tva_par_taux(
        &app.pool,
        entite::courante(),
        q.get("debut").map(String::as_str),
        q.get("fin").map(String::as_str),
    )?;
    let mut csv = String::from("\u{feff}Mois;Sens;Taux de TVA;Base HT;TVA\n");
    for (mois, sens, taux, ht, tva) in &lignes {
        csv.push_str(&format!(
            "{};{};{};{};{}\n",
            csv_champ(mois),
            csv_champ(sens),
            csv_champ(taux.trim_end_matches('0').trim_end_matches('.')),
            csv_nombre(*ht),
            csv_nombre(*tva),
        ));
    }
    Ok(csv_response(csv, "tva-par-taux.csv"))
}

async fn export_tiers(State(app): State<App>) -> Res<Response> {
    let csv = db::impex::exporter_tiers(&app.pool, entite::courante())?;
    Ok(csv_response(csv, "tiers.csv"))
}

async fn export_produits(State(app): State<App>) -> Res<Response> {
    let csv = db::impex::exporter_produits(&app.pool, entite::courante())?;
    Ok(csv_response(csv, "produits.csv"))
}

// --- sauvegarde -------------------------------------------------------------

async fn sauvegarde_page(Query(q): Query<HashMap<String, String>>) -> Res<Markup> {
    let racine =
        std::env::var("DOLIBARR_SAUVEGARDES").unwrap_or_else(|_| "sauvegardes".to_string());
    let listes = db::sauvegarde::lister(std::path::Path::new(&racine));

    Ok(ui::page(
        "parametres",
        "sauvegarde",
        "Sauvegarde",
        html! {
            @if let Some(fait) = q.get("fait") {
                p .flash { "Sauvegarde créée : " (fait) }
            }
            @if q.contains_key("echec") {
                p .erreur {(crate::langue::tr("La sauvegarde a échoué. Vérifiez que `mysqldump` est installé (variable DOLIBARR_MYSQLDUMP).")) }
            }
            h1 {(crate::langue::tr("Sauvegarde")) }
            p .sous-titre {(crate::langue::tr("Dump SQL complet + dossier documentaire, dans un dossier horodaté.")) }

            form method="post" action="/parametres/sauvegarde" .actions {
                button type="submit" {(crate::langue::tr("Sauvegarder maintenant")) }
                span .sous-titre { "Dossier : " (racine) }
            }

            h2 {(crate::langue::tr("Sauvegardes existantes")) }
            @if listes.is_empty() {
                (ui::vide("Aucune sauvegarde pour l'instant.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("Fichiers")) } } }
                    tbody {
                        @for (nom, n) in &listes {
                            tr { td .ref { (nom) } td .num { (n) } }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Restaurer")) }
            p {(crate::langue::tr("La restauration se fait en ligne de commande, c'est volontaire :")) }
            code {(crate::langue::tr("mysql -h 127.0.0.1 -P 3307 -u root -p dolibarr < sauvegardes/<date>/base.sql")) }
        },
    ))
}

async fn sauvegarder_maintenant(State(app): State<App>) -> Res<Redirect> {
    let racine =
        std::env::var("DOLIBARR_SAUVEGARDES").unwrap_or_else(|_| "sauvegardes".to_string());
    match db::sauvegarde::sauvegarder(&app.pool, std::path::Path::new(&racine)) {
        Ok(s) => {
            let nom = s
                .dossier
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            Ok(Redirect::to(&format!("/parametres/sauvegarde?fait={nom}")))
        }
        Err(_) => Ok(Redirect::to("/parametres/sauvegarde?echec=1")),
    }
}

fn csv_response(contenu: String, nom: &str) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{nom}\""),
            ),
        ],
        contenu,
    )
        .into_response()
}

fn pdf_response(bytes: &[u8], nom: &str) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{nom}\""),
            ),
        ],
        bytes.to_vec(),
    )
        .into_response()
}
