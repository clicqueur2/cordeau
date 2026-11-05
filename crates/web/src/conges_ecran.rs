//! Ecran des congés : qui s'absente, quand, et ce que ça coûte en jours.
//!
//! Dolibarr a `holiday.php`. Le circuit est en quatre temps : la demande part,
//! un responsable la valide, les ressources humaines l'approuvent, et c'est
//! seulement à l'approbation que les jours sont décomptés du solde.
//!
//! # Le chiffre affiché est une borne haute
//!
//! Les jours ouvrés sont calculés du lundi au vendredi. Les jours fériés
//! français ne sont pas connus du noyau : un congé qui enjambe le 15 août
//! compte donc un jour de trop. C'est écrit sur l'écran, pas seulement dans le
//! code — un chiffre qui se prétend exact alors qu'il ignore un férié fait
//! signer des soldes faux.

use std::collections::HashMap;

use axum::extract::State;
use axum::response::Redirect;
use axum::Form;
use db::conges::{self, NewConge};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La page : la liste, le formulaire, et les actions.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let demandes = conges::list(&app.pool, entity, 200, 0)?;
    let utilisateurs = db::user::list(&app.pool, entity)?;
    let types = conges::types(&app.pool, entity)?;

    Ok(ui::page(
        "conges",
        "conges",
        "Congés",
        html! {
            h1 { (crate::langue::tr("Congés")) }
            p .sous-titre {
                (crate::langue::tr("Les demandes d'absence, du dépôt à l'approbation. Les jours ne sont décomptés qu'à l'approbation."))
            }

            h2 { (crate::langue::tr("Nouvelle demande")) }
            @if types.is_empty() || utilisateurs.is_empty() {
                (ui::vide(
                    "Il manque un type de congé actif ou un utilisateur : la saisie n'est pas possible.",
                    None,
                ))
            } @else {
                form method="post" action="/conges" {
                    label for="fk_user" { (crate::langue::tr("Salarié")) } " "
                    select name="fk_user" id="fk_user" {
                        @for u in &utilisateurs {
                            option value=(u.rowid) { (u.login.clone()) }
                        }
                    }
                    " "
                    label for="fk_type" { (crate::langue::tr("Type")) } " "
                    select name="fk_type" id="fk_type" {
                        @for (id, libelle) in &types {
                            option value=(id) { (libelle.clone()) }
                        }
                    }
                    " "
                    label for="fk_validator" { (crate::langue::tr("Validé par")) } " "
                    select name="fk_validator" id="fk_validator" {
                        @for u in &utilisateurs {
                            option value=(u.rowid) { (u.login.clone()) }
                        }
                    }
                    br;
                    label for="date_debut" { (crate::langue::tr("Du")) } " "
                    input type="date" name="date_debut" id="date_debut" required;
                    " "
                    label for="date_fin" { (crate::langue::tr("au")) } " "
                    input type="date" name="date_fin" id="date_fin" required;
                    " "
                    label {
                        input type="checkbox" name="halfday" value="1";
                        " " (crate::langue::tr("demi-journées"))
                    }
                    " "
                    label for="description" { (crate::langue::tr("Motif")) } " "
                    input type="text" name="description" id="description" required
                          style="min-width:260px";
                    " "
                    button type="submit" { (crate::langue::tr("Déposer")) }
                }
                p style="color:#78736c;font-size:13px;margin:8px 0 0" {
                    (crate::langue::tr("Jours ouvrés calculés du lundi au vendredi. Les jours fériés ne sont pas connus : le nombre affiché est une borne haute."))
                }
            }

            h2 { (crate::langue::tr("Demandes")) }
            @if demandes.is_empty() {
                (ui::vide("Aucune demande de congé.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Salarié")) }
                            th { (crate::langue::tr("Type")) }
                            th { (crate::langue::tr("Période")) }
                            th { (crate::langue::tr("Jours ouvrés")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for c in &demandes {
                            tr {
                                td .ref { (c.ref_.clone()) }
                                td { (c.demandeur.clone().unwrap_or_else(|| "—".to_string())) }
                                td { (c.type_conge.clone().unwrap_or_else(|| "—".to_string())) }
                                td .ref {
                                    (c.date_debut.clone()) " → " (c.date_fin.clone())
                                }
                                td .num { (c.nb_open_day.normalize().to_string()) }
                                td { (pastille(c.statut)) }
                                td { (actions(c)) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Les boutons disponibles pour une demande, selon son état.
///
/// Une demande refusée ou annulée n'a plus d'action : la remettre en circuit
/// demanderait une nouvelle demande, et donc une nouvelle trace.
fn actions(c: &conges::Conge) -> maud::Markup {
    html! {
        @if c.statut == conges::STATUT_DEMANDE {
            form method="post" action=(format!("/conges/{}/valider", c.rowid)) style="display:inline" {
                button type="submit" class="petit" { (crate::langue::tr("Valider")) }
            }
            " "
            form method="post" action=(format!("/conges/{}/annuler", c.rowid)) style="display:inline" {
                button type="submit" class="petit secondaire" {
                    (crate::langue::tr("Annuler"))
                }
            }
        } @else if c.statut == conges::STATUT_VALIDE {
            form method="post" action=(format!("/conges/{}/approuver", c.rowid)) style="display:inline" {
                button type="submit" class="petit" { (crate::langue::tr("Approuver")) }
            }
            " "
            form method="post" action=(format!("/conges/{}/refuser", c.rowid)) style="display:inline" {
                input type="text" name="motif" placeholder=(crate::langue::tr("motif"))
                      style="width:130px";
                " "
                button type="submit" class="petit secondaire" {
                    (crate::langue::tr("Refuser"))
                }
            }
        }
    }
}

/// Depose une demande.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();
    let nombre = |cle: &str| -> Result<i64, Erreur> {
        lire(cle)
            .parse::<i64>()
            .map_err(|_| Erreur(format!("{cle} manquant")))
    };

    let new = NewConge {
        fk_user: nombre("fk_user")?,
        fk_type: nombre("fk_type")?,
        description: lire("description"),
        date_debut: lire("date_debut"),
        date_fin: lire("date_fin"),
        halfday: f.contains_key("halfday"),
        fk_validator: nombre("fk_validator")?,
        note_private: None,
        note_public: None,
    };
    conges::create(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to("/conges"))
}

/// Valide une demande. Le valideur enregistre est celui de la session.
pub async fn valider(
    State(app): State<App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Res<Redirect> {
    let entity = entite::courante();
    let demande = conges::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("congé {id} introuvable")))?;
    // Le validateur designe porte la responsabilite ; a defaut, la demande
    // elle-meme, sinon la validation n'aurait pas d'auteur.
    let validateur = if demande.fk_validator > 0 {
        demande.fk_validator
    } else {
        demande.fk_user
    };
    conges::valider(&app.pool, entity, id, validateur)?;
    Ok(Redirect::to("/conges"))
}

/// Approuve une demande.
pub async fn approuver(
    State(app): State<App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Res<Redirect> {
    let entity = entite::courante();
    let demande = conges::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("congé {id} introuvable")))?;
    conges::approuver(&app.pool, entity, id, demande.fk_validator)?;
    Ok(Redirect::to("/conges"))
}

/// Refuse une demande, avec un motif.
pub async fn refuser(
    State(app): State<App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let motif = f
        .get("motif")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("un refus doit être motivé".into()))?;
    let entity = entite::courante();
    let demande = conges::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("congé {id} introuvable")))?;
    conges::refuser(&app.pool, entity, id, demande.fk_validator, &motif)?;
    Ok(Redirect::to("/conges"))
}

/// Annule une demande.
pub async fn annuler(
    State(app): State<App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Res<Redirect> {
    conges::annuler(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/conges"))
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        conges::STATUT_APPROUVE => ("p-payee", "approuvé"),
        conges::STATUT_VALIDE => ("p-partielle", "validé"),
        conges::STATUT_REFUSE => ("p-impayee", "hors circuit"),
        _ => ("p-brouillon", "demandé"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}
