//! Ecran des immobilisations : ce que l'entreprise possède et qui s'use.
//!
//! Dolibarr a `asset.php`. Une immobilisation n'est pas une dépense : c'est un
//! bien qui reste au bilan et dont la valeur diminue chaque année. L'écran
//! montre donc **la valeur nette comptable**, pas le prix d'achat — c'est elle
//! qui entre dans les comptes.
//!
//! # Le plan est calculé, jamais saisi
//!
//! L'amortissement linéaire se déduit de la valeur et de la durée. Recopier des
//! annuités dans un formulaire garantirait qu'un jour l'une d'elles ne tombe
//! pas juste, et un plan faux ne se voit qu'à la clôture. La dernière annuité
//! porte le reliquat d'arrondi, pour que la somme des dotations égale
//! exactement la valeur.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::immobilisation::{self, NewImmobilisation};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let biens = immobilisation::list(&app.pool, entity, 200, 0)?;
    let brut: Decimal = biens.iter().map(|b| b.valeur).sum();
    let net: Decimal = biens.iter().map(|b| b.valeur_nette).sum();

    Ok(ui::page(
        "immobilisations",
        "immobilisations",
        "Immobilisations",
        html! {
            h1 { (crate::langue::tr("Immobilisations")) }
            p .sous-titre {
                (crate::langue::tr("Ce que l'entreprise possède et qui s'use. La valeur nette entre dans les comptes, pas le prix d'achat."))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Valeur brute"),
                    &ui::euros(brut),
                    Some(&crate::langue::tr("prix d'acquisition")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Valeur nette"),
                    &ui::euros(net),
                    Some(&crate::langue::tr("après amortissements")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Biens"),
                    &biens.len().to_string(),
                    Some(&crate::langue::tr("y compris les biens cédés")),
                    false,
                ))
            }

            h2 { (crate::langue::tr("Nouvelle immobilisation")) }
            form method="post" action="/immobilisations" {
                label for="label" { (crate::langue::tr("Libellé")) } " "
                input type="text" name="label" id="label" required style="min-width:240px"
                      placeholder=(crate::langue::tr("Camionnette, outillage, ordinateur…"));
                " "
                label for="valeur" { (crate::langue::tr("Valeur HT")) } " "
                input type="text" name="valeur" id="valeur" required class="champ-court";
                " "
                label for="duree" { (crate::langue::tr("Durée (ans)")) } " "
                input type="text" name="duree" id="duree" value="5" style="width:70px";
                " "
                label for="date" { (crate::langue::tr("Mise en service")) } " "
                input type="date" name="date" id="date" required;
                " "
                button type="submit" { (crate::langue::tr("Créer et calculer le plan")) }
            }
            p class="aide petite sous-champ" {
                (crate::langue::tr("Amortissement linéaire. La dernière annuité porte le reliquat d'arrondi pour que la somme des dotations égale exactement la valeur."))
            }

            h2 { (crate::langue::tr("Biens")) }
            @if biens.is_empty() {
                (ui::vide("Aucune immobilisation enregistrée.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Libellé")) }
                            th { (crate::langue::tr("Mise en service")) }
                            th { (crate::langue::tr("Durée")) }
                            th { (crate::langue::tr("Valeur")) }
                            th { (crate::langue::tr("Nette")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for b in &biens {
                            tr {
                                td .ref {
                                    a href=(format!("/immobilisations/{}", b.rowid)) { (b.reference.clone()) }
                                }
                                td { (b.label.clone()) }
                                td .ref { (b.date_mise_en_service.clone()) }
                                td .ref { (b.duree_annees) " ans" }
                                td .num { (ui::euros(b.valeur)) }
                                td .num { (ui::euros(b.valeur_nette)) }
                                td {
                                    @if b.date_cession.is_some() {
                                        span class="pastille p-brouillon" {
                                            (crate::langue::tr("cédé"))
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

/// Le plan d'amortissement d'un bien.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let b = immobilisation::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("immobilisation {id} introuvable")))?;
    let plan = immobilisation::plan_en_base(&app.pool, entity, id)?;

    Ok(ui::page(
        "immobilisations",
        "immobilisations",
        &b.reference,
        html! {
            h1 { (b.reference.clone()) " · " (b.label.clone()) }
            p .sous-titre {
                (ui::euros(b.valeur)) " " (crate::langue::tr("sur")) " "
                (b.duree_annees) " " (crate::langue::tr("ans, mise en service le")) " "
                (b.date_mise_en_service.clone())
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Valeur nette"),
                    &ui::euros(b.valeur_nette),
                    Some(&crate::langue::tr("valeur d'acquisition moins les dotations")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Déjà amorti"),
                    &ui::euros(b.valeur - b.valeur_nette),
                    None,
                    false,
                ))
            }

            @if let Some(d) = &b.date_cession {
                h2 { (crate::langue::tr("Cession")) }
                (ui::flash(&format!(
                    "{} {}",
                    crate::langue::tr("Cédé le"),
                    d
                )))
                p class="aide" {
                    (crate::langue::tr("Un bien cédé reste au registre : c'est une pièce comptable, pas une ligne à effacer."))
                }
            }

            h2 { (crate::langue::tr("Plan d'amortissement")) }
            @if plan.is_empty() {
                (ui::vide("Aucune annuité enregistrée.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Exercice")) }
                            th { (crate::langue::tr("Clôture")) }
                            th { (crate::langue::tr("Dotation")) }
                            th { (crate::langue::tr("Cumul")) }
                        }
                    }
                    tbody {
                        @for a in &plan {
                            tr {
                                td .num { (a.exercice) }
                                td .ref { (a.date.clone()) }
                                td .num { (ui::euros(a.dotation)) }
                                td .num { (ui::euros(a.cumule)) }
                            }
                        }
                    }
                }
            }

            @if b.date_cession.is_none() {
                h2 { (crate::langue::tr("Céder ce bien")) }
                form method="post" action=(format!("/immobilisations/{}/ceder", b.rowid)) {
                    label for="date_cession" { (crate::langue::tr("Date")) } " "
                    input type="date" name="date" id="date_cession" required;
                    " "
                    label for="prix" { (crate::langue::tr("Prix de cession HT")) } " "
                    input type="text" name="prix" id="prix" required class="champ-court";
                    " "
                    button type="submit" class="petit secondaire" {
                        (crate::langue::tr("Céder"))
                    }
                }
                p class="aide petite" {
                    (crate::langue::tr("La cession ne supprime rien : elle date la sortie et fixe le prix. Un bien cédé reste au registre."))
                }
            }
        },
    ))
}

/// Cree une immobilisation et son plan.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    // L'auteur vient du contexte de la requete, pas d'un champ de formulaire :
    // une piece comptable doit porter celui qui l'a creee, pas celui qu'on a
    // bien voulu designer.
    let login = crate::utilisateur::courant()
        .ok_or_else(|| Erreur("aucun utilisateur en session".into()))?;
    let entity = entite::courante();
    let auteur = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;

    let date = lire("date");
    let new = NewImmobilisation {
        label: lire("label"),
        valeur: lire("valeur")
            .parse::<Decimal>()
            .map_err(|_| Erreur("valeur invalide".into()))?,
        recovered_vat: None,
        // Acquisition = mise en service : c'est le cas courant d'un bien sorti
        // d'une facture. La colonne est NOT NULL, et une date vide n'a pas de
        // sens comptable.
        date_acquisition: date.clone(),
        date_mise_en_service: date,
        duree_annees: lire("duree")
            .parse::<u32>()
            .map_err(|_| Erreur("durée invalide".into()))?,
        fk_user_creat: auteur,
        note_public: None,
        note_private: None,
    };
    let id = immobilisation::create(&app.pool, entity, &new)?;
    Ok(Redirect::to(&format!("/immobilisations/{id}")))
}

/// Cede un bien.
pub async fn ceder(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let date = f
        .get("date")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("date de cession manquante".into()))?;
    let prix: Decimal = f
        .get("prix")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
        .parse()
        .map_err(|_| Erreur("prix de cession invalide".into()))?;
    immobilisation::ceder(&app.pool, entite::courante(), id, &date, prix)?;
    Ok(Redirect::to(&format!("/immobilisations/{id}")))
}
