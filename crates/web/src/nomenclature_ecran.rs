//! Ecran des nomenclatures : de quoi un ouvrage est fait.
//!
//! Dolibarr a `bom.php`. Une nomenclature dit « 1 m2 de mur en pierre = tant de
//! pierres, tant de sable, tant de ciment ». C'est le chiffrage qui ne se
//! redevine pas a chaque devis.
//!
//! # Le cout de matiere refuse de mentir
//!
//! Dolibarr ecrit `0` quand aucun prix n'est connu pour un composant. Un zero
//! dans un cout de revient ne se distingue pas d'un composant gratuit : le
//! devis sort trop bas et personne ne le voit. Ici, `cout_matiere` **echoue en
//! nommant le produit sans prix**. Pas de total partiel.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::nomenclature::{self, NouveauComposant, NouvelleNomenclature};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste et le formulaire.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let nomenclatures = nomenclature::list(&app.pool, entity, 200, 0)?;
    let produits = db::product::list(&app.pool, entity, 500, 0).unwrap_or_default();

    Ok(ui::page(
        "nomenclatures",
        "nomenclatures",
        "Nomenclatures",
        html! {
            h1 { (crate::langue::tr("Nomenclatures")) }
            p .sous-titre {
                (crate::langue::tr("De quoi un ouvrage est fait : « 1 m² de mur en pierre = tant de pierres, tant de sable, tant de ciment ». Le chiffrage ne se redévine pas à chaque devis."))
            }

            h2 { (crate::langue::tr("Nouvelle nomenclature")) }
            @if produits.is_empty() {
                (ui::vide(
                    "Aucun produit : une nomenclature compose un produit à partir d'autres.",
                    Some(("/produits", "Voir les produits")),
                ))
            } @else {
                form method="post" action="/nomenclatures" {
                    label for="ref" { (crate::langue::tr("Référence")) } " "
                    input type="text" name="ref" id="ref" required style="width:160px"
                          placeholder="MUR-PIERRE-M2";
                    " "
                    label for="label" { (crate::langue::tr("Libellé")) } " "
                    input type="text" name="label" id="label" required style="min-width:240px"
                          placeholder=(crate::langue::tr("Mur en pierre, au m²"));
                    br;
                    label for="fk_product" { (crate::langue::tr("Produit fini")) } " "
                    select name="fk_product" id="fk_product" style="min-width:280px" {
                        @for p in &produits {
                            option value=(p.rowid) { (p.label.clone()) }
                        }
                    }
                    " "
                    label for="qty" { (crate::langue::tr("Quantité produite")) } " "
                    input type="text" name="qty" id="qty" value="1" style="width:90px" required;
                    br;
                    label for="comp_product" { (crate::langue::tr("Composant")) } " "
                    select name="comp_product" id="comp_product" style="min-width:280px" {
                        @for p in &produits {
                            option value=(p.rowid) { (p.label.clone()) }
                        }
                    }
                    " "
                    label for="comp_qty" { (crate::langue::tr("Quantité")) } " "
                    input type="text" name="comp_qty" id="comp_qty" required style="width:90px";
                    " "
                    button type="submit" { (crate::langue::tr("Créer")) }
                }
                p style="color:#78736c;font-size:13px;margin:8px 0 0" {
                    (crate::langue::tr("La quantité d'en-tête dit à quoi se rapporte la nomenclature : 1 pour un m², 10 pour 10 m². Les autres composants s'ajoutent sur la fiche."))
                }
            }

            h2 { (crate::langue::tr("Nomenclatures")) }
            @if nomenclatures.is_empty() {
                (ui::vide("Aucune nomenclature.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Libellé")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for n in &nomenclatures {
                            tr {
                                td .ref {
                                    a href=(format!("/nomenclatures/{}", n.rowid)) { (n.ref_.clone()) }
                                }
                                td { (n.label.clone()) }
                                td { (pastille(n.statut)) }
                                td {
                                    a href=(format!("/nomenclatures/{}", n.rowid)) class="petit secondaire" {
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

/// La fiche d'une nomenclature : ses composants et son cout.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let n = nomenclature::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("nomenclature {id} introuvable")))?;
    let composants = nomenclature::composants(&app.pool, id)?;
    // Le cout peut echouer : un composant sans prix. On l'affiche tel quel
    // plutot que de rendre un total partiel.
    let cout = nomenclature::cout_matiere(&app.pool, entity, id);

    Ok(ui::page(
        "nomenclatures",
        "nomenclatures",
        &n.ref_,
        html! {
            h1 { (n.ref_.clone()) " · " (n.label.clone()) }
            p .sous-titre {
                @if !n.ref_produit.trim().is_empty() {
                    (n.ref_produit.clone()) " · "
                }
                (crate::langue::tr("quantité produite")) " " (n.qty.normalize().to_string())
            }

            @match &cout {
                Ok(v) => {
                    div .cartes {
                        (ui::carte(
                            &crate::langue::tr("Coût matière"),
                            &ui::euros(*v),
                            Some(&crate::langue::tr("somme des composants au prix connu")),
                            false,
                        ))
                    }
                }
                Err(e) => {
                    (ui::echec(&format!(
                        "{} {}",
                        crate::langue::tr("Coût matière incomplet :"),
                        e
                    )))
                }
            }

            h2 { (crate::langue::tr("Composants")) }
            @if composants.is_empty() {
                (ui::vide("Aucun composant.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Produit")) }
                            th { (crate::langue::tr("Quantité")) }
                            th { (crate::langue::tr("Commentaire")) }
                        }
                    }
                    tbody {
                        @for c in &composants {
                            tr {
                                td { (c.ref_produit.clone()) " · " (c.label_produit.clone()) }
                                td .num { (c.qty.normalize().to_string()) }
                                td {
                                    (c.commentaire.clone().unwrap_or_else(|| "—".to_string()))
                                }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Actions")) }
            @if n.statut == nomenclature::STATUT_BROUILLON {
                form method="post" action=(format!("/nomenclatures/{}/valider", n.rowid)) style="margin-top:14px" {
                    button type="submit" { (crate::langue::tr("Valider")) }
                }
                p style="color:#78736c;font-size:13.5px" {
                    (crate::langue::tr("Valider fige la composition. Une nomenclature validée est un prix de revient constaté, pas une hypothèse."))
                }
            } @else {
                p style="color:#78736c;font-size:13.5px" {
                    (crate::langue::tr("Cette nomenclature est validée ou désactivée. Sa composition ne bouge plus."))
                }
            }
        },
    ))
}

/// Cree une nomenclature avec son premier composant.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();
    let decimal = |cle: &str, quoi: &str| -> Result<Decimal, Erreur> {
        lire(cle)
            .parse::<Decimal>()
            .map_err(|_| Erreur(format!("{quoi} invalide")))
    };

    let login = crate::utilisateur::courant()
        .ok_or_else(|| Erreur("aucun utilisateur en session".into()))?;
    let entity = entite::courante();
    let auteur = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;

    let entete = NouvelleNomenclature {
        ref_: lire("ref"),
        label: lire("label"),
        fk_product: lire("fk_product")
            .parse::<i64>()
            .map_err(|_| Erreur("produit fini manquant".into()))?,
        qty: decimal("qty", "quantité produite")?,
        description: None,
        fk_user_creat: auteur,
    };
    let composants = vec![NouveauComposant::new(
        lire("comp_product")
            .parse::<i64>()
            .map_err(|_| Erreur("composant manquant".into()))?,
        decimal("comp_qty", "quantité du composant")?,
    )];

    let id = nomenclature::creer(&app.pool, entity, &entete, &composants)?;
    Ok(Redirect::to(&format!("/nomenclatures/{id}")))
}

/// Valide une nomenclature.
pub async fn valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    nomenclature::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/nomenclatures/{id}")))
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        nomenclature::STATUT_VALIDEE => ("p-payee", "validée"),
        nomenclature::STATUT_DESACTIVEE => ("p-brouillon", "désactivée"),
        _ => ("p-brouillon", "brouillon"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}
