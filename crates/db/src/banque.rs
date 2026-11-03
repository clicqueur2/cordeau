//! Banques : comptes (`llx_bank_account`) et ecritures (`llx_bank`).
//!
//! Le solde n'est pas stocke : il se deduit des ecritures, comme dans Dolibarr.
//! Une ecriture est ecrite par les reglements (`db::payment`) ; ce module les
//! relit et permet de tenir les comptes.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;
use std::str::FromStr;

/// Un compte bancaire.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Compte {
    pub rowid: i64,
    pub ref_: String,
    pub label: String,
    pub banque: Option<String>,
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub devise: Option<String>,
    pub clos: bool,
}

/// Une ecriture bancaire, telle qu'elle apparait au releve.
#[derive(Debug, Clone, PartialEq)]
pub struct Ecriture {
    pub rowid: i64,
    pub dateo: Option<String>,
    pub label: Option<String>,
    pub montant: Decimal,
    pub fk_account: Option<i64>,
    pub rapproche: bool,
}

fn opt_s(row: &mysql::Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &mysql::Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &mysql::Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

/// Comptes de l'entite, ouverts d'abord.
pub fn comptes(pool: &Pool, entity: i64) -> Result<Vec<Compte>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT rowid, ref, label, bank, iban_prefix, bic, currency_code, clos \
         FROM llx_bank_account WHERE entity = :entity \
         ORDER BY clos ASC, rowid ASC",
        params! { "entity" => entity },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Compte {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            label: opt_s(&r, 2).unwrap_or_default(),
            banque: opt_s(&r, 3),
            iban: opt_s(&r, 4),
            bic: opt_s(&r, 5),
            devise: opt_s(&r, 6),
            clos: opt_i(&r, 7).unwrap_or(0) != 0,
        })
        .collect())
}

/// Solde d'un compte : somme des ecritures, arrondie au centime.
pub fn solde(pool: &Pool, entity: i64, compte: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let brut: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(b.amount), 0) AS CHAR) FROM llx_bank b \
         JOIN llx_bank_account a ON a.rowid = b.fk_account \
         WHERE a.entity = :entity AND b.fk_account = :compte",
        params! { "entity" => entity, "compte" => compte },
    )?;
    Ok(brut
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .map(money::round_money)
        .unwrap_or(Decimal::ZERO))
}

/// Dernieres ecritures, tous comptes confondus.
pub fn ecritures(pool: &Pool, entity: i64, limit: u32) -> Result<Vec<Ecriture>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT b.rowid, DATE_FORMAT(b.dateo, '%Y-%m-%d'), b.label, \
                CAST(b.amount AS CHAR), b.fk_account, b.rappro \
         FROM llx_bank b JOIN llx_bank_account a ON a.rowid = b.fk_account \
         WHERE a.entity = :entity \
         ORDER BY b.dateo DESC, b.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Ecriture {
            rowid: opt_i(&r, 0).unwrap_or(0),
            dateo: opt_s(&r, 1),
            label: opt_s(&r, 2),
            montant: opt_dec(&r, 3).unwrap_or_default(),
            fk_account: opt_i(&r, 4),
            rapproche: opt_i(&r, 5).unwrap_or(0) != 0,
        })
        .collect())
}

/// Cree un compte bancaire. La reference est obligatoire et unique par entite.
pub fn creer_compte(
    pool: &Pool,
    entity: i64,
    ref_: &str,
    label: &str,
    banque: Option<&str>,
    iban: Option<&str>,
    bic: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let ref_ = ref_.trim();
    if ref_.is_empty() || label.trim().is_empty() {
        return Err("reference et libelle du compte obligatoires".into());
    }
    let mut conn = pool.get_conn()?;
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_bank_account WHERE entity = :entity AND ref = :ref",
        params! { "entity" => entity, "ref" => ref_ },
    )?;
    if deja.is_some() {
        return Err(format!("un compte porte deja la reference {ref_}").into());
    }
    conn.exec_drop(
        "INSERT INTO llx_bank_account \
         (datec, ref, label, entity, bank, iban_prefix, bic, currency_code, clos, \
          courant, fk_pays) \
         VALUES (NOW(), :ref, :label, :entity, :banque, :iban, :bic, 'EUR', 0, 1, 0)",
        params! {
            "ref" => ref_,
            "label" => label.trim(),
            "entity" => entity,
            "banque" => banque,
            "iban" => iban,
            "bic" => bic,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Ligne brute d'un compte bancaire : banque, libelle, IBAN, BIC.
type LigneCompte = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Compte bancaire formate pour un document : banque, IBAN, BIC.
///
/// Renvoie `None` si le compte n'existe pas ou n'a ni IBAN ni libelle utile :
/// un RIB vide vaut mieux qu'un RIB faux.
pub fn compte_libelle(
    pool: &Pool,
    entity: i64,
    compte: i64,
) -> Result<Option<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ligne: Option<LigneCompte> = conn.exec_first(
        "SELECT bank, label, iban_prefix, bic FROM llx_bank_account \
             WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => compte },
    )?;
    let Some((banque, label, iban, bic)) = ligne else {
        return Ok(None);
    };
    let iban = iban.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let mut morceaux: Vec<String> = Vec::new();
    let banque = banque
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty());
    let label = label
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty());
    // Banque puis titulaire : presentation des releves et des factures.
    if let Some(b) = &banque {
        morceaux.push(b.clone());
    }
    if let Some(l) = &label {
        if banque.as_deref().map(|b| b.to_lowercase()) != Some(l.to_lowercase()) {
            morceaux.push(l.clone());
        }
    }
    if let Some(i) = iban {
        morceaux.push(format!("IBAN {i}"));
    }
    if let Some(b) = bic.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        morceaux.push(format!("BIC {b}"));
    }
    if morceaux.len() < 2 {
        return Ok(None);
    }
    Ok(Some(format!("RIB — {}", morceaux.join(" — "))))
}

/// Rapproche une ecriture (ou la retire du rapprochement).
pub fn rapprocher(
    pool: &Pool,
    entity: i64,
    ecriture: i64,
    fait: bool,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_bank b JOIN llx_bank_account a ON a.rowid = b.fk_account \
         SET b.rappro = :fait WHERE a.entity = :entity AND b.rowid = :id",
        params! { "fait" => if fait { 1 } else { 0 }, "entity" => entity, "id" => ecriture },
    )?;
    Ok(())
}

/// Met a jour un compte bancaire. La reference reste, elle identifie le compte
/// dans les ecritures deja passees.
#[allow(clippy::too_many_arguments)]
pub fn modifier_compte(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    label: &str,
    banque: Option<&str>,
    iban: Option<&str>,
    bic: Option<&str>,
    clos: bool,
) -> Result<(), Box<dyn Error>> {
    let label = label.trim();
    if label.is_empty() {
        return Err("le libelle du compte est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_bank_account SET label = :label, bank = :banque, \
          iban_prefix = :iban, bic = :bic, clos = :clos \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "label" => label,
            "banque" => banque,
            "iban" => iban,
            "bic" => bic,
            "clos" => if clos { 1 } else { 0 },
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Un compte precis.
pub fn compte(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Compte>, Box<dyn Error>> {
    Ok(comptes(pool, entity)?
        .into_iter()
        .find(|c| c.rowid == rowid))
}
