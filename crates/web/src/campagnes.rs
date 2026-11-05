//! Campagnes d'e-mailing : composition du message, choix de la cible, essai.
//!
//! L'ecran ne simule rien : il compte les tiers qui ont vraiment une adresse et
//! le bouton d'envoi envoie vraiment un message. Mais il n'envoie qu'au seul
//! destinataire de test. Une campagne complete demande de tracer chaque envoi
//! dans `llx_mailing` (une ligne par mailing, `statut` 2 = envoye) ; tant que
//! cette table n'est pas ecrite ici, un envoi en masse produirait des courriers
//! sans trace, impossibles a relancer comme a auditer. Un essai honnete vaut
//! mieux qu'une campagne a moitie tracee.
//!
//! Les cibles suivent `llx_societe.client` et `llx_societe.fournisseur`
//! (tinyint) : 0 = ni client ni prospect, 1 = client, 2 = prospect, 3 = client
//! et prospect. La correspondance ci-dessous reprend celle de
//! `db::divers::tiers_filtres`, qui est la reference du depot pour filtrer les
//! tiers ; elle n'est jamais construite a partir d'une saisie.

use std::collections::{BTreeSet, HashMap};

use axum::extract::{Query, State};
use axum::response::Redirect;
use maud::{html, Markup};

use crate::{Erreur, Res};

/// Les cibles proposees, dans l'ordre d'affichage : `(cle, libelle)`.
const CIBLES: [(&str, &str); 3] = [
    ("clients", "Clients"),
    ("prospects", "Prospects"),
    ("fournisseurs", "Fournisseurs"),
];

/// La cle vient d'une liste fermee : une valeur inconnue retombe sur `clients`.
fn cible_valide(cle: &str) -> bool {
    CIBLES.iter().any(|(c, _)| *c == cle)
}

