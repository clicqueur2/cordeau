//! Ecran des constantes : parcourir et modifier les reglages de Dolibarr.
//!
//! C'est l'ecran que Dolibarr appelle `admin/const.php`. Il manquait, et il
//! manquait beaucoup : sans lui, regler un serveur SMTP, un code VATEX ou un
//! format obligeait a passer par SQL.
//!
//! Une valeur sensible ne s'affiche pas. On peut la **remplacer** — c'est le
//! geste utile — mais pas la lire.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Form;
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La page : les constantes, filtrables par nom.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let filtre = q.get("q").map(|s| s.trim().to_string()).unwrap_or_default();
    let filtre_slice = if filtre.is_empty() {
        None
    } else {
        Some(filtre.as_str())
    };
    let listes = db::constantes::liste(&app.pool, entity, filtre_slice, 300)?;
    let ok = q.get("ok").map(|s| s.as_str());

    Ok(ui::page(
        "parametres",
        "constantes",
        "Constantes",
        html! {
            h1 { (crate::langue::tr("Constantes")) }
            p .sous-titre {
                (crate::langue::tr("Tous les réglages de Dolibarr sont ici, dans llx_const. Une valeur vide supprime la ligne : c'est ainsi qu'on revient au défaut."))
            }

            @if ok == Some("ecrite") { (ui::flash("Constante enregistrée.")) }
            @if ok == Some("supprimee") { (ui::flash("Constante supprimée : le défaut s'applique de nouveau.")) }

            form method="get" action="/parametres/constantes" {
                div .actions {
                    input type="search" name="q" value=(filtre) placeholder="Filtrer par nom (SMTP, MAIN_…)"
                         .champ-large aria-label=(crate::langue::tr("Filtrer"));
                    button type="submit" class="petit" { (crate::langue::tr("Filtrer")) }
                    @if !filtre.is_empty() {
                        a .bouton.secondaire.petit href="/parametres/constantes" { (crate::langue::tr("Tout voir")) }
                    }
                }
            }

            p .sous-titre {
                (listes.len().to_string()) " " (crate::langue::tr("constante(s)"))
                @if !filtre.is_empty() {
                    " " (crate::langue::tr("pour")) " « " (filtre.clone()) " »"
                }
            }

            @if listes.is_empty() {
                (ui::vide("Aucune constante ne correspond.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Nom")) }
                            th { (crate::langue::tr("Valeur")) }
                            th .num { (crate::langue::tr("Entité")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for c in &listes {
                            tr {
                                td .ref { (c.nom.clone()) }
                                td {
                                    form method="post" action="/parametres/constantes".rangee {
                                        input type="hidden" name="nom" value=(c.nom.clone());
                                        @if c.sensible {
                                            input type="password" name="valeur" value=""
                                                  placeholder=(db::constantes::valeur_affichable(&c.nom, &c.valeur))
                                                  class="pleine" autocomplete="off";
                                        } @else {
                                            input type="text" name="valeur" value=(c.valeur.clone())
                                                  class="pleine";
                                        }
                                        button type="submit" class="petit" { (crate::langue::tr("Écrire")) }
                                    }
                                }
                                td .num { (c.entite.to_string()) }
                                td {
                                    @if c.sensible {
                                        span .pastille.p-brouillon { "secret" }
                                    } @else if c.entite == 0 {
                                        span .pastille.p-validee { "global" }
                                    } @else {
                                        span .pastille.p-payee { "société" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Créer une constante")) }
            form method="post" action="/parametres/constantes" {
                div .actions {
                    input type="text" name="nom" placeholder="MA_CLE" size="28" required;
                    input type="text" name="valeur" placeholder=(crate::langue::tr("valeur")) size="34" required;
                    button type="submit" { (crate::langue::tr("Écrire")) }
                }
                p class="aide" {
                    (crate::langue::tr("Le nom accepte lettres, chiffres et souligné. Une valeur vide supprime la constante."))
                }
            }
        },
    ))
}

/// Ecrit une constante dans l'entite de la session.
pub async fn ecrire(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let nom = f.get("nom").map(|s| s.trim()).unwrap_or("");
    let valeur = f.get("valeur").cloned().unwrap_or_default();
    if nom.is_empty() {
        return Err(Erreur("nom de constante manquant".into()));
    }

    db::constantes::ecrire(&app.pool, entite::courante(), nom, &valeur)?;

    // Une valeur de mot de passe ne doit pas finir dans le journal.
    let retour = if valeur.trim().is_empty() {
        "supprimee"
    } else {
        "ecrite"
    };
    Ok(Redirect::to(&format!("/parametres/constantes?ok={retour}")))
}
