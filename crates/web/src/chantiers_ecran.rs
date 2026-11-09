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

            @let visibles = colonnes::visibles(
                &app.pool,
                "chantiers",
                &["ref", "titre", "client", "debut", "fin", "budget", "statut"],
            )?;
            (colonnes::choix_colonnes(
                "chantiers",
                &["ref", "titre", "client", "debut", "fin", "budget", "statut"],
                &visibles,
            ))

            @if liste.is_empty() {
                (ui::vide("Aucun chantier.", None))
            } @else {
                table {
                    thead {
                        tr {
                            @if colonnes::montre(&visibles, "ref") { th {(crate::langue::tr("Réf.")) } }
                            @if colonnes::montre(&visibles, "titre") { th {(crate::langue::tr("Chantier")) } }
                            @if colonnes::montre(&visibles, "client") { th {(crate::langue::tr("Client")) } }
                            @if colonnes::montre(&visibles, "debut") { th {(crate::langue::tr("Début")) } }
                            @if colonnes::montre(&visibles, "fin") { th {(crate::langue::tr("Fin")) } }
                            @if colonnes::montre(&visibles, "budget") { th .num {(crate::langue::tr("Budget")) } }
                            @if colonnes::montre(&visibles, "statut") { th { "État" } }
                        }
                    }
                    tbody {
                        @for c in &liste {
                            tr {
                                @if colonnes::montre(&visibles, "ref") {
                                    td .ref { a href={ "/chantiers/" (c.rowid) } { (c.ref_) } }
                                }
                                @if colonnes::montre(&visibles, "titre") {
                                    td { a href={ "/chantiers/" (c.rowid) } { (c.title) } }
                                }
                                @if colonnes::montre(&visibles, "client") {
                                    td { (nom_tiers(p, c.fk_soc)?) }
                                }
                                @if colonnes::montre(&visibles, "debut") {
                                    td .ref { (app::fmt_date_fr(c.date_start.as_deref())) }
                                }
                                @if colonnes::montre(&visibles, "fin") {
                                    td .ref { (app::fmt_date_fr(c.date_end.as_deref())) }
                                }
                                @if colonnes::montre(&visibles, "budget") {
                                    td .num { (ui::euros(c.budget.unwrap_or_default())) }
                                }
                                @if colonnes::montre(&visibles, "statut") {
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
            }

            h2 {(crate::langue::tr("Nouveau chantier")) }
            form method="post" action="/chantiers" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Client")) }
                            td {
                                select name="soc" class="pleine" {
                                    option value="" {(crate::langue::tr("— chantier interne —")) }
                                    @for c in &clients {
                                        option value=(c.rowid) { (c.nom) }
                                    }
                                }
                            }
                        }
                        tr { th {(crate::langue::tr("Intitulé")) } td { input type="text" name="title" required class="pleine"; } }
                        tr { th {(crate::langue::tr("Début")) } td { input type="date" name="debut" class="champ-moyen"; } }
                        tr { th {(crate::langue::tr("Fin prévue")) } td { input type="date" name="fin" class="champ-moyen"; } }
                        tr { th {(crate::langue::tr("Budget HT")) } td { input type="number" name="budget" step="0.01" min="0".champ-moyen; } }
                        tr { th {(crate::langue::tr("Description")) } td { input type="text" name="description" class="pleine"; } }
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

            @if !taches.is_empty() {
                @let barres: Vec<ui::BarreGantt> = taches
                    .iter()
                    .map(|t| ui::BarreGantt {
                        label: t.label.clone(),
                        debut: t.date_start.clone().unwrap_or_default(),
                        fin: t.date_end.clone().unwrap_or_default(),
                        avancement: t.progress,
                    })
                    .collect();
                h2 {(crate::langue::tr("Planning")) }
                (ui::gantt(&barres, &aujourdhui(p)?))
                p .aide.petite {
                    (crate::langue::tr("La ligne pointillée est aujourd'hui : ce qui la dépasse est en retard."))
                }
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
                        tr { th {(crate::langue::tr("Libellé")) } td { input type="text" name="label" required class="pleine"; } }
                        tr { th {(crate::langue::tr("Début")) } td { input type="date" name="debut" class="champ-moyen"; } }
                        tr { th {(crate::langue::tr("Fin")) } td { input type="date" name="fin" class="champ-moyen"; } }
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

/// Le planning de tous les chantiers, sur une seule echelle de temps.
///
/// C'est la vue qui manque a un tableau de bord : ce qui commence, ce qui finit,
/// et ce qui se chevauche. Les barres vont de la date de debut a la date de fin
/// prevue ; l'avancement se suit tache par tache, sur la fiche.
pub(crate) async fn gantt_chantiers(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let liste = db::projet::list(p, entite::courante(), 300, 0)?;
    let jour = aujourdhui(p)?;
    let barres: Vec<ui::BarreGantt> = liste
        .iter()
        .map(|c| ui::BarreGantt {
            label: format!("{} — {}", c.ref_, c.title),
            debut: c.date_start.clone().unwrap_or_default(),
            fin: c.date_end.clone().unwrap_or_default(),
            avancement: 100,
        })
        .collect();

    Ok(ui::page(
        "chantiers",
        "gantt",
        "Planning des chantiers",
        html! {
            h1 {(crate::langue::tr("Planning des chantiers")) }
            p .sous-titre {
                (crate::langue::tr("Tous les chantiers sur une seule échelle : ce qui se chevauche se voit."))
            }
            (ui::gantt(&barres, &jour))
            p .aide.petite {
                (crate::langue::tr("La ligne pointillée est aujourd'hui. L'avancement se suit tâche par tâche, sur la fiche du chantier."))
            }
        },
    ))
}
