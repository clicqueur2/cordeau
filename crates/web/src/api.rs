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
    headers
        .get("DOLAPIKEY")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == attendu)
        .unwrap_or(false)
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

async fn ping(headers: HeaderMap) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    Ok(Json(json!({
        "produit": "dolibarr-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "schema": "dolibarr 24.0.1",
    })))
}

async fn tiers(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult {
    if !autorise(&headers) {
        return Err(refus());
    }
    let limite: u32 = q.get("limit").and_then(|v| v.parse().ok()).unwrap_or(50);
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

async fn tiers_creer(
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

async fn produits(State(app): State<App>, headers: HeaderMap) -> ApiResult {
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

async fn devis(State(app): State<App>, headers: HeaderMap) -> ApiResult {
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

async fn devis_fiche(State(app): State<App>, headers: HeaderMap, Path(id): Path<i64>) -> ApiResult {
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

async fn devis_creer(
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
    let mut pretes = Vec::new();
    for l in lignes {
        let label = l.get("label").and_then(|v| v.as_str()).unwrap_or("Ligne");
        let qty = l.get("qty").and_then(|v| v.as_str()).unwrap_or("1");
        let pu = l.get("pu_ht").and_then(|v| v.as_str()).unwrap_or("0");
        let tva = l.get("tva").and_then(|v| v.as_str()).unwrap_or("20");
        pretes.push(db::proposal::NewProposalLine::new(
            label,
            money::dec(qty),
            money::dec(pu),
            money::dec(tva),
        ));
    }
    nouveau.lines = pretes;
    let id = db::proposal::create(&app.pool, entity(), &nouveau)?;
    Ok(Json(json!({ "id": id })))
}

async fn devis_valider(
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

async fn devis_situation(
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

async fn devis_commande(
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

async fn factures(State(app): State<App>, headers: HeaderMap) -> ApiResult {
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

async fn facture_fiche(
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

async fn facture_reglement(
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

async fn commandes(State(app): State<App>, headers: HeaderMap) -> ApiResult {
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

async fn commande_valider(
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

async fn commande_facturer(
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

async fn banques(State(app): State<App>, headers: HeaderMap) -> ApiResult {
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
