//! Ecran des achats fournisseurs : demandes de prix et commandes.
//!
//! Dolibarr a `supplier_proposal.php` et `supplier_order.php`. Le premier
//! demande un prix, le second engage l'achat. Les deux vivent ici parce qu'ils
//! partagent le meme geste : on choisit un fournisseur, on decrit ce qu'on
//! veut, et on regarde ce que ca coute.
//!
//! # Pourquoi les deux dans un seul ecran
//!
//! Une demande de prix qui aboutit devient une commande. Les separer
//! obligerait a ressaisir les lignes, et la deuxieme saisie ne serait jamais
//! exactement la premiere.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::commande_fournisseur::{self, NewCommandeFournisseur, NewLigneAchat};
use db::proposition_fournisseur::{self, NewLigneDemande, NewPropositionFournisseur};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La page : les deux listes et les deux formulaires.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let demandes = proposition_fournisseur::list(&app.pool, entity, 100, 0)?;
    let commandes = commande_fournisseur::list(&app.pool, entity, 100, 0)?;
    let fournisseurs = db::achat::fournisseurs(&app.pool, entity, 500, 0)?;

    Ok(ui::page(
        "achats-fournisseur",
        "achats-fournisseur",
        "Achats fournisseurs",
        html! {
            h1 { (crate::langue::tr("Achats fournisseurs")) }
            p .sous-titre {
                (crate::langue::tr("La demande de prix et la commande. Une demande qui aboutit devient une commande : on ne ressaisit pas les lignes."))
            }

            @if fournisseurs.is_empty() {
                (ui::vide(
                    "Aucun fournisseur enregistré : il n'y a personne à qui demander un prix.",
                    Some(("/fournisseurs", "Voir les fournisseurs")),
                ))
            } @else {
                h2 { (crate::langue::tr("Demander un prix")) }
                form method="post" action="/achats-fournisseur/demande" {
                    label for="fk_soc" { (crate::langue::tr("Fournisseur")) } " "
                    select name="fk_soc" id="fk_soc" style="min-width:260px" {
                        @for f in &fournisseurs {
                            option value=(f.rowid) { (f.nom.clone()) }
                        }
                    }
                    br;
                    label for="label" { (crate::langue::tr("Désignation")) } " "
                    input type="text" name="label" id="label" required style="min-width:320px";
                    " "
                    label for="qty" { (crate::langue::tr("Qté")) } " "
                    input type="text" name="qty" id="qty" value="1" style="width:80px" required;
                    " "
                    label for="subprice" { (crate::langue::tr("Prix HT attendu")) } " "
                    input type="text" name="subprice" id="subprice" required style="width:120px";
                    " "
                    label for="tva_tx" { (crate::langue::tr("TVA %")) } " "
                    input type="text" name="tva_tx" id="tva_tx" value="20" style="width:70px";
                    " "
                    button type="submit" { (crate::langue::tr("Créer la demande")) }
                }

                h2 { (crate::langue::tr("Commander")) }
                form method="post" action="/achats-fournisseur/commande" {
                    label for="fk_soc_cmd" { (crate::langue::tr("Fournisseur")) } " "
                    select name="fk_soc" id="fk_soc_cmd" style="min-width:260px" {
                        @for f in &fournisseurs {
                            option value=(f.rowid) { (f.nom.clone()) }
                        }
                    }
                    " "
                    label for="ref_supplier" { (crate::langue::tr("Réf. fournisseur")) } " "
                    input type="text" name="ref_supplier" id="ref_supplier" style="width:160px";
                    br;
                    label for="label_cmd" { (crate::langue::tr("Désignation")) } " "
                    input type="text" name="label" id="label_cmd" required style="min-width:320px";
                    " "
                    label for="qty_cmd" { (crate::langue::tr("Qté")) } " "
                    input type="text" name="qty" id="qty_cmd" value="1" style="width:80px" required;
                    " "
                    label for="subprice_cmd" { (crate::langue::tr("Prix HT")) } " "
                    input type="text" name="subprice" id="subprice_cmd" required style="width:120px";
                    " "
                    label for="tva_cmd" { (crate::langue::tr("TVA %")) } " "
                    input type="text" name="tva_tx" id="tva_cmd" value="20" style="width:70px";
                    " "
                    button type="submit" { (crate::langue::tr("Créer la commande")) }
                }
            }

            h2 { (crate::langue::tr("Demandes de prix")) }
            @if demandes.is_empty() {
                (ui::vide("Aucune demande de prix.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Fournisseur")) }
                            th { (crate::langue::tr("Total TTC")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for d in &demandes {
                            tr {
                                td .ref { (d.ref_.clone()) }
                                td { (d.fournisseur.clone().unwrap_or_else(|| "—".to_string())) }
                                td .num { (montant(d.total_ttc)) }
                                td { (pastille_demande(d.fk_statut)) }
                                td { (actions_demande(d)) }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Commandes")) }
            @if commandes.is_empty() {
                (ui::vide("Aucune commande fournisseur.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Fournisseur")) }
                            th { (crate::langue::tr("Total TTC")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for c in &commandes {
                            tr {
                                td .ref { (c.ref_.clone()) }
                                td { (c.fournisseur.clone().unwrap_or_else(|| "—".to_string())) }
                                td .num { (montant(c.total_ttc)) }
                                td { (pastille_commande(c.fk_statut)) }
                                td {
                                    a href=(format!("/receptions?commande={}", c.rowid))
                                      class="petit secondaire" {
                                        (crate::langue::tr("Réceptionner"))
                                    }
                                }
                            }
                        }
                    }
                }
            }

            p style="color:#78736c;font-size:13px;margin-top:16px" {
                (crate::langue::tr("Le cycle d'une commande fournisseur va de la validation à la réception. Chaque étape refuse de se produire hors séquence : une commande non approuvée ne part pas."))
            }
        },
    ))
}

/// Les boutons d'une demande de prix.
fn actions_demande(d: &proposition_fournisseur::PropositionFournisseur) -> maud::Markup {
    html! {
        @if d.fk_statut == proposition_fournisseur::STATUT_BROUILLON {
            form method="post" action=(format!("/achats-fournisseur/demande/{}/valider", d.rowid)) style="display:inline" {
                button type="submit" class="petit" { (crate::langue::tr("Valider")) }
            }
        } @else if d.fk_statut == proposition_fournisseur::STATUT_VALIDEE {
            form method="post" action=(format!("/achats-fournisseur/demande/{}/cloturer", d.rowid)) style="display:inline" {
                button type="submit" class="petit" { (crate::langue::tr("Clore")) }
            }
            " "
            form method="post" action=(format!("/achats-fournisseur/demande/{}/refuser", d.rowid)) style="display:inline" {
                button type="submit" class="petit secondaire" { (crate::langue::tr("Refuser")) }
            }
        }
    }
}

/// Cree une demande de prix.
pub async fn demande(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let mut new = NewPropositionFournisseur::new(
        lire("fk_soc")
            .parse::<i64>()
            .map_err(|_| Erreur("fournisseur manquant".into()))?,
    );
    new.lines = vec![NewLigneDemande::new(
        &lire("label"),
        decimal(&lire("qty"), "quantité")?,
        decimal(&lire("subprice"), "prix")?,
        decimal(&lire("tva_tx"), "TVA")?,
    )];

    proposition_fournisseur::create(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to("/achats-fournisseur"))
}

/// Valide une demande de prix.
pub async fn demande_valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    proposition_fournisseur::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/achats-fournisseur"))
}

/// Clot une demande de prix.
pub async fn demande_cloturer(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    proposition_fournisseur::cloturer(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/achats-fournisseur"))
}

/// Refuse une demande de prix.
pub async fn demande_refuser(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    proposition_fournisseur::refuser(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/achats-fournisseur"))
}

/// Cree une commande fournisseur.
pub async fn commande(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let mut new = NewCommandeFournisseur::new(
        lire("fk_soc")
            .parse::<i64>()
            .map_err(|_| Erreur("fournisseur manquant".into()))?,
    );
    new.ref_supplier = Some(lire("ref_supplier")).filter(|s| !s.is_empty());
    new.lines = vec![NewLigneAchat::new(
        &lire("label"),
        decimal(&lire("qty"), "quantité")?,
        decimal(&lire("subprice"), "prix")?,
        decimal(&lire("tva_tx"), "TVA")?,
    )];

    commande_fournisseur::create(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to("/achats-fournisseur"))
}

/// Un montant, ou un tiret quand il n'y en a pas.
fn montant(v: Option<Decimal>) -> String {
    v.map(ui::euros).unwrap_or_else(|| "—".to_string())
}

/// Un texte lu en nombre, avec un message qui dit lequel.
fn decimal(texte: &str, quoi: &str) -> Result<Decimal, Erreur> {
    texte
        .parse::<Decimal>()
        .map_err(|_| Erreur(format!("{quoi} invalide")))
}

/// La pastille d'une demande.
fn pastille_demande(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        proposition_fournisseur::STATUT_CLOSE => ("p-payee", "close"),
        proposition_fournisseur::STATUT_REFUSEE => ("p-impayee", "refusée"),
        proposition_fournisseur::STATUT_VALIDEE => ("p-partielle", "envoyée"),
        _ => ("p-brouillon", "brouillon"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}

/// La pastille d'une commande.
fn pastille_commande(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        5 | 4 => ("p-payee", "close"),
        3 => ("p-partielle", "commandée"),
        2 => ("p-validee", "approuvée"),
        1 => ("p-validee", "validée"),
        _ => ("p-brouillon", "brouillon"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_montant_absent_s_affiche_en_tiret() {
        assert_eq!(montant(None), "—");
        assert_eq!(montant(Some(Decimal::ZERO)), ui::euros(Decimal::ZERO));
    }
}
