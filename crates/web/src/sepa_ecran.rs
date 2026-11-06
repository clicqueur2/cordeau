//! Ecran SEPA : les factures fournisseurs a payer, et le fichier de virement.
//!
//! Deux gestes : on regarde ce qui est du, on telecharge le `pain.001` que la
//! banque lira. Les factures sans IBAN sont signalees : une seule suffit a
//! faire rejeter le lot entier, autant le dire avant.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use maud::html;

/// Date du jour au format `AAAA-MM-JJ`, sans dependance de calendrier.
fn aujourd_hui() -> String {
    let secondes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let jours = (secondes / 86_400) as i64;
    let z = jours + 719_468;
    let ere = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - ere * 146_097;
    let annee_ere = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let annee = annee_ere + ere * 400;
    let jour_annee = doe - (365 * annee_ere + annee_ere / 4 - annee_ere / 100);
    let mp = (5 * jour_annee + 2) / 153;
    let jour = jour_annee - (153 * mp + 2) / 5 + 1;
    let mois = mp + if mp < 10 { 3 } else { -9 };
    let annee = annee + i64::from(mois <= 2);
    format!("{annee:04}-{mois:02}-{jour:02}")
}

/// Liste des factures a payer et bouton de telechargement.
pub async fn page(State(app): State<crate::App>) -> crate::Res<maud::Markup> {
    let factures =
        db::sepa::factures_fournisseurs_a_payer(&app.pool, crate::entite::courante(), 500)?;
    let total: i64 = factures.iter().map(|f| f.montant_centimes).sum();
    let sans_iban = factures.iter().filter(|f| f.iban.is_none()).count();

    Ok(crate::ui::page(
        "achats",
        "sepa",
        "Virements SEPA",
        html! {
            h1 {(crate::langue::tr("Virements SEPA")) }
            p .sous-titre {
                (factures.len().to_string()) " facture(s) fournisseur à payer, "
                (crate::ui::euros(rust_decimal::Decimal::new(total, 2)))
                " au total. Le fichier produit est un pain.001.001.03."
            }
            @if sans_iban > 0 {
                p .erreur {
                    (sans_iban.to_string())
                    " facture(s) sans IBAN : la banque rejetterait le lot entier. "
                    "Renseignez le RIB du fournisseur avant de générer."
                }
            }
            @if factures.is_empty() {
                (crate::ui::vide("Aucune facture fournisseur à payer.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Fournisseur")) } th {(crate::langue::tr("Échéance")) }
                            th {(crate::langue::tr("IBAN")) } th .num {(crate::langue::tr("Reste à payer")) }
                        }
                    }
                    tbody {
                        @for f in &factures {
                            tr {
                                td .ref { (f.reference()) }
                                td { (f.fournisseur) }
                                td { (f.date_echeance.clone().unwrap_or_default()) }
                                td {
                                    @match &f.iban {
                                        Some(i) => (i),
                                        None => span class="rouge" { "manquant" },
                                    }
                                }
                                td .num {
                                    (crate::ui::euros(rust_decimal::Decimal::new(f.montant_centimes, 2)))
                                }
                            }
                        }
                    }
                }
            }
            div .actions {
                a .bouton href="/achats/sepa/fichier" {(crate::langue::tr("Télécharger le fichier SEPA")) }
            }
        },
    ))
}

/// Le fichier XML, en piece jointe.
pub async fn fichier(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> crate::Res<Response> {
    let execution = q.get("date").cloned().unwrap_or_else(aujourd_hui);

    let factures =
        db::sepa::factures_fournisseurs_a_payer(&app.pool, crate::entite::courante(), 500)?;
    if factures.is_empty() {
        return Err(crate::Erreur("aucune facture fournisseur a payer".into()));
    }

    let lire = |cle: &str| {
        db::company::get_const(&app.pool, cle)
            .ok()
            .flatten()
            .unwrap_or_default()
    };
    let emetteur = sepa::Emetteur {
        nom: lire("MAIN_INFO_SOCIETE_NOM"),
        iban: lire("DOLIBARR_RS_IBAN"),
        bic: lire("DOLIBARR_RS_BIC"),
    };

    let lot: Vec<sepa::Virement> = factures
        .iter()
        .filter_map(|f| {
            let iban = f.iban.clone()?;
            Some(sepa::Virement {
                nom: f.fournisseur.clone(),
                iban,
                bic: f.bic.clone().unwrap_or_else(|| emetteur.bic.clone()),
                centimes: f.montant_centimes,
                motif: format!("Facture {}", f.reference()),
                reference: f.reference().to_string(),
            })
        })
        .collect();

    let xml = sepa::virement_xml(&emetteur, &execution, &lot)
        .map_err(|e| crate::Erreur(e.to_string()))?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"virement-sepa.xml\"",
            ),
        ],
        xml,
    )
        .into_response())
}
