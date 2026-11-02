//! Ecrans de liste et de reglage qui remplacent les entrees vides du menu.
//!
//! Regle : un ecran qui n'affiche rien n'existe pas. Chaque page ci-dessous lit
//! de vraies donnees ou ecrit de vraies constantes Dolibarr.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use maud::{html, Markup};

use crate::{message, Erreur, Res, ENTITY};

/// Un champ de reglage : constante Dolibarr, libelle, type, valeur par defaut.
pub struct Champ {
    pub cle: &'static str,
    pub libelle: &'static str,
    /// `text`, `nombre`, `oui_non` ou `textarea`.
    pub genre: &'static str,
    pub defaut: &'static str,
}

pub struct Section {
    pub cle: &'static str,
    pub titre: &'static str,
    pub intro: &'static str,
    pub champs: &'static [Champ],
}

pub const SECTIONS: &[Section] = &[
    Section {
        cle: "affichage",
        titre: "Affichage",
        intro: "Langue, devise et format de date par defaut. Ces constantes sont celles que lit Dolibarr.",
        champs: &[
            Champ { cle: "MAIN_LANG_DEFAULT", libelle: "Langue par defaut", genre: "text", defaut: "fr_FR" },
            Champ { cle: "MAIN_MONNAIE", libelle: "Devise", genre: "text", defaut: "EUR" },
            Champ { cle: "DOLIBARR_RS_FORMAT_DATE", libelle: "Format de date", genre: "text", defaut: "d/m/Y" },
        ],
    },
    Section {
        cle: "menus",
        titre: "Menus",
        intro: "Reglage de la barre de navigation.",
        champs: &[Champ {
            cle: "DOLIBARR_RS_MASQUER_MENU_HAUT",
            libelle: "Masquer la barre de modules",
            genre: "oui_non",
            defaut: "0",
        }],
    },
    Section {
        cle: "traduction",
        titre: "Traduction",
        intro: "Langue de l'interface. Les fichiers de traduction Dolibarr restent la reference.",
        champs: &[
            Champ { cle: "MAIN_LANG_DEFAULT", libelle: "Langue de l'interface", genre: "text", defaut: "fr_FR" },
            Champ { cle: "DOLIBARR_RS_LANGUE_SECONDAIRE", libelle: "Langue secondaire", genre: "text", defaut: "" },
        ],
    },
    Section {
        cle: "valeurs",
        titre: "Valeurs, filtres et tris par defaut",
        intro: "Taille des listes et ordre de tri.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_LIGNES_PAR_PAGE", libelle: "Lignes par page", genre: "nombre", defaut: "50" },
            Champ { cle: "DOLIBARR_RS_TRI_DEFAUT", libelle: "Tri par defaut", genre: "text", defaut: "date_desc" },
        ],
    },
    Section {
        cle: "widgets",
        titre: "Widgets",
        intro: "Cartes affichees sur le tableau de bord, separees par des virgules.",
        champs: &[Champ {
            cle: "DOLIBARR_RS_WIDGETS",
            libelle: "Cartes du tableau de bord",
            genre: "text",
            defaut: "facture, encaisse, reste, retard",
        }],
    },
    Section {
        cle: "alertes",
        titre: "Alertes",
        intro: "Seuils qui declenchent une alerte sur les impayes.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_ALERTE_RETARD_JOURS", libelle: "Alerte apres (jours de retard)", genre: "nombre", defaut: "5" },
            Champ { cle: "DOLIBARR_RS_ALERTE_EMAIL", libelle: "Destinataire des alertes", genre: "text", defaut: "" },
        ],
    },
    Section {
        cle: "securite",
        titre: "Securite",
        intro: "Cle d'API et longueur minimale des mots de passe.",
        champs: &[
            Champ { cle: "DOLIBARR_API_KEY", libelle: "Cle d'API (en-tete DOLAPIKEY)", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_PASSWORD_MIN", libelle: "Longueur minimale du mot de passe", genre: "nombre", defaut: "10" },
        ],
    },
    Section {
        cle: "limites",
        titre: "Limites et precision",
        intro: "Arrondis et decalage des listes.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_DECIMALES", libelle: "Decimales affichees", genre: "nombre", defaut: "2" },
            Champ { cle: "DOLIBARR_RS_LIMITE_EXPORT", libelle: "Lignes maximum a l'export", genre: "nombre", defaut: "5000" },
        ],
    },
    Section {
        cle: "pdf",
        titre: "Modeles de PDF",
        intro: "Marges du gabarit, en millimetres. Le gabarit les lit a chaque rendu.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_PDF_MARGE_HAUT", libelle: "Marge haute", genre: "nombre", defaut: "14" },
            Champ { cle: "DOLIBARR_RS_PDF_MARGE_BAS", libelle: "Marge basse", genre: "nombre", defaut: "34" },
            Champ { cle: "DOLIBARR_RS_PDF_MARGE_COTE", libelle: "Marge laterale", genre: "nombre", defaut: "10" },
        ],
    },
    Section {
        cle: "emails",
        titre: "Emails et notifications",
        intro: "Parametres SMTP. L'envoi n'est pas encore implemente : les valeurs sont enregistrees pour la suite.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_SMTP_HOST", libelle: "Serveur SMTP", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_SMTP_PORT", libelle: "Port", genre: "nombre", defaut: "587" },
            Champ { cle: "DOLIBARR_RS_SMTP_UTILISATEUR", libelle: "Utilisateur", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_EMAIL_EXPEDITEUR", libelle: "Adresse d'expedition", genre: "text", defaut: "" },
        ],
    },
    Section {
        cle: "sms",
        titre: "SMS",
        intro: "Fournisseur d'envoi de SMS. Aucun envoi n'est effectue pour l'instant.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_SMS_FOURNISSEUR", libelle: "Fournisseur", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_SMS_EXPEDITEUR", libelle: "Expediteur", genre: "text", defaut: "" },
        ],
    },
    Section {
        cle: "divers",
        titre: "Divers",
        intro: "Pied de page imprime sur les documents.",
        champs: &[Champ {
            cle: "DOLIBARR_RS_PIED_PAGE",
            libelle: "Pied de page",
            genre: "textarea",
            defaut: "",
        }],
    },
];

