//! Ecran de cloture d'exercice.
//!
//! On montre d'abord ce que la cloture **ferait** : les charges, les produits,
//! le resultat, et les lignes exactes qui seraient ecrites. Le geste est
//! ensuite explicite, et refuse si l'exercice est deja clos.
//!
//! Un ecran de comptabilite qui agit sans montrer ce qu'il fait n'est pas un
//! outil, c'est une boite noire. Celui-ci montre les lignes avant de les poser.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Form;
use db::cloture;
use maud::html;

use crate::{entite, ui, App, Res};

/// Ce que l'ecran demande : un exercice, ou ses bornes.
struct Bornes {
    exercice: String,
    debut: String,
    fin: String,
    date_cloture: String,
}

fn lire(q: &HashMap<String, String>, aujourdhui: &str) -> Bornes {
    // Par defaut : l'annee en cours, en exercice calendaire. C'est le cas de
    // loin le plus courant ; les autres se saisissent.
    let annee_defaut: String = aujourdhui.chars().take(4).collect();
    let exercice = q
        .get("exercice")
        .map(|s| s.trim().to_string())
        .filter(|s| s.chars().all(|c| c.is_ascii_digit()) && s.len() == 4)
        .unwrap_or(annee_defaut);

    let debut = q
        .get("debut")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{exercice}-01-01"));
    let fin = q
        .get("fin")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{exercice}-12-31"));
    let date_cloture = q
        .get("date_cloture")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fin.clone());

    Bornes {
        exercice,
        debut,
        fin,
        date_cloture,
    }
}

