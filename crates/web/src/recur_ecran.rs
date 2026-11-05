//! Ecran des recurrences : ce qui doit partir aujourd'hui, et le bouton pour le faire.
//!
//! Le moteur de dates est dans `recurrence`, la lecture des series dans
//! `db::recurrence`, la generation dans `db::recur_generation`. Cet ecran ne
//! fait qu'enchainer les trois — et **ne valide rien** : les factures sortent en
//! brouillon, a relire avant envoi.

use axum::extract::State;
use axum::response::Redirect;
use maud::html;

/// Ce qui est du, et depuis quand.
pub async fn page(State(app): State<crate::App>) -> crate::Res<maud::Markup> {
    let aujourd_hui = aujourd_hui();
    let dues = db::recurrence::a_emettre(&app.pool, crate::entite::courante(), &aujourd_hui, 200)?;

    Ok(crate::ui::page(
        "commerce",
        "recurrences",
        "Récurrences",
        html! {
            h1 {(crate::langue::tr("Récurrences")) }
            p .sous-titre {
                "Séries Dolibarr (llx_facture_rec) dont l'échéance est tombée. "
                "La génération produit un "
                strong { "brouillon" }
                " : rien n'est validé ni envoyé."
            }
            @if dues.is_empty() {
                (crate::ui::vide("Aucune échéance à émettre aujourd'hui.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Source")) } th {(crate::langue::tr("N° série")) } th {(crate::langue::tr("Échéance")) }
                            th .num {(crate::langue::tr("Déjà émise")) }
                        }
                    }
                    tbody {
                        @for d in &dues {
                            tr {
                                td { (d.source.clone()) }
                                td .ref { (d.id.to_string()) }
                                td { (d.regle.debut.clone()) }
                                td .num {
                                    @match &d.derniere_emission {
                                        Some(x) => (x.clone()),
                                        None => span style="color:#a33" { "non" },
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    form method="post" action="/recurrences/generer" style="display:inline" {
                        button type="submit" {(crate::langue::tr("Générer les brouillons")) }
                    }
                }
            }
            p style="color:#78736c;font-size:13px" {
                "Date du jour : " (aujourd_hui)
                ". Un seul document par série et par échéance : la clé "
                code {(crate::langue::tr("PA-RECUR-…")) }
                " empêche le doublon, même si le traitement passe deux fois."
            }
        },
    ))
}

/// Genere les brouillons dus et annonce le resultat.
pub async fn generer(State(app): State<crate::App>) -> crate::Res<Redirect> {
    let aujourd_hui = aujourd_hui();
    let faits = db::recur_generation::generer_tout(
        &app.pool,
        crate::entite::courante(),
        &aujourd_hui,
        200,
    )?;

    if faits.a_des_echecs() {
        // Le motif voyage dans l'adresse : l'ecran le montre, l'utilisateur
        // sait quoi corriger sans ouvrir de journal.
        let motif = faits
            .echecs
            .first()
            .map(|e| e.motif.clone())
            .unwrap_or_default();
        let motif: String = motif.chars().take(180).collect();
        Ok(Redirect::to(&format!(
            "/recurrences?ko={}",
            urlencode(&motif)
        )))
    } else if faits.creees.is_empty() {
        Ok(Redirect::to("/recurrences?ok=rien"))
    } else {
        let n = faits.creees.len();
        Ok(Redirect::to(&format!("/recurrences?ok={n}")))
    }
}

/// Encode un texte pour une adresse : seuls les caracteres surs passent tels
/// quels, le reste devient `%XX`.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for octet in s.as_bytes() {
        match octet {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b' ' => {
                if *octet == b' ' {
                    out.push('+');
                } else {
                    out.push(*octet as char);
                }
            }
            _ => out.push_str(&format!("%{octet:02X}")),
        }
    }
    out
}

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
