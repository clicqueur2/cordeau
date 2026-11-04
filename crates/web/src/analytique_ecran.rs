//! Ecran de comptabilite analytique : les chantiers, et leur marge.
//!
//! Trois gestes : creer un axe, creer un compte (un chantier), affecter une
//! ligne du livre a un ou plusieurs comptes. Puis lire la marge.
//!
//! Le chiffre qui compte n'est pas la marge, c'est le **taux d'affectation**.
//! Une marge calculee sur 10 % des lignes n'est pas une marge, c'est un
//! echantillon : l'ecran l'affiche a cote, sans le cacher.

use std::collections::HashMap;
use std::str::FromStr;

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Form;
use db::analytique;
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// Lit « 3:70, 4:30 » en parts.
fn parts(brut: &str) -> Result<Vec<(i64, Decimal)>, String> {
    let mut out = Vec::new();
    for morceau in brut.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (compte, pct) = morceau
            .split_once(':')
            .ok_or_else(|| format!("« {morceau} » n'est pas de la forme compte:pourcentage"))?;
        let compte: i64 = compte
            .trim()
            .parse()
            .map_err(|_| format!("compte invalide : {compte}"))?;
        let pct =
            Decimal::from_str(pct.trim()).map_err(|_| format!("pourcentage invalide : {pct}"))?;
        out.push((compte, pct));
    }
    analytique::verifier_parts(&out)?;
    Ok(out)
}

