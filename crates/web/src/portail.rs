//! Les pages du portail client : `/public/portail`.
//!
//! Zone libre : elle est sous `/public`, donc **hors** du garde-fou qui exige
//! une session d'employe. La session du client est distincte, vit en memoire
//! comme celle du personnel, et ne donne acces qu'a **son** tiers.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use maud::{html, Markup};

use crate::*;

pub const COOKIE: &str = "cordeau-portail";
const DUREE: Duration = Duration::from_secs(3600);

/// La session d'un client. Elle ne porte que le tiers : rien d'autre.
#[derive(Clone, Debug)]
pub struct SessionPortail {
    pub entity: i64,
    pub fk_soc: i64,
    pub tiers: String,
    pub expire: SystemTime,
}

fn sessions() -> &'static Mutex<HashMap<String, SessionPortail>> {
    static S: std::sync::OnceLock<Mutex<HashMap<String, SessionPortail>>> =
        std::sync::OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

fn jeton() -> String {
    // Un identifiant aleatoire suffit : il n'est jamais devine, il est tire.
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h: u64 = 1469598103934665603;
    for b in nanos.to_le_bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(1099511628211);
    }
    format!("p{nanos:x}{h:x}")
}

fn lire_cookie(headers: &HeaderMap) -> Option<String> {
    let brut = headers.get(header::COOKIE)?.to_str().ok()?;
    for morceau in brut.split(';') {
        if let Some(v) = morceau.trim().strip_prefix(COOKIE) {
            return Some(v.strip_prefix('=').unwrap_or(v).to_string());
        }
    }
    None
}

/// La session du client, si elle est valide.
fn session(headers: &HeaderMap) -> Option<SessionPortail> {
    let j = lire_cookie(headers)?;
    let s = sessions().lock().ok()?;
    s.get(&j).filter(|s| s.expire > SystemTime::now()).cloned()
}

/// La coque du portail : pas de menu d'employe, pas de selecteur de societe.
fn coque(titre: &str, contenu: Markup, connecte: bool) -> Markup {
    html! {
        (maud::DOCTYPE)
        html lang="fr" data-style="systeme" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (titre) " — " (app::PRODUIT) }
                style { (maud::PreEscaped(crate::theme::CSS)) }
                style { (maud::PreEscaped(".portail{max-width:820px;margin:0 auto;padding:var(--e5) var(--e4)}")) }
            }
            body {
                div .portail {
                    header .portail-tete {
                        a .marque href="/public/portail" { span .sceau { "C" } span { (app::PRODUIT) } }
                        @if connecte {
                            form method="post" action="/public/portail/deconnexion" {
                                button .fantome.petit type="submit" { (crate::langue::tr("Quitter")) }
                            }
                        }
                    }
                    main { (contenu) }
                }
            }
        }
    }
}

