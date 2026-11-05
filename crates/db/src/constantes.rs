//! Les constantes : la table des reglages de Dolibarr.
//!
//! `llx_const` porte **tous** les reglages du logiciel : nom de la societe,
//! serveur SMTP, modules actifs, formats d'affichage, codes VATEX… Dolibarr a un
//! ecran pour la parcourir (`admin/const.php`) ; nous n'en avions aucun, et on
//! repondait jusqu'ici « configurez dans le dictionnaire » sans donner le moyen
//! de le faire.
//!
//! Trois principes :
//!
//! 1. **Une constante sensible n'affiche pas sa valeur.** Un mot de passe SMTP
//!    ou une cle d'API se lit `••••••` a l'ecran ; on peut la remplacer, pas la
//!    lire. Un ecran de reglages ne doit pas etre un endroit ou l'on vole des
//!    secrets.
//! 2. **L'entite 0 est globale, l'entite N est propre a la societe.** On lit les
//!    deux, on ecrit dans celle de la session.
//! 3. **Une constante vide se supprime, elle ne vaut pas chaine vide.** Dolibarr
//!    teste `getDolGlobalString()` sur l'existence : une ligne vide et une ligne
//!    absente n'ont pas le meme effet.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Une constante, telle que l'ecran la montre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constante {
    pub nom: String,
    pub valeur: String,
    pub entite: i64,
    /// Vrai si le nom evoque un secret : la valeur n'est pas affichee.
    pub sensible: bool,
}

/// Le nom evoque-t-il un secret ?
///
/// On se fonde sur le **nom**, jamais sur la valeur : une valeur courte n'est
/// pas un secret, et une valeur longue n'en est pas un non plus. Les noms
/// Dolibarr sont parlants (`..._MOTDEPASSE`, `..._PASSWORD`, `..._API_KEY`).
pub fn est_sensible(nom: &str) -> bool {
    let n = nom.to_uppercase();
    [
        "MOTDEPASSE",
        "PASSWORD",
        "SECRET",
        "API_KEY",
        "APIKEY",
        "TOKEN",
        "PRIVATE_KEY",
    ]
    .iter()
    .any(|motif| n.contains(motif))
}

/// La valeur affichable : masquee quand le nom est sensible.
pub fn valeur_affichable(nom: &str, valeur: &str) -> String {
    if !est_sensible(nom) || valeur.is_empty() {
        return valeur.to_string();
    }
    // On garde la longueur visible : savoir qu'un mot de passe fait 12 ou 60
    // caracteres aide a diagnostiquer sans reveler lequel.
    "•".repeat(valeur.chars().count().min(24))
}

/// Les constantes visibles depuis une entite : les globales et les siennes.
///
/// `filtre` borne sur le nom, en correspondance partielle, insensible a la casse.
pub fn liste(
    pool: &Pool,
    entity: i64,
    filtre: Option<&str>,
    limite: u32,
) -> Result<Vec<Constante>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let motif = format!(
        "%{}%",
        filtre.unwrap_or("").trim().to_uppercase().replace('%', "")
    );
    let rows: Vec<(String, Option<String>, i64)> = conn.exec(
        "SELECT name, value, entity FROM llx_const \
         WHERE (entity = :entity OR entity = 0) AND UPPER(name) LIKE :motif \
         ORDER BY name ASC \
         LIMIT :limite",
        params! { "entity" => entity, "motif" => motif, "limite" => limite.clamp(1, 500) },
    )?;

    Ok(rows
        .into_iter()
        .map(|(nom, valeur, entite)| Constante {
            sensible: est_sensible(&nom),
            nom,
            valeur: valeur.unwrap_or_default(),
            entite,
        })
        .collect())
}

/// Lit une constante : celle de l'entite d'abord, la globale ensuite.
pub fn lire(pool: &Pool, entity: i64, nom: &str) -> Result<Option<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let v: Option<String> = conn.exec_first(
        "SELECT value FROM llx_const WHERE name = :nom AND entity = :entity LIMIT 1",
        params! { "nom" => nom, "entity" => entity },
    )?;
    if v.is_some() {
        return Ok(v);
    }
    Ok(conn.exec_first(
        "SELECT value FROM llx_const WHERE name = :nom AND entity = 0 LIMIT 1",
        params! { "nom" => nom },
    )?)
}

/// Ecrit une constante dans l'entite de la session.
///
/// Une valeur vide **supprime** la ligne : c'est ainsi qu'on revient au defaut.
/// Le nom est verifie : lettres, chiffres et souligne seulement, pour qu'aucune
/// valeur saisie ne puisse se glisser dans un identifiant SQL.
pub fn ecrire(pool: &Pool, entity: i64, nom: &str, valeur: &str) -> Result<(), Box<dyn Error>> {
    let nom = nom.trim();
    if !nom_valide(nom) {
        return Err(format!("nom de constante invalide : « {nom} »").into());
    }
    let mut conn = pool.get_conn()?;

    if valeur.trim().is_empty() {
        conn.exec_drop(
            "DELETE FROM llx_const WHERE name = :nom AND entity = :entity",
            params! { "nom" => nom, "entity" => entity },
        )?;
        return Ok(());
    }

    conn.exec_drop(
        "INSERT INTO llx_const (name, entity, value, type, visible) \
         VALUES (:nom, :entity, :valeur, 'chaine', 0) \
         ON DUPLICATE KEY UPDATE value = VALUES(value)",
        params! { "nom" => nom, "entity" => entity, "valeur" => valeur },
    )?;
    Ok(())
}

/// Un nom de constante acceptable : majuscules, chiffres, souligne.
fn nom_valide(s: &str) -> bool {
    !s.is_empty() && s.len() <= 180 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_nom_qui_evoque_un_secret_est_masque() {
        assert!(est_sensible("DOLIBARR_RS_SMTP_MOTDEPASSE"));
        assert!(est_sensible("STRIPE_API_KEY"));
        assert!(est_sensible("quelque_token"));
        assert!(!est_sensible("MAIN_INFO_SOCIETE_NOM"));
        assert!(!est_sensible("MAIN_LANG_DEFAULT"));
    }

    #[test]
    fn la_valeur_masquee_garde_la_longueur() {
        let m = valeur_affichable("SMTP_PASSWORD", "secret-de-12");
        assert_eq!(m.chars().count(), 12);
        assert!(!m.contains("secret"));
    }

    #[test]
    fn une_valeur_non_sensible_est_lisible() {
        assert_eq!(
            valeur_affichable("MAIN_INFO_SOCIETE_NOM", "Cordeau"),
            "Cordeau"
        );
    }

    #[test]
    fn une_valeur_sensible_vide_reste_vide() {
        assert_eq!(valeur_affichable("API_KEY", ""), "");
    }

    #[test]
    fn le_masque_est_borne() {
        let long = "x".repeat(200);
        assert_eq!(valeur_affichable("TOKEN", &long).chars().count(), 24);
    }

    #[test]
    fn un_nom_de_constante_est_verifie() {
        assert!(nom_valide("MAIN_INFO_SOCIETE_NOM"));
        assert!(!nom_valide(""));
        assert!(!nom_valide("MAIN; DROP TABLE llx_const"));
        assert!(!nom_valide("nom avec espace"));
        assert!(!nom_valide("nom'quote"));
    }
}
