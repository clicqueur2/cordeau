//! API REST, pour brancher n8n, des scripts ou un agent.
//!
//! Authentification par en-tete `DOLAPIKEY`, comme Dolibarr. La cle vient de
//! la variable d'environnement `DOLIBARR_API_KEY`, sinon de la constante
//! `DOLIBARR_RS_API_KEY` dans `llx_const` (lue une fois au demarrage).
//!
//! Les montants sortent en chaines decimales, jamais en flottants : un agent
//! qui arrondit un total est un agent qui casse une comptabilite.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_decimal::Decimal;
use serde_json::{json, Value};

use crate::App;

/// Entite servie par l'API (une cle, une entite).
pub fn entity() -> i64 {
    std::env::var("DOLIBARR_API_ENTITY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

/// Cle d'API configuree (environnement d'abord).
pub fn cle_attendue() -> String {
    std::env::var("DOLIBARR_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| "dolibarr-rs".to_string())
}

fn autorise(headers: &HeaderMap) -> bool {
    let attendu = cle_attendue();
    let recue = headers
        .get("DOLAPIKEY")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    egal_temps_constant(recue, &attendu)
}

/// Comparaison a temps constant.
///
/// `==` sort au premier octet different : le temps de reponse dit combien de
/// caracteres de la cle sont justes, et une cle se devine octet par octet. Ce
/// n'est pas une faille a distance sur un serveur local, mais la correction
/// coute huit lignes.
///
/// La longueur, elle, est divulguee : la cle a une longueur connue de son
/// proprietaire, et le taire n'apporte rien.
fn egal_temps_constant(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut difference = 0u8;
    for (x, y) in a.iter().zip(b) {
        difference |= x ^ y;
    }
    difference == 0
}

type ApiResult = Result<Json<Value>, ApiErreur>;

/// Erreur JSON lisible par un automate : code + message, jamais de page HTML.
pub struct ApiErreur(axum::http::StatusCode, String);

impl IntoResponse for ApiErreur {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "erreur": self.1 }))).into_response()
    }
}

impl From<Box<dyn std::error::Error>> for ApiErreur {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        ApiErreur(axum::http::StatusCode::BAD_REQUEST, e.to_string())
    }
}

fn dec_str(d: Option<Decimal>) -> Value {
    match d {
        Some(v) => Value::String(v.normalize().to_string()),
        None => Value::Null,
    }
}

fn refus() -> ApiErreur {
    ApiErreur(
        axum::http::StatusCode::UNAUTHORIZED,
        "cle d'API absente ou invalide (en-tete DOLAPIKEY)".into(),
    )
}

/// Routes de l'API, a fusionner dans le routeur principal.
pub fn routes() -> Router<App> {
    Router::new()
        .route("/ping", get(ping))
        .route("/tiers", get(tiers).post(tiers_creer))
        .route("/produits", get(produits))
        .route("/devis", get(devis).post(devis_creer))
        .route("/devis/:id", get(devis_fiche))
        .route("/devis/:id/valider", post(devis_valider))
        .route("/devis/:id/situation", post(devis_situation))
        .route("/devis/:id/commande", post(devis_commande))
        .route("/factures", get(factures))
        .route("/factures/:id", get(facture_fiche))
        .route("/factures/:id/reglements", post(facture_reglement))
        .route("/commandes", get(commandes))
        .route("/commandes/:id/valider", post(commande_valider))
        .route("/commandes/:id/facturer", post(commande_facturer))
        .route("/banques", get(banques))
}

