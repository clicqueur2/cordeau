//! Ecran de lettrage : les comptes ouverts, et le rapprochement en un geste.
//!
//! On montre d'abord ce qui reste a rapprocher, compte par compte. Puis on
//! lettre, et on dit exactement ce qui a ete apparié et ce qui reste ouvert.
//!
//! Un lettrage FIFO n'est pas un rapprochement intelligent : c'est ecrit sur
//! l'ecran, parce qu'un comptable doit savoir que deux reglements de meme
//! montant se lettrent dans l'ordre des dates et pas selon la bonne facture.

use axum::extract::{Path, State};
use maud::html;

use crate::{entite, ui, App, Res};

/// La page : ce qui est ouvert, et le dernier lettrage fait.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let comptes = db::lettrage::comptes_ouverts(&app.pool, entity)?;

    Ok(ui::page(
        "comptabilite",
        "lettrage",
        "Lettrage",
        html! {
            h1 {(crate::langue::tr("Lettrage")) }
            p .sous-titre {
                "Rapprocher la créance de son encaissement. Le calcul est FIFO : le débit le plus "
                "ancien mange le crédit le plus ancien. Ce n'est pas un rapprochement intelligent — "
                "deux règlements du même montant se lettrent dans l'ordre des dates."
            }

            @if comptes.is_empty() {
                (ui::vide(
                    "Aucun compte ouvert : tout ce qui est en base est rapproché.",
                    Some(("/comptabilite/livre-tenu", "Voir le livre tenu")),
                ))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Compte")) }
                            th .num {(crate::langue::tr("Solde non lettré")) }
                            th .num {(crate::langue::tr("Lignes")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for (compte, solde, n) in &comptes {
                            tr {
                                td .ref { (compte) }
                                td .num { (ui::euros(*solde)) }
                                td .num { (n.to_string()) }
                                td {
                                    form method="post" action={ "/comptabilite/lettrage/" (compte) } {
                                        button type="submit" class="petit" {(crate::langue::tr("Lettrer")) }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            p style="color:#78736c;font-size:13.5px;margin-top:16px" {
                "Seules les lignes non lettrées sont prises. Rejouer un lettrage ne défait pas "
                "le précédent : il ne travaille que sur le reste ouvert."
            }
        },
    ))
}

/// Lettre un compte et rend le compte rendu.
pub async fn lettrer(State(app): State<App>, Path(compte): Path<String>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let bilan = db::lettrage::lettrer(&app.pool, entity, &compte)?;

    Ok(ui::page(
        "comptabilite",
        "lettrage",
        "Lettrage",
        html! {
            h1 { "Lettrage du compte " (compte.clone()) }
            p .sous-titre {
                (bilan.paires.len().to_string()) " paire(s) rapprochée(s), "
                (bilan.restes.len().to_string()) " ligne(s) encore ouverte(s)."
            }

            div .cartes {
                (ui::carte(
                    "Rapproché",
                    &ui::euros(bilan.total()),
                    Some("montant apparié"),
                    false,
                ))
                (ui::carte(
                    "Reste ouvert",
                    &ui::euros(bilan.reste()),
                    Some("à encaisser ou à payer"),
                    bilan.reste() > rust_decimal::Decimal::ZERO,
                ))
            }

            @if bilan.paires.is_empty() {
                (ui::vide(
                    "Aucune paire : il n'y a pas à la fois un débit et un crédit à rapprocher sur ce compte.",
                    None,
                ))
            } @else {
                h2 {(crate::langue::tr("Paires")) }
                table {
                    thead {
                        tr {
                            th .num {(crate::langue::tr("Débit (ligne)")) }
                            th .num {(crate::langue::tr("Crédit (ligne)")) }
                            th .num {(crate::langue::tr("Montant")) }
                        }
                    }
                    tbody {
                        @for p in &bilan.paires {
                            tr {
                                td .num { (p.debit.to_string()) }
                                td .num { (p.credit.to_string()) }
                                td .num { (ui::euros(p.montant)) }
                            }
                        }
                    }
                }
            }

            @if !bilan.restes.is_empty() {
                h2 {(crate::langue::tr("Ce qui reste ouvert")) }
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Ligne")) }
                            th {(crate::langue::tr("Date")) }
                            th {(crate::langue::tr("Libellé")) }
                            th .num {(crate::langue::tr("Débit")) }
                            th .num {(crate::langue::tr("Crédit")) }
                        }
                    }
                    tbody {
                        @for l in &bilan.restes {
                            tr {
                                td .num { (l.rowid.to_string()) }
                                td .ref { (l.date.clone()) }
                                td { (l.libelle.clone()) }
                                td .num {
                                    @if l.debit > rust_decimal::Decimal::ZERO {
                                        (ui::euros(l.debit))
                                    }
                                }
                                td .num {
                                    @if l.credit > rust_decimal::Decimal::ZERO {
                                        (ui::euros(l.credit))
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div .actions {
                a .bouton href="/comptabilite/lettrage" {(crate::langue::tr("Retour au lettrage")) }
            }
        },
    ))
}