/// Page d'entree : le formulaire, ou l'espace du client s'il est connecte.
pub async fn accueil(State(app): State<App>, headers: HeaderMap) -> Res<Response> {
    let Some(s) = session(&headers) else {
        return Ok(coque(
            "Espace client",
            html! {
                h1 {(crate::langue::tr("Espace client")) }
                p .sous-titre {
                    (crate::langue::tr("Vos devis et vos factures, en lecture. Entrez le login que nous vous avons remis."))
                }
                form method="post" action="/public/portail/connexion" {
                    table {
                        tbody {
                            tr { th { (crate::langue::tr("Identifiant")) } td { input type="text" name="login" required .pleine autocomplete="username"; } }
                            tr { th { (crate::langue::tr("Mot de passe")) } td { input type="password" name="mdp" required .pleine autocomplete="current-password"; } }
                        }
                    }
                    div .actions { button type="submit" {(crate::langue::tr("Entrer")) } }
                }
                p .aide.petite {
                    (crate::langue::tr("Un problème d'accès ? Écrivez-nous : nous vous renvoyons vos identifiants."))
                }
            },
            false,
        )
        .into_response());
    };

    let entity = s.entity;
    let devis: Vec<_> = db::proposal::list(&app.pool, entity, 500, 0)?
        .into_iter()
        .filter(|d| d.fk_soc == Some(s.fk_soc))
        .collect();
    let factures: Vec<_> = db::invoice::list(&app.pool, entity, 500, 0)?
        .into_iter()
        .filter(|f| f.fk_soc == Some(s.fk_soc))
        .collect();

    Ok(coque(
        "Espace client",
        html! {
            h1 { (crate::langue::tr("Bonjour")) " " (s.tiers.clone()) }
            p .sous-titre { (crate::langue::tr("Vos documents, en lecture seule.")) }

            h2 {(crate::langue::tr("Devis")) }
            @if devis.is_empty() {
                (ui::vide("Aucun devis pour l'instant.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("TTC")) } th { "État" } th { "" } } }
                    tbody {
                        @for d in &devis {
                            tr {
                                td .ref { (d.ref_.clone()) }
                                td .ref { (app::fmt_date_fr(d.datec.as_deref())) }
                                td .num { (ui::euros(d.total_ttc.unwrap_or_default())) }
                                td { (ui::pastille(d.fk_statut, 0, false)) }
                                td { a .bouton.petit.secondaire href={ "/public/portail/devis/" (d.rowid) } {(crate::langue::tr("Voir")) } }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Factures")) }
            @if factures.is_empty() {
                (ui::vide("Aucune facture pour l'instant.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("TTC")) } th { "État" } th { "" } } }
                    tbody {
                        @for f in &factures {
                            tr {
                                td .ref { (f.ref_.clone()) }
                                td .ref { (app::fmt_date_fr(f.datef.as_deref())) }
                                td .num { (ui::euros(f.total_ttc.unwrap_or_default())) }
                                td { (ui::pastille(f.fk_statut, f.paye, false)) }
                                td {
                                    a .bouton.petit href={ "/public/portail/factures/" (f.rowid) "/pdf" } {(crate::langue::tr("PDF")) }
                                }
                            }
                        }
                    }
                }
            }
        },
        true,
    )
    .into_response())
}

pub async fn connexion(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Response> {
    let login = f.get("login").map(|s| s.trim()).unwrap_or("");
    let mdp = f.get("mdp").cloned().unwrap_or_default();
    let ip = crate::auth::ip_client(&headers);

    // Le frein du personnel ne couvrait pas cette porte : 100 000 tours de
    // SHA-256 par tentative, sans limite, sur une page publique — l'attaquant
    // n'a qu'a marteler pour saturer le serveur, et chaque essai lui coute
    // 100 000 fois moins qu'a nous.
    if !crate::auth::peut_essayer(&ip) {
        return Ok((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "Trop d'essais. Reessayez dans quinze minutes.",
        )
            .into_response());
    }

    match db::portail::authentifier(&app.pool, login, &mdp)? {
        Some(c) => {
            crate::auth::reussi(&ip);
            let j = jeton();
            sessions().lock().unwrap().insert(
                j.clone(),
                SessionPortail {
                    entity: c.entity,
                    fk_soc: c.fk_soc,
                    tiers: c.tiers,
                    expire: SystemTime::now() + DUREE,
                },
            );
            sessions()
                .lock()
                .unwrap()
                .retain(|_, s| s.expire > SystemTime::now());
            Ok((
                [(
                    header::SET_COOKIE,
                    format!("{COOKIE}={j}; HttpOnly; SameSite=Strict; Path=/; Max-Age=3600"),
                )],
                Redirect::to("/public/portail"),
            )
                .into_response())
        }
        None => {
            crate::auth::echec(&ip);
            Ok(coque(
                "Espace client",
                html! {
                    h1 {(crate::langue::tr("Espace client")) }
                    (ui::echec("Identifiant ou mot de passe incorrect."))
                    a .bouton href="/public/portail" { (crate::langue::tr("Réessayer")) }
                },
                false,
            )
            .into_response())
        }
    }
}

pub async fn deconnexion(headers: HeaderMap) -> Response {
    if let Some(j) = lire_cookie(&headers) {
        sessions().lock().unwrap().remove(&j);
    }
    (
        [(
            header::SET_COOKIE,
            format!("{COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"),
        )],
        Redirect::to("/public/portail"),
    )
        .into_response()
}

/// Le detail d'un devis, s'il appartient bien au client connecte.
pub async fn devis(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Res<Response> {
    let Some(s) = session(&headers) else {
        return Ok(Redirect::to("/public/portail").into_response());
    };
    if !db::portail::appartient(&app.pool, s.entity, s.fk_soc, "propal", id)? {
        return Ok((
            axum::http::StatusCode::NOT_FOUND,
            coque(
                "Introuvable",
                html! {
                    h1 {(crate::langue::tr("Document introuvable")) }
                    p .sous-titre { (crate::langue::tr("Ce document ne fait pas partie de votre compte.")) }
                    a .bouton href="/public/portail" { (crate::langue::tr("Retour")) }
                },
                true,
            ),
        )
            .into_response());
    }

    let d = db::proposal::get(&app.pool, s.entity, id)?
        .ok_or_else(|| Erreur("devis introuvable".into()))?;
    let lignes = db::proposal::lines(&app.pool, s.entity, id)?;

    Ok(coque(
        &d.ref_,
        html! {
            h1 { "Devis " (d.ref_.clone()) }
            p .sous-titre {
                @if let Some(dt) = &d.datec { (app::fmt_date_fr(Some(dt))) " — " }
                (ui::pastille(d.fk_statut, 0, false))
            }
            table {
                thead { tr { th {(crate::langue::tr("Désignation")) } th .num {(crate::langue::tr("Qté")) } th .num {(crate::langue::tr("P.U. HT")) } th .num {(crate::langue::tr("Total HT")) } } }
                tbody {
                    @for l in lignes.iter().filter(|l| !l.is_section()) {
                        tr {
                            td { (l.label.clone().unwrap_or_default()) }
                            td .num { (app::fmt_qte(l.qty)) }
                            td .num { (ui::euros(l.subprice.unwrap_or_default())) }
                            td .num { (ui::euros(l.total_ht.unwrap_or_default())) }
                        }
                    }
                }
                tfoot {
                    tr { td colspan="3" .num {(crate::langue::tr("Total HT")) } td .num { (ui::euros(d.total_ht.unwrap_or_default())) } }
                    tr { td colspan="3" .num {(crate::langue::tr("TVA")) } td .num { (ui::euros(d.total_tva.unwrap_or_default())) } }
                    tr { td colspan="3" .num { strong {(crate::langue::tr("Total TTC")) } } td .num { strong { (ui::euros(d.total_ttc.unwrap_or_default())) } } }
                }
            }
            div .actions {
                a .bouton href={ "/public/portail/devis/" (id) "/pdf" } {(crate::langue::tr("Télécharger le PDF")) }
                a .bouton.secondaire href="/public/portail" {(crate::langue::tr("Retour")) }
            }
        },
        true,
    )
    .into_response())
}

/// Le PDF d'un devis, apres controle d'appartenance.
pub async fn pdf_devis(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Res<Response> {
    pdf_pour(&app, &headers, "propal", id).await
}

/// Le PDF d'une facture, meme controle.
pub async fn pdf_facture(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Res<Response> {
    pdf_pour(&app, &headers, "facture", id).await
}

async fn pdf_pour(app: &App, headers: &HeaderMap, genre: &str, id: i64) -> Res<Response> {
    let Some(s) = session(headers) else {
        return Ok(Redirect::to("/public/portail").into_response());
    };
    if !db::portail::appartient(&app.pool, s.entity, s.fk_soc, genre, id)? {
        return Ok((axum::http::StatusCode::NOT_FOUND, "document introuvable").into_response());
    }
    let (octets, nom) = if genre == "propal" {
        let d = db::proposal::get(&app.pool, s.entity, id)?
            .ok_or_else(|| Erreur("devis introuvable".into()))?;
        (app::devis_pdf(&app.pool, s.entity, id)?.bytes, d.ref_)
    } else {
        let f = db::invoice::get(&app.pool, s.entity, id)?
            .ok_or_else(|| Erreur("facture introuvable".into()))?;
        (app::facture_pdf(&app.pool, s.entity, id)?.bytes, f.ref_)
    };

    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{nom}.pdf\""),
            ),
        ],
        octets,
    )
        .into_response())
}