/// La page : axes, comptes, marge, et ce qui n'est pas affecte.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let aujourdhui = db::aujourdhui(&app.pool)?;
    let annee: String = aujourdhui.chars().take(4).collect();
    let debut = q
        .get("debut")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{annee}-01-01"));
    let fin = q
        .get("fin")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{annee}-12-31"));

    let axes = analytique::axes(&app.pool, entity)?;
    let comptes = analytique::comptes(&app.pool, entity, None)?;
    let marges = analytique::marge(&app.pool, entity, &debut, &fin)?;
    let total = analytique::lignes_periode(&app.pool, entity, &debut, &fin)?;
    let orphelines = analytique::non_reparties(&app.pool, entity, &debut, &fin)?;
    let taux = if total > 0 {
        (100.0 * (total - orphelines) as f64 / total as f64).round() as i64
    } else {
        0
    };
    let ok = q.get("ok").map(|s| s.as_str());

    Ok(ui::page(
        "comptabilite",
        "analytique",
        "Comptabilité analytique",
        html! {
            h1 { "Comptabilité analytique" }
            p .sous-titre {
                "Un axe, des comptes — les chantiers —, et la part de chaque ligne d'écriture "
                "qui leur revient. La marge est la somme des contributions, produits moins charges."
            }

            @if ok == Some("axe") { (ui::flash("Axe créé.")) }
            @if ok == Some("compte") { (ui::flash("Compte analytique créé.")) }
            @if ok == Some("affecte") { (ui::flash("Ligne affectée.")) }

            h2 { "Axes et comptes" }
            @if axes.is_empty() {
                (ui::vide("Aucun axe analytique sur cette entité.", None))
            } @else {
                table {
                    thead { tr { th { "Axe" } th { "Code" } th { "Comptes" } } }
                    tbody {
                        @for a in &axes {
                            tr {
                                td { (a.libelle) }
                                td .ref { (a.code) }
                                td {
                                    @let dedans: Vec<&analytique::Compte> =
                                        comptes.iter().filter(|c| c.fk_axe == a.rowid).collect();
                                    @if dedans.is_empty() {
                                        span style="color:#78736c" { "aucun" }
                                    } @else {
                                        @for (i, c) in dedans.iter().enumerate() {
                                            @if i > 0 { " · " }
                                            span .ref { (c.code) } " " (c.libelle)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div .actions {
                form method="post" action="/comptabilite/analytique/axe" style="display:flex;gap:8px" {
                    input type="text" name="code" placeholder="CHANTIERS" size="12" required;
                    input type="text" name="libelle" placeholder="Chantiers" size="18" required;
                    button type="submit" class="petit" { "Créer l'axe" }
                }
            }
            @if !axes.is_empty() {
                div .actions {
                    form method="post" action="/comptabilite/analytique/compte" style="display:flex;gap:8px" {
                        select name="fk_axe" required {
                            @for a in &axes { option value=(a.rowid) { (a.libelle) } }
                        }
                        input type="text" name="code" placeholder="CH-2026-014" size="14" required;
                        input type="text" name="libelle" placeholder="Rue des Vignes" size="20" required;
                        button type="submit" class="petit" { "Créer le compte" }
                    }
                }
            }

            h2 { "Marge par chantier" }
            form method="get" action="/comptabilite/analytique" {
                div .actions {
                    "Du " input type="text" name="debut" value=(debut) size="12";
                    " au " input type="text" name="fin" value=(fin) size="12";
                    button type="submit" { "Recalculer" }
                }
            }

            div .cartes {
                (ui::carte(
                    "Lignes affectées",
                    &format!("{taux}\u{202f}%"),
                    Some(if total > 0 {
                        "le reste n'entre pas dans la marge"
                    } else {
                        "aucune écriture sur la période"
                    }),
                    total > 0 && taux < 80,
                ))
                (ui::carte(
                    "Marge totale",
                    &ui::euros(marges.iter().map(|m| m.montant).sum()),
                    Some("sur les lignes affectées"),
                    false,
                ))
            }

            @if marges.is_empty() {
                (ui::vide(
                    "Aucune ligne affectée sur cette période : la marge ne peut pas être calculée.",
                    None,
                ))
            } @else {
                table {
                    thead {
                        tr {
                            th { "Compte" }
                            th { "Chantier" }
                            th .num { "Lignes" }
                            th .num { "Marge" }
                        }
                    }
                    tbody {
                        @for m in &marges {
                            tr {
                                td .ref { (m.code) }
                                td { (m.libelle) }
                                td .num { (m.lignes.to_string()) }
                                td .num { (ui::euros(m.montant)) }
                            }
                        }
                    }
                }
            }

            h2 { "Affecter une ligne" }
            p style="color:#78736c;font-size:13.5px" {
                "Le numéro de ligne est le " code { "rowid" } " dans "
                code { "llx_accounting_bookkeeping" } ". Format : "
                code { "compte:pourcentage" } ", plusieurs parts séparées par des virgules "
                "— la somme doit valoir 100."
            }
            form method="post" action="/comptabilite/analytique/affecter" {
                div .actions {
                    input type="number" name="ligne" placeholder="Ligne" size="8" required;
                    input type="text" name="parts" placeholder="3:70, 4:30" size="24" required;
                    button type="submit" class="petit" { "Affecter" }
                }
            }
        },
    ))
}

/// Cree un axe.
pub async fn creer_axe(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let code = f.get("code").map(|s| s.trim()).unwrap_or("");
    let libelle = f.get("libelle").map(|s| s.trim()).unwrap_or("");
    analytique::creer_axe(&app.pool, entite::courante(), code, libelle)?;
    Ok(Redirect::to("/comptabilite/analytique?ok=axe"))
}

/// Cree un compte analytique.
pub async fn creer_compte(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let fk_axe = f
        .get("fk_axe")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("axe manquant".into()))?;
    let code = f.get("code").map(|s| s.trim()).unwrap_or("");
    let libelle = f.get("libelle").map(|s| s.trim()).unwrap_or("");
    analytique::creer_compte(&app.pool, entite::courante(), fk_axe, code, libelle)?;
    Ok(Redirect::to("/comptabilite/analytique?ok=compte"))
}

/// Affecte une ligne du livre a des comptes analytiques.
pub async fn affecter(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let ligne = f
        .get("ligne")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("ligne manquante ou invalide".into()))?;
    let brut = f.get("parts").map(|s| s.as_str()).unwrap_or("");
    let parts = parts(brut)?;

    analytique::repartir(&app.pool, entite::courante(), ligne, &parts)?;
    Ok(Redirect::to("/comptabilite/analytique?ok=affecte"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deux_parts_qui_font_cent_sont_acceptees() {
        let p = parts("3:70, 4:30").unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].0, 3);
        assert_eq!(p[0].1, Decimal::from(70));
        assert_eq!(p[1].0, 4);
    }

    #[test]
    fn une_seule_part_a_cent_est_acceptee() {
        assert_eq!(parts("7:100").unwrap().len(), 1);
    }

    #[test]
    fn un_total_different_de_cent_est_refuse() {
        assert!(parts("3:70, 4:20").is_err());
    }

    #[test]
    fn une_ecriture_mal_formee_est_refusee() {
        assert!(parts("3").is_err());
        assert!(parts("trois:100").is_err());
        assert!(parts("3:beaucoup").is_err());
        assert!(parts("").is_err());
    }

    #[test]
    fn les_espaces_et_virgules_en_trop_ne_genent_pas() {
        let p = parts(" 3 : 50 ,, 4 : 50 ").unwrap();
        assert_eq!(p.len(), 2);
    }
}
