//! Les commandes clients : suite d'un devis signe.
//!
//! Une commande engage le client. Elle se valide, puis se facture — en totalite
//! ou par acompte. L'expedition s'appuie dessus : on ne livre pas ce qui n'a pas
//! ete commande.

use crate::*;

pub(crate) async fn commandes(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let liste = db::commande::list(p, entite::courante(), 200, 0)?;

    Ok(ui::page(
        "commandes",
        "liste",
        "Commandes",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Commandes clients")) }
            p .sous-titre {(crate::langue::tr("Le maillon entre le devis signé et la facture.")) }
            @if liste.is_empty() {
                (ui::vide("Aucune commande. Elle se crée depuis un devis validé.", Some(("/devis", "Voir les devis"))))
            } @else {
                form id="actions-commandes" method="post" action="/commandes/actions" {}
                div .actions {
                    button type="submit" form="actions-commandes" name="action" value="valider" {
                        (crate::langue::tr("Valider la sélection"))
                    }
                    span style="color:#78736c;font-size:13.5px" {
                        (crate::langue::tr("Seules les commandes en brouillon sont cochables."))
                    }
                }
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Client")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("TTC")) } th { "État" } } }
                    tbody {
                        @for c in &liste {
                            tr {
                                td .ref {
                                    @if c.fk_statut == 0 {
                                        input type="checkbox" name={"id_" (c.rowid)} value="1"
                                              form="actions-commandes"
                                              aria-label=(crate::langue::tr("sélectionner cette commande"));
                                        " "
                                    }
                                    a href={ "/commandes/" (c.rowid) } { (c.ref_) }
                                }
                                td { (nom_tiers(p, Some(c.fk_soc))?) }
                                td .ref { (app::fmt_date_fr(c.date_commande.as_deref())) }
                                td .num { (ui::euros(c.total_ttc.unwrap_or_default())) }
                                td {
                                    @match c.fk_statut {
                                        0 => { span .pastille.p-brouillon { "brouillon" } }
                                        1 => { span .pastille.p-validee {(crate::langue::tr("validée")) } }
                                        2 => { span .pastille.p-payee {(crate::langue::tr("livrée")) } }
                                        _ => { span .pastille.p-brouillon {(crate::langue::tr("annulée")) } }
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

pub(crate) async fn commande_fiche(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let c = db::commande::get(p, entite::courante(), id)?
        .ok_or(Erreur("Commande introuvable".into()))?;
    let lignes = db::commande::lines(p, entite::courante(), id)?;
    let acompte_existe = db::invoice::acompte_existe(&app.pool, entite::courante(), &c.ref_)?;

    Ok(ui::page(
        "commandes",
        "liste",
        &c.ref_,
        html! {
            (message(q.get("ok")))
            h1 { "Commande " (c.ref_) " " (ui::pastille(c.fk_statut, 0, false)) }
            p .sous-titre {
                "Client : " strong { (nom_tiers(p, Some(c.fk_soc))?) }
                @if let Some(d) = &c.date_commande { " — du " (app::fmt_date_fr(Some(d))) }
            }

            (table_lignes(
                lignes.iter().map(|l| (
                    l.is_section(),
                    l.label.clone().unwrap_or_default(),
                    app::fmt_qte(l.qty),
                    l.subprice.unwrap_or_default(),
                    app::fmt_qte(l.tva_tx),
                    l.total_ht.unwrap_or_default(),
                )),
                c.total_ht.unwrap_or_default(),
                c.total_tva.unwrap_or_default(),
                c.total_ttc.unwrap_or_default(),
                None,
            ))

            div .actions {
                @if c.est_brouillon() {
                    form method="post" action={ "/commandes/" (id) "/valider" } {
                        button type="submit" {(crate::langue::tr("Valider la commande")) }
                    }
                } @else {
                    form method="post" action={ "/commandes/" (id) "/facturer" } {
                        button type="submit" {(crate::langue::tr("Facturer la commande")) }
                    }
                }
                @if c.deposit_percent.unwrap_or_default() > rust_decimal::Decimal::ZERO && !acompte_existe {
                    form method="post" action={ "/commandes/" (id) "/acompte" } {
                        button type="submit" {
                            "Créer l'acompte (" (app::fmt_qte(c.deposit_percent)) " %)"
                        }
                    }
                }
                a .bouton.secondaire href={ "/commandes/" (id) "/pdf" } {(crate::langue::tr("Télécharger le PDF")) }
                a .bouton.secondaire href="/commandes" {(crate::langue::tr("Retour")) }
            }
        },
    ))
}

pub(crate) async fn commande_pdf(State(app): State<App>, Path(id): Path<i64>) -> Res<Response> {
    let rendu = app::commande_pdf(&app.pool, entite::courante(), id)?;
    Ok(pdf_response(&rendu.bytes, &format!("commande-{id}.pdf")))
}

pub(crate) async fn commande_facturer(
    State(app): State<App>,
    Path(id): Path<i64>,
) -> Res<Redirect> {
    let facture = db::invoice::create_from_commande(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/factures/{facture}?ok=facturee")))
}

/// Cree la facture d'acompte d'une commande (type 3, une ligne par taux).
pub(crate) async fn commande_acompte(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let facture = db::invoice::create_deposit_from_commande(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/factures/{facture}?ok=acompte")))
}

pub(crate) async fn commande_valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    db::commande::validate(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/commandes/{id}?ok=validee")))
}
