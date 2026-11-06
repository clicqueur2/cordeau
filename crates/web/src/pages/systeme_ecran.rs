//! Ecrans systeme : journal, navigateur de tables, greffons, installation.
//!
//! Ce sont les ecrans qu'on ouvre quand quelque chose ne va pas : voir ce que
//! la base contient vraiment, ce que les modules ont enregistre, et refaire
//! l'installation quand il n'y a plus personne a connecter.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use maud::{html, Markup};

use crate::{Erreur, Res};

/// Le journal des evenements : qui s'est connecte, qui a change quoi.
///
/// La table `llx_events` existe dans Dolibarr mais son cceur ne la remplit
/// presque pas : sans cet ecran, un incident de securite ne se raconte pas.
pub async fn evenements_page(State(app): State<crate::App>) -> Res<Markup> {
    let entity = crate::entite::courante();
    let evenements = db::events::recents(&app.pool, entity, 200)?;
    let types = db::events::par_type(&app.pool, entity)?;
    let total = db::events::total(&app.pool, entity)?;

    Ok(crate::ui::page(
        "parametres",
        "evenements",
        "Journal des événements",
        html! {
            h1 {(crate::langue::tr("Journal des événements")) }
            p .sous-titre {
                (total.to_string()) " événement(s) sur cette entité. "
                "Le c\u{153}ur de Dolibarr ne remplit presque pas cette table ; "
                "ce noyau y consigne les connexions, les échecs et les changements de module."
            }

            @if types.is_empty() {
                (crate::ui::vide("Aucun événement consigné sur cette entité.", None))
            } @else {
                h2 {(crate::langue::tr("Par type")) }
                table {
                    thead { tr { th {(crate::langue::tr("Type")) } th .num {(crate::langue::tr("Nombre")) } } }
                    tbody {
                        @for (type_, nombre) in &types {
                            tr {
                                td .ref { (type_) }
                                td .num { (nombre.to_string()) }
                            }
                        }
                    }
                }

                h2 {(crate::langue::tr("Les 200 derniers")) }
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Quand")) }
                            th {(crate::langue::tr("Type")) }
                            th {(crate::langue::tr("Compte")) }
                            th {(crate::langue::tr("Détail")) }
                            th {(crate::langue::tr("Adresse")) }
                        }
                    }
                    tbody {
                        @for e in &evenements {
                            tr {
                                td .ref { (e.date.clone()) }
                                td .ref { (e.type_.clone()) }
                                td { (e.login.clone().unwrap_or_else(|| "—".into())) }
                                td { (e.description.clone()) }
                                td .ref { (e.ip.clone()) }
                            }
                        }
                    }
                }
                p style="color:#78736c;font-size:13.5px;margin-top:14px" {
                    "L'adresse est lue dans " code {(crate::langue::tr("X-Forwarded-For")) } " quand un proxy la pose, "
                    "sinon la connexion locale. Derrière un proxy mal configuré, elle est "
                    "indicative et non une preuve."
                }
            }
        },
    ))
}

/// Toutes les tables de la base, avec leur nombre de lignes.
///
/// C'est le filet de securite du portage : aucune donnee de Dolibarr n'est
/// invisible, meme quand son ecran metier n'est pas encore ecrit.
pub async fn tables_page(State(app): State<crate::App>) -> Res<Markup> {
    let tables = db::tables::tables(&app.pool)?;
    let total: u64 = tables.iter().map(|t| t.lignes).sum();

    Ok(crate::ui::page(
        "parametres",
        "tables",
        "Tables de la base",
        html! {
            h1 {(crate::langue::tr("Tables de la base")) }
            p .sous-titre {
                (tables.len().to_string()) " tables, " (total.to_string())
                " lignes (comptage approximatif de MariaDB). Lecture seule : "
                "les ecritures passent par les ecrans metier."
            }
            table {
                thead {
                    tr { th {(crate::langue::tr("Table")) } th .num {(crate::langue::tr("Lignes")) } th {(crate::langue::tr("Commentaire")) } }
                }
                tbody {
                    @for t in &tables {
                        tr {
                            td { a href={ "/parametres/tables/" (t.nom) } { (t.nom) } }
                            td .num { (t.lignes.to_string()) }
                            td class="muet" { (t.commentaire.clone()) }
                        }
                    }
                }
            }
        },
    ))
}

