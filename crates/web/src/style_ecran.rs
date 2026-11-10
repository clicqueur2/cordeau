//! Le choix du style : sept apparences, montrées avant d'être choisies.
//!
//! Le style suit le **compte** (`llx_user_param`, clé `cordeau.style`), pas le
//! navigateur : on retrouve la même apparence d'un poste à l'autre, et la page
//! arrive déjà habillée — plus de flash au chargement.
//!
//! Chaque aperçu porte les jetons du style qu'il montre : la feuille des aperçus
//! est la même que la feuille réelle, réécrite mécaniquement (voir
//! `theme::css_apercus`). Un aperçu qui divergerait du rendu ne servirait à rien.

use std::collections::HashMap;

use axum::extract::State;
use axum::response::Redirect;
use axum::Form;
use maud::html;

use crate::*;

pub(crate) async fn page(
    State(_app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let style_actuel = utilisateur::style();

    Ok(ui::page(
        "parametres",
        "style",
        "Style d'affichage",
        html! {
            style { (maud::PreEscaped(crate::theme::css_apercus())) }
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Style d'affichage")) }
            p .sous-titre {
                (crate::langue::tr("Le style suit votre compte : vous le retrouvez sur n'importe quel poste, et la page s'affiche déjà habillée."))
            }

            form method="post" action="/style" {
                div .grille-styles {
                    @for (cle, nom, intention) in crate::theme::STYLES {
                        label .style-carte {
                            input type="radio" name="style" value=(cle)
                                  checked[*cle == style_actuel];
                            div .apercu data-style=(cle) {
                                div .apercu-titre { (crate::langue::tr(nom)) }
                                div .apercu-kpi {
                                    span .apercu-titre-kpi { "Facturé" }
                                    span .apercu-valeur { "12 480 €" }
                                }
                                div .apercu-ligne {
                                    span .pastille.p-payee { "payée" }
                                    span .apercu-nombre { "1 250,00 €" }
                                }
                                div .apercu-actions {
                                    span .bouton.petit { "Action" }
                                    span .bouton.secondaire.petit { "Autre" }
                                }
                            }
                            div .style-texte {
                                strong { (crate::langue::tr(nom)) }
                                span { (crate::langue::tr(intention)) }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Appliquer ce style")) }
                    span .aide.petite {
                        (crate::langue::tr("Le réglage vaut pour votre compte, sur toutes les sociétés."))
                    }
                }
            }
        },
    ))
}

pub(crate) async fn choisir(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let choisi = f
        .get("style")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("style manquant".into()))?;
    // La liste vient d'un formulaire : on n'accepte qu'un style declare.
    if !crate::theme::STYLES
        .iter()
        .any(|(cle, _, _)| *cle == choisi)
    {
        return Err(Erreur(format!("style inconnu : {choisi}")));
    }
    let login = utilisateur::courant().ok_or_else(|| Erreur("session sans utilisateur".into()))?;
    let id = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;
    db::preferences::ecrire(&app.pool, entite::courante(), id, "cordeau.style", &choisi)?;
    Ok(Redirect::to("/style?ok=style"))
}
