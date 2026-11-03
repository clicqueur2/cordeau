//! Ecrans de liste et de reglage qui remplacent les entrees vides du menu.
//!
//! Regle : un ecran qui n'affiche rien n'existe pas. Chaque page ci-dessous lit
//! de vraies donnees ou ecrit de vraies constantes Dolibarr.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use maud::{html, Markup};

use crate::{message, Erreur, Res};

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
            Champ { cle: "DOLIBARR_RS_PDF_MODELE", libelle: "Modele de PDF (standard, epure, compact)", genre: "text", defaut: "standard" },
            Champ { cle: "DOLIBARR_RS_PDF_MARGE_HAUT", libelle: "Marge haute", genre: "nombre", defaut: "14" },
            Champ { cle: "DOLIBARR_RS_PDF_MARGE_BAS", libelle: "Marge basse", genre: "nombre", defaut: "34" },
            Champ { cle: "DOLIBARR_RS_PDF_MARGE_COTE", libelle: "Marge laterale", genre: "nombre", defaut: "10" },
        ],
    },
    Section {
        cle: "emails",
        titre: "Emails et notifications",
        intro: "Parametres SMTP. L'envoi est actif : serveur, compte, mot de passe, puis un essai pour verifier.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_SMTP_HOST", libelle: "Serveur SMTP", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_SMTP_PORT", libelle: "Port", genre: "nombre", defaut: "587" },
            Champ { cle: "DOLIBARR_RS_SMTP_UTILISATEUR", libelle: "Utilisateur", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_EMAIL_EXPEDITEUR", libelle: "Adresse d'expedition", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_SMTP_MOTDEPASSE", libelle: "Mot de passe SMTP", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_SMTP_SECURITE", libelle: "Securite (aucune, starttls, directe)", genre: "text", defaut: "" },
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
            @if s.cle == "emails" {
                h2 { "Envoyer un essai" }
                p .sous-titre {
                    "Le message part vraiment, par le serveur ci-dessus. Une erreur SMTP s'affiche "
                    "telle quelle : c'est elle qui dit quoi corriger."
                }
                (message(q.get("ok")))
                @if let Some(e) = q.get("erreur") {
                    p .erreur { (e) }
                }
                form method="post" action="/parametres/emails/essai" {
                    table {
                        tbody {
                            tr {
                                th { "Destinataire" }
                                td {
                                    input type="email" name="destinataire" required
                                          value=(q.get("destinataire").cloned().unwrap_or_default())
                                          style="width:100%";
                                }
                            }
                        }
                    }
                    div .actions { button type="submit" { "Envoyer l'essai" } }
                }
            }
        },
    ))
}

/// Envoie un e-mail d'essai par le serveur SMTP configure.
///
/// C'est le seul endroit qui parle au reseau pour du courrier : l'appel est
/// bloquant, donc execute hors du fil asynchrone. Une erreur SMTP remonte son
/// code et sa phrase, telles quelles — « 535 5.7.8 Authentication failed »
/// vaut mieux qu'un « l'envoi a echoue ».
pub async fn email_essai(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let destinataire = f
        .get("destinataire")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("destinataire obligatoire".into()))?;

    let const_ou = |cle: &str| {
        db::company::get_const(&app.pool, cle)
            .ok()
            .flatten()
            .unwrap_or_default()
    };
    let hote = const_ou("DOLIBARR_RS_SMTP_HOST");
    if hote.trim().is_empty() {
        return Ok(Redirect::to(
            "/parametres/reglage/emails?erreur=Serveur%20SMTP%20non%20renseigne",
        ));
    }
    let port = const_ou("DOLIBARR_RS_SMTP_PORT")
        .parse::<u16>()
        .unwrap_or(587);
    let mut mot_de_passe = const_ou("DOLIBARR_RS_SMTP_MOTDEPASSE");
    // La variable d'environnement l'emporte : un secret n'a pas a trainer en
    // base quand le serveur peut le fournir.
    if let Ok(v) = std::env::var("DOLIBARR_SMTP_PASSWORD") {
        if !v.is_empty() {
            mot_de_passe = v;
        }
    }

    let config = mail::Config {
        hote: hote.trim().to_string(),
        port,
        utilisateur: const_ou("DOLIBARR_RS_SMTP_UTILISATEUR").trim().to_string(),
        mot_de_passe,
        expediteur: const_ou("DOLIBARR_RS_EMAIL_EXPEDITEUR").trim().to_string(),
        nom_expediteur: const_ou("MAIN_INFO_SOCIETE_NOM").trim().to_string(),
        securite: match const_ou("DOLIBARR_RS_SMTP_SECURITE").trim() {
            "aucune" => mail::Securite::Aucune,
            "directe" => mail::Securite::Directe,
            "starttls" => mail::Securite::Starttls,
            // Par defaut : 465 direct, tout le reste en STARTTLS.
            _ => mail::Config::securite_pour(port),
        },
        delai: 20,
    };

    let message = mail::Message::texte(
        &config.expediteur,
        &[destinataire.as_str()],
        "Essai d'envoi - noyau Dolibarr en Rust",
        "Ce message a ete envoye par le noyau.

Si vous le lisez, le serveur SMTP, le compte et le mot de passe sont bons.

--\nNoyau Dolibarr en Rust",
    );

    let resultat = tokio::task::spawn_blocking(move || mail::envoyer(&config, &message)).await;

    match resultat {
        Ok(Ok(())) => Ok(Redirect::to("/parametres/reglage/emails?ok=essai_envoye")),
        Ok(Err(e)) => Ok(Redirect::to(&format!(
            "/parametres/reglage/emails?erreur={}&destinataire={}",
            encode(&e.to_string()),
            encode(&destinataire)
        ))),
        Err(e) => Ok(Redirect::to(&format!(
            "/parametres/reglage/emails?erreur={}",
            encode(&format!("tache interrompue : {e}"))
        ))),
    }
}

/// Encode une valeur pour un parametre d'URL.
fn encode(s: &str) -> String {
    let mut sortie = String::new();
    for octet in s.bytes() {
        match octet {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                sortie.push(octet as char)
            }
            b' ' => sortie.push('+'),
            autre => sortie.push_str(&format!("%{autre:02X}")),
        }
    }
    sortie
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
    let liste = db::divers::reglements(&app.pool, crate::entite::courante(), 200)?;
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
    let liste: Vec<_> = db::invoice::list(&app.pool, crate::entite::courante(), 500, 0)?
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
    let liste = db::divers::contrats(&app.pool, crate::entite::courante(), 200)?;
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
            @let mes_groupes = db::groupe::groupes_de(&app.pool, crate::entite::courante(), id)?;
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
        db::droit::definir(&app.pool, crate::entite::courante(), id, d.id, actif)?;
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
    let liste = db::groupe::list(&app.pool, crate::entite::courante())?;
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
    db::groupe::creer(&app.pool, crate::entite::courante(), &nom, note.as_deref())?;
    Ok(Redirect::to("/parametres/groupes?ok=cree"))
}

/// Fiche d'un groupe : membres et droits.
pub async fn groupe_fiche(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let g = db::groupe::get(&app.pool, crate::entite::courante(), id)?
        .ok_or_else(|| Erreur("groupe introuvable".into()))?;
    let membres = db::groupe::membres(&app.pool, crate::entite::courante(), id)?;
    let droits = db::groupe::droits(&app.pool, id)?;
    let utilisateurs = db::user::list(&app.pool, crate::entite::courante())?;

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
    for u in db::user::list(&app.pool, crate::entite::courante())? {
        let membre = f.contains_key(&format!("u{}", u.rowid));
        db::groupe::definir_membre(&app.pool, crate::entite::courante(), id, u.rowid, membre)?;
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
        db::groupe::definir_droit(&app.pool, crate::entite::courante(), id, d.id, actif)?;
    }
    Ok(Redirect::to(&format!("/parametres/groupes/{id}?ok=droits")))
}

/// Stock : niveaux par produit et entrepot, derniers mouvements.
pub async fn stock(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let niveaux = db::stock::niveaux(&app.pool, crate::entite::courante())?;
    let mouvements = db::stock::mouvements(&app.pool, crate::entite::courante(), 50)?;
    let entrepots = db::stock::entrepots(&app.pool, crate::entite::courante())?;
    let produits = db::product::list(&app.pool, crate::entite::courante(), 200, 0)?;

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
        crate::entite::courante(),
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
    db::stock::creer_entrepot(
        &app.pool,
        crate::entite::courante(),
        &reference,
        lieu.as_deref(),
    )?;
    Ok(Redirect::to("/stock?ok=entrepot"))
}

