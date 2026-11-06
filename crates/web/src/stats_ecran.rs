//! Ecrans de statistiques et d'echeancier.
//!
//! Deux pages faites pour etre montrees : une courbe de chiffre d'affaires par
//! mois en barres, les meilleurs clients et chantiers, et six mois de
//! tresorerie prevue.
//!
//! Pas de bibliotheque graphique : des barres en CSS. Une largeur en pourcentage
//! suffit a lire une tendance, et un SVG ou un canvas ajouterait une dependance
//! pour un resultat moins lisible.

use std::collections::HashMap;

use axum::extract::{Query, State};
use maud::{html, Markup};
use rust_decimal::Decimal;

use crate::{entite, ui, App, Res};

/// Une barre horizontale, proportionnelle a `valeur` sur `maximum`.
fn barre(valeur: Decimal, maximum: Decimal, couleur: &str) -> Markup {
    let part = if maximum > Decimal::ZERO {
        (valeur / maximum * Decimal::from(100)).round_dp(1)
    } else {
        Decimal::ZERO
    }
    .max(Decimal::ZERO)
    .min(Decimal::from(100));
    let style = format!(
        "width:{}%;height:10px;background:{couleur}",
        part.normalize()
    );
    html! {
        div style="background:#f0eeea;height:10px;margin-top:4px" {
            div style=(style) {}
        }
    }
}

/// Le maximum d'une serie, pour echelonner les barres.
fn maximum(valeurs: impl Iterator<Item = Decimal>) -> Decimal {
    valeurs.fold(Decimal::ZERO, |a, b| if b > a { b } else { a })
}

