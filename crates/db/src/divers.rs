//! Lectures transverses : reglements, contrats, listes filtrees de tiers.
//!
//! Ces ecrans n'ont pas de logique propre : ils montrent ce que Dolibarr a
//! deja en base, sans le recalculer.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reglement {
    pub rowid: i64,
    pub ref_: String,
    pub date: Option<String>,
    pub montant: Decimal,
    pub mode: Option<String>,
    pub tiers: Option<String>,
    pub factures: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contrat {
    pub rowid: i64,
    pub ref_: String,
    pub tiers: Option<String>,
    pub date: Option<String>,
    pub statut: i16,
    pub total_ttc: Option<Decimal>,
}

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

/// Reglements enregistres, du plus recent au plus ancien, avec le tiers et les
/// factures qu'ils lettrent.
pub fn reglements(pool: &Pool, entity: i64, limit: u32) -> Result<Vec<Reglement>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT p.rowid, p.ref, DATE_FORMAT(p.datep, '%Y-%m-%d'), \
                CAST(p.amount AS CHAR), t.code AS mode, s.nom, \
                GROUP_CONCAT(DISTINCT f.ref ORDER BY f.ref SEPARATOR ', ') \
         FROM llx_paiement p \
         LEFT JOIN llx_c_paiement t ON t.id = p.fk_paiement \
         LEFT JOIN llx_paiement_facture pf ON pf.fk_paiement = p.rowid \
         LEFT JOIN llx_facture f ON f.rowid = pf.fk_facture \
         LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE p.entity = :entity \
         GROUP BY p.rowid, p.ref, p.datep, p.amount, t.code, s.nom \
         ORDER BY p.datep DESC, p.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Reglement {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            date: opt_s(&r, 2),
            montant: opt_dec(&r, 3).unwrap_or_default(),
            mode: opt_s(&r, 4),
            tiers: opt_s(&r, 5),
            factures: opt_s(&r, 6),
        })
        .collect())
}

/// Contrats clients (`llx_contrat`).
pub fn contrats(pool: &Pool, entity: i64, limit: u32) -> Result<Vec<Contrat>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT c.rowid, c.ref, s.nom, DATE_FORMAT(c.date_contrat, '%Y-%m-%d'), \
                c.statut, CAST(c.total_ttc AS CHAR) \
         FROM llx_contrat c LEFT JOIN llx_societe s ON s.rowid = c.fk_soc \
         WHERE c.entity = :entity \
         ORDER BY c.date_contrat DESC, c.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Contrat {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            tiers: opt_s(&r, 2),
            date: opt_s(&r, 3),
            statut: opt_i(&r, 4).unwrap_or(0) as i16,
            total_ttc: opt_dec(&r, 5),
        })
        .collect())
}

/// Tiers filtres : `prospect` (`client = 2` ou `3`) ou `fournisseur`.
///
/// Dolibarr encode le role dans deux colonnes : `client` (0 aucun, 1 client,
/// 2 prospect, 3 client et prospect) et `fournisseur` (0/1).
pub fn tiers_filtres(
    pool: &Pool,
    entity: i64,
    filtre: &str,
    limit: u32,
) -> Result<Vec<crate::party::Party>, Box<dyn Error>> {
    let condition = match filtre {
        "prospect" => "client IN (2, 3)",
        "fournisseur" => "fournisseur = 1",
        "client" => "client IN (1, 3)",
        _ => "1 = 1",
    };
    crate::party::list_filtre(pool, entity, condition, limit)
}

/// Une expedition (`llx_expedition`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Expedition {
    pub rowid: i64,
    pub ref_: String,
    pub tiers: Option<String>,
    pub date: Option<String>,
    pub livraison: Option<String>,
    pub statut: i16,
}

/// Expeditions, de la plus recente a la plus ancienne.
pub fn expeditions(
    pool: &Pool,
    entity: i64,
    limit: u32,
) -> Result<Vec<Expedition>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT e.rowid, e.ref, s.nom, DATE_FORMAT(e.date_creation, '%Y-%m-%d'), \
                DATE_FORMAT(e.date_delivery, '%Y-%m-%d'), e.fk_statut \
         FROM llx_expedition e LEFT JOIN llx_societe s ON s.rowid = e.fk_soc \
         WHERE e.entity = :entity \
         ORDER BY e.date_creation DESC, e.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Expedition {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            tiers: opt_s(&r, 2),
            date: opt_s(&r, 3),
            livraison: opt_s(&r, 4),
            statut: opt_i(&r, 5).unwrap_or(0) as i16,
        })
        .collect())
}
/// Une commande fournisseur, en lecture.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandeFourn {
    pub rowid: i64,
    pub ref_: String,
    pub ref_supplier: Option<String>,
    pub tiers: Option<String>,
    pub date: Option<String>,
    pub statut: i16,
    pub total_ttc: Option<Decimal>,
}

/// Commandes fournisseurs (llx_commande_fournisseur).
pub fn commandes_fournisseurs(
    pool: &Pool,
    entity: i64,
    limit: u32,
) -> Result<Vec<CommandeFourn>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT c.rowid, c.ref, c.ref_supplier, s.nom, \
                DATE_FORMAT(c.date_commande, '%Y-%m-%d'), c.fk_statut, \
                CAST(c.total_ttc AS CHAR) \
         FROM llx_commande_fournisseur c LEFT JOIN llx_societe s ON s.rowid = c.fk_soc \
         WHERE c.entity = :entity \
         ORDER BY c.date_commande DESC, c.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| CommandeFourn {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            ref_supplier: opt_s(&r, 2),
            tiers: opt_s(&r, 3),
            date: opt_s(&r, 4),
            statut: opt_i(&r, 5).unwrap_or(0) as i16,
            total_ttc: opt_dec(&r, 6),
        })
        .collect())
}
