//! Gestion documentaire : `llx_ecm_files` de Dolibarr.
//!
//! Les fichiers sont ranges sur disque selon la convention Dolibarr
//! (`documents/<modulepart>/<ref>/<ref>.pdf`) et indexes en base, ce qui les fait
//! apparaitre dans l'onglet « Documents » de la fiche. Un PDF qui n'est pas
//! archive n'est pas une piece : c'est une impression.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use mysql::prelude::*;
use mysql::{params, Pool};

/// Racine des documents. Dolibarr utilise `<dolibarr>/documents`.
pub fn racine() -> PathBuf {
    std::env::var("DOLIBARR_DOCUMENTS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("documents"))
}

/// Document a deposer dans la GED.
pub struct DocumentAAchiver<'a> {
    pub modulepart: &'a str,
    pub reference: &'a str,
    pub object_type: &'a str,
    pub object_id: i64,
    pub label: &'a str,
    pub contenu: &'a [u8],
    pub extension: &'a str,
}

/// Ecrit le fichier puis l'indexe. Renvoie le `rowid` de `llx_ecm_files`.
pub fn archiver(
    pool: &Pool,
    entity: i64,
    doc: &DocumentAAchiver<'_>,
) -> Result<i64, Box<dyn Error>> {
    let dossier = racine().join(doc.modulepart).join(doc.reference);
    fs::create_dir_all(&dossier)?;

    let nom_fichier = format!("{}.{}", doc.reference, doc.extension);
    fs::write(dossier.join(&nom_fichier), doc.contenu)?;

    // `filepath` porte le dossier, `filename` le fichier : c'est ainsi que
    // Dolibarr retrouve ensuite le document.
    let filepath = format!("{}/{}", doc.modulepart, doc.reference);
    let taille = doc.contenu.len() as i64;
    let type_fichier = format!("application/{}", doc.extension);

    let mut conn = pool.get_conn()?;
    let date_c = crate::colonnes::date_creation(pool, "llx_ecm_files")?;
    conn.exec_drop(
        format!(
            "INSERT INTO llx_ecm_files \
         (ref, label, entity, filepath, filename, src_object_type, src_object_id, \
          gen_or_uploaded, {date_c}) \
         VALUES (:reference, :label, :entity, :filepath, :filename, :otype, :oid, \
          'generated', NOW())"
        ),
        params! {
            "reference" => doc.reference,
            "label" => doc.label,
            "entity" => entity,
            "filepath" => filepath,
            "filename" => nom_fichier,
            "otype" => doc.object_type,
            "oid" => doc.object_id,
        },
    )?;
    let rowid = conn.last_insert_id() as i64;

    // Les colonnes `filetype` / `filesize` ne sont pas obligatoires mais Dolibarr
    // les affiche ; on les renseigne quand elles existent.
    let _ = conn.exec_drop(
        "UPDATE llx_ecm_files SET filetype = :t, filesize = :s, \
          fullpath_orig = '' WHERE rowid = :rowid",
        params! { "t" => type_fichier, "s" => taille, "rowid" => rowid },
    );

    Ok(rowid)
}

/// Un document indexe dans la GED.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentIndexe {
    pub rowid: i64,
    pub label: String,
    pub filepath: String,
    pub filename: String,
    pub date_c: Option<String>,
}

/// Tous les documents indexes de l'entite, du plus recent au plus ancien.
pub fn tous(pool: &Pool, entity: i64, limit: u32) -> Result<Vec<DocumentIndexe>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let date_c = crate::colonnes::date_creation(pool, "llx_ecm_files")?;
    let sql = format!(
        "SELECT rowid, label, filepath, filename, DATE_FORMAT({date_c}, '%Y-%m-%d') \
         FROM llx_ecm_files WHERE entity = :entity \
         ORDER BY {date_c} DESC, rowid DESC LIMIT :limit"
    );
    let rows: Vec<mysql::Row> = conn.exec(sql, params! { "entity" => entity, "limit" => limit })?;
    let s = |r: &mysql::Row, i: usize| r.get_opt::<String, _>(i).and_then(|v| v.ok());
    Ok(rows
        .into_iter()
        .map(|r| DocumentIndexe {
            rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
            label: s(&r, 1).unwrap_or_default(),
            filepath: s(&r, 2).unwrap_or_default(),
            filename: s(&r, 3).unwrap_or_default(),
            date_c: s(&r, 4),
        })
        .collect())
}

/// Chemins des documents rattaches a un objet.
pub fn pour_element(
    pool: &Pool,
    entity: i64,
    object_type: &str,
    object_id: i64,
) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<(String, String)> = conn.exec(
        "SELECT filepath, filename FROM llx_ecm_files \
         WHERE entity = :entity AND src_object_type = :otype AND src_object_id = :oid \
         ORDER BY rowid DESC",
        params! { "entity" => entity, "otype" => object_type, "oid" => object_id },
    )?;
    Ok(lignes)
}

pub fn supprimer_pour_element(
    pool: &Pool,
    entity: i64,
    object_type: &str,
    object_id: i64,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let fichiers = pour_element(pool, entity, object_type, object_id)?;
    conn.exec_drop(
        "DELETE FROM llx_ecm_files \
         WHERE entity = :entity AND src_object_type = :otype AND src_object_id = :oid",
        params! { "entity" => entity, "otype" => object_type, "oid" => object_id },
    )?;
    for (dossier, nom) in fichiers {
        let chemin: PathBuf = racine().join(&dossier).join(&nom);
        if chemin.is_file() {
            let _ = fs::remove_file(&chemin);
        }
        nettoyer_dossier(&racine().join(&dossier));
    }
    Ok(())
}

/// Retire un dossier s'il est vide.
fn nettoyer_dossier(dossier: &Path) {
    if dossier.is_dir()
        && fs::read_dir(dossier)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false)
    {
        let _ = fs::remove_dir(dossier);
    }
}