/// La page : apercu de ce que la cloture ferait, puis le geste.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let aujourdhui = db::aujourdhui(&app.pool)?;
    let b = lire(&q, &aujourdhui);

    let apercu = cloture::apercu(&app.pool, entity, &b.debut, &b.fin, &b.date_cloture)?;
    let deja = cloture::deja_cloture(&app.pool, entity, &b.exercice)?;
    let instantanes = cloture::instantanes(&app.pool, entity)?;
    let ok = q.get("ok").map(|s| s.as_str());

    Ok(ui::page(
        "comptabilite",
        "cloture",
        "Clôture d'exercice",
        html! {
            h1 {(crate::langue::tr("Clôture d'exercice")) }
            p .sous-titre {
                "Les comptes de charges (classe 6) et de produits (classe 7) sont soldés, "
                "et leur différence est portée au compte 12 « Résultat de l'exercice »."
            }

            @if ok == Some("clos") {
                (ui::flash("Exercice clos : écriture posée et instantané déposé."))
            }

            form method="get" action="/comptabilite/cloture" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Exercice")) }
                            td { input type="text" name="exercice" value=(b.exercice) size="6"; }
                            th {(crate::langue::tr("Du")) }
                            td { input type="text" name="debut" value=(b.debut) size="12"; }
                            th {(crate::langue::tr("Au")) }
                            td { input type="text" name="fin" value=(b.fin) size="12"; }
                            th {(crate::langue::tr("Date de clôture")) }
                            td { input type="text" name="date_cloture" value=(b.date_cloture) size="12"; }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Recalculer")) }
                }
            }

            h2 {(crate::langue::tr("Ce que la clôture ferait")) }
            div .cartes {
                (ui::carte("Charges", &ui::euros(apercu.charges), Some("classe 6"), false))
                (ui::carte("Produits", &ui::euros(apercu.produits), Some("classe 7"), false))
                (ui::carte(
                    if apercu.resultat >= rust_decimal::Decimal::ZERO { "Bénéfice" } else { "Perte" },
                    &ui::euros(if apercu.resultat < rust_decimal::Decimal::ZERO { -apercu.resultat } else { apercu.resultat }),
                    Some("porté au compte 12"),
                    apercu.resultat < rust_decimal::Decimal::ZERO,
                ))
            }

            @if !apercu.a_solder() {
                (ui::vide(
                    "Rien à solder sur cette période : aucune écriture de charge ou de produit.",
                    None,
                ))
            } @else {
                h2 {(crate::langue::tr("Les lignes qui seraient écrites")) }
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Compte")) }
                            th {(crate::langue::tr("Libellé")) }
                            th .num {(crate::langue::tr("Débit")) }
                            th .num {(crate::langue::tr("Crédit")) }
                        }
                    }
                    tbody {
                        @for l in &apercu.lignes {
                            tr {
                                td .ref { (l.compte) }
                                td { (l.libelle) }
                                td .num { @if l.debit != rust_decimal::Decimal::ZERO { (ui::euros(l.debit)) } }
                                td .num { @if l.credit != rust_decimal::Decimal::ZERO { (ui::euros(l.credit)) } }
                            }
                        }
                    }
                }

                @if deja {
                    p .erreur {
                        "L'exercice " (b.exercice) " est déjà clos. Rejouer la clôture réécrit "
                        "la même écriture (même numéro) : le livre ne double pas, mais l'opération "
                        "laisse une trace dans le journal des événements."
                    }
                }

                form method="post" action="/comptabilite/cloture" {
                    input type="hidden" name="exercice" value=(b.exercice);
                    input type="hidden" name="debut" value=(b.debut);
                    input type="hidden" name="fin" value=(b.fin);
                    input type="hidden" name="date_cloture" value=(b.date_cloture);
                    div .actions {
                        @if deja {
                            button type="submit" {(crate::langue::tr("Rejouer la clôture")) }
                        } @else {
                            button type="submit" { "Clore l'exercice " (b.exercice) }
                        }
                        @if deja {
                            input type="hidden" name="forcer" value="1";
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Instantanés déposés")) }
            @if instantanes.is_empty() {
                (ui::vide("Aucune clôture déposée sur cette entité.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Exercice")) }
                            th {(crate::langue::tr("Période")) }
                            th .num {(crate::langue::tr("Charges")) }
                            th .num {(crate::langue::tr("Produits")) }
                            th .num {(crate::langue::tr("Résultat")) }
                            th .num {(crate::langue::tr("Comptes")) }
                        }
                    }
                    tbody {
                        @for i in &instantanes {
                            tr {
                                td .ref { (i.exercice) }
                                td { (i.debut) " → " (i.fin) }
                                td .num { (i.charges) }
                                td .num { (i.produits) }
                                td .num { (i.resultat) }
                                td .num { (i.comptes.to_string()) }
                            }
                        }
                    }
                }
                p style="color:#78736c;font-size:13.5px;margin-top:14px" {
                    "L'instantané est la photographie de l'exercice au moment où il se ferme : "
                    "il répondra plus tard sans recalculer sur un livre qui aura bougé."
                }
            }
        },
    ))
}

/// Le geste : poser l'ecriture de cloture et l'instantane.
pub async fn cloturer(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let entity = entite::courante();
    let v = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let exercice = v("exercice");
    let debut = v("debut");
    let fin = v("fin");
    let date_cloture = v("date_cloture");
    let forcer = f.get("forcer").map(|s| s == "1").unwrap_or(false);

    if exercice.is_empty() || debut.is_empty() || fin.is_empty() || date_cloture.is_empty() {
        return Err(crate::Erreur("exercice ou période incomplète".into()));
    }

    let apercu = cloture::cloturer(
        &app.pool,
        entity,
        &exercice,
        &debut,
        &fin,
        &date_cloture,
        forcer,
    )?;

    // Une cloture change le resultat de la societe : cela se consigne.
    let _ = db::events::enregistrer(
        &app.pool,
        &db::events::Nouvel {
            type_: "ACCOUNTING_CLOSING",
            entity,
            fk_user: None,
            description: &format!(
                "cloture exercice {exercice} du {debut} au {fin} : resultat {}",
                apercu.resultat.normalize()
            ),
            ip: &crate::auth::ip_client(&headers),
            agent: crate::auth::agent_client(&headers).as_deref(),
            fk_object: None,
            methode: None,
        },
    );

    Ok(Redirect::to(&format!(
        "/comptabilite/cloture?exercice={exercice}&debut={debut}&fin={fin}&date_cloture={date_cloture}&ok=clos"
    )))
}
