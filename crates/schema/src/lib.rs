//! Lecture du schema SQL officiel Dolibarr.
//!
//! Le binaire n'impose pas son propre modele : il applique et verifie **le schema
//! de Dolibarr tel qu'il est publie**, ce qui garantit la compatibilite des donnees
//! et des modules externes.

pub mod ddl;
pub mod model;

pub use ddl::{parse_sql, ParseStats};
pub use model::{Column, ForeignKey, Index, Schema, Table};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Loaded {
    pub schema: Schema,
    pub stats: ParseStats,
    pub files: Vec<PathBuf>,
}

fn sql_files(dir: &Path, keys: bool) -> io::Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map(|x| x == "sql").unwrap_or(false)
                && p.file_name()
                    .map(|n| n.to_string_lossy().ends_with(".key.sql"))
                    .unwrap_or(false)
                    == keys
        })
        .collect();
    out.sort();
    Ok(out)
}

/// Charge `install/mysql/tables` : d'abord les tables, puis les cles et index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prepared {
    pub files: usize,
    pub statements: usize,
}

/// Reproduit `load_sql()` de Dolibarr : suppression des commentaires ligne par ligne,
/// remplacement de `type=innodb`, decoupage sur `;`, substitution de `__ENTITY__`.
///
/// Le decoupage est volontairement **aveugle aux guillemets**, comme `explode(';')`
/// cote Dolibarr : une apostrophe dans un commentaire (`can't`) ne doit pas casser
/// le decoupage du fichier.
pub fn prepare_file(sql: &str, entity: i64) -> (String, usize) {
    let entity = entity.to_string();
    let mut kept = String::with_capacity(sql.len());

    for line in sql.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("--") {
            continue;
        }
        kept.push_str(&strip_inline_comment(line));
        kept.push('\n');
    }
    let kept = replace_ci(&kept, "type=innodb", "ENGINE=innodb");

    let mut out = String::with_capacity(kept.len());
    let mut count = 0;
    for stmt in kept.split(';') {
        let s = stmt.trim();
        if s.is_empty() {
            continue;
        }
        out.push_str(&replace_ci(s, "__entity__", &entity));
        out.push_str(";\n");
        count += 1;
    }
    (out, count)
}

fn strip_inline_comment(line: &str) -> String {
    // Dolibarr retire `--` **et tout ce qui suit**, sur toute la ligne
    // (`preg_replace('/--.*$/m')`), sans exiger d'espace avant. On faisait la
    // meme chose a une condition pres : il fallait un blanc avant `--`, donc
    // `...)--Value (Price of the content...)` passait au travers. Sur les
    // scripts de la 12, ce seul cas faisait echouer la moitie des tables.
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'-' && bytes[i + 1] == b'-' {
            return line[..i].trim_end().to_string();
        }
        i += 1;
    }
    line.to_string()
}

