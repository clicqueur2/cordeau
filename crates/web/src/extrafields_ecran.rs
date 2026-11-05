//! Ecran des champs libres : les definitions, et leurs valeurs.
//!
//! Dolibarr ajoute une **colonne** a la table de l'objet pour chaque champ
//! libre (`options_<nom>`), et tient la definition dans `llx_extrafields`.
//! Jusqu'ici le noyau n'en montrait rien : les donnees existaient, saisies
//! dans Dolibarr, et restaient invisibles.
//!
//! Trois ecrans :
//!
//! - `/parametres/extrafields` : les objets qui portent des champs, et combien ;
//! - `/parametres/extrafields/:elementtype` : les champs de cet objet ;
//! - `/extrafields/:elementtype/:id` : les valeurs d'un objet, modifiables.
//!
//! Le formulaire emploie les noms de Dolibarr (`options_<nom>`) : ce qu'on poste
//! ici serait accepte par son `action=update`, et l'inverse est vrai.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use axum::Form;
use db::extrafields::{self, Definition};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// Les objets qui portent au moins un champ libre.
pub async fn liste(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let cibles = extrafields::cibles(&app.pool, entity)?;

    Ok(ui::page(
        "parametres",
        "extrafields",
        "Champs libres",
        html! {
            h1 {(crate::langue::tr("Champs libres")) }
            p .sous-titre {
                "Dolibarr ajoute une colonne a la table de l'objet pour chaque champ libre. "
                "Ces valeurs etaient dans la base, aucun ecran ne les montrait."
            }
            @if cibles.is_empty() {
                (ui::vide(
                    "Aucun champ libre n'est defini sur cette entite.",
                    None
                ))
                p style="color:#78736c;font-size:14px" {
                    "Les definitions se creent dans Dolibarr (Configuration, Champs libres) ; "
                    "elles apparaissent ici des qu'elles existent en base."
                }
            } @else {
                table {
                    thead {
                        tr { th {(crate::langue::tr("Objet")) } th {(crate::langue::tr("Table")) } th .num {(crate::langue::tr("Champs")) } }
                    }
                    tbody {
                        @for (elementtype, nombre) in &cibles {
                            tr {
                                td {
                                    a href={ "/parametres/extrafields/" (elementtype) } { (elementtype) }
                                }
                                td .ref { (extrafields::table_cible(elementtype)) }
                                td .num { (nombre.to_string()) }
                            }
                        }
                    }
                }
                p style="color:#78736c;font-size:13.5px;margin-top:14px" {
                    (extrafields::TYPES.len().to_string())
                    " types reconnus par le coeur de Dolibarr, de "
                    code { "varchar" } " a " code { "polygon" } "."
                }
            }
        },
    ))
}

/// Les champs d'un objet.
pub async fn cible(State(app): State<App>, Path(elementtype): Path<String>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let table = extrafields::table_cible(&elementtype);
    let presentes = extrafields::colonnes_presentes(&app.pool, &table).unwrap_or_default();
    let champs: Vec<Definition> = extrafields::definitions(&app.pool, entity)?
        .into_iter()
        .filter(|d| d.elementtype == elementtype)
        .collect();

    Ok(ui::page(
        "parametres",
        "extrafields",
        &format!("Champs libres : {elementtype}"),
        html! {
            h1 { "Champs libres : " (elementtype) }
            p .sous-titre { "Table " code { (table.clone()) } }

            @if champs.is_empty() {
                (ui::vide(
                    "Aucun champ pour cet objet.",
                    Some(("/parametres/extrafields", "Retour")),
                ))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Nom")) }
                            th {(crate::langue::tr("Libelle")) }
                            th {(crate::langue::tr("Type")) }
                            th {(crate::langue::tr("Colonne")) }
                            th {(crate::langue::tr("Etat")) }
                        }
                    }
                    tbody {
                        @for d in &champs {
                            tr {
                                td .ref { (d.nom) }
                                td { (d.libelle) }
                                td {
                                    (d.type_)
                                    @if let Some(t) = &d.taille {
                                        @if !t.trim().is_empty() {
                                            " (" (t) ")"
                                        }
                                    }
                                }
                                td .ref {
                                    (d.colonne())
                                    @if !presentes.contains(&d.colonne()) {
                                        span style="color:#a33a2a" {(crate::langue::tr(" — absente")) }
                                    }
                                }
                                td {
                                    @if !d.actif() {
                                        span .pastille.p-brouillon { "desactive" }
                                    } @else if d.calcule_par_expression() {
                                        span .pastille.p-validee { "calcule" }
                                    } @else {
                                        span .pastille.p-payee { "actif" }
                                    }
                                    @if d.requis { span .pastille.p-retard { "requis" } }
                                }
                            }
                        }
                    }
                }
                p style="color:#78736c;font-size:13.5px;margin-top:14px" {
                    "Pour ouvrir les valeurs d'un objet : "
                    code { "/extrafields/" (elementtype) "/<numero>" }
                    " — le numero est le " code { "rowid" } " de la ligne dans "
                    code { (table) } "."
                }
            }
        },
    ))
}

