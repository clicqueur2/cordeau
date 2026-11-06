//! Documents (GED) et agenda.
//!
//! Deux vues transverses : le document appartient a une piece, l'evenement a
//! une echeance. Aucune des deux n'a de table a elle ici ; elles lisent ce que
//! les autres ecrivent.

use crate::*;

pub(crate) async fn documents(State(app): State<App>) -> Res<Markup> {
    let docs = db::ged::tous(&app.pool, entite::courante(), 200)?;
    Ok(ui::page(
        "documents",
        "ged",
        "Documents",
        html! {
            h1 {(crate::langue::tr("Documents archivés")) }
            p .sous-titre {
                "Les PDF de devis et de factures sont déposés ici à chaque édition, "
                "rangés comme dans Dolibarr (documents/<module>/<réf>/)."
            }
            @if docs.is_empty() {
                (ui::vide("Aucun document archivé.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Libellé")) } th {(crate::langue::tr("Dossier")) } th {(crate::langue::tr("Fichier")) } } }
                    tbody {
                        @for d in &docs {
                            tr {
                                td .ref { (d.date_c.clone().unwrap_or_default()) }
                                td { (d.label) }
                                td .ref { (d.filepath) }
                                td .ref { (d.filename) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn agenda(State(app): State<App>) -> Res<Markup> {
    let evenements = db::journal::recents(&app.pool, entite::courante(), 200)?;
    Ok(ui::page(
        "agenda",
        "evenements",
        "Agenda",
        html! {
            h1 {(crate::langue::tr("Agenda")) }
            p .sous-titre {
                "Les événements sont écrits dans le journal Dolibarr (llx_actioncomm) : "
                "validation de devis, relances, règlements."
            }
            h2 {(crate::langue::tr("Nouvel événement")) }
            form method="post" action="/agenda/nouveau" .actions {
                input type="text" name="label" placeholder="Objet du rendez-vous" required.champ-large;
                input type="date" name="date" aria-label="Date";
                button type="submit" {(crate::langue::tr("Ajouter")) }
            }

            @if evenements.is_empty() {
                (ui::vide("Aucun événement.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Événement")) } th {(crate::langue::tr("Objet")) } } }
                    tbody {
                        @for e in &evenements {
                            tr {
                                td .ref { (e.datep.clone().unwrap_or_default()) }
                                td { (e.label) }
                                td {
                                    @if let (Some(t), Some(id)) = (&e.elementtype, e.fk_element) {
                                        @if t == "facture" {
                                            a href={ "/factures/" (id) } { "Facture " (id) }
                                        } @else if t == "propal" {
                                            a href={ "/devis/" (id) } { "Devis " (id) }
                                        } @else {
                                            (t) " " (id)
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
