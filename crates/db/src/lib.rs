//! Acces MySQL et verification du schema reel contre le manifeste officiel.
//!
//! Le noyau Rust lit et ecrit le schema de Dolibarr tel quel. Aucune migration :
//! on **verifie** que la base est bien celle qu'on attend, et on refuse de demarrer
//! sinon. Les colonnes ajoutees par un module sont signalees, pas considerees comme
//! une erreur.

use std::collections::BTreeMap;
use std::error::Error;

pub mod achat;
pub mod achat_ecriture;
pub mod banque;
pub mod commande;
pub mod company;
pub mod compta;
pub mod contact;
pub mod dict;
pub mod divers;
pub mod droit;
pub mod fec;
pub mod ged;
pub mod grand_livre;
pub mod groupe;
pub mod impex;
pub mod invoice;
pub mod journal;
pub mod module;
pub mod party;
pub mod payment;
pub mod product;
pub mod projet;
pub mod proposal;
pub mod reglement;
pub mod relance;
pub mod sauvegarde;
pub mod situation;
pub mod stock;
pub mod tables;
pub mod ticket;
pub mod user;

use mysql::prelude::*;
use mysql::Opts;
pub use mysql::Pool;

pub fn pool(dsn: &str) -> Result<Pool, Box<dyn Error>> {
    let opts = Opts::from_url(dsn)?;
    Ok(Pool::new(opts)?)
}

/// Informations sur le serveur et la base, pour l'ecran de configuration.
#[derive(Debug, Clone, Default)]
pub struct InfosBase {
    pub version_serveur: String,
    pub base: String,
    pub hote: String,
    pub port: String,
    pub tables: i64,
    pub colonnes: i64,
}

pub fn infos_base(pool: &Pool) -> Result<InfosBase, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let version: Option<String> = conn.query_first("SELECT VERSION()")?;
    let base: Option<String> = conn.query_first("SELECT DATABASE()")?;
    let hote: Option<String> = conn.query_first("SELECT @@hostname")?;
    let port: Option<String> = conn.query_first("SELECT @@port")?;
    let tables: Option<i64> = conn.query_first(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE()",
    )?;
    let colonnes: Option<i64> = conn.query_first(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE()",
    )?;
    Ok(InfosBase {
        version_serveur: version.unwrap_or_default(),
        base: base.unwrap_or_default(),
        hote: hote.unwrap_or_default(),
        port: port.unwrap_or_default(),
        tables: tables.unwrap_or(0),
        colonnes: colonnes.unwrap_or(0),
    })
}

/// Modules Dolibarr actives (`llx_const` : `MAIN_MODULE_XXX = 1`).
pub fn modules_actifs(pool: &Pool) -> Result<Vec<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<String> = conn.query(
        "SELECT name FROM llx_const \
         WHERE name LIKE 'MAIN\\_MODULE\\_%' AND value = '1' ORDER BY name",
    )?;
    Ok(lignes
        .into_iter()
        .map(|n| {
            n.trim_start_matches("MAIN_MODULE_")
                .trim_end_matches("_M")
                .to_string()
        })
        .collect())
}

/// Date du jour au format `AAAA-MM-JJ`, telle que la voit le serveur de base.
pub fn aujourdhui(pool: &Pool) -> Result<String, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let d: Option<String> = conn.query_first("SELECT DATE_FORMAT(CURDATE(), '%Y-%m-%d')")?;
    Ok(d.unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveColumn {
    pub table: String,
    pub column: String,
    pub sql_type: String,
}

pub fn introspect(pool: &Pool, schema: &str) -> Result<Vec<LiveColumn>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(String, String, String)> = conn.exec(
        "SELECT TABLE_NAME, COLUMN_NAME, COLUMN_TYPE \
         FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ? \
         ORDER BY TABLE_NAME, ORDINAL_POSITION",
        (schema,),
    )?;
    Ok(rows
        .into_iter()
        .map(|(t, c, ty)| LiveColumn {
            table: t.to_ascii_lowercase(),
            column: c.to_ascii_lowercase(),
            sql_type: normalize_type(&ty),
        })
        .collect())
}

/// Ramene un type MySQL a une forme comparable avec le DDL de Dolibarr.
///
/// `integer` -> `int`, `boolean` -> `tinyint`, `real` -> `double`
/// (equivalents MariaDB par defaut), suppression de la largeur d'affichage
/// des entiers (`int(11)` -> `int`) et des espaces dans les arguments
/// (`double(24, 8)` -> `double(24,8)`).
pub fn normalize_type(t: &str) -> String {
    let t = t.trim().to_ascii_lowercase();
    let (base_raw, args) = match t.find('(') {
        Some(i) => {
            let close = t[i..].find(')').map(|j| i + j).unwrap_or(t.len() - 1);
            (t[..i].trim().to_string(), t[i..=close].replace(' ', ""))
        }
        None => (
            t.split_whitespace().next().unwrap_or("").to_string(),
            String::new(),
        ),
    };
    let base = match base_raw.as_str() {
        "integer" => "int",
        "real" => "double",
        "boolean" | "bool" => "tinyint",
        other => other,
    };
    let args = if matches!(
        base,
        "int" | "tinyint" | "smallint" | "mediumint" | "bigint"
    ) {
        String::new()
    } else {
        args
    };
    format!("{base}{args}")
}

