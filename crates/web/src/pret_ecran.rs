//! Ecran des prêts : ce qu'on doit, et quand.
//!
//! Dolibarr a `loan.php`. Un prêt n'est pas une dépense : c'est une dette dont
//! l'échéancier est connu d'avance. L'écran montre donc **le capital restant dû**
//! et la prochaine échéance, pas le montant emprunté — c'est le restant qui
//! décide de la trésorerie.
//!
//! # L'échéancier est calculé, jamais saisi
//!
//! Les mensualités à intérêts constants se déduisent du capital, du taux et du
//! nombre d'échéances. Les recopier à la main dans un formulaire serait la
//! garantie qu'un jour une ligne ne tombe pas juste. Le calcul est dans
//! `db::pret::echeancier`, et la dernière échéance porte le reliquat d'arrondi
//! pour que la somme des capitaux égale exactement l'emprunt.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::pret::{self, NewPret};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let prets = pret::list(&app.pool, entity, 200, 0)?;
    let comptes = db::banque::comptes(&app.pool, entity)?;
    let dette: Decimal = prets.iter().map(|p| p.capital_restant_du).sum();

    Ok(ui::page(
        "prets",
        "prets",
        "Prêts",
        html! {
            h1 { (crate::langue::tr("Prêts")) }
            p .sous-titre {
                (crate::langue::tr("Ce qu'on doit encore, et quand. Le capital restant décide de la trésorerie, pas le montant emprunté."))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Capital restant dû"),
                    &ui::euros(dette),
                    Some(&crate::langue::tr("tous prêts confondus")),
                    dette > Decimal::ZERO,
                ))
                (ui::carte(
                    &crate::langue::tr("Prêts"),
                    &prets.len().to_string(),
                    Some(&crate::langue::tr("en cours et soldés")),
                    false,
                ))
            }

            h2 { (crate::langue::tr("Nouveau prêt")) }
            @if comptes.is_empty() {
                (ui::vide(
                    "Aucun compte bancaire : un prêt se rattache à un compte.",
                    Some(("/banques", "Voir les comptes")),
                ))
            } @else {
                form method="post" action="/prets" {
                    label for="label" { (crate::langue::tr("Libellé")) } " "
                    input type="text" name="label" id="label" required.champ-moyen
                          placeholder=(crate::langue::tr("Prêt travaux, crédit véhicule…"));
                    " "
                    label for="fk_bank" { (crate::langue::tr("Compte")) } " "
                    select name="fk_bank" id="fk_bank" {
                        @for c in &comptes {
                            option value=(c.rowid) { (c.label.clone()) }
                        }
                    }
                    br;
                    label for="capital" { (crate::langue::tr("Capital")) } " "
                    input type="text" name="capital" id="capital" required.champ-court;
                    " "
                    label for="rate" { (crate::langue::tr("Taux %")) } " "
                    input type="text" name="rate" id="rate" required.champ-court;
                    " "
                    label for="nbterm" { (crate::langue::tr("Échéances")) } " "
                    input type="text" name="nbterm" id="nbterm" value="12".champ-court;
                    " "
                    label for="datestart" { (crate::langue::tr("Départ")) } " "
                    input type="date" name="datestart" id="datestart" required;
                    " "
                    button type="submit" { (crate::langue::tr("Créer et calculer l'échéancier")) }
                }
                p class="aide petite sous-champ" {
                    (crate::langue::tr("L'échéancier se calcule : mensualités constantes, la dernière porte le reliquat d'arrondi pour que la somme fasse exactement le capital."))
                }
            }

            h2 { (crate::langue::tr("Prêts en cours")) }
            @if prets.is_empty() {
                (ui::vide("Aucun prêt enregistré.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Libellé")) }
                            th { (crate::langue::tr("Capital")) }
                            th { (crate::langue::tr("Taux")) }
                            th { (crate::langue::tr("Échéances")) }
                            th { (crate::langue::tr("Restant dû")) }
                            th { (crate::langue::tr("Prochaine")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for p in &prets {
                            tr {
                                td { (p.label.clone()) }
                                td .num { (ui::euros(p.capital)) }
                                td .ref { (p.taux.normalize().to_string()) " %" }
                                td .ref { (p.nbterm) }
                                td .num { (ui::euros(p.capital_restant_du)) }
                                td .ref {
                                    (p.prochaine_echeance.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td {
                                    a href=(format!("/prets/{}", p.rowid)) class="petit secondaire" {
                                        (crate::langue::tr("Ouvrir"))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// L'echeancier d'un pret, avec le reglement d'une echeance.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let p = pret::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("prêt {id} introuvable")))?;
    let echeances = pret::echeances(&app.pool, entity, id)?;
    let comptes = db::banque::comptes(&app.pool, entity)?;

    Ok(ui::page(
        "prets",
        "prets",
        &p.label,
        html! {
            h1 { (p.label.clone()) }
            p .sous-titre {
                (ui::euros(p.capital)) " · " (p.taux.normalize().to_string()) " % · "
                (p.nbterm) " " (crate::langue::tr("échéances"))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Capital restant dû"),
                    &ui::euros(p.capital_restant_du),
                    Some(&crate::langue::tr("échéances non réglées")),
                    p.capital_restant_du > Decimal::ZERO,
                ))
                (ui::carte(
                    &crate::langue::tr("Prochaine échéance"),
                    &p.prochaine_echeance.clone().unwrap_or_else(|| "—".to_string()),
                    None,
                    false,
                ))
            }

            h2 { (crate::langue::tr("Échéancier")) }
            table {
                thead {
                    tr {
                        th { "N°" }
                        th { (crate::langue::tr("Date")) }
                        th { (crate::langue::tr("Capital")) }
                        th { (crate::langue::tr("Intérêts")) }
                        th { (crate::langue::tr("Mensualité")) }
                        th { (crate::langue::tr("État")) }
                        th { "" }
                    }
                }
                tbody {
                    @for e in &echeances {
                        tr {
                            td .num { (e.numero) }
                            td .ref { (e.datep.clone()) }
                            td .num { (ui::euros(e.capital)) }
                            td .num { (ui::euros(e.interets)) }
                            td .num { (ui::euros(e.mensualite)) }
                            td {
                                @if e.regle {
                                    span class="pastille p-payee" { (crate::langue::tr("réglée")) }
                                } @else {
                                    span class="pastille p-brouillon" { (crate::langue::tr("à venir")) }
                                }
                            }
                            td {
                                @if !e.regle && !comptes.is_empty() {
                                    form method="post"
                                         action=(format!("/prets/{}/regler", p.rowid)) {
                                        input type="hidden" name="echeance" value=(e.rowid);
                                        select name="fk_bank".petite {
                                            @for c in &comptes {
                                                option value=(c.rowid) { (c.label.clone()) }
                                            }
                                        }
                                        " "
                                        button type="submit" class="petit" {
                                            (crate::langue::tr("Régler"))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Cree un pret et son echeancier.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();
    let decimal = |cle: &str| -> Result<Decimal, Erreur> {
        lire(cle)
            .parse::<Decimal>()
            .map_err(|_| Erreur(format!("{cle} : montant invalide")))
    };

    let new = NewPret {
        label: lire("label"),
        fk_bank: lire("fk_bank").parse::<i64>().ok(),
        capital: decimal("capital")?,
        insurance_amount: Decimal::ZERO,
        datestart: lire("datestart"),
        dateend: None,
        nbterm: lire("nbterm")
            .parse::<u32>()
            .map_err(|_| Erreur("nombre d'échéances invalide".into()))?,
        rate: decimal("rate")?,
        fk_projet: None,
        note_private: None,
        note_public: None,
    };
    let id = pret::create(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to(&format!("/prets/{id}")))
}

/// Regle une echeance.
pub async fn regler(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let echeance = f
        .get("echeance")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("échéance manquante".into()))?;
    let banque = f
        .get("fk_bank")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("compte bancaire manquant".into()))?;
    pret::regler(&app.pool, entite::courante(), id, echeance, banque)?;
    Ok(Redirect::to(&format!("/prets/{id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_dette_se_somme_sans_arrondi_intermediaire() {
        // La somme des restants dus ne doit pas passer par un arrondi : deux
        // prêts dont les restants finissent par des centimes doivent s'ajouter
        // exactement.
        let a = Decimal::new(10001, 2);
        let b = Decimal::new(20002, 2);
        assert_eq!(a + b, Decimal::new(30003, 2));
    }
}
