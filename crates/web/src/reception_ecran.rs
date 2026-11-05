//! Ecran des réceptions : l'entrée de marchandise fournisseur.
//!
//! Dolibarr a `reception.php`. C'est le pendant fournisseur de l'expédition :
//! le camion arrive, on constate ce qui est là, on valide, puis on réceptionne.
//!
//! # Ce que cet écran ne fait pas, et pourquoi
//!
//! **Il ne bouge pas le stock.** Une réception devrait augmenter les quantités
//! en stock, mais écrire le stock sans le module de mouvements complets
//! produirait des quantités fantômes : impossible ensuite de dire d'où vient un
//! écart. La réception est donc tracée, et le mouvement viendra avec le module
//! dédié. Le manque est assumé et écrit ici plutôt que découvert plus tard.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::reception::{self, NewReception, NewReceptionLine};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let receptions = reception::list(&app.pool, entity, 200, 0)?;
    let fournisseurs = db::achat::fournisseurs(&app.pool, entity, 500, 0)?;

    Ok(ui::page(
        "receptions",
        "receptions",
        "Réceptions",
        html! {
            h1 { (crate::langue::tr("Réceptions")) }
            p .sous-titre {
                (crate::langue::tr("L'entrée de marchandise : ce que le fournisseur a livré, constaté puis réceptionné."))
            }

            h2 { (crate::langue::tr("Nouvelle réception")) }
            @if fournisseurs.is_empty() {
                (ui::vide(
                    "Aucun fournisseur enregistré : il n'y a personne à réceptionner.",
                    Some(("/fournisseurs", "Voir les fournisseurs")),
                ))
            } @else {
                form method="post" action="/receptions" {
                    label for="fk_soc" { (crate::langue::tr("Fournisseur")) } " "
                    select name="fk_soc" id="fk_soc" style="min-width:260px" {
                        @for f in &fournisseurs {
                            option value=(f.rowid) { (f.nom.clone()) }
                        }
                    }
                    " "
                    label for="ref_supplier" { (crate::langue::tr("Réf. bon")) } " "
                    input type="text" name="ref_supplier" id="ref_supplier"
                          placeholder=(crate::langue::tr("bon de livraison du fournisseur"));
                    br;
                    label for="description" { (crate::langue::tr("Désignation")) } " "
                    input type="text" name="description" id="description" required
                          style="min-width:320px";
                    " "
                    label for="qty" { (crate::langue::tr("Quantité")) } " "
                    input type="text" name="qty" id="qty" value="1" style="width:90px" required;
                    " "
                    button type="submit" { (crate::langue::tr("Créer le bon")) }
                }
                p style="color:#78736c;font-size:13px;margin:8px 0 0" {
                    (crate::langue::tr("La saisie se fait ligne par ligne ; le bon se complète ensuite. Une réception ne valide rien tant que vous ne l'avez pas décidé."))
                }
            }

            h2 { (crate::langue::tr("Bons existants")) }
            @if receptions.is_empty() {
                (ui::vide("Aucune réception.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Fournisseur")) }
                            th { (crate::langue::tr("Réf. bon")) }
                            th { (crate::langue::tr("Quantité")) }
                            th { (crate::langue::tr("État")) }
                        }
                    }
                    tbody {
                        @for r in &receptions {
                            tr {
                                td .ref {
                                    a href=(format!("/receptions/{}", r.rowid)) { (r.ref_.clone()) }
                                }
                                td { (r.tiers.clone().unwrap_or_else(|| "—".to_string())) }
                                td .ref { (r.ref_supplier.clone().unwrap_or_else(|| "—".to_string())) }
                                td .num { (r.quantite_totale.normalize().to_string()) }
                                td { (pastille(r.fk_statut)) }
                            }
                        }
                    }
                }
            }

            p style="color:#78736c;font-size:13px;margin-top:16px" {
                (crate::langue::tr("Le stock n'est pas modifié par cet écran : le mouvement de stock viendra avec le module dédié, pour que chaque quantité ait une origine traçable."))
            }
        },
    ))
}

