//! Ecran des modeles de PDF : lequel chaque piece utilise.
//!
//! C'est `admin/pdf.php` de Dolibarr. Il manquait, et le choix se faisait par
//! variable d'environnement — c'est-à-dire pas par l'utilisateur.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Form;
use db::pdf_modele::{Objet, MODELES};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La page : un choix par type de piece.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let choix: Vec<(Objet, Option<String>)> = Objet::TOUS
        .into_iter()
        .map(|o| {
            let m = db::pdf_modele::modele(&app.pool, entity, o).unwrap_or(None);
            (o, m)
        })
        .collect();
    let ok = q.get("ok").map(|s| s.as_str());

    Ok(ui::page(
        "parametres",
        "modeles-pdf",
        "Modèles de PDF",
        html! {
            h1 { (crate::langue::tr("Modèles de PDF")) }
            p .sous-titre {
                (crate::langue::tr("Chaque type de pièce choisit son modèle. Le réglage est écrit dans la constante de Dolibarr (FACTURE_ADDON_PDF, COMMANDE_ADDON_PDF, PROPALE_ADDON_PDF) : un choix fait ici vaut aussi pour lui."))
            }

            @if ok == Some("enregistre") { (ui::flash("Modèles enregistrés.")) }

            form method="post" action="/parametres/modeles-pdf" {
                table {
                    thead {
                        tr { th { (crate::langue::tr("Pièce")) } th { (crate::langue::tr("Modèle")) } th { (crate::langue::tr("Constante")) } }
                    }
                    tbody {
                        @for (objet, actuel) in &choix {
                            tr {
                                td { (crate::langue::tr(objet.libelle())) }
                                td {
                                    select name=(objet.code()) class="pleine" {
                                        option value="" selected[actuel.is_none()] {
                                            (crate::langue::tr("Standard (défaut)"))
                                        }
                                        @for (code, libelle) in MODELES {
                                            option value=(*code) selected[actuel.as_deref() == Some(*code)] {
                                                (crate::langue::tr(libelle))
                                            }
                                        }
                                    }
                                }
                                td .ref { (objet.constante()) }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" { (crate::langue::tr("Enregistrer")) }
                }
            }

            h2 { (crate::langue::tr("Ce que chaque modèle change")) }
            table {
                thead { tr { th { (crate::langue::tr("Code")) } th { (crate::langue::tr("Effet")) } } }
                tbody {
                    @for (code, libelle) in MODELES {
                        tr {
                            td .ref { (*code) }
                            td { (crate::langue::tr(libelle)) }
                        }
                    }
                }
            }

            p style="color:#78736c;font-size:13.5px;margin-top:16px" {
                (crate::langue::tr("Le modèle est figé sur le PDF produit : changer le réglage ne réécrit pas les documents déjà archivés. Régénérez la pièce pour appliquer le nouveau."))
            }
        },
    ))
}

/// Enregistre les trois choix.
pub async fn enregistrer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let entity = entite::courante();
    // Un code saisi de travers doit etre refuse ici, pas silencieusement remplace
    // par le standard a la generation.
    verifier(&f)?;
    let mut au_moins_un = false;

    for objet in Objet::TOUS {
        if let Some(valeur) = f.get(objet.code()) {
            db::pdf_modele::choisir(&app.pool, entity, objet, valeur)?;
            if !valeur.trim().is_empty() {
                au_moins_un = true;
            }
        }
    }
    if !au_moins_un {
        // Tout remettre au standard est un choix legitime : on ne le refuse pas,
        // mais on l'assume explicitement.
        return Ok(Redirect::to("/parametres/modeles-pdf?ok=enregistre"));
    }

    Ok(Redirect::to("/parametres/modeles-pdf?ok=enregistre"))
}

/// Verifie qu'un code recu correspond a un modele connu (aide aux tests).
pub fn code_connu(code: &str) -> bool {
    MODELES.iter().any(|(c, _)| *c == code)
}

/// Rend une erreur si le formulaire porte un code inconnu.
pub fn verifier(f: &HashMap<String, String>) -> Result<(), Erreur> {
    for objet in Objet::TOUS {
        if let Some(valeur) = f.get(objet.code()) {
            if !valeur.trim().is_empty() && !code_connu(valeur) {
                return Err(Erreur(format!(
                    "modèle inconnu pour {} : « {valeur} »",
                    objet.libelle()
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(paires: &[(&str, &str)]) -> HashMap<String, String> {
        paires
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn les_codes_des_trois_modeles_sont_connus() {
        assert!(code_connu("standard"));
        assert!(code_connu("epure"));
        assert!(code_connu("compact"));
        assert!(!code_connu("luxe"));
    }

    #[test]
    fn un_formulaire_valide_passe() {
        assert!(verifier(&form(&[("devis", "compact"), ("facture", "epure")])).is_ok());
        // Le vide est permis : c'est « défaut ».
        assert!(verifier(&form(&[("facture", "")])).is_ok());
    }

    #[test]
    fn un_modele_inconnu_est_refuse() {
        assert!(verifier(&form(&[("facture", "luxe")])).is_err());
    }
}
