//! Les tiers : liste, fiche, creation, modification.
//!
//! Un tiers porte deux roles dans la meme table : client, fournisseur, ou les
//! deux (`client = 3`). C'est ce qui explique qu'un fournisseur qui achete chez
//! nous reste un fournisseur, sans creer deux fiches pour la meme entreprise.

use crate::*;

pub(crate) async fn tiers(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let filtre = q.get("q").cloned().unwrap_or_default().to_lowercase();
    let type_ = q.get("type").cloned().unwrap_or_default();
    // `type` vient d'une liste fermee : la condition SQL est choisie ici, jamais
    // construite a partir du texte saisi.
    let listes: Vec<_> = db::divers::tiers_filtres(&app.pool, entite::courante(), &type_, 2000)?
        .into_iter()
        .filter(|p| filtre.is_empty() || p.nom.to_lowercase().contains(&filtre))
        .collect();
    let total_db = db::party::count(&app.pool, entite::courante()).unwrap_or(0);
    let affiches = listes.len();
    let titre = match type_.as_str() {
        "prospect" => "Prospects",
        "fournisseur" => "Fournisseurs",
        "client" => "Clients",
        _ => "Tiers",
    };
    let onglet = match type_.as_str() {
        "prospect" => "prospects",
        "fournisseur" => "fournisseurs",
        _ => "liste",
    };

    Ok(ui::page(
        "tiers",
        onglet,
        titre,
        html! {
            h1 { (titre) }
            p .sous-titre { (affiches) " fiche(s)" (ui::compte_tronque(affiches, total_db)) }
            form method="get" action="/tiers" .actions {
                input type="text" name="q" placeholder="Nom du client" value=(filtre);
                @if !type_.is_empty() {
                    input type="hidden" name="type" value=(type_);
                }
                button type="submit" {(crate::langue::tr("Chercher")) }
            }
            @if listes.is_empty() {
                (ui::vide("Aucun tiers ne correspond.", Some(("/tiers", "Voir tous les tiers"))))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Nom")) } th {(crate::langue::tr("Ville")) } th {(crate::langue::tr("Type")) } } }
                    tbody {
                        @for p in &listes {
                            tr {
                                td .ref { a href={ "/tiers/" (p.rowid) } { (p.code_client.clone().unwrap_or_default()) } }
                                td { a href={ "/tiers/" (p.rowid) } { (p.nom) } }
                                td { (p.town.clone().unwrap_or_default()) }
                                td { (match p.client { 1 => "client", 2 => "prospect", 3 => "client et prospect", _ => "—" }) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn tiers_fiche(State(app): State<App>, Path(id): Path<i64>) -> Res<Markup> {
    let p = &app.pool;
    let t = db::party::get(p, entite::courante(), id)?.ok_or(Erreur("Tiers introuvable".into()))?;
    let jour = aujourdhui(p)?;
    let devis: Vec<_> = db::proposal::list(p, entite::courante(), 500, 0)?
        .into_iter()
        .filter(|d| d.fk_soc == Some(id))
        .collect();
    let factures: Vec<_> = db::invoice::list(p, entite::courante(), 500, 0)?
        .into_iter()
        .filter(|f| f.fk_soc == Some(id))
        .collect();
    let evts_tiers = db::journal::pour_tiers(p, entite::courante(), id, 20)?;

    Ok(ui::page(
        "tiers",
        "liste",
        &t.nom,
        html! {
            h1 { (t.nom) }
            p .sous-titre { "Code client " (t.code_client.clone().unwrap_or_default()) }
            div .actions {
                a .bouton.secondaire href={ "/tiers/" (id) "/modifier" } {(crate::langue::tr("Modifier la fiche")) }
            }

            div .fiche {
                div {
                    h2 style="margin-top:0" {(crate::langue::tr("Coordonnées")) }
                    dl {
                        dt {(crate::langue::tr("Adresse")) }
                        dd { (t.address.clone().unwrap_or_default()) }
                        dt {(crate::langue::tr("Ville")) }
                        dd { (format!("{} {}", t.zip.clone().unwrap_or_default(), t.town.clone().unwrap_or_default()).trim()) }
                        dt {(crate::langue::tr("Email")) }
                        dd { (t.email.clone().unwrap_or_default()) }
                        dt {(crate::langue::tr("Téléphone")) }
                        dd { (t.phone.clone().unwrap_or_default()) }
                    }
                }
                div {
                    h2 style="margin-top:0" {(crate::langue::tr("Identification")) }
                    dl {
                        dt {(crate::langue::tr("SIRET")) }
                        dd { (t.siret.clone().unwrap_or_default()) }
                        dt {(crate::langue::tr("TVA intracommunautaire")) }
                        dd { (t.tva_intra.clone().unwrap_or_default()) }
                    }
                }
            }

            @if !evts_tiers.is_empty() {
                h2 {(crate::langue::tr("Historique")) }
                table {
                    tbody {
                        @for e in &evts_tiers {
                            tr {
                                td .ref { (e.datep.clone().unwrap_or_default()) }
                                td { (e.label) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Devis")) }
            @if devis.is_empty() {
                (ui::vide("Aucun devis pour ce client.", None))
            } @else {
                table {
                    tbody {
                        @for d in &devis {
                            tr {
                                td .ref { a href={ "/devis/" (d.rowid) } { (d.ref_) } }
                                td .num { (ui::euros(d.total_ttc.unwrap_or_default())) }
                                td { (ui::pastille(d.fk_statut, 0, false)) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Factures")) }
            @if factures.is_empty() {
                (ui::vide("Aucune facture pour ce client.", None))
            } @else {
                table {
                    tbody {
                        @for f in &factures {
                            tr {
                                td .ref { a href={ "/factures/" (f.rowid) } { (f.ref_) } }
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

pub(crate) async fn tiers_nouveau(
    State(_app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let mut nom = String::new();
    let mut adresse = String::new();
    let mut cp = String::new();
    let mut ville = String::new();
    let mut siret = String::new();
    let mut retour: Option<String> = None;

    // Remplissage depuis l'annuaire de l'Etat, sans JavaScript : deux GET.
    if let Some(s) = q.get("siret").filter(|s| !s.trim().is_empty()) {
        match annuaire::par_siret(s) {
            Ok(Some(e)) => {
                nom = e.nom.clone();
                adresse = e.adresse.clone();
                cp = e.code_postal.clone();
                ville = e.commune.clone();
                siret = e.siret.clone();
                retour = Some(format!("Fiche remplie depuis l'INSEE : {}", e.nom));
            }
            Ok(None) => retour = Some("Aucune entreprise trouvee pour ce numero.".into()),
            Err(e) => retour = Some(format!("Annuaire indisponible : {e}")),
        }
    }
    if let Some(a) = q.get("adresse").filter(|s| !s.trim().is_empty()) {
        match annuaire::adresses(a, 5) {
            Ok(liste) if !liste.is_empty() => {
                let adr = &liste[0];
                adresse = adr.numero_voie.clone();
                cp = adr.code_postal.clone();
                ville = adr.commune.clone();
                retour = Some(format!("Adresse retenue : {}", adr.libelle));
            }
            Ok(_) => retour = Some("Aucune adresse trouvee.".into()),
            Err(e) => retour = Some(format!("Base Adresse indisponible : {e}")),
        }
    }

    Ok(ui::page(
        "tiers",
        "nouveau",
        "Nouveau tiers",
        html! {
            h1 {(crate::langue::tr("Nouveau tiers")) }
            @if let Some(m) = &retour {
                p .flash { (m) }
            }

            h2 {(crate::langue::tr("Remplir automatiquement")) }
            div .actions {
                form method="get" action="/tiers/nouveau" {
                    input type="text" name="siret" placeholder="SIRET ou SIREN" style="width:190px";
                    button type="submit" {(crate::langue::tr("Chercher l'entreprise")) }
                }
                form method="get" action="/tiers/nouveau" {
                    input type="text" name="adresse" placeholder="17 chemin du Grimpillon, Saint-Cyr" style="width:330px";
                    button type="submit" class="secondaire" {(crate::langue::tr("Chercher l'adresse")) }
                }
            }

            h2 {(crate::langue::tr("Fiche")) }
            form method="post" action="/tiers/nouveau" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" required value=(nom) class="pleine"; } }
                        tr { th {(crate::langue::tr("Adresse")) } td { input type="text" name="adresse" value=(adresse) class="pleine"; } }
                        tr { th {(crate::langue::tr("Code postal")) } td { input type="text" name="cp" value=(cp) style="width:120px"; } }
                        tr { th {(crate::langue::tr("Ville")) } td { input type="text" name="ville" value=(ville) class="pleine"; } }
                        tr { th {(crate::langue::tr("Email")) } td { input type="email" name="email" class="pleine"; } }
                        tr { th {(crate::langue::tr("Telephone")) } td { input type="text" name="tel" class="pleine"; } }
                        tr { th {(crate::langue::tr("SIRET")) } td { input type="text" name="siret" value=(siret) class="pleine"; } }
                        tr { th {(crate::langue::tr("TVA intracommunautaire")) } td { input type="text" name="tva_intra" class="pleine"; } }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Creer le tiers")) }
                    a .bouton.secondaire href="/tiers" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

pub(crate) async fn tiers_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |cle: &str| {
        f.get(cle)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let mut nouveau = db::party::NewParty::client(f.get("nom").map(|s| s.trim()).unwrap_or(""));
    nouveau.address = champ("adresse");
    nouveau.zip = champ("cp");
    nouveau.town = champ("ville");
    nouveau.email = champ("email");
    nouveau.phone = champ("tel");
    nouveau.siret = champ("siret");
    nouveau.tva_intra = champ("tva_intra");
    let id = db::party::create(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to(&format!("/tiers/{id}")))
}

pub(crate) async fn tiers_modifier(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let mut t = db::party::get(&app.pool, entite::courante(), id)?
        .ok_or(Erreur("Tiers introuvable".into()))?;
    let mut retour: Option<String> = None;

    // Même recette que la création : deux GET qui pré-remplissent la fiche.
    if let Some(s) = q.get("siret").filter(|s| !s.trim().is_empty()) {
        match annuaire::par_siret(s) {
            Ok(Some(e)) => {
                t.nom = e.nom;
                t.address = Some(e.adresse).filter(|a| !a.is_empty());
                t.zip = Some(e.code_postal).filter(|a| !a.is_empty());
                t.town = Some(e.commune).filter(|a| !a.is_empty());
                t.siret = Some(e.siret);
                retour = Some("Fiche complétée depuis l'INSEE.".to_string());
            }
            Ok(None) => retour = Some("Aucune entreprise trouvée.".into()),
            Err(e) => retour = Some(format!("Annuaire indisponible : {e}")),
        }
    }
    if let Some(a) = q.get("adresse").filter(|s| !s.trim().is_empty()) {
        match annuaire::adresses(a, 1) {
            Ok(liste) if !liste.is_empty() => {
                let adr = &liste[0];
                t.address = Some(adr.numero_voie.clone());
                t.zip = Some(adr.code_postal.clone());
                t.town = Some(adr.commune.clone());
                retour = Some(format!("Adresse retenue : {}", adr.libelle));
            }
            Ok(_) => retour = Some("Aucune adresse trouvée.".into()),
            Err(e) => retour = Some(format!("Base Adresse indisponible : {e}")),
        }
    }

    Ok(ui::page(
        "tiers",
        "liste",
        &format!("Modifier {}", t.nom),
        html! {
            h1 { "Modifier " (t.nom) }
            p .sous-titre { "Code client " (t.code_client.clone().unwrap_or_default()) }
            @if let Some(m) = &retour {
                p .flash { (m) }
            }

            h2 {(crate::langue::tr("Remplir automatiquement")) }
            div .actions {
                form method="get" action={ "/tiers/" (id) "/modifier" } {
                    input type="text" name="siret" placeholder="SIRET ou SIREN" style="width:190px";
                    button type="submit" {(crate::langue::tr("Chercher l'entreprise")) }
                }
                form method="get" action={ "/tiers/" (id) "/modifier" } {
                    input type="text" name="adresse" placeholder="17 chemin du Grimpillon, Saint-Cyr" style="width:330px";
                    button type="submit" class="secondaire" {(crate::langue::tr("Chercher l'adresse")) }
                }
            }

            form method="post" action={ "/tiers/" (id) "/modifier" } {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" required value=(t.nom) class="pleine"; } }
                        tr { th {(crate::langue::tr("Adresse")) } td { input type="text" name="adresse" value=(t.address.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("Code postal")) } td { input type="text" name="cp" value=(t.zip.clone().unwrap_or_default()) style="width:120px"; } }
                        tr { th {(crate::langue::tr("Ville")) } td { input type="text" name="ville" value=(t.town.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("Email")) } td { input type="email" name="email" value=(t.email.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("Telephone")) } td { input type="text" name="tel" value=(t.phone.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("SIRET")) } td { input type="text" name="siret" value=(t.siret.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("TVA intracommunautaire")) } td { input type="text" name="tva_intra" value=(t.tva_intra.clone().unwrap_or_default()) class="pleine"; } }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton.secondaire href={ "/tiers/" (id) } {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

pub(crate) async fn tiers_enregistrer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |cle: &str| {
        f.get(cle)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let mut modif = db::party::NewParty::client(f.get("nom").map(|s| s.trim()).unwrap_or(""));
    modif.address = champ("adresse");
    modif.zip = champ("cp");
    modif.town = champ("ville");
    modif.email = champ("email");
    modif.phone = champ("tel");
    modif.siret = champ("siret");
    modif.tva_intra = champ("tva_intra");
    db::party::update(&app.pool, entite::courante(), id, &modif)?;
    Ok(Redirect::to(&format!("/tiers/{id}")))
}
