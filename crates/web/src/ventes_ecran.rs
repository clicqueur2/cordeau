//! Les devis : de la saisie a la signature.
//!
//! Le cycle est court mais rigide : brouillon, valide, envoye, signe (ou
//! perdu). Une fois valide, les lignes ne se modifient plus : le client a recu
//! un document, et le changer apres coup reviendrait a modifier une piece deja
//! transmise.

use crate::*;

pub(crate) async fn devis(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let statut = q.get("statut").cloned().unwrap_or_default();
    let filtre = q
        .get("q")
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();

    let tous = db::proposal::list(p, entite::courante(), 200, 0)?;
    let total = tous.len();
    let listes: Vec<_> = tous
        .into_iter()
        .filter(|d| {
            let par_statut = match statut.as_str() {
                "brouillon" => d.fk_statut == 0,
                "valide" => d.fk_statut == 1,
                "signe" => d.fk_statut == 2,
                "perdu" => d.fk_statut == 3,
                _ => true,
            };
            let par_texte = filtre.is_empty() || d.ref_.to_lowercase().contains(&filtre);
            par_statut && par_texte
        })
        .collect();

    Ok(ui::page(
        "devis",
        "liste",
        "Devis",
        html! {
            h1 {(crate::langue::tr("Devis")) }
            form method="get" action="/devis" {
                div .actions {
                    select name="statut" aria-label="Statut" {
                        option value="" selected[statut.is_empty()] {(crate::langue::tr("Tous")) }
                        option value="brouillon" selected[statut == "brouillon"] {(crate::langue::tr("Brouillons")) }
                        option value="valide" selected[statut == "valide"] {(crate::langue::tr("Validés")) }
                        option value="signe" selected[statut == "signe"] {(crate::langue::tr("Signés")) }
                        option value="perdu" selected[statut == "perdu"] {(crate::langue::tr("Perdus")) }
                    }
                    input type="search" name="q" value=(filtre) placeholder="Référence…" aria-label="Recherche";
                    button type="submit" class="petit" {(crate::langue::tr("Filtrer")) }
                    @if !statut.is_empty() || !filtre.is_empty() {
                        a .bouton.secondaire.petit href="/devis" {(crate::langue::tr("Tout voir")) }
                    }
                }
            }
            p .sous-titre {
                (listes.len()) " devis"
                @if listes.len() != total {
                    " sur " (total.to_string())
                }
            }
            // Le formulaire est declare a part et les cases s'y rattachent par
            // leur attribut `form` : un formulaire ne peut pas entourer une
            // partie de tableau, et on ne va pas imbriquer des tables.
            form id="actions-devis" method="post" action="/devis/actions" {}
            div .actions {
                button type="submit" form="actions-devis" name="action" value="valider" {(crate::langue::tr("Valider la sélection")) }
                span class="aide" {(crate::langue::tr("Seuls les devis en brouillon sont cochables."))
                }
            }
            @if listes.is_empty() {
                (ui::vide("Aucun devis.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Client")) } th .num {(crate::langue::tr("HT")) } th .num {(crate::langue::tr("TTC")) } th {(crate::langue::tr("Statut")) } th { "" } } }
                    tbody {
                        @for d in &listes {
                            tr {
                                td .ref {
                                    @if d.fk_statut == 0 {
                                        input type="checkbox" name={"id_" (d.rowid)}
                                              value="1" form="actions-devis"
                                              aria-label="sélectionner ce devis";
                                        " "
                                    }
                                    a href={ "/devis/" (d.rowid) } { (d.ref_) }
                                }
                                td { (nom_tiers(p, d.fk_soc)?) }
                                td .num { (ui::euros(d.total_ht.unwrap_or_default())) }
                                td .num { (ui::euros(d.total_ttc.unwrap_or_default())) }
                                td { (ui::pastille(d.fk_statut, 0, false)) }
                                td { a .bouton.petit.secondaire href={ "/devis/" (d.rowid) } {(crate::langue::tr("Ouvrir")) } }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn devis_fiche(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let d =
        db::proposal::get(p, entite::courante(), id)?.ok_or(Erreur("Devis introuvable".into()))?;
    let lignes = db::proposal::lines(p, entite::courante(), id)?;
    let reglement =
        db::reglement::condition(p, entite::courante(), d.fk_cond_reglement.unwrap_or(0))
            .ok()
            .flatten()
            .map(|c| c.libelle_fr())
            .filter(|l| !l.trim().is_empty());
    let situations = db::situation::situations(p, entite::courante(), id)?;
    let total_devis = db::situation::total_devis(p, entite::courante(), id)?;
    let deja_facture: Decimal = situations.iter().map(|s| s.total_ttc).sum();
    let reste_a_facturer = total_devis - deja_facture;

    Ok(ui::page(
        "devis",
        "liste",
        &d.ref_,
        html! {
            (message(q.get("ok")))
            h1 { "Devis " (d.ref_) " " (ui::pastille(d.fk_statut, 0, false)) }
            p .sous-titre { "Client : " strong { (nom_tiers(p, d.fk_soc)?) }
                @if let Some(r) = &reglement { " — règlement " (r) }
                @if let Some(a) = d.deposit_percent { " — acompte " (app::fmt_qte(Some(a))) " %" } }

            (table_lignes(
                lignes.iter().map(|l| (
                    l.is_section(),
                    l.label.clone().unwrap_or_default(),
                    app::fmt_dec(l.qty),
                    l.subprice.unwrap_or_default(),
                    app::fmt_dec(l.tva_tx),
                    l.total_ht.unwrap_or_default(),
                )),
                d.total_ht.unwrap_or_default(),
                d.total_tva.unwrap_or_default(),
                d.total_ttc.unwrap_or_default(),
                None,
            ))

            div .actions {
                a .bouton.secondaire href={ "/devis/" (id) "/pdf" } {(crate::langue::tr("Télécharger le PDF")) }
                @if d.fk_statut == 0 {
                    a .bouton.secondaire href={ "/devis/" (id) "/modifier" } {(crate::langue::tr("Modifier")) }
                    form method="post" action={ "/devis/" (id) "/valider" } {
                        button type="submit" {(crate::langue::tr("Valider le devis")) }
                    }
                }
                @if d.fk_statut == 1 {
                    form method="post" action={ "/devis/" (id) "/signer" } {
                        button type="submit" {(crate::langue::tr("Signer le devis")) }
                    }
                    form method="post" action={ "/devis/" (id) "/perdu" } {
                        button type="submit" class="secondaire" {(crate::langue::tr("Marquer perdu")) }
                    }
                }
                @if situations.is_empty() {
                    form method="post" action={ "/devis/" (id) "/facturer" } {
                        button type="submit" {(crate::langue::tr("Facturer ce devis")) }
                    }
                    form method="post" action={ "/devis/" (id) "/commander" } {
                        button type="submit" class="secondaire" {(crate::langue::tr("Créer la commande")) }
                    }
                } @else {
                    p class="aide petite" {(crate::langue::tr("Ce devis est facturé par situations : la facture globale est désactivée pour éviter la double facturation."))
                    }
                }
            }

            h2 {(crate::langue::tr("Facturation par situations")) }
            p .sous-titre {
                "Le devis se facture par tranches. La somme des situations égale le devis ; "
                "la dernière prend le reste au centime."
            }
            @if situations.is_empty() {
                (ui::vide("Aucune situation facturée.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("N°")) } th {(crate::langue::tr("Facture")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("Part")) } th .num {(crate::langue::tr("TTC")) } th { "" } } }
                    tbody {
                        @for s in &situations {
                            tr {
                                td .ref { (s.counter) }
                                td .ref { a href={ "/factures/" (s.rowid) } { (s.ref_) } }
                                td .ref { (s.datef.clone().unwrap_or_default()) }
                                td .num { (app::fmt_qte(Some(s.pourcentage))) " %" }
                                td .num { (ui::euros(s.total_ttc)) }
                                td {
                                    @if s.finale {
                                        span .pastille.p-payee { "solde" }
                                    }
                                }
                            }
                        }
                    }
                    tfoot {
                        tr {
                            td colspan="4" .num {(crate::langue::tr("Reste à facturer")) }
                            td .num .total-fort { (ui::euros(reste_a_facturer)) }
                            td {}
                        }
                    }
                }
            }
            form method="post" action={ "/devis/" (id) "/situation" } .actions {
                input type="number" name="pct" step="0.01" min="0.01" max="100"
                      placeholder="%" class="champ-court" aria-label="Pourcentage" required;
                input type="date" name="date" aria-label="Date de la situation";
                button type="submit" {(crate::langue::tr("Créer la facture de situation")) }
            }
        },
    ))
}

pub(crate) fn table_lignes(
    lignes: impl Iterator<Item = (bool, String, String, Decimal, String, Decimal)>,
    total_ht: Decimal,
    total_tva: Decimal,
    total_ttc: Decimal,
    reste: Option<Decimal>,
) -> Markup {
    html! {
        table {
            thead { tr {
                th {(crate::langue::tr("Désignation")) } th .num {(crate::langue::tr("Qté")) } th .num {(crate::langue::tr("PU HT")) }
                th .num {(crate::langue::tr("TVA")) } th .num {(crate::langue::tr("Total HT")) }
            } }
            tbody {
                @for (section, label, qty, pu, tva, total) in lignes {
                    @if section {
                        tr { td colspan="5" style="background:#f6f4f0;font-weight:600" { (label) } }
                    } @else {
                        tr {
                            td { (label) }
                            td .num { (qty) }
                            td .num { (ui::euros(pu)) }
                            td .num { (tva) " %" }
                            td .num { (ui::euros(total)) }
                        }
                    }
                }
            }
            tfoot {
                tr { td colspan="4" .num {(crate::langue::tr("Total HT")) } td .num { (ui::euros(total_ht)) } }
                tr { td colspan="4" .num {(crate::langue::tr("TVA")) } td .num { (ui::euros(total_tva)) } }
                tr { td colspan="4" .num {(crate::langue::tr("Total TTC")) } td .num { (ui::euros(total_ttc)) } }
                @if let Some(r) = reste {
                    tr { td colspan="4" .num {(crate::langue::tr("Reste dû")) } td .num .total-fort { (ui::euros(r)) } }
                }
            }
        }
    }
}

pub(crate) async fn devis_pdf(State(app): State<App>, Path(id): Path<i64>) -> Res<Response> {
    let rendu = app::devis_pdf(&app.pool, entite::courante(), id)?;
    Ok(pdf_response(&rendu.bytes, &format!("devis-{id}.pdf")))
}

pub(crate) async fn devis_situation(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let pct: Decimal = f
        .get("pct")
        .map(|v| v.replace(',', "."))
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Erreur("pourcentage de la situation invalide".into()))?;
    let date = f
        .get("date")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let facture = db::situation::creer(&app.pool, entite::courante(), id, pct, date.as_deref())?;
    Ok(Redirect::to(&format!("/factures/{facture}?ok=situation")))
}

pub(crate) async fn devis_facturer(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let nouveau = db::invoice::create_from_proposal(&app.pool, entite::courante(), id)?;
    if let Some(d) = db::proposal::get(&app.pool, entite::courante(), id)? {
        tracer(
            &app,
            d.fk_soc,
            "propal",
            id,
            &format!("Devis {} facturé", d.ref_),
        );
    }
    Ok(Redirect::to(&format!("/factures/{nouveau}?ok=facture")))
}

pub(crate) async fn devis_nouveau(State(app): State<App>) -> Res<Markup> {
    let clients = db::party::list(&app.pool, entite::courante(), 500, 0)?;
    let tva = db::dict::taux_tva(&app.pool, entite::courante(), 1)?;
    let conditions = db::reglement::conditions(&app.pool, entite::courante())?;
    // Le chantier est facultatif, mais c'est lui qui rend la marge calculable.
    let chantiers = db::projet::list(&app.pool, entite::courante(), 200, 0)?;
    Ok(ui::page(
        "devis",
        "nouveau",
        "Nouveau devis",
        html! {
            h1 {(crate::langue::tr("Nouveau devis")) }
            form method="post" action="/devis/nouveau" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" required class="pleine" {
                                    @for c in &clients {
                                        option value=(c.rowid) { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Chantier")) }
                            td {
                                select name="projet" class="pleine" {
                                    option value="" { (crate::langue::tr("— aucun —")) }
                                    @for c in &chantiers {
                                        option value=(c.rowid) { (c.ref_.clone()) " " (c.title.clone()) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Note interne")) } td { input type="text" name="note" class="pleine"; } }
                        tr {
                            th {(crate::langue::tr("Règlement")) }
                            td {
                                select name="cond" class="pleine" {
                                    @for c in &conditions {
                                        option value=(c.rowid) { (c.libelle_fr()) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Acompte (%)")) } td { input type="number" name="acompte" step="0.01" min="0" max="100" class="champ-court" placeholder="0"; } }
                        tr { th {(crate::langue::tr("Valable jusqu'au")) } td { input type="date" name="validite" class="champ-moyen"; } }
                    }
                }
                h2 {(crate::langue::tr("Lignes")) }
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Designation")) }
                            th .num {(crate::langue::tr("Qte")) }
                            th .num {(crate::langue::tr("PU HT")) }
                            th {(crate::langue::tr("TVA")) }
                        }
                    }
                    tbody {
                        @for i in 1..=4 {
                            tr {
                                td { input type="text" name=(format!("l{i}_lib")) class="pleine"; }
                                td { input type="number" name=(format!("l{i}_qte")) step="0.01" min="0" value="1" class="champ-court"; }
                                td { input type="number" name=(format!("l{i}_pu")) step="0.01" min="0" style="width:120px"; }
                                td {
                                    select name=(format!("l{i}_tva")) {
                                        @for t in &tva {
                                            option value=(t.taux) { (app::fmt_dec(Some(t.taux))) " %" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Creer le devis")) }
                    a .bouton.secondaire href="/devis" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

pub(crate) async fn devis_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let fk_soc = f
        .get("soc")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("client invalide".into()))?;

    let mut lignes = Vec::new();
    for i in 1..=4 {
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
        lignes.push(db::proposal::NewProposalLine::new(lib, qte, pu, tva));
    }
    if lignes.is_empty() {
        return Err(Erreur("au moins une ligne est necessaire".into()));
    }
    // Une ligne a 0 % de TVA doit dire pourquoi (EN 16931, BT-120). Le controle
    // est ici, pas dans la couche donnees : c'est a l'ecran qu'on peut aller
    // corriger le dictionnaire, et c'est ici que le message sert.
    db::dict::exiger_motif_tva_zero(
        &app.pool,
        entite::courante(),
        &lignes
            .iter()
            .map(|l: &db::proposal::NewProposalLine| l.tva_tx)
            .collect::<Vec<_>>(),
    )
    .map_err(|e| Erreur(e.to_string()))?;

    let mut nouveau = db::proposal::NewProposal::new(fk_soc);
    nouveau.note_private = f.get("note").filter(|s| !s.trim().is_empty()).cloned();
    nouveau.fk_cond_reglement = f.get("cond").and_then(|v| v.parse().ok());
    // Le chantier : sans lui, aucune marge analytique et aucune statistique par
    // chantier ne peut exister.
    nouveau.fk_projet = f
        .get("projet")
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v > 0);
    nouveau.fin_validite = f
        .get("validite")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    nouveau.deposit_percent = f
        .get("acompte")
        .filter(|s| !s.trim().is_empty())
        .and_then(|v| v.replace(',', ".").parse().ok());
    nouveau.lines = lignes;
    let id = db::proposal::create(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to(&format!("/devis/{id}")))
}

pub(crate) const NB_LIGNES_DEVIS: usize = 10;

pub(crate) async fn devis_modifier(State(app): State<App>, Path(id): Path<i64>) -> Res<Markup> {
    let p = &app.pool;
    let d =
        db::proposal::get(p, entite::courante(), id)?.ok_or(Erreur("Devis introuvable".into()))?;
    if d.fk_statut != 0 {
        return Err(Erreur(
            "Seul un devis en brouillon peut etre modifie.".into(),
        ));
    }
    let lignes = db::proposal::lines(p, entite::courante(), id)?;
    let clients = db::party::list(p, entite::courante(), 500, 0)?;
    let tva = db::dict::taux_tva(p, entite::courante(), 1)?;
    let conditions = db::reglement::conditions(p, entite::courante())?;

    // Lignes pre-remplies : (section, libelle, qte, pu, tva).
    let mut prefill: Vec<(String, String, String, String, String)> = Vec::new();
    let mut section_courante = String::new();
    for l in &lignes {
        if l.is_section() {
            section_courante = l.label.clone().unwrap_or_default();
            prefill.push((
                section_courante.clone(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ));
        } else {
            prefill.push((
                section_courante.clone(),
                l.label.clone().unwrap_or_default(),
                app::fmt_dec(l.qty),
                app::fmt_dec(l.subprice),
                app::fmt_dec(l.tva_tx),
            ));
        }
    }
    while prefill.len() < NB_LIGNES_DEVIS {
        prefill.push((
            String::new(),
            String::new(),
            "1".into(),
            String::new(),
            String::new(),
        ));
    }

    Ok(ui::page(
        "devis",
        "liste",
        &format!("Modifier {}", d.ref_),
        html! {
            h1 { "Modifier le devis " (d.ref_) }
            p .sous-titre {(crate::langue::tr("Les lignes sont remplacees en bloc. Une ligne sans libelle mais avec un nom de section ouvre une section."))
            }
            form method="post" action={ "/devis/" (id) "/modifier" } {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" required class="pleine" {
                                    @for c in &clients {
                                        option value=(c.rowid) selected[d.fk_soc == Some(c.rowid)] { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Note interne")) } td { input type="text" name="note" value=(d.note_private.clone().unwrap_or_default()) class="pleine"; } }
                        tr {
                            th {(crate::langue::tr("Règlement")) }
                            td {
                                select name="cond" class="pleine" {
                                    @for c in &conditions {
                                        option value=(c.rowid) selected[d.fk_cond_reglement == Some(c.rowid)] { (c.libelle_fr()) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Acompte (%)")) } td { input type="number" name="acompte" step="0.01" min="0" max="100" value=(app::fmt_dec(d.deposit_percent)) class="champ-court"; } }
                        tr { th {(crate::langue::tr("Valable jusqu'au")) } td { input type="date" name="validite" value=(d.fin_validite.clone().unwrap_or_default()) class="champ-moyen"; } }
                    }
                }
                h2 {(crate::langue::tr("Lignes")) }
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Section")) }
                            th {(crate::langue::tr("Designation")) }
                            th .num {(crate::langue::tr("Qte")) }
                            th .num {(crate::langue::tr("PU HT")) }
                            th {(crate::langue::tr("TVA")) }
                        }
                    }
                    tbody {
                        @for (i, (sec, lib, qte, pu, tva_l)) in prefill.iter().enumerate() {
                            tr {
                                td { input type="text" name=(format!("r{}_sec", i + 1)) value=(sec) style="width:130px"; }
                                td { input type="text" name=(format!("r{}_lib", i + 1)) value=(lib) class="pleine"; }
                                td { input type="number" name=(format!("r{}_qte", i + 1)) step="0.01" min="0" value=(qte) style="width:80px"; }
                                td { input type="number" name=(format!("r{}_pu", i + 1)) step="0.01" min="0" value=(pu) class="champ-court"; }
                                td {
                                    select name=(format!("r{}_tva", i + 1)) {
                                        @for t in &tva {
                                            option value=(t.taux) selected[tva_l == &app::fmt_dec(Some(t.taux))] {
                                                (app::fmt_dec(Some(t.taux))) " %"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton.secondaire href={ "/devis/" (id) } {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

pub(crate) async fn devis_enregistrer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let fk_soc = f
        .get("soc")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("client invalide".into()))?;

    let mut lignes = Vec::new();
    for i in 1..=NB_LIGNES_DEVIS {
        let lib = f.get(&format!("r{i}_lib")).map(|s| s.trim()).unwrap_or("");
        if lib.is_empty() {
            continue;
        }
        let section = f
            .get(&format!("r{i}_sec"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let qte: Decimal = f
            .get(&format!("r{i}_qte"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let pu: Decimal = f
            .get(&format!("r{i}_pu"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let tva: Decimal = f
            .get(&format!("r{i}_tva"))
            .and_then(|v| v.replace(',', ".").parse().ok())
            .unwrap_or_default();
        let mut ligne = db::proposal::NewProposalLine::new(lib, qte, pu, tva);
        ligne.section = section;
        lignes.push(ligne);
    }
    if lignes.is_empty() {
        return Err(Erreur("au moins une ligne est necessaire".into()));
    }

    let mut modif = db::proposal::NewProposal::new(fk_soc);
    modif.note_private = f.get("note").filter(|s| !s.trim().is_empty()).cloned();
    modif.fk_cond_reglement = f.get("cond").and_then(|v| v.parse().ok());
    modif.fin_validite = f
        .get("validite")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    modif.deposit_percent = f
        .get("acompte")
        .filter(|s| !s.trim().is_empty())
        .and_then(|v| v.replace(',', ".").parse().ok());
    modif.lines = lignes;
    db::proposal::update(&app.pool, entite::courante(), id, &modif)?;
    Ok(Redirect::to(&format!("/devis/{id}?ok=enregistre")))
}

pub(crate) async fn devis_valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    db::proposal::validate(&app.pool, entite::courante(), id)?;
    if let Some(d) = db::proposal::get(&app.pool, entite::courante(), id)? {
        tracer(
            &app,
            d.fk_soc,
            elementtype_devis(),
            id,
            &format!("Devis {} validé", d.ref_),
        );
        let _ = app::archiver_devis(&app.pool, entite::courante(), id);
    }
    Ok(Redirect::to(&format!("/devis/{id}?ok=valide")))
}

/// Signe un devis : c'est le passage au statut 2, celui qui ouvre la
/// facturation. Dolibarr ne connait que `close` pour cette transition.
pub(crate) async fn devis_signer(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    db::proposal::cloturer(&app.pool, entite::courante(), id, 2)?;
    if let Some(d) = db::proposal::get(&app.pool, entite::courante(), id)? {
        tracer(
            &app,
            d.fk_soc,
            elementtype_devis(),
            id,
            &format!("Devis {} signé", d.ref_),
        );
    }
    Ok(Redirect::to(&format!("/devis/{id}?ok=signe")))
}

/// Marque un devis perdu (statut 3). Il reste consultable.
pub(crate) async fn devis_perdu(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    db::proposal::cloturer(&app.pool, entite::courante(), id, 3)?;
    if let Some(d) = db::proposal::get(&app.pool, entite::courante(), id)? {
        tracer(
            &app,
            d.fk_soc,
            elementtype_devis(),
            id,
            &format!("Devis {} marqué perdu", d.ref_),
        );
    }
    Ok(Redirect::to(&format!("/devis/{id}?ok=perdu")))
}

pub(crate) fn elementtype_devis() -> &'static str {
    "propal"
}

pub(crate) async fn devis_commander(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let commande = db::commande::create_from_proposal(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/commandes/{commande}?ok=cree")))
}