/// Ecran de composition : cibles, nombre de destinataires, message, essai.
///
/// La cible affichee est celle du nombre mis en avant : le serveur ne rend pas
/// de JavaScript, donc le compteur ne peut pas suivre un menu deroulant. On
/// donne les trois comptes dans un tableau et `?cible=` choisit celui qu'on
/// met en avant et qui part preselectionne dans le formulaire.
pub async fn page(
    State(app): State<crate::App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let entity = crate::entite::courante();
    let cible = q
        .get("cible")
        .map(String::as_str)
        .filter(|c| cible_valide(c))
        .unwrap_or("clients");

    // Les trois comptes sont calcules a chaque affichage : une adresse ajoutee
    // dans la fiche tiers se voit immediatement, sans cache a invalider.
    let mut comptes: Vec<(&str, &str, usize)> = Vec::with_capacity(CIBLES.len());
    for (cle, libelle) in CIBLES {
        comptes.push((cle, libelle, destinataires(&app.pool, entity, cle)?.len()));
    }
    let n_cible = comptes
        .iter()
        .find(|(cle, _, _)| *cle == cible)
        .map(|(_, _, n)| *n)
        .unwrap_or(0);

    // L'adresse d'essai revient dans le formulaire apres un envoi ou une erreur :
    // on ne fait pas retaper l'adresse a chaque tentative.
    let test = q.get("test").cloned().unwrap_or_default();

    Ok(crate::ui::page(
        "communication",
        "emailings",
        "Campagnes",
        html! {
            h1 {(crate::langue::tr("Campagnes d'e-mailing")) }
            p .sous-titre {(crate::langue::tr("Composition du message, choix de la cible, puis essai avant tout envoi."))
            }

            @if q.get("ok").map(String::as_str) == Some("essai") {
                (crate::ui::flash(&format!(
                    "Essai envoye. La campagne vers {n_cible} destinataire(s) n'est pas lancee : \
                     cet ecran n'envoie pour l'instant qu'un message de controle."
                )))
            }
            @if let Some(e) = q.get("erreur") {
                p .erreur { (e) }
            }

            h2 {(crate::langue::tr("Destinataires")) }
            table {
                thead {
                    tr {
                        th {(crate::langue::tr("Cible")) }
                        th .num {(crate::langue::tr("Adresses")) }
                        th { "" }
                    }
                }
                tbody {
                    @for (cle, libelle, n) in comptes.iter().copied() {
                        tr {
                            td { (libelle) }
                            td .num { (n.to_string()) }
                            td {
                                @if cle == cible {
                                    span .pastille.p-validee { "choisie" }
                                } @else {
                                    a href=(format!("/campagnes?cible={cle}")) {(crate::langue::tr("Choisir")) }
                                }
                            }
                        }
                    }
                }
            }
            p style="color:#78736c;font-size:12.5px;margin:8px 0 0" {
                "Seuls les tiers de la societe courante ayant une adresse e-mail sont comptes, \
                 une fois par adresse."
            }

            h2 {(crate::langue::tr("Message")) }
            form method="post" action="/campagnes/envoyer" {
                table {
                    tbody {
                        tr {
                            th {(crate::langue::tr("Cible")) }
                            td {
                                select name="cible" {
                                    @for (cle, libelle) in CIBLES {
                                        option value=(cle) selected[cle == cible] { (libelle) }
                                    }
                                }
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Sujet")) }
                            td { input type="text" name="sujet" required style="width:100%"; }
                        }
                        tr {
                            th {(crate::langue::tr("Message")) }
                            td {
                                textarea name="corps" rows="12" required
                                    style="width:100%;font:inherit;padding:8px 10px;border:1px solid #e6e2dc" {}
                            }
                        }
                        tr {
                            th {(crate::langue::tr("Destinataire de test")) }
                            td {
                                input type="email" name="test" required value=(test)
                                      style="width:100%";
                                p style="color:#78736c;font-size:12.5px;margin:6px 0 0" {
                                    "Le message part a cette seule adresse. La liste complete \
                                     n'est pas sollicitee tant que l'envoi n'est pas trace."
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Envoyer l'essai")) }
                    span style="color:#78736c;font-size:13px" {
                        (n_cible) " destinataire(s) dans la cible choisie"
                    }
                }
            }
        },
    ))
}

/// Envoie le message au seul destinataire de test.
///
/// C'est le pendant de `pages::email_essai` : les constantes SMTP sont lues de
/// la meme facon, la variable d'environnement `DOLIBARR_SMTP_PASSWORD` l'emporte
/// sur la base, et l'appel reseau est bloquant donc execute hors du fil
/// asynchrone. Une erreur SMTP remonte son code et sa phrase tels quels.
pub async fn envoyer(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let entity = crate::entite::courante();

    let champ = |cle: &str| {
        f.get(cle)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    // La cible ne sert ici qu'a verifier qu'il y a bien une liste a viser : on
    // n'ecrit pas encore dans `llx_mailing`, donc on ne peut pas pretendre
    // envoyer a tout le monde sans laisser de trace.
    let cible = match champ("cible") {
        Some(c) if cible_valide(&c) => c,
        _ => "clients".to_string(),
    };
    let test = champ("test").ok_or_else(|| Erreur("destinataire de test obligatoire".into()))?;
    let sujet = champ("sujet").unwrap_or_else(|| "Message de Pierre Angulaire".to_string());
    let corps = champ("corps").ok_or_else(|| Erreur("le message est vide".into()))?;

    // L'adresse de test est gardee dans l'URL : apres une erreur, le formulaire
    // revient rempli et on ne retape pas tout.
    let retour = |parametre: &str, message: &str| {
        Redirect::to(&format!(
            "/campagnes?{parametre}={}&cible={}&test={}",
            encode(message),
            encode(&cible),
            encode(&test)
        ))
    };

    if !test.contains('@') || test.starts_with('@') || test.ends_with('@') {
        return Ok(retour("erreur", "adresse de test invalide"));
    }
    if destinataires(&app.pool, entity, &cible)?.is_empty() {
        return Ok(retour(
            "erreur",
            "aucun tiers de cette cible n'a d'adresse e-mail",
        ));
    }

    // La configuration SMTP est construite au meme endroit que pour l'envoi
    // d'une facture : deux copies d'un reglage finissent par diverger.
    let config = match crate::courriel::config(&app.pool) {
        Ok(c) => c,
        Err(motif) => return Ok(retour("erreur", &motif)),
    };

    let message = mail::Message::texte(&config.expediteur, &[test.as_str()], &sujet, &corps);

    let resultat = tokio::task::spawn_blocking(move || mail::envoyer(&config, &message)).await;

    match resultat {
        Ok(Ok(())) => Ok(Redirect::to(&format!(
            "/campagnes?ok=essai&cible={}&test={}",
            encode(&cible),
            encode(&test)
        ))),
        Ok(Err(e)) => Ok(retour("erreur", &e.to_string())),
        Err(e) => Ok(retour("erreur", &format!("tache interrompue : {e}"))),
    }
}

/// Adresses e-mail non vides des tiers de l'entite, filtrees par cible.
///
/// La condition SQL est choisie dans une liste fermee (jamais composee a partir
/// du texte saisi) et reprend `db::divers::tiers_filtres`. Les adresses sont
/// ramenees en minuscules et dedoublonnees : deux fiches peuvent porter la meme
/// adresse (un contact repris en doublon), et une campagne ne doit pas envoyer
/// deux fois au meme endroit. L'ordre est alphabetique, donc stable d'un
/// affichage a l'autre.
pub fn destinataires(
    pool: &db::Pool,
    entity: i64,
    cible: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let condition = match cible {
        "clients" => "client IN (1, 3)",
        "prospects" => "client IN (2, 3)",
        "fournisseurs" => "fournisseur = 1",
        autre => return Err(format!("cible inconnue : {autre}").into()),
    };

    // Aucun plafond : le compte affiche doit etre le vrai, sans quoi la campagne
    // annoncerait un nombre de destinataires faux.
    let tiers = db::party::list_filtre(pool, entity, condition, u32::MAX)?;

    let mut vues = BTreeSet::new();
    let mut adresses = Vec::new();
    for tiers in tiers {
        let Some(mail) = tiers.email else { continue };
        let mail = mail.trim().to_lowercase();
        if mail.is_empty() || !mail.contains('@') {
            continue;
        }
        if vues.insert(mail.clone()) {
            adresses.push(mail);
        }
    }
    Ok(adresses)
}

/// Encode une valeur pour un parametre d'URL, comme `pages::encode` (prive la-bas,
/// donc recopie ici a l'identique).
fn encode(s: &str) -> String {
    let mut sortie = String::new();
    for octet in s.bytes() {
        match octet {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                sortie.push(octet as char)
            }
            b' ' => sortie.push('+'),
            autre => sortie.push_str(&format!("%{autre:02X}")),
        }
    }
    sortie
}