/// Les memes routes sous les noms de Dolibarr (`proposals`, `invoices`…).
///
/// C'est ce routeur qui rend Cordeau **remplacable** et non seulement
/// concurrent : une integration ecrite pour Dolibarr n'a rien a changer.
pub fn routes_dolibarr() -> Router<App> {
    Router::new()
        .route("/status", get(ping))
        .route("/thirdparties", get(tiers).post(tiers_creer))
        .route("/products", get(produits))
        .route("/proposals", get(devis).post(devis_creer))
        .route("/proposals/:id", get(devis_fiche).put(devis_modifier))
        .route("/proposals/:id/validate", post(devis_valider))
        .route("/proposals/:id/situation", post(devis_situation))
        .route("/proposals/:id/order", post(devis_commande))
        .route("/invoices", get(factures))
        .route("/invoices/:id", get(facture_fiche).put(facture_modifier))
        .route("/invoices/:id/payments", post(facture_reglement))
        .route("/orders", get(commandes))
        .route("/orders/:id/validate", post(commande_valider))
        .route("/orders/:id/invoice", post(commande_facturer))
        .route("/bankaccounts", get(banques))
        .route("/contacts", get(contacts))
        .route("/contacts/:id", get(contact_fiche))
        .route("/projects", get(projets))
        .route("/projects/:id", get(projet_fiche).put(projet_modifier))
        .route("/users", get(utilisateurs))
        .route("/thirdparties/:id", axum::routing::put(tiers_modifier))
        .route("/documents/download", get(document_telecharger))
        .route(
            "/documents/builddoc",
            axum::routing::put(document_construire),
        )
}

pub(crate) async fn ping(headers: HeaderMap) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    Ok(Json(json!({
        "produit": "dolibarr-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "schema": "dolibarr 24.0.1",
    })))
}

/// Telecharge un document archive : `?modulepart=facture&original_file=F-…/F-….pdf`
///
/// Dolibarr renvoie le fichier brut ; on fait pareil, avec le meme chemin
/// relatif. Un PDF absent est un 404, pas un 500.
pub(crate) async fn document_telecharger(
    State(_app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, ApiErreur> {
    if !autorise(&headers) {
        return Err(refus());
    }
    let modulepart = q
        .get("modulepart")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ApiErreur(
                axum::http::StatusCode::BAD_REQUEST,
                "modulepart requis".into(),
            )
        })?;
    let original = q
        .get("original_file")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ApiErreur(
                axum::http::StatusCode::BAD_REQUEST,
                "original_file requis".into(),
            )
        })?;

    // Le parametre vient d'un client HTTP. Deux pieges, pas un :
    //
    // 1. `..` sort du dossier ;
    // 2. **`Path::join` avec un composant absolu REMPLACE la racine** — sur
    //    Windows `C:\Windows\win.ini` et `\\serveur\partage` ne contiennent
    //    aucun `..` et ne commencent pas par `/` : l'ancien garde les laissait
    //    passer, et le serveur lisait n'importe quel fichier de la machine.
    //
    // On refuse donc tout caractere qui permet de sortir, puis on verifie que le
    // chemin **resolu** reste sous la racine. Le controle final est le seul qui
    // vaille : il ne depend d'aucune liste de motifs.
    let sur = |s: &str| {
        !s.is_empty()
            && !s.contains("..")
            && !s.contains('\\')
            && !s.contains(':')
            && !s.starts_with('/')
            && !s.starts_with('.')
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    };
    if !sur(&modulepart) || !sur(&original) {
        return Err(ApiErreur(
            axum::http::StatusCode::BAD_REQUEST,
            "chemin refuse".into(),
        ));
    }

    let racine = db::ged::racine();
    let chemin = racine.join(&modulepart).join(&original);
    let (Ok(racine_reelle), Ok(chemin_reel)) = (racine.canonicalize(), chemin.canonicalize())
    else {
        return Err(ApiErreur(
            axum::http::StatusCode::NOT_FOUND,
            format!("document introuvable : {modulepart}/{original}"),
        ));
    };
    if !chemin_reel.starts_with(&racine_reelle) {
        return Err(ApiErreur(
            axum::http::StatusCode::BAD_REQUEST,
            "chemin hors du dossier des documents".into(),
        ));
    }
    let chemin = chemin_reel;
    match std::fs::read(&chemin) {
        Ok(octets) => {
            let nom = chemin
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("document.pdf")
                .to_string();
            Ok((
                [
                    (
                        axum::http::header::CONTENT_TYPE,
                        "application/pdf".to_string(),
                    ),
                    (
                        axum::http::header::CONTENT_DISPOSITION,
                        format!("attachment; filename=\"{nom}\""),
                    ),
                ],
                octets,
            )
                .into_response())
        }
        Err(_) => Err(ApiErreur(
            axum::http::StatusCode::NOT_FOUND,
            format!("document introuvable : {modulepart}/{original}"),
        )),
    }
}

