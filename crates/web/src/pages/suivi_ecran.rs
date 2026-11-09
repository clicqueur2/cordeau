//! Suivi : reglements, avoirs, contrats, journal.
//!
//! Quatre lectures transverses. Aucune n'ecrit : ce sont les ecrans qu'on
//! ouvre pour comprendre d'ou vient un solde ou pourquoi une piece existe.

use axum::extract::State;
use maud::{html, Markup};

use crate::Res;

/// Reglements : liste des encaissements, toutes factures confondues.
pub async fn reglements(State(app): State<crate::App>) -> Res<Markup> {
    let liste = db::divers::reglements(&app.pool, crate::entite::courante(), 200)?;
    Ok(crate::ui::page(
        "factures",
        "paiements",
        "Reglements",
        html! {
            h1 {(crate::langue::tr("Saisie des reglements")) }
            p .sous-titre {(crate::langue::tr("Tous les encaissements enregistres. Un reglement se saisit depuis la fiche de la facture concerne."))
            }
            @if liste.is_empty() {
                (crate::ui::vide("Aucun reglement.", Some(("/factures", "Voir les factures"))))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Ref.")) } th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Tiers")) } th {(crate::langue::tr("Factures")) } th {(crate::langue::tr("Mode")) } th .num {(crate::langue::tr("Montant")) } } }
                    tbody {
                        @for r in &liste {
                            tr {
                                td .ref { (r.ref_) }
                                td .ref { (r.date.clone().unwrap_or_default()) }
                                td { (r.tiers.clone().unwrap_or_default()) }
                                td .ref { (r.factures.clone().unwrap_or_default()) }
                                td { (r.mode.clone().unwrap_or_default()) }
                                td .num { (crate::ui::euros(r.montant)) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Avoirs : factures de type 2.
pub async fn avoirs(State(app): State<crate::App>) -> Res<Markup> {
    let liste: Vec<_> = db::invoice::list(&app.pool, crate::entite::courante(), 500, 0)?
        .into_iter()
        .filter(|f| f.type_ == db::invoice::TYPE_AVOIR)
        .collect();
    Ok(crate::ui::page(
        "factures",
        "avoirs",
        "Avoirs",
        html! {
            h1 {(crate::langue::tr("Avoirs")) }
            p .sous-titre {(crate::langue::tr("Les avoirs annulent une facture : on ne supprime jamais une piece comptable.")) }
            @if liste.is_empty() {
                (crate::ui::vide("Aucun avoir.", Some(("/factures", "Voir les factures"))))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Ref.")) } th {(crate::langue::tr("Tiers")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("TTC")) } th { "" } } }
                    tbody {
                        @for f in &liste {
                            tr {
                                td .ref { a href={ "/factures/" (f.rowid) } { (f.ref_) } }
                                td { (crate::nom_tiers(&app.pool, f.fk_soc)?) }
                                td .ref { (f.datef.clone().unwrap_or_default()) }
                                td .num { (crate::ui::euros(f.total_ttc.unwrap_or_default())) }
                                td { a .bouton.petit.secondaire href={ "/factures/" (f.rowid) } {(crate::langue::tr("Ouvrir")) } }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Journal des erreurs : les dernieres lignes ecrites par le serveur.
pub async fn journal(State(_app): State<crate::App>) -> Res<Markup> {
    let lignes = crate::journal_erreurs::dernieres(200);
    let total = crate::journal_erreurs::compter();
    let chemin = crate::journal_erreurs::fichier();
    Ok(crate::ui::page(
        "parametres",
        "journal",
        "Journal des erreurs",
        html! {
            h1 {(crate::langue::tr("Journal des erreurs")) }
            p .sous-titre {
                (total) " ligne(s) - fichier " code { (chemin.display().to_string()) }
            }
            @if lignes.is_empty() {
                (crate::ui::vide("Aucune erreur enregistree.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Chemin")) } th {(crate::langue::tr("Message")) } } }
                    tbody {
                        @for l in lignes.iter().rev() {
                            @let morceaux: Vec<&str> = l.splitn(3, " | ").collect();
                            tr {
                                td .ref { (morceaux.first().copied().unwrap_or("")) }
                                td .ref { (morceaux.get(1).copied().unwrap_or("")) }
                                td { (morceaux.get(2).copied().unwrap_or(l.as_str())) }
                            }
                        }
                    }
                }
            }
        },
    ))
}
