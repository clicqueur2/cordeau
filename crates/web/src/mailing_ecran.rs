//! Ecran des mailings : les campagnes d'e-mailing.
//!
//! Dolibarr a `admin/mailing.php`. Une campagne se prepare, se valide, s'approuve,
//! puis part. Cet ecran s'arrete a l'avant-derniere etape : il **n'envoie rien**.
//!
//! # Pourquoi « marquer envoyé » et non « envoyer »
//!
//! Un envoi en masse est une operation qu'on ne defait pas : une faute de
//! destinataire ne se corrige pas apres coup. Le noyau enregistre donc qu'un
//! envoi **a eu lieu** — par le binaire de courriel, par un outil dedie, ou a la
//! main — sans jamais le declencher depuis un ecran. Le nom de la fonction dit
//! ce qu'elle fait ; « envoyer » serait plus flatteur et faux.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::mailing::{self, NewMailing};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La liste et le formulaire.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let campagnes = mailing::list(&app.pool, entity, 200, 0)?;
    let pretes = campagnes
        .iter()
        .filter(|c| c.statut == mailing::STATUT_APPROUVEE)
        .count();

    Ok(ui::page(
        "mailings",
        "mailings",
        "Mailings",
        html! {
            h1 { (crate::langue::tr("Mailings")) }
            p .sous-titre {
                (crate::langue::tr("Les campagnes d'e-mailing : préparation, validation, approbation. L'envoi lui-même ne part pas d'ici."))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Campagnes"),
                    &campagnes.len().to_string(),
                    Some(&crate::langue::tr("toutes périodes")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Prêtes à partir"),
                    &pretes.to_string(),
                    Some(&crate::langue::tr("approuvées, pas encore envoyées")),
                    pretes > 0,
                ))
            }

            h2 { (crate::langue::tr("Nouvelle campagne")) }
            form method="post" action="/mailings" {
                label for="titre" { (crate::langue::tr("Titre")) } " "
                input type="text" name="titre" id="titre" required style="min-width:240px"
                      placeholder=(crate::langue::tr("Nom interne de la campagne"));
                br;
                label for="sujet" { (crate::langue::tr("Objet")) } " "
                input type="text" name="sujet" id="sujet" required style="min-width:320px";
                br;
                label for="email_from" { (crate::langue::tr("Expéditeur")) } " "
                input type="email" name="email_from" id="email_from" required style="width:240px";
                " "
                label for="cible" { (crate::langue::tr("Cible")) } " "
                input type="text" name="cible" id="cible" style="width:200px"
                      placeholder=(crate::langue::tr("clients, prospects…"));
                br;
                label for="body" { (crate::langue::tr("Message")) }
                textarea name="body" id="body" rows="5" required
                         style="width:100%;font-family:ui-monospace,monospace;font-size:13px" {}
                div .actions {
                    button type="submit" { (crate::langue::tr("Créer le brouillon")) }
                }
                p style="color:#78736c;font-size:13px;margin:0" {
                    (crate::langue::tr("Une campagne née en brouillon ne part jamais toute seule : il faut la valider, puis l'approuver."))
                }
            }

            h2 { (crate::langue::tr("Campagnes")) }
            @if campagnes.is_empty() {
                (ui::vide("Aucune campagne.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Titre")) }
                            th { (crate::langue::tr("Objet")) }
                            th { (crate::langue::tr("Cible")) }
                            th { (crate::langue::tr("Destinataires")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for c in &campagnes {
                            tr {
                                td { a href=(format!("/mailings/{}", c.rowid)) { (c.titre.clone()) } }
                                td { (c.sujet.clone()) }
                                td { (c.cible.clone().unwrap_or_else(|| "—".to_string())) }
                                td .num { (c.nbemail) }
                                td { (pastille(c.statut)) }
                                td {
                                    a href=(format!("/mailings/{}", c.rowid)) class="petit secondaire" {
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

/// La fiche d'une campagne.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let c = mailing::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("campagne {id} introuvable")))?;
    // Le nombre stocke peut mentir : on recompte pour l'afficher.
    let reels = mailing::compter_destinataires(&app.pool, entity, id).unwrap_or(0);

    Ok(ui::page(
        "mailings",
        "mailings",
        &c.titre,
        html! {
            h1 { (c.titre.clone()) }
            p .sous-titre {
                (c.sujet.clone())
                @if let Some(f) = &c.email_from {
                    " · " (crate::langue::tr("expédié par")) " " (f.clone())
                }
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Destinataires"),
                    &reels.to_string(),
                    Some(&crate::langue::tr("recomptés, pas le chiffre stocké")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("État"),
                    &libelle(c.statut),
                    None,
                    c.statut == mailing::STATUT_ENVOYEE,
                ))
            }

            @if let Some(corps) = c.body.clone().filter(|b| !b.trim().is_empty()) {
                h2 { (crate::langue::tr("Message")) }
                pre style="background:#fbfaf8;border:1px solid #e6e2dc;padding:14px;font-size:13px;white-space:pre-wrap;margin:0" {
                    (corps)
                }
            }

            h2 { (crate::langue::tr("Actions")) }
            @match c.statut {
                0 => {
                    form method="post" action=(format!("/mailings/{}/valider", c.rowid)) {
                        button type="submit" { (crate::langue::tr("Valider")) }
                    }
                }
                1 => {
                    form method="post" action=(format!("/mailings/{}/approuver", c.rowid)) {
                        button type="submit" { (crate::langue::tr("Approuver")) }
                    }
                }
                2 => {
                    form method="post" action=(format!("/mailings/{}/marquer-envoye", c.rowid)) {
                        button type="submit" { (crate::langue::tr("Marquer comme envoyée")) }
                    }
                    p style="color:#78736c;font-size:13.5px" {
                        (crate::langue::tr("Ce bouton n'envoie rien : il enregistre qu'un envoi a eu lieu. Un envoi en masse ne se déclenche pas depuis un écran — une faute de destinataire ne se corrige pas."))
                    }
                }
                _ => {
                    p style="color:#78736c;font-size:13.5px" {
                        (crate::langue::tr("Cette campagne est partie. Elle ne se modifie plus."))
                    }
                }
            }
        },
    ))
}

/// Cree un brouillon de campagne.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let new = NewMailing {
        titre: lire("titre"),
        sujet: lire("sujet"),
        body: lire("body"),
        email_from: lire("email_from"),
        name_from: None,
        email_replyto: None,
        messtype: mailing::MESSTYPE_EMAIL.to_string(),
        cible: Some(lire("cible")).filter(|s| !s.is_empty()),
        note_private: None,
        note_public: None,
    };
    let id = mailing::create(&app.pool, entite::courante(), &new)?;
    Ok(Redirect::to(&format!("/mailings/{id}")))
}

/// Valide une campagne.
pub async fn valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    mailing::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/mailings/{id}")))
}

/// Approuve une campagne.
pub async fn approuver(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    mailing::approuver(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/mailings/{id}")))
}

/// Enregistre qu'un envoi a eu lieu. N'envoie rien.
pub async fn marquer_envoye(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    mailing::marquer_envoye(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/mailings/{id}")))
}

/// Le libelle d'un statut.
fn libelle(statut: i16) -> String {
    match statut {
        mailing::STATUT_BROUILLON => "brouillon",
        mailing::STATUT_VALIDEE => "validée",
        mailing::STATUT_APPROUVEE => "approuvée",
        mailing::STATUT_ENVOYEE => "envoyée",
        _ => "inconnu",
    }
    .to_string()
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let classe = match statut {
        mailing::STATUT_ENVOYEE => "p-payee",
        mailing::STATUT_APPROUVEE => "p-partielle",
        _ => "p-brouillon",
    };
    html! { span class=(format!("pastille {classe}")) { (libelle(statut)) } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_statut_a_un_libelle_lisible() {
        assert_eq!(libelle(0), "brouillon");
        assert_eq!(libelle(3), "envoyée");
        // Un statut inconnu ne s'affiche pas comme un nombre.
        assert_eq!(libelle(42), "inconnu");
    }
}
