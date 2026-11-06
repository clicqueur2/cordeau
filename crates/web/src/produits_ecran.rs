//! Les produits et services.
//!
//! Un produit porte un prix, une TVA et une unite ; un service n'a pas de
//! stock. La distinction tient dans `fk_product_type`, et elle se lit partout
//! ailleurs : une ligne de facture herite de ce type.

use crate::*;

pub(crate) async fn produits(State(app): State<App>) -> Res<Markup> {
    let listes = db::product::list(&app.pool, entite::courante(), 500, 0)?;
    Ok(ui::page(
        "produits",
        "liste",
        "Produits",
        html! {
            h1 {(crate::langue::tr("Produits et services")) }
            p .sous-titre { (listes.len()) " référence(s)" }
            @if listes.is_empty() {
                (ui::vide("Aucun produit.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Libellé")) } th .num {(crate::langue::tr("PU HT")) } th {(crate::langue::tr("Type")) } th {(crate::langue::tr("Unité")) } } }
                    tbody {
                        @for pr in &listes {
                            tr {
                                td .ref { (pr.ref_) }
                                td { (pr.label) }
                                td .num { (ui::euros(pr.price.unwrap_or_default())) }
                                td { (if pr.est_service() { "service" } else { "produit" }) }
                                td { (pr.fk_unit.map(|u| u.to_string()).unwrap_or_default()) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Fiche d'un produit : consultation et edition des champs courants.
pub(crate) async fn produit_fiche(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = db::product::get(&app.pool, entite::courante(), id)?
        .ok_or_else(|| Erreur("produit introuvable".into()))?;
    let tva = db::dict::taux_tva(&app.pool, entite::courante(), 1)?;

    Ok(ui::page(
        "produits",
        "liste",
        &p.ref_,
        html! {
            (message(q.get("ok")))
            h1 { (p.label) " " }
            p .sous-titre { "Référence " code { (p.ref_) } " — la référence ne change jamais : elle sert de clé dans les documents déjà émis." }
            form method="post" action={ "/produits/" (id) "/modifier" } {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Désignation")) } td { input type="text" name="label" value=(p.label) required class="pleine"; } }
                        tr { th {(crate::langue::tr("Prix HT")) } td { input type="number" name="prix" step="0.01" min="0" value=(app::fmt_dec(p.price)).champ-moyen; } }
                        tr {
                            th {(crate::langue::tr("TVA")) }
                            td {
                                select name="tva" {
                                    @for t in &tva {
                                        option value=(t.taux) selected[p.tva_tx == Some(t.taux)] { (app::fmt_qte(Some(t.taux))) " %" }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Description")) } td { input type="text" name="description" value=(p.description.clone().unwrap_or_default()) class="pleine"; } }
                        tr {
                            th {(crate::langue::tr("En vente")) }
                            td {
                                select name="vente" {
                                    option value="1" selected[p.on_sell()] {(crate::langue::tr("Oui")) }
                                    option value="0" selected[!p.on_sell()] {(crate::langue::tr("Non (masqué)")) }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton.secondaire href="/produits" {(crate::langue::tr("Retour")) }
                }
            }
            p .sous-titre { "Prix TTC calculé : " strong { (app::fmt_eur(p.price_ttc)) } }
        },
    ))
}

pub(crate) async fn produit_enregistrer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let label = champ("label").ok_or_else(|| Erreur("désignation obligatoire".into()))?;
    let prix: Decimal = champ("prix")
        .and_then(|v| v.replace(',', ".").parse().ok())
        .unwrap_or_default();
    let tva: Decimal = champ("tva")
        .and_then(|v| v.replace(',', ".").parse().ok())
        .unwrap_or_default();
    let description = champ("description");
    let vente = f.get("vente").map(|v| v == "1").unwrap_or(true);
    db::product::update(
        &app.pool,
        entite::courante(),
        id,
        &label,
        prix,
        tva,
        description.as_deref(),
        vente,
    )?;
    Ok(Redirect::to(&format!("/produits/{id}?ok=enregistre")))
}

pub(crate) async fn produits_nouveau(State(app): State<App>) -> Res<Markup> {
    let unites = db::dict::unites(&app.pool)?;
    let tva = db::dict::taux_tva(&app.pool, entite::courante(), 1)?;
    Ok(ui::page(
        "produits",
        "nouveau",
        "Nouveau produit",
        html! {
            h1 {(crate::langue::tr("Nouveau produit")) }
            form method="post" action="/produits/nouveau" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Reference")) } td { input type="text" name="ref" required.champ-moyen; } }
                        tr { th {(crate::langue::tr("Libelle")) } td { input type="text" name="label" required class="pleine"; } }
                        tr { th {(crate::langue::tr("Prix HT")) } td { input type="number" name="prix" step="0.01" min="0".champ-moyen; } }
                        tr {
                            th {(crate::langue::tr("Taux de TVA")) }
                            td {
                                select name="tva" {
                                    @for t in &tva {
                                        option value=(t.taux) { (app::fmt_dec(Some(t.taux))) " %" }
                                    }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Unite")) }
                            td {
                                select name="unite" {
                                    option value="" { "aucune" }
                                    @for u in &unites {
                                        option value=(u.code) { (u.libelle) " (" (u.code) ")" }
                                    }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Nature")) }
                            td {
                                select name="nature" {
                                    option value="produit" {(crate::langue::tr("Produit")) }
                                    option value="service" {(crate::langue::tr("Service")) }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Creer le produit")) }
                    a .bouton.secondaire href="/produits" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

pub(crate) async fn produits_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let reference = f.get("ref").map(|s| s.trim()).unwrap_or("");
    let label = f.get("label").map(|s| s.trim()).unwrap_or("");
    let mut nouveau = db::product::NewProduct::new(reference, label);
    nouveau.price = f.get("prix").and_then(|v| v.replace(',', ".").parse().ok());
    nouveau.tva_tx = f.get("tva").and_then(|v| v.replace(',', ".").parse().ok());
    nouveau.fk_unit = match f.get("unite").map(|s| s.as_str()) {
        Some(c) if !c.is_empty() => db::product::unit_id(&app.pool, c)?,
        _ => None,
    };
    if f.get("nature").map(|s| s == "service").unwrap_or(false) {
        nouveau.fk_product_type = db::product::TYPE_SERVICE;
    }
    db::product::create(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to("/produits"))
}
