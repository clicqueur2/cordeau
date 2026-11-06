//! Ecran des modeles d'e-mail : les messages pre-ecrits, par type d'objet.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Form;
use db::modeles::{self, TYPES_VENTE};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La page : les modeles existants, et de quoi en creer un.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let listes = modeles::tous(&app.pool, entity)?;
    let ok = q.get("ok").map(|s| s.as_str());

    Ok(ui::page(
        "parametres",
        "modeles-emails",
        "Modèles d'e-mail",
        html! {
            h1 { (crate::langue::tr("Modèles d'e-mail")) }
            p .sous-titre {
                (crate::langue::tr("Un modèle porte un objet et un corps. Les variables __REF__ ou __TOTAL_TTC__ sont remplacées à l'envoi ; une variable inconnue reste visible plutôt que de laisser un trou dans le message."))
            }

            @if ok == Some("cree") { (ui::flash("Modèle créé.")) }
            @if ok == Some("bascule") { (ui::flash("Modèle mis à jour.")) }

            @if listes.is_empty() {
                (ui::vide("Aucun modèle en base. Créez-en un ci-dessous.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Libellé")) }
                            th { (crate::langue::tr("Type")) }
                            th { (crate::langue::tr("Objet du message")) }
                            th { "État" }
                            th { "" }
                        }
                    }
                    tbody {
                        @for m in &listes {
                            tr {
                                td { (m.libelle.clone()) }
                                td .ref { (m.type_template.clone()) }
                                td class="muet" { (m.sujet.clone()) }
                                td {
                                    @if m.actif {
                                        span .pastille.p-payee { "actif" }
                                    } @else {
                                        span .pastille.p-brouillon { "inactif" }
                                    }
                                    @if m.defaut { span .pastille.p-validee { "défaut" } }
                                }
                                td {
                                    form method="post" action="/parametres/modeles-emails/basculer" {
                                        input type="hidden" name="rowid" value=(m.rowid);
                                        input type="hidden" name="actif" value=(if m.actif { "0" } else { "1" });
                                        button type="submit" class="petit secondaire" {
                                            (if m.actif { "Désactiver" } else { "Activer" })
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Créer un modèle")) }
            form method="post" action="/parametres/modeles-emails/creer" {
                table {
                    tbody {
                        tr {
                            th { (crate::langue::tr("Type")) }
                            td {
                                select name="type_template" required {
                                    @for (code, libelle) in TYPES_VENTE {
                                        option value=(*code) { (crate::langue::tr(libelle)) }
                                    }
                                }
                            }
                            th { (crate::langue::tr("Libellé")) }
                            td { input type="text" name="libelle" size="26" required; }
                        }
                        tr {
                            th { (crate::langue::tr("Objet")) }
                            td colspan="3" {
                                input type="text" name="sujet" class="pleine"
                                      value="Votre facture __REF__";
                            }
                        }
                        tr {
                            th { (crate::langue::tr("Message")) }
                            td colspan="3" {
                                textarea name="corps" rows="7" class="pleine" {
                                    "Bonjour,\n\nVeuillez trouver ci-joint la facture __REF__ d'un montant de __TOTAL_TTC__ €.\n\nBien cordialement,\n"
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" { (crate::langue::tr("Créer le modèle")) }
                    span class="aide" {
                        (crate::langue::tr("Variables disponibles : __REF__, __TOTAL_TTC__, __TOTAL_HT__, __CLIENT__, __ECHEANCE__, __SOCIETE__."))
                    }
                }
            }
        },
    ))
}

/// Cree un modele.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let type_template = f
        .get("type_template")
        .map(|s| s.trim().to_string())
        .ok_or_else(|| Erreur("type de modèle manquant".into()))?;
    let libelle = f.get("libelle").map(|s| s.trim()).unwrap_or("");
    let sujet = f.get("sujet").map(|s| s.as_str()).unwrap_or("");
    let corps = f.get("corps").map(|s| s.as_str()).unwrap_or("");

    modeles::creer(
        &app.pool,
        entite::courante(),
        &type_template,
        libelle,
        sujet,
        corps,
        false,
    )?;
    Ok(Redirect::to("/parametres/modeles-emails?ok=cree"))
}

/// Active ou desactive.
pub async fn basculer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let rowid = f
        .get("rowid")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("modèle manquant".into()))?;
    let actif = f.get("actif").map(|s| s == "1").unwrap_or(false);

    modeles::basculer(&app.pool, entite::courante(), rowid, actif)?;
    Ok(Redirect::to("/parametres/modeles-emails?ok=bascule"))
}
