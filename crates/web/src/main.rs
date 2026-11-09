//! Interface web : lecture et actions sur la base Dolibarr.
//!
//! Rendu cote serveur (maud), sans JavaScript. Les actions sont des `POST`
//! suivis d'une redirection ; le message de confirmation passe par `?ok=`.

mod actions_masse;
mod adherent_ecran;
mod analytique_ecran;
mod api;
mod auth;
pub(crate) use utilisateurs_ecran::{utilisateurs, utilisateurs_basculer, utilisateurs_creer};
mod utilisateurs_ecran;
pub(crate) use imports_ecran::{import_factures, import_page, import_produits, import_tiers};
mod imports_ecran;
pub(crate) use comptabilite_ecran::{
    comptabilite, export_achats, export_fec, export_grand_livre, export_produits, export_tiers,
    export_tva, export_ventes, fec_generer, fec_page, grand_livre, pdf_response, sauvegarde_page,
    sauvegarder_maintenant,
};
mod comptabilite_ecran;
pub(crate) use parametres_ecran::{
    condition_reglement_creer, condition_reglement_desactiver, parametres, parametres_base,
    parametres_cgv_modele, parametres_dictionnaires, parametres_module_basculer,
    parametres_modules, parametres_societe,
};
mod parametres_ecran;
pub(crate) use documents_ecran::{agenda, documents};
mod documents_ecran;
pub(crate) use banques_ecran::{banque_creer, banque_rapprocher, banques};
mod banques_ecran;
pub(crate) use achats_ecran::{
    achat_facture_creer, achat_facture_nouvelle, achat_facture_valider, achat_fournisseur_creer,
    achats,
};
mod achats_ecran;
mod achats_fournisseur_ecran;
pub(crate) use chantiers_ecran::{chantier_creer, chantier_fiche, chantier_tache_creer, chantiers};
mod chantiers_ecran;
pub(crate) use commandes_ecran::{
    commande_acompte, commande_facturer, commande_fiche, commande_pdf, commande_valider, commandes,
};
mod commandes_ecran;
pub(crate) use carnet_ecran::{carnet, marges_chantiers};
mod carnet_ecran;
pub(crate) use facturation_ecran::{
    factures, factures_avoir, factures_creer, factures_fiche, factures_nouveau, factures_pdf,
    factures_recurrence, factures_reglement, factures_valider, relances,
};
mod facturation_ecran;
pub(crate) use ventes_ecran::{
    devis, devis_commander, devis_creer, devis_enregistrer, devis_facturer, devis_fiche,
    devis_modifier, devis_nouveau, devis_pdf, devis_perdu, devis_signer, devis_situation,
    devis_valider, table_lignes,
};
mod ventes_ecran;
pub(crate) use produits_ecran::{
    produit_enregistrer, produit_fiche, produits, produits_creer, produits_nouveau,
};
mod produits_ecran;
pub(crate) use tiers_ecran::{
    tiers, tiers_creer, tiers_enregistrer, tiers_fiche, tiers_modifier, tiers_nouveau,
};
mod tiers_ecran;
pub(crate) use accueil_ecran::{
    accueil, aujourdhui, en_retard, message, nom_tiers, pluriel, recherche,
};
mod accueil_ecran;
mod campagnes;
mod categories_ecran;
mod cloture_ecran;
mod conges_ecran;
mod connaissances_ecran;
mod constantes_ecran;
mod courriel;
mod devises_ecran;
mod dons_ecran;
mod entite;
mod expedition_ecran;
mod extrafields_ecran;
mod garde;
pub(crate) mod icones;
mod immobilisation_ecran;
mod interv_ecran;
mod inventaire_ecran;
mod journal_erreurs;
mod journaux_ecran;
mod langue;
mod lettrage_ecran;
mod livraison_ecran;
mod livre_tenu;
mod mailing_ecran;
mod manifeste;
mod marge_ecran;
mod menu;
mod menu_actif;
mod modeles_ecran;
mod mrp_ecran;
mod nomenclature_ecran;
mod note_frais_ecran;
mod notifications_ecran;
mod pages;
mod partenariat_ecran;
mod pdf_modele_ecran;
mod planification_ecran;
mod plugin;
mod pos_ecran;
mod poste_ecran;
mod preset_ecran;
mod pret_ecran;
mod rapprochement_ecran;
mod reception_ecran;
mod recur_ecran;
mod routes;
mod securite_ecran;
mod sepa_ecran;
mod site_web_ecran;
mod stats_ecran;
mod theme;
mod traduction_ecran;
mod transfert_ecran;
pub(crate) mod ui;
mod utilisateur;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
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