/// Construit le PDF d'une piece et rend le fichier : `{ modulepart, id }`.
///
/// C'est l'appel que fait n8n avant d'attacher un devis a un brouillon. On
/// repond le PDF directement, comme Dolibarr.
pub(crate) async fn document_construire(
    State(app): State<App>,
    headers: HeaderMap,
    Json(corps): Json<Value>,
) -> Result<Response, ApiErreur> {
    if !autorise(&headers) {
        return Err(refus());
    }
    let modulepart = corps
        .get("modulepart")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let id = corps
        .get("id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ApiErreur(axum::http::StatusCode::BAD_REQUEST, "id requis".into()))?;
    let entity = entity();

    let (ref_, octets) = match modulepart.as_str() {
        "propale" | "proposal" => {
            let devis = db::proposal::get(&app.pool, entity, id)?.ok_or_else(|| {
                ApiErreur(
                    axum::http::StatusCode::NOT_FOUND,
                    "devis introuvable".into(),
                )
            })?;
            let pdf = app::devis_pdf(&app.pool, entity, id)?.bytes;
            (devis.ref_, pdf)
        }
        "facture" | "invoice" => {
            let facture = db::invoice::get(&app.pool, entity, id)?.ok_or_else(|| {
                ApiErreur(
                    axum::http::StatusCode::NOT_FOUND,
                    "facture introuvable".into(),
                )
            })?;
            let pdf = app::facture_pdf(&app.pool, entity, id)?.bytes;
            (facture.ref_, pdf)
        }
        autre => {
            return Err(ApiErreur(
                axum::http::StatusCode::BAD_REQUEST,
                format!("modulepart \"{autre}\" non gere : propale ou facture"),
            ))
        }
    };

    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/pdf".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{ref_}.pdf\""),
            ),
        ],
        octets,
    )
        .into_response())
}

/// Contacts de l'entite, ou d'un tiers si `soc` est fourni.
async fn contacts(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let entity = entity();
    let liste = match q.get("soc").and_then(|v| v.trim().parse::<i64>().ok()) {
        Some(soc) => db::contact::par_tiers(&app.pool, entity, soc)?,
        None => db::contact::list(&app.pool, entity, 500)?,
    };
    Ok(Json(json!(liste
        .iter()
        .map(|c| json!({
            "id": c.rowid,
            "socid": c.fk_soc,
            "lastname": c.nom,
            "firstname": c.prenom,
            "email": c.email,
            "phone": c.telephone,
            "poste": c.poste,
            "ville": c.ville,
            "actif": c.actif,
        }))
        .collect::<Vec<_>>())))
}

async fn contact_fiche(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let c = db::contact::get(&app.pool, entity(), id)?.ok_or_else(|| {
        ApiErreur(
            axum::http::StatusCode::NOT_FOUND,
            "contact introuvable".into(),
        )
    })?;
    Ok(Json(json!({
        "id": c.rowid,
        "socid": c.fk_soc,
        "lastname": c.nom,
        "firstname": c.prenom,
        "email": c.email,
        "phone": c.telephone,
        "poste": c.poste,
        "ville": c.ville,
    })))
}

/// Chantiers (projets Dolibarr).
/// Met a jour l'entete d'un devis : notes et fin de validite.
///
/// Le statut et les lignes ne passent pas par ici : valider un devis a ses
/// propres controles, et reecrire des lignes par un PUT est une porte ouverte
/// aux totaux faux.
async fn devis_modifier(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(corps): Json<Value>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let champ = |cle: &str| corps.get(cle).and_then(|v| v.as_str());
    db::proposal::maj_entete(
        &app.pool,
        entity(),
        id,
        champ("note_private"),
        champ("note_public"),
        champ("fin_validite"),
    )?;
    let d = db::proposal::get(&app.pool, entity(), id)?.ok_or_else(|| {
        ApiErreur(
            axum::http::StatusCode::NOT_FOUND,
            "devis introuvable".into(),
        )
    })?;
    Ok(Json(json!({
        "id": d.rowid,
        "ref": d.ref_,
        "statut": d.fk_statut,
        "fin_validite": d.fin_validite,
        "note_private": d.note_private,
        "note_public": d.note_public,
    })))
}