/// Les valeurs d'un objet, modifiables.
pub async fn objet(
    State(app): State<App>,
    Path((elementtype, id)): Path<(String, i64)>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let valeurs = extrafields::valeurs(&app.pool, &elementtype, entity, id)?;
    let ok = q.contains_key("ok");

    Ok(ui::page(
        "parametres",
        "extrafields",
        &format!("Champs libres : {elementtype} {id}"),
        html! {
            h1 {(crate::langue::tr("Champs libres")) }
            p .sous-titre { (elementtype) " n° " (id.to_string()) " · entite " (entity.to_string()) }

            @if ok {
                (ui::flash("Champs libres enregistres."))
            }

            @if valeurs.is_empty() {
                (ui::vide(
                    "Aucun champ libre actif pour cet objet.",
                    Some(("/parametres/extrafields", "Retour aux champs libres")),
                ))
            } @else {
                form method="post" action={ "/extrafields/" (elementtype) "/" (id.to_string()) } {
                    table {
                        thead {
                            tr { th {(crate::langue::tr("Champ")) } th {(crate::langue::tr("Valeur")) } th {(crate::langue::tr("Type")) } }
                        }
                        tbody {
                            @for (d, valeur, existe) in &valeurs {
                                tr {
                                    td {
                                        div { (d.libelle.clone()) }
                                        div .ref style="color:#78736c;font-size:12.5px" {
                                            (d.nom.clone())
                                            @if d.requis {(crate::langue::tr(" · requis")) }
                                        }
                                    }
                                    td {
                                        @if !*existe {
                                            span style="color:#a33a2a" {
                                                "colonne absente : " (d.colonne())
                                            }
                                        } @else if d.calcule_par_expression() {
                                            (valeur.clone().unwrap_or_else(|| "—".into()))
                                            div style="color:#78736c;font-size:12.5px" {(crate::langue::tr("calcule, non saisissable"))
                                            }
                                        } @else {
                                            (champ(d, valeur.as_deref()))
                                        }
                                    }
                                    td { (d.type_.clone()) }
                                }
                            }
                        }
                    }
                    div .actions {
                        button type="submit" {(crate::langue::tr("Enregistrer")) }
                        a .bouton.secondaire href={ "/parametres/extrafields/" (elementtype) } {(crate::langue::tr("Voir les definitions"))
                        }
                    }
                }
            }
        },
    ))
}

/// Enregistre les valeurs postees.
pub async fn enregistrer(
    State(app): State<App>,
    Path((elementtype, id)): Path<(String, i64)>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    // Dolibarr poste `options_<nom>` : on garde son vocabulaire.
    let saisies: Vec<(String, String)> = f
        .iter()
        .filter_map(|(k, v)| {
            k.strip_prefix("options_")
                .map(|nom| (nom.to_string(), v.clone()))
        })
        .collect();

    if saisies.is_empty() {
        return Err(Erreur("aucun champ a enregistrer".into()));
    }

    extrafields::enregistrer(&app.pool, &elementtype, entite::courante(), id, &saisies)?;
    Ok(Redirect::to(&format!(
        "/extrafields/{elementtype}/{id}?ok=1"
    )))
}

