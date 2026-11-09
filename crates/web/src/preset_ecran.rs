//! Ecran des presets : installer un socle de modules d'un clic.
//!
//! Dolibarr s'installe nu, avec trois modules allumes sur quatre-vingt-quatorze.
//! Celui qui monte un fonds de commerce passe une heure a cocher des cases dont
//! il ne sait pas toujours ce qu'elles font.
//!
//! # Ce que l'ecran ne fait pas
//!
//! Il n'**eteint** rien. Appliquer un preset ajoute un socle ; les modules
//! qu'on avait allumes restent allumes. Et il n'ecrit aucune donnee metier :
//! pas de client d'exemple, pas de produit fictif. Une base pre-remplie de faux
//! est une base qu'on ne nettoie jamais.

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Form;
use db::preset::{self, PRESETS};
use maud::html;
use std::collections::HashMap;

use crate::{entite, ui, App, Erreur, Res};

/// La page : les socles proposes, et ce que le dernier a fait.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let actifs = db::module::actifs(&app.pool, entity).unwrap_or_default();

    // Le compte rendu du dernier preset applique, relu depuis l'adresse : un
    // rapport qui ne se lit qu'une fois suffit, et il ne survit pas a un
    // rechargement — c'est voulu, ce n'est pas un etat.
    let applique = q.get("applique").cloned().unwrap_or_default();
    let actives: Vec<String> = q
        .get("actives")
        .map(|v| {
            v.split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let constantes: Vec<String> = q
        .get("constantes")
        .map(|v| {
            v.split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let inconnus: Vec<String> = q
        .get("inconnus")
        .map(|v| {
            v.split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    Ok(ui::page(
        "presets",
        "presets",
        "Presets de base",
        html! {
            h1 { (crate::langue::tr("Presets de base")) }
            p .sous-titre {
                (crate::langue::tr("Un socle de modules cohérent pour un métier. Appliquer un preset allume des modules ; il n'en éteint aucun, et il n'écrit aucune donnée."))
            }

            @if !applique.is_empty() {
                @if actives.is_empty() {
                    (ui::flash(&format!(
                        "{} : rien à activer, tout était déjà en place.",
                        applique
                    )))
                } @else {
                    (ui::flash(&format!(
                        "{} : {} module(s) activé(s) — {}.",
                        applique,
                        actives.len(),
                        actives.join(", ")
                    )))
                }
                @if !constantes.is_empty() {
                    p .aide.bloc {
                        (crate::langue::tr("Réglages posés : "))
                        (constantes.join(", "))
                    }
                }
                @if !inconnus.is_empty() {
                    (ui::echec(&format!(
                        "Non activés, inconnus du catalogue : {}.",
                        inconnus.join(", ")
                    )))
                }
            }

            @for p in PRESETS {
                @let tous_presents = p.modules.iter().all(|m| actifs.contains(*m));
                h2 { (p.libelle) }
                p .aide.marge-basse { (p.description) }
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Module")) }
                            th { (crate::langue::tr("État")) }
                        }
                    }
                    tbody {
                        @for m in p.modules {
                            @let allume = actifs.contains(*m);
                            tr {
                                td .ref { (*m) }
                                td {
                                    @if allume {
                                        span class="pastille p-payee" { (crate::langue::tr("actif")) }
                                    } @else {
                                        span class="pastille p-brouillon" { (crate::langue::tr("éteint")) }
                                    }
                                }
                            }
                        }
                    }
                }
                @if !p.constantes.is_empty() {
                    p .aide.apres {
                        (crate::langue::tr("Pose aussi : "))
                        @for (nom, valeur) in p.constantes {
                            code { (nom) " = " (valeur) } " "
                        }
                    }
                }
                @if tous_presents {
                    p .aide.apres {
                        (crate::langue::tr("Ce socle est déjà en place."))
                    }
                } @else {
                    form method="post" action="/presets" class="haut" {
                        input type="hidden" name="code" value=(p.code);
                        button type="submit" class="petit" {
                            (crate::langue::tr("Appliquer")) " " (p.libelle)
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Ce qu'un preset ne fait pas")) }
            p class="aide" {
                (crate::langue::tr("Il n'éteint aucun module : ce que vous avez allumé reste allumé. Il n'écrit aucune donnée métier — pas de client, pas de produit, pas de tarif. Et le schéma de la base ne change pas d'un octet : c'est la sélection qui change, et elle se défait module par module."))
            }
        },
    ))
}

/// Applique un preset et revient avec le compte rendu dans l'adresse.
pub async fn appliquer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let code = f
        .get("code")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("preset manquant".into()))?;

    let entity = entite::courante();
    let rapport = preset::appliquer(&app.pool, entity, &code)?;

    // Le libelle se relit de la liste : c'est lui qu'on affiche, pas le code.
    let libelle = preset::preset(&code).map(|p| p.libelle).unwrap_or("Preset");
    Ok(Redirect::to(&format!(
        "/presets?applique={}&actives={}&constantes={}&inconnus={}",
        urlencode(libelle),
        urlencode(&rapport.actives.join(",")),
        urlencode(&rapport.constantes.join(",")),
        urlencode(&rapport.inconnus.join(",")),
    )))
}

/// Encode un texte pour une adresse.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for octet in s.as_bytes() {
        match octet {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*octet as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{octet:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_virgule_dans_un_libelle_ne_casse_pas_l_adresse() {
        // Les listes de modules sont separees par des virgules : un libelle qui
        // en contient une doit etre encode, sinon le decoupage se decale.
        assert_eq!(urlencode("BTP, maconnerie"), "BTP%2C+maconnerie");
        assert_eq!(urlencode("btp"), "btp");
    }
}
