//! Actions de masse : valider quarante factures en un geste.
//!
//! Le formulaire HTML ne sait pas poster un tableau, et un `HashMap` ne garde
//! que la derniere valeur d'un nom repete. Chaque case porte donc son propre
//! nom, `id_<rowid>` — c'est le meme procede que les champs libres, et il ne
//! demande aucun JavaScript.
//!
//! Une action de masse ne s'arrete pas au premier refus : elle **essaie
//! chaque ligne** et rend un bilan. Blocage a la premiere erreur, c'est
//! l'utilisateur qui ne sait pas ce qui est passe et ce qui ne l'est pas.

use std::collections::HashMap;

use axum::extract::State;
use axum::Form;
use maud::{html, Markup};

use crate::{entite, ui, App, Erreur, Res};

/// Les identifiants coches, tries et sans doublon.
pub fn ids(f: &HashMap<String, String>) -> Vec<i64> {
    let mut v: Vec<i64> = f
        .keys()
        .filter_map(|k| k.strip_prefix("id_"))
        .filter_map(|s| s.parse::<i64>().ok())
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Ce qu'une action de masse a donne.
pub struct Bilan {
    pub faits: usize,
    pub refuses: Vec<(i64, String)>,
}

impl Bilan {
    fn nouveau() -> Self {
        Self {
            faits: 0,
            refuses: Vec::new(),
        }
    }

    fn ajouter(&mut self, id: i64, r: Result<(), Box<dyn std::error::Error>>) {
        match r {
            Ok(()) => self.faits += 1,
            Err(e) => self.refuses.push((id, e.to_string())),
        }
    }
}

/// Valide une selection de factures.
pub async fn factures(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Markup> {
    let choix = ids(&f);
    if choix.is_empty() {
        return Err(Erreur("aucune facture selectionnee".into()));
    }
    let action = f.get("action").map(|s| s.as_str()).unwrap_or("");
    if action != "valider" {
        return Err(Erreur(format!("action inconnue : « {action} »")));
    }

    let entity = entite::courante();
    let mut bilan = Bilan::nouveau();
    for id in choix {
        let r = db::invoice::validate(&app.pool, entity, id).map(|()| {
            // Le PDF part dans la GED, comme a la validation a l'unite.
            let _ = app::archiver_facture(&app.pool, entity, id);
        });
        bilan.ajouter(id, r);
    }

    Ok(resultat("Factures", &bilan, "/factures"))
}

/// Valide une selection de devis.
pub async fn devis(State(app): State<App>, Form(f): Form<HashMap<String, String>>) -> Res<Markup> {
    let choix = ids(&f);
    if choix.is_empty() {
        return Err(Erreur("aucun devis selectionne".into()));
    }
    let action = f.get("action").map(|s| s.as_str()).unwrap_or("");
    if action != "valider" {
        return Err(Erreur(format!("action inconnue : « {action} »")));
    }

    let entity = entite::courante();
    let mut bilan = Bilan::nouveau();
    for id in choix {
        let r = db::proposal::validate(&app.pool, entity, id).map(|()| {
            let _ = app::archiver_devis(&app.pool, entity, id);
        });
        bilan.ajouter(id, r);
    }

    Ok(resultat("Devis", &bilan, "/devis"))
}

/// Valide une selection de commandes clients.
pub async fn commandes(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Markup> {
    let choix = ids(&f);
    if choix.is_empty() {
        return Err(Erreur("aucune commande sélectionnée".into()));
    }
    let action = f.get("action").map(|s| s.as_str()).unwrap_or("");
    if action != "valider" {
        return Err(Erreur(format!("action inconnue : « {action} »")));
    }

    let entity = entite::courante();
    let mut bilan = Bilan::nouveau();
    for id in choix {
        bilan.ajouter(id, db::commande::validate(&app.pool, entity, id));
    }
    Ok(resultat("Commandes", &bilan, "/commandes"))
}

/// Ferme une selection de tickets.
///
/// Fermer n'est pas supprimer : la reclamation reste, avec sa date de cloture.
/// Un ticket ferme peut etre rouvert depuis sa fiche.
pub async fn tickets(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Markup> {
    let choix = ids(&f);
    if choix.is_empty() {
        return Err(Erreur("aucun ticket sélectionné".into()));
    }
    let action = f.get("action").map(|s| s.as_str()).unwrap_or("");
    if action != "fermer" {
        return Err(Erreur(format!("action inconnue : « {action} »")));
    }

    let entity = entite::courante();
    let mut bilan = Bilan::nouveau();
    for id in choix {
        bilan.ajouter(id, db::ticket::fermer(&app.pool, entity, id));
    }
    Ok(resultat("Tickets", &bilan, "/tickets"))
}

/// Le compte rendu, plutot qu'une redirection muette.
///
/// On ne renvoie pas un `?faits=3` : le motif d'un refus ne tient pas dans une
/// URL sans se faire maltraiter, et c'est justement le motif qui sert.
fn resultat(quoi: &str, bilan: &Bilan, retour: &str) -> Markup {
    ui::page(
        "factures",
        "liste",
        &format!("{quoi} — résultat"),
        html! {
            h1 { (quoi) " — résultat de l'action de masse" }
            p .sous-titre {(crate::langue::tr("Chaque ligne a été essayée. Un refus n'arrête pas les autres.")) }

            @if bilan.faits > 0 {
                (ui::flash(&format!("{} validé(s).", bilan.faits)))
            }

            @if !bilan.refuses.is_empty() {
                h2 { (bilan.refuses.len().to_string()) " refus(s)" }
                table {
                    thead { tr { th {(crate::langue::tr("N°")) } th {(crate::langue::tr("Motif")) } } }
                    tbody {
                        @for (id, motif) in &bilan.refuses {
                            tr {
                                td .ref { (id.to_string()) }
                                td { (motif) }
                            }
                        }
                    }
                }
            }

            div .actions {
                a .bouton href=(retour) {(crate::langue::tr("Retour à la liste")) }
            }
        },
    )
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
    fn les_cases_cochees_sont_lues() {
        let f = form(&[
            ("id_12", "1"),
            ("id_4", "1"),
            ("id_9", "1"),
            ("action", "valider"),
        ]);
        assert_eq!(ids(&f), vec![4, 9, 12]);
    }

    #[test]
    fn les_doublons_et_le_bruit_sont_ignores() {
        let f = form(&[
            ("id_4", "1"),
            ("id_4", "1"),
            ("id_x", "1"),
            ("id_", "1"),
            ("action", "valider"),
        ]);
        assert_eq!(ids(&f), vec![4]);
    }

    #[test]
    fn une_selection_vide_ne_rend_rien() {
        assert!(ids(&form(&[("action", "valider")])).is_empty());
    }

    #[test]
    fn un_identifiant_negatif_ou_nul_est_refuse_par_le_sql() {
        // On ne filtre pas ici : c'est la requete qui ne trouvera rien. Ce test
        // fige la decision — pas de filtrage silencieux cote formulaire.
        let f = form(&[("id_0", "1"), ("id_-3", "1")]);
        assert_eq!(ids(&f), vec![-3, 0]);
    }

    /// Un refus n'arrete pas les suivants : le bilan compte les deux.
    #[test]
    fn un_refus_n_interrompt_pas_la_serie() {
        let mut b = Bilan::nouveau();
        b.ajouter(1, Ok(()));
        b.ajouter(2, Err("facture deja validee".into()));
        b.ajouter(3, Ok(()));
        assert_eq!(b.faits, 2);
        assert_eq!(b.refuses.len(), 1);
        assert_eq!(b.refuses[0].0, 2);
        assert!(b.refuses[0].1.contains("deja validee"));
    }
}
