//! Contrats : un engagement dans le temps, pas une facture.
//!
//! Dolibarr tient `llx_contrat` et `llx_contratdet`. Un contrat se cree en
//! brouillon, se valide, puis chacune de ses lignes s'active et se desactive a
//! ses propres dates — c'est ce qui en fait un abonnement et non une vente.
//!
//! L'ecran fait ce que la base sait faire : creer, ajouter une ligne, valider,
//! clore. Rien de plus, rien de moins.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use axum::Form;
use db::contrat::{self, NewContrat, NewContratLine, STATUT_BROUILLON, STATUT_VALIDE};
use maud::html;
use rust_decimal::Decimal;

use crate::*;

pub(crate) async fn contrats(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let liste = contrat::list(p, entite::courante(), 300, 0)?;
    let clients = db::party::list(p, entite::courante(), 500, 0)?;
    let en_cours = liste.iter().filter(|c| c.statut == STATUT_VALIDE).count();
    let total: Decimal = liste
        .iter()
        .filter(|c| c.statut == STATUT_VALIDE)
        .map(|c| c.total_ttc.unwrap_or_default())
        .sum();

    Ok(ui::page(
        "contrats",
        "liste",
        "Contrats",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Contrats")) }
            p .sous-titre {
                (crate::langue::tr("Un engagement dans le temps : il se valide une fois, puis ses lignes s'activent à leurs dates."))
            }

            div .kpis {
                div .kpi.fort {
                    div .titre {(crate::langue::tr("Contrats en cours")) }
                    div .valeur { (en_cours) }
                }
                div .kpi {
                    div .titre {(crate::langue::tr("Valeur TTC")) }
                    div .valeur { (ui::euros(total)) }
                    div .note {(crate::langue::tr("contrats validés")) }
                }
            }

            h2 {(crate::langue::tr("Nouveau contrat")) }
            @if clients.is_empty() {
                (ui::vide("Aucun tiers : un contrat se signe avec quelqu'un.", Some(("/tiers/nouveau", "Créer un tiers"))))
            } @else {
                form method="post" action="/contrats" {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Client")) }
                                td {
                                    select name="soc" required.pleine {
                                        @for c in &clients {
                                            option value=(c.rowid) { (c.nom.clone()) }
                                        }
                                    }
                                }
                            }
                            tr { th {(crate::langue::tr("Début")) } td { input type="date" name="debut" required.champ-moyen; } }
                            tr { th {(crate::langue::tr("Fin de validité")) } td { input type="date" name="fin" .champ-moyen; } }
                            tr { th {(crate::langue::tr("Référence client")) } td { input type="text" name="ref_client" .pleine; } }
                        }
                    }
                    div .actions {
                        button type="submit" {(crate::langue::tr("Créer le brouillon")) }
                        a .bouton.secondaire href="/contrats" {(crate::langue::tr("Annuler")) }
                    }
                }
            }

            h2 {(crate::langue::tr("Tous les contrats")) }
            @if liste.is_empty() {
                (ui::vide("Aucun contrat enregistré.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Réf.")) }
                            th {(crate::langue::tr("Client")) }
                            th {(crate::langue::tr("Début")) }
                            th {(crate::langue::tr("Fin")) }
                            th .num {(crate::langue::tr("TTC")) }
                            th { "État" }
                        }
                    }
                    tbody {
                        @for c in &liste {
                            tr {
                                td .ref { a href={ "/contrats/" (c.rowid) } { (c.ref_.clone()) } }
                                td { (nom_tiers(p, Some(c.fk_soc))?) }
                                td .ref { (app::fmt_date_fr(c.date_contrat.as_deref())) }
                                td .ref { (app::fmt_date_fr(c.fin_validite.as_deref())) }
                                td .num { (ui::euros(c.total_ttc.unwrap_or_default())) }
                                td {
                                    @if c.statut == STATUT_BROUILLON {
                                        span .pastille.p-brouillon { "brouillon" }
                                    } @else if c.statut == STATUT_VALIDE {
                                        span .pastille.p-validee {(crate::langue::tr("validé")) }
                                    } @else {
                                        span .pastille.p-payee {(crate::langue::tr("clos")) }
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

pub(crate) async fn contrat_fiche(State(app): State<App>, Path(id): Path<i64>) -> Res<Markup> {
    let p = &app.pool;
    let c = contrat::get(p, entite::courante(), id)?
        .ok_or_else(|| Erreur(format!("contrat {id} introuvable")))?;
    let lignes = contrat::lines(p, entite::courante(), id)?;
    let produits = db::product::list(p, entite::courante(), 300, 0)?;

    Ok(ui::page(
        "contrats",
        "liste",
        &c.ref_,
        html! {
            h1 { (c.ref_.clone()) }
            p .sous-titre {
                (nom_tiers(p, Some(c.fk_soc))?) " — "
                @if c.statut == STATUT_BROUILLON { "brouillon" }
                @else if c.statut == STATUT_VALIDE { (crate::langue::tr("validé")) }
                @else { (crate::langue::tr("clos")) }
            }

            div .actions {
                @if c.statut == STATUT_BROUILLON {
                    form method="post" action={ "/contrats/" (id) "/valider" } {
                        button type="submit" {(crate::langue::tr("Valider le contrat")) }
                    }
                }
                @if c.statut == STATUT_VALIDE {
                    form method="post" action={ "/contrats/" (id) "/clore" } {
                        button type="submit" class="secondaire" {(crate::langue::tr("Clore le contrat")) }
                    }
                }
                a .bouton.secondaire href="/contrats" {(crate::langue::tr("Retour")) }
            }

            h2 {(crate::langue::tr("Lignes")) }
            @if lignes.is_empty() {
                (ui::vide("Aucune ligne : un contrat sans ligne ne vaut rien.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Désignation")) }
                            th .num {(crate::langue::tr("Qté")) }
                            th .num {(crate::langue::tr("P.U. HT")) }
                            th .num {(crate::langue::tr("TVA")) }
                            th .num {(crate::langue::tr("Total HT")) }
                            th {(crate::langue::tr("Active du")) }
                            th {(crate::langue::tr("au")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for l in &lignes {
                            tr {
                                td { (l.label.clone().unwrap_or_default()) }
                                td .num { (l.qty.unwrap_or_default().normalize().to_string()) }
                                td .num { (ui::euros(l.subprice.unwrap_or_default())) }
                                td .num { (l.tva_tx.unwrap_or_default().normalize().to_string()) " %" }
                                td .num { (ui::euros(l.total_ht.unwrap_or_default())) }
                                td .ref { (app::fmt_date_fr(l.date_ouverture_prevue.as_deref())) }
                                td .ref { (app::fmt_date_fr(l.date_fin_validite.as_deref())) }
                                td {
                                    form method="post"
                                         action={ "/contrats/" (id) "/ligne/" (l.rowid) "/basculer" } {
                                        input type="hidden" name="actif" value=(if l.est_active() { "0" } else { "1" });
                                        button type="submit" class="petit secondaire" {
                                            @if l.est_active() { (crate::langue::tr("Désactiver")) }
                                            @else { (crate::langue::tr("Activer")) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Ajouter une ligne")) }
            form method="post" action={ "/contrats/" (id) "/ligne" } {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Produit")) }
                            td {
                                select name="produit" .pleine {
                                    option value="" {(crate::langue::tr("— ligne libre —")) }
                                    @for pr in &produits {
                                        option value=(pr.rowid) { (pr.ref_.clone()) " — " (pr.label.clone()) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Désignation")) } td { input type="text" name="label" required.pleine; } }
                        tr { th {(crate::langue::tr("Quantité")) } td { input type="text" name="qty" value="1" .champ-court; } }
                        tr { th {(crate::langue::tr("Prix unitaire HT")) } td { input type="text" name="pu" required.champ-court; } }
                        tr { th {(crate::langue::tr("TVA %")) } td { input type="text" name="tva" value="20" .champ-court; } }
                        tr { th {(crate::langue::tr("Active du")) } td { input type="date" name="du" .champ-moyen; } }
                        tr { th {(crate::langue::tr("au")) } td { input type="date" name="au" .champ-moyen; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Ajouter")) } }
            }
        },
    ))
}

pub(crate) async fn contrat_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let soc: i64 = champ("soc")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Erreur("client obligatoire".into()))?;
    // `fk_user_author` porte une cle etrangere vers `llx_user` : on la resout
    // depuis la session, jamais depuis le formulaire.
    let login = utilisateur::courant().ok_or_else(|| Erreur("session sans utilisateur".into()))?;
    let auteur = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;
    let mut nouveau = NewContrat {
        fk_soc: soc,
        date_contrat: champ("debut"),
        fin_validite: champ("fin"),
        ref_customer: champ("ref_client"),
        note_private: None,
        note_public: None,
        fk_user_author: Some(auteur),
    };
    if nouveau.date_contrat.is_none() {
        nouveau.date_contrat = Some(aujourdhui(&app.pool)?);
    }
    let id = contrat::create(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to(&format!("/contrats/{id}?ok=cree")))
}

pub(crate) async fn contrat_ligne(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let dec = |c: &str| -> Result<Decimal, Erreur> {
        f.get(c)
            .map(|v| v.trim().replace(',', "."))
            .and_then(|v| v.parse::<Decimal>().ok())
            .ok_or_else(|| Erreur(format!("{c} : nombre invalide")))
    };
    let ligne = NewContratLine {
        fk_product: champ("produit").and_then(|v| v.parse().ok()),
        label: champ("label").ok_or_else(|| Erreur("désignation obligatoire".into()))?,
        description: None,
        qty: f
            .get("qty")
            .map(|v| v.trim().replace(',', ".").parse().unwrap_or(Decimal::ONE))
            .unwrap_or(Decimal::ONE),
        subprice: dec("pu")?,
        remise_percent: Decimal::ZERO,
        tva_tx: dec("tva")?,
        date_ouverture_prevue: champ("du"),
        date_fin_validite: champ("au"),
    };
    contrat::add_line(&app.pool, entite::courante(), id, &ligne)?;
    Ok(Redirect::to(&format!("/contrats/{id}?ok=ligne")))
}

pub(crate) async fn contrat_valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    contrat::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/contrats/{id}?ok=valide")))
}

pub(crate) async fn contrat_clore(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    contrat::clore(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/contrats/{id}?ok=clos")))
}

pub(crate) async fn contrat_ligne_basculer(
    State(app): State<App>,
    Path((id, ligne)): Path<(i64, i64)>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let actif = f.get("actif").map(|v| v.trim() == "1").unwrap_or(false);
    if actif {
        contrat::activer_ligne(&app.pool, entite::courante(), ligne)?;
    } else {
        contrat::desactiver_ligne(&app.pool, entite::courante(), ligne)?;
    }
    Ok(Redirect::to(&format!("/contrats/{id}?ok=ligne")))
}
