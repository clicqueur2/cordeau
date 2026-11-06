//! Fiche fournisseur et fiche de compte bancaire.
//!
//! Deux fiches de detail qui completent les listes : ce sont les seuls endroits
//! ou l'on modifie des coordonnees qui se retrouvent sur des documents
//! (bon de commande, RIB).
use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use maud::{html, Markup};

use crate::{message, Erreur, Res};

/// Fiche d'un fournisseur.
pub async fn fournisseur_fiche(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let f = db::achat::fournisseur_par_id(&app.pool, crate::entite::courante(), id)?
        .ok_or_else(|| Erreur("fournisseur introuvable".into()))?;
    let factures: Vec<_> =
        db::achat::factures_fournisseur(&app.pool, crate::entite::courante(), 500, 0)?
            .into_iter()
            .filter(|x| x.fk_soc == id)
            .collect();
    let total: rust_decimal::Decimal = factures
        .iter()
        .map(|x| x.total_ttc.unwrap_or_default())
        .sum();

    Ok(crate::ui::page(
        "achats",
        "fournisseurs",
        &f.nom,
        html! {
            (message(q.get("ok")))
            h1 { (f.nom) }
            p .sous-titre {
                "Code " code { (f.code_fournisseur.clone().unwrap_or_default()) }
                " — " (factures.len()) " facture(s), " (crate::ui::euros(total)) " TTC"
            }
            form method="post" action={ "/achats/fournisseur/" (id) "/modifier" } {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" value=(f.nom) required class="pleine"; } }
                        tr { th {(crate::langue::tr("E-mail")) } td { input type="email" name="email" value=(f.email.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("Téléphone")) } td { input type="text" name="telephone" value="" class="pleine"; } }
                        tr { th {(crate::langue::tr("Adresse")) } td { input type="text" name="adresse" value=(f.address.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("Ville")) } td { input type="text" name="ville" value=(f.town.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("SIRET")) } td { input type="text" name="siret" value=(f.siret.clone().unwrap_or_default()) class="pleine"; } }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton.secondaire href="/achats" {(crate::langue::tr("Retour")) }
                }
            }

            @if !factures.is_empty() {
                h2 {(crate::langue::tr("Factures fournisseurs")) }
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Chez le fournisseur")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("TTC")) } } }
                    tbody {
                        @for x in &factures {
                            tr {
                                td .ref { (x.ref_) }
                                td .ref { (x.ref_supplier.clone().unwrap_or_default()) }
                                td .ref { (app::fmt_date_fr(x.datef.as_deref())) }
                                td .num { (crate::ui::euros(x.total_ttc.unwrap_or_default())) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub async fn fournisseur_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let nom = champ("nom").ok_or_else(|| Erreur("nom obligatoire".into()))?;
    db::achat::modifier_fournisseur(
        &app.pool,
        crate::entite::courante(),
        id,
        &nom,
        champ("email").as_deref(),
        champ("telephone").as_deref(),
        champ("adresse").as_deref(),
        champ("ville").as_deref(),
        champ("siret").as_deref(),
    )?;
    Ok(Redirect::to(&format!(
        "/achats/fournisseur/{id}/modifier?ok=enregistre"
    )))
}

/// Fiche d'un compte bancaire.
pub async fn compte_fiche(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let c = db::banque::compte(&app.pool, crate::entite::courante(), id)?
        .ok_or_else(|| Erreur("compte introuvable".into()))?;
    let solde = db::banque::solde(&app.pool, crate::entite::courante(), id)?;

    Ok(crate::ui::page(
        "banques",
        "comptes",
        &c.ref_,
        html! {
            (message(q.get("ok")))
            h1 { (c.label) }
            p .sous-titre { "Référence " code { (c.ref_) } " — solde " strong { (crate::ui::euros(solde)) } }
            form method="post" action={ "/banques/" (id) "/modifier" } {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Libellé")) } td { input type="text" name="label" value=(c.label) required class="pleine"; } }
                        tr { th {(crate::langue::tr("Banque")) } td { input type="text" name="banque" value=(c.banque.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("IBAN")) } td { input type="text" name="iban" value=(c.iban.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("BIC")) } td { input type="text" name="bic" value=(c.bic.clone().unwrap_or_default()).champ-large; } }
                        tr {
                            th {(crate::langue::tr("Compte clos")) }
                            td {
                                select name="clos" {
                                    option value="0" selected[!c.clos] {(crate::langue::tr("Non")) }
                                    option value="1" selected[c.clos] {(crate::langue::tr("Oui")) }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton.secondaire href="/banques" {(crate::langue::tr("Retour")) }
                }
            }
        },
    ))
}

pub async fn compte_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let label = champ("label").ok_or_else(|| Erreur("libellé obligatoire".into()))?;
    db::banque::modifier_compte(
        &app.pool,
        crate::entite::courante(),
        id,
        &label,
        champ("banque").as_deref(),
        champ("iban").as_deref(),
        champ("bic").as_deref(),
        f.get("clos").map(|v| v == "1").unwrap_or(false),
    )?;
    Ok(Redirect::to(&format!(
        "/banques/{id}/modifier?ok=enregistre"
    )))
}