/// La page des statistiques.
pub async fn statistiques(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let entity = entite::courante();
    let aujourdhui = db::aujourdhui(&app.pool)?;
    // Douze mois par defaut : une annee se lit d'un coup d'oeil.
    let mois: usize = q
        .get("mois")
        .and_then(|s| s.parse().ok())
        .unwrap_or(12)
        .clamp(3, 36);

    let serie = db::stats::par_mois(&app.pool, entity, &aujourdhui, mois)?;
    let depuis = serie
        .first()
        .map(|m| format!("{}-01", m.mois))
        .unwrap_or_default();
    let clients = db::stats::top_clients(&app.pool, entity, &depuis, 8)?;
    let chantiers = db::stats::top_chantiers(&app.pool, entity, &depuis, 8)?;
    let encours = db::stats::encours_client(&app.pool, entity)?;

    let total_ventes: Decimal = serie.iter().map(|m| m.ventes).sum();
    let total_achats: Decimal = serie.iter().map(|m| m.achats).sum();
    let maxi = maximum(serie.iter().map(|m| m.ventes.max(m.achats)));
    let maxi_client = maximum(clients.iter().map(|c| c.montant));
    let maxi_chantier = maximum(chantiers.iter().map(|c| c.montant));
    let marge_brute = total_ventes - total_achats;

    Ok(ui::page(
        "comptabilite",
        "statistiques",
        "Statistiques",
        html! {
            h1 { (crate::langue::tr("Statistiques")) }
            p .sous-titre {
                (crate::langue::tr("Le chiffre d'affaires est net : les avoirs sont déduits. Une facture en brouillon ne compte pas."))
            }

            form method="get" action="/statistiques" {
                div .actions {
                    select name="mois" aria-label=(crate::langue::tr("Période")) {
                        option value="3" selected[mois == 3] { "3 mois" }
                        option value="6" selected[mois == 6] { "6 mois" }
                        option value="12" selected[mois == 12] { "12 mois" }
                        option value="24" selected[mois == 24] { "24 mois" }
                        option value="36" selected[mois == 36] { "36 mois" }
                    }
                    button type="submit" class="petit" { (crate::langue::tr("Recalculer")) }
                }
            }

            div .cartes {
                (ui::carte("Ventes", &ui::euros(total_ventes), Some("TTC, avoirs déduits"), false))
                (ui::carte("Achats", &ui::euros(total_achats), Some("TTC"), false))
                (ui::carte(
                    "Marge brute",
                    &ui::euros(marge_brute),
                    Some("ventes moins achats"),
                    marge_brute < Decimal::ZERO,
                ))
                (ui::carte("Encours client", &ui::euros(encours), Some("reste à encaisser"), encours > Decimal::ZERO))
            }

            h2 { (crate::langue::tr("Ventes et achats par mois")) }
            @if serie.is_empty() {
                (ui::vide("Aucune donnée sur la période.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Mois")) }
                            th .num { (crate::langue::tr("Ventes")) }
                            th { "" }
                            th .num { (crate::langue::tr("Achats")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for m in &serie {
                            tr {
                                td .ref { (m.mois.clone()) }
                                td .num { (ui::euros(m.ventes)) }
                                td style="width:32%" { (barre(m.ventes, maxi, "#8a5a2b")) }
                                td .num { (ui::euros(m.achats)) }
                                td style="width:32%" { (barre(m.achats, maxi, "#b6b0a7")) }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Meilleurs clients")) }
            @if clients.is_empty() {
                (ui::vide("Aucune facture validée sur la période.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Client")) }
                            th .num { (crate::langue::tr("Factures")) }
                            th .num { (crate::langue::tr("Total TTC")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for c in &clients {
                            tr {
                                td { (c.libelle.clone()) }
                                td .num { (c.nombre.to_string()) }
                                td .num { (ui::euros(c.montant)) }
                                td style="width:28%" { (barre(c.montant, maxi_client, "#2f6b45")) }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Meilleurs chantiers")) }
            @if chantiers.is_empty() {
                (ui::vide("Aucune facture rattachée à un chantier sur la période.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Chantier")) }
                            th .num { (crate::langue::tr("Factures")) }
                            th .num { (crate::langue::tr("Total TTC")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for c in &chantiers {
                            tr {
                                td { (c.libelle.clone()) }
                                td .num { (c.nombre.to_string()) }
                                td .num { (ui::euros(c.montant)) }
                                td style="width:28%" { (barre(c.montant, maxi_chantier, "#2f5d7c")) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// La page de l'echeancier.
pub async fn echeancier(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let entity = entite::courante();
    let aujourdhui = db::aujourdhui(&app.pool)?;
    let mois: usize = q
        .get("mois")
        .and_then(|s| s.parse().ok())
        .unwrap_or(6)
        .clamp(3, 24);

    let lignes = db::stats::echeancier(&app.pool, entity, &aujourdhui, mois)?;
    let total_enc: Decimal = lignes.iter().map(|e| e.encaissements).sum();
    let total_dec: Decimal = lignes.iter().map(|e| e.decaissements).sum();
    let maxi = maximum(lignes.iter().map(|e| e.encaissements.max(e.decaissements)));

    // Le cumul dit ce que la derniere ligne ne dit pas : la position de
    // tresorerie a la fin du mois.
    let mut cumul = Decimal::ZERO;
    let cumuls: Vec<Decimal> = lignes
        .iter()
        .map(|e| {
            cumul += e.solde();
            cumul
        })
        .collect();

    Ok(ui::page(
        "comptabilite",
        "echeancier",
        "Échéancier",
        html! {
            h1 { (crate::langue::tr("Échéancier")) }
            p .sous-titre {
                (crate::langue::tr("Ce qui doit rentrer et sortir, à l'échéance des pièces et non à leur date. Un client en retard apparaît sur le mois où il aurait dû payer."))
            }

            form method="get" action="/echeancier" {
                div .actions {
                    select name="mois" aria-label=(crate::langue::tr("Période")) {
                        option value="3" selected[mois == 3] { "3 mois" }
                        option value="6" selected[mois == 6] { "6 mois" }
                        option value="12" selected[mois == 12] { "12 mois" }
                    }
                    button type="submit" class="petit" { (crate::langue::tr("Recalculer")) }
                }
            }

            div .cartes {
                (ui::carte("Encaissements", &ui::euros(total_enc), Some("sur la période"), false))
                (ui::carte("Décaissements", &ui::euros(total_dec), Some("sur la période"), false))
                (ui::carte(
                    "Solde",
                    &ui::euros(total_enc - total_dec),
                    Some("encaissements moins décaissements"),
                    total_enc < total_dec,
                ))
            }

            table {
                thead {
                    tr {
                        th { (crate::langue::tr("Mois")) }
                        th .num { (crate::langue::tr("Encaissements")) }
                        th { "" }
                        th .num { (crate::langue::tr("Décaissements")) }
                        th { "" }
                        th .num { (crate::langue::tr("Solde")) }
                        th .num { (crate::langue::tr("Cumul")) }
                    }
                }
                tbody {
                    @for (i, e) in lignes.iter().enumerate() {
                        tr {
                            td .ref { (e.mois.clone()) }
                            td .num { (ui::euros(e.encaissements)) }
                            td style="width:20%" { (barre(e.encaissements, maxi, "#2f6b45")) }
                            td .num { (ui::euros(e.decaissements)) }
                            td style="width:20%" { (barre(e.decaissements, maxi, "#a33a2a")) }
                            td .num {
                                @if e.solde() < Decimal::ZERO {
                                    span.rouge { (ui::euros(e.solde())) }
                                } @else {
                                    (ui::euros(e.solde()))
                                }
                            }
                            td .num { (ui::euros(cumuls[i])) }
                        }
                    }
                }
            }

            p style="color:#78736c;font-size:13.5px;margin-top:16px" {
                (crate::langue::tr("Le cumul est la position de trésorerie à la fin du mois, si tout est encaissé et payé à l'échéance. Ce n'est pas un solde bancaire : il ne tient pas compte de ce qui était déjà en caisse."))
            }
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn le_maximum_ignore_les_valeurs_negatives() {
        assert_eq!(maximum([d("10"), d("-5"), d("3")].into_iter()), d("10"));
        assert_eq!(maximum(Vec::new().into_iter()), Decimal::ZERO);
    }

    #[test]
    fn une_barre_ne_depasse_jamais_la_largeur() {
        let s = barre(d("50"), d("100"), "#000").into_string();
        assert!(s.contains("width:50%"), "{s}");
        let s = barre(d("200"), d("100"), "#000").into_string();
        assert!(s.contains("width:100%"), "plafonnee : {s}");
    }

    #[test]
    fn une_barre_sur_un_maximum_nul_ne_divise_pas_par_zero() {
        let s = barre(d("10"), Decimal::ZERO, "#000").into_string();
        assert!(s.contains("width:0%"), "{s}");
    }
}
