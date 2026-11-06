//! Les achats : factures fournisseurs et fournisseurs.
//!
//! Une facture fournisseur est l'image d'une facture client, mais elle engage
//! une depense : elle se saisit, se valide, et alimente la comptabilite.

use crate::*;

pub(crate) async fn achats(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let liste = db::achat::fournisseurs(p, entite::courante(), 500, 0)?;
    let factures = db::achat::factures_fournisseur(p, entite::courante(), 100, 0)?;
    let paiements = db::divers::paiements_fournisseurs(p, entite::courante(), 50)?;
    let total = db::achat::total_achats(p, entite::courante())?;

    Ok(ui::page(
        "achats",
        "fournisseurs",
        "Achats",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Achats")) }
            p .sous-titre {(crate::langue::tr("Fournisseurs et factures fournisseurs (llx_societe fournisseur à 1, llx_facture_fourn)."))
            }

            div .cartes {
                (ui::carte("Achats (TTC)", &ui::euros(total), None, false))
                (ui::carte("Fournisseurs", &liste.len().to_string(), None, false))
            }

            h2 {(crate::langue::tr("Fournisseurs")) }
            @if liste.is_empty() {
                (ui::vide("Aucun fournisseur.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Nom")) } th {(crate::langue::tr("Ville")) } th {(crate::langue::tr("SIRET")) } th {(crate::langue::tr("E-mail")) } } }
                    tbody {
                        @for f in &liste {
                            tr {
                                td .ref { (f.code_fournisseur.clone().unwrap_or_default()) }
                                td { (f.nom) }
                                td { (f.town.clone().unwrap_or_default()) }
                                td .ref { (f.siret.clone().unwrap_or_default()) }
                                td { (f.email.clone().unwrap_or_default()) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouveau fournisseur")) }
            form method="post" action="/achats/fournisseur" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" required class="pleine"; } }
                        tr { th {(crate::langue::tr("Code fournisseur")) } td { input type="text" name="code" placeholder="généré si vide".champ-moyen; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le fournisseur")) } }
            }

            @if !paiements.is_empty() {
                h2 {(crate::langue::tr("Paiements fournisseurs")) }
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Mode")) } th .num {(crate::langue::tr("Montant")) } } }
                    tbody {
                        @for (ref_, date, montant, mode) in &paiements {
                            tr {
                                td .ref { (ref_) }
                                td .ref { (app::fmt_date_fr(date.as_deref())) }
                                td { (mode.clone().unwrap_or_default()) }
                                td .num { (ui::euros(*montant)) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Factures fournisseurs")) }
            div .actions {
                a .bouton href="/achats/facture" {(crate::langue::tr("Saisir une facture fournisseur")) }
            }
            @if factures.is_empty() {
                (ui::vide("Aucune facture fournisseur.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Chez le fournisseur")) } th {(crate::langue::tr("Fournisseur")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("TTC")) } th { "État" } th { "" } } }
                    tbody {
                        @for f in &factures {
                            tr {
                                td .ref { (f.ref_) }
                                td .ref { (f.ref_supplier.clone().unwrap_or_default()) }
                                td { (f.fournisseur.clone().unwrap_or_default()) }
                                td .ref { (f.datef.clone().unwrap_or_default()) }
                                td .num { (ui::euros(f.total_ttc.unwrap_or_default())) }
                                td { (ui::pastille(f.fk_statut, if f.paye { 1 } else { 0 }, false)) }
                                td {
                                    @if f.est_brouillon() {
                                        form method="post" action={ "/achats/facture/" (f.rowid) "/valider" } {
                                            button type="submit" class="secondaire petit" {(crate::langue::tr("Valider")) }
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

pub(crate) async fn achat_facture_nouvelle(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let fournisseurs = db::achat::fournisseurs(&app.pool, entite::courante(), 500, 0)?;
    let tva = db::dict::taux_tva(&app.pool, entite::courante(), 1)?;

    Ok(ui::page(
        "achats",
        "facture",
        "Nouvelle facture fournisseur",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Nouvelle facture fournisseur")) }
            p .sous-titre {(crate::langue::tr("Saisie d'un achat reçu. La facture est créée en brouillon.")) }
            form method="post" action="/achats/facture" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Fournisseur")) }
                            td {
                                select name="soc" required class="pleine" {
                                    @for f in &fournisseurs {
                                        option value=(f.rowid) { (f.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Référence chez le fournisseur")) } td { input type="text" name="ref_supplier" class="pleine"; } }
                        tr { th {(crate::langue::tr("Date")) } td { input type="date" name="datef" class="champ-moyen"; } }
                        tr { th {(crate::langue::tr("Note interne")) } td { input type="text" name="note" class="pleine"; } }
                    }
                }
                h2 {(crate::langue::tr("Lignes")) }
                table {
                    thead { tr { th {(crate::langue::tr("Désignation")) } th .num {(crate::langue::tr("Qté")) } th .num {(crate::langue::tr("PU HT")) } th {(crate::langue::tr("TVA")) } } }
                    tbody {
                        @for i in 1..=5 {
                            tr {
                                td { input type="text" name=(format!("l{i}_lib")) class="pleine"; }
                                td { input type="number" name=(format!("l{i}_qte")) step="0.01" min="0" value="1" class="champ-court"; }
                                td { input type="number" name=(format!("l{i}_pu")) step="0.01" min="0".champ-court; }
                                td {
                                    select name=(format!("l{i}_tva")) {
                                        @for t in &tva {
                                            option value=(t.taux) { (app::fmt_qte(Some(t.taux))) " %" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Créer la facture fournisseur")) }
                    a .bouton.secondaire href="/achats" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

pub(crate) async fn achat_facture_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let fk_soc = f
        .get("soc")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("fournisseur invalide".into()))?;
    let mut lignes = Vec::new();
    for i in 1..=5 {
        let lib = f.get(&format!("l{i}_lib")).map(|s| s.trim()).unwrap_or("");
        if lib.is_empty() {
            continue;
        }
        let qte: Decimal = f
            .get(&format!("l{i}_qte"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let pu: Decimal = f
            .get(&format!("l{i}_pu"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let tva: Decimal = f
            .get(&format!("l{i}_tva"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        lignes.push(db::achat_ecriture::LigneAchat::new(lib, qte, pu, tva));
    }
    if lignes.is_empty() {
        return Err(Erreur("au moins une ligne est nécessaire".into()));
    }
    let mut nouvelle = db::achat_ecriture::NouvelleFactureAchat::new(fk_soc);
    nouvelle.ref_supplier = f
        .get("ref_supplier")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    nouvelle.datef = f
        .get("datef")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    nouvelle.note = f
        .get("note")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    nouvelle.lines = lignes;
    db::achat_ecriture::creer(&app.pool, entite::courante(), &nouvelle)?;
    Ok(Redirect::to("/achats?ok=facture"))
}

pub(crate) async fn achat_facture_valider(
    State(app): State<App>,
    Path(id): Path<i64>,
) -> Res<Redirect> {
    db::achat_ecriture::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/achats?ok=facture-validee"))
}

pub(crate) async fn achat_fournisseur_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let nom = f
        .get("nom")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("nom du fournisseur obligatoire".into()))?;
    let code = f
        .get("code")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    db::achat::creer_fournisseur(&app.pool, entite::courante(), &nom, code.as_deref())?;
    Ok(Redirect::to("/achats?ok=fournisseur"))
}