/// Le controle de saisie d'un champ, selon son type.
///
/// Un type qu'on ne sait pas rendre n'est pas devine : il reste un champ texte,
/// avec son type affiche a cote.
fn champ(d: &Definition, valeur: Option<&str>) -> maud::Markup {
    let nom = format!("options_{}", d.nom);
    let v = valeur.unwrap_or_default();
    let type_ = d.type_.as_str();

    match type_ {
        "text" | "html" => html! {
            textarea name=(nom) rows="4" style="width:100%" { (v) }
        },
        "boolean" => html! {
            input type="checkbox" name=(nom) value="1" checked[v == "1" || v.eq_ignore_ascii_case("true")];
        },
        "select" | "radio" => {
            let options = d.options();
            if options.is_empty() {
                html! { input type="text" name=(nom) value=(v); }
            } else {
                html! {
                    select name=(nom) {
                        option value="" { "—" }
                        @for (cle, libelle) in &options {
                            option value=(cle.as_str()) selected[*cle == v] { (libelle.as_str()) }
                        }
                    }
                }
            }
        }
        "date" | "datetime" | "datetimegmt" => {
            // MariaDB rend `AAAA-MM-JJ HH:MM:SS` : l'input veut `AAAA-MM-JJ`.
            let court: String = v.chars().take(10).collect();
            html! { input type="date" name=(nom) value=(court); }
        }
        "int" | "double" | "price" | "duration" | "stars" | "link" => html! {
            input type="number" name=(nom) value=(v) step="any";
        },
        "password" => html! {
            input type="password" name=(nom) value=(v) autocomplete="off";
        },
        "email" | "mail" => html! {
            input type="email" name=(nom) value=(v);
        },
        "url" => html! {
            input type="url" name=(nom) value=(v);
        },
        "phone" => html! {
            input type="tel" name=(nom) value=(v);
        },
        _ => {
            let taille = d.taille.clone().unwrap_or_default();
            html! {
                input type="text" name=(nom) value=(v) maxlength=(taille);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(type_: &str, param: Option<&str>, taille: Option<&str>) -> Definition {
        Definition {
            rowid: 1,
            nom: "mon_champ".into(),
            elementtype: "customer".into(),
            libelle: "Mon champ".into(),
            type_: type_.into(),
            taille: taille.map(str::to_string),
            defaut: None,
            requis: false,
            enabled: Some("1".into()),
            pos: 0,
            param: param.map(str::to_string),
            liste: None,
            aide: None,
            calcule: None,
        }
    }

    /// Une liste choisie se rend en `select`, avec l'option courante marquee.
    #[test]
    fn une_liste_se_rend_en_select_avec_la_valeur_courante() {
        let html = champ(
            &def("select", Some(r#"{"r":"Rouge","b":"Bleu"}"#), None),
            Some("r"),
        )
        .into_string();
        assert!(html.contains("<select"), "un select est attendu : {html}");
        assert!(html.contains(r#"name="options_mon_champ""#), "{html}");
        assert!(html.contains("Rouge") && html.contains("Bleu"), "{html}");
        assert!(
            html.contains(r#"value="r" selected"#),
            "l'option courante doit etre selectionnee : {html}"
        );
        // Le vocabulaire de Dolibarr est conserve.
        assert!(html.contains(r#"options_mon_champ"#));
    }

    /// Une liste sans options lisibles reste un champ texte, pas une liste vide.
    #[test]
    fn une_liste_sans_options_reste_un_champ_texte() {
        let html = champ(&def("select", None, None), Some("r")).into_string();
        assert!(!html.contains("<select"), "{html}");
        assert!(html.contains(r#"type="text""#), "{html}");
    }

    #[test]
    fn un_texte_long_se_rend_en_zone_de_saisie() {
        let html = champ(&def("text", None, None), Some("mur nord")).into_string();
        assert!(html.contains("<textarea"), "{html}");
        assert!(html.contains("mur nord"), "{html}");
    }

    #[test]
    fn un_booleen_est_une_case_a_cocher_cochee_si_vrai() {
        let html = champ(&def("boolean", None, None), Some("1")).into_string();
        assert!(html.contains(r#"type="checkbox""#), "{html}");
        assert!(html.contains("checked"), "{html}");

        let html = champ(&def("boolean", None, None), Some("0")).into_string();
        assert!(!html.contains("checked"), "{html}");
    }

    /// MariaDB rend `AAAA-MM-JJ HH:MM:SS` : l'input de type date veut la date
    /// seule, sinon le navigateur affiche un champ vide.
    #[test]
    fn une_date_est_tronquee_au_jour() {
        let html = champ(&def("date", None, None), Some("2026-11-04 17:32:00")).into_string();
        assert!(html.contains(r#"value="2026-11-04""#), "{html}");
        assert!(!html.contains("17:32"), "{html}");
    }

    #[test]
    fn un_nombre_garde_le_point_decimal() {
        let html = champ(&def("price", None, None), Some("1234.56")).into_string();
        assert!(html.contains(r#"type="number""#), "{html}");
        assert!(html.contains(r#"value="1234.56""#), "{html}");
        assert!(html.contains(r#"step="any""#), "{html}");
    }

    #[test]
    fn un_type_inconnu_reste_un_champ_texte_borne() {
        let html = champ(&def("polygon", None, Some("32")), Some("POINT(1 2)")).into_string();
        assert!(html.contains(r#"type="text""#), "{html}");
        assert!(html.contains(r#"maxlength="32""#), "{html}");
    }
}
