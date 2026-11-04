//! Ecran du livre tenu : ce qui est **ecrit** dans llx_accounting_bookkeeping.
//!
//! Le grand livre « derive » (calcule depuis les documents) existait deja.
//! Celui-ci montre l'autre : les lignes reellement enregistrees, celles que
//! Dolibarr PHP lirait. Les deux doivent se ressembler — c'est justement ce
//! qu'on veut pouvoir comparer.

use std::collections::BTreeMap;

use axum::extract::State;
use maud::html;
use rust_decimal::Decimal;
use std::str::FromStr;

/// Les 500 dernieres lignes ecrites, avec le total debit = credit.
pub async fn page(State(app): State<crate::App>) -> crate::Res<maud::Markup> {
    let lignes = db::divers::lignes_brutes(
        &app.pool,
        crate::entite::courante(),
        "SELECT code_journal, DATE_FORMAT(doc_date, '%Y-%m-%d'), doc_ref, \
                numero_compte, COALESCE(label_operation, ''), \
                CAST(debit AS CHAR), CAST(credit AS CHAR) \
         FROM llx_accounting_bookkeeping \
         WHERE entity = :entity \
         ORDER BY doc_date DESC, piece_num DESC, rowid DESC LIMIT 500",
    )?;

    let champ = |l: &Vec<Option<String>>, i: usize| -> String {
        l.get(i).cloned().flatten().unwrap_or_default()
    };
    let nombre =
        |s: &str| Decimal::from_str(if s.is_empty() { "0" } else { s }).unwrap_or_default();

    let mut total_debit = Decimal::ZERO;
    let mut total_credit = Decimal::ZERO;
    let mut par_compte: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();
    for l in &lignes {
        let d = nombre(&champ(l, 5));
        let c = nombre(&champ(l, 6));
        total_debit += d;
        total_credit += c;
        let e = par_compte
            .entry(champ(l, 3))
            .or_insert((Decimal::ZERO, Decimal::ZERO));
        e.0 += d;
        e.1 += c;
    }
    let equilibre = total_debit == total_credit;

    Ok(crate::ui::page(
        "comptabilite",
        "livre-tenu",
        "Livre tenu",
        html! {
            h1 { "Livre tenu" }
            p .sous-titre {
                "Lignes réellement écrites dans "
                code { "llx_accounting_bookkeeping" }
                " — celles que Dolibarr PHP lit. À comparer avec le grand livre dérivé."
            }
            @if lignes.is_empty() {
                (crate::ui::vide(
                    "Aucune écriture enregistrée. Le livre se remplit quand une facture est passée en comptabilité.",
                    Some(("/comptabilite/grand-livre", "Voir le grand livre dérivé")),
                ))
            } @else {
                div .cartes {
                    (crate::ui::carte("Écritures", &lignes.len().to_string(), None, false))
                    (crate::ui::carte("Total débit", &crate::ui::euros(total_debit), None, false))
                    (crate::ui::carte("Total crédit", &crate::ui::euros(total_credit), None, false))
                    (crate::ui::carte(
                        "Équilibre",
                        if equilibre { "oui" } else { "NON" },
                        Some(if equilibre { "débit = crédit" } else { "le livre ne tient pas" }),
                        !equilibre,
                    ))
                }
                h2 { "Balance par compte" }
                table {
                    thead { tr { th { "Compte" } th .num { "Débit" } th .num { "Crédit" } th .num { "Solde" } } }
                    tbody {
                        @for (compte, (d, c)) in &par_compte {
                            tr {
                                td .ref { (compte) }
                                td .num { (crate::ui::euros(*d)) }
                                td .num { (crate::ui::euros(*c)) }
                                td .num { (crate::ui::euros(*d - *c)) }
                            }
                        }
                    }
                }
                h2 { "Dernières lignes" }
                div style="overflow-x:auto" {
                    table {
                        thead {
                            tr {
                                th { "Journal" } th { "Date" } th { "Pièce" }
                                th { "Compte" } th { "Libellé" } th .num { "Débit" } th .num { "Crédit" }
                            }
                        }
                        tbody {
                            @for l in &lignes {
                                tr {
                                    td { (champ(l, 0)) }
                                    td { (champ(l, 1)) }
                                    td .ref { (champ(l, 2)) }
                                    td .ref { (champ(l, 3)) }
                                    td { (champ(l, 4)) }
                                    td .num { (crate::ui::euros(nombre(&champ(l, 5)))) }
                                    td .num { (crate::ui::euros(nombre(&champ(l, 6)))) }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}