/// Quel code HTTP pour quel message.
///
/// « introuvable » et « inconnue » disent qu'une adresse ne mene nulle part :
/// c'est un 404, pas une panne. Tout le reste est un 500, et se journalise.
fn code_pour(message: &str) -> StatusCode {
    let m = message.to_lowercase();
    if m.contains("introuvable") || m.contains("inconnu") {
        StatusCode::NOT_FOUND
    } else if m.contains("obligatoire") || m.contains("invalide") {
        // Un champ manquant n'est pas une adresse morte : c'est une demande
        // mal formee.
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

impl IntoResponse for Erreur {
    fn into_response(self) -> Response {
        // Toute erreur servie au navigateur laisse une trace.
        crate::journal_erreurs::enregistrer("erreur", &self.0);
        // Un document qui n'existe pas n'est pas une panne du serveur : c'est
        // une adresse qui ne mene nulle part. Repondre 500 fait croire a un
        // incident, et les sondes de disponibilite crient au loup.
        let code = code_pour(&self.0);
        (
            code,
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

    // Les tables du module Marges : Dolibarr n'en a aucune, il recalcule a la
    // volee. La creation est idempotente, et une base qui refuse de les creer
    // n'empeche pas le reste du noyau de demarrer — seules les marges seront
    // indisponibles, et le message le dit.
    if let Err(e) = db::marge::preparer(&pool) {
        eprintln!("tables de marge indisponibles : {e}");
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

    // Le routeur complet, ses couches comprises. Voir `routes`.
    let routes = routes::routes(app);

    let ecoute =
        std::env::var("DOLIBARR_WEB_ADDR").unwrap_or_else(|_| "127.0.0.1:8090".to_string());
    let listener = tokio::net::TcpListener::bind(&ecoute)
        .await
        .expect("port indisponible");
    println!("dolibarr-web sur http://{ecoute}");
    axum::serve(listener, routes).await.expect("serveur");
}

// --- session ----------------------------------------------------------------

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

// --- tableau de bord --------------------------------------------------------

// --- tiers ------------------------------------------------------------------

// --- produits ---------------------------------------------------------------

// --- devis ------------------------------------------------------------------

// --- factures ---------------------------------------------------------------

// --- paramètres et dictionnaires --------------------------------------------

// --- creation ---------------------------------------------------------------

// --- edition -----------------------------------------------------------------

// --- facture directe et relances --------------------------------------------

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

// --- import / export --------------------------------------------------------

// --- sauvegarde -------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Une adresse qui ne mene nulle part n'est pas un incident.
    #[test]
    fn une_liste_inconnue_repond_404() {
        assert_eq!(code_pour("liste inconnue : Product"), StatusCode::NOT_FOUND);
        assert_eq!(code_pour("Tiers introuvable"), StatusCode::NOT_FOUND);
    }

    /// Un champ manquant est une demande mal formee, pas une adresse morte.
    #[test]
    fn un_champ_obligatoire_repond_400() {
        assert_eq!(
            code_pour("nom du contact obligatoire"),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(code_pour("statut invalide"), StatusCode::BAD_REQUEST);
    }

    /// Le reste est une vraie panne, et se journalise comme telle.
    #[test]
    fn une_erreur_de_base_repond_500() {
        assert_eq!(
            code_pour("Table 'llx_facture' doesn't exist"),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