/// Met a jour l'entete d'une facture : notes, echeance, reference client.
async fn facture_modifier(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(corps): Json<Value>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let champ = |cle: &str| corps.get(cle).and_then(|v| v.as_str());
    db::invoice::maj_entete(
        &app.pool,
        entity(),
        id,
        champ("note_private"),
        champ("note_public"),
        champ("date_lim_reglement"),
        champ("ref_client"),
    )?;
    let f = db::invoice::get(&app.pool, entity(), id)?.ok_or_else(|| {
        ApiErreur(
            axum::http::StatusCode::NOT_FOUND,
            "facture introuvable".into(),
        )
    })?;
    Ok(Json(json!({
        "id": f.rowid,
        "ref": f.ref_,
        "statut": f.fk_statut,
        "paye": f.paye,
        "date_lim_reglement": f.date_lim_reglement,
        "ref_client": f.ref_client,
        "note_private": f.note_private,
        "note_public": f.note_public,
    })))
}

async fn projets(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    // Un `limit` non plafonne fait lire et serialiser la table entiere : c'est
    // un deni de service a un parametre de requete.
    let limite = q
        .get("limit")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(100)
        .clamp(1, 200);
    Ok(Json(json!(db::projet::list(
        &app.pool,
        entity(),
        limite,
        0
    )?
    .iter()
    .map(|p| json!({
        "id": p.rowid,
        "ref": p.ref_,
        "title": p.title,
        "socid": p.fk_soc,
        "date_start": p.date_start,
        "date_end": p.date_end,
        "budget": dec_str(p.budget),
        "statut": p.fk_statut,
    }))
    .collect::<Vec<_>>())))
}

async fn projet_fiche(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let entity = entity();
    let p = db::projet::get(&app.pool, entity, id)?.ok_or_else(|| {
        ApiErreur(
            axum::http::StatusCode::NOT_FOUND,
            "projet introuvable".into(),
        )
    })?;
    let taches = db::projet::taches(&app.pool, entity, id)?;
    Ok(Json(json!({
        "id": p.rowid,
        "ref": p.ref_,
        "title": p.title,
        "socid": p.fk_soc,
        "date_start": p.date_start,
        "date_end": p.date_end,
        "budget": dec_str(p.budget),
        "statut": p.fk_statut,
        "taches": taches.iter().map(|t| json!({
            "id": t.rowid,
            "label": t.label,
            "debut": t.date_start,
            "fin": t.date_end,
            "avancement": t.progress,
        })).collect::<Vec<_>>(),
    })))
}

/// Utilisateurs de l'entite. Jamais de mot de passe, meme hache.
async fn utilisateurs(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    Ok(Json(json!(db::user::list(&app.pool, entity())?
        .iter()
        .map(|u| json!({
            "id": u.rowid,
            "login": u.login,
            "nom": u.nom_complet(),
            "email": u.email,
            "actif": u.actif(),
        }))
        .collect::<Vec<_>>())))
}

