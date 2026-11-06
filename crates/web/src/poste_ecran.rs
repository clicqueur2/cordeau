//! Ecran des postes de charge : les moyens de production.
//!
//! Dolibarr a `workstation.php`. Un poste de charge est une ressource de
//! production — une table de découpe, un four, un atelier — avec un coût
//! horaire. C'est ce qui donne un prix au temps dans une nomenclature.
//!
//! # Le coût horaire est une somme, pas un choix
//!
//! Dolibarr calcule `thm_operator_estimated + thm_machine_estimated`. Le nombre
//! d'opérateurs n'entre pas dans la formule : il décrit l'organisation, pas le
//! coût. L'écran affiche donc le total du noyau et non une interpretation.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::poste_charge::{self, ModifPoste, NouveauPoste};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste et le formulaire.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let postes = poste_charge::list(&app.pool, entity, 200, 0)?;

    Ok(ui::page(
        "postes",
        "postes",
        "Postes de charge",
        html! {
            h1 { (crate::langue::tr("Postes de charge")) }
            p .sous-titre {
                (crate::langue::tr("Les moyens de production : une table de découpe, un four, un atelier. Leur coût horaire donne un prix au temps dans une nomenclature."))
            }

            h2 { (crate::langue::tr("Nouveau poste")) }
            form method="post" action="/postes" {
                label for="ref" { (crate::langue::tr("Référence")) } " "
                input type="text" name="ref" id="ref" required style="width:180px"
                      placeholder=(crate::langue::tr("ATELIER-1"));
                " "
                label for="label" { (crate::langue::tr("Libellé")) } " "
                input type="text" name="label" id="label" required style="min-width:240px";
                " "
                label for="nb_operators" { (crate::langue::tr("Opérateurs")) } " "
                input type="number" name="nb_operators" id="nb_operators" value="1" min="0" class="champ-court";
                " "
                label for="hourly_cost" { (crate::langue::tr("Coût horaire")) } " "
                input type="text" name="hourly_cost" id="hourly_cost" required class="champ-court";
                " "
                button type="submit" { (crate::langue::tr("Créer")) }
            }
            p class="aide petite sous-champ" {
                (crate::langue::tr("Un poste naît désactivé : c'est l'état de repos dans Dolibarr, il n'y a pas de brouillon sur cette fiche."))
            }

            h2 { (crate::langue::tr("Postes")) }
            @if postes.is_empty() {
                (ui::vide("Aucun poste de charge.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Libellé")) }
                            th { (crate::langue::tr("Opérateurs")) }
                            th { (crate::langue::tr("Coût horaire")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for p in &postes {
                            tr {
                                td .ref { (p.ref_.clone()) }
                                td { (p.label.clone()) }
                                td .num { (p.nb_operators) }
                                td .num { (ui::euros(p.hourly_cost)) }
                                td { (pastille(p.statut)) }
                                td {
                                    @if p.statut == poste_charge::STATUT_ACTIF {
                                        form method="post" action=(format!("/postes/{}/desactiver", p.rowid)) class="en-ligne" {
                                            button type="submit" class="petit secondaire" {
                                                (crate::langue::tr("Désactiver"))
                                            }
                                        }
                                    } @else {
                                        form method="post" action=(format!("/postes/{}/valider", p.rowid)) class="en-ligne" {
                                            button type="submit" class="petit" {
                                                (crate::langue::tr("Activer"))
                                            }
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

/// La fiche d'un poste : ses ressources, ses groupes, son cout.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let p = poste_charge::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("poste {id} introuvable")))?;
    let ressources = poste_charge::ressources_de(&app.pool, entity, id).unwrap_or_default();
    let groupes = poste_charge::groupes_de(&app.pool, entity, id).unwrap_or_default();
    let cout = poste_charge::cout_horaire(&app.pool, entity, id).unwrap_or(Decimal::ZERO);

    Ok(ui::page(
        "postes",
        "postes",
        &p.ref_,
        html! {
            h1 { (p.ref_.clone()) " · " (p.label.clone()) }
            p .sous-titre {
                (p.nb_operators) " " (crate::langue::tr("opérateur(s)"))
                @if let Some(d) = &p.description {
                    " · " (d.clone())
                }
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Coût horaire"),
                    &ui::euros(cout),
                    Some(&crate::langue::tr("opérateur + machine, la formule du noyau")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Ressources liées"),
                    &ressources.len().to_string(),
                    None,
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Groupes liés"),
                    &groupes.len().to_string(),
                    None,
                    false,
                ))
            }

            h2 { (crate::langue::tr("Modifier")) }
            form method="post" action=(format!("/postes/{}/modifier", p.rowid)) {
                label for="label" { (crate::langue::tr("Libellé")) } " "
                input type="text" name="label" id="label" value=(p.label.clone()) required style="min-width:240px";
                " "
                label for="nb_operators" { (crate::langue::tr("Opérateurs")) } " "
                input type="number" name="nb_operators" id="nb_operators" value=(p.nb_operators) min="0" class="champ-court";
                " "
                label for="hourly_cost" { (crate::langue::tr("Coût horaire")) } " "
                input type="text" name="hourly_cost" id="hourly_cost"
                      value=(p.hourly_cost.to_string()) required class="champ-court";
                " "
                button type="submit" class="petit" { (crate::langue::tr("Enregistrer")) }
            }
            p class="aide petite sous-champ" {
                (crate::langue::tr("La référence ne se modifie pas : c'est elle qu'on retrouve dans une nomenclature."))
            }

            h2 { (crate::langue::tr("Ressources")) }
            @if ressources.is_empty() {
                (ui::vide("Aucune ressource attachée.", None))
            } @else {
                p { (ressources.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ")) }
            }
            form method="post" action=(format!("/postes/{}/ressource", p.rowid)) {
                label for="fk_ressource" { (crate::langue::tr("Attacher la ressource n°")) } " "
                input type="number" name="fk_ressource" id="fk_ressource" min="1" required class="champ-court";
                " "
                button type="submit" class="petit" { (crate::langue::tr("Attacher")) }
            }

            h2 { (crate::langue::tr("Groupes")) }
            @if groupes.is_empty() {
                (ui::vide("Aucun groupe attaché.", None))
            } @else {
                p { (groupes.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(", ")) }
            }
            form method="post" action=(format!("/postes/{}/groupe", p.rowid)) {
                label for="fk_groupe" { (crate::langue::tr("Attacher le groupe n°")) } " "
                input type="number" name="fk_groupe" id="fk_groupe" min="1" required class="champ-court";
                " "
                button type="submit" class="petit" { (crate::langue::tr("Attacher")) }
            }
        },
    ))
}

/// Cree un poste.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let login = crate::utilisateur::courant()
        .ok_or_else(|| Erreur("aucun utilisateur en session".into()))?;
    let auteur = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;

    let new = NouveauPoste {
        ref_: lire("ref"),
        label: lire("label"),
        description: None,
        nb_operators: lire("nb_operators").parse::<i32>().unwrap_or(1),
        hourly_cost: lire("hourly_cost")
            .parse::<Decimal>()
            .map_err(|_| Erreur("coût horaire invalide".into()))?,
        fk_user_creat: auteur,
    };
    let id = poste_charge::creer(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to(&format!("/postes/{id}")))
}

/// Modifie un poste.
pub async fn modifier(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let modif = ModifPoste {
        label: lire("label"),
        description: None,
        nb_operators: lire("nb_operators").parse::<i32>().unwrap_or(1),
        hourly_cost: lire("hourly_cost")
            .parse::<Decimal>()
            .map_err(|_| Erreur("coût horaire invalide".into()))?,
    };
    poste_charge::modifier(&app.pool, entite::courante(), id, &modif)?;
    Ok(Redirect::to(&format!("/postes/{id}")))
}

/// Active un poste.
pub async fn valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    poste_charge::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/postes/{id}")))
}

/// Desactive un poste.
pub async fn desactiver(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    poste_charge::desactiver(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/postes"))
}

/// Attache une ressource.
pub async fn attacher_ressource(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let ressource = f
        .get("fk_ressource")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("ressource manquante".into()))?;
    poste_charge::attacher_ressource(&app.pool, entite::courante(), id, ressource)?;
    Ok(Redirect::to(&format!("/postes/{id}")))
}

/// Attache un groupe.
pub async fn attacher_groupe(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let groupe = f
        .get("fk_groupe")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("groupe manquant".into()))?;
    poste_charge::attacher_groupe(&app.pool, entite::courante(), id, groupe)?;
    Ok(Redirect::to(&format!("/postes/{id}")))
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        poste_charge::STATUT_ACTIF => ("p-payee", "actif"),
        _ => ("p-brouillon", "désactivé"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}