#[derive(Debug, Default)]
pub struct Report {
    pub expected_tables: usize,
    pub live_tables: usize,
    pub missing_tables: Vec<String>,
    pub missing_columns: Vec<String>,
    pub extra_columns: Vec<String>,
    pub type_mismatches: Vec<String>,
}

impl Report {
    pub fn is_compatible(&self) -> bool {
        self.missing_tables.is_empty()
            && self.missing_columns.is_empty()
            && self.type_mismatches.is_empty()
    }
}

pub fn compare(manifest: &schema::Manifest, live: &[LiveColumn]) -> Report {
    let mut live_map: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut live_tables = std::collections::BTreeSet::new();
    for c in live {
        live_tables.insert(c.table.clone());
        live_map.insert((c.table.clone(), c.column.clone()), c.sql_type.clone());
    }

    let mut rep = Report {
        expected_tables: manifest.tables.len(),
        live_tables: live_tables.len(),
        ..Default::default()
    };

    for (tname, table) in &manifest.tables {
        let tname = tname.to_ascii_lowercase();
        if !live_tables.contains(&tname) {
            rep.missing_tables.push(tname.clone());
            continue;
        }
        for col in &table.columns {
            let key = (tname.clone(), col.name.to_ascii_lowercase());
            match live_map.get(&key) {
                None => rep.missing_columns.push(format!("{}.{}", key.0, key.1)),
                Some(actual) => {
                    let expected = normalize_type(&col.sql_type);
                    if *actual != expected {
                        rep.type_mismatches.push(format!(
                            "{}.{} attendu {} trouve {}",
                            key.0, key.1, expected, actual
                        ));
                    }
                }
            }
        }
    }

    let expected: std::collections::BTreeSet<(String, String)> = manifest
        .tables
        .iter()
        .flat_map(|(t, table)| {
            let t = t.to_ascii_lowercase();
            table
                .columns
                .iter()
                .map(move |c| (t.clone(), c.name.to_ascii_lowercase()))
        })
        .collect();

    for key in live_map.keys() {
        if !expected.contains(key) {
            rep.extra_columns.push(format!("{}.{}", key.0, key.1));
        }
    }

    rep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisation_des_types() {
        assert_eq!(normalize_type("integer"), "int");
        assert_eq!(normalize_type("int(11)"), "int");
        assert_eq!(normalize_type("int(11) unsigned"), "int");
        assert_eq!(normalize_type("tinyint(4)"), "tinyint");
        assert_eq!(normalize_type("smallint(6)"), "smallint");
        assert_eq!(normalize_type("double(24,8)"), "double(24,8)");
        assert_eq!(normalize_type("double(24, 8)"), "double(24,8)");
        assert_eq!(normalize_type("double(7,4)"), "double(7,4)");
        assert_eq!(normalize_type("real"), "double");
        assert_eq!(normalize_type("boolean"), "tinyint");
        assert_eq!(normalize_type("tinyint(1)"), "tinyint");
        assert_eq!(normalize_type("varchar(30)"), "varchar(30)");
        assert_eq!(normalize_type("VARCHAR(30)"), "varchar(30)");
        assert_eq!(normalize_type("text"), "text");
        assert_eq!(normalize_type("datetime"), "datetime");
    }

    #[test]
    fn comparaison_detecte_les_ecarts() {
        let mut m = schema::Manifest {
            generator: String::new(),
            dolibarr_version: String::new(),
            source_commit: String::new(),
            stats: schema::Stats {
                files: 0,
                statements: 0,
                create_table: 0,
                alter_table: 0,
                other: 0,
                unparsed: 0,
                tables: 1,
                columns: 2,
                indexes: 0,
                foreign_keys: 0,
            },
            fingerprint: String::new(),
            tables: Default::default(),
        };
        let mut t = schema::Table::new("llx_x");
        t.columns.push(schema::Column {
            name: "rowid".into(),
            sql_type: "integer".into(),
            nullable: false,
            default: None,
            auto_increment: true,
            primary_key: true,
            unique: false,
        });
        t.columns.push(schema::Column {
            name: "total".into(),
            sql_type: "double(24,8)".into(),
            nullable: true,
            default: None,
            auto_increment: false,
            primary_key: false,
            unique: false,
        });
        m.tables.insert("llx_x".into(), t);

        let ok = vec![
            LiveColumn {
                table: "llx_x".into(),
                column: "rowid".into(),
                sql_type: "int".into(),
            },
            LiveColumn {
                table: "llx_x".into(),
                column: "total".into(),
                sql_type: "double(24,8)".into(),
            },
        ];
        assert!(compare(&m, &ok).is_compatible());

        let mut bad = ok.clone();
        bad[1].sql_type = "double".into();
        let rep = compare(&m, &bad);
        assert!(!rep.is_compatible());
        assert_eq!(rep.type_mismatches.len(), 1);

        let mut with_extra = ok.clone();
        with_extra.push(LiveColumn {
            table: "llx_x".into(),
            column: "mon_module".into(),
            sql_type: "varchar(10)".into(),
        });
        let rep = compare(&m, &with_extra);
        assert!(
            rep.is_compatible(),
            "une colonne de module n'est pas une erreur"
        );
        assert_eq!(rep.extra_columns.len(), 1);
    }
}
