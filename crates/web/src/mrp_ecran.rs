//! Ecran MRP : la demande a livrer, le stock, et ce qu'il faut lancer.
//!
//! Trois chiffres par article, et une evidence : **ce qui est en stock ne
//! s'achete pas deux fois**. Le reste est une hypothese de calcul — lot, delai
//! — que l'on saisit parce que la fiche produit du schema du coeur ne les porte
//! pas. L'ecran le dit en clair plutot que de laisser croire a une donnee.

use std::collections::HashMap;
use std::str::FromStr;

use axum::extract::{Query, State};
use maud::html;
use mrp::Proposition;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Res};

/// Lit un decimal saisi en francais (virgule acceptee).
fn dec(v: Option<&String>, defaut: &str) -> Decimal {
    v.map(|s| s.trim().replace(',', "."))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| defaut.to_string())
        .parse::<Decimal>()
        .unwrap_or_else(|_| Decimal::from_str(defaut).unwrap_or_default())
}

/// La page.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let aujourdhui = db::aujourdhui(&app.pool)?;

    // Les hypotheses de calcul, saisies par l'utilisateur.
    let lot = dec(q.get("lot"), "0");
    let minimum = dec(q.get("minimum"), "0");
    let delai: i64 = q
        .get("delai")
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    // Date de besoin par defaut : fin du mois courant, et non « aujourd'hui »,
    // qui ne laisserait aucun delai.
    let date_besoin = q
        .get("date")
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() == 10)
        .unwrap_or_else(|| aujourdhui.clone());

    let articles = db::mrp::articles(&app.pool, entity, lot, minimum, delai)?;
    let propositions: Vec<Proposition> = articles
        .iter()
        .map(|a| mrp::proposer(a, &date_besoin, &aujourdhui))
        .collect();
    let a_lancer: Vec<&Proposition> = propositions
        .iter()
        .filter(|p| p.quantite > Decimal::ZERO)
        .collect();
    let en_retard = a_lancer.iter().filter(|p| p.en_retard).count();

    Ok(ui::page(
        "produits",
        "mrp",
        "Besoins et ordres proposés",
        html! {
            h1 { "Besoins et ordres proposés" }
            p .sous-titre {
                "Demande = lignes de commandes clients validées, non expédiées. "
                "Besoin net = demande − stock. Ce qui est en stock ne s'achète pas deux fois."
            }

            form method="get" action="/mrp" {
                table {
                    tbody {
                        tr {
                            th { "Lot d'achat" }
                            td { input type="text" name="lot" value=(lot.normalize().to_string()) size="6"; }
                            th { "Minimum" }
                            td { input type="text" name="minimum" value=(minimum.normalize().to_string()) size="6"; }
                            th { "Délai (jours)" }
                            td { input type="number" name="delai" value=(delai.to_string()) size="4"; }
                            th { "Date de besoin" }
                            td { input type="date" name="date" value=(date_besoin) ; }
                        }
                    }
                }
                div .actions {
                    button type="submit" { "Recalculer" }
                }
            }

            div .cartes {
                (ui::carte(
                    "Articles à lancer",
                    &a_lancer.len().to_string(),
                    Some("quantité proposée non nulle"),
                    false,
                ))
                (ui::carte(
                    "En retard",
                    &en_retard.to_string(),
                    Some("date de lancement déjà passée"),
                    en_retard > 0,
                ))
            }

            @if articles.is_empty() {
                (ui::vide(
                    "Aucune demande à livrer : aucune commande client validée en attente.",
                    Some(("/commandes", "Voir les commandes")),
                ))
            } @else {
                table {
                    thead {
                        tr {
                            th { "Produit" }
                            th .num { "Demande" }
                            th .num { "Stock" }
                            th .num { "Besoin net" }
                            th .num { "Proposé" }
                            th { "Lancer le" }
                        }
                    }
                    tbody {
                        @for p in &propositions {
                            tr {
                                td { (p.reference.clone()) }
                                td .num { (p.besoin_brut.normalize().to_string()) }
                                td .num { (p.stock.normalize().to_string()) }
                                td .num { (p.besoin_net.normalize().to_string()) }
                                td .num {
                                    @if p.quantite > Decimal::ZERO {
                                        strong { (p.quantite.normalize().to_string()) }
                                    } @else {
                                        span style="color:#78736c" { "—" }
                                    }
                                }
                                td {
                                    @if p.quantite == Decimal::ZERO {
                                        span style="color:#78736c" { "couvert" }
                                    } @else if p.en_retard {
                                        span .pastille.p-retard { (p.date_lancement.clone()) }
                                    } @else {
                                        span .pastille.p-validee { (p.date_lancement.clone()) }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            p style="color:#78736c;font-size:13.5px;margin-top:16px" {
                "Ce que le calcul ne déduit pas encore : les "
                strong { "entrées attendues" }
                " (commandes fournisseurs en cours, ordres de fabrication). Elles sont donc "
                "comptées comme nulles, et le besoin net est majoré d'autant. Le lot, le minimum "
                "et le délai sont des hypothèses saisies ici : la fiche produit du schéma du cœur "
                "ne les porte pas."
            }
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_decimal_francais_se_lit() {
        assert_eq!(
            dec(Some(&"1,5".to_string()), "0"),
            Decimal::from_str("1.5").unwrap()
        );
        assert_eq!(dec(Some(&"25".to_string()), "0"), Decimal::from(25));
    }

    #[test]
    fn une_saisie_vide_ou_illisible_prend_le_defaut() {
        assert_eq!(dec(None, "2"), Decimal::from(2));
        assert_eq!(dec(Some(&"".to_string()), "2"), Decimal::from(2));
        assert_eq!(dec(Some(&"abc".to_string()), "2"), Decimal::from(2));
    }
}