/// Met a jour les champs courants d'un tiers.
///
/// Un champ absent garde sa valeur : un PUT partiel ne doit pas vider une fiche.
/// C'est la difference entre « modifier » et « remplacer ».
async fn tiers_modifier(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(corps): Json<Value>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let entity = entity();
    let actuel = db::party::get(&app.pool, entity, id)?.ok_or_else(|| {
        ApiErreur(
            axum::http::StatusCode::NOT_FOUND,
            "tiers introuvable".into(),
        )
    })?;

    let champ = |cle: &str| {
        corps
            .get(cle)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let nom = champ("name")
        .or(champ("nom"))
        .unwrap_or_else(|| actuel.nom.clone());
    let mut maj = db::party::NewParty::client(&nom);
    maj.address = champ("address").or(actuel.address.clone());
    maj.zip = champ("zip").or(actuel.zip.clone());
    maj.town = champ("town").or(actuel.town.clone());
    maj.email = champ("email").or(actuel.email.clone());
    maj.phone = champ("phone").or(actuel.phone.clone());
    maj.siret = champ("siret").or(actuel.siret.clone());
    maj.tva_intra = champ("tva_intra").or(actuel.tva_intra.clone());
    db::party::update(&app.pool, entity, id, &maj)?;
    Ok(Json(json!({ "id": id, "modifie": true })))
}

/// Met a jour les dates, le titre et le budget d'un chantier.
async fn projet_modifier(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(corps): Json<Value>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let entity = entity();
    let actuel = db::projet::get(&app.pool, entity, id)?.ok_or_else(|| {
        ApiErreur(
            axum::http::StatusCode::NOT_FOUND,
            "projet introuvable".into(),
        )
    })?;
    let champ = |cle: &str| {
        corps
            .get(cle)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let titre = champ("title").unwrap_or_else(|| actuel.title.clone());
    let mut maj = db::projet::NewProjet::new(&titre);
    maj.fk_soc = actuel.fk_soc;
    maj.date_start = champ("date_start").or(actuel.date_start.clone());
    maj.date_end = champ("date_end").or(actuel.date_end.clone());
    maj.description = actuel.description.clone();
    maj.budget = corps
        .get("budget")
        .and_then(|v| v.as_str())
        .and_then(|v| v.trim().replace(',', ".").parse().ok())
        .or(actuel.budget)
        .unwrap_or_default();
    db::projet::update(&app.pool, entity, id, &maj)?;
    Ok(Json(json!({ "id": id, "modifie": true })))
}

async fn tiers(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let limite: u32 = q
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
        .clamp(1, 200);
    let liste = db::party::list(&app.pool, entity(), limite, 0)?;
    Ok(Json(json!(liste
        .iter()
        .map(|t| json!({
            "id": t.rowid,
            "nom": t.nom,
            "email": t.email,
            "ville": t.town,
            "siret": t.siret,
            "client": t.client,
            "fournisseur": t.fournisseur,
        }))
        .collect::<Vec<_>>())))
}

pub(crate) async fn tiers_creer(
    State(app): State<App>,
    headers: HeaderMap,
    Json(corps): Json<Value>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let nom = corps
        .get("nom")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .ok_or_else(|| {
            ApiErreur(
                axum::http::StatusCode::BAD_REQUEST,
                "champ nom obligatoire".into(),
            )
        })?;
    let mut nouveau = db::party::NewParty::client(nom);
    nouveau.email = corps
        .get("email")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    nouveau.town = corps
        .get("ville")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    nouveau.siret = corps
        .get("siret")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let id = db::party::create(&app.pool, entity(), &nouveau)?;
    Ok(Json(json!({ "id": id })))
}

pub(crate) async fn produits(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let liste = db::product::list(&app.pool, entity(), 100, 0)?;
    Ok(Json(json!(liste
        .iter()
        .map(|p| json!({
            "id": p.rowid,
            "ref": p.ref_,
            "label": p.label,
            "prix_ht": dec_str(p.price),
            "tva": dec_str(p.tva_tx),
        }))
        .collect::<Vec<_>>())))
}

pub(crate) async fn devis(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let liste = db::proposal::list(&app.pool, entity(), 100, 0)?;
    Ok(Json(json!(liste
        .iter()
        .map(|d| json!({
            "id": d.rowid,
            "ref": d.ref_,
            "tiers": d.fk_soc,
            "date": d.datep,
            "statut": d.fk_statut,
            "total_ht": dec_str(d.total_ht),
            "total_ttc": dec_str(d.total_ttc),
        }))
        .collect::<Vec<_>>())))
}

pub(crate) async fn devis_fiche(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let d = db::proposal::get(&app.pool, entity(), id)?.ok_or_else(|| {
        ApiErreur(
            axum::http::StatusCode::NOT_FOUND,
            "devis introuvable".into(),
        )
    })?;
    let lignes = db::proposal::lines(&app.pool, entity(), id)?;
    Ok(Json(json!({
        "id": d.rowid,
        "ref": d.ref_,
        "tiers": d.fk_soc,
        "statut": d.fk_statut,
        "total_ht": dec_str(d.total_ht),
        "total_ttc": dec_str(d.total_ttc),
        "lignes": lignes.iter().map(|l| json!({
            "label": l.label,
            "qty": dec_str(l.qty),
            "pu_ht": dec_str(l.subprice),
            "tva": dec_str(l.tva_tx),
            "total_ht": dec_str(l.total_ht),
        })).collect::<Vec<_>>(),
        "situations": db::situation::situations(&app.pool, entity(), id)?.iter().map(|s| json!({
            "id": s.rowid,
            "ref": s.ref_,
            "numero": s.counter,
            "finale": s.finale,
            "pourcentage": dec_str(Some(s.pourcentage)),
            "total_ttc": dec_str(Some(s.total_ttc)),
        })).collect::<Vec<_>>(),
    })))
}

pub(crate) async fn devis_creer(
    State(app): State<App>,
    headers: HeaderMap,
    Json(corps): Json<Value>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let fk_soc = corps.get("tiers").and_then(|v| v.as_i64()).ok_or_else(|| {
        ApiErreur(
            axum::http::StatusCode::BAD_REQUEST,
            "champ tiers obligatoire".into(),
        )
    })?;
    let lignes = corps
        .get("lignes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            ApiErreur(
                axum::http::StatusCode::BAD_REQUEST,
                "champ lignes obligatoire".into(),
            )
        })?;

    let mut nouveau = db::proposal::NewProposal::new(fk_soc);
    nouveau.fin_validite = corps
        .get("validite")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    // Un prix peut arriver en chaine (`"120.00"`) ou en nombre (`120`) : les deux
    // sont legitimes, et n'accepter que la chaine faisait **0 €** en silence sur
    // un nombre. Une quantite par defaut de 1 est un choix ; un prix par defaut
    // de 0 est un devis faux.
    fn nombre_json(v: Option<&Value>, defaut: Option<&str>) -> Result<String, ApiErreur> {
        match v {
            Some(Value::String(s)) => Ok(s.trim().to_string()),
            Some(Value::Number(n)) => Ok(n.to_string()),
            Some(Value::Null) | None => defaut.map(str::to_string).ok_or_else(|| {
                ApiErreur(
                    axum::http::StatusCode::BAD_REQUEST,
                    "chaque ligne doit porter un prix `pu_ht`".into(),
                )
            }),
            Some(autre) => Err(ApiErreur(
                axum::http::StatusCode::BAD_REQUEST,
                format!("prix illisible : {autre}"),
            )),
        }
    }

    let mut pretes = Vec::new();
    for l in lignes {
        let label = l.get("label").and_then(|v| v.as_str()).unwrap_or("Ligne");
        let qty = nombre_json(l.get("qty"), Some("1"))?;
        let pu = nombre_json(l.get("pu_ht"), None)?;
        let tva = nombre_json(l.get("tva"), Some("20"))?;
        // `money::tenter` : une valeur venue d'un client ne doit pas paniquer.
        let dec = |v: &str, nom: &str| {
            money::tenter(v)
                .map_err(|e| ApiErreur(axum::http::StatusCode::BAD_REQUEST, format!("{nom} : {e}")))
        };
        let qte = dec(&qty, "qty")?;
        let prix = dec(&pu, "pu_ht")?;
        let taux = dec(&tva, "tva")?;
        pretes.push(db::proposal::NewProposalLine::new(label, qte, prix, taux));
    }
    nouveau.lines = pretes;
    let id = db::proposal::create(&app.pool, entity(), &nouveau)?;
    Ok(Json(json!({ "id": id })))
}

