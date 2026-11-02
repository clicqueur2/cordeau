//! Commandes clients (`llx_commande` + `llx_commandedet`).
//!
//! La commande est le maillon entre le devis signe et la facture. Elle porte
//! les memes lignes, avec leurs unites, et se numerote `CO` + AAAMM + 4 chiffres
//! comme le fait `mod_commande_marbre`.

use std::error::Error;
use std::str::FromStr;

use money::{line_totals, sum, LineInput};
use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

pub const PREFIX: &str = "CO";
pub const STATUT_BROUILLON: i16 = 0;
pub const STATUT_VALIDEE: i16 = 1;
pub const STATUT_LIVREE: i16 = 2;
pub const STATUT_ANNULEE: i16 = 3;
const PRODUCT_TYPE_SECTION: i16 = 9;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Commande {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub fk_soc: i64,
    pub date_commande: Option<String>,
    pub fk_statut: i16,
    pub total_ht: Option<Decimal>,
    pub total_tva: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub fk_cond_reglement: Option<i64>,
    pub deposit_percent: Option<Decimal>,
}

impl Commande {
    pub fn est_brouillon(&self) -> bool {
        self.fk_statut == STATUT_BROUILLON
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandeLine {
    pub rowid: i64,
    pub fk_commande: i64,
    pub fk_product: Option<i64>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub tva_tx: Option<Decimal>,
    pub qty: Option<Decimal>,
    pub remise_percent: Option<Decimal>,
    pub subprice: Option<Decimal>,
    pub total_ht: Option<Decimal>,
    pub total_tva: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    pub product_type: i16,
    pub rang: i32,
    pub fk_unit: Option<i64>,
}

impl CommandeLine {
    pub fn is_section(&self) -> bool {
        self.product_type == PRODUCT_TYPE_SECTION
    }
}

const SELECT_COLS: &str = "rowid, entity, ref, fk_soc, \
     DATE_FORMAT(date_commande, '%Y-%m-%d'), fk_statut, \
     CAST(total_ht AS CHAR), CAST(total_tva AS CHAR), CAST(total_ttc AS CHAR), \
     note_private, note_public, fk_cond_reglement, CAST(deposit_percent AS CHAR)";

const LINE_COLS: &str = "d.rowid, d.fk_commande, d.fk_product, d.label, d.description, \
     CAST(d.tva_tx AS CHAR), CAST(d.qty AS CHAR), CAST(d.remise_percent AS CHAR), \
     CAST(d.subprice AS CHAR), CAST(d.total_ht AS CHAR), CAST(d.total_tva AS CHAR), \
     CAST(d.total_ttc AS CHAR), d.product_type, d.rang, d.fk_unit";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Commande {
    Commande {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        fk_soc: opt_i(&row, 3).unwrap_or(0),
        date_commande: opt_s(&row, 4),
        fk_statut: opt_i(&row, 5).unwrap_or(0) as i16,
        total_ht: opt_dec(&row, 6),
        total_tva: opt_dec(&row, 7),
        total_ttc: opt_dec(&row, 8),
        note_private: opt_s(&row, 9),
        note_public: opt_s(&row, 10),
        fk_cond_reglement: opt_i(&row, 11),
        deposit_percent: opt_dec(&row, 12),
    }
}

pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Commande>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_commande WHERE entity = ? \
         ORDER BY date_commande DESC, rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limit, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Commande>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} FROM llx_commande WHERE entity = ? AND rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

pub fn lines(pool: &Pool, entity: i64, rowid: i64) -> Result<Vec<CommandeLine>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {LINE_COLS} FROM llx_commandedet d \
         JOIN llx_commande c ON c.rowid = d.fk_commande \
         WHERE c.entity = ? AND d.fk_commande = ? ORDER BY d.rang ASC, d.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, rowid))?;
    Ok(rows
        .into_iter()
        .map(|row| CommandeLine {
            rowid: opt_i(&row, 0).unwrap_or(0),
            fk_commande: opt_i(&row, 1).unwrap_or(0),
            fk_product: opt_i(&row, 2),
            label: opt_s(&row, 3),
            description: opt_s(&row, 4),
            tva_tx: opt_dec(&row, 5),
            qty: opt_dec(&row, 6),
            remise_percent: opt_dec(&row, 7),
            subprice: opt_dec(&row, 8),
            total_ht: opt_dec(&row, 9),
            total_tva: opt_dec(&row, 10),
            total_ttc: opt_dec(&row, 11),
            product_type: opt_i(&row, 12).unwrap_or(0) as i16,
            rang: opt_i(&row, 13).unwrap_or(0) as i32,
            fk_unit: opt_i(&row, 14),
        })
        .collect())
}

