//! Ecran des partenariats : une relation avec un tiers ou un adherent.
//!
//! Dolibarr a `partnership.php`. Le cycle est explicite : brouillon, soumis,
//! accepte, refuse, annule. Chaque etape refuse de se produire hors sequence,
//! parce qu'un partenariat accepte qu'on annule apres coup n'a jamais ete un
//! partenariat accepte.
//!
//! # Le statut annule vaut 9, pas 4
//!
//! C'est la valeur de `Partnership::STATUS_CANCELED` dans la source Dolibarr.
//! Une fiche a 4 n'aurait aucun libelle dans `LibStatut()` et s'afficherait
//! comme un statut inconnu.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::partenariat::{self, NouveauPartenariat};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La page.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let partenariats = partenariat::list(&app.pool, entity, 200, 0)?;
    let types = partenariat::types(&app.pool).unwrap_or_default();
    let en_cours = partenariat::en_cours(&app.pool, entity).unwrap_or(0);
    let clients = db::party::list(&app.pool, entity, 500, 0)?;

    Ok(ui::page(
        "partenariats",
        "partenariats",
        "Partenariats",
        html! {
            h1 { (crate::langue::tr("Partenariats")) }
            p .sous-titre {
                (crate::langue::tr("Une relation avec un tiers ou un adhérent : école, fournisseur, association. Elle se soumet, s'accepte, et se refuse avec un motif."))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("En cours"),
                    &en_cours.to_string(),
                    Some(&crate::langue::tr("acceptés et non échus")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Total"),
                    &partenariats.len().to_string(),
                    Some(&crate::langue::tr("toutes situations")),
                    false,
                ))
            }

            h2 { (crate::langue::tr("Nouveau partenariat")) }
            @if types.is_empty() {
                (ui::vide(
                    "Aucun type de partenariat déclaré. Dolibarr n'en sème aucun : il faut les créer dans le dictionnaire avant de nouer un partenariat.",
                    None,
                ))
            } @else if clients.is_empty() {
                (ui::vide(
                    "Aucun tiers enregistré : un partenariat se noue avec quelqu'un.",
                    Some(("/tiers", "Voir les tiers")),
                ))
            } @else {
                form method="post" action="/partenariats" {
                    label for="ref" { (crate::langue::tr("Référence")) } " "
                    input type="text" name="ref" id="ref" required style="width:200px"
                          placeholder=(crate::langue::tr("partenariat-ecole-2026"));
                    " "
                    label for="fk_type" { (crate::langue::tr("Type")) } " "
                    select name="fk_type" id="fk_type" {
                        @for (id, libelle) in &types {
                            option value=(id) { (libelle.clone()) }
                        }
                    }
                    br;
                    label for="fk_soc" { (crate::langue::tr("Tiers")) } " "
                    select name="fk_soc" id="fk_soc" style="min-width:280px" {
                        @for c in &clients {
                            option value=(c.rowid) { (c.nom.clone()) }
                        }
                    }
                    " "
                    label for="date_partnership_start" { (crate::langue::tr("Début")) } " "
                    input type="date" name="date_partnership_start" id="date_partnership_start" required;
                    " "
                    label for="date_partnership_end" { (crate::langue::tr("Fin")) } " "
                    input type="date" name="date_partnership_end" id="date_partnership_end";
                    " "
                    button type="submit" { (crate::langue::tr("Créer")) }
                }
                p style="color:#78736c;font-size:13px;margin:8px 0 0" {
                    (crate::langue::tr("Un partenariat se crée en brouillon. Il ne devient vivant qu'après soumission puis acceptation."))
                }
            }

            h2 { (crate::langue::tr("Partenariats")) }
            @if partenariats.is_empty() {
                (ui::vide("Aucun partenariat.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Type")) }
                            th { (crate::langue::tr("Tiers")) }
                            th { (crate::langue::tr("Période")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for p in &partenariats {
                            tr {
                                td .ref { (p.ref_.clone()) }
                                td {
                                    (p.type_libelle.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td {
                                    (p.tiers_nom.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td .ref {
                                    (p.date_debut.clone().unwrap_or_else(|| "—".to_string()))
                                    @if let Some(f) = &p.date_fin {
                                        " → " (f.clone())
                                    }
                                }
                                td { (pastille(p.statut)) }
                                td { (actions(p)) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Les boutons disponibles selon l'etat.
fn actions(p: &partenariat::Partenariat) -> maud::Markup {
    html! {
        @if p.est_brouillon() {
            form method="post" action=(format!("/partenariats/{}/soumettre", p.rowid)) style="display:inline" {
                button type="submit" class="petit" { (crate::langue::tr("Soumettre")) }
            }
        } @else if p.est_valide() {
            form method="post" action=(format!("/partenariats/{}/accepter", p.rowid)) style="display:inline" {
                button type="submit" class="petit" { (crate::langue::tr("Accepter")) }
            }
            " "
            form method="post" action=(format!("/partenariats/{}/refuser", p.rowid)) style="display:inline" {
                input type="text" name="motif" required placeholder=(crate::langue::tr("motif"))
                      style="width:130px";
                " "
                button type="submit" class="petit secondaire" {
                    (crate::langue::tr("Refuser"))
                }
            }
        } @else if p.est_accepte() {
            form method="post" action=(format!("/partenariats/{}/annuler", p.rowid)) style="display:inline" {
                input type="text" name="motif" required placeholder=(crate::langue::tr("motif"))
                      style="width:130px";
                " "
                button type="submit" class="petit secondaire" {
                    (crate::langue::tr("Annuler"))
                }
            }
        }
    }
}

/// Cree un partenariat.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();
    let option = |cle: &str| Some(lire(cle)).filter(|s| !s.is_empty());

    let login = crate::utilisateur::courant()
        .ok_or_else(|| Erreur("aucun utilisateur en session".into()))?;
    let entity = entite::courante();
    let auteur = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;

    let new = NouveauPartenariat {
        ref_: lire("ref"),
        fk_type: lire("fk_type")
            .parse::<i64>()
            .map_err(|_| Erreur("type de partenariat manquant".into()))?,
        // La liste ne propose que des tiers existants : le `0` ne doit pas
        // arriver ici, mais s'il arrive, le module le refuse.
        fk_soc: lire("fk_soc").parse::<i64>().ok(),
        fk_member: None,
        email_partnership: None,
        date_partnership_start: lire("date_partnership_start"),
        date_partnership_end: option("date_partnership_end"),
        url_to_check: None,
        fk_user_creat: auteur,
    };
    partenariat::creer(&app.pool, entity, &new)?;
    Ok(Redirect::to("/partenariats"))
}

/// Soumet un partenariat.
pub async fn soumettre(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    partenariat::soumettre(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/partenariats"))
}

/// Accepte un partenariat.
pub async fn accepter(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    partenariat::accepter(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to("/partenariats"))
}

/// Refuse un partenariat, avec un motif.
pub async fn refuser(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let motif = motif_obligatoire(&f)?;
    partenariat::refuser(&app.pool, entite::courante(), id, &motif)?;
    Ok(Redirect::to("/partenariats"))
}

/// Annule un partenariat, avec un motif.
pub async fn annuler(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let motif = motif_obligatoire(&f)?;
    partenariat::annuler(&app.pool, entite::courante(), id, &motif)?;
    Ok(Redirect::to("/partenariats"))
}

/// Le motif, exige : un refus ou une annulation sans raison ne s'explique plus
/// six mois plus tard.
fn motif_obligatoire(f: &HashMap<String, String>) -> Result<String, Erreur> {
    f.get("motif")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("un refus ou une annulation doit être motivé".into()))
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let (classe, texte) = match statut {
        partenariat::STATUT_ACCEPTE => ("p-payee", "accepté"),
        partenariat::STATUT_VALIDE => ("p-partielle", "soumis"),
        partenariat::STATUT_REFUSE => ("p-impayee", "refusé"),
        partenariat::STATUT_ANNULE => ("p-brouillon", "annulé"),
        _ => ("p-brouillon", "brouillon"),
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}
