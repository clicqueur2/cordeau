//! Journal des impressions : qui a imprimé quoi, sur quelle imprimante.
//!
//! Dolibarr a un module « Étiquettes / Impressions » dont la table
//! `llx_printing` n'est pas un objet métier : c'est une **trace**. Une ligne par
//! impression — le nom de l'imprimante, son emplacement, le module d'origine,
//! le nombre de copies, l'utilisateur.
//!
//! # Pourquoi une trace
//!
//! Sur un document réglementé — une facture, un avoir — savoir qu'une seconde
//! copie est sortie, et sur quelle imprimante, n'est pas de la curiosité : c'est
//! la réponse à « d'où vient ce papier ». Un journal qu'on peut effacer ne
//! prouve rien, mais un journal qu'on n'écrit pas ne prouve rien du tout.
//!
//! # Cette table n'a pas d'entité
//!
//! Vérifié sur le schéma : `llx_printing` ne porte **aucune colonne `entity`**.
//! Une impression est donc un fait de l'installation, pas d'une société. C'est
//! cohérent — l'imprimante du bureau est la même pour les deux sociétés — mais
//! cela veut dire qu'une liste filtrée par entité n'a pas de sens ici, et les
//! fonctions ci-dessous ne prennent pas ce paramètre.
//!
//! # Ce que ce module ne fait pas
//!
//! Il n'imprime rien. Il enregistre qu'une impression a eu lieu, comme
//! `mailing::marquer_envoye` enregistre un envoi sans l'envoyer. Un module qui
//! prétendrait piloter une imprimante depuis un serveur web mentirait sur ce
//! qu'il peut.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool, Row};

/// Longueur maximale des colonnes courtes (`varchar(16)`), en octets.
pub const CODE_MAX: usize = 16;

/// Une impression consignée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Impression {
    pub rowid: i64,
    /// Date de l'impression, `AAAA-MM-JJ HH:MM`.
    pub date: Option<String>,
    pub imprimante: String,
    pub emplacement: String,
    pub identifiant: String,
    /// Nombre de copies demandées (au moins une).
    pub copies: i32,
    /// Module d'origine (`facture`, `propal`, `commande`...).
    pub module: String,
    pub pilote: String,
    /// Utilisateur qui a lancé l'impression, s'il est connu.
    pub userid: Option<i64>,
}

/// Ce qu'on veut consigner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NouvelleImpression {
    pub imprimante: String,
    pub emplacement: String,
    pub identifiant: String,
    pub copies: i32,
    pub module: String,
    pub pilote: String,
    pub userid: Option<i64>,
}

impl NouvelleImpression {
    /// Une impression d'une copie, sur une imprimante nommée.
    pub fn nouvelle(module: &str, imprimante: &str) -> Self {
        Self {
            imprimante: imprimante.to_string(),
            emplacement: "—".to_string(),
            identifiant: imprimante.to_string(),
            copies: 1,
            module: module.to_string(),
            pilote: "defaut".to_string(),
            userid: None,
        }
    }
}

/// Consigne une impression. Rend son numero.
pub fn enregistrer(pool: &Pool, nouvelle: &NouvelleImpression) -> Result<i64, Box<dyn Error>> {
    verifier(nouvelle)?;

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_printing \
           (datec, printer_name, printer_location, printer_id, copy, module, driver, userid) \
         VALUES (NOW(), :nom, :lieu, :identifiant, :copies, :module, :pilote, :userid)",
        params! {
            "nom" => nouvelle.imprimante.trim(),
            "lieu" => nouvelle.emplacement.trim(),
            "identifiant" => nouvelle.identifiant.trim(),
            "copies" => nouvelle.copies,
            "module" => nouvelle.module.trim(),
            "pilote" => nouvelle.pilote.trim(),
            "userid" => nouvelle.userid,
        },
    )?;
    // Sur une connexion directe (et non une transaction), `last_insert_id`
    // rend un `u64` : il n'y a pas d'absence a traiter ici.
    Ok(conn.last_insert_id() as i64)
}

