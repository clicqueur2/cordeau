//! Salaires et bulletins : ce que l'entreprise doit a ses gens.
//!
//! Dolibarr tient `llx_salary`. Un bulletin n'est pas une facture : il ne
//! s'annule pas, il se regularise par un second bulletin positif. L'ecran suit
//! cette regle — on cree, on paie, et on supprime seulement un brouillon.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use axum::Form;
use db::salaire::{self, NouveauSalaire};
use maud::html;
use rust_decimal::Decimal;

use crate::*;

pub(crate) async fn salaires(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let liste = salaire::list(p, entity, 300, 0)?;
    let gens = db::user::list(p, entity)?;
    let comptes = db::banque::comptes(p, entity)?;
    let annee: i32 = aujourdhui(p)?
        .get(..4)
        .and_then(|a| a.parse().ok())
        .unwrap_or(2026);
    let total_annee = salaire::total_par_annee(p, entity, annee).unwrap_or_default();
    let impayes: Decimal = liste.iter().filter(|s| !s.paye).map(|s| s.amount).sum();

    Ok(ui::page(
        "salaires",
        "liste",
        "Salaires",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Salaires")) }
            p .sous-titre {
                (crate::langue::tr("Un bulletin ne s'annule pas : il se régularise par un second bulletin positif."))
            }

            div .kpis {
                div .kpi.fort {
                    div .titre {(crate::langue::tr("Versé en")) " " (annee) }
                    div .valeur { (ui::euros(total_annee)) }
                }
                div .kpi.alerte {
                    div .titre {(crate::langue::tr("À payer")) }
                    div .valeur { (ui::euros(impayes)) }
                }
            }

            h2 {(crate::langue::tr("Nouveau bulletin")) }
            @if gens.is_empty() {
                (ui::vide("Aucun utilisateur : un bulletin se rattache à quelqu'un.", Some(("/utilisateurs", "Voir les utilisateurs"))))
            } @else {
                form method="post" action="/salaires" {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Salarié")) }
                                td {
                                    select name="user" required.pleine {
                                        @for g in &gens {
                                            option value=(g.rowid) { (g.nom_complet()) " (" (g.login.clone()) ")" }
                                        }
                                    }
                                }
                            }
                            tr { th {(crate::langue::tr("Libellé")) } td { input type="text" name="label" required.pleine placeholder="Salaire de septembre"; } }
                            tr { th {(crate::langue::tr("Montant")) } td { input type="text" name="amount" required.champ-court; } }
                            tr { th {(crate::langue::tr("Date de paiement")) } td { input type="date" name="datep" required.champ-moyen; } }
                            tr { th {(crate::langue::tr("Date de valeur")) } td { input type="date" name="datev" .champ-moyen; } }
                        }
                    }
                    div .actions {
                        button type="submit" {(crate::langue::tr("Créer le bulletin")) }
                    }
                }
            }

            h2 {(crate::langue::tr("Bulletins")) }
            @if liste.is_empty() {
                (ui::vide("Aucun bulletin enregistré.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Réf.")) }
                            th {(crate::langue::tr("Salarié")) }
                            th {(crate::langue::tr("Libellé")) }
                            th {(crate::langue::tr("Date")) }
                            th .num {(crate::langue::tr("Montant")) }
                            th { "État" }
                            th { "" }
                        }
                    }
                    tbody {
                        @for s in &liste {
                            tr {
                                td .ref { (s.ref_.clone()) }
                                td { (s.login.clone().unwrap_or_default()) }
                                td { (s.label.clone().unwrap_or_default()) }
                                td .ref { (app::fmt_date_fr(s.datep.as_deref())) }
                                td .num { (ui::euros(s.amount)) }
                                td {
                                    @if s.paye {
                                        span .pastille.p-payee {(crate::langue::tr("payé")) }
                                    } @else {
                                        span .pastille.p-brouillon {(crate::langue::tr("à payer")) }
                                    }
                                }
                                td {
                                    @if !s.paye && !comptes.is_empty() {
                                        form method="post" action={ "/salaires/" (s.rowid) "/payer" } {
                                            select name="banque" .petite {
                                                @for c in &comptes {
                                                    option value=(c.rowid) { (c.label.clone()) }
                                                }
                                            }
                                            " "
                                            button type="submit" class="petit" {(crate::langue::tr("Payer")) }
                                        }
                                    } @else if !s.paye {
                                        form method="post" action={ "/salaires/" (s.rowid) "/supprimer" } {
                                            button type="submit" class="petit secondaire" {(crate::langue::tr("Supprimer")) }
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

pub(crate) async fn salaire_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let montant: Decimal = f
        .get("amount")
        .map(|v| v.trim().replace(',', "."))
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Erreur("montant invalide".into()))?;
    let jour = aujourdhui(&app.pool)?;
    let nouveau = NouveauSalaire {
        fk_user: champ("user")
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| Erreur("salarié obligatoire".into()))?,
        datep: champ("datep").unwrap_or_else(|| jour.clone()),
        datev: champ("datev").unwrap_or(jour),
        amount: montant,
        label: champ("label").ok_or_else(|| Erreur("libellé obligatoire".into()))?,
        fk_project: None,
        paye: false,
    };
    salaire::creer(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to("/salaires?ok=cree"))
}

pub(crate) async fn salaire_payer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let banque: i64 = f
        .get("banque")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| Erreur("compte bancaire obligatoire".into()))?;
    // 2 = virement, comme dans Dolibarr.
    let jour = aujourdhui(&app.pool)?;
    salaire::payer(&app.pool, entite::courante(), id, banque, 2, &jour, None)?;
    Ok(Redirect::to("/salaires?ok=paye"))
}

pub(crate) async fn salaire_supprimer(
    State(app): State<App>,
    Path(id): Path<i64>,
) -> Res<Redirect> {
    salaire::supprimer(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/salaires?ok=supprime"))
}
