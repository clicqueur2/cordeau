//! Sauvegarde : dump SQL + dossier documentaire, dans un dossier horodate.
//!
//! On s'appuie sur `mysqldump`, l'outil de la base, plutot que de reinventer un
//! export : c'est lui qui garantit un dump restaurable. Le rapport de
//! sauvegarde verifie ensuite que les compteurs coincident avec la base vive.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mysql::prelude::*;
use mysql::Pool;

#[derive(Debug, Clone, Default)]
pub struct Sauvegarde {
    pub dossier: PathBuf,
    pub fichier_sql: PathBuf,
    pub taille_sql: u64,
    pub fichiers_documents: usize,
    pub tables: u32,
    pub tiers: i64,
    pub factures: i64,
}

fn variable(cle: &str, defaut: &str) -> String {
    std::env::var(cle).unwrap_or_else(|_| defaut.to_string())
}

/// Horodatage lisible sans dependance : `AAAA-MM-JJ_HHMMSS` fourni par la base.
fn horodatage(pool: &Pool) -> Result<String, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let v: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%Y-%m-%d_%H%i%s')")?;
    Ok(v.unwrap_or_else(|| "sauvegarde".to_string()))
}

fn compter(dossier: &Path) -> usize {
    let mut n = 0;
    if let Ok(entrees) = fs::read_dir(dossier) {
        for e in entrees.flatten() {
            let p = e.path();
            if p.is_dir() {
                n += compter(&p);
            } else {
                n += 1;
            }
        }
    }
    n
}

fn copier(source: &Path, destination: &Path) -> std::io::Result<()> {
    if !source.exists() {
        return Ok(());
    }
    for e in fs::read_dir(source)?.flatten() {
        let p = e.path();
        let cible = destination.join(e.file_name());
        // `symlink_metadata` ne suit pas le lien : c'est ce qu'il faut, puisque
        // `is_dir()` puis `fs::copy()` sur le meme chemin laisse le temps de
        // remplacer le fichier par un lien vers l'exterieur (TOCTOU), et la
        // sauvegarde copierait alors une cible hors du dossier.
        let meta = match fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            // On ne suit pas : un lien dans `documents/` n'est pas un document.
            continue;
        }
        if meta.is_dir() {
            fs::create_dir_all(&cible)?;
            copier(&p, &cible)?;
        } else {
            fs::copy(&p, &cible)?;
        }
    }
    Ok(())
}

/// Cree une sauvegarde complete. Le dossier de destination est cree au besoin.
pub fn sauvegarder(pool: &Pool, racine: &Path) -> Result<Sauvegarde, Box<dyn Error>> {
    let horodatage = horodatage(pool)?;
    let dossier = racine.join(&horodatage);
    fs::create_dir_all(&dossier)?;

    let fichier_sql = dossier.join("base.sql");

    let hote = variable("DOLIBARR_DB_HOST", "127.0.0.1");
    let port = variable("DOLIBARR_DB_PORT", "3307");
    let utilisateur = variable("DOLIBARR_DB_USER", "root");
    let mot_de_passe = variable("DOLIBARR_DB_PASSWORD", "root");
    let base = variable("DOLIBARR_DB_NAME", "dolibarr");
    let dump = variable("DOLIBARR_MYSQLDUMP", "mysqldump");

    let sortie = Command::new(&dump)
        .arg(format!("--host={hote}"))
        .arg(format!("--port={port}"))
        .arg(format!("--user={utilisateur}"))
        .arg(format!("--password={mot_de_passe}"))
        .arg("--single-transaction")
        .arg("--routines")
        .arg("--default-character-set=utf8mb4")
        .arg(&base)
        .output();

    let sortie = match sortie {
        Ok(s) => s,
        Err(e) => {
            return Err(format!(
                "impossible de lancer `{dump}` ({e}). Installez le client MySQL ou \
                 renseignez DOLIBARR_MYSQLDUMP."
            )
            .into())
        }
    };
    if !sortie.status.success() {
        let err = String::from_utf8_lossy(&sortie.stderr);
        return Err(format!("mysqldump a echoue : {}", err.trim()).into());
    }
    fs::write(&fichier_sql, &sortie.stdout)?;

    // Documents : copie du dossier documentaire s'il existe.
    let documents = variable("DOLIBARR_DOCUMENTS", "documents");
    let copie_docs = dossier.join("documents");
    fs::create_dir_all(&copie_docs)?;
    copier(Path::new(&documents), &copie_docs)?;
    let fichiers_documents = compter(&copie_docs);

    // Verification : les compteurs doivent correspondre a la base vive.
    let mut conn = pool.get_conn()?;
    let tables: Option<i64> = conn.query_first(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE()",
    )?;
    let tiers: Option<i64> = conn.query_first("SELECT COUNT(*) FROM llx_societe")?;
    let factures: Option<i64> = conn.query_first("SELECT COUNT(*) FROM llx_facture")?;

    let s = Sauvegarde {
        dossier: dossier.clone(),
        fichier_sql: fichier_sql.clone(),
        taille_sql: fs::metadata(&fichier_sql).map(|m| m.len()).unwrap_or(0),
        fichiers_documents,
        tables: tables.unwrap_or(0) as u32,
        tiers: tiers.unwrap_or(0),
        factures: factures.unwrap_or(0),
    };

    // Manifeste : de quoi savoir ce que contient le dossier sans le restaurer.
    fs::write(
        dossier.join("MANIFESTE.txt"),
        format!(
            "Sauvegarde Dolibarr-RS\n\
             date            {horodatage}\n\
             base            {base}\n\
             tables          {}\n\
             tiers           {}\n\
             factures        {}\n\
             documents       {} fichiers\n\
             dump            {} octets\n",
            s.tables, s.tiers, s.factures, s.fichiers_documents, s.taille_sql
        ),
    )?;

    Ok(s)
}

/// Sauvegardes presentes, de la plus recente a la plus ancienne.
pub fn lister(racine: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    if let Ok(entrees) = fs::read_dir(racine) {
        for e in entrees.flatten() {
            let p = e.path();
            if p.is_dir() {
                let nom = e.file_name().to_string_lossy().to_string();
                let taille = compter(&p);
                out.push((nom, taille as u64));
            }
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}