fn next_ref<Q>(conn: &mut Q, entity: i64, offset: u32) -> Result<String, Box<dyn Error>>
where
    Q: mysql::prelude::Queryable,
{
    let periode: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%y%m')")?;
    let periode = periode.unwrap_or_else(|| "0001".to_string());
    let prefixe = format!("{PREFIX}{periode}-");
    let sql = "SELECT MAX(CAST(SUBSTRING(ref FROM 8) AS SIGNED)) \
               FROM llx_commande WHERE entity = :entity AND ref LIKE :motif";
    let dernier: Option<Option<i64>> = conn.exec_first(
        sql,
        params! { "entity" => entity, "motif" => format!("{prefixe}%") },
    )?;
    let n = dernier.flatten().unwrap_or(0) + 1 + offset as i64;
    Ok(format!("{prefixe}{n:04}"))
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

fn is_deadlock(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1213 || s.code == 1205)
}

/// Cree une commande a partir d'un devis valide, en reprenant ses lignes.
///
/// La commande garde la trace du devis dans `llx_element_element`
/// (`propal` -> `commande`) : c'est ce qui permet de savoir, plus tard, quels
/// devis ont ete commandes.
pub fn create_from_proposal(
    pool: &Pool,
    entity: i64,
    fk_propal: i64,
) -> Result<i64, Box<dyn Error>> {
    let devis = crate::proposal::get(pool, entity, fk_propal)?
        .ok_or_else(|| format!("devis {fk_propal} introuvable"))?;
    if devis.fk_statut < 1 {
        return Err("un devis non valide ne se commande pas".into());
    }
    let fk_soc = devis.fk_soc.ok_or("le devis n'a pas de tiers")?;
    let lignes = crate::proposal::lines(pool, entity, fk_propal)?;

    let mut conn = pool.get_conn()?;
    let verrou = format!("dolibarr_rs_commande_{entity}");
    let pris: Option<i64> = conn.exec_first("SELECT GET_LOCK(?, 10)", (verrou.as_str(),))?;
    if pris != Some(1) {
        return Err("verrou de numerotation indisponible (10 s)".into());
    }
    let resultat = creer_sous_verrou(&mut conn, entity, &devis, fk_soc, &lignes);
    let _: Option<i64> = conn.exec_first("SELECT RELEASE_LOCK(?)", (verrou.as_str(),))?;
    resultat
}

