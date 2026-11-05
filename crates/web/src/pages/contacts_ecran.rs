//! Contacts, tickets, agenda et commandes fournisseurs.
//!
//! Les contacts appartiennent a un tiers, les tickets a un probleme, l'agenda a
//! une echeance. Ce sont les ecrans du quotidien : ils doivent repondre vite,
//! parce qu'on les ouvre au telephone.
use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use maud::{html, Markup};

use crate::{message, Erreur, Res};

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
            h1 {(crate::langue::tr("Contacts et adresses")) }
            p .sous-titre {(crate::langue::tr("Un contact est une personne ; le tiers est la société. Les deux sont séparés, comme dans Dolibarr."))
            }

            h2 {(crate::langue::tr("Contacts")) }
            @if liste.is_empty() {
                (crate::ui::vide("Aucun contact.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Nom")) } th {(crate::langue::tr("Société")) } th {(crate::langue::tr("Fonction")) } th {(crate::langue::tr("E-mail")) } th {(crate::langue::tr("Téléphone")) } th {(crate::langue::tr("Ville")) } } }
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

            h2 {(crate::langue::tr("Nouveau contact")) }
            form method="post" action="/contacts" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Société")) }
                            td {
                                select name="soc" style="width:100%" {
                                    option value="" {(crate::langue::tr("— aucun (particulier) —")) }
                                    @for s in &clients {
                                        option value=(s.rowid) { (s.nom) }
                                    }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Civilité")) }
                            td {
                                select name="civilite" {
                                    option value="" { "—" }
                                    option value="M." {(crate::langue::tr("M.")) }
                                    option value="Mme" {(crate::langue::tr("Mme")) }
                                    option value="Mlle" {(crate::langue::tr("Mlle")) }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Prénom")) } td { input type="text" name="prenom" style="width:100%"; } }
                        tr { th {(crate::langue::tr("Fonction")) } td { input type="text" name="poste" style="width:100%"; } }
                        tr { th {(crate::langue::tr("E-mail")) } td { input type="email" name="email" style="width:100%"; } }
                        tr { th {(crate::langue::tr("Téléphone")) } td { input type="text" name="telephone" style="width:100%"; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le contact")) } }
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
            h1 {(crate::langue::tr("Commandes fournisseurs")) }
            p .sous-titre {(crate::langue::tr("Lues dans llx_commande_fournisseur : le noyau les affiche, l'écriture viendra.")) }
            @if liste.is_empty() {
                (crate::ui::vide("Aucune commande fournisseur.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Ref.")) } th {(crate::langue::tr("Fournisseur")) } th {(crate::langue::tr("Chez le fournisseur")) } th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Statut")) } th .num {(crate::langue::tr("TTC")) } } }
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
            p .sous-titre {(crate::langue::tr("Écritures dérivées des documents, triées par date.")) }
            form method="get" action="/comptabilite/journal" .actions {
                input type="hidden" name="sens" value=(sens);
                input type="date" name="debut" value=(debut.clone().unwrap_or_default()) aria-label="Début";
                input type="date" name="fin" value=(fin.clone().unwrap_or_default()) aria-label="Fin";
                button type="submit" {(crate::langue::tr("Filtrer")) }
                a .bouton.secondaire href={ "/export/ventes.csv?debut=" (debut.clone().unwrap_or_default()) "&fin=" (fin.clone().unwrap_or_default()) } {(crate::langue::tr("CSV ventes")) }
                a .bouton.secondaire href={ "/export/achats.csv?debut=" (debut.clone().unwrap_or_default()) "&fin=" (fin.clone().unwrap_or_default()) } {(crate::langue::tr("CSV achats")) }
            }
            @if lignes.is_empty() {
                (crate::ui::vide("Aucune écriture sur la période.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Référence")) } th {(crate::langue::tr("Type")) } th {(crate::langue::tr("Tiers")) } th .num {(crate::langue::tr("HT")) } th .num {(crate::langue::tr("TVA")) } th .num {(crate::langue::tr("TTC")) } } }
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
                            td colspan="4" .num {(crate::langue::tr("Totaux")) }
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
            h1 {(crate::langue::tr("Tickets de support")) }
            p .sous-titre {(crate::langue::tr("Une réclamation client tracée : sujet, société, statut. Numérotation TK + AAMM + compteur."))
            }

            @if liste.is_empty() {
                (crate::ui::vide("Aucun ticket.", None))
            } @else {
                form id="actions-tickets" method="post" action="/tickets/actions" {}
                div .actions {
                    button type="submit" form="actions-tickets" name="action" value="fermer" {
                        (crate::langue::tr("Fermer la sélection"))
                    }
                    span style="color:#78736c;font-size:13.5px" {
                        (crate::langue::tr("Fermer n'est pas supprimer : le ticket garde son historique."))
                    }
                }
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Sujet")) } th {(crate::langue::tr("Société")) } th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Gravité")) } th { "État" } th { "" } } }
                    tbody {
                        @for t in &liste {
                            tr {
                                td .ref {
                                    @if t.statut != 8 {
                                        input type="checkbox" name={"id_" (t.rowid)} value="1"
                                              form="actions-tickets"
                                              aria-label=(crate::langue::tr("sélectionner ce ticket"));
                                        " "
                                    }
                                    (t.ref_)
                                }
                                td { (t.sujet) }
                                td { (t.tiers.clone().unwrap_or_default()) }
                                td .ref { (app::fmt_date_fr(t.date.as_deref())) }
                                td { (t.gravite.clone().unwrap_or_default()) }
                                td {
                                    @if t.statut == 8 { span .pastille.p-payee {(crate::langue::tr("fermé")) } }
                                    @else if t.statut == 0 { span .pastille.p-brouillon { "nouveau" } }
                                    @else { span .pastille.p-validee {(crate::langue::tr("en cours")) } }
                                }
                                td {
                                    @if t.statut != 8 {
                                        form method="post" action={ "/tickets/" (t.rowid) "/fermer" } {
                                            button type="submit" class="secondaire petit" {(crate::langue::tr("Fermer")) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouveau ticket")) }
            form method="post" action="/tickets" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Société")) }
                            td {
                                select name="soc" style="width:100%" {
                                    option value="" {(crate::langue::tr("— aucun —")) }
                                    @for c in &clients {
                                        option value=(c.rowid) { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Sujet")) } td { input type="text" name="sujet" required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Message")) } td { textarea name="message" rows="4" style="width:100%;font:inherit;padding:8px 10px;border:1px solid #e6e2dc" {} } }
                        tr {
                            th {(crate::langue::tr("Gravité")) }
                            td {
                                select name="gravite" {
                                    option value="LOW" {(crate::langue::tr("Basse")) }
                                    option value="NORMAL" {(crate::langue::tr("Normale")) }
                                    option value="HIGH" {(crate::langue::tr("Haute")) }
                                    option value="BLOCKING" {(crate::langue::tr("Bloquante")) }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le ticket")) } }
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
                @else {(crate::langue::tr("Sans société (particulier)")) }
            }
            form method="post" action={ "/contacts/" (id) "/modifier" } {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" value=(c.nom) required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Prénom")) } td { input type="text" name="prenom" value=(c.prenom.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Fonction")) } td { input type="text" name="poste" value=(c.poste.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("E-mail")) } td { input type="email" name="email" value=(c.email.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Téléphone")) } td { input type="text" name="telephone" value=(c.telephone.clone().unwrap_or_default()) style="width:100%"; } }
                        tr { th {(crate::langue::tr("Ville")) } td { input type="text" name="ville" value=(c.ville.clone().unwrap_or_default()) style="width:100%"; } }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton.secondaire href="/contacts" {(crate::langue::tr("Retour")) }
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
                    div .libelle {(crate::langue::tr("Message")) }
                    p style="white-space:pre-wrap" { (m) }
                }
            }
            form method="post" action={ "/tickets/" (id) "/statut" } .actions {
                select name="statut" {
                    option value="0" selected[t.statut == 0] {(crate::langue::tr("Nouveau")) }
                    option value="1" selected[t.statut == 1] {(crate::langue::tr("En cours")) }
                    option value="8" selected[t.statut == 8] {(crate::langue::tr("Fermé")) }
                }
                button type="submit" {(crate::langue::tr("Changer l'état")) }
                a .bouton.secondaire href="/tickets" {(crate::langue::tr("Retour")) }
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
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" style="width:100%" {
                                    option value="" {(crate::langue::tr("— chantier interne —")) }
                                    @for s in &clients {
                                        option value=(s.rowid) selected[c.fk_soc == Some(s.rowid)] { (s.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Intitulé")) } td { input type="text" name="title" value=(c.title) required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Début")) } td { input type="date" name="debut" value=(c.date_start.clone().unwrap_or_default()) style="width:170px"; } }
                        tr { th {(crate::langue::tr("Fin prévue")) } td { input type="date" name="fin" value=(c.date_end.clone().unwrap_or_default()) style="width:170px"; } }
                        tr { th {(crate::langue::tr("Budget HT")) } td { input type="number" name="budget" step="0.01" min="0" value=(app::fmt_dec(c.budget)) style="width:150px"; } }
                        tr { th {(crate::langue::tr("Description")) } td { input type="text" name="description" value=(c.description.clone().unwrap_or_default()) style="width:100%"; } }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton.secondaire href={ "/chantiers/" (id) } {(crate::langue::tr("Retour")) }
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