fn section(cle: &str) -> Option<&'static Section> {
    SECTIONS.iter().find(|s| s.cle == cle)
}

/// Ecran de reglage generique : une section = une entree du menu.
pub async fn reglages(
    State(app): State<crate::App>,
    Path(cle): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let s = section(&cle).ok_or_else(|| Erreur("reglage inconnu".into()))?;
    let mut valeurs = Vec::new();
    for c in s.champs {
        let v = db::company::get_const(&app.pool, c.cle)?
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| c.defaut.to_string());
        valeurs.push(v);
    }

    Ok(crate::ui::page(
        "parametres",
        s.cle,
        s.titre,
        html! {
            (message(q.get("ok")))
            h1 { (s.titre) }
            p .sous-titre { (s.intro) }
            form method="post" action={ "/parametres/reglage/" (s.cle) } {
                table {
                    tbody {
                        @for (i, c) in s.champs.iter().enumerate() {
                            tr {
                                th { (c.libelle) }
                                td {
                                    @if c.genre == "oui_non" {
                                        select name=(c.cle) {
                                            option value="0" selected[valeurs[i] != "1"] { "Non" }
                                            option value="1" selected[valeurs[i] == "1"] { "Oui" }
                                        }
                                    } @else if c.genre == "nombre" {
                                        input type="number" name=(c.cle) value=(valeurs[i]) style="width:130px";
                                    } @else if c.genre == "textarea" {
                                        textarea name=(c.cle) rows="4" style="width:100%;font:inherit;padding:8px 10px;border:1px solid #e6e2dc" {
                                            (valeurs[i])
                                        }
                                    } @else {
                                        input type="text" name=(c.cle) value=(valeurs[i]) style="width:100%";
                                    }
                                    p style="color:#78736c;font-size:12px;margin:6px 0 0" { "constante " code { (c.cle) } }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" { "Enregistrer" } }
            }
        },
    ))
}

pub async fn reglages_enregistrer(
    State(app): State<crate::App>,
    Path(cle): Path<String>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let s = section(&cle).ok_or_else(|| Erreur("reglage inconnu".into()))?;
    for c in s.champs {
        let valeur = f.get(c.cle).cloned().unwrap_or_default();
        db::company::set_const(&app.pool, c.cle, valeur.trim())?;
    }
    Ok(Redirect::to(&format!(
        "/parametres/reglage/{cle}?ok=reglage"
    )))
}

/// Reglements : liste des encaissements, toutes factures confondues.
pub async fn reglements(State(app): State<crate::App>) -> Res<Markup> {
    let liste = db::divers::reglements(&app.pool, ENTITY, 200)?;
    Ok(crate::ui::page(
        "factures",
        "paiements",
        "Reglements",
        html! {
            h1 { "Saisie des reglements" }
            p .sous-titre {
                "Tous les encaissements enregistres. Un reglement se saisit depuis la fiche de la facture concerne."
            }
            @if liste.is_empty() {
                (crate::ui::vide("Aucun reglement.", Some(("/factures", "Voir les factures"))))
            } @else {
                table {
                    thead { tr { th { "Ref." } th { "Date" } th { "Tiers" } th { "Factures" } th { "Mode" } th .num { "Montant" } } }
                    tbody {
                        @for r in &liste {
                            tr {
                                td .ref { (r.ref_) }
                                td .ref { (r.date.clone().unwrap_or_default()) }
                                td { (r.tiers.clone().unwrap_or_default()) }
                                td .ref { (r.factures.clone().unwrap_or_default()) }
                                td { (r.mode.clone().unwrap_or_default()) }
                                td .num { (crate::ui::euros(r.montant)) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Avoirs : factures de type 2.
pub async fn avoirs(State(app): State<crate::App>) -> Res<Markup> {
    let liste: Vec<_> = db::invoice::list(&app.pool, ENTITY, 500, 0)?
        .into_iter()
        .filter(|f| f.type_ == db::invoice::TYPE_AVOIR)
        .collect();
    Ok(crate::ui::page(
        "factures",
        "avoirs",
        "Avoirs",
        html! {
            h1 { "Avoirs" }
            p .sous-titre { "Les avoirs annulent une facture : on ne supprime jamais une piece comptable." }
            @if liste.is_empty() {
                (crate::ui::vide("Aucun avoir.", Some(("/factures", "Voir les factures"))))
            } @else {
                table {
                    thead { tr { th { "Ref." } th { "Tiers" } th { "Date" } th .num { "TTC" } th { "" } } }
                    tbody {
                        @for f in &liste {
                            tr {
                                td .ref { a href={ "/factures/" (f.rowid) } { (f.ref_) } }
                                td { (crate::nom_tiers(&app.pool, f.fk_soc)?) }
                                td .ref { (f.datef.clone().unwrap_or_default()) }
                                td .num { (crate::ui::euros(f.total_ttc.unwrap_or_default())) }
                                td { a .bouton.petit.secondaire href={ "/factures/" (f.rowid) } { "Ouvrir" } }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Contrats clients.
pub async fn contrats(State(app): State<crate::App>) -> Res<Markup> {
    let liste = db::divers::contrats(&app.pool, ENTITY, 200)?;
    Ok(crate::ui::page(
        "devis",
        "contrats",
        "Contrats",
        html! {
            h1 { "Contrats" }
            p .sous-titre { "Contrats clients enregistres dans Dolibarr (llx_contrat)." }
            @if liste.is_empty() {
                (crate::ui::vide("Aucun contrat.", None))
            } @else {
                table {
                    thead { tr { th { "Ref." } th { "Tiers" } th { "Date" } th { "Statut" } th .num { "TTC" } } }
                    tbody {
                        @for c in &liste {
                            tr {
                                td .ref { (c.ref_) }
                                td { (c.tiers.clone().unwrap_or_default()) }
                                td .ref { (c.date.clone().unwrap_or_default()) }
                                td { (match c.statut { 1 => "actif", 2 => "clos", _ => "brouillon" }) }
                                td .num { (crate::ui::euros(c.total_ttc.unwrap_or_default())) }
                            }
                        }
                    }
                }
            }
        },
    ))
}
/// Journal des erreurs : les dernieres lignes ecrites par le serveur.
pub async fn journal(State(_app): State<crate::App>) -> Res<Markup> {
    let lignes = crate::journal_erreurs::dernieres(200);
    let total = crate::journal_erreurs::compter();
    let chemin = crate::journal_erreurs::fichier();
    Ok(crate::ui::page(
        "parametres",
        "journal",
        "Journal des erreurs",
        html! {
            h1 { "Journal des erreurs" }
            p .sous-titre {
                (total) " ligne(s) - fichier " code { (chemin.display().to_string()) }
            }
            @if lignes.is_empty() {
                (crate::ui::vide("Aucune erreur enregistree.", None))
            } @else {
                table {
                    thead { tr { th { "Date" } th { "Chemin" } th { "Message" } } }
                    tbody {
                        @for l in lignes.iter().rev() {
                            @let morceaux: Vec<&str> = l.splitn(3, " | ").collect();
                            tr {
                                td .ref { (morceaux.first().copied().unwrap_or("")) }
                                td .ref { (morceaux.get(1).copied().unwrap_or("")) }
                                td { (morceaux.get(2).copied().unwrap_or(l.as_str())) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Droits d'acces d'un utilisateur, un par ecran.
pub async fn utilisateur_droits(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let u =
        db::user::get(&app.pool, id)?.ok_or_else(|| Erreur("utilisateur introuvable".into()))?;
    let attribues = db::droit::droits(&app.pool, id)?;

    Ok(crate::ui::page(
        "parametres",
        "utilisateurs",
        &format!("Droits de {}", u.login),
        html! {
            (message(q.get("ok")))
            h1 { "Droits de " (u.login) }
            p .sous-titre {
                "Un administrateur a tous les droits. Pour les autres comptes, chaque case ouvre un ecran."
            }
            form method="post" action={ "/parametres/utilisateurs/" (id) "/droits" } {
                table {
                    thead { tr { th { "Ecran" } th { "Autorise" } } }
                    tbody {
                        @for d in db::droit::DEFINITIONS {
                            tr {
                                td { (d.libelle) }
                                td {
                                    input type="checkbox" name=(format!("d{}", d.id)) value="1"
                                          checked[attribues.contains(&d.id)];
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" { "Enregistrer les droits" }
                    a .bouton.secondaire href="/parametres/utilisateurs" { "Retour" }
                }
            }

            h2 { "Droits hérités des groupes" }
            @let mes_groupes = db::groupe::groupes_de(&app.pool, ENTITY, id)?;
            @if mes_groupes.is_empty() {
                p { "Ce compte n'appartient à aucun groupe." }
            } @else {
                table {
                    thead { tr { th { "Groupe" } th { "Droits portés" } } }
                    tbody {
                        @for g in &mes_groupes {
                            @let droits_groupe = db::groupe::droits(&app.pool, g.rowid)?;
                            tr {
                                td { a href={ "/parametres/groupes/" (g.rowid) } { (g.nom) } }
                                td {
                                    @for d in db::droit::DEFINITIONS.iter().filter(|d| droits_groupe.contains(&d.id)) {
                                        (d.libelle) " · "
                                    }
                                }
                            }
                        }
                    }
                }
                p style="color:#78736c;font-size:13px" {
                    "Les droits ci-dessus sont accordés par le groupe : pour les retirer, il faut modifier le groupe."
                }
            }
        },
    ))
}

pub async fn utilisateur_droits_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    for d in db::droit::DEFINITIONS {
        let actif = f.contains_key(&format!("d{}", d.id));
        db::droit::definir(&app.pool, ENTITY, id, d.id, actif)?;
    }
    Ok(Redirect::to(&format!(
        "/parametres/utilisateurs/{id}/droits?ok=droits"
    )))
}

/// Liste des groupes et creation.
pub async fn groupes(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let liste = db::groupe::list(&app.pool, ENTITY)?;
    Ok(crate::ui::page(
        "parametres",
        "groupes",
        "Groupes",
        html! {
            (message(q.get("ok")))
            h1 { "Groupes" }
            p .sous-titre {
                "Un groupe porte des droits ; ses membres en heritent. Un utilisateur peut cumuler plusieurs groupes."
            }
            @if liste.is_empty() {
                (crate::ui::vide("Aucun groupe.", None))
            } @else {
                table {
                    thead { tr { th { "Groupe" } th { "Note" } th { "" } } }
                    tbody {
                        @for g in &liste {
                            tr {
                                td { a href={ "/parametres/groupes/" (g.rowid) } { (g.nom) } }
                                td { (g.note.clone().unwrap_or_default()) }
                                td { a .bouton.petit.secondaire href={ "/parametres/groupes/" (g.rowid) } { "Ouvrir" } }
                            }
                        }
                    }
                }
            }
            h2 { "Nouveau groupe" }
            form method="post" action="/parametres/groupes" {
                table {
                    tbody {
                        tr { th { "Nom" } td { input type="text" name="nom" required style="width:100%"; } }
                        tr { th { "Note" } td { input type="text" name="note" style="width:100%"; } }
                    }
                }
                div .actions { button type="submit" { "Creer le groupe" } }
            }
        },
    ))
}

pub async fn groupe_creer(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let nom = f
        .get("nom")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("nom du groupe obligatoire".into()))?;
    let note = f
        .get("note")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    db::groupe::creer(&app.pool, ENTITY, &nom, note.as_deref())?;
    Ok(Redirect::to("/parametres/groupes?ok=cree"))
}

/// Fiche d'un groupe : membres et droits.
pub async fn groupe_fiche(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let g = db::groupe::get(&app.pool, ENTITY, id)?
        .ok_or_else(|| Erreur("groupe introuvable".into()))?;
    let membres = db::groupe::membres(&app.pool, ENTITY, id)?;
    let droits = db::groupe::droits(&app.pool, id)?;
    let utilisateurs = db::user::list(&app.pool, ENTITY)?;

    Ok(crate::ui::page(
        "parametres",
        "groupes",
        &g.nom,
        html! {
            (message(q.get("ok")))
            h1 { "Groupe " (g.nom) }
            p .sous-titre { "Les membres heritent des droits coches ci-dessous." }

            h2 { "Membres" }
            form method="post" action={ "/parametres/groupes/" (id) "/membres" } {
                table {
                    thead { tr { th { "Utilisateur" } th { "Membre" } } }
                    tbody {
                        @for u in &utilisateurs {
                            tr {
                                td { (u.login) }
                                td {
                                    input type="checkbox" name=(format!("u{}", u.rowid)) value="1"
                                          checked[membres.iter().any(|(mid, _)| *mid == u.rowid)];
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" { "Enregistrer les membres" } }
            }

            h2 { "Droits du groupe" }
            form method="post" action={ "/parametres/groupes/" (id) "/droits" } {
                table {
                    thead { tr { th { "Ecran" } th { "Autorise" } } }
                    tbody {
                        @for d in db::droit::DEFINITIONS {
                            tr {
                                td { (d.libelle) }
                                td {
                                    input type="checkbox" name=(format!("d{}", d.id)) value="1"
                                          checked[droits.contains(&d.id)];
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" { "Enregistrer les droits du groupe" }
                    a .bouton.secondaire href="/parametres/groupes" { "Retour" }
                }
            }
        },
    ))
}

pub async fn groupe_membres_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    for u in db::user::list(&app.pool, ENTITY)? {
        let membre = f.contains_key(&format!("u{}", u.rowid));
        db::groupe::definir_membre(&app.pool, ENTITY, id, u.rowid, membre)?;
    }
    Ok(Redirect::to(&format!(
        "/parametres/groupes/{id}?ok=membres"
    )))
}

pub async fn groupe_droits_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    for d in db::droit::DEFINITIONS {
        let actif = f.contains_key(&format!("d{}", d.id));
        db::groupe::definir_droit(&app.pool, ENTITY, id, d.id, actif)?;
    }
    Ok(Redirect::to(&format!("/parametres/groupes/{id}?ok=droits")))
}

/// Stock : niveaux par produit et entrepot, derniers mouvements.
pub async fn stock(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let niveaux = db::stock::niveaux(&app.pool, ENTITY)?;
    let mouvements = db::stock::mouvements(&app.pool, ENTITY, 50)?;
    let entrepots = db::stock::entrepots(&app.pool, ENTITY)?;
    let produits = db::product::list(&app.pool, ENTITY, 200, 0)?;

    Ok(crate::ui::page(
        "produits",
        "stock",
        "Stock",
        html! {
            (message(q.get("ok")))
            h1 { "Stock" }
            p .sous-titre {
                "Le niveau affiche est la somme des mouvements, jamais une valeur saisie a part."
            }

            h2 { "Niveaux" }
            @if niveaux.is_empty() {
                (crate::ui::vide("Aucun stock enregistre.", None))
            } @else {
                table {
                    thead { tr { th { "Produit" } th { "Designation" } th { "Entrepot" } th .num { "Quantite" } } }
                    tbody {
                        @for n in &niveaux {
                            tr {
                                td .ref { (n.ref_produit) }
                                td { (n.label_produit) }
                                td .ref { (n.entrepot) }
                                td .num { (app::fmt_qte(Some(n.quantite))) }
                            }
                        }
                    }
                }
            }

            h2 { "Nouveau mouvement" }
            @if entrepots.is_empty() {
                (crate::ui::vide("Creez d'abord un entrepot.", None))
                form method="post" action="/stock/entrepot" .actions {
                    input type="text" name="ref" placeholder="Reference de l'entrepot" required;
                    input type="text" name="lieu" placeholder="Lieu";
                    button type="submit" { "Creer l'entrepot" }
                }
            } @else {
                form method="post" action="/stock/mouvement" {
                    table {
                        tbody {
                            tr {
                                th { "Produit" }
                                td {
                                    select name="produit" required style="width:100%" {
                                        @for p in &produits {
                                            option value=(p.rowid) { (p.ref_) " - " (p.label) }
                                        }
                                    }
                                }
                            }
                            tr {
                                th { "Entrepot" }
                                td {
                                    select name="entrepot" required {
                                        @for e in &entrepots {
                                            option value=(e.rowid) { (e.ref_) }
                                        }
                                    }
                                }
                            }
                            tr { th { "Quantite" } td { input type="number" name="quantite" step="0.01" min="0.01" value="1" style="width:110px"; } }
                            tr {
                                th { "Sens" }
                                td {
                                    select name="sens" {
                                        option value="0" { "Entree" }
                                        option value="1" { "Sortie" }
                                    }
                                }
                            }
                            tr { th { "Libelle" } td { input type="text" name="label" style="width:100%"; } }
                        }
                    }
                    div .actions { button type="submit" { "Enregistrer le mouvement" } }
                }
                form method="post" action="/stock/entrepot" .actions {
                    input type="text" name="ref" placeholder="Nouvel entrepot" required;
                    input type="text" name="lieu" placeholder="Lieu";
                    button type="submit" class="secondaire" { "Creer l'entrepot" }
                }
            }

            h2 { "Derniers mouvements" }
            @if mouvements.is_empty() {
                (crate::ui::vide("Aucun mouvement.", None))
            } @else {
                table {
                    thead { tr { th { "Date" } th { "Produit" } th { "Entrepot" } th { "Sens" } th .num { "Quantite" } th { "Libelle" } } }
                    tbody {
                        @for m in &mouvements {
                            tr {
                                td .ref { (app::fmt_date_fr(m.date.as_deref())) }
                                td .ref { (m.ref_produit) }
                                td .ref { (m.entrepot) }
                                td {
                                    @if m.type_ == 0 { "entree" } @else { "sortie" }
                                }
                                td .num { (app::fmt_qte(Some(m.quantite))) }
                                td { (m.label.clone().unwrap_or_default()) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub async fn stock_mouvement(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| f.get(c).map(|s| s.trim().to_string());
    let produit = champ("produit")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("produit obligatoire".into()))?;
    let entrepot = champ("entrepot")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("entrepot obligatoire".into()))?;
    let quantite = champ("quantite")
        .map(|v| v.replace(',', "."))
        .and_then(|v| v.parse::<rust_decimal::Decimal>().ok())
        .ok_or_else(|| Erreur("quantite invalide".into()))?;
    let sens: i16 = champ("sens").and_then(|v| v.parse().ok()).unwrap_or(0);
    let label = champ("label").filter(|s| !s.is_empty());
    db::stock::mouvement(
        &app.pool,
        ENTITY,
        produit,
        entrepot,
        quantite,
        sens,
        label.as_deref(),
    )?;
    Ok(Redirect::to("/stock?ok=mouvement"))
}

pub async fn stock_entrepot(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let reference = champ("ref").ok_or_else(|| Erreur("reference obligatoire".into()))?;
    let lieu = champ("lieu");
    db::stock::creer_entrepot(&app.pool, ENTITY, &reference, lieu.as_deref())?;
    Ok(Redirect::to("/stock?ok=entrepot"))
}

/// Expeditions clients.
pub async fn expeditions(State(app): State<crate::App>) -> Res<Markup> {
    let liste = db::divers::expeditions(&app.pool, ENTITY, 200)?;
    Ok(crate::ui::page(
        "devis",
        "expeditions",
        "Expeditions",
        html! {
            h1 { "Expeditions" }
            p .sous-titre { "Bons de livraison enregistres dans Dolibarr (llx_expedition)." }
            @if liste.is_empty() {
                (crate::ui::vide("Aucune expedition.", None))
            } @else {
                table {
                    thead { tr { th { "Ref." } th { "Tiers" } th { "Creation" } th { "Livraison" } th { "Statut" } } }
                    tbody {
                        @for e in &liste {
                            tr {
                                td .ref { (e.ref_) }
                                td { (e.tiers.clone().unwrap_or_default()) }
                                td .ref { (app::fmt_date_fr(e.date.as_deref())) }
                                td .ref { (app::fmt_date_fr(e.livraison.as_deref())) }
                                td {
                                    @if e.statut >= 2 { span .pastille.p-payee { "livree" } }
                                    @else if e.statut == 1 { span .pastille.p-validee { "validee" } }
                                    @else { span .pastille.p-brouillon { "brouillon" } }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Ecran principal d'un module, s'il existe dans le noyau.
pub fn ecran_module(code: &str) -> Option<&'static str> {
    Some(match code {
        "Societe" => "/tiers",
        "Propale" => "/devis",
        "Commande" => "/commandes",
        "Facture" => "/factures",
        "Product" | "Service" => "/produits",
        "Stock" => "/stock",
        "Contrat" => "/contrats",
        "Expedition" => "/expeditions",
        "Fournisseur" | "SupplierInvoice" | "SupplierOrder" => "/achats",
        "Banque" => "/banques",
        "Projet" => "/chantiers",
        "Agenda" => "/agenda",
        "ECM" => "/documents",
        "Comptabilite" | "Accounting" => "/comptabilite",
        "User" => "/parametres/utilisateurs",
        "Import" | "Export" => "/parametres/import",
        "Cron" => "/parametres/sauvegarde",
        _ => return None,
    })
}

/// Page de configuration d'un module, quand elle existe.
///
/// Dolibarr affiche une roue crantee a cote de chaque module actif ; on fait
/// pareil, mais uniquement quand le reglage existe reellement.
pub fn configuration_module(code: &str) -> Option<&'static str> {
    Some(match code {
        "Facture" | "Propale" => "/parametres/reglage/pdf",
        "Mailing" | "Notification" => "/parametres/reglage/emails",
        "Societe" => "/parametres",
        "User" => "/parametres/utilisateurs",
        "Comptabilite" | "Accounting" | "Tax" => "/parametres/reglage/limites",
        "Import" | "Export" => "/parametres/import",
        "Cron" => "/parametres/sauvegarde",
        "Api" | "WebServices" => "/parametres/reglage/securite",
        _ => return None,
    })
}