/// Expeditions clients.
pub async fn expeditions(State(app): State<crate::App>) -> Res<Markup> {
    let liste = db::divers::expeditions(&app.pool, crate::entite::courante(), 200)?;
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

/// Options d'un module : les constantes qu'il declare, lues et ecrites dans
/// `llx_const`. C'est la page que Dolibarr affiche derriere la roue crantee.
pub async fn options_module(
    State(app): State<crate::App>,
    axum::extract::Path(code): axum::extract::Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let module =
        crate::plugin::par_code(&code).ok_or_else(|| Erreur(format!("module inconnu : {code}")))?;
    let actif = crate::menu_actif::est_actif(module.code);

    let mut valeurs = Vec::new();
    for o in module.options {
        let v = db::company::get_const(&app.pool, o.cle)?
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| o.defaut.to_string());
        valeurs.push(v);
    }

    Ok(crate::ui::page(
        "parametres",
        "modules",
        module.nom,
        html! {
            (message(q.get("ok")))
            h1 { "Options du module " (module.nom) }
            p .sous-titre {
                "Version " (module.version) " — constante d'activation "
                code { "MAIN_MODULE_" (module.code.to_uppercase()) }
                @if actif { " (activé)" } @else { " (inactif)" }
            }

            @if module.options.is_empty() {
                (crate::ui::vide("Ce module n'a pas d'option réglable.", Some(("/parametres/modules", "Retour aux modules"))))
            } @else {
                form method="post" action={ "/parametres/module/" (module.code) } {
                    table {
                        tbody {
                            @for (i, o) in module.options.iter().enumerate() {
                                tr {
                                    th { (o.libelle) }
                                    td {
                                        @if o.genre == crate::plugin::Genre::OuiNon {
                                            select name=(o.cle) {
                                                option value="0" selected[valeurs[i] != "1"] { "Non" }
                                                option value="1" selected[valeurs[i] == "1"] { "Oui" }
                                            }
                                        } @else if o.genre == crate::plugin::Genre::Nombre {
                                            input type="number" name=(o.cle) value=(valeurs[i]) style="width:130px";
                                        } @else {
                                            input type="text" name=(o.cle) value=(valeurs[i]) style="width:100%";
                                        }
                                        p style="color:#78736c;font-size:12px;margin:6px 0 0" {
                                            (o.aide) " — constante " code { (o.cle) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div .actions {
                        button type="submit" { "Enregistrer les options" }
                        a .bouton.secondaire href="/parametres/modules" { "Retour aux modules" }
                    }
                }
            }

            h2 { "Ecrans fournis" }
            @if module.entrees.is_empty() {
                p { "Aucun écran déclaré." }
            } @else {
                ul {
                    @for e in module.entrees {
                        li { a href=(e.route) { (e.libelle) } }
                    }
                }
            }
        },
    ))
}

pub async fn options_module_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(code): axum::extract::Path<String>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let module =
        crate::plugin::par_code(&code).ok_or_else(|| Erreur(format!("module inconnu : {code}")))?;
    for o in module.options {
        let valeur = f.get(o.cle).cloned().unwrap_or_default();
        db::company::set_const(&app.pool, o.cle, valeur.trim())?;
    }
    Ok(Redirect::to(&format!(
        "/parametres/module/{code}?ok=options"
    )))
}

/// Contacts et adresses, comme Dolibarr : la personne distincte de la societe.
pub async fn contacts(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let liste = db::contact::list(&app.pool, crate::entite::courante(), 300)?;
    let clients = db::party::list(&app.pool, crate::entite::courante(), 500, 0)?;

    Ok(crate::ui::page(
        "tiers",
        "contacts",
        "Contacts et adresses",
        html! {
            (message(q.get("ok")))
            h1 { "Contacts et adresses" }
            p .sous-titre {
                "Un contact est une personne ; le tiers est la société. Les deux sont séparés, comme dans Dolibarr."
            }

            h2 { "Contacts" }
            @if liste.is_empty() {
                (crate::ui::vide("Aucun contact.", None))
            } @else {
                table {
                    thead { tr { th { "Nom" } th { "Société" } th { "Fonction" } th { "E-mail" } th { "Téléphone" } th { "Ville" } } }
                    tbody {
                        @for c in &liste {
                            tr {
                                td { strong { (c.nom_complet()) } }
                                td { (c.tiers.clone().unwrap_or_default()) }
                                td { (c.poste.clone().unwrap_or_default()) }
                                td .ref { (c.email.clone().unwrap_or_default()) }
                                td .ref { (c.telephone.clone().unwrap_or_default()) }
                                td { (c.ville.clone().unwrap_or_default()) }
                            }
                        }
                    }
                }
            }

            h2 { "Nouveau contact" }
            form method="post" action="/contacts" {
                table {
                    tbody {
                        tr {
                            th { "Société" }
                            td {
                                select name="soc" style="width:100%" {
                                    option value="" { "— aucun (particulier) —" }
                                    @for s in &clients {
                                        option value=(s.rowid) { (s.nom) }
                                    }
                                }
                            }
                        }
                        tr {
                            th { "Civilité" }
                            td {
                                select name="civilite" {
                                    option value="" { "—" }
                                    option value="M." { "M." }
                                    option value="Mme" { "Mme" }
                                    option value="Mlle" { "Mlle" }
                                }
                            }
                        }
                        tr { th { "Nom" } td { input type="text" name="nom" required style="width:100%"; } }
                        tr { th { "Prénom" } td { input type="text" name="prenom" style="width:100%"; } }
                        tr { th { "Fonction" } td { input type="text" name="poste" style="width:100%"; } }
                        tr { th { "E-mail" } td { input type="email" name="email" style="width:100%"; } }
                        tr { th { "Téléphone" } td { input type="text" name="telephone" style="width:100%"; } }
                    }
                }
                div .actions { button type="submit" { "Créer le contact" } }
            }
        },
    ))
}

pub async fn contact_creer(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let nom = champ("nom").ok_or_else(|| Erreur("nom du contact obligatoire".into()))?;
    let fk_soc = champ("soc").and_then(|v| v.parse::<i64>().ok());
    db::contact::creer(
        &app.pool,
        crate::entite::courante(),
        fk_soc,
        champ("civilite").as_deref(),
        &nom,
        champ("prenom").as_deref(),
        champ("email").as_deref(),
        champ("telephone").as_deref(),
        champ("poste").as_deref(),
    )?;
    Ok(Redirect::to("/contacts?ok=cree"))
}

/// Nouvel evenement d'agenda (`llx_actioncomm`, type Rendez-vous).
pub async fn agenda_creer(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let label = champ("label").ok_or_else(|| Erreur("libelle obligatoire".into()))?;
    let date = champ("date");
    let tiers = champ("soc").and_then(|v| v.parse::<i64>().ok());
    db::journal::creer_evenement(
        &app.pool,
        crate::entite::courante(),
        &label,
        date.as_deref(),
        tiers,
    )?;
    Ok(Redirect::to("/agenda?ok=evenement"))
}

/// Commandes fournisseurs (lecture) : `llx_commande_fournisseur`.
pub async fn commandes_fournisseurs(State(app): State<crate::App>) -> Res<Markup> {
    let liste = db::divers::commandes_fournisseurs(&app.pool, crate::entite::courante(), 200)?;
    Ok(crate::ui::page(
        "devis",
        "commandes-fournisseurs",
        "Commandes fournisseurs",
        html! {
            h1 { "Commandes fournisseurs" }
            p .sous-titre { "Lues dans llx_commande_fournisseur : le noyau les affiche, l'écriture viendra." }
            @if liste.is_empty() {
                (crate::ui::vide("Aucune commande fournisseur.", None))
            } @else {
                table {
                    thead { tr { th { "Ref." } th { "Fournisseur" } th { "Chez le fournisseur" } th { "Date" } th { "Statut" } th .num { "TTC" } } }
                    tbody {
                        @for c in &liste {
                            tr {
                                td .ref { (c.ref_) }
                                td { (c.tiers.clone().unwrap_or_default()) }
                                td .ref { (c.ref_supplier.clone().unwrap_or_default()) }
                                td .ref { (app::fmt_date_fr(c.date.as_deref())) }
                                td { (c.statut) }
                                td .num { (crate::ui::euros(c.total_ttc.unwrap_or_default())) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Journal comptable d'un type : ventes ou achats.
pub async fn journal_comptable(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let sens = q
        .get("sens")
        .cloned()
        .unwrap_or_else(|| "ventes".to_string());
    let debut = q.get("debut").cloned().filter(|s| !s.is_empty());
    let fin = q.get("fin").cloned().filter(|s| !s.is_empty());

    let lignes = if sens == "achats" {
        db::compta::journal_achats(
            &app.pool,
            crate::entite::courante(),
            debut.as_deref(),
            fin.as_deref(),
        )?
    } else {
        db::compta::journal_ventes(
            &app.pool,
            crate::entite::courante(),
            debut.as_deref(),
            fin.as_deref(),
        )?
    };
    let total_ht: rust_decimal::Decimal = lignes.iter().map(|l| l.total_ht).sum();
    let total_tva: rust_decimal::Decimal = lignes.iter().map(|l| l.total_tva).sum();
    let total_ttc: rust_decimal::Decimal = lignes.iter().map(|l| l.total_ttc).sum();
    let titre = if sens == "achats" {
        "Journal des achats"
    } else {
        "Journal des ventes"
    };

    Ok(crate::ui::page(
        "comptabilite",
        "journaux",
        titre,
        html! {
            h1 { (titre) }
            p .sous-titre { "Écritures dérivées des documents, triées par date." }
            form method="get" action="/comptabilite/journal" .actions {
                input type="hidden" name="sens" value=(sens);
                input type="date" name="debut" value=(debut.clone().unwrap_or_default()) aria-label="Début";
                input type="date" name="fin" value=(fin.clone().unwrap_or_default()) aria-label="Fin";
                button type="submit" { "Filtrer" }
                a .bouton.secondaire href={ "/export/ventes.csv?debut=" (debut.clone().unwrap_or_default()) "&fin=" (fin.clone().unwrap_or_default()) } { "CSV ventes" }
                a .bouton.secondaire href={ "/export/achats.csv?debut=" (debut.clone().unwrap_or_default()) "&fin=" (fin.clone().unwrap_or_default()) } { "CSV achats" }
            }
            @if lignes.is_empty() {
                (crate::ui::vide("Aucune écriture sur la période.", None))
            } @else {
                table {
                    thead { tr { th { "Date" } th { "Référence" } th { "Type" } th { "Tiers" } th .num { "HT" } th .num { "TVA" } th .num { "TTC" } } }
                    tbody {
                        @for l in &lignes {
                            tr {
                                td .ref { (app::fmt_date_fr(Some(&l.date))) }
                                td .ref { (l.reference) }
                                td { (l.type_document) }
                                td { (l.tiers) }
                                td .num { (crate::ui::euros(l.total_ht)) }
                                td .num { (crate::ui::euros(l.total_tva)) }
                                td .num { (crate::ui::euros(l.total_ttc)) }
                            }
                        }
                    }
                    tfoot {
                        tr {
                            td colspan="4" .num { "Totaux" }
                            td .num { (crate::ui::euros(total_ht)) }
                            td .num { (crate::ui::euros(total_tva)) }
                            td .num { (crate::ui::euros(total_ttc)) }
                        }
                    }
                }
            }
        },
    ))
}

/// Tickets de support : reclamations clients.
pub async fn tickets(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let liste = db::ticket::list(&app.pool, crate::entite::courante(), 200)?;
    let clients = db::party::list(&app.pool, crate::entite::courante(), 500, 0)?;

    Ok(crate::ui::page(
        "tickets",
        "liste",
        "Tickets",
        html! {
            (message(q.get("ok")))
            h1 { "Tickets de support" }
            p .sous-titre {
                "Une réclamation client tracée : sujet, société, statut. Numérotation TK + AAMM + compteur."
            }

            @if liste.is_empty() {
                (crate::ui::vide("Aucun ticket.", None))
            } @else {
                table {
                    thead { tr { th { "Réf." } th { "Sujet" } th { "Société" } th { "Date" } th { "Gravité" } th { "État" } th { "" } } }
                    tbody {
                        @for t in &liste {
                            tr {
                                td .ref { (t.ref_) }
                                td { (t.sujet) }
                                td { (t.tiers.clone().unwrap_or_default()) }
                                td .ref { (app::fmt_date_fr(t.date.as_deref())) }
                                td { (t.gravite.clone().unwrap_or_default()) }
                                td {
                                    @if t.statut == 8 { span .pastille.p-payee { "fermé" } }
                                    @else if t.statut == 0 { span .pastille.p-brouillon { "nouveau" } }
                                    @else { span .pastille.p-validee { "en cours" } }
                                }
                                td {
                                    @if t.statut != 8 {
                                        form method="post" action={ "/tickets/" (t.rowid) "/fermer" } {
                                            button type="submit" class="secondaire petit" { "Fermer" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 { "Nouveau ticket" }
            form method="post" action="/tickets" {
                table {
                    tbody {
                        tr {
                            th { "Société" }
                            td {
                                select name="soc" style="width:100%" {
                                    option value="" { "— aucun —" }
                                    @for c in &clients {
                                        option value=(c.rowid) { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr { th { "Sujet" } td { input type="text" name="sujet" required style="width:100%"; } }
                        tr { th { "Message" } td { textarea name="message" rows="4" style="width:100%;font:inherit;padding:8px 10px;border:1px solid #e6e2dc" {} } }
                        tr {
                            th { "Gravité" }
                            td {
                                select name="gravite" {
                                    option value="LOW" { "Basse" }
                                    option value="NORMAL" { "Normale" }
                                    option value="HIGH" { "Haute" }
                                    option value="BLOCKING" { "Bloquante" }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" { "Créer le ticket" } }
            }
        },
    ))
}

pub async fn ticket_creer(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let sujet = champ("sujet").ok_or_else(|| Erreur("sujet du ticket obligatoire".into()))?;
    let fk_soc = champ("soc").and_then(|v| v.parse::<i64>().ok());
    db::ticket::creer(
        &app.pool,
        crate::entite::courante(),
        fk_soc,
        &sujet,
        champ("message").as_deref(),
        champ("gravite").as_deref(),
    )?;
    Ok(Redirect::to("/tickets?ok=cree"))
}

pub async fn ticket_fermer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Res<Redirect> {
    db::ticket::fermer(&app.pool, crate::entite::courante(), id)?;
    Ok(Redirect::to("/tickets?ok=ferme"))
}

/// Edition d'un contact.
pub async fn contact_fiche(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let c = db::contact::get(&app.pool, crate::entite::courante(), id)?
        .ok_or_else(|| Erreur("contact introuvable".into()))?;

    Ok(crate::ui::page(
        "tiers",
        "contacts",
        &c.nom_complet(),
        html! {
            (message(q.get("ok")))
            h1 { (c.nom_complet()) }
            p .sous-titre {
                @if let Some(t) = &c.tiers { "Société : " strong { (t) } }
                @else { "Sans société (particulier)" }
            }
            form method="post" action={ "/contacts/" (id) "/modifier" } {
                table {
                    tbody {
                        tr { th { "Nom" } td { input type="text" name="nom" value=(c.nom) required style="width:100%"; } }
                        tr { th { "Prénom" } td { input type="text" name="prenom" value=(c.prenom.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th { "Fonction" } td { input type="text" name="poste" value=(c.poste.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th { "E-mail" } td { input type="email" name="email" value=(c.email.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th { "Téléphone" } td { input type="text" name="telephone" value=(c.telephone.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th { "Ville" } td { input type="text" name="ville" value=(c.ville.clone().unwrap_or_default()) style="width:100%"; } }
                    }
                }
                div .actions {
                    button type="submit" { "Enregistrer" }
                    a .bouton.secondaire href="/contacts" { "Retour" }
                }
            }
        },
    ))
}

pub async fn contact_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let nom = champ("nom").ok_or_else(|| Erreur("nom obligatoire".into()))?;
    db::contact::modifier(
        &app.pool,
        crate::entite::courante(),
        id,
        &nom,
        champ("prenom").as_deref(),
        champ("email").as_deref(),
        champ("telephone").as_deref(),
        champ("poste").as_deref(),
        champ("ville").as_deref(),
    )?;
    Ok(Redirect::to(&format!(
        "/contacts/{id}/modifier?ok=enregistre"
    )))
}

/// Fiche d'un ticket : message complet et changement de statut.
pub async fn ticket_fiche(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let t = db::ticket::get(&app.pool, crate::entite::courante(), id)?
        .ok_or_else(|| Erreur("ticket introuvable".into()))?;

    Ok(crate::ui::page(
        "tickets",
        "liste",
        &t.ref_,
        html! {
            (message(q.get("ok")))
            h1 { (t.sujet) " " }
            p .sous-titre {
                (t.ref_) " — " (t.tiers.clone().unwrap_or_else(|| "sans société".into()))
                @if let Some(d) = &t.date { " — ouvert le " (app::fmt_date_fr(Some(d))) }
                @if let Some(g) = &t.gravite { " — gravité " (g) }
            }
            @if let Some(m) = &t.message {
                div .carte style="margin-bottom:22px" {
                    div .libelle { "Message" }
                    p style="white-space:pre-wrap" { (m) }
                }
            }
            form method="post" action={ "/tickets/" (id) "/statut" } .actions {
                select name="statut" {
                    option value="0" selected[t.statut == 0] { "Nouveau" }
                    option value="1" selected[t.statut == 1] { "En cours" }
                    option value="8" selected[t.statut == 8] { "Fermé" }
                }
                button type="submit" { "Changer l'état" }
                a .bouton.secondaire href="/tickets" { "Retour" }
            }
        },
    ))
}

pub async fn ticket_statut(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let statut: i16 = f
        .get("statut")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Erreur("statut invalide".into()))?;
    db::ticket::changer_statut(&app.pool, crate::entite::courante(), id, statut)?;
    Ok(Redirect::to(&format!("/tickets/{id}?ok=statut")))
}

/// Edition d'un chantier.
pub async fn chantier_modifier(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let c = db::projet::get(&app.pool, crate::entite::courante(), id)?
        .ok_or_else(|| Erreur("chantier introuvable".into()))?;
    let clients = db::party::list(&app.pool, crate::entite::courante(), 500, 0)?;

    Ok(crate::ui::page(
        "chantiers",
        "liste",
        &c.ref_,
        html! {
            (message(q.get("ok")))
            h1 { "Modifier " (c.ref_) }
            p .sous-titre { (c.title) }
            form method="post" action={ "/chantiers/" (id) "/modifier" } {
                table {
                    tbody {
                        tr {
                            th { "Client" }
                            td {
                                select name="soc" style="width:100%" {
                                    option value="" { "— chantier interne —" }
                                    @for s in &clients {
                                        option value=(s.rowid) selected[c.fk_soc == Some(s.rowid)] { (s.nom) }
                                    }
                                }
                            }
                        }
                        tr { th { "Intitulé" } td { input type="text" name="title" value=(c.title) required style="width:100%"; } }
                        tr { th { "Début" } td { input type="date" name="debut" value=(c.date_start.clone().unwrap_or_default()) style="width:170px"; } }
                        tr { th { "Fin prévue" } td { input type="date" name="fin" value=(c.date_end.clone().unwrap_or_default()) style="width:170px"; } }
                        tr { th { "Budget HT" } td { input type="number" name="budget" step="0.01" min="0" value=(app::fmt_dec(c.budget)) style="width:150px"; } }
                        tr { th { "Description" } td { input type="text" name="description" value=(c.description.clone().unwrap_or_default()) style="width:100%"; } }
                    }
                }
                div .actions {
                    button type="submit" { "Enregistrer" }
                    a .bouton.secondaire href={ "/chantiers/" (id) } { "Retour" }
                }
            }
        },
    ))
}

pub async fn chantier_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let titre = champ("title").ok_or_else(|| Erreur("intitulé obligatoire".into()))?;
    let mut nouveau = db::projet::NewProjet::new(&titre);
    nouveau.fk_soc = champ("soc").and_then(|v| v.parse().ok());
    nouveau.date_start = champ("debut");
    nouveau.date_end = champ("fin");
    nouveau.description = champ("description");
    nouveau.budget = f
        .get("budget")
        .map(|v| v.replace(',', "."))
        .and_then(|v| v.parse().ok())
        .unwrap_or_default();
    db::projet::update(&app.pool, crate::entite::courante(), id, &nouveau)?;
    Ok(Redirect::to(&format!(
        "/chantiers/{id}/modifier?ok=enregistre"
    )))
}

/// Fiche d'un fournisseur.
pub async fn fournisseur_fiche(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let f = db::achat::fournisseur_par_id(&app.pool, crate::entite::courante(), id)?
        .ok_or_else(|| Erreur("fournisseur introuvable".into()))?;
    let factures: Vec<_> =
        db::achat::factures_fournisseur(&app.pool, crate::entite::courante(), 500, 0)?
            .into_iter()
            .filter(|x| x.fk_soc == id)
            .collect();
    let total: rust_decimal::Decimal = factures
        .iter()
        .map(|x| x.total_ttc.unwrap_or_default())
        .sum();

    Ok(crate::ui::page(
        "achats",
        "fournisseurs",
        &f.nom,
        html! {
            (message(q.get("ok")))
            h1 { (f.nom) }
            p .sous-titre {
                "Code " code { (f.code_fournisseur.clone().unwrap_or_default()) }
                " — " (factures.len()) " facture(s), " (crate::ui::euros(total)) " TTC"
            }
            form method="post" action={ "/achats/fournisseur/" (id) "/modifier" } {
                table {
                    tbody {
                        tr { th { "Nom" } td { input type="text" name="nom" value=(f.nom) required style="width:100%"; } }
                        tr { th { "E-mail" } td { input type="email" name="email" value=(f.email.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th { "Téléphone" } td { input type="text" name="telephone" value="" style="width:100%"; } }
                        tr { th { "Adresse" } td { input type="text" name="adresse" value=(f.address.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th { "Ville" } td { input type="text" name="ville" value=(f.town.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th { "SIRET" } td { input type="text" name="siret" value=(f.siret.clone().unwrap_or_default()) style="width:100%"; } }
                    }
                }
                div .actions {
                    button type="submit" { "Enregistrer" }
                    a .bouton.secondaire href="/achats" { "Retour" }
                }
            }

            @if !factures.is_empty() {
                h2 { "Factures fournisseurs" }
                table {
                    thead { tr { th { "Réf." } th { "Chez le fournisseur" } th { "Date" } th .num { "TTC" } } }
                    tbody {
                        @for x in &factures {
                            tr {
                                td .ref { (x.ref_) }
                                td .ref { (x.ref_supplier.clone().unwrap_or_default()) }
                                td .ref { (app::fmt_date_fr(x.datef.as_deref())) }
                                td .num { (crate::ui::euros(x.total_ttc.unwrap_or_default())) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub async fn fournisseur_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let nom = champ("nom").ok_or_else(|| Erreur("nom obligatoire".into()))?;
    db::achat::modifier_fournisseur(
        &app.pool,
        crate::entite::courante(),
        id,
        &nom,
        champ("email").as_deref(),
        champ("telephone").as_deref(),
        champ("adresse").as_deref(),
        champ("ville").as_deref(),
        champ("siret").as_deref(),
    )?;
    Ok(Redirect::to(&format!(
        "/achats/fournisseur/{id}/modifier?ok=enregistre"
    )))
}

/// Fiche d'un compte bancaire.
pub async fn compte_fiche(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let c = db::banque::compte(&app.pool, crate::entite::courante(), id)?
        .ok_or_else(|| Erreur("compte introuvable".into()))?;
    let solde = db::banque::solde(&app.pool, crate::entite::courante(), id)?;

    Ok(crate::ui::page(
        "banques",
        "comptes",
        &c.ref_,
        html! {
            (message(q.get("ok")))
            h1 { (c.label) }
            p .sous-titre { "Référence " code { (c.ref_) } " — solde " strong { (crate::ui::euros(solde)) } }
            form method="post" action={ "/banques/" (id) "/modifier" } {
                table {
                    tbody {
                        tr { th { "Libellé" } td { input type="text" name="label" value=(c.label) required style="width:100%"; } }
                        tr { th { "Banque" } td { input type="text" name="banque" value=(c.banque.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th { "IBAN" } td { input type="text" name="iban" value=(c.iban.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th { "BIC" } td { input type="text" name="bic" value=(c.bic.clone().unwrap_or_default()) style="width:260px"; } }
                        tr {
                            th { "Compte clos" }
                            td {
                                select name="clos" {
                                    option value="0" selected[!c.clos] { "Non" }
                                    option value="1" selected[c.clos] { "Oui" }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" { "Enregistrer" }
                    a .bouton.secondaire href="/banques" { "Retour" }
                }
            }
        },
    ))
}

pub async fn compte_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let label = champ("label").ok_or_else(|| Erreur("libellé obligatoire".into()))?;
    db::banque::modifier_compte(
        &app.pool,
        crate::entite::courante(),
        id,
        &label,
        champ("banque").as_deref(),
        champ("iban").as_deref(),
        champ("bic").as_deref(),
        f.get("clos").map(|v| v == "1").unwrap_or(false),
    )?;
    Ok(Redirect::to(&format!(
        "/banques/{id}/modifier?ok=enregistre"
    )))
}

/// Une liste en lecture seule : un module, une requete, des colonnes.
///
/// Ces modules existent dans Dolibarr et ont un ecran ; le noyau les affiche
/// sans les ecrire. C'est moins que Dolibarr, mais ce n'est pas rien : on voit
/// ce que la base contient, et l'ecriture se fait dans Dolibarr si besoin.
pub struct ListeSimple {
    pub code: &'static str,
    pub titre: &'static str,
    pub intro: &'static str,
    pub sql: &'static str,
    pub colonnes: &'static [&'static str],
}

pub const LISTES: &[ListeSimple] = &[
    ListeSimple {
        code: "Adherent",
        titre: "Adhérents",
        intro: "Adhérents enregistrés dans Dolibarr (llx_adherent).",
        sql: "SELECT rowid, ref, CONCAT(lastname, ' ', COALESCE(firstname, '')) AS nom, \
                     COALESCE(town, ''), COALESCE(email, ''), COALESCE(datefin, '') \
              FROM llx_adherent WHERE entity = :entity \
              ORDER BY rowid DESC LIMIT 200",
        colonnes: &["N°", "Réf.", "Nom", "Ville", "E-mail", "Adhésion jusqu'au"],
    },
    ListeSimple {
        code: "Don",
        titre: "Dons",
        intro: "Dons reçus (llx_don), lecture seule.",
        sql: "SELECT rowid, ref, DATE_FORMAT(datedon, '%Y-%m-%d'), \
                     CAST(amount AS CHAR), CAST(fk_statut AS CHAR) \
              FROM llx_don WHERE entity = :entity \
              ORDER BY datedon DESC, rowid DESC LIMIT 200",
        colonnes: &["N°", "Réf.", "Date", "Montant", "État"],
    },
    ListeSimple {
        code: "Reception",
        titre: "Réceptions",
        intro: "Réceptions de marchandises (llx_reception), lecture seule.",
        sql: "SELECT r.ref, DATE_FORMAT(r.date_creation, '%Y-%m-%d'), \
                     COALESCE(s.nom, ''), CAST(r.fk_statut AS CHAR) \
              FROM llx_reception r LEFT JOIN llx_societe s ON s.rowid = r.fk_soc \
              WHERE r.entity = :entity \
              ORDER BY r.date_creation DESC, r.rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Date", "Fournisseur", "Statut"],
    },
    ListeSimple {
        code: "Categorie",
        titre: "Catégories",
        intro: "Catégories de produits, de tiers et de contacts (llx_categorie).",
        sql: "SELECT rowid, label, CAST(type AS CHAR), COALESCE(description, '') \
              FROM llx_categorie WHERE entity = :entity OR entity = 0 \
              ORDER BY label ASC LIMIT 200",
        colonnes: &["N°", "Libellé", "Type", "Description"],
    },
    ListeSimple {
        code: "ExpenseReport",
        titre: "Notes de frais",
        intro: "Notes de frais (llx_expensereport), lecture seule.",
        sql: "SELECT ref, DATE_FORMAT(date_debut, '%Y-%m-%d'), \
                     DATE_FORMAT(date_fin, '%Y-%m-%d'), \
                     CAST(total_ttc AS CHAR), CAST(fk_statut AS CHAR) \
              FROM llx_expensereport WHERE entity = :entity \
              ORDER BY date_debut DESC, rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Du", "Au", "Total TTC", "Statut"],
    },
    ListeSimple {
        code: "Holiday",
        titre: "Congés",
        intro: "Demandes de congés (llx_holiday), lecture seule.",
        sql: "SELECT ref, DATE_FORMAT(date_debut, '%Y-%m-%d'), \
                     DATE_FORMAT(date_fin, '%Y-%m-%d'), CAST(statut AS CHAR) \
              FROM llx_holiday WHERE entity = :entity \
              ORDER BY date_debut DESC, rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Du", "Au", "Statut"],
    },
    ListeSimple {
        code: "Mailing",
        titre: "Emailings",
        intro: "Campagnes d'e-mailing (llx_mailing), lecture seule.",
        sql: "SELECT titre, COALESCE(sujet, ''), DATE_FORMAT(date_creat, '%Y-%m-%d'), \
                     CASE statut WHEN 0 THEN 'Brouillon' WHEN 1 THEN 'Validé' \
                                 WHEN 2 THEN 'Envoyé' ELSE CAST(statut AS CHAR) END \
              FROM llx_mailing WHERE entity = :entity \
              ORDER BY date_creat DESC, rowid DESC LIMIT 200",
        colonnes: &["Titre", "Sujet", "Créé le", "Statut"],
    },
    ListeSimple {
        code: "Prelevement",
        titre: "Prélèvements",
        intro: "Bordereaux de prélèvement SEPA (llx_prelevement_bons), lecture seule.",
        sql: "SELECT p.ref, DATE_FORMAT(p.datec, '%Y-%m-%d'), \
                     CAST(p.amount AS CHAR), \
                     CASE p.statut WHEN 0 THEN 'Brouillon' WHEN 1 THEN 'Validé' \
                                   WHEN 2 THEN 'Déposé' WHEN 3 THEN 'Crédité' \
                                   WHEN 4 THEN 'Rejeté' ELSE CAST(p.statut AS CHAR) END \
              FROM llx_prelevement_bons p \
              WHERE p.entity = :entity \
              ORDER BY p.datec DESC, p.rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Date", "Montant", "Statut"],
    },
    ListeSimple {
        code: "Ficheinter",
        titre: "Fiches d'intervention",
        intro: "Fiches d'intervention (llx_fichinter), lecture seule.",
        sql: "SELECT f.ref, DATE_FORMAT(f.datec, '%Y-%m-%d'), \
                     COALESCE(s.nom, ''), \
                     CASE f.fk_statut WHEN 0 THEN 'Brouillon' WHEN 1 THEN 'Validée' \
                                      WHEN 2 THEN 'Clôturée' ELSE CAST(f.fk_statut AS CHAR) END \
              FROM llx_fichinter f \
              LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
              WHERE f.entity = :entity \
              ORDER BY f.datec DESC, f.rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Date", "Tiers", "Statut"],
    },
    ListeSimple {
        code: "Bookmark",
        titre: "Signets",
        intro: "Signets enregistrés par les utilisateurs (llx_bookmark).",
        sql: "SELECT title, url, DATE_FORMAT(dateb, '%Y-%m-%d'), \
                     COALESCE(u.login, '') \
              FROM llx_bookmark b \
              LEFT JOIN llx_user u ON u.rowid = b.fk_user \
              WHERE b.entity = :entity \
              ORDER BY b.position ASC, b.rowid ASC LIMIT 200",
        colonnes: &["Titre", "Adresse", "Créé le", "Utilisateur"],
    },
    ListeSimple {
        code: "Tax",
        titre: "Taux de TVA",
        intro: "Dictionnaire des taux de TVA (llx_c_tva) : c'est lui qui alimente \
                les factures, pas une liste figée dans le code.",
        sql: "SELECT rowid, code, CAST(taux AS CHAR), COALESCE(note, ''), \
                     CASE active WHEN 1 THEN 'actif' ELSE 'inactif' END \
              FROM llx_c_tva WHERE entity = :entity \
              ORDER BY taux DESC, code ASC LIMIT 200",
        colonnes: &["N°", "Code", "Taux", "Note", "État"],
    },
    ListeSimple {
        code: "Salaries",
        titre: "Salaires",
        intro: "Bulletins de salaire (llx_salary), lecture seule.",
        sql: "SELECT s.ref, DATE_FORMAT(s.datec, '%Y-%m-%d'), \
                     COALESCE(CONCAT(u.firstname, ' ', u.lastname), ''), \
                     CAST(s.amount AS CHAR), \
                     CASE s.paye WHEN 1 THEN 'Payé' ELSE 'À payer' END \
              FROM llx_salary s \
              LEFT JOIN llx_user u ON u.rowid = s.fk_user \
              WHERE s.entity = :entity \
              ORDER BY s.datec DESC, s.rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Date", "Salarié", "Montant", "Paiement"],
    },
    ListeSimple {
        code: "Loan",
        titre: "Prêts",
        intro: "Emprunts et échéanciers (llx_loan), lecture seule.",
        sql: "SELECT rowid, label, CAST(capital AS CHAR), \
                     DATE_FORMAT(datestart, '%Y-%m-%d'), CAST(nbterm AS CHAR), \
                     CAST(rate AS CHAR), CAST(capital_position AS CHAR) \
              FROM llx_loan WHERE entity = :entity \
              ORDER BY datestart DESC, rowid DESC LIMIT 200",
        colonnes: &[
            "N°",
            "Libellé",
            "Capital",
            "Début",
            "Échéances",
            "Taux",
            "Capital restant",
        ],
    },
    ListeSimple {
        code: "ProductBatch",
        titre: "Lots et séries",
        intro: "Lots suivis (llx_product_lot), lecture seule.",
        sql: "SELECT l.rowid, l.batch, COALESCE(p.ref, ''), COALESCE(p.label, ''), \
                     COALESCE(DATE_FORMAT(l.eatby, '%Y-%m-%d'), ''), \
                     COALESCE(DATE_FORMAT(l.sellby, '%Y-%m-%d'), '') \
              FROM llx_product_lot l \
              LEFT JOIN llx_product p ON p.rowid = l.fk_product \
              WHERE l.entity = :entity \
              ORDER BY l.rowid DESC LIMIT 200",
        colonnes: &[
            "N°",
            "Lot",
            "Réf. produit",
            "Libellé",
            "À consommer avant",
            "À vendre avant",
        ],
    },
    ListeSimple {
        code: "Accounting",
        titre: "Écritures comptables",
        intro: "Table llx_accounting_bookkeeping : ce que Dolibarr y écrit. Vide tant \
                qu'aucun export n'a été validé — le grand livre de l'écran Comptabilité \
                est, lui, dérivé des documents.",
        sql: "SELECT numero_compte, COALESCE(label_compte, ''), \
                     COALESCE(label_operation, ''), DATE_FORMAT(doc_date, '%Y-%m-%d'), \
                     CAST(debit AS CHAR), CAST(credit AS CHAR) \
              FROM llx_accounting_bookkeeping WHERE entity = :entity \
              ORDER BY doc_date DESC, rowid DESC LIMIT 200",
        colonnes: &[
            "Compte",
            "Libellé compte",
            "Opération",
            "Date",
            "Débit",
            "Crédit",
        ],
    },
    ListeSimple {
        code: "Asset",
        titre: "Immobilisations",
        intro: "Biens immobilisés (llx_asset), lecture seule.",
        sql: "SELECT a.rowid, a.ref, a.label, DATE_FORMAT(a.date_acquisition, '%Y-%m-%d'), \
                     CAST(a.acquisition_value_ht AS CHAR), \
                     COALESCE(DATE_FORMAT(a.disposal_date, '%Y-%m-%d'), '') \
              FROM llx_asset a WHERE a.entity = :entity \
              ORDER BY a.date_acquisition DESC, a.rowid DESC LIMIT 200",
        colonnes: &["N°", "Réf.", "Libellé", "Acquis le", "Valeur HT", "Cédé le"],
    },
    ListeSimple {
        code: "Resource",
        titre: "Ressources",
        intro: "Ressources partagées : salles, véhicules, matériel (llx_resource).",
        sql: "SELECT rowid, ref, COALESCE(description, ''), COALESCE(town, ''), \
                     COALESCE(phone, ''), COALESCE(email, ''), \
                     CASE fk_statut WHEN 0 THEN 'inactif' ELSE 'actif' END \
              FROM llx_resource WHERE entity = :entity \
              ORDER BY ref ASC LIMIT 200",
        colonnes: &[
            "N°",
            "Réf.",
            "Description",
            "Ville",
            "Téléphone",
            "E-mail",
            "État",
        ],
    },
    ListeSimple {
        code: "PaymentByBankTransfer",
        titre: "Virements",
        intro: "Règlements par virement (llx_paiement, moyen de paiement VIR) : c'est \
                la matière du bordereau de virement.",
        sql: "SELECT p.ref, DATE_FORMAT(p.datep, '%Y-%m-%d'), \
                     CAST(p.amount AS CHAR), COALESCE(ba.label, ''), COALESCE(p.num_paiement, '') \
              FROM llx_paiement p \
              JOIN llx_c_paiement cp ON cp.id = p.fk_paiement \
              LEFT JOIN llx_bank b ON b.rowid = p.fk_bank \
              LEFT JOIN llx_bank_account ba ON ba.rowid = b.fk_account \
              WHERE p.entity = :entity AND cp.code = 'VIR' \
              ORDER BY p.datep DESC, p.rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Date", "Montant", "Compte bancaire", "N° pièce"],
    },
];

/// Listes ajoutees par les modules declaratifs.
///
/// Un `RwLock` et non un `OnceLock` : chaque module ajoute les siennes, et le
/// premier arrive ne doit pas condamner les suivants. Chaque liste est fuitee
/// a l'enregistrement, donc les references rendues sont vraiment `'static`.
static LISTES_DECLAREES: std::sync::RwLock<Vec<&'static ListeSimple>> =
    std::sync::RwLock::new(Vec::new());

/// Ajoute des listes venues d'un module declaratif.
pub fn enregistrer_listes(listes: Vec<ListeSimple>) {
    let mut v = LISTES_DECLAREES.write().unwrap_or_else(|e| e.into_inner());
    for l in listes {
        if v.iter().any(|x| x.code.eq_ignore_ascii_case(l.code)) {
            continue;
        }
        v.push(Box::leak(Box::new(l)));
    }
}

/// Toutes les listes : celles du noyau, puis celles des modules.
pub fn liste_simple(code: &str) -> Option<&'static ListeSimple> {
    if let Some(l) = LISTES.iter().find(|l| l.code.eq_ignore_ascii_case(code)) {
        return Some(l);
    }
    LISTES_DECLAREES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|l| l.code.eq_ignore_ascii_case(code))
        .copied()
}

/// Affiche une liste en lecture seule, avec un lien de creation quand le
/// module en a un.
pub async fn liste_simple_page(
    State(app): State<crate::App>,
    axum::extract::Path(code): axum::extract::Path<String>,
) -> Res<Markup> {
    let liste = liste_simple(&code).ok_or_else(|| Erreur(format!("liste inconnue : {code}")))?;
    let lignes = db::divers::lignes_brutes(&app.pool, crate::entite::courante(), liste.sql)?;
    let creation = saisie(&code).is_some();

    let valeur = |r: &Vec<Option<String>>, i: usize| -> String {
        r.get(i).cloned().flatten().unwrap_or_default()
    };

    Ok(crate::ui::page(
        "listes",
        "liste",
        liste.titre,
        html! {
            h1 { (liste.titre) }
            p .sous-titre { (liste.intro) }
            @if creation {
                div .actions {
                    a .bouton href={ "/liste/" (code) "/nouveau" } { "Nouveau" }
                }
            }
            @if lignes.is_empty() {
                (crate::ui::vide("Aucune ligne.", None))
            } @else {
                table {
                    thead {
                        tr {
                            @for c in liste.colonnes { th { (c) } }
                        }
                    }
                    tbody {
                        @for r in &lignes {
                            tr {
                                @for (i, _) in liste.colonnes.iter().enumerate() {
                                    td {
                                        @if i == 0 && creation {
                                            a href={ "/liste/" (code) "/fiche/" (valeur(r, 0)) } {
                                                (valeur(r, 0))
                                            }
                                        } @else {
                                            (valeur(r, i))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            p style="color:#78736c;font-size:13px" {
                @if creation {
                    "Écran de création et de lecture. Le reste des champs se saisit dans Dolibarr."
                } @else {
                    "Lecture seule : les écritures se font dans Dolibarr."
                }
            }
        },
    ))
}

// ---------------------------------------------------------------------------
// Saisie simple : un formulaire de creation pour les modules que le noyau ne
// couvre pas encore en ecriture. Meme code que la liste : un module n'a qu'un
// identifiant.
// ---------------------------------------------------------------------------

/// Genre de champ, qui commande le controle de saisie du navigateur.
#[derive(Clone, Copy, PartialEq)]
pub enum Sorte {
    Texte,
    Nombre,
    Date,
    Entier,
}

impl Sorte {
    fn type_html(self) -> &'static str {
        match self {
            Sorte::Texte => "text",
            Sorte::Nombre => "text",
            Sorte::Date => "date",
            Sorte::Entier => "number",
        }
    }

    /// Normalise une valeur saisie : les nombres se tapent en francais
    /// (espace insecable et virgule), MySQL attend un point.
    pub fn normaliser(self, v: &str) -> Option<String> {
        let v = v.trim();
        if v.is_empty() {
            return None;
        }
        match self {
            Sorte::Nombre | Sorte::Entier => Some(
                v.chars()
                    .filter(|c| !c.is_whitespace() && *c != '\u{a0}')
                    .collect::<String>()
                    .replace(',', "."),
            ),
            _ => Some(v.to_string()),
        }
    }
}

/// Un champ du formulaire de creation.
pub struct ChampSaisie {
    /// Nom de la colonne. Il vient d'une table fermee, jamais de la requete.
    pub nom: &'static str,
    pub libelle: &'static str,
    pub sorte: Sorte,
    pub obligatoire: bool,
    /// Source d'une liste deroulante (`produits`, `types_adherent`...).
    pub choix: Option<&'static str>,
    /// Valeur proposee a la creation.
    pub defaut: &'static str,
}

/// Un formulaire de creation.
pub struct Saisie {
    pub code: &'static str,
    pub table: &'static str,
    pub champs: &'static [ChampSaisie],
    /// Colonnes posees d'office : (`colonne`, expression SQL). `{user}` est
    /// remplace par l'identifiant de l'utilisateur connecte.
    pub fixes: &'static [(&'static str, &'static str)],
    /// Reference a numeroter : (`colonne`, prefixe).
    pub reference: Option<(&'static str, &'static str)>,
    /// Cle primaire : `rowid` presque partout, `id` dans les dictionnaires.
    pub pk: &'static str,
    /// La table porte-t-elle une colonne `entity` ? Faux pour un dictionnaire
    /// national (pays, types d'evenement).
    pub entite: bool,
    /// La cle est-elle auto-incrementee ? Faux pour `llx_c_country`, seme avec
    /// des identifiants fixes par l'installateur Dolibarr.
    pub pk_auto: bool,
}

const fn c(
    nom: &'static str,
    libelle: &'static str,
    sorte: Sorte,
    obligatoire: bool,
) -> ChampSaisie {
    ChampSaisie {
        nom,
        libelle,
        sorte,
        obligatoire,
        choix: None,
        defaut: "",
    }
}

/// Un champ alimente par une liste deroulante.
const fn cc(nom: &'static str, libelle: &'static str, source: &'static str) -> ChampSaisie {
    ChampSaisie {
        nom,
        libelle,
        sorte: Sorte::Entier,
        obligatoire: true,
        choix: Some(source),
        defaut: "",
    }
}

pub const SAISIES: &[Saisie] = &[
    Saisie {
        code: "Loan",
        table: "llx_loan",
        champs: &[
            c("label", "Libellé", Sorte::Texte, true),
            c("capital", "Capital emprunté", Sorte::Nombre, true),
            c("rate", "Taux (%)", Sorte::Nombre, true),
            c("datestart", "Première échéance", Sorte::Date, false),
            c("dateend", "Dernière échéance", Sorte::Date, false),
            c("nbterm", "Nombre d'échéances", Sorte::Entier, false),
        ],
        fixes: &[("datec", "NOW()"), ("paid", "0"), ("active", "1")],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Asset",
        table: "llx_asset",
        champs: &[
            c("ref", "Référence", Sorte::Texte, true),
            c("label", "Libellé", Sorte::Texte, true),
            c("date_acquisition", "Date d'acquisition", Sorte::Date, true),
            c("date_start", "Début d'amortissement", Sorte::Date, true),
            c("acquisition_value_ht", "Valeur HT", Sorte::Nombre, false),
            c("qty", "Quantité", Sorte::Nombre, false),
        ],
        fixes: &[
            ("date_creation", "NOW()"),
            ("fk_user_creat", "{user}"),
            ("status", "0"),
        ],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "ProductBatch",
        table: "llx_product_lot",
        champs: &[
            cc("fk_product", "Produit", "produits"),
            c("batch", "Numéro de lot", Sorte::Texte, true),
            c("manufacturing_date", "Fabrication", Sorte::Date, false),
            c("eatby", "À consommer avant", Sorte::Date, false),
            c("sellby", "À vendre avant", Sorte::Date, false),
        ],
        fixes: &[],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Resource",
        table: "llx_resource",
        champs: &[
            c("ref", "Référence", Sorte::Texte, true),
            c("description", "Description", Sorte::Texte, false),
            c("town", "Ville", Sorte::Texte, false),
            c("phone", "Téléphone", Sorte::Texte, false),
            c("email", "E-mail", Sorte::Texte, false),
        ],
        fixes: &[("datec", "NOW()"), ("fk_statut", "1")],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Tax",
        table: "llx_c_tva",
        champs: &[
            c("code", "Code (ex. TVA20)", Sorte::Texte, true),
            c("taux", "Taux (%)", Sorte::Nombre, true),
            c("note", "Note", Sorte::Texte, false),
        ],
        // France : c'est le pays de la base. Un taux etranger se saisit dans
        // Dolibarr, avec son pays.
        fixes: &[("fk_pays", "1"), ("active", "1")],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Adherent",
        table: "llx_adherent",
        champs: &[
            cc("fk_adherent_type", "Type d'adhérent", "types_adherent"),
            c("lastname", "Nom", Sorte::Texte, true),
            c("firstname", "Prénom", Sorte::Texte, false),
            c("town", "Ville", Sorte::Texte, false),
            c("email", "E-mail", Sorte::Texte, false),
            c("datefin", "Adhésion jusqu'au", Sorte::Date, false),
        ],
        fixes: &[("morphy", "'phy'"), ("statut", "1"), ("public", "0")],
        reference: Some(("ref", "ADH")),
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Categorie",
        table: "llx_categorie",
        champs: &[
            c("label", "Libellé", Sorte::Texte, true),
            cc("type", "Type", "types_categorie"),
        ],
        fixes: &[("visible", "1"), ("fk_parent", "0")],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Don",
        table: "llx_don",
        champs: &[
            c("amount", "Montant", Sorte::Nombre, true),
            c("datedon", "Date du don", Sorte::Date, false),
            cc("fk_soc", "Donateur", "tiers"),
        ],
        fixes: &[
            ("fk_country", "1"),
            ("fk_user_author", "{user}"),
            ("fk_statut", "1"),
        ],
        reference: Some(("ref", "DON")),
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
];

/// Formulaires ajoutes par les modules declaratifs.
static SAISIES_DECLAREES: std::sync::RwLock<Vec<&'static Saisie>> =
    std::sync::RwLock::new(Vec::new());

/// Ajoute des formulaires venus d'un module declaratif.
pub fn enregistrer_saisies(saisies: Vec<Saisie>) {
    let mut v = SAISIES_DECLAREES.write().unwrap_or_else(|e| e.into_inner());
    for s in saisies {
        if v.iter().any(|x| x.code.eq_ignore_ascii_case(s.code)) {
            continue;
        }
        v.push(Box::leak(Box::new(s)));
    }
}

pub fn saisie(code: &str) -> Option<&'static Saisie> {
    if let Some(s) = SAISIES.iter().find(|s| s.code.eq_ignore_ascii_case(code)) {
        return Some(s);
    }
    SAISIES_DECLAREES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|s| s.code.eq_ignore_ascii_case(code))
        .copied()
}

/// Formulaire de creation.
pub async fn liste_saisie_form(
    State(app): State<crate::App>,
    axum::extract::Path(code): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Res<Markup> {
    let s = saisie(&code)
        .ok_or_else(|| Erreur(format!("aucun formulaire de création pour : {code}")))?;
    let liste = liste_simple(&code).ok_or_else(|| Erreur(format!("liste inconnue : {code}")))?;
    // Le refus d'une saisie incomplete revient ici, indexe sur le champ, pour
    // que le message soit lisible au lieu d'une page d'erreur.
    let manquant = q
        .get("erreur")
        .and_then(|i| i.parse::<usize>().ok())
        .and_then(|i| s.champs.get(i))
        .map(|ch| format!("« {} » est obligatoire.", ch.libelle));

    // Les listes deroulantes sont chargees une fois par source.
    let mut cache: HashMap<&'static str, Vec<(String, String)>> = HashMap::new();
    let options: Vec<Option<Vec<(String, String)>>> = {
        let mut v = Vec::new();
        for ch in s.champs {
            match ch.choix {
                Some(src) => {
                    if !cache.contains_key(src) {
                        let liste =
                            db::divers::options_liste(&app.pool, crate::entite::courante(), src)?;
                        cache.insert(src, liste);
                    }
                    v.push(cache.get(src).cloned());
                }
                None => v.push(None),
            }
        }
        v
    };

    Ok(crate::ui::page(
        "listes",
        "liste",
        liste.titre,
        html! {
            h1 { "Nouveau — " (liste.titre) }
            @if let Some(m) = &manquant {
                p style="background:#fdf1ef;border-left:3px solid #a33;padding:10px 12px;margin:0 0 16px" {
                    (m)
                }
            }
            p .sous-titre {
                "Création dans "
                code { (s.table) }
                " : les valeurs sont contrôlées par la base, une contrainte manquante remonte ici au lieu d'être inventée."
            }
            form method="post" action={ "/liste/" (code) "/nouveau" } {
                table {
                    tbody {
                        @for (i, ch) in s.champs.iter().enumerate() {
                            tr {
                                th {
                                    (ch.libelle)
                                    @if ch.obligatoire { span style="color:#a33" { " *" } }
                                }
                                td {
                                    @if let Some(opts) = options.get(i).cloned().flatten() {
                                        select name=(ch.nom) {
                                            @for (valeur, libelle) in &opts {
                                                option value=(valeur) { (libelle) }
                                            }
                                        }
                                    } @else {
                                        input
                                            type=(ch.sorte.type_html())
                                            name=(ch.nom)
                                            value=(ch.defaut)
                                            required[ch.obligatoire];
                                        @if ch.sorte == Sorte::Nombre {
                                            p style="color:#78736c;font-size:12px;margin:6px 0 0" {
                                                "Montant en euros, virgule ou point."
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" { "Créer" }
                    a .bouton href={ "/liste/" (code) } style="margin-left:8px" { "Annuler" }
                }
            }
        },
    ))
}

/// Enregistre la creation.
pub async fn liste_saisie_creer(
    State(app): State<crate::App>,
    axum::extract::Path(code): axum::extract::Path<String>,
    axum::Extension(courant): axum::Extension<crate::Courant>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let s = saisie(&code)
        .ok_or_else(|| Erreur(format!("aucun formulaire de création pour : {code}")))?;

    // Reference numerotee quand la table en exige une et que personne ne l'a
    // saisie (adherents, dons).
    let mut references: HashMap<&'static str, String> = HashMap::new();
    if let Some((colonne, prefixe)) = s.reference {
        references.insert(
            colonne,
            db::divers::suivant_ref(
                &app.pool,
                s.table,
                colonne,
                prefixe,
                crate::entite::courante(),
            )?,
        );
    }

    let mut valeurs: Vec<(String, Option<String>)> = Vec::new();
    for (i, ch) in s.champs.iter().enumerate() {
        let mut brut = f.get(ch.nom).map(String::as_str).unwrap_or("");
        if brut.trim().is_empty() {
            if let Some(r) = references.get(ch.nom) {
                brut = r;
            } else if !ch.defaut.is_empty() {
                brut = ch.defaut;
            }
        }
        let v = ch.sorte.normaliser(brut);
        if ch.obligatoire && v.is_none() {
            return Ok(Redirect::to(&format!("/liste/{code}/nouveau?erreur={i}")));
        }
        valeurs.push((ch.nom.to_string(), v));
    }

    // Une reference numerotee n'est pas forcement un champ du formulaire :
    // elle doit quand meme entrer en base (adherents, dons).
    for (colonne, valeur) in &references {
        if !valeurs.iter().any(|(c, _)| c == colonne) {
            valeurs.push((colonne.to_string(), Some(valeur.clone())));
        }
    }

    // Les colonnes posees d'office : soit un utilisateur, soit une expression
    // SQL fermee (constante du registre, jamais une saisie).
    let mut exprs: Vec<(&'static str, &'static str)> = Vec::new();
    let mut user: Option<i64> = None;
    for (col, expr) in s.fixes {
        if *expr == "{user}" {
            if user.is_none() {
                user = db::user::par_login(&app.pool, &courant.login)?.map(|u| u.rowid);
            }
            valeurs.push((col.to_string(), user.map(|u| u.to_string())));
        } else {
            exprs.push((col, expr));
        }
    }

    db::divers::inserer(
        &app.pool,
        cible(s),
        crate::entite::courante(),
        &valeurs,
        &exprs,
    )?;
    Ok(Redirect::to(&format!("/liste/{code}?ok=cree")))
}

/// La cible d'ecriture d'un formulaire declare.
fn cible(s: &Saisie) -> db::divers::Cible<'_> {
    db::divers::Cible {
        table: s.table,
        cle: s.pk,
        entite: s.entite,
        cle_auto: s.pk_auto,
    }
}

/// Met une valeur de la base sous une forme lisible dans un champ.
///
/// Les colonnes `double(24,8)` rendent « 3.50000000 » ; l'utilisateur doit voir
/// « 3,5 ». Les nombres passent par le decimal, jamais par un flottant.
fn valeur_affichable(ch: &ChampSaisie, v: &Option<String>) -> String {
    let brut = v.clone().unwrap_or_default();
    match ch.sorte {
        Sorte::Nombre | Sorte::Entier => {
            use std::str::FromStr;
            rust_decimal::Decimal::from_str(&brut)
                .map(|d| d.normalize().to_string())
                .unwrap_or(brut)
        }
        _ => brut,
    }
}

/// Fiche d'une ligne : les memes champs que la creation, remplis.
pub async fn fiche_saisie(
    State(app): State<crate::App>,
    axum::extract::Path((code, id)): axum::extract::Path<(String, i64)>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Res<Markup> {
    let s = saisie(&code).ok_or_else(|| Erreur(format!("aucune fiche pour : {code}")))?;
    let liste = liste_simple(&code).ok_or_else(|| Erreur(format!("liste inconnue : {code}")))?;

    let colonnes: Vec<&str> = s.champs.iter().map(|ch| ch.nom).collect();
    let ligne = db::divers::lire_ligne_par(
        &app.pool,
        cible(s),
        crate::entite::courante(),
        id,
        &colonnes,
    )?
    .ok_or_else(|| Erreur(format!("ligne {id} introuvable dans {}", s.table)))?;
    let valeurs: Vec<String> = s
        .champs
        .iter()
        .enumerate()
        .map(|(i, ch)| valeur_affichable(ch, ligne.get(i).unwrap_or(&None)))
        .collect();

    let manquant = q
        .get("erreur")
        .and_then(|i| i.parse::<usize>().ok())
        .and_then(|i| s.champs.get(i))
        .map(|ch| format!("« {} » est obligatoire.", ch.libelle));

    Ok(crate::ui::page(
        "listes",
        "liste",
        liste.titre,
        html! {
            h1 { (liste.titre) " — n° " (id.to_string()) }
            @if let Some(m) = &manquant {
                p style="background:#fdf1ef;border-left:3px solid #a33;padding:10px 12px;margin:0 0 16px" {
                    (m)
                }
            }
            form method="post" action={ "/liste/" (code) "/fiche/" (id.to_string()) } {
                table {
                    tbody {
                        @for (i, ch) in s.champs.iter().enumerate() {
                            tr {
                                th {
                                    (ch.libelle)
                                    @if ch.obligatoire { span style="color:#a33" { " *" } }
                                }
                                td {
                                    input
                                        type=(ch.sorte.type_html())
                                        name=(ch.nom)
                                        value=(valeurs.get(i).cloned().unwrap_or_default())
                                        required[ch.obligatoire];
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" { "Enregistrer" }
                    a .bouton href={ "/liste/" (code) } style="margin-left:8px" { "Retour" }
                }
            }
            form method="post"
                 action={ "/liste/" (code) "/fiche/" (id.to_string()) "/supprimer" }
                 onsubmit="return confirm('Supprimer définitivement cette ligne ?')"
                 style="margin-top:18px" {
                button type="submit"
                       style="background:#a33;color:#fff;border:0;padding:9px 16px;cursor:pointer" {
                    "Supprimer"
                }
                span style="color:#78736c;font-size:13px;margin-left:10px" {
                    "Suppression directe dans " code { (s.table) } " : Dolibarr ne passera pas par là."
                }
            }
        },
    ))
}

/// Enregistre la modification d'une fiche.
pub async fn fiche_saisie_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path((code, id)): axum::extract::Path<(String, i64)>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let s = saisie(&code).ok_or_else(|| Erreur(format!("aucune fiche pour : {code}")))?;

    let mut valeurs: Vec<(String, Option<String>)> = Vec::new();
    for (i, ch) in s.champs.iter().enumerate() {
        let brut = f.get(ch.nom).map(String::as_str).unwrap_or("");
        let v = ch.sorte.normaliser(brut);
        if ch.obligatoire && v.is_none() {
            return Ok(Redirect::to(&format!(
                "/liste/{code}/fiche/{id}?erreur={i}"
            )));
        }
        valeurs.push((ch.nom.to_string(), v));
    }

    // Les colonnes posees d'office ne sont pas retouchees : une date de
    // creation ne se reecrit pas.
    db::divers::mettre_a_jour_par(
        &app.pool,
        cible(s),
        crate::entite::courante(),
        id,
        &valeurs,
        &[],
    )?;
    Ok(Redirect::to(&format!("/liste/{code}?ok=modifie")))
}

/// Supprime une fiche.
pub async fn fiche_saisie_supprimer(
    State(app): State<crate::App>,
    axum::extract::Path((code, id)): axum::extract::Path<(String, i64)>,
) -> Res<Redirect> {
    let s = saisie(&code).ok_or_else(|| Erreur(format!("aucune fiche pour : {code}")))?;
    db::divers::supprimer_ligne_par(&app.pool, cible(s), crate::entite::courante(), id)?;
    Ok(Redirect::to(&format!("/liste/{code}?ok=supprime")))
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
            h1 { "Tables de la base" }
            p .sous-titre {
                (tables.len().to_string()) " tables, " (total.to_string())
                " lignes (comptage approximatif de MariaDB). Lecture seule : "
                "les ecritures passent par les ecrans metier."
            }
            table {
                thead {
                    tr { th { "Table" } th .num { "Lignes" } th { "Commentaire" } }
                }
                tbody {
                    @for t in &tables {
                        tr {
                            td { a href={ "/parametres/tables/" (t.nom) } { (t.nom) } }
                            td .num { (t.lignes.to_string()) }
                            td style="color:#78736c" { (t.commentaire.clone()) }
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
                a .bouton.secondaire href={ "/parametres/tables/" (nom) "?page=" ((page + 1).to_string()) } {
                    "Page suivante"
                }
                a .bouton.secondaire href="/parametres/tables" style="margin-left:8px" {
                    "Toutes les tables"
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
            h1 { "Greffons" }
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
                    thead { tr { th { "Greffon" } th { "Fichier" } } }
                    tbody {
                        @for g in &liste {
                            tr {
                                td { (g.nom.clone()) }
                                td style="color:#78736c" { (g.chemin.display().to_string()) }
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

    Ok(crate::ui::page(
        "accueil",
        "installation",
        "Installation",
        html! {
            h1 { "Installation" }
            p .sous-titre {
                "État du noyau sur cette base. Rien n'est modifié par cette page : "
                "elle constate, elle ne répare pas."
            }
            @if etat.pret() {
                (crate::ui::flash("Installation complète : le noyau peut servir."))
            } @else {
                p .erreur { "Installation incomplète : voir les points ci-dessous." }
            }
            table {
                thead { tr { th { "Point" } th { "État" } th .num { "Valeur" } } }
                tbody {
                    tr {
                        td { "Schéma Dolibarr" }
                        td {
                            @if etat.tables >= etat.tables_attendues { "complet" } @else { "incomplet" }
                        }
                        td .num {
                            (etat.tables.to_string()) " / " (etat.tables_attendues.to_string())
                        }
                    }
                    tr {
                        td { "Administrateurs" }
                        td { @if etat.admins > 0 { "présent" } @else { "aucun" } }
                        td .num { (etat.admins.to_string()) }
                    }
                    tr {
                        td { "Utilisateurs" }
                        td { "" }
                        td .num { (etat.utilisateurs.to_string()) }
                    }
                    tr {
                        td { "Modules semés" }
                        td { @if etat.modules_semes > 0 { "présents" } @else { "aucun" } }
                        td .num { (etat.modules_semes.to_string()) }
                    }
                    tr {
                        td { "Société" }
                        td {
                            @match &etat.societe {
                                Some(n) => (n.clone()),
                                None => span style="color:#a33" { "sans nom" },
                            }
                        }
                        td { "" }
                    }
                    tr {
                        td { "SMTP" }
                        td {
                            @match &etat.smtp {
                                Some(h) => (h.clone()),
                                None => span style="color:#a33" { "non configuré" },
                            }
                        }
                        td { "" }
                    }
                }
            }
            @if !etat.manques().is_empty() {
                h2 { "Ce qui manque" }
                ul {
                    @for m in etat.manques() { li { (m) } }
                }
            }
            @if etat.admins == 0 {
                h2 { "Créer l'administrateur" }
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
                                th { "Identifiant" }
                                td { input type="text" name="login" required value="admin" style="width:100%"; }
                            }
                            tr {
                                th { "Mot de passe" }
                                td { input type="password" name="mot_de_passe" required minlength="8" style="width:100%"; }
                            }
                            tr {
                                th { "E-mail" }
                                td { input type="email" name="email" style="width:100%"; }
                            }
                            tr {
                                th { "Nom de la société" }
                                td { input type="text" name="societe" style="width:100%"; }
                            }
                        }
                    }
                    div .actions { button type="submit" { "Installer" } }
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
