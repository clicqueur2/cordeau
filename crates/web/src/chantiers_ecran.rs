//! Les chantiers : projets, taches, et ce qu'ils coutent.
//!
//! Un chantier est un projet Dolibarr. Il porte les devis, les commandes et les
//! factures qui s'y rattachent ; c'est ce qui permet de dire si l'affaire a
//! tenu ses marges, ce qu'aucun total general ne montre.

use crate::*;

pub(crate) async fn chantiers(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let liste = db::projet::list(p, entite::courante(), 500, 0)?;
    let clients = db::party::list(p, entite::courante(), 500, 0)?;

    Ok(ui::page(
        "chantiers",
        "liste",
        "Chantiers",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Chantiers")) }
            p .sous-titre {(crate::langue::tr("Un chantier porte son budget et ses tâches : c'est le projet Dolibarr (llx_projet)."))
            }

            @if liste.is_empty() {
                (ui::vide("Aucun chantier.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Chantier")) } th {(crate::langue::tr("Client")) } th {(crate::langue::tr("Début")) } th {(crate::langue::tr("Fin")) } th .num {(crate::langue::tr("Budget")) } th { "État" } } }
                    tbody {
                        @for c in &liste {
                            tr {
                                td .ref { a href={ "/chantiers/" (c.rowid) } { (c.ref_) } }
                                td { a href={ "/chantiers/" (c.rowid) } { (c.title) } }
                                td { (nom_tiers(p, c.fk_soc)?) }
                                td .ref { (c.date_start.clone().unwrap_or_default()) }
                                td .ref { (c.date_end.clone().unwrap_or_default()) }
                                td .num { (ui::euros(c.budget.unwrap_or_default())) }
                                td {
                                    @if c.est_ouvert() {
                                        span .pastille.p-validee { "ouvert" }
                                    } @else {
                                        span .pastille.p-payee { "clos" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouveau chantier")) }
            form method="post" action="/chantiers" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" style="width:100%" {
                                    option value="" {(crate::langue::tr("— chantier interne —")) }
                                    @for c in &clients {
                                        option value=(c.rowid) { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Intitulé")) } td { input type="text" name="title" required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Début")) } td { input type="date" name="debut" style="width:170px"; } }
                        tr { th {(crate::langue::tr("Fin prévue")) } td { input type="date" name="fin" style="width:170px"; } }
                        tr { th {(crate::langue::tr("Budget HT")) } td { input type="number" name="budget" step="0.01" min="0" style="width:150px"; } }
                        tr { th {(crate::langue::tr("Description")) } td { input type="text" name="description" style="width:100%"; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le chantier")) } }
            }
        },
    ))
}

pub(crate) async fn chantier_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let titre = champ("title").ok_or_else(|| Erreur("intitulé du chantier obligatoire".into()))?;
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
    let id = db::projet::create(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to(&format!("/chantiers/{id}?ok=cree")))
}

pub(crate) async fn chantier_fiche(
    State(app): State<App>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let c =
        db::projet::get(p, entite::courante(), id)?.ok_or(Erreur("Chantier introuvable".into()))?;
    let taches = db::projet::taches(p, entite::courante(), id)?;

    Ok(ui::page(
        "chantiers",
        "liste",
        &c.ref_,
        html! {
            (message(q.get("ok")))
            h1 { (c.title) " " (if c.est_ouvert() { ui::pastille(1, 0, false) } else { ui::pastille(2, 1, false) }) }
            p .sous-titre {
                (c.ref_) " — client : " strong { (nom_tiers(p, c.fk_soc)?) }
                @if let Some(d) = &c.date_start { " — du " (d) }
                @if let Some(d) = &c.date_end { " au " (d) }
            }

            div .cartes {
                (ui::carte("Budget HT", &ui::euros(c.budget.unwrap_or_default()), None, false))
                (ui::carte("Tâches", &taches.len().to_string(), None, false))
            }

            @if let Some(d) = &c.description {
                p { (d) }
            }

            h2 {(crate::langue::tr("Tâches")) }
            @if taches.is_empty() {
                (ui::vide("Aucune tâche.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Tâche")) } th {(crate::langue::tr("Début")) } th {(crate::langue::tr("Fin")) } th .num {(crate::langue::tr("Avancement")) } } }
                    tbody {
                        @for t in &taches {
                            tr {
                                td .ref { (t.ref_.clone().unwrap_or_default()) }
                                td { (t.label) }
                                td .ref { (t.date_start.clone().unwrap_or_default()) }
                                td .ref { (t.date_end.clone().unwrap_or_default()) }
                                td .num { (t.progress) " %" }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouvelle tâche")) }
            form method="post" action={ "/chantiers/" (id) "/tache" } {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Libellé")) } td { input type="text" name="label" required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Début")) } td { input type="date" name="debut" style="width:170px"; } }
                        tr { th {(crate::langue::tr("Fin")) } td { input type="date" name="fin" style="width:170px"; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Ajouter la tâche")) } }
            }
        },
    ))
}

pub(crate) async fn chantier_tache_creer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let label = champ("label").ok_or_else(|| Erreur("libellé de la tâche obligatoire".into()))?;
    db::projet::creer_tache(
        &app.pool,
        entite::courante(),
        id,
        &label,
        champ("debut").as_deref(),
        champ("fin").as_deref(),
    )?;
    Ok(Redirect::to(&format!("/chantiers/{id}?ok=tache")))
}
