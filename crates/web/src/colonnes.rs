//! Le choix des colonnes : ce que chacun veut voir, et pas ce qu'on lui impose.
//!
//! Dolibarr range ce reglage dans `llx_user_param` ; on fait pareil, avec une cle
//! a nous (`cordeau.colonnes.<liste>`). L'ecran ne connait pas les colonnes : il
//! recoit une liste de noms et ne montre que ceux qui ont ete coches.
//!
//! Le formulaire est un `<details>` : il ne mange pas de place tant qu'on n'a pas
//! besoin de lui.

use std::collections::HashMap;

use axum::extract::State;
use axum::response::Redirect;
use axum::Form;
use maud::{html, Markup};

use crate::*;

/// L'identifiant de l'utilisateur de la session, resolu une fois.
pub(crate) fn utilisateur_id(pool: &db::Pool) -> Result<i64, Erreur> {
    let login = utilisateur::courant().ok_or_else(|| Erreur("session sans utilisateur".into()))?;
    Ok(db::user::par_login(pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid)
}

/// Les colonnes visibles d'une liste, dans l'ordre du code.
pub(crate) fn visibles(
    pool: &db::Pool,
    liste: &str,
    defaut: &[&str],
) -> Result<Vec<String>, Erreur> {
    let id = utilisateur_id(pool)?;
    Ok(db::preferences::colonnes(
        pool,
        entite::courante(),
        id,
        liste,
        defaut,
    )?)
}

/// Vrai si la colonne doit s'afficher.
pub(crate) fn montre(visibles: &[String], nom: &str) -> bool {
    visibles.iter().any(|c| c == nom)
}

/// Le panneau de choix des colonnes.
///
/// `disponibles` porte les noms techniques ; le libelle affiche vient de la
/// traduction du nom, ce qui evite de maintenir deux listes.
pub(crate) fn choix_colonnes(liste: &str, disponibles: &[&str], visibles: &[String]) -> Markup {
    html! {
        details .reglage {
            summary { (crate::langue::tr("Colonnes affichées")) }
            form method="post" action="/colonnes" {
                input type="hidden" name="liste" value=(liste);
                @for c in disponibles {
                    label .etiquette {
                        input type="checkbox" name={"c_" (c)} value="1"
                              checked[montre(visibles, c)];
                        " " (crate::langue::tr(c))
                    }
                }
                div .actions {
                    button type="submit" class="petit" {(crate::langue::tr("Appliquer")) }
                    span.aide.petite {
                        (crate::langue::tr("Le réglage est gardé pour votre compte, sur cette société."))
                    }
                }
            }
        }
    }
}

/// Enregistre les colonnes cochees pour la liste demandee.
pub(crate) async fn enregistrer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let liste = f
        .get("liste")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("liste manquante".into()))?;
    // Le nom de la liste vient d'un formulaire : on n'accepte que des
    // identifiants simples, jamais une chaine libre.
    if !liste
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(Erreur("nom de liste refusé".into()));
    }
    let mut retenues: Vec<String> = f
        .keys()
        .filter_map(|k| k.strip_prefix("c_").map(|n| n.to_string()))
        .collect();
    retenues.sort();
    let id = utilisateur_id(&app.pool)?;
    db::preferences::enregistrer_colonnes(&app.pool, entite::courante(), id, &liste, &retenues)?;
    Ok(Redirect::to(&format!("/{liste}?ok=colonnes")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seules_les_colonnes_retenues_s_affichent() {
        let visibles = vec!["nom".to_string(), "ville".to_string()];
        assert!(montre(&visibles, "nom"));
        assert!(!montre(&visibles, "telephone"));
    }

    #[test]
    fn un_nom_de_liste_avec_des_caracteres_bizarres_est_refuse() {
        let mauvais = "tiers;drop";
        assert!(!mauvais
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        let bon = "tiers";
        assert!(bon
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }
}