/// Les dernieres impressions, de la plus recente a la plus ancienne.
pub fn recentes(pool: &Pool, limite: u32) -> Result<Vec<Impression>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT rowid, DATE_FORMAT(datec, '%Y-%m-%d %H:%i'), printer_name, \
                printer_location, printer_id, copy, module, driver, userid \
         FROM llx_printing ORDER BY datec DESC, rowid DESC LIMIT :limite",
        params! { "limite" => limite.clamp(1, 500) },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| Impression {
            rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
            date: r.get_opt::<String, _>(1).and_then(|v| v.ok()),
            imprimante: texte(&r, 2),
            emplacement: texte(&r, 3),
            identifiant: texte(&r, 4),
            copies: r.get_opt::<i32, _>(5).and_then(|v| v.ok()).unwrap_or(1),
            module: texte(&r, 6),
            pilote: texte(&r, 7),
            userid: r.get_opt::<i64, _>(8).and_then(|v| v.ok()),
        })
        .collect())
}

/// Combien d'impressions par module, du plus frequent au moins frequent.
///
/// Sert a repondre a « qu'est-ce qui sort le plus de l'imprimante » — question
/// qui n'a pas de reponse sans journal.
pub fn par_module(pool: &Pool) -> Result<Vec<(String, i64)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(String, i64)> = conn.exec(
        "SELECT module, COUNT(*) FROM llx_printing \
         GROUP BY module ORDER BY COUNT(*) DESC, module ASC",
        (),
    )?;
    Ok(rows)
}

/// Le nombre total d'impressions consignees.
pub fn total(pool: &Pool) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.query_first("SELECT COUNT(*) FROM llx_printing")?;
    Ok(n.unwrap_or(0))
}

/// Verifie qu'une impression est consignable.
///
/// Trois refus, et chacun a une raison : sans nom d'imprimante la ligne ne dit
/// rien ; zero copie n'est pas une impression ; et un module plus long que sa
/// colonne ferait echouer l'INSERT en mode strict — ou pire, serait tronque.
fn verifier(nouvelle: &NouvelleImpression) -> Result<(), Box<dyn Error>> {
    if nouvelle.imprimante.trim().is_empty() {
        return Err("le nom de l'imprimante est obligatoire".into());
    }
    if nouvelle.copies < 1 {
        return Err("une impression compte au moins une copie".into());
    }
    if nouvelle.module.trim().is_empty() {
        return Err("le module d'origine est obligatoire".into());
    }
    if nouvelle.module.trim().len() > CODE_MAX {
        return Err(format!("le module dépasse {CODE_MAX} octets").into());
    }
    if nouvelle.pilote.trim().len() > CODE_MAX {
        return Err(format!("le pilote dépasse {CODE_MAX} octets").into());
    }
    Ok(())
}

fn texte(row: &Row, i: usize) -> String {
    row.get_opt::<String, _>(i)
        .and_then(|v| v.ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_impression_normale_passe() {
        assert!(verifier(&NouvelleImpression::nouvelle("facture", "HP LaserJet")).is_ok());
    }

    #[test]
    fn un_nom_d_imprimante_vide_est_refuse() {
        let mut n = NouvelleImpression::nouvelle("facture", "   ");
        n.imprimante = "  ".to_string();
        assert!(verifier(&n).is_err());
    }

    #[test]
    fn zero_copie_n_est_pas_une_impression() {
        let mut n = NouvelleImpression::nouvelle("facture", "HP");
        n.copies = 0;
        assert!(verifier(&n).is_err());
        n.copies = -1;
        assert!(verifier(&n).is_err());
    }

    #[test]
    fn un_module_trop_long_est_refuse() {
        let mut n = NouvelleImpression::nouvelle("facture", "HP");
        n.module = "x".repeat(CODE_MAX + 1);
        assert!(verifier(&n).is_err());
        n.module = "x".repeat(CODE_MAX);
        assert!(verifier(&n).is_ok(), "la longueur exacte doit passer");
    }

    #[test]
    fn le_constructeur_pose_une_copie_et_le_module() {
        let n = NouvelleImpression::nouvelle("propal", "Brother");
        assert_eq!(n.copies, 1);
        assert_eq!(n.module, "propal");
        assert_eq!(n.imprimante, "Brother");
    }
}
