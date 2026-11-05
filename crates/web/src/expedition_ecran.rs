//! Ecran des expeditions : le bon de livraison.
//!
//! Dolibarr a `expedition.php`. Le principe : une expedition nait presque
//! toujours d'une **commande validee** — on ne livre pas ce qui n'a pas ete
//! commande. La creation depuis une commande est donc le chemin normal, et la
//! saisie libre reste possible pour un envoi sans commande.
//!
//! Trois etapes, et aucune reversible : brouillon, validee, expediee. Un bon de
//! livraison valide ne se reouvre pas — c'est une piece que le client peut
//! recevoir, et la rouvrir apres coup reviendrait a modifier un document deja
//! transmis.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::expedition::{self, NewExpedition, NewExpeditionLine};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let expeditions = expedition::list(&app.pool, entity, 200, 0)?;
    // Seules les commandes validees peuvent donner une expedition : une
    // commande en brouillon n'engage rien.
    let commandes: Vec<db::commande::Commande> = db::commande::list(&app.pool, entity, 200, 0)?
        .into_iter()
        .filter(|c| c.fk_statut == 1)
        .collect();

    Ok(ui::page(
        "expeditions",
        "expeditions",
        "Expéditions",
        html! {
            h1 { (crate::langue::tr("Expéditions")) }
            p .sous-titre {
                (crate::langue::tr("Les bons de livraison. Une expédition naît d'une commande validée : on ne livre pas ce qui n'a pas été commandé."))
            }

            h2 { (crate::langue::tr("Nouvelle expédition")) }
            @if commandes.is_empty() {
                (ui::vide(
                    "Aucune commande validée : il n'y a rien à expédier. Validez d'abord une commande.",
                    Some(("/commandes", "Voir les commandes")),
                ))
            } @else {
                form method="post" action="/expeditions/depuis-commande" {
                    label for="commande" { (crate::langue::tr("Depuis la commande")) } " "
                    select name="commande" id="commande" style="min-width:320px" {
                        @for c in &commandes {
                            @let montant = c.total_ttc.map(ui::euros).unwrap_or_else(|| "—".to_string());
                            option value=(c.rowid) {
                                (c.ref_.clone()) " — " (montant)
                            }
                        }
                    }
                    " "
                    button type="submit" { (crate::langue::tr("Créer le bon")) }
                }
            }

            h2 { (crate::langue::tr("Bons existants")) }
            @if expeditions.is_empty() {
                (ui::vide("Aucune expédition.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Client")) }
                            th { (crate::langue::tr("Créée le")) }
                            th { (crate::langue::tr("Lignes")) }
                            th { (crate::langue::tr("Quantité")) }
                            th { (crate::langue::tr("État")) }
                            th { (crate::langue::tr("Suivi")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for e in &expeditions {
                            tr {
                                td .ref {
                                    a href=(format!("/expeditions/{}", e.rowid)) { (e.ref_.clone()) }
                                }
                                td { (e.tiers.clone().unwrap_or_else(|| "—".to_string())) }
                                td .ref { (e.date_creation.clone().unwrap_or_else(|| "—".to_string())) }
                                td { (e.nb_lignes) }
                                td .ref { (e.quantite_totale.normalize().to_string()) }
                                td { (pastille(e.fk_statut)) }
                                td .ref {
                                    (e.tracking_number.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td {
                                    a href=(format!("/expeditions/{}", e.rowid)) class="petit secondaire" {
                                        (crate::langue::tr("Ouvrir"))
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

/// Le detail d'un bon, avec ses actions.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let e = expedition::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("expédition {id} introuvable")))?;
    let lignes = expedition::lines(&app.pool, entity, id)?;

    Ok(ui::page(
        "expeditions",
        "expeditions",
        &e.ref_,
        html! {
            h1 { (e.ref_.clone()) }
            p .sous-titre {
                (e.tiers.clone().unwrap_or_else(|| "—".to_string()))
                @if let Some(r) = &e.ref_customer {
                    " · " (crate::langue::tr("référence client")) " " (r.clone())
                }
            }

            table {
                tbody {
                    tr {
                        th { (crate::langue::tr("État")) }
                        td { (pastille(e.fk_statut)) }
                    }
                    tr {
                        th { (crate::langue::tr("Créée le")) }
                        td .ref { (e.date_creation.clone().unwrap_or_else(|| "—".to_string())) }
                    }
                    @if let Some(d) = &e.date_valid {
                        tr {
                            th { (crate::langue::tr("Validée le")) }
                            td .ref { (d.clone()) }
                        }
                    }
                    @if let Some(d) = &e.date_expedition {
                        tr {
                            th { (crate::langue::tr("Expédiée le")) }
                            td .ref { (d.clone()) }
                        }
                    }
                    @if let Some(d) = &e.tracking_number {
                        tr {
                            th { (crate::langue::tr("Numéro de suivi")) }
                            td .ref { (d.clone()) }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Lignes")) }
            @if lignes.is_empty() {
                (ui::vide("Ce bon n'a aucune ligne.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Désignation")) }
                            th { (crate::langue::tr("Quantité")) }
                            th { (crate::langue::tr("Entrepôt")) }
                        }
                    }
                    tbody {
                        @for l in &lignes {
                            tr {
                                td {
                                    (l.description.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td .ref { (l.qty.normalize().to_string()) }
                                td .ref {
                                    (l.fk_entrepot.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string()))
                                }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Actions")) }
            @if e.fk_statut == expedition::STATUT_BROUILLON {
                form method="post" action=(format!("/expeditions/{}/valider", e.rowid)) {
                    button type="submit" { (crate::langue::tr("Valider")) }
                }
                p style="color:#78736c;font-size:13.5px" {
                    (crate::langue::tr("Valider rend le bon définitif : il ne se rouvrira plus, et il ne pourra plus être supprimé."))
                }
            } @else if e.fk_statut == expedition::STATUT_VALIDEE {
                form method="post" action=(format!("/expeditions/{}/expedier", e.rowid)) {
                    label for="date" { (crate::langue::tr("Date de départ")) } " "
                    input type="date" name="date" id="date" required;
                    " "
                    label for="tracking" { (crate::langue::tr("Suivi")) } " "
                    input type="text" name="tracking" id="tracking" placeholder="optionnel";
                    " "
                    button type="submit" { (crate::langue::tr("Marquer expédiée")) }
                }
                form method="post" action=(format!("/expeditions/{}/supprimer", e.rowid))
                     style="margin-top:10px" {
                    button type="submit" class="petit secondaire" {
                        (crate::langue::tr("Supprimer"))
                    }
                }
            } @else {
                p style="color:#78736c;font-size:13.5px" {
                    (crate::langue::tr("Ce bon est expédié. Il ne se modifie plus."))
                }
            }
        },
    ))
}

/// Cree un bon a partir d'une commande validee.
pub async fn depuis_commande(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let entity = entite::courante();
    let id_commande = f
        .get("commande")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("commande manquante".into()))?;

    let commande = db::commande::get(&app.pool, entity, id_commande)?
        .ok_or_else(|| Erreur(format!("commande {id_commande} introuvable")))?;
    if commande.fk_statut != 1 {
        return Err(Erreur(
            "une expédition ne se crée que depuis une commande validée".into(),
        ));
    }
    let lignes_commande = db::commande::lines(&app.pool, entity, id_commande)?;
    if lignes_commande.is_empty() {
        return Err(Erreur("cette commande n'a aucune ligne".into()));
    }

    let mut new = NewExpedition::new(commande.fk_soc);
    new.ref_customer = Some(commande.ref_.clone());
    new.note_public = commande.note_public.clone();
    new.lines = lignes_commande
        .iter()
        .map(|l| {
            let libelle = l
                .label
                .clone()
                .or_else(|| l.description.clone())
                .unwrap_or_else(|| "Article".to_string());
            let mut ligne = NewExpeditionLine::new(&libelle, l.qty.unwrap_or(Decimal::ONE));
            ligne.fk_product = l.fk_product;
            ligne
        })
        .collect();

    let id = expedition::create(&app.pool, entity, &new)?;
    Ok(Redirect::to(&format!("/expeditions/{id}")))
}

/// Valide un bon.
pub async fn valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    expedition::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/expeditions/{id}")))
}

/// Marque un bon comme expedie.
pub async fn expedier(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let date = f
        .get("date")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("date de départ manquante".into()))?;
    let tracking = f
        .get("tracking")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    expedition::expedier(
        &app.pool,
        entite::courante(),
        id,
        &date,
        tracking.as_deref(),
    )?;
    Ok(Redirect::to(&format!("/expeditions/{id}")))
}

/// Supprime un brouillon.
pub async fn supprimer(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    expedition::supprimer(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/expeditions"))
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        expedition::STATUT_EXPEDIEE => ("p-payee", "expédiée"),
        expedition::STATUT_VALIDEE => ("p-partielle", "validée"),
        _ => ("p-brouillon", "brouillon"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_trois_etats_ont_chacun_leur_pastille() {
        for statut in [
            expedition::STATUT_BROUILLON,
            expedition::STATUT_VALIDEE,
            expedition::STATUT_EXPEDIEE,
        ] {
            let rendu = pastille(statut).into_string();
            assert!(rendu.contains("pastille"), "{rendu}");
        }
    }
}