pub(crate) async fn devis_valider(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    db::proposal::validate(&app.pool, entity(), id)?;
    Ok(Json(json!({ "id": id, "statut": 1 })))
}

pub(crate) async fn devis_situation(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(corps): Json<Value>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let pct = corps
        .get("pourcentage")
        .and_then(|v| v.as_str())
        .map(money::dec)
        .ok_or_else(|| {
            ApiErreur(
                axum::http::StatusCode::BAD_REQUEST,
                "champ pourcentage obligatoire".into(),
            )
        })?;
    let date = corps.get("date").and_then(|v| v.as_str());
    let facture = db::situation::creer(&app.pool, entity(), id, pct, date)?;
    Ok(Json(json!({ "facture": facture })))
}

pub(crate) async fn devis_commande(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let commande = db::commande::create_from_proposal(&app.pool, entity(), id)?;
    Ok(Json(json!({ "commande": commande })))
}

pub(crate) async fn factures(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let liste = db::invoice::list(&app.pool, entity(), 100, 0)?;
    Ok(Json(json!(liste
        .iter()
        .map(|f| {
            let solde = db::payment::balance(&app.pool, entity(), f.rowid).ok();
            json!({
                "id": f.rowid,
                "ref": f.ref_,
                "type": f.type_,
                "tiers": f.fk_soc,
                "date": f.datef,
                "echeance": f.date_lim_reglement,
                "statut": f.fk_statut,
                "total_ttc": dec_str(f.total_ttc),
                "reste_du": dec_str(solde.map(|s| s.reste)),
            })
        })
        .collect::<Vec<_>>())))
}