/// Le detail d'un bon.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let r = reception::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("réception {id} introuvable")))?;
    let lignes = reception::lines(&app.pool, entity, id)?;

    Ok(ui::page(
        "receptions",
        "receptions",
        &r.ref_,
        html! {
            h1 { (r.ref_.clone()) }
            p .sous-titre {
                (r.tiers.clone().unwrap_or_else(|| "—".to_string()))
                @if let Some(b) = &r.ref_supplier {
                    " · " (crate::langue::tr("bon")) " " (b.clone())
                }
            }

            table {
                tbody {
                    tr {
                        th { (crate::langue::tr("État")) }
                        td { (pastille(r.fk_statut)) }
                    }
                    @if let Some(d) = &r.date_valid {
                        tr {
                            th { (crate::langue::tr("Validée le")) }
                            td .ref { (d.clone()) }
                        }
                    }
                    @if let Some(d) = &r.date_reception {
                        tr {
                            th { (crate::langue::tr("Réceptionnée le")) }
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
                            th { (crate::langue::tr("Lot")) }
                            th { (crate::langue::tr("Entrepôt")) }
                        }
                    }
                    tbody {
                        @for l in &lignes {
                            tr {
                                td {
                                    (l.description.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td .num { (l.qty.normalize().to_string()) }
                                td .ref { (l.batch.clone().unwrap_or_else(|| "—".to_string())) }
                                td .ref {
                                    (l.fk_entrepot.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string()))
                                }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Actions")) }
            @if r.fk_statut == reception::STATUT_BROUILLON {
                form method="post" action=(format!("/receptions/{}/valider", r.rowid)) {
                    button type="submit" { (crate::langue::tr("Valider")) }
                }
            } @else if r.fk_statut == reception::STATUT_VALIDEE {
                form method="post" action=(format!("/receptions/{}/receptionner", r.rowid)) {
                    label for="date" { (crate::langue::tr("Date de réception")) } " "
                    input type="date" name="date" id="date" required;
                    " "
                    button type="submit" { (crate::langue::tr("Réceptionner")) }
                }
                form method="post" action=(format!("/receptions/{}/supprimer", r.rowid))
                     style="margin-top:10px" {
                    button type="submit" class="petit secondaire" {
                        (crate::langue::tr("Supprimer"))
                    }
                }
            } @else {
                p style="color:#78736c;font-size:13.5px" {
                    (crate::langue::tr("Cette réception est close. Le stock n'a pas été mouvementé : c'est le module de stock qui le fera."))
                }
            }
        },
    ))
}

/// Cree un bon avec sa premiere ligne.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let fk_soc = lire("fk_soc")
        .parse::<i64>()
        .map_err(|_| Erreur("fournisseur manquant".into()))?;
    let description = lire("description");
    if description.is_empty() {
        return Err(Erreur("la désignation est obligatoire".into()));
    }
    let qty: Decimal = lire("qty")
        .parse()
        .map_err(|_| Erreur("quantité invalide".into()))?;

    let mut new = NewReception::new(fk_soc);
    new.ref_supplier = Some(lire("ref_supplier")).filter(|s| !s.is_empty());
    new.lines = vec![NewReceptionLine::new(&description, qty)];

    let id = reception::create(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to(&format!("/receptions/{id}")))
}

/// Valide un bon.
pub async fn valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    reception::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/receptions/{id}")))
}

/// Receptionne un bon.
pub async fn receptionner(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let date = f
        .get("date")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("date de réception manquante".into()))?;
    reception::receptionner(&app.pool, entite::courante(), id, &date)?;
    Ok(Redirect::to(&format!("/receptions/{id}")))
}

/// Supprime un brouillon.
pub async fn supprimer(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    reception::supprimer(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/receptions"))
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        reception::STATUT_RECEPTIONNEE => ("p-payee", "réceptionnée"),
        reception::STATUT_VALIDEE => ("p-partielle", "validée"),
        _ => ("p-brouillon", "brouillon"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}