fn replace_ci(s: &str, needle: &str, with: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let needle = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if lower[i..].starts_with(&needle) {
            out.push_str(with);
            i += needle.len();
        } else {
            let ch = s[i..].chars().next().expect("caractere");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Produit les trois fichiers a charger dans l'ordre : tables, cles, dictionnaires.
pub fn prepare_dir(mysql_dir: &Path, out_dir: &Path, entity: i64) -> io::Result<Prepared> {
    fs::create_dir_all(out_dir)?;
    let mut total = Prepared {
        files: 0,
        statements: 0,
    };

    let groups: [(&str, bool, &str); 3] = [
        ("tables", false, "1_tables.sql"),
        ("tables", true, "2_keys.sql"),
        ("data", false, "3_data.sql"),
    ];

    for (sub, keys, out_name) in groups {
        let dir = mysql_dir.join(sub);
        let mut files = sql_files(&dir, keys)?;
        if sub == "data" {
            files.retain(|p| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                name.starts_with("llx_")
                    && !name.contains('-')
                    && !name.starts_with("llx_accounting_account_")
            });
        }
        let mut buffer = String::new();
        for path in &files {
            let sql = fs::read_to_string(path)?;
            let (txt, n) = prepare_file(&sql, entity);
            buffer.push_str(&txt);
            total.files += 1;
            total.statements += n;
        }
        fs::write(out_dir.join(out_name), buffer)?;
    }

    Ok(total)
}

pub fn load_tables(dir: &Path) -> io::Result<Loaded> {
    let mut schema = Schema::default();
    let mut stats = ParseStats::default();
    let mut files = Vec::new();

    for pass_keys in [false, true] {
        for path in sql_files(dir, pass_keys)? {
            let sql = fs::read_to_string(&path)?;
            parse_sql(&mut schema, &sql, &mut stats);
            stats.files += 1;
            files.push(path);
        }
    }

    Ok(Loaded {
        schema,
        stats,
        files,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub files: usize,
    pub statements: usize,
    pub create_table: usize,
    pub alter_table: usize,
    pub other: usize,
    pub unparsed: usize,
    pub tables: usize,
    pub columns: usize,
    pub indexes: usize,
    pub foreign_keys: usize,
}

impl From<&Loaded> for Stats {
    fn from(l: &Loaded) -> Self {
        Self {
            files: l.stats.files,
            statements: l.stats.statements,
            create_table: l.stats.create_table,
            alter_table: l.stats.alter_table,
            other: l.stats.other,
            unparsed: l.stats.unparsed,
            tables: l.schema.tables.len(),
            columns: l.schema.column_count(),
            indexes: l.schema.index_count(),
            foreign_keys: l.schema.foreign_key_count(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub generator: String,
    pub dolibarr_version: String,
    pub source_commit: String,
    pub stats: Stats,
    pub fingerprint: String,
    pub tables: BTreeMap<String, Table>,
}

impl Manifest {
    pub fn build(loaded: &Loaded, version: &str, commit: &str) -> Self {
        let mut hasher = Sha256::new();
        let canonical = serde_json::to_string(&loaded.schema.tables).unwrap_or_default();
        hasher.update(canonical.as_bytes());
        let fingerprint = format!("{:x}", hasher.finalize());

        Self {
            generator: format!("dolibarr-rs schema {}", env!("CARGO_PKG_VERSION")),
            dolibarr_version: version.to_string(),
            source_commit: commit.to_string(),
            stats: Stats::from(loaded),
            fingerprint,
            tables: loaded.schema.tables.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vendor_tables() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("_vendor")
            .join("dolibarr-src")
            .join("htdocs")
            .join("install")
            .join("mysql")
            .join("tables")
    }

    fn loaded() -> Loaded {
        let dir = vendor_tables();
        assert!(
            dir.is_dir(),
            "schema officiel absent : {} â€” lancer `git sparse-checkout add htdocs/install/mysql`",
            dir.display()
        );
        load_tables(&dir).expect("lecture du schema officiel")
    }

    #[test]
    fn schema_officiel_24_0_1_est_integralement_parsable() {
        let l = loaded();
        let s = Stats::from(&l);
        assert_eq!(s.files, 749, "fichiers SQL attendus");
        assert_eq!(s.create_table, 410, "tables creees");
        assert_eq!(s.tables, 410, "tables dans le modele");
        assert_eq!(
            s.unparsed, 0,
            "instructions non comprises: {:#?}",
            l.stats.unparsed_examples
        );
        assert_eq!(s.alter_table, 1007, "instructions ALTER");
        assert_eq!(s.columns, 5362, "colonnes");
        assert_eq!(s.indexes, 749, "index et contraintes uniques");
        assert_eq!(s.foreign_keys, 235, "cles etrangeres");
    }

    #[test]
    fn chaque_table_a_au_moins_une_colonne() {
        let l = loaded();
        for (name, t) in &l.schema.tables {
            assert!(!t.columns.is_empty(), "table sans colonne: {name}");
        }
    }

    #[test]
    fn colonne_cle_primaire_auto_increment() {
        let l = loaded();
        let t = l.schema.table("llx_societe").expect("llx_societe");
        let rowid = t.column("rowid").expect("rowid");
        assert!(rowid.primary_key);
        assert!(rowid.auto_increment);
        assert_eq!(rowid.sql_type, "integer");
    }

    #[test]
    fn types_monetaires_conserves_a_l_identique() {
        let l = loaded();
        let d = l
            .schema
            .table("llx_propaldet")
            .and_then(|t| t.column("total_ht"))
            .expect("llx_propaldet.total_ht");
        assert_eq!(d.sql_type, "double(24,8)");
        assert!(d.nullable, "total_ht est nullable dans le schema Dolibarr");
        assert_eq!(d.default.as_deref(), Some("0"));

        let qty = l
            .schema
            .table("llx_propaldet")
            .and_then(|t| t.column("qty"))
            .expect("qty");
        assert_eq!(qty.sql_type, "real");

        let tva = l
            .schema
            .table("llx_propaldet")
            .and_then(|t| t.column("tva_tx"))
            .expect("tva_tx");
        assert_eq!(tva.sql_type, "double(7,4)");
    }

    #[test]
    fn varchar_et_not_null() {
        let l = loaded();
        let r = l
            .schema
            .table("llx_propal")
            .and_then(|t| t.column("ref"))
            .expect("llx_propal.ref");
        assert_eq!(r.sql_type, "varchar(30)");
        assert!(!r.nullable);
    }

    #[test]
    fn cles_etrangeres_et_index_uniques_resolus() {
        let l = loaded();
        let t = l.schema.table("llx_propal").expect("llx_propal");
        let fk = t
            .foreign_keys
            .iter()
            .find(|f| f.name == "fk_propal_fk_soc")
            .expect("fk_propal_fk_soc");
        assert_eq!(fk.columns, vec!["fk_soc"]);
        assert_eq!(fk.ref_table, "llx_societe");
        assert_eq!(fk.ref_columns, vec!["rowid"]);

        let uk = t
            .indexes
            .iter()
            .find(|i| i.name == "uk_propal_ref")
            .expect("uk_propal_ref");
        assert!(uk.unique);
        assert_eq!(uk.columns, vec!["ref", "entity"]);
    }

    #[test]
    fn commentaires_et_lignes_commentees_ignores() {
        let mut schema = Schema::default();
        let mut stats = ParseStats::default();
        parse_sql(
            &mut schema,
            "-- create table fantome (id integer);\nCREATE TABLE t (id integer NOT NULL);\n--ALTER TABLE t ADD INDEX idx_x (id);",
            &mut stats,
        );
        assert_eq!(stats.create_table, 1);
        assert_eq!(stats.alter_table, 0);
        assert_eq!(schema.tables.len(), 1);
    }

    #[test]
    fn preparation_ne_produit_pas_d_instruction_vide() {
        let sql = "-- commentaire\ncreate table t (\n  id integer,  -- la colonne de l'identifiant\n  name varchar(10)\n)ENGINE=innodb;\n";
        let (out, n) = prepare_file(sql, 1);
        assert_eq!(n, 1, "{out}");
        assert!(!out.contains(";;"), "instruction vide: {out}");
        assert!(out.contains("ENGINE=innodb;"));
        assert!(!out.contains("--"));
    }

    #[test]
    fn preparation_remplace_entity_et_type_innodb() {
        let sql = "INSERT INTO llx_x (entity) VALUES (__ENTITY__);\nALTER TABLE t type=innodb;\n";
        let (out, n) = prepare_file(sql, 3);
        assert_eq!(n, 2);
        assert!(out.contains("VALUES (3)"));
        assert!(out.contains("ENGINE=innodb"));
    }

    #[test]
    fn manifeste_deterministe() {
        let l = loaded();
        let a = Manifest::build(&l, "24.0.1", "b795838");
        let b = Manifest::build(&l, "24.0.1", "b795838");
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a.fingerprint.len(), 64);
        let json = serde_json::to_string(&a).unwrap();
        let back: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tables.len(), 410);
    }
}

/// Un point-virgule dans une chaine ne coupe pas l'instruction.
///
/// C'est la difference entre `split_statements` et un `split(';')` naif :
/// une valeur comme `'devis; signature'` serait coupee en deux, et la
/// migration echouerait sur une instruction tronquee.
#[test]
fn un_point_virgule_dans_une_chaine_ne_coupe_pas() {
    let sql = "INSERT INTO t VALUES ('a;b', 'c');\nUPDATE t SET x = ';';\n";
    let st = ddl::split_statements(sql);
    assert_eq!(st.len(), 2, "{st:?}");
    assert!(st[0].contains("'a;b'"), "{st:?}");
}

/// Les commentaires disparaissent, et l'instruction suivante survit.
///
/// MariaDB ne reconnait `--` comme commentaire que suivi d'un blanc :
/// `-- note` disparait, `a - -b` reste une soustraction.
#[test]
fn les_commentaires_ne_collent_pas_a_l_instruction() {
    let sql = "-- note de bas de page\nCREATE TABLE t (a int);\n# autre\n";
    let st = ddl::split_statements(sql);
    assert_eq!(st.len(), 1, "{st:?}");
    assert!(st[0].starts_with("CREATE TABLE"), "{st:?}");
}

/// Sans blanc apres `--`, ce n'est pas un commentaire : c'est une
/// soustraction, et on ne la mange pas.
#[test]
fn un_double_moins_sans_blanc_reste_du_code() {
    let st = ddl::split_statements("SELECT 3 - -2;");
    assert_eq!(st.len(), 1, "{st:?}");
    assert!(st[0].contains("- -2"), "{st:?}");
}
