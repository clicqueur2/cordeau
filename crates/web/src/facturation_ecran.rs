//! Les factures : liste, fiche, validation, reglement, relances.
//!
//! Une facture validee recoit sa reference definitive et ne bouge plus. Un
//! reglement ne modifie pas la facture : il s'y ajoute, et c'est la somme des
//! reglements qui dit si elle est payee.

use crate::*;

pub(crate) async fn factures(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let jour = aujourdhui(p)?;
    let statut = q.get("statut").cloned().unwrap_or_default();
    let filtre = q
        .get("q")
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();

    let toutes = db::invoice::list(p, entite::courante(), 200, 0)?;
    let total = toutes.len();
    let listes: Vec<_> = toutes
        .into_iter()
        .filter(|f| {
            // Le statut vient d'une liste fermee : aucune valeur saisie n'arrive
            // jusqu'ici, donc pas de condition composee a partir du texte.
            let par_statut = match statut.as_str() {
                "brouillon" => f.fk_statut == 0,
                "validee" => f.fk_statut == 1 && f.paye == 0,
                "payee" => f.paye == 1,
                _ => true,
            };
            let par_texte = filtre.is_empty() || f.ref_.to_lowercase().contains(&filtre);
            par_statut && par_texte
        })
        .collect();

    Ok(ui::page(
        "factures",
        "liste",
        "Factures",
        html! {
            h1 {(crate::langue::tr("Factures")) }
            form method="get" action="/factures" {
                div .actions {
                    select name="statut" aria-label="Statut" {
                        option value="" selected[statut.is_empty()] {(crate::langue::tr("Toutes")) }
                        option value="brouillon" selected[statut == "brouillon"] {(crate::langue::tr("Brouillons")) }
                        option value="validee" selected[statut == "validee"] {(crate::langue::tr("Validées, non payées")) }
                        option value="payee" selected[statut == "payee"] {(crate::langue::tr("Payées")) }
                    }
                    input type="search" name="q" value=(filtre) placeholder="Référence…" aria-label="Recherche";
                    button type="submit" class="petit" {(crate::langue::tr("Filtrer")) }
                    @if !statut.is_empty() || !filtre.is_empty() {
                        a .bouton.secondaire.petit href="/factures" {(crate::langue::tr("Tout voir")) }
                    }
                }
            }
            p .sous-titre {
                (listes.len()) " facture(s)"
                @if listes.len() != total {
                    " sur " (total.to_string())
                }
            }
            form id="actions-factures" method="post" action="/factures/actions" {}
            div .actions {
                button type="submit" form="actions-factures" name="action" value="valider" {(crate::langue::tr("Valider la sélection")) }
                span style="color:#78736c;font-size:13.5px" {(crate::langue::tr("Seules les factures en brouillon sont cochables."))
                }
            }
            @if listes.is_empty() {
                (ui::vide("Aucune facture.", Some(("/devis", "Partir d'un devis"))))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Client")) } th {(crate::langue::tr("Échéance")) } th .num {(crate::langue::tr("TTC")) } th .num {(crate::langue::tr("Reste dû")) } th {(crate::langue::tr("Statut")) } } }
                    tbody {
                        @for f in &listes {
                            tr {
                                td .ref {
                                    @if f.fk_statut == 0 {
                                        input type="checkbox" name={"id_" (f.rowid)}
                                              value="1" form="actions-factures"
                                              aria-label="sélectionner cette facture";
                                        " "
                                    }
                                    a href={ "/factures/" (f.rowid) } { (f.ref_) }
                                }
                                td { (nom_tiers(p, f.fk_soc)?) }
                                td { (f.date_lim_reglement.clone().unwrap_or_default()) }
                                td .num { (ui::euros(f.total_ttc.unwrap_or_default())) }
                                td .num { (ui::euros(db::payment::balance(p, entite::courante(), f.rowid)?.reste)) }
                                td { (ui::pastille(f.fk_statut, f.paye, en_retard(f.date_lim_reglement.as_deref(), f.fk_statut, f.paye, &jour))) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn factures_fiche(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let jour = aujourdhui(p)?;
    let f =
        db::invoice::get(p, entite::courante(), id)?.ok_or(Erreur("Facture introuvable".into()))?;
    let lignes = db::invoice::lines(p, entite::courante(), id)?;
    let solde = db::payment::balance(p, entite::courante(), id)?;
    let reglements = db::payment::list_for_invoice(p, entite::courante(), id)?;
    let docs = db::ged::pour_element(p, entite::courante(), "facture", id)?;
    let evts = db::journal::pour_element(p, entite::courante(), "facture", id)?;
    let reglement =
        db::reglement::condition(p, entite::courante(), f.fk_cond_reglement.unwrap_or(0))
            .ok()
            .flatten()
            .map(|c| c.libelle_fr())
            .filter(|l| !l.trim().is_empty());

    // Le courriel propose : l'adresse du tiers, un objet qui porte la reference,
    // et un corps court. L'utilisateur garde la main, mais il n'a rien a taper.
    let courriel_tiers = match f.fk_soc {
        Some(soc) => db::party::get(p, entite::courante(), soc)?
            .and_then(|t| t.email)
            .unwrap_or_default(),
        None => String::new(),
    };
    // Les modeles de la famille « envoi de facture » : si l'utilisateur en a
    // choisi un, il remplace le message par defaut, variables substituees.
    let entity = entite::courante();
    let modeles = db::modeles::pour(p, entity, "invoice_send")?;
    let choisi = q
        .get("modele")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .and_then(|id| modeles.iter().find(|m| m.rowid == id));
    let client_nom = nom_tiers(p, f.fk_soc)?;

    let (courriel_sujet, courriel_corps) = match choisi {
        Some(m) => {
            let variables = [
                ("REF", f.ref_.clone()),
                ("CLIENT", client_nom.clone()),
                ("TOTAL_TTC", ui::euros(f.total_ttc.unwrap_or_default())),
                (
                    "TOTAL_HT",
                    f.total_ht.unwrap_or_default().round_dp(2).to_string(),
                ),
                ("ECHEANCE", f.date_lim_reglement.clone().unwrap_or_default()),
                ("SOCIETE", db::company::get(p, entity)?.nom),
            ];
            (
                db::modeles::substituer(&m.sujet, &variables, &|cle| crate::langue::tr(cle)),
                db::modeles::substituer(&m.corps, &variables, &|cle| crate::langue::tr(cle)),
            )
        }
        None => (
            format!("Votre facture {} — Pierre Angulaire", f.ref_),
            format!(
                "Bonjour,\n\nVeuillez trouver ci-joint la facture {} d'un montant de {} €.\n\n\
                 Bien cordialement,\n",
                f.ref_,
                f.total_ttc.unwrap_or_default().round_dp(2)
            ),
        ),
    };

    Ok(ui::page(
        "factures",
        "liste",
        &f.ref_,
        html! {
            (message(q.get("ok")))
            h1 { (if f.type_ == 2 { "Avoir " } else if f.type_ == db::invoice::TYPE_SITUATION { "Facture de situation " } else { "Facture " }) (f.ref_) " " (ui::pastille(f.fk_statut, f.paye, en_retard(f.date_lim_reglement.as_deref(), f.fk_statut, f.paye, &jour))) }
            p .sous-titre { "Client : " strong { (nom_tiers(p, f.fk_soc)?) }
                @if let Some(e) = &f.date_lim_reglement { " — échéance " (e) }
                @if let Some(r) = &reglement { " — règlement " (r) } }

            (table_lignes(
                lignes.iter().map(|l| (
                    l.is_section(),
                    l.label.clone().unwrap_or_default(),
                    app::fmt_dec(l.qty),
                    l.subprice.unwrap_or_default(),
                    app::fmt_dec(l.tva_tx),
                    l.total_ht.unwrap_or_default(),
                )),
                f.total_ht.unwrap_or_default(),
                f.total_tva.unwrap_or_default(),
                f.total_ttc.unwrap_or_default(),
                Some(solde.reste),
            ))

            div .actions {
                a .bouton.secondaire href={ "/factures/" (id) "/pdf" } {(crate::langue::tr("Télécharger le PDF")) }
                @if f.est_brouillon() {
                    form method="post" action={ "/factures/" (id) "/valider" } {
                        button type="submit" {(crate::langue::tr("Valider la facture")) }
                    }
                }
                @if f.fk_statut >= 1 && f.type_ != 2 {
                    form method="post" action={ "/factures/" (id) "/avoir" } {
                        button type="submit" class="secondaire" {(crate::langue::tr("Créer un avoir")) }
                    }
                }
            }

            @if !f.est_brouillon() {
                h2 {(crate::langue::tr("Envoyer par e-mail")) }
                @if !modeles.is_empty() {
                    form method="get" action={ "/factures/" (id) } {
                        div .actions {
                            select name="modele" aria-label=(crate::langue::tr("Modèle d'e-mail")) {
                                option value="" { (crate::langue::tr("Message par défaut")) }
                                @for m in &modeles {
                                    option value=(m.rowid)
                                           selected[choisi.map(|c| c.rowid) == Some(m.rowid)] {
                                        (m.libelle.clone())
                                    }
                                }
                            }
                            button type="submit" class="petit secondaire" {
                                (crate::langue::tr("Appliquer le modèle"))
                            }
                        }
                    }
                }
                form method="post" action={ "/factures/" (id) "/envoyer" } {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Destinataire")) }
                                td {
                                    input type="email" name="destinataire" value=(courriel_tiers)
                                          required style="width:300px";
                                }
                            }
                            tr {
                                th {(crate::langue::tr("Objet")) }
                                td { input type="text" name="sujet" value=(courriel_sujet) style="width:100%"; }
                            }
                            tr {
                                th {(crate::langue::tr("Message")) }
                                td { textarea name="corps" rows="5" style="width:100%" { (courriel_corps) } }
                            }
                        }
                    }
                    div .actions {
                        button type="submit" {(crate::langue::tr("Envoyer la facture")) }
                        span style="color:#78736c;font-size:13.5px" {(crate::langue::tr("Le PDF archivé part en pièce jointe. L'envoi est tracé."))
                        }
                    }
                }
            }

            @if !f.est_brouillon() {
                h2 {(crate::langue::tr("Facture récurrente")) }
                form method="post" action={ "/factures/" (id) "/recurrence" } {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Périodicité")) }
                                td {
                                    select name="periode" aria-label="Périodicité" {
                                        option value="mois" {(crate::langue::tr("Tous les mois")) }
                                        option value="trimestre" {(crate::langue::tr("Tous les trimestres")) }
                                        option value="semaine" {(crate::langue::tr("Toutes les semaines")) }
                                        option value="an" {(crate::langue::tr("Tous les ans")) }
                                    }
                                }
                                th {(crate::langue::tr("Prochaine échéance")) }
                                td { input type="date" name="prochaine" required; }
                                th {(crate::langue::tr("Nombre")) }
                                td {
                                    input type="number" name="maximum" min="0" value="0"
                                          title="0 = sans fin";
                                }
                            }
                        }
                    }
                    div .actions {
                        button type="submit" {(crate::langue::tr("Créer la série")) }
                        span style="color:#78736c;font-size:13.5px" {(crate::langue::tr("La série est une copie figée : modifier cette facture ne la change pas."))
                        }
                    }
                }
            }

            @if !solde.soldee && !f.est_brouillon() {
                h2 {(crate::langue::tr("Enregistrer un règlement")) }
                form method="post" action={ "/factures/" (id) "/reglement" } .actions {
                    input type="number" name="montant" step="0.01" min="0.01"
                          value=(solde.reste.to_string()) style="width:150px" aria-label="Montant";
                    select name="mode" aria-label="Mode de règlement" {
                        option value="virement" {(crate::langue::tr("Virement")) }
                        option value="cheque" {(crate::langue::tr("Chèque")) }
                        option value="especes" {(crate::langue::tr("Espèces")) }
                        option value="carte" {(crate::langue::tr("Carte")) }
                    }
                    button type="submit" {(crate::langue::tr("Encaisser")) }
                }
            }

            @if !docs.is_empty() {
                h2 {(crate::langue::tr("Documents archivés")) }
                table {
                    tbody {
                        @for (dossier, nom) in &docs {
                            tr {
                                td .ref { (nom) }
                                td { (dossier) }
                            }
                        }
                    }
                }
            }

            @if !evts.is_empty() {
                h2 {(crate::langue::tr("Historique")) }
                table {
                    tbody {
                        @for e in &evts {
                            tr {
                                td .ref { (e.datep.clone().unwrap_or_default()) }
                                td { (e.label) }
                            }
                        }
                    }
                }
            }

            @if !reglements.is_empty() {
                h2 {(crate::langue::tr("Règlements")) }
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("Montant")) } } }
                    tbody {
                        @for r in &reglements {
                            tr {
                                td .ref { (r.ref_) }
                                td { (r.datep.clone().unwrap_or_default()) }
                                td .num { (ui::euros(r.amount.unwrap_or_default())) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn factures_pdf(State(app): State<App>, Path(id): Path<i64>) -> Res<Response> {
    let rendu = app::facture_pdf(&app.pool, entite::courante(), id)?;
    Ok(pdf_response(&rendu.bytes, &format!("facture-{id}.pdf")))
}

pub(crate) async fn factures_avoir(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    let avoir = db::invoice::create_avoir(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/factures/{avoir}?ok=avoir")))
}

pub(crate) async fn factures_valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    db::invoice::validate(&app.pool, entite::courante(), id)?;
    if let Some(f) = db::invoice::get(&app.pool, entite::courante(), id)? {
        tracer(
            &app,
            f.fk_soc,
            "facture",
            id,
            &format!("Facture {} validée", f.ref_),
        );
        // Le PDF part dans la GED : un document non archivé n'est pas une pièce.
        let _ = app::archiver_facture(&app.pool, entite::courante(), id);

        // L'ecriture comptable, dans la table que Dolibarr lit. Une erreur ici
        // ne defait pas la validation : la facture est validee, et le message
        // dit que la comptabilite n'a pas suivi — un echec silencieux laisserait
        // un grand livre incomplet sans que personne ne le sache.
        if let Err(e) =
            db::persist_ecritures::ecrire_facture_vente(&app.pool, entite::courante(), id)
        {
            eprintln!("[cordeau] ecriture comptable de la facture {id} impossible : {e}");
        }
    }
    Ok(Redirect::to(&format!("/factures/{id}?ok=validee")))
}

#[derive(serde::Deserialize)]
pub(crate) struct FormulaireReglement {
    montant: String,
    mode: String,
}

pub(crate) async fn factures_reglement(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(form): Form<FormulaireReglement>,
) -> Res<Redirect> {
    let montant = form
        .montant
        .replace(',', ".")
        .trim_matches('"')
        .parse::<Decimal>()
        .map_err(|_| Erreur("montant invalide".into()))?;
    let mode = match form.mode.as_str() {
        "virement" => db::payment::MODE_VIREMENT,
        "cheque" => db::payment::MODE_CHEQUE,
        "especes" => db::payment::MODE_ESPECES,
        "carte" => db::payment::MODE_CARTE,
        _ => db::payment::MODE_VIREMENT,
    };
    db::payment::create(
        &app.pool,
        entite::courante(),
        &db::payment::NewPayment {
            fk_facture: id,
            montant,
            fk_paiement: mode,
            ..Default::default()
        },
    )?;
    if let Some(f) = db::invoice::get(&app.pool, entite::courante(), id)? {
        tracer(
            &app,
            f.fk_soc,
            "facture",
            id,
            &format!(
                "Règlement de {} sur la facture {}",
                app::fmt_dec(Some(montant)),
                f.ref_
            ),
        );
    }
    Ok(Redirect::to(&format!("/factures/{id}?ok=reglement")))
}

/// Cree une serie de factures recurrentes depuis une facture validee.
pub(crate) async fn factures_recurrence(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let entity = entite::courante();
    let champ = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let prochaine = champ("prochaine");
    let frequence: i32 = champ("frequence").parse().unwrap_or(1);
    let maximum: i32 = champ("maximum").parse().unwrap_or(0);
    // La periodicite vient d'une liste fermee : on choisit l'unite ici, on ne la
    // compose jamais a partir du texte saisi.
    let (freq, unite) = match champ("periode").as_str() {
        "semaine" => (1, "w"),
        "mois" => (frequence.max(1), "m"),
        "trimestre" => (3, "m"),
        "an" => (1, "y"),
        autre => return Err(Erreur(format!("periodicite inconnue : « {autre} »"))),
    };

    let id_serie = db::recurrence::depuis_facture(
        &app.pool,
        entity,
        id,
        "",
        &prochaine,
        (freq, unite),
        maximum,
    )?;

    tracer(
        &app,
        None,
        "facture_rec",
        id_serie,
        &format!("Série de factures créée depuis la facture {id}"),
    );

    Ok(Redirect::to(&format!("/factures/{id}?ok=serie")))
}

pub(crate) async fn factures_nouveau(State(app): State<App>) -> Res<Markup> {
    let clients = db::party::list(&app.pool, entite::courante(), 500, 0)?;
    // Le chantier est ce qui rend la marge par affaire calculable. Sans lui,
    // une facture ne se range dans aucun projet et les statistiques par
    // chantier restent vides.
    let chantiers = db::projet::list(&app.pool, entite::courante(), 500, 0).unwrap_or_default();
    let tva = db::dict::taux_tva(&app.pool, entite::courante(), 1)?;
    let conditions = db::reglement::conditions(&app.pool, entite::courante())?;
    Ok(ui::page(
        "factures",
        "nouveau",
        "Nouvelle facture",
        html! {
            h1 {(crate::langue::tr("Nouvelle facture")) }
            p .sous-titre {(crate::langue::tr("Sans devis préalable. La facture est créée en brouillon.")) }
            form method="post" action="/factures/nouveau" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" required style="width:100%" {
                                    @for c in &clients {
                                        option value=(c.rowid) { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Bon de commande")) } td { input type="text" name="ref_client" style="width:100%"; } }
                        tr {
                            th {(crate::langue::tr("Chantier")) }
                            td {
                                select name="projet" style="width:100%" {
                                    option value="0" { (crate::langue::tr("Aucun")) }
                                    @for c in &chantiers {
                                        option value=(c.rowid) { (c.title.clone()) }
                                    }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Règlement")) }
                            td {
                                select name="cond" style="width:100%" {
                                    @for c in &conditions {
                                        option value=(c.rowid) { (c.libelle_fr()) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Date de la prestation")) } td { input type="date" name="date_prestation" style="width:170px"; } }
                        tr { th {(crate::langue::tr("Note interne")) } td { input type="text" name="note" style="width:100%"; } }
                    } }
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
                        @for i in 1..=6 {
                            tr {
                                td { input type="text" name=(format!("l{i}_lib")) style="width:100%"; }
                                td { input type="number" name=(format!("l{i}_qte")) step="0.01" min="0" value="1" style="width:90px"; }
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
                    button type="submit" {(crate::langue::tr("Créer la facture")) }
                    a .bouton.secondaire href="/factures" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

pub(crate) async fn factures_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let fk_soc = f
        .get("soc")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("client invalide".into()))?;

    let mut lignes = Vec::new();
    for i in 1..=6 {
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
        lignes.push(db::invoice::NewInvoiceLine::new(lib, qte, pu, tva));
    }
    if lignes.is_empty() {
        return Err(Erreur("au moins une ligne est nécessaire".into()));
    }

    let mut nouvelle = db::invoice::NewInvoice::new(fk_soc);
    nouvelle.note_private = f.get("note").filter(|s| !s.trim().is_empty()).cloned();
    nouvelle.fk_cond_reglement = f.get("cond").and_then(|v| v.parse().ok());
    nouvelle.ref_client = f
        .get("ref_client")
        .filter(|s| !s.trim().is_empty())
        .cloned();
    nouvelle.date_prestation = f
        .get("date_prestation")
        .filter(|s| !s.trim().is_empty())
        .cloned();
    // `0` veut dire « aucun chantier » : on ne pose pas un lien vers le projet
    // zero, qui n'existe pas.
    nouvelle.fk_projet = f
        .get("projet")
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0);
    nouvelle.lines = lignes;
    let id = db::invoice::create(&app.pool, entite::courante(), &nouvelle)?;
    Ok(Redirect::to(&format!("/factures/{id}")))
}

pub(crate) async fn relances(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let listes = db::relance::en_retard(p, entite::courante())?;
    let montant = db::relance::total(&listes);

    Ok(ui::page(
        "relances",
        "liste",
        "Relances",
        html! {
            h1 {(crate::langue::tr("Relances")) }
            p .sous-titre {
                (listes.len()) " facture(s) échue(s) — " (ui::euros(montant)) " à relancer"
            }
            @if listes.is_empty() {
                (ui::vide("Aucune facture en retard. Rien à relancer.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Facture")) }
                            th {(crate::langue::tr("Client")) }
                            th {(crate::langue::tr("Échéance")) }
                            th .num {(crate::langue::tr("Retard")) }
                            th .num {(crate::langue::tr("Reste dû")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for r in &listes {
                            tr {
                                td .ref { a href={ "/factures/" (r.fk_facture) } { (r.reference) } }
                                td { (r.client) }
                                td { (r.echeance.clone().unwrap_or_default()) }
                                td .num { (r.jours_retard) " j" }
                                td .num { (ui::euros(r.reste)) }
                                td {
                                    @if let Some(mail) = &r.email {
                                        a .bouton.petit.secondaire
                                          href={ "mailto:" (mail) "?subject=Relance facture " (r.reference)
                                                 "&body=" (urlencode(&db::relance::texte(r))) }
                                          {(crate::langue::tr("Préparer le mail")) }
                                    } @else {
                                        span .vide {(crate::langue::tr("pas d'e-mail")) }
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
