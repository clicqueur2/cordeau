//! Les produits et services.
//!
//! Un produit porte un prix, une TVA et une unite ; un service n'a pas de
//! stock. La distinction tient dans `fk_product_type`, et elle se lit partout
//! ailleurs : une ligne de facture herite de ce type.

use crate::*;

pub(crate) async fn produits(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let filtre = q.get("q").cloned().unwrap_or_default().to_lowercase();
    let genre = q.get("genre").cloned().unwrap_or_default();
    let en_vente = q.get("en_vente").map(|v| v == "1").unwrap_or(false);

    let listes: Vec<_> = db::product::list(&app.pool, entite::courante(), 2000, 0)?
        .into_iter()
        .filter(|p| {
            filtre.is_empty()
                || p.ref_.to_lowercase().contains(&filtre)
                || p.label.to_lowercase().contains(&filtre)
        })
        .filter(|p| match genre.as_str() {
            "service" => p.est_service(),
            "produit" => !p.est_service(),
            _ => true,
        })
        .filter(|p| !en_vente || (p.tosell == 1 && p.hidden == 0))
        .collect();

    const DISPONIBLES: &[&str] = &["ref", "libelle", "prix", "type", "unite", "tva"];
    let visibles = colonnes::visibles(&app.pool, "produits", &["ref", "libelle", "prix", "type"])?;

    Ok(ui::page(
        "produits",
        "liste",
        "Produits",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Produits et services")) }
            div .barre-outils {
                form method="get" action="/produits" {
                    input type="search" name="q" value=(filtre)
                          placeholder=(crate::langue::tr("Référence ou libellé")) .champ-moyen;
                    select name="genre" .champ-moyen {
                        option value="" selected[genre.is_empty()] { (crate::langue::tr("Tout")) }
                        option value="produit" selected[genre == "produit"] { (crate::langue::tr("Produits")) }
                        option value="service" selected[genre == "service"] { (crate::langue::tr("Services")) }
                    }
                    label .etiquette {
                        input type="checkbox" name="en_vente" value="1" checked[en_vente];
                        " " (crate::langue::tr("en vente"))
                    }
                    button type="submit" {(crate::langue::tr("Chercher")) }
                    a .bouton.secondaire.petit href="/produits" {(crate::langue::tr("Réinitialiser")) }
                }
                span .pousse {}
                p .compte-liste { strong { (listes.len()) } " " (crate::langue::tr("référence(s)")) }
            }

            (colonnes::choix_colonnes("produits", DISPONIBLES, &visibles))

            @if listes.is_empty() {
                (ui::vide("Aucun produit ne correspond.", Some(("/produits/nouveau", "Créer un produit"))))
            } @else {
                table {
                    thead {
                        tr {
                            @if colonnes::montre(&visibles, "ref") { th {(crate::langue::tr("Réf.")) } }
                            @if colonnes::montre(&visibles, "libelle") { th {(crate::langue::tr("Libellé")) } }
                            @if colonnes::montre(&visibles, "prix") { th .num {(crate::langue::tr("PU HT")) } }
                            @if colonnes::montre(&visibles, "type") { th {(crate::langue::tr("Type")) } }
                            @if colonnes::montre(&visibles, "unite") { th {(crate::langue::tr("Unité")) } }
                            @if colonnes::montre(&visibles, "tva") { th .num {(crate::langue::tr("TVA")) } }
                        }
                    }
                    tbody {
                        @for pr in &listes {
                            tr {
                                @if colonnes::montre(&visibles, "ref") {
                                    td .ref { a href={ "/produits/" (pr.rowid) } { (pr.ref_.clone()) } }
                                }
                                @if colonnes::montre(&visibles, "libelle") {
                                    td { a href={ "/produits/" (pr.rowid) } { (pr.label.clone()) } }
                                }
                                @if colonnes::montre(&visibles, "prix") { td .num { (ui::euros(pr.price.unwrap_or_default())) } }
                                @if colonnes::montre(&visibles, "type") {
                                    td { (if pr.est_service() { "service" } else { "produit" }) }
                                }
                                @if colonnes::montre(&visibles, "unite") { td { (pr.fk_unit.map(|u| u.to_string()).unwrap_or_default()) } }
                                @if colonnes::montre(&visibles, "tva") { td .num { (pr.tva_tx.map(|t| t.normalize().to_string()).unwrap_or_default()) } }
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
