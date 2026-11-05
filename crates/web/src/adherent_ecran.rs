//! Ecran des adhérents : les membres, leurs cotisations, leur état.
//!
//! Dolibarr a `adherents/list.php`. Trois choses comptent et l'ecran les met en
//! tete : combien sont **a jour**, combien ne le sont plus, et jusqu'a quand
//! chacun a paye.
//!
//! # La cotisation ne raccourcit jamais
//!
//! Prolonger une adhesion ecrase la date de fin ; la raccourcir retirerait des
//! mois deja payes. Le module refuse donc une date anterieure a celle deja
//! enregistree, et l'ecran ne propose que d'aller plus loin.
//!
//! # Aucun mot de passe ici
//!
//! Un adherent peut avoir un acces (espace membre), mais le mot de passe ne se
//! saisit pas sur cet ecran : il se definit par une fonction separee qui recoit
//! une empreinte deja calculee. Un mot de passe en clair dans un formulaire est
//! un mot de passe dans les journaux du serveur.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::adherent::{self, NewAdherent};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La page.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let adherents = adherent::list(&app.pool, entity, 200, 0)?;
    let (a_jour, total) = adherent::a_jour(&app.pool, entity).unwrap_or((0, 0));
    let types = adherent::types(&app.pool, entity).unwrap_or_default();

    Ok(ui::page(
        "adherents",
        "adherents",
        "Adhérents",
        html! {
            h1 { (crate::langue::tr("Adhérents")) }
            p .sous-titre {
                (crate::langue::tr("Les membres, leur cotisation et sa date d'échéance. Un adhérent à jour n'est pas un adhérent qui a payé un jour : c'est un adhérent dont la cotisation couvre aujourd'hui."))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("À jour"),
                    &a_jour.to_string(),
                    Some(&crate::langue::tr("cotisation en cours")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Total"),
                    &total.to_string(),
                    Some(&crate::langue::tr("toutes situations")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("À relancer"),
                    &total.saturating_sub(a_jour).to_string(),
                    Some(&crate::langue::tr("échus ou suspendus")),
                    total > a_jour,
                ))
            }

            h2 { (crate::langue::tr("Nouvel adhérent")) }
            @if types.is_empty() {
                (ui::vide(
                    "Aucun type d'adhérent actif : il faut en déclarer un avant d'inscrire quelqu'un.",
                    None,
                ))
            } @else {
                form method="post" action="/adherents" {
                    label for="lastname" { (crate::langue::tr("Nom")) } " "
                    input type="text" name="lastname" id="lastname" required style="width:180px";
                    " "
                    label for="firstname" { (crate::langue::tr("Prénom")) } " "
                    input type="text" name="firstname" id="firstname" style="width:150px";
                    " "
                    label for="fk_adherent_type" { (crate::langue::tr("Type")) } " "
                    select name="fk_adherent_type" id="fk_adherent_type" {
                        @for (id, libelle) in &types {
                            option value=(id) { (libelle.clone()) }
                        }
                    }
                    br;
                    label for="email" { (crate::langue::tr("Courriel")) } " "
                    input type="email" name="email" id="email" style="width:230px";
                    " "
                    label for="login" { (crate::langue::tr("Identifiant")) } " "
                    input type="text" name="login" id="login" style="width:140px";
                    " "
                    button type="submit" { (crate::langue::tr("Inscrire")) }
                }
                p style="color:#78736c;font-size:13px;margin:8px 0 0" {
                    (crate::langue::tr("L'identifiant est facultatif. S'il est fourni, il doit être libre : deux adhérents ne peuvent pas se connecter sous le même nom."))
                }
            }

            h2 { (crate::langue::tr("Adhérents")) }
            @if adherents.is_empty() {
                (ui::vide("Aucun adhérent.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Nom")) }
                            th { (crate::langue::tr("Type")) }
                            th { (crate::langue::tr("Cotisation jusqu'au")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for a in &adherents {
                            tr {
                                td .ref { (a.ref_.clone()) }
                                td { (a.nom_complet()) }
                                td {
                                    (a.type_libelle.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td .ref {
                                    (a.date_fin.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td { (pastille(a.statut)) }
                                td { (actions(a)) }
                            }
                        }
                    }
                }
            }

            p style="color:#78736c;font-size:13px;margin-top:16px" {
                (crate::langue::tr("Prolonger une cotisation écrase la date de fin ; la raccourcir retirerait des mois déjà payés, et c'est refusé."))
            }
        },
    ))
}

/// Les actions disponibles pour un adherent.
fn actions(a: &adherent::Adherent) -> maud::Markup {
    html! {
        @if a.statut == adherent::STATUT_SUSPENDU {
            form method="post" action=(format!("/adherents/{}/reactiver", a.rowid)) style="display:inline" {
                button type="submit" class="petit" { (crate::langue::tr("Réactiver")) }
            }
        } @else {
            form method="post" action=(format!("/adherents/{}/cotiser", a.rowid)) {
                input type="date" name="jusqu_a" required style="font-size:13px";
                " "
                button type="submit" class="petit" { (crate::langue::tr("Cotiser")) }
            }
            " "
            form method="post" action=(format!("/adherents/{}/suspendre", a.rowid)) style="display:inline" {
                button type="submit" class="petit secondaire" {
                    (crate::langue::tr("Suspendre"))
                }
            }
        }
    }
}

/// Inscrit un adherent.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();
    let option = |cle: &str| Some(lire(cle)).filter(|s| !s.is_empty());

    let new = NewAdherent {
        lastname: lire("lastname"),
        firstname: option("firstname"),
        email: option("email"),
        login: option("login"),
        fk_adherent_type: lire("fk_adherent_type")
            .parse::<i64>()
            .map_err(|_| Erreur("type d'adhérent manquant".into()))?,
        societe: None,
        address: None,
        zip: None,
        town: None,
        phone: None,
        phone_mobile: None,
        note_private: None,
        note_public: None,
    };
    adherent::create(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to("/adherents"))
}

/// Prolonge une cotisation.
pub async fn cotiser(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let jusqu_a = f
        .get("jusqu_a")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("date de cotisation manquante".into()))?;
    adherent::cotiser(&app.pool, entite::courante(), id, &jusqu_a)?;
    Ok(Redirect::to("/adherents"))
}

/// Suspend un adherent.
pub async fn suspendre(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    adherent::suspendre(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/adherents"))
}

/// Reactive un adherent.
pub async fn reactiver(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    adherent::reactiver(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/adherents"))
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        adherent::STATUT_A_JOUR => ("p-payee", "à jour"),
        adherent::STATUT_BROUILLON => ("p-brouillon", "brouillon"),
        _ => ("p-impayee", "suspendu"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}
