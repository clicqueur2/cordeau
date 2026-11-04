//! Ecran des devises : ce qui est fige, et de quoi figer.
//!
//! Une seule idee a faire passer : le taux est **fige sur la piece**. Une
//! facture de mars garde le taux de mars, quoi que fasse le marche ensuite.
//! L'ecran le dit, et refuse de re-evaluer une piece validee.

use std::collections::HashMap;
use std::str::FromStr;

use axum::extract::State;
use axum::response::Redirect;
use axum::Form;
use db::multidevise::{self, Document};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La page : les devises connues, et les pieces qui en portent une.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let devises = multidevise::devises(&app.pool)?;
    let factures = multidevise::portant_une_devise(&app.pool, entity, Document::Facture, 50)?;
    let devis = multidevise::portant_une_devise(&app.pool, entity, Document::Propal, 50)?;

    Ok(ui::page(
        "factures",
        "devises",
        "Devises",
        html! {
            h1 { "Devises" }
            p .sous-titre {
                "Le taux est figé sur la pièce, jamais dans une table de marché : "
                "une facture de mars 2026 garde le taux de mars. Les montants de base restent "
                "en monnaie de la société ; la devise est une traduction."
            }

            h2 { "Figer un taux" }
            form method="post" action="/devises/figer" {
                div .actions {
                    select name="document" aria-label="Type de document" {
                        option value="facture" { "Facture" }
                        option value="devis" { "Devis" }
                        option value="commande" { "Commande" }
                    }
                    input type="number" name="id" placeholder="N° du document" size="8" required;
                    select name="code" aria-label="Devise" {
                        @for d in &devises {
                            option value=(d.code.as_str()) { (d.code) " — " (d.libelle) }
                        }
                    }
                    input type="text" name="taux" placeholder="1,08" size="8" required;
                    button type="submit" class="petit" { "Figer" }
                }
                p style="color:#78736c;font-size:13.5px;margin:0" {
                    "Le taux est le nombre d'unités de devise pour 1 €. "
                    "EUR 100 à 1,08 CHF donnent 108,00 CHF. "
                    "Seule une facture en brouillon se réévalue."
                }
            }

            h2 { "Factures en devise" }
            @if factures.is_empty() {
                (ui::vide("Aucune facture ne porte de devise.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { "N°" }
                            th { "Réf." }
                            th { "Devise" }
                            th .num { "Taux" }
                            th .num { "TTC devise" }
                            th { "" }
                        }
                    }
                    tbody {
                        @for (id, reference, t) in &factures {
                            tr {
                                td .ref { (id.to_string()) }
                                td .ref { a href={ "/factures/" (id) } { (reference) } }
                                td { (t.code) }
                                td .num { (t.tx.normalize().to_string()) }
                                td .num { (montant(t.total_ttc, &t.code)) }
                                td { }
                            }
                        }
                    }
                }
            }

            h2 { "Devis en devise" }
            @if devis.is_empty() {
                (ui::vide("Aucun devis ne porte de devise.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { "N°" }
                            th { "Réf." }
                            th { "Devise" }
                            th .num { "Taux" }
                            th .num { "TTC devise" }
                        }
                    }
                    tbody {
                        @for (id, reference, t) in &devis {
                            tr {
                                td .ref { (id.to_string()) }
                                td .ref { a href={ "/devis/" (id) } { (reference) } }
                                td { (t.code) }
                                td .num { (t.tx.normalize().to_string()) }
                                td .num { (montant(t.total_ttc, &t.code)) }
                            }
                        }
                    }
                }
            }

            h2 { "Devises connues" }
            table {
                thead { tr { th { "Code" } th { "Libellé" } th .num { "Décimales" } } }
                tbody {
                    @for d in &devises {
                        tr {
                            td .ref { (d.code) }
                            td { (d.libelle) }
                            td .num { (d.decimales.to_string()) }
                        }
                    }
                }
            }
        },
    ))
}

/// Un montant en devise, sans symbole invente : le code devant suffit.
fn montant(v: Decimal, code: &str) -> String {
    format!("{} {code}", v.round_dp(2).normalize())
}

/// Fige le taux demande.
pub async fn figer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let doc = f
        .get("document")
        .and_then(|s| Document::depuis_nom(s))
        .ok_or_else(|| Erreur("type de document inconnu".into()))?;
    let id = f
        .get("id")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("numéro de document invalide".into()))?;
    let code = f.get("code").map(|s| s.trim()).unwrap_or("");
    // Le taux se saisit en français : virgule acceptée.
    let brut = f
        .get("taux")
        .map(|s| s.trim().replace(',', "."))
        .unwrap_or_default();
    let taux = Decimal::from_str(&brut).map_err(|_| Erreur(format!("taux illisible : {brut}")))?;

    multidevise::figer(&app.pool, entite::courante(), doc, id, code, taux, false)?;
    Ok(Redirect::to("/devises"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_montant_en_devise_porte_son_code() {
        assert_eq!(
            montant(Decimal::from_str("129.60").unwrap(), "CHF"),
            "129.6 CHF"
        );
        assert_eq!(montant(Decimal::from_str("108").unwrap(), "USD"), "108 USD");
    }
}
