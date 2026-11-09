//! Les colonnes que Dolibarr renomme d'une majeure a l'autre.
//!
//! Dolibarr 25.0.0 (janvier 2027) renomme la colonne de date de creation de
//! sept tables, et toutes ces formes finissent en `date_creation` :
//!
//! | table | 24.0.1 | 25.0.0 |
//! |---|---|---|
//! | `llx_holiday`, `llx_expensereport`, `llx_multicurrency` | `date_create` | `date_creation` |
//! | `llx_element_contact` | `datecreate` | `date_creation` |
//! | `llx_ecm_directories`, `llx_ecm_files` | `date_c` | `date_creation` |
//! | `llx_mailing`, `llx_mailing_unsubscribe` | `date_creat` | `date_creation` |
//!
//! Cordeau doit lire les deux : la production est en 24, la prochaine majeure
//! arrive en janvier. On demande donc a `information_schema` quel nom existe,
//! une fois par table et par processus, puis on garde la reponse.
//!
//! Ce n'est pas une migration : on ne renomme rien, on constate.

use std::collections::HashMap;
use std::error::Error;
use std::sync::{Mutex, OnceLock};

use mysql::prelude::*;
use mysql::{params, Pool};

/// Les tables concernees et les noms candidats, du plus ancien au plus recent.
pub const RENOMMEES: &[(&str, &[&str])] = &[
    ("llx_holiday", &["date_create", "date_creation"]),
    ("llx_expensereport", &["date_create", "date_creation"]),
    ("llx_multicurrency", &["date_create", "date_creation"]),
    ("llx_element_contact", &["datecreate", "date_creation"]),
    ("llx_ecm_directories", &["date_c", "date_creation"]),
    ("llx_ecm_files", &["date_c", "date_creation"]),
    ("llx_mailing", &["date_creat", "date_creation"]),
    ("llx_mailing_unsubscribe", &["date_creat", "date_creation"]),
];

fn cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Le premier candidat present dans la liste des colonnes reelles.
///
/// Separé de la base pour etre verifiable : c'est la seule decision du module.
pub fn choisir(noms: &[String], candidats: &[&str]) -> String {
    candidats
        .iter()
        .find(|c| noms.iter().any(|n| n.eq_ignore_ascii_case(c)))
        .map(|c| (*c).to_string())
        .unwrap_or_else(|| candidats[0].to_string())
}

/// Le nom reel de la colonne de date de creation d'une table.
///
/// Rend le nom de la 24 si la table n'existe pas encore : une erreur plus claire
/// viendra de la requete qui l'utilise, pas d'ici.
pub fn date_creation(pool: &Pool, table: &str) -> Result<String, Box<dyn Error>> {
    let candidats = RENOMMEES
        .iter()
        .find(|(t, _)| *t == table)
        .map(|(_, c)| *c)
        .ok_or_else(|| format!("{table} n'est pas une table renommee par Dolibarr 25"))?;

    if let Ok(garde) = cache().lock() {
        if let Some(nom) = garde.get(table) {
            return Ok(nom.clone());
        }
    }

    let mut conn = pool.get_conn()?;
    let noms: Vec<String> = conn.exec(
        "SELECT COLUMN_NAME FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = :table",
        params! { "table" => table },
    )?;
    let nom = choisir(&noms, candidats);

    if let Ok(mut garde) = cache().lock() {
        garde.insert(table.to_string(), nom.clone());
    }
    Ok(nom)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noms(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn sur_une_base_24_on_lit_l_ancien_nom() {
        let table = noms(&["rowid", "date_create", "date_debut"]);
        assert_eq!(
            choisir(&table, &["date_create", "date_creation"]),
            "date_create"
        );
    }

    #[test]
    fn sur_une_base_25_on_lit_le_nouveau() {
        let table = noms(&["rowid", "date_creation", "date_debut"]);
        assert_eq!(
            choisir(&table, &["date_create", "date_creation"]),
            "date_creation"
        );
    }

    #[test]
    fn une_table_inconnue_retombe_sur_le_nom_de_la_24() {
        // Une requete qui echouera dira « unknown column date_create » : c'est
        // plus clair qu'un silence.
        assert_eq!(choisir(&[], &["date_c", "date_creation"]), "date_c");
    }

    #[test]
    fn la_casse_des_colonnes_ne_change_rien() {
        let table = noms(&["DATE_CREATION"]);
        assert_eq!(
            choisir(&table, &["date_create", "date_creation"]),
            "date_creation"
        );
    }

    #[test]
    fn les_huit_tables_annoncees_sont_toutes_la() {
        assert_eq!(RENOMMEES.len(), 8);
        for (table, candidats) in RENOMMEES {
            assert!(table.starts_with("llx_"), "{table}");
            assert_eq!(candidats.last(), Some(&"date_creation"), "{table}");
        }
    }
}
