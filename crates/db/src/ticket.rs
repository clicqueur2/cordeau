//! Tickets de support (`llx_ticket`).
//!
//! Un ticket, c'est une reclamation client tracee : sujet, message, statut,
//! societe concernee. Dolibarr le numerote `TK` + AAAMM + 4 chiffres.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

pub const PREFIX: &str = "TK";
/// Nouveau (ouvert, non lu).
pub const STATUT_NOUVEAU: i16 = 0;
/// En cours.
pub const STATUT_EN_COURS: i16 = 1;
/// Ferme.
pub const STATUT_FERME: i16 = 8;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ticket {
    pub rowid: i64,
    pub ref_: String,
    pub sujet: String,
    pub fk_soc: Option<i64>,
    pub tiers: Option<String>,
    pub date: Option<String>,
    pub statut: i16,
    pub gravite: Option<String>,
    pub message: Option<String>,
}

pub fn list(pool: &Pool, entity: i64, limit: u32) -> Result<Vec<Ticket>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT t.rowid, t.ref, COALESCE(t.subject, ''), t.fk_soc, s.nom, \
                DATE_FORMAT(t.datec, '%Y-%m-%d %H:%i'), t.fk_statut, t.severity_code, t.message \
         FROM llx_ticket t LEFT JOIN llx_societe s ON s.rowid = t.fk_soc \
         WHERE t.entity = :entity \
         ORDER BY t.datec DESC, t.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    let s = |r: &mysql::Row, i: usize| r.get_opt::<String, _>(i).and_then(|v| v.ok());
    let n = |r: &mysql::Row, i: usize| r.get_opt::<i64, _>(i).and_then(|v| v.ok());
    Ok(rows
        .iter()
        .map(|r| Ticket {
            rowid: n(r, 0).unwrap_or(0),
            ref_: s(r, 1).unwrap_or_default(),
            sujet: s(r, 2).unwrap_or_default(),
            fk_soc: n(r, 3),
            tiers: s(r, 4),
            date: s(r, 5),
            statut: n(r, 6).unwrap_or(0) as i16,
            gravite: s(r, 7),
            message: s(r, 8),
        })
        .collect())
}

/// Cree un ticket. Le sujet est obligatoire.
pub fn creer(
    pool: &Pool,
    entity: i64,
    fk_soc: Option<i64>,
    sujet: &str,
    message: Option<&str>,
    gravite: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let sujet = sujet.trim();
    if sujet.is_empty() {
        return Err("le sujet du ticket est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    if let Some(soc) = fk_soc {
        let existe: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :id",
            params! { "entity" => entity, "id" => soc },
        )?;
        if existe.is_none() {
            return Err(format!("le tiers {soc} n'existe pas dans l'entite {entity}").into());
        }
    }
    for essai in 0..5u32 {
        let reference = reference(&mut conn, entity, essai)?;
        let insere = conn.exec_drop(
            // `track_id` est NOT NULL sans defaut : Dolibarr y met un jeton
            // de suivi. On reprend la reference, unique par construction.
            "INSERT INTO llx_ticket \
             (entity, ref, track_id, fk_soc, subject, message, fk_statut, \
              severity_code, type_code, datec) \
             VALUES (:entity, :ref, :ref, :soc, :sujet, :message, :statut, \
              :gravite, 'ISSUE', NOW())",
            params! {
                "entity" => entity,
                "ref" => reference.as_str(),
                "soc" => fk_soc,
                "sujet" => sujet,
                "message" => message,
                "statut" => STATUT_NOUVEAU,
                "gravite" => gravite.unwrap_or("NORMAL"),
            },
        );
        match insere {
            Ok(()) => return Ok(conn.last_insert_id() as i64),
            Err(e) if is_duplicate(&e) && essai < 4 => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une reference de ticket unique".into())
}

fn reference(
    conn: &mut mysql::PooledConn,
    entity: i64,
    offset: u32,
) -> Result<String, Box<dyn Error>> {
    let periode: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%y%m')")?;
    let periode = periode.unwrap_or_else(|| "0001".to_string());
    let prefixe = format!("{PREFIX}{periode}-");
    let dernier: Option<Option<i64>> = conn.exec_first(
        "SELECT MAX(CAST(SUBSTRING(ref FROM 8) AS SIGNED)) \
         FROM llx_ticket WHERE entity = :entity AND ref LIKE :motif",
        params! { "entity" => entity, "motif" => format!("{prefixe}%") },
    )?;
    let n = dernier.flatten().unwrap_or(0) + 1 + offset as i64;
    Ok(format!("{prefixe}{n:04}"))
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

/// Ferme un ticket.
pub fn fermer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_ticket SET fk_statut = :statut, date_close = NOW() \
         WHERE entity = :entity AND rowid = :id",
        params! { "statut" => STATUT_FERME, "entity" => entity, "id" => rowid },
    )?;
    Ok(())
}

/// Un ticket precis.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Ticket>, Box<dyn Error>> {
    Ok(list(pool, entity, 10_000)?
        .into_iter()
        .find(|t| t.rowid == rowid))
}

/// Change le statut d'un ticket (0 nouveau, 1 en cours, 8 ferme).
pub fn changer_statut(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    statut: i16,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    if statut == STATUT_FERME {
        conn.exec_drop(
            "UPDATE llx_ticket SET fk_statut = :statut, date_close = NOW() \
             WHERE entity = :entity AND rowid = :rowid",
            params! { "statut" => statut, "entity" => entity, "rowid" => rowid },
        )?;
    } else {
        conn.exec_drop(
            "UPDATE llx_ticket SET fk_statut = :statut WHERE entity = :entity AND rowid = :rowid",
            params! { "statut" => statut, "entity" => entity, "rowid" => rowid },
        )?;
    }
    Ok(())
}