/// Le contenu d'une table, colonne par colonne.
pub async fn table_page(
    State(app): State<crate::App>,
    Path(nom): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let page: u64 = q
        .get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(1)
        .max(1);
    let limite = 100u64;
    let (colonnes, lignes) = db::tables::lignes(
        &app.pool,
        &nom,
        crate::entite::courante(),
        limite,
        (page - 1) * limite,
    )?;

    Ok(crate::ui::page(
        "parametres",
        "tables",
        &nom,
        html! {
            h1 { (nom.clone()) }
            p .sous-titre {
                (colonnes.len().to_string()) " colonne(s), " (lignes.len().to_string())
                " ligne(s) — page " (page.to_string())
                " · lecture seule"
            }
            @if lignes.is_empty() {
                (crate::ui::vide("Table vide.", Some(("/parametres/tables", "Retour aux tables"))))
            } @else {
                div style="overflow-x:auto" {
                    table {
                        thead {
                            tr { @for c in &colonnes { th { (c.clone()) } } }
                        }
                        tbody {
                            @for l in &lignes {
                                tr {
                                    @for v in l {
                                        td {
                                            (v.clone().unwrap_or_default())
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div .actions {
                a .bouton.secondaire href={ "/parametres/tables/" (nom) "?page=" ((page + 1).to_string()) } {(crate::langue::tr("Page suivante"))
                }
                a .bouton.secondaire href="/parametres/tables" style="margin-left:8px" {(crate::langue::tr("Toutes les tables"))
                }
            }
        },
    ))
}

/// Greffons charges depuis le dossier : ce que le noyau accepte d'executer.
pub async fn greffons_page() -> Res<Markup> {
    let dossier = std::env::var("DOLIBARR_GREFFONS").unwrap_or_else(|_| "greffons".to_string());
    let liste = greffons::charger_dossier(std::path::Path::new(&dossier))
        .map_err(|e| Erreur(e.to_string()))?;

    Ok(crate::ui::page(
        "parametres",
        "greffons",
        "Greffons",
        html! {
            h1 {(crate::langue::tr("Greffons")) }
            p .sous-titre {
                "Bibliothèques chargées à chaud, contrat ABI C version "
                (greffons::API.to_string())
                ". Dossier : " code { (dossier) }
            }
            @if liste.is_empty() {
                (crate::ui::vide(
                    "Aucun greffon chargé. Déposez une bibliothèque (.dll, .so) dans le dossier.",
                    None,
                ))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Greffon")) } th {(crate::langue::tr("Fichier")) } } }
                    tbody {
                        @for g in &liste {
                            tr {
                                td { (g.nom.clone()) }
                                td class="muet" { (g.chemin.display().to_string()) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Etat de l'installation : ce qui est pret, ce qui manque, et quoi faire.
///
/// La page ne montre que des compteurs — aucun nom, aucun identifiant — et
/// n'ecrit rien : elle peut donc etre consultee sans etre connecte, ce qui est
/// exactement le cas au premier demarrage.
pub async fn installation_page(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let attendues = db::installation::TABLES_ATTENDUES;
    let etat = db::installation::etat(&app.pool, crate::entite::courante(), attendues)?;

    // Ce que la base porte deja : sert a proposer un socle sur une base nue.
    let actifs = db::module::actifs(&app.pool, crate::entite::courante()).unwrap_or_default();

    Ok(crate::ui::page(
        "accueil",
        "installation",
        "Installation",
        html! {
            h1 {(crate::langue::tr("Installation")) }
            p .sous-titre {
                "État du noyau sur cette base. Rien n'est modifié par cette page : "
                "elle constate, elle ne répare pas."
            }
            @if etat.pret() {
                (crate::ui::flash("Installation complète : le noyau peut servir."))
            } @else {
                p .erreur {(crate::langue::tr("Installation incomplète : voir les points ci-dessous.")) }
            }

            // Une base neuve n'a presque aucun module allume : c'est ici qu'on
            // choisit un socle, pas apres coup dans un menu de reglages qu'on ne
            // trouve pas. La condition est le nombre de modules actifs, pas
            // l'anciennete de la base : une installation ancienne mais nue doit
            // voir la proposition elle aussi.
            @if actifs.len() <= 3 {
                h2 {(crate::langue::tr("Choisir un socle")) }
                p class="aide" {
                    "Cette base n'a que " (actifs.len()) " module(s) actif(s). "
                    "Un socle en allume d'un coup ceux dont un metier a besoin. "
                    "Il n'en eteint aucun et n'ecrit aucune donnee."
                }
                @for p in db::preset::PRESETS {
                    form method="post" action="/presets" style="display:inline-block;margin:0 8px 8px 0" {
                        input type="hidden" name="code" value=(p.code);
                        button type="submit" class="petit" { (p.libelle) }
                    }
                }
                p class="aide petite" {
                    a href="/presets" { (crate::langue::tr("Voir le detail des socles")) }
                }
            }
            table {
                thead { tr { th {(crate::langue::tr("Point")) } th { "État" } th .num {(crate::langue::tr("Valeur")) } } }
                tbody {
                    tr {
                        td {(crate::langue::tr("Schéma Dolibarr")) }
                        td {
                            @if etat.tables >= etat.tables_attendues { "complet" } @else { "incomplet" }
                        }
                        td .num {
                            (etat.tables.to_string()) " / " (etat.tables_attendues.to_string())
                        }
                    }
                    tr {
                        td {(crate::langue::tr("Administrateurs")) }
                        td { @if etat.admins > 0 {(crate::langue::tr("présent")) } @else { "aucun" } }
                        td .num { (etat.admins.to_string()) }
                    }
                    tr {
                        td {(crate::langue::tr("Utilisateurs")) }
                        td { "" }
                        td .num { (etat.utilisateurs.to_string()) }
                    }
                    tr {
                        td {(crate::langue::tr("Modules semés")) }
                        td { @if etat.modules_semes > 0 {(crate::langue::tr("présents")) } @else { "aucun" } }
                        td .num { (etat.modules_semes.to_string()) }
                    }
                    tr {
                        td {(crate::langue::tr("Société")) }
                        td {
                            @match &etat.societe {
                                Some(n) => (n.clone()),
                                None => span class="rouge" {(crate::langue::tr("sans nom")) },
                            }
                        }
                        td { "" }
                    }
                    tr {
                        td {(crate::langue::tr("Barre de menus")) }
                        td {
                            @if crate::menu::depuis_base() {(crate::langue::tr("lue dans llx_menu")) } @else {(crate::langue::tr("celle du noyau")) }
                        }
                        td { "" }
                    }
                    tr {
                        td {(crate::langue::tr("SMTP")) }
                        td {
                            @match &etat.smtp {
                                Some(h) => (h.clone()),
                                None => span class="rouge" {(crate::langue::tr("non configuré")) },
                            }
                        }
                        td { "" }
                    }
                }
            }
            @if !etat.manques().is_empty() {
                h2 {(crate::langue::tr("Ce qui manque")) }
                ul {
                    @for m in etat.manques() { li { (m) } }
                }
            }
            @if etat.admins == 0 {
                h2 {(crate::langue::tr("Créer l'administrateur")) }
                p .sous-titre {
                    "Cette page n'accepte la création que tant qu'aucun administrateur "
                    "n'existe : ensuite elle refuse, pour ne pas devenir une porte dérobée."
                }
                @if let Some(e) = q.get("erreur") {
                    p .erreur { (e) }
                }
                form method="post" action="/installation" {
                    table {
                        tbody {
                            tr {
                                th {(crate::langue::tr("Identifiant")) }
                                td { input type="text" name="login" required value="admin" class="pleine"; }
                            }
                            tr {
                                th {(crate::langue::tr("Mot de passe")) }
                                td { input type="password" name="mot_de_passe" required minlength="8" class="pleine"; }
                            }
                            tr {
                                th {(crate::langue::tr("E-mail")) }
                                td { input type="email" name="email" class="pleine"; }
                            }
                            tr {
                                th {(crate::langue::tr("Nom de la société")) }
                                td { input type="text" name="societe" class="pleine"; }
                            }
                        }
                    }
                    div .actions { button type="submit" {(crate::langue::tr("Installer")) } }
                }
            }
        },
    ))
}

/// Cree le premier administrateur et nomme la societe.
///
/// Garde absolue : n'ecrit que si **aucun** administrateur n'existe. Sans cela,
/// `/installation` resterait ouverte et permettrait de recreer un compte admin
/// sur une instance en service. Le compte est cree avec le hachage bcrypt de
/// Dolibarr (`$2y$`), donc il fonctionne aussi bien ici que dans Dolibarr PHP.
pub async fn installation_creer(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let entity = crate::entite::courante();

    // La garde, avant tout le reste.
    let etat = db::installation::etat(&app.pool, entity, db::installation::TABLES_ATTENDUES)?;
    if etat.admins > 0 {
        return Ok(Redirect::to(
            "/installation?erreur=Installation%20deja%20faite",
        ));
    }

    let champ = |cle: &str| {
        f.get(cle)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let login = champ("login").ok_or_else(|| Erreur("identifiant obligatoire".into()))?;
    let mot_de_passe =
        champ("mot_de_passe").ok_or_else(|| Erreur("mot de passe obligatoire".into()))?;
    if mot_de_passe.chars().count() < 8 {
        return Ok(Redirect::to(
            "/installation?erreur=Mot%20de%20passe%20trop%20court%20(8%20caracteres%20minimum)",
        ));
    }

    db::user::creer(
        &app.pool,
        entity,
        &db::user::NouvelUtilisateur {
            login: login.clone(),
            mot_de_passe,
            nom: "Administrateur".to_string(),
            prenom: String::new(),
            email: champ("email"),
            admin: true,
        },
    )?;

    if let Some(societe) = champ("societe") {
        db::company::set_const(&app.pool, "MAIN_INFO_SOCIETE_NOM", &societe)?;
    }

    // Les modules : sans eux, la barre de menus serait vide au premier ecran.
    db::module::semer_modules_portes(&app.pool, entity)?;

    Ok(Redirect::to("/connexion?ok=installe"))
}
