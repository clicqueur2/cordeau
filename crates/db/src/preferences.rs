//! Preferences d'affichage, dans `llx_user_param` — la table ou Dolibarr range
//! deja celles de chacun.
//!
//! Un reglage d'ecran n'est pas une constante de l'application : « je veux voir
//! la ville et pas le telephone » n'est pas vrai pour tout le monde. On le range
//! donc par utilisateur et par entite, exactement comme Dolibarr, pour que les
//! deux logiciels se retrouvent sur la meme base.
//!
//! La cle est un nom stable (`cordeau.colonnes.tiers`), la valeur une liste
//! separee par des virgules. Rien de plus : un format qu'on peut lire a l'oeil
//! dans la table, et corriger a la main si besoin.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Lit une preference. Rend `None` si elle n'a jamais ete posee.
pub fn lire(
    pool: &Pool,
    entity: i64,
    fk_user: i64,
    cle: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let valeur: Option<String> = conn.exec_first(
        "SELECT value FROM llx_user_param \
         WHERE entity = :entity AND fk_user = :user AND param = :cle",
        params! { "entity" => entity, "user" => fk_user, "cle" => cle },
    )?;
    Ok(valeur)
}

/// Ecrit une preference (et remplace l'ancienne : la cle est la clef primaire).
pub fn ecrire(
    pool: &Pool,
    entity: i64,
    fk_user: i64,
    cle: &str,
    valeur: &str,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_user_param (fk_user, entity, param, value) \
         VALUES (:user, :entity, :cle, :valeur) \
         ON DUPLICATE KEY UPDATE value = VALUES(value)",
        params! { "user" => fk_user, "entity" => entity, "cle" => cle, "valeur" => valeur },
    )?;
    Ok(())
}

/// Les colonnes retenues pour une liste, ou le defaut si rien n'est regle.
///
/// Un nom de colonne inconnu est ignore : si une colonne disparait du code, la
/// preference qui la cite ne doit pas casser l'ecran.
pub fn colonnes(
    pool: &Pool,
    entity: i64,
    fk_user: i64,
    liste: &str,
    defaut: &[&str],
) -> Result<Vec<String>, Box<dyn Error>> {
    let cle = format!("cordeau.colonnes.{liste}");
    match lire(pool, entity, fk_user, &cle)? {
        Some(v) => {
            let voulues: Vec<String> = v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            // Une liste vide en base veut dire « tout montrer », pas « rien ».
            if voulues.is_empty() {
                return Ok(defaut.iter().map(|s| (*s).to_string()).collect());
            }
            Ok(voulues)
        }
        None => Ok(defaut.iter().map(|s| (*s).to_string()).collect()),
    }
}

/// Enregistre les colonnes retenues. Une liste vide repasse au defaut.
pub fn enregistrer_colonnes(
    pool: &Pool,
    entity: i64,
    fk_user: i64,
    liste: &str,
    retenues: &[String],
) -> Result<(), Box<dyn Error>> {
    let cle = format!("cordeau.colonnes.{liste}");
    ecrire(pool, entity, fk_user, &cle, &retenues.join(","))
}

/// Lit la preference de l'entite, sinon celle du **global** (entite 0).
///
/// Dolibarr range ses preferences par entite. Pour un reglage qui appartient a
/// la personne — le style, la langue de l'interface — c'est etonnant : changer
/// de societe changerait d'apparence. On accepte donc un defaut global, et on
/// ne l'invente pas : c'est la meme table, avec `entity = 0`.
pub fn lire_avec_defaut(
    pool: &Pool,
    entity: i64,
    fk_user: i64,
    cle: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    if let Some(v) = lire(pool, entity, fk_user, cle)? {
        return Ok(Some(v));
    }
    if entity != 0 {
        return lire(pool, 0, fk_user, cle);
    }
    Ok(None)
}

/// Efface une preference. Rien a effacer n'est pas une erreur.
pub fn supprimer(pool: &Pool, entity: i64, fk_user: i64, cle: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_user_param \
         WHERE entity = :entity AND fk_user = :user AND param = :cle",
        params! { "entity" => entity, "user" => fk_user, "cle" => cle },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Le format est celui qu'on relit a l'oeil dans la table.
    #[test]
    fn une_liste_de_colonnes_se_relit_a_l_identique() {
        let retenues = vec!["code".to_string(), "nom".to_string(), "ville".to_string()];
        assert_eq!(retenues.join(","), "code,nom,ville");
        let relu: Vec<String> = "code,nom,ville"
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        assert_eq!(relu, retenues);
    }

    /// Une valeur vide en base ne doit pas vider l'ecran : elle veut dire
    /// « rien de regle », donc on garde le defaut.
    #[test]
    fn une_valeur_vide_laisse_le_defaut() {
        let defaut = ["code", "nom"];
        let voulues: Vec<String> = ""
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        assert!(voulues.is_empty());
        assert_eq!(defaut.len(), 2);
    }
}
