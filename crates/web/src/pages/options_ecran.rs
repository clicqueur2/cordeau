//! Options d'un module : les constantes qu'il declare.
//!
//! C'est la page que Dolibarr affiche derriere la roue crantee d'un module.
//! Une option non renseignee prend son defaut, ecrit dans la declaration du
//! module — jamais une valeur inventee a l'affichage.
use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use maud::{html, Markup};

use crate::{message, Erreur, Res};

/// Les expeditions ont leur propre ecran : elles se creent depuis une commande
/// validee, se valident puis s'expedient. Voir `expedition_ecran`.
/// Options d'un module : les constantes qu'il declare, lues et ecrites dans
/// `llx_const`. C'est la page que Dolibarr affiche derriere la roue crantee.
pub async fn options_module(
    State(app): State<crate::App>,
    axum::extract::Path(code): axum::extract::Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let module =
        crate::plugin::par_code(&code).ok_or_else(|| Erreur(format!("module inconnu : {code}")))?;
    let actif = crate::menu_actif::est_actif(module.code);

    let mut valeurs = Vec::new();
    for o in module.options {
        let v = db::company::get_const(&app.pool, o.cle)?
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| o.defaut.to_string());
        valeurs.push(v);
    }

    Ok(crate::ui::page(
        "parametres",
        "modules",
        module.nom,
        html! {
            (message(q.get("ok")))
            h1 { "Options du module " (module.nom) }
            p .sous-titre {
                "Version " (module.version) " — constante d'activation "
                code { "MAIN_MODULE_" (module.code.to_uppercase()) }
                @if actif {(crate::langue::tr(" (activé)")) } @else { " (inactif)" }
            }

            @if module.options.is_empty() {
                (crate::ui::vide("Ce module n'a pas d'option réglable.", Some(("/parametres/modules", "Retour aux modules"))))
            } @else {
                form method="post" action={ "/parametres/module/" (module.code) } {
                    table {
                        tbody {
                            @for (i, o) in module.options.iter().enumerate() {
                                tr {
                                    th { (o.libelle) }
                                    td {
                                        @if o.genre == crate::plugin::Genre::OuiNon {
                                            select name=(o.cle) {
                                                option value="0" selected[valeurs[i] != "1"] {(crate::langue::tr("Non")) }
                                                option value="1" selected[valeurs[i] == "1"] {(crate::langue::tr("Oui")) }
                                            }
                                        } @else if o.genre == crate::plugin::Genre::Nombre {
                                            input type="number" name=(o.cle) value=(valeurs[i]).champ-moyen;
                                        } @else {
                                            input type="text" name=(o.cle) value=(valeurs[i]) class="pleine";
                                        }
                                        p style="color:#78736c;font-size:12px;margin:6px 0 0" {
                                            (o.aide) " — constante " code { (o.cle) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div .actions {
                        button type="submit" {(crate::langue::tr("Enregistrer les options")) }
                        a .bouton.secondaire href="/parametres/modules" {(crate::langue::tr("Retour aux modules")) }
                    }
                }
            }

            h2 {(crate::langue::tr("Ecrans fournis")) }
            @if module.entrees.is_empty() {
                p {(crate::langue::tr("Aucun écran déclaré.")) }
            } @else {
                ul {
                    @for e in module.entrees {
                        li { a href=(e.route) { (e.libelle) } }
                    }
                }
            }
        },
    ))
}

pub async fn options_module_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path(code): axum::extract::Path<String>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let module =
        crate::plugin::par_code(&code).ok_or_else(|| Erreur(format!("module inconnu : {code}")))?;
    for o in module.options {
        let valeur = f.get(o.cle).cloned().unwrap_or_default();
        db::company::set_const(&app.pool, o.cle, valeur.trim())?;
    }
    Ok(Redirect::to(&format!(
        "/parametres/module/{code}?ok=options"
    )))
}