pub(crate) async fn facture_fiche(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let f = db::invoice::get(&app.pool, entity(), id)?.ok_or_else(|| {
        ApiErreur(
            axum::http::StatusCode::NOT_FOUND,
            "facture introuvable".into(),
        )
    })?;
    let lignes = db::invoice::lines(&app.pool, entity(), id)?;
    let solde = db::payment::balance(&app.pool, entity(), id)?;
    Ok(Json(json!({
        "id": f.rowid,
        "ref": f.ref_,
        "type": f.type_,
        "tiers": f.fk_soc,
        "statut": f.fk_statut,
        "total_ttc": dec_str(f.total_ttc),
        "regle": dec_str(Some(solde.regle)),
        "reste_du": dec_str(Some(solde.reste)),
        "situation": f.situation_counter,
        "lignes": lignes.iter().map(|l| json!({
            "label": l.label,
            "qty": dec_str(l.qty),
            "total_ht": dec_str(l.total_ht),
        })).collect::<Vec<_>>(),
    })))
}

pub(crate) async fn facture_reglement(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(corps): Json<Value>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let montant = corps
        .get("montant")
        .and_then(|v| v.as_str())
        .map(money::dec)
        .ok_or_else(|| {
            ApiErreur(
                axum::http::StatusCode::BAD_REQUEST,
                "champ montant obligatoire".into(),
            )
        })?;
    let mode = match corps
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("virement")
    {
        "cheque" => db::payment::MODE_CHEQUE,
        "especes" => db::payment::MODE_ESPECES,
        "carte" => db::payment::MODE_CARTE,
        "prelevement" => db::payment::MODE_PRELEVEMENT,
        _ => db::payment::MODE_VIREMENT,
    };
    let nouveau = db::payment::NewPayment {
        fk_facture: id,
        montant,
        fk_paiement: mode,
        fk_compte: corps.get("compte").and_then(|v| v.as_i64()),
        date: corps
            .get("date")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        ..Default::default()
    };
    let paiement = db::payment::create(&app.pool, entity(), &nouveau)?;
    Ok(Json(json!({ "paiement": paiement })))
}

pub(crate) async fn commandes(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let liste = db::commande::list(&app.pool, entity(), 100, 0)?;
    Ok(Json(json!(liste
        .iter()
        .map(|c| json!({
            "id": c.rowid,
            "ref": c.ref_,
            "tiers": c.fk_soc,
            "statut": c.fk_statut,
            "total_ttc": dec_str(c.total_ttc),
        }))
        .collect::<Vec<_>>())))
}

pub(crate) async fn commande_valider(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    db::commande::validate(&app.pool, entity(), id)?;
    Ok(Json(json!({ "id": id, "statut": 1 })))
}

pub(crate) async fn commande_facturer(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let facture = db::invoice::create_from_commande(&app.pool, entity(), id)?;
    Ok(Json(json!({ "facture": facture })))
}

pub(crate) async fn banques(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let comptes = db::banque::comptes(&app.pool, entity())?;
    let mut sortie = Vec::new();
    for c in &comptes {
        let solde = db::banque::solde(&app.pool, entity(), c.rowid)?;
        sortie.push(json!({
            "id": c.rowid,
            "ref": c.ref_,
            "label": c.label,
            "solde": dec_str(Some(solde)),
            "clos": c.clos,
        }));
    }
    Ok(Json(json!(sortie)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparaison_de_cle_egale() {
        assert!(egal_temps_constant("secret", "secret"));
        assert!(!egal_temps_constant("secret", "secreT"));
        assert!(!egal_temps_constant("secret", "secre"));
        assert!(!egal_temps_constant("", "secret"));
        // Deux chaines vides sont egales : la fonction ne sert qu'a comparer
        // une cle configuree, jamais a valider une absence.
        assert!(egal_temps_constant("", ""));
    }
}
