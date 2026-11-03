//! Explorateur de tables : les 410 tables du schema, lisibles sans code dedie.
//!
//! Ecrire un ecran par table a la main, c'est 410 ecrans et des mois. Les
//! tables sont deja decrites par le schema : on les lit dans
//! `information_schema` et on les affiche. Ce n'est pas un ecran metier — pas
//! de calcul, pas de regle — mais c'est **toutes les donnees de Dolibarr,
//! visibles en Rust**, aujourd'hui.
//!
//! Reserve aux administrateurs : l'ecran montre tout, y compris les tables
//! techniques.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool, Row};

/// Une ligne de table, toutes colonnes en texte.
pub type Ligne = Vec<Option<String>>;

/// Une table du schema, avec son nombre de lignes approximatif.
pub struct Table {
    pub nom: String,
    pub lignes: u64,
    pub commentaire: String,
}

/// Les tables de la base, triees par nom.
pub fn tables(pool: &Pool) -> Result<Vec<Table>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(String, Option<u64>, Option<String>)> = conn.exec(
        "SELECT TABLE_NAME, TABLE_ROWS, TABLE_COMMENT FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE' \
         ORDER BY TABLE_NAME ASC",
        (),
    )?;
    Ok(rows
        .into_iter()
        .map(|(nom, lignes, commentaire)| Table {
            nom,
            lignes: lignes.unwrap_or(0),
            commentaire: commentaire.unwrap_or_default(),
        })
        .collect())
}

/// Les colonnes d'une table, avec leur type SQL, dans l'ordre du schema.
pub fn colonnes_avec_types(
    pool: &Pool,
    table: &str,
) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    if !nom_valide(table) {
        return Err("nom de table refuse".into());
    }
    let mut conn = pool.get_conn()?;
    let rows: Vec<(String, String)> = conn.exec(
        "SELECT COLUMN_NAME, DATA_TYPE FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = :table \
         ORDER BY ORDINAL_POSITION ASC",
        params! { "table" => table },
    )?;
    Ok(rows)
}

/// Les noms de colonnes, sans les types.
pub fn colonnes(pool: &Pool, table: &str) -> Result<Vec<String>, Box<dyn Error>> {
    Ok(colonnes_avec_types(pool, table)?
        .into_iter()
        .map(|(c, _)| c)
        .collect())
}

/// Les lignes d'une table, en texte, pour l'affichage.
///
/// Quand la table porte une colonne `entity`, on ne montre que l'entite
/// courante : un explorateur ne doit pas devenir une fuite entre entites.
pub fn lignes(
    pool: &Pool,
    table: &str,
    entity: i64,
    limite: u64,
    decalage: u64,
) -> Result<(Vec<String>, Vec<Ligne>), Box<dyn Error>> {
    let types = colonnes_avec_types(pool, table)?;
    if types.is_empty() {
        return Err(format!("table inconnue : {table}").into());
    }
    let colonnes: Vec<String> = types.iter().map(|(c, _)| c.clone()).collect();
    for c in &colonnes {
        if !nom_valide(c) {
            return Err(format!("colonne refusee : {c}").into());
        }
    }

    // Chaque type a sa facon de devenir du texte : un point geometrique passe
    // par ST_AsText, un binaire par sa taille. `CAST(... AS CHAR)` echoue sur
    // les deux (erreur 4079 « Illegal parameter data type »).
    let projection = types
        .iter()
        .map(|(nom, type_sql)| match type_sql.as_str() {
            "point" | "geometry" | "linestring" | "polygon" | "multipoint" | "multilinestring"
            | "multipolygon" | "geometrycollection" | "geomcollection" => {
                format!("ST_AsText(`{nom}`)")
            }
            "blob" | "tinyblob" | "mediumblob" | "longblob" | "binary" | "varbinary" => {
                format!("CONCAT('(', LENGTH(`{nom}`), ' octets)')")
            }
            _ => format!("CAST(`{nom}` AS CHAR)"),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let filtre = if colonnes.iter().any(|c| c == "entity") {
        " WHERE entity = ?".to_string()
    } else {
        String::new()
    };
    // La table vient d'information_schema (donc de la base elle-meme) et son
    // nom est revalide ; la limite et le decalage sont bornes.
    let sql = format!(
        "SELECT {projection} FROM `{table}`{filtre} LIMIT {} OFFSET {}",
        limite.clamp(1, 500),
        decalage
    );

    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = if filtre.is_empty() {
        conn.exec(sql, ())?
    } else {
        conn.exec(sql, (entity,))?
    };

    let valeurs = rows
        .iter()
        .map(|r| {
            (0..r.len())
                .map(|i| {
                    r.get_opt::<String, _>(i)
                        .and_then(|v| v.ok())
                        .or_else(|| {
                            r.get_opt::<i64, _>(i)
                                .and_then(|v| v.ok())
                                .map(|n| n.to_string())
                        })
                        .or_else(|| {
                            r.get_opt::<f64, _>(i).and_then(|v| v.ok()).map(|n| {
                                let mut s = format!("{n}");
                                if s.contains('.') {
                                    while s.ends_with('0') {
                                        s.pop();
                                    }
                                    if s.ends_with('.') {
                                        s.pop();
                                    }
                                }
                                s
                            })
                        })
                })
                .collect()
        })
        .collect();

    Ok((colonnes, valeurs))
}

/// Un identifiant SQL acceptable : lettres, chiffres, souligne.
fn nom_valide(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_nom_de_table_bizarre_est_refuse() {
        for nom in ["llx_user; DROP TABLE llx_user", "llx user", "", "llx_user`"] {
            assert!(!nom_valide(nom), "accepte a tort : {nom}");
        }
        assert!(nom_valide("llx_societe"));
        assert!(nom_valide("llx_c_paiement"));
    }
}
