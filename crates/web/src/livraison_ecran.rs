//! Ecran des livraisons : le bon de livraison client.
//!
//! Dolibarr a `delivery.php`. Trois temps : le bon se prepare, se valide, puis
//! se livre. Un bon valide ne se reouvre pas — le client peut l'avoir recu.
//!
//! # Le stock ne bouge pas
//!
//! Une livraison decremente le stock dans Dolibarr, mais `llx_deliverydet` ne
//! porte **aucun `fk_entrepot`** : on ne sait pas d'ou part la marchandise.
//! Ecrire le stock reviendrait a vider un entrepot au hasard. Le bon est donc
//! trace, et le mouvement viendra avec le module dedie.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::livraison::{self, NewLigneLivraison, NewLivraison};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste et le formulaire.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let bons = livraison::list(&app.pool, entity, 200, 0)?;
    let clients = db::party::list(&app.pool, entity, 500, 0)?;

    Ok(ui::page(
        "livraisons",
        "livraisons",
        "Livraisons",
        html! {
            h1 { (crate::langue::tr("Livraisons")) }
            p .sous-titre {
                (crate::langue::tr("Le bon de livraison : ce qui part chez le client. Il se valide, puis se livre."))
            }

            h2 { (crate::langue::tr("Nouveau bon")) }
            @if clients.is_empty() {
                (ui::vide(
                    "Aucun client enregistré : il n'y a personne à livrer.",
                    Some(("/tiers", "Voir les tiers")),
                ))
            } @else {
                form method="post" action="/livraisons" {
                    label for="fk_soc" { (crate::langue::tr("Client")) } " "
                    select name="fk_soc" id="fk_soc".champ-large {
                        @for c in &clients {
                            option value=(c.rowid) { (c.nom.clone()) }
                        }
                    }
                    " "
                    label for="ref_customer" { (crate::langue::tr("Réf. client")) } " "
                    input type="text" name="ref_customer" id="ref_customer".champ-moyen;
                    br;
                    label for="description" { (crate::langue::tr("Désignation")) } " "
                    input type="text" name="description" id="description" required
                         .champ-large;
                    " "
                    label for="qty" { (crate::langue::tr("Qté")) } " "
                    input type="text" name="qty" id="qty" value="1".champ-court required;
                    " "
                    label for="subprice" { (crate::langue::tr("P.U. HT")) } " "
                    input type="text" name="subprice" id="subprice" required class="champ-court";
                    " "
                    button type="submit" { (crate::langue::tr("Créer le bon")) }
                }
                p class="aide petite sous-champ" {
                    (crate::langue::tr("La saisie se fait ligne par ligne. Un bon n'engage rien tant qu'il n'est pas validé."))
                }
            }

            h2 { (crate::langue::tr("Bons de livraison")) }
            @if bons.is_empty() {
                (ui::vide("Aucune livraison enregistrée.", None))
            } @else {
                table {
                    thead{
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Client")) }
                            th { (crate::langue::tr("Créé le")) }
                            th { (crate::langue::tr("Quantité")) }
                            th { (crate::langue::tr("Total HT")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for b in &bons {
                            tr {
                                td .ref { (b.ref_.clone()) }
                                td { (b.tiers.clone().unwrap_or_else(|| "—".to_string())) }
                                td .ref {
                                    (b.date_creation.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td .num { (b.quantite_totale.normalize().to_string()) }
                                td .num { (ui::euros(b.total_ht)) }
                                td { (pastille(b.fk_statut)) }
                                td { (actions(b)) }
                            }
                        }
                    }
                }
            }

            p class="aide petite haut" {
                (crate::langue::tr("Ce bon ne modifie pas le stock : une ligne de livraison ne dit pas d'où part la marchandise. Le mouvement viendra avec le module de stock."))
            }
        },
    ))
}

/// Les boutons disponibles selon l'etat.
fn actions(b: &livraison::Livraison) -> maud::Markup {
    html! {
        @if b.fk_statut == livraison::STATUT_BROUILLON {
            form method="post" action=(format!("/livraisons/{}/valider", b.rowid)) class="en-ligne" {
                button type="submit" class="petit" { (crate::langue::tr("Valider")) }
            }
        } @else if b.fk_statut == livraison::STATUT_VALIDEE {
            form method="post" action=(format!("/livraisons/{}/livrer", b.rowid)) {
                input type="date" name="date" required.petite;
                " "
                button type="submit" class="petit" { (crate::langue::tr("Livrer")) }
            }
        }
    }
}

/// Cree un bon avec sa premiere ligne.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let mut new = NewLivraison::new(
        lire("fk_soc")
            .parse::<i64>()
            .map_err(|_| Erreur("client manquant".into()))?,
    );
    new.ref_customer = Some(lire("ref_customer")).filter(|s| !s.is_empty());
    new.lines = vec![NewLigneLivraison::new(
        &lire("description"),
        lire("qty")
            .parse::<Decimal>()
            .map_err(|_| Erreur("quantité invalide".into()))?,
        lire("subprice")
            .parse::<Decimal>()
            .map_err(|_| Erreur("prix invalide".into()))?,
    )];

    livraison::create(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to("/livraisons"))
}

/// Valide un bon.
pub async fn valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    livraison::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/livraisons"))
}

/// Marque un bon comme livre.
pub async fn livrer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let date = f
        .get("date")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("date de livraison manquante".into()))?;
    livraison::livrer(&app.pool, entite::courante(), id, &date)?;
    Ok(Redirect::to("/livraisons"))
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        livraison::STATUT_LIVREE => ("p-payee", "livrée"),
        livraison::STATUT_VALIDEE => ("p-partielle", "validée"),
        _ => ("p-brouillon", "brouillon"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}
