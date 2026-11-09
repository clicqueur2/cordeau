//! Recrutement : les offres, et les gens qui y repondent.
//!
//! Dolibarr tient `llx_recruitment_recruitmentjobposition` et
//! `llx_recruitment_recruitmentcandidature`. Une offre se publie, se pourvoit ou
//! se clot ; une candidature avance par etapes, et une etape retiree par
//! Dolibarr (`ENTRETIEN_PLANIFIE_RETIRE`) est refusee par la base — l'ecran ne la
//! propose donc pas.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use axum::Form;
use db::recrutement::{self, NouvelleCandidature, NouvelleOffre, STATUT_CLOTUREE, STATUT_PUBLIEE};
use maud::html;

use crate::*;

pub(crate) async fn recrutement(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let offres = recrutement::offres(p, entity, 200)?;
    let gens = db::user::list(p, entity)?;

    Ok(ui::page(
        "recrutement",
        "liste",
        "Recrutement",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Recrutement")) }
            p .sous-titre {
                (crate::langue::tr("Une offre se publie, se pourvoit ou se clôt. Une candidature avance par étapes."))
            }

            h2 {(crate::langue::tr("Nouvelle offre")) }
            @if gens.is_empty() {
                (ui::vide("Aucun utilisateur pour porter le recrutement.", Some(("/utilisateurs", "Voir les utilisateurs"))))
            } @else {
                form method="post" action="/recrutement" {
                    table {
                        tbody {
                            tr { th {(crate::langue::tr("Intitulé")) } td { input type="text" name="label" required.pleine placeholder="Maçon qualifié"; } }
                            tr { th {(crate::langue::tr("Description")) } td { textarea name="description" rows="3".pleine.zone-texte {} } }
                            tr {
                                th {(crate::langue::tr("Recruteur")) }
                                td {
                                    select name="recruteur" required {
                                        @for g in &gens {
                                            option value=(g.rowid) { (g.nom_complet()) }
                                        }
                                    }
                                }
                            }
                            tr { th {(crate::langue::tr("Postes")) } td { input type="number" name="postes" value="1" min="1" .champ-court; } }
                            tr { th {(crate::langue::tr("Date prévue")) } td { input type="date" name="date" .champ-moyen; } }
                        }
                    }
                    div .actions { button type="submit" {(crate::langue::tr("Créer l'offre")) } }
                }
            }

            h2 {(crate::langue::tr("Offres")) }
            @if offres.is_empty() {
                (ui::vide("Aucune offre.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Réf.")) }
                            th {(crate::langue::tr("Intitulé")) }
                            th {(crate::langue::tr("Postes")) }
                            th {(crate::langue::tr("Recruteur")) }
                            th { "État" }
                            th { "" }
                        }
                    }
                    tbody {
                        @for o in &offres {
                            tr {
                                td .ref { a href={ "/recrutement/" (o.rowid) } { (o.ref_.clone()) } }
                                td { (o.label.clone()) }
                                td .num { (o.nb_postes) }
                                td { (o.recruteur.clone().unwrap_or_default()) }
                                td {
                                    @if o.statut == STATUT_PUBLIEE {
                                        span .pastille.p-validee {(crate::langue::tr("publiée")) }
                                    } @else if o.statut == STATUT_CLOTUREE {
                                        span .pastille.p-payee {(crate::langue::tr("close")) }
                                    } @else {
                                        span .pastille.p-brouillon { "brouillon" }
                                    }
                                }
                                td {
                                    @if o.statut == 0 {
                                        form method="post" action={ "/recrutement/" (o.rowid) "/publier" } {
                                            button type="submit" class="petit" {(crate::langue::tr("Publier")) }
                                        }
                                    } @else if o.statut != STATUT_CLOTUREE {
                                        form method="post" action={ "/recrutement/" (o.rowid) "/clore" } {
                                            button type="submit" class="petit secondaire" {(crate::langue::tr("Clore")) }
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

pub(crate) async fn recrutement_offre(State(app): State<App>, Path(id): Path<i64>) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let offre = recrutement::offres(p, entity, 500)?
        .into_iter()
        .find(|o| o.rowid == id)
        .ok_or_else(|| Erreur(format!("offre {id} introuvable")))?;
    let candidats = recrutement::candidatures(p, entity, id)?;
    let bilan = recrutement::bilan_postes(p, entity, id).ok();

    Ok(ui::page(
        "recrutement",
        "liste",
        &offre.ref_,
        html! {
            h1 { (offre.label.clone()) }
            p .sous-titre { (offre.ref_.clone()) " — " (recrutement::libelle_statut_offre(offre.statut)) }

            @if let Some(b) = &bilan {
                div .kpis {
                    div .kpi {
                        div .titre {(crate::langue::tr("Postes")) }
                        div .valeur { (b.prevus) }
                    }
                    div .kpi.bon {
                        div .titre {(crate::langue::tr("Pourvus")) }
                        div .valeur { (b.pourvus) }
                    }
                    div .kpi {
                        div .titre {(crate::langue::tr("Candidatures")) }
                        div .valeur { (candidats.len()) }
                    }
                }
            }

            h2 {(crate::langue::tr("Candidatures")) }
            @if candidats.is_empty() {
                (ui::vide("Aucune candidature.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Nom")) }
                            th {(crate::langue::tr("Email")) }
                            th {(crate::langue::tr("Origine")) }
                            th { "État" }
                            th { "" }
                        }
                    }
                    tbody {
                        @for c in &candidats {
                            tr {
                                td { (c.lastname.clone().unwrap_or_default()) " " (c.firstname.clone().unwrap_or_default()) }
                                td .ref { (c.email.clone().unwrap_or_default()) }
                                td { (c.origine.clone().unwrap_or_default()) }
                                td { (recrutement::libelle_statut_candidature(c.statut)) }
                                td {
                                    form method="post" action={ "/recrutement/candidature/" (c.rowid) "/statut" } {
                                        select name="statut" .petite {
                                            @for (v, l) in STATUTS_CANDIDATURE {
                                                option value=(v) selected[c.statut == *v] { (l) }
                                            }
                                        }
                                        " "
                                        button type="submit" class="petit secondaire" {(crate::langue::tr("Changer")) }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Ajouter une candidature")) }
            form method="post" action={ "/recrutement/" (id) "/candidature" } {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" required .pleine; } }
                        tr { th {(crate::langue::tr("Prénom")) } td { input type="text" name="prenom" .pleine; } }
                        tr { th {(crate::langue::tr("Email")) } td { input type="email" name="email" .pleine; } }
                        tr { th {(crate::langue::tr("Téléphone")) } td { input type="text" name="tel" .pleine; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Ajouter")) } }
            }
        },
    ))
}

/// Les etapes d'une candidature, telles que la base les accepte.
pub(crate) const STATUTS_CANDIDATURE: &[(i16, &str)] = &[
    (0, "brouillon"),
    (1, "reçue"),
    (2, "présélection"),
    (3, "entretien"),
    (4, "retenue"),
    (9, "refusée"),
];

pub(crate) async fn recrutement_offre_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let recruteur: i64 = champ("recruteur")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Erreur("recruteur obligatoire".into()))?;
    // `valider_offre` refuse une reference vide : on la fabrique comme le fait
    // Dolibarr, `REC` + annee + compteur.
    let reference = db::divers::suivant_ref(
        &app.pool,
        "llx_recruitment_recruitmentjobposition",
        "ref",
        "REC",
        entite::courante(),
    )?;
    let offre = NouvelleOffre {
        ref_: reference,
        label: champ("label").ok_or_else(|| Erreur("intitulé obligatoire".into()))?,
        description: champ("description").unwrap_or_default(),
        fk_user_recruiter: recruteur,
        date_planned: champ("date"),
        nb_recruitment_planned: f
            .get("postes")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(1),
        fk_country: None,
        fk_soc: None,
    };
    let id = recrutement::creer_offre(&app.pool, entite::courante(), &offre)?;
    Ok(Redirect::to(&format!("/recrutement/{id}?ok=offre")))
}

pub(crate) async fn recrutement_publier(
    State(app): State<App>,
    Path(id): Path<i64>,
) -> Res<Redirect> {
    recrutement::publier_offre(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/recrutement/{id}?ok=publiee")))
}

pub(crate) async fn recrutement_clore(
    State(app): State<App>,
    Path(id): Path<i64>,
) -> Res<Redirect> {
    recrutement::clore_offre(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/recrutement/{id}?ok=close")))
}

pub(crate) async fn recrutement_candidature(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let candidature = NouvelleCandidature {
        fk_offre: id,
        lastname: champ("nom").ok_or_else(|| Erreur("nom obligatoire".into()))?,
        firstname: champ("prenom"),
        email: champ("email"),
        phone: champ("tel"),
        fk_origin: None,
        date_birth: None,
        note: None,
    };
    recrutement::creer_candidature(&app.pool, entite::courante(), &candidature)?;
    Ok(Redirect::to(&format!("/recrutement/{id}?ok=candidature")))
}

pub(crate) async fn recrutement_statut_candidature(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let statut: i16 = f
        .get("statut")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| Erreur("statut invalide".into()))?;
    recrutement::changer_statut_candidature(&app.pool, entite::courante(), id, statut)?;
    Ok(Redirect::to("/recrutement?ok=statut"))
}