fn creer_sous_verrou(
    conn: &mut mysql::PooledConn,
    entity: i64,
    devis: &crate::proposal::Proposal,
    fk_soc: i64,
    lignes: &[crate::proposal::ProposalLine],
) -> Result<i64, Box<dyn Error>> {
    let utiles = |l: &&crate::proposal::ProposalLine| !l.is_section();
    let total_ht = sum(&lignes
        .iter()
        .filter(utiles)
        .map(|l| l.total_ht.unwrap_or_default())
        .collect::<Vec<_>>());
    let total_tva = sum(&lignes
        .iter()
        .filter(utiles)
        .map(|l| l.total_tva.unwrap_or_default())
        .collect::<Vec<_>>());
    let total_ttc = sum(&lignes
        .iter()
        .filter(utiles)
        .map(|l| l.total_ttc.unwrap_or_default())
        .collect::<Vec<_>>());

    for attempt in 0..5u32 {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let reference = next_ref(&mut tx, entity, attempt)?;
        let insere = tx.exec_drop(
            "INSERT INTO llx_commande \
             (entity, ref, fk_soc, date_creation, date_commande, fk_statut, \
              total_ht, total_tva, total_ttc, note_private, note_public, \
              fk_cond_reglement, deposit_percent) \
             VALUES (:entity, :ref, :fk_soc, NOW(), COALESCE(:datep, CURDATE()), :statut, \
              :total_ht, :total_tva, :total_ttc, :note_private, :note_public, \
              :cond, :deposit)",
            params! {
                "entity" => entity,
                "ref" => reference.as_str(),
                "fk_soc" => fk_soc,
                "datep" => devis.datep.as_deref(),
                "statut" => STATUT_BROUILLON,
                "total_ht" => total_ht.to_string(),
                "total_tva" => total_tva.to_string(),
                "total_ttc" => total_ttc.to_string(),
                "note_private" => devis.note_private.as_deref(),
                "note_public" => devis.note_public.as_deref(),
                "cond" => devis.fk_cond_reglement,
                "deposit" => devis.deposit_percent.map(|d| d.to_string()),
            },
        );
        match insere {
            Ok(()) => {
                let rowid = tx.last_insert_id().unwrap_or(0) as i64;
                for (rang, l) in lignes.iter().enumerate() {
                    let t = line_totals(&LineInput::new(
                        &l.qty.unwrap_or_default().to_string(),
                        &l.subprice.unwrap_or_default().to_string(),
                        &l.remise_percent.unwrap_or_default().to_string(),
                        &l.tva_tx.unwrap_or_default().to_string(),
                    ));
                    tx.exec_drop(
                        "INSERT INTO llx_commandedet \
                         (fk_commande, fk_product, label, description, tva_tx, qty, \
                          remise_percent, subprice, price, total_ht, total_tva, \
                          total_ttc, product_type, rang, special_code, fk_unit) \
                         VALUES (:fk_commande, :fk_product, :label, :description, :tva_tx, \
                          :qty, :remise, :subprice, :subprice, :total_ht, :total_tva, \
                          :total_ttc, :product_type, :rang, :special_code, :fk_unit)",
                        params! {
                            "fk_commande" => rowid,
                            "fk_product" => l.fk_product,
                            "label" => l.label.as_deref(),
                            "description" => l.description.as_deref(),
                            "tva_tx" => l.tva_tx.unwrap_or_default().to_string(),
                            "qty" => l.qty.unwrap_or_default().to_string(),
                            "remise" => l.remise_percent.unwrap_or_default().to_string(),
                            "subprice" => l.subprice.unwrap_or_default().to_string(),
                            "total_ht" => t.total_ht.to_string(),
                            "total_tva" => t.total_vat.to_string(),
                            "total_ttc" => t.total_ttc.to_string(),
                            "product_type" => l.product_type,
                            "rang" => rang as i32,
                            "special_code" => l.special_code,
                            "fk_unit" => l.fk_unit,
                        },
                    )?;
                }
                tx.exec_drop(
                    "INSERT INTO llx_element_element \
                     (fk_source, sourcetype, fk_target, targettype) \
                     VALUES (:propal, 'propal', :commande, 'commande')",
                    params! { "propal" => devis.rowid, "commande" => rowid },
                )?;
                tx.commit()?;
                return Ok(rowid);
            }
            Err(e) if (is_duplicate(&e) || is_deadlock(&e)) && attempt < 4 => {
                tx.rollback()?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une reference de commande unique".into())
}

/// Passe une commande de brouillon a validee.
pub fn validate(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_commande WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => return Err(format!("commande {rowid} introuvable").into()),
        Some(s) if s != STATUT_BROUILLON as i64 => {
            return Err("cette commande n'est pas en brouillon".into())
        }
        _ => {}
    }
    let lignes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_commandedet WHERE fk_commande = :rowid",
        params! { "rowid" => rowid },
    )?;
    if lignes.unwrap_or(0) == 0 {
        return Err("une commande sans ligne ne se valide pas".into());
    }
    conn.exec_drop(
        "UPDATE llx_commande SET fk_statut = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}
