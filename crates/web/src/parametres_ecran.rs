//! Parametres : societe, modules, dictionnaires, base.
//!
//! Tout ce qui se regle une fois et s'oublie. Le point commun de ces ecrans est
//! qu'ils ecrivent dans `llx_const` ou dans un dictionnaire : une valeur fausse
//! ici se propage partout, sans message d'erreur.

use crate::*;

pub(crate) async fn parametres(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let mut s = db::company::get(&app.pool, entite::courante())?;
    let mut retour: Option<String> = None;

    // Meme recette que pour les tiers : l'annuaire de l'Etat pre-remplit la fiche.
    if let Some(num) = q.get("siret").filter(|v| !v.trim().is_empty()) {
        match annuaire::par_siret(num) {
            Ok(Some(e)) => {
                s.nom = e.nom;
                s.adresse = Some(e.adresse).filter(|a| !a.is_empty());
                s.code_postal = Some(e.code_postal).filter(|a| !a.is_empty());
                s.ville = Some(e.commune).filter(|a| !a.is_empty());
                s.siret = Some(e.siret);
                retour = Some("Fiche completee depuis l'INSEE.".to_string());
            }
            Ok(None) => retour = Some("Aucune entreprise trouvee.".into()),
            Err(e) => retour = Some(format!("Annuaire indisponible : {e}")),
        }
    }
    if let Some(a) = q.get("adresse").filter(|v| !v.trim().is_empty()) {
        match annuaire::adresses(a, 1) {
            Ok(liste) if !liste.is_empty() => {
                let adr = &liste[0];
                s.adresse = Some(adr.numero_voie.clone());
                s.code_postal = Some(adr.code_postal.clone());
                s.ville = Some(adr.commune.clone());
                retour = Some(format!("Adresse retenue : {}", adr.libelle));
            }
            Ok(_) => retour = Some("Aucune adresse trouvee.".into()),
            Err(e) => retour = Some(format!("Base Adresse indisponible : {e}")),
        }
    }

    Ok(ui::page(
        "parametres",
        "societe",
        "Paramètres — société",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Société émettrice")) }
            p .sous-titre {(crate::langue::tr("Ces informations alimentent l'en-tête et les mentions de tous les documents.")) }

            @if let Some(m) = &retour {
                p .flash { (m) }
            }

            div .actions {
                form method="get" action="/parametres" {
                    input type="text" name="siret" placeholder="SIRET ou SIREN".champ-moyen;
                    button type="submit" {(crate::langue::tr("Chercher l'entreprise")) }
                }
                form method="get" action="/parametres" {
                    input type="text" name="adresse" placeholder="17 chemin du Grimpillon, Saint-Cyr".champ-large;
                    button type="submit" class="secondaire" {(crate::langue::tr("Chercher l'adresse")) }
                }
            }

            form method="post" action="/parametres/societe" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Denomination")) } td { input type="text" name="nom" value=(s.nom) class="pleine"; } }
                        tr { th {(crate::langue::tr("Forme juridique")) } td { input type="text" name="forme" value=(s.forme_juridique.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("Capital")) } td { input type="text" name="capital" value=(s.capital.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("RCS")) } td { input type="text" name="rcs" value=(s.rcs.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("Adresse")) } td { input type="text" name="adresse" value=(s.adresse.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("Code postal")) } td { input type="text" name="cp" value=(s.code_postal.clone().unwrap_or_default()).champ-court; } }
                        tr { th {(crate::langue::tr("Ville")) } td { input type="text" name="ville" value=(s.ville.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("SIRET")) } td { input type="text" name="siret" value=(s.siret.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("TVA intracommunautaire")) } td { input type="text" name="tva_intra" value=(s.tva_intra.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("Email")) } td { input type="text" name="email" value=(s.email.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("IBAN")) } td { input type="text" name="iban" value=(s.iban.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("BIC")) } td { input type="text" name="bic" value=(s.bic.clone().unwrap_or_default()).champ-large; } }
                        tr { th {(crate::langue::tr("Assurance décennale (assureur)")) } td { input type="text" name="assurance_assureur" value=(s.assurance_assureur.clone().unwrap_or_default()) class="pleine"; } }
                        tr { th {(crate::langue::tr("Assurance (n° de police)")) } td { input type="text" name="assurance_police" value=(s.assurance_police.clone().unwrap_or_default()).champ-large; } }
                        tr { th {(crate::langue::tr("Assurance (couverture)")) } td { input type="text" name="assurance_couverture" value=(s.assurance_couverture.clone().unwrap_or_default()) class="pleine"; } }
                        tr {
                            th {(crate::langue::tr("TVA taux réduit (travaux)")) }
                            td {
                                select name="tva_taux_reduit" {
                                    option value="0" selected[!s.tva_taux_reduit] {(crate::langue::tr("Non")) }
                                    option value="1" selected[s.tva_taux_reduit] {(crate::langue::tr("Oui — attestation 279-0 bis imprimée")) }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("TVA sur les encaissements")) }
                            td {
                                select name="tva_encaissements" {
                                    option value="0" selected[!s.tva_sur_encaissements] {(crate::langue::tr("Non — TVA sur les débits")) }
                                    option value="1" selected[s.tva_sur_encaissements] {(crate::langue::tr("Oui — TVA exigible sur les encaissements")) }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Conditions générales de vente")) }
                            td {
                                textarea name="cgv" rows="6".pleine {
                                    (s.cgv.clone().unwrap_or_default())
                                }
                                p.aide.petite.sous-champ {(crate::langue::tr("Imprimées en bas de chaque devis et facture. Une ligne fait un paragraphe."))
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Delai de paiement (jours)")) } td { input type="number" name="delai" value=(s.delai_paiement_jours) class="champ-court"; } }
                        tr {
                            th {(crate::langue::tr("Regime de TVA")) }
                            td {
                                select name="franchise" {
                                    option value="0" selected[!s.franchise_en_base] {(crate::langue::tr("Assujetti")) }
                                    option value="1" selected[s.franchise_en_base] {(crate::langue::tr("Franchise en base (art. 293 B)")) }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Enregistrer")) } }
            }
            form method="post" action="/parametres/cgv-modele" .actions {
                button type="submit" class="secondaire" {(crate::langue::tr("Insérer un modèle de CGV")) }
            }
        },
    ))
}

/// Modules / applications : catalogue du cœur, activation par constante
/// Dolibarr (`MAIN_MODULE_*` dans `llx_const`).
pub(crate) async fn parametres_modules(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let actifs = db::module::actifs(&app.pool, entite::courante())?;
    let familles: [(&str, &str); 11] = [
        ("base", "Base"),
        ("crm", "Commercial et relation client"),
        ("financial", "Financier"),
        ("products", "Produits et stocks"),
        ("projects", "Projets et chantiers"),
        ("srm", "Achats et fournisseurs"),
        ("hr", "Ressources humaines"),
        ("ecm", "Documents"),
        ("portal", "Portails et points de vente"),
        ("interface", "Interfaces et connecteurs"),
        ("technic", "Technique"),
    ];

    Ok(ui::page(
        "parametres",
        "modules",
        "Modules et applications",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Modules et applications")) }
            p .sous-titre {
                "L'activation s'écrit dans les constantes Dolibarr ("
                code { "llx_const" }
                "), pas dans une table propre : Dolibarr PHP lit le même état."
            }

            @for (cle, libelle) in familles {
                @let liste: Vec<_> = db::module::CATALOGUE.iter().filter(|m| m.famille == cle).collect();
                @if !liste.is_empty() {
                    h2 { (libelle) }
                    table {
                        thead { tr { th {(crate::langue::tr("Module")) } th {(crate::langue::tr("Nom technique")) } th {(crate::langue::tr("Statut")) } th { "" } } }
                        tbody {
                            @for m in liste {
                                @let actif = actifs.contains(&m.code.to_uppercase());
                                tr {
                                    td {
                                        strong { (m.nom) }
                                        @if m.porte {
                                            " "
                                            span .pastille.p-payee.minuscule {(crate::langue::tr("porté")) }
                                        }
                                    }
                                    td .ref { (m.code) }
                                    td {
                                        @if actif {
                                            span .pastille.p-validee {(crate::langue::tr("activé")) }
                                        } @else {
                                            span .pastille.p-brouillon { "inactif" }
                                        }
                                    }
                                    td .num {
                                        @if let Some(ecran) = plugin::par_code(m.code).and_then(|x| x.ecran) {
                                            a .bouton.petit.secondaire.ecart-droit href=(ecran) {(crate::langue::tr("Ouvrir")) }
                                        }
                                        @if actif {
                                            @if plugin::par_code(m.code).is_some() {
                                                a .bouton.petit.secondaire.ecart-droit href={ "/parametres/module/" (m.code) } title="Options du module" {(crate::langue::tr("Options")) }
                                            }
                                        }
                                        form method="post" action="/parametres/modules" class="en-ligne" {
                                            input type="hidden" name="code" value=(m.code);
                                            input type="hidden" name="actif" value=(if actif { "0" } else { "1" });
                                            button type="submit" class="secondaire petit" {
                                                (if actif { "Désactiver" } else { "Activer" })
                                            }
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

pub(crate) async fn parametres_module_basculer(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let code = f
        .get("code")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("module non précisé".into()))?;
    let actif = f.get("actif").map(|v| v == "1").unwrap_or(false);
    db::module::definir_actif(&app.pool, entite::courante(), &code, actif)?;
    menu_actif::charger(&app.pool, entite::courante())?;
    // Un module qui s'active change ce que l'ERP sait faire : cela se consigne.
    let _ = db::events::enregistrer(
        &app.pool,
        &db::events::Nouvel {
            type_: if actif {
                db::events::MODULE_ACTIVE
            } else {
                db::events::MODULE_DESACTIVE
            },
            entity: entite::courante(),
            fk_user: None,
            description: &format!(
                "module {code} {}",
                if actif { "active" } else { "desactive" }
            ),
            ip: &auth::ip_client(&headers),
            agent: auth::agent_client(&headers).as_deref(),
            fk_object: None,
            methode: None,
        },
    );
    Ok(Redirect::to("/parametres/modules?ok=module"))
}

/// Insère un modèle de CGV dans les paramètres, pour ne pas partir d'une page
/// blanche. Le texte reste modifiable ensuite.
pub(crate) async fn parametres_cgv_modele(State(app): State<App>) -> Res<Redirect> {
    let mut s = db::company::get(&app.pool, entite::courante())?;
    s.cgv = Some(
        "Devis valable 30 jours ; la signature vaut acceptation des présentes conditions.\n\
         Paiement par virement sur le compte indiqué ci-dessous.\n\
         Les ouvrages sont garantis selon les garanties légales : parfait achèvement, \
         bon fonctionnement et garantie décennale.\n\
         Toute réclamation doit être adressée par écrit dans les huit jours suivant la réception."
            .to_string(),
    );
    db::company::save(&app.pool, &s)?;
    Ok(Redirect::to("/parametres?ok=cgv"))
}

pub(crate) async fn parametres_dictionnaires(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let unites = db::dict::unites(p)?;
    let tva = db::dict::taux_tva(p, entite::courante(), 1)?;
    let modes = db::dict::modes_paiement(p)?;
    let conditions = db::reglement::conditions(p, entite::courante())?;
    let recensement = db::dict::recensement(p)?;

    Ok(ui::page(
        "parametres",
        "dictionnaires",
        "Paramètres — dictionnaires",
        html! {
            h1 {(crate::langue::tr("Dictionnaires")) }
            p .sous-titre {(crate::langue::tr("Lus directement dans la base Dolibarr, jamais recopiés. Ajouter une entrée dans Dolibarr la rend disponible ici.")) }

            h2 {(crate::langue::tr("Taux de TVA")) }
            table {
                thead { tr { th {(crate::langue::tr("Taux")) } th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Note")) } } }
                tbody {
                    @for t in &tva {
                        tr { td .num { (app::fmt_dec(Some(t.taux))) " %" } td { (t.code) } td { (t.libelle) } }
                    }
                }
            }

            h2 {(crate::langue::tr("Conditions de règlement")) }
            table {
                thead { tr { th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Libellé client")) } th .num {(crate::langue::tr("Délai")) } th { "" } } }
                tbody {
                    @for c in &conditions {
                        tr {
                            td .ref { (c.code) }
                            td { (c.libelle_fr()) }
                            td .num { (c.nbjour) " j" }
                            td {
                                form method="post" action="/parametres/conditions-reglement/desactiver" {
                                    input type="hidden" name="id" value=(c.rowid);
                                    button type="submit" class="secondaire petit" {(crate::langue::tr("Désactiver")) }
                                }
                            }
                        }
                    }
                }
            }

            h3 {(crate::langue::tr("Nouvelle condition")) }
            form method="post" action="/parametres/conditions-reglement" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Code")) } td { input type="text" name="code" required placeholder="RECEPTION".champ-moyen; } }
                        tr { th {(crate::langue::tr("Libellé client")) } td { input type="text" name="libelle" required placeholder="Règlement à réception de facture, par virement" class="pleine"; } }
                        tr { th {(crate::langue::tr("Délai (jours)")) } td { input type="number" name="nbjour" min="0" value="0" class="champ-court"; } }
                        tr {
                            th {(crate::langue::tr("Fin de mois")) }
                            td {
                                select name="fin_de_mois" {
                                    option value="0" {(crate::langue::tr("Non")) }
                                    option value="1" {(crate::langue::tr("Oui — dernier jour du mois")) }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer la condition")) } }
            }

            h2 {(crate::langue::tr("Modes de paiement")) }
            table {
                thead { tr { th {(crate::langue::tr("Id")) } th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Libellé")) } } }
                tbody {
                    @for m in &modes { tr { td .num { (m.id) } td .ref { (m.code) } td { (m.libelle) } } }
                }
            }

            h2 {(crate::langue::tr("Unités de mesure")) }
            table {
                thead { tr { th {(crate::langue::tr("Code")) } th {(crate::langue::tr("Libellé")) } } }
                tbody {
                    @for u in &unites { tr { td .ref { (u.code) } td { (u.libelle) } } }
                }
            }

            h2 {(crate::langue::tr("Toutes les tables de dictionnaire")) }
            p .sous-titre { (recensement.len()) " tables llx_c_*" }
            table {
                thead { tr { th {(crate::langue::tr("Table")) } th .num {(crate::langue::tr("Lignes")) } } }
                tbody {
                    @for (t, n) in &recensement {
                        tr { td .ref { (t) } td .num { (n) } }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn parametres_base(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let i = db::infos_base(p)?;
    let modules = db::modules_actifs(p)?;
    let s = db::company::get(p, entite::courante())?;
    let manquants = legal::champs_manquants(
        &app::societe_legale(&s),
        &legal::Client {
            nom: "client".into(),
            ..Default::default()
        },
    );

    Ok(ui::page(
        "parametres",
        "base",
        "Paramètres — base de données",
        html! {
            h1 {(crate::langue::tr("Base de données")) }
            p .sous-titre {(crate::langue::tr("Connexion et état du schéma Dolibarr")) }

            table {
                tbody {
                    tr { th {(crate::langue::tr("Serveur")) } td { (i.version_serveur) } }
                    tr { th {(crate::langue::tr("Hôte")) } td { (i.hote) ":" (i.port) } }
                    tr { th {(crate::langue::tr("Base")) } td { (i.base) } }
                    tr { th {(crate::langue::tr("Tables")) } td .num { (i.tables) } }
                    tr { th {(crate::langue::tr("Colonnes")) } td .num { (i.colonnes) } }
                    tr { th {(crate::langue::tr("Schéma de référence")) } td {(crate::langue::tr("Dolibarr 24.0.1")) } }
                    tr {
                        th {(crate::langue::tr("Conformité de la société")) }
                        td {
                            @if manquants.is_empty() {
                                span .pastille.p-payee {(crate::langue::tr("complète")) }
                            } @else {
                                span .pastille.p-retard { (manquants.join(", ")) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Modules Dolibarr activés")) }
            @if modules.is_empty() {
                (ui::vide("Aucun module activé dans llx_const.", None))
            } @else {
                p { (modules.join(" · ")) }
            }
        },
    ))
}

pub(crate) async fn parametres_societe(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let mut s = db::company::get(&app.pool, entite::courante())?;
    let lire = |cle: &str| f.get(cle).map(|v| v.trim().to_string());
    s.nom = lire("nom").unwrap_or(s.nom);
    s.forme_juridique = lire("forme");
    s.capital = lire("capital");
    s.rcs = lire("rcs");
    s.adresse = lire("adresse");
    s.code_postal = lire("cp");
    s.ville = lire("ville");
    s.siret = lire("siret");
    s.tva_intra = lire("tva_intra");
    s.email = lire("email");
    // Champs qui etaient affiches mais jamais enregistres : IBAN, BIC,
    // assurance et CGV. Un formulaire qui ne sauvegarde pas est un piege.
    s.iban = lire("iban");
    s.bic = lire("bic");
    s.assurance_assureur = lire("assurance_assureur");
    s.assurance_police = lire("assurance_police");
    s.assurance_couverture = lire("assurance_couverture");
    s.cgv = lire("cgv");
    s.tva_sur_encaissements = f
        .get("tva_encaissements")
        .map(|v| v == "1")
        .unwrap_or(false);
    s.tva_taux_reduit = f.get("tva_taux_reduit").map(|v| v == "1").unwrap_or(false);
    s.franchise_en_base = f.get("franchise").map(|v| v == "1").unwrap_or(false);
    s.delai_paiement_jours = f
        .get("delai")
        .and_then(|v| v.parse().ok())
        .unwrap_or(s.delai_paiement_jours);
    db::company::save(&app.pool, &s)?;
    Ok(Redirect::to("/parametres?ok=societe"))
}

pub(crate) async fn condition_reglement_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let code = champ("code").ok_or_else(|| Erreur("code de la condition obligatoire".into()))?;
    let libelle =
        champ("libelle").ok_or_else(|| Erreur("libellé de la condition obligatoire".into()))?;
    let nbjour: i64 = champ("nbjour").and_then(|v| v.parse().ok()).unwrap_or(0);
    let fin_de_mois = f.get("fin_de_mois").map(|v| v == "1").unwrap_or(false);
    db::reglement::creer_condition(
        &app.pool,
        entite::courante(),
        &code,
        &libelle,
        nbjour,
        fin_de_mois,
    )?;
    Ok(Redirect::to("/parametres/dictionnaires?ok=condition"))
}

pub(crate) async fn condition_reglement_desactiver(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let id = f
        .get("id")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("condition non précisée".into()))?;
    db::reglement::desactiver_condition(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/parametres/dictionnaires?ok=condition"))
}
