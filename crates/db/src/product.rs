//! Produits et services (`llx_product`).
//!
//! Les montants sont lus avec `CAST(... AS CHAR)` : le schema les stocke en
//! `double(24,8)`, on refuse de les faire transiter par un flottant.

use mysql::prelude::*;
use mysql::{params, Pool, Row};
use rust_decimal::Decimal;
use std::error::Error;
use std::str::FromStr;

pub const TYPE_PRODUCT: i16 = 0;
pub const TYPE_SERVICE: i16 = 1;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Product {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub label: String,
    pub price: Option<Decimal>,
    pub price_ttc: Option<Decimal>,
    pub price_base_type: String,
    pub tva_tx: Option<Decimal>,
    pub fk_unit: Option<i64>,
    pub fk_product_type: i16,
    pub tosell: i16,
    pub tobuy: i16,
    pub hidden: i16,
    pub barcode: Option<String>,
    pub datec: Option<String>,
    /// Description commerciale (`description`).
    pub description: Option<String>,
}

impl Product {
    pub fn est_service(&self) -> bool {
        self.fk_product_type == TYPE_SERVICE
    }

    /// Le produit est-il propose a la vente (`tosell`) ?
    pub fn on_sell(&self) -> bool {
        self.tosell != 0
    }
}

#[derive(Debug, Clone)]
pub struct NewProduct {
    pub ref_: String,
    pub label: String,
    pub price: Option<Decimal>,
    pub tva_tx: Option<Decimal>,
    pub price_base_type: String,
    pub fk_unit: Option<i64>,
    pub fk_product_type: i16,
    pub tosell: i16,
    pub tobuy: i16,
    pub barcode: Option<String>,
}

impl NewProduct {
    pub fn new(ref_: &str, label: &str) -> Self {
        Self {
            ref_: ref_.to_string(),
            label: label.to_string(),
            price: None,
            tva_tx: None,
            price_base_type: "HT".to_string(),
            fk_unit: None,
            fk_product_type: TYPE_PRODUCT,
            tosell: 1,
            tobuy: 1,
            barcode: None,
        }
    }
}

const SELECT_COLS: &str = "rowid, entity, ref, label, \
     CAST(price AS CHAR), CAST(price_ttc AS CHAR), price_base_type, \
     CAST(tva_tx AS CHAR), fk_unit, fk_product_type, tosell, tobuy, hidden, barcode, \
     DATE_FORMAT(datec, '%Y-%m-%d %H:%i:%s'), description";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Product {
    Product {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        label: opt_s(&row, 3).unwrap_or_default(),
        price: opt_dec(&row, 4),
        price_ttc: opt_dec(&row, 5),
        price_base_type: opt_s(&row, 6).unwrap_or_else(|| "HT".to_string()),
        tva_tx: opt_dec(&row, 7),
        fk_unit: opt_i(&row, 8),
        fk_product_type: opt_i(&row, 9).unwrap_or(0) as i16,
        tosell: opt_i(&row, 10).unwrap_or(1) as i16,
        tobuy: opt_i(&row, 11).unwrap_or(1) as i16,
        hidden: opt_i(&row, 12).unwrap_or(0) as i16,
        barcode: opt_s(&row, 13),
        description: opt_s(&row, 15),
        datec: opt_s(&row, 14),
    }
}

pub fn count(pool: &Pool, entity: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_product WHERE entity = ?",
        (entity,),
    )?;
    Ok(n.unwrap_or(0))
}

pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Product>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_product WHERE entity = ? \
         ORDER BY ref ASC, rowid ASC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limit, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Product>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} FROM llx_product WHERE entity = ? AND rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

pub fn by_ref(
    pool: &Pool,
    entity: i64,
    reference: &str,
) -> Result<Option<Product>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} FROM llx_product WHERE entity = ? AND ref = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, reference))?;
    Ok(row.map(map_row))
}

/// Identifiant d'une unite de `llx_c_units` a partir de son code (`KG`, `M`, `H87`...).
pub fn unit_id(pool: &Pool, code: &str) -> Result<Option<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let id: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_c_units WHERE code = ? AND active = 1 LIMIT 1",
        (code,),
    )?;
    Ok(id)
}

pub fn create(pool: &Pool, entity: i64, new: &NewProduct) -> Result<i64, Box<dyn Error>> {
    if new.ref_.trim().is_empty() {
        return Err("la reference est obligatoire".into());
    }
    if new.label.trim().is_empty() {
        return Err("le libelle est obligatoire".into());
    }

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_product \
         (entity, ref, label, price, price_ttc, price_base_type, tva_tx, fk_unit, \
          fk_product_type, tosell, tobuy, barcode, hidden, datec) \
         VALUES (:entity, :ref, :label, :price, :price_ttc, :price_base_type, :tva_tx, \
          :fk_unit, :fk_product_type, :tosell, :tobuy, :barcode, 0, NOW())",
        params! {
            "entity" => entity,
            "ref" => new.ref_.trim(),
            "label" => new.label.trim(),
            "price" => new.price.map(|d| d.to_string()),
            // Meme arrondi que update : deux fonctions d'arrondi pour la
            // meme colonne donnaient deux prix TTC selon le chemin (releve
            // dans EXTRACTIONS/produit.md).
            "price_ttc" => new.price.map(|d| {
                let taux = new.tva_tx.unwrap_or_default() / Decimal::from(100);
                money::round_unit_price(d * (Decimal::ONE + taux)).to_string()
            }),
            "price_base_type" => new.price_base_type.as_str(),
            "tva_tx" => new.tva_tx.map(|d| d.to_string()),
            "fk_unit" => new.fk_unit,
            "fk_product_type" => new.fk_product_type,
            "tosell" => new.tosell,
            "tobuy" => new.tobuy,
            "barcode" => new.barcode.as_deref(),
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

pub fn set_hidden(pool: &Pool, entity: i64, rowid: i64, hidden: i16) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_product SET hidden = ?, tms = NOW() WHERE entity = ? AND rowid = ?",
        (hidden, entity, rowid),
    )?;
    Ok(())
}

/// Suppression physique : reservee aux bases de test.
pub fn delete(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_product_price WHERE fk_product = ?",
        (rowid,),
    )?;
    conn.exec_drop(
        "DELETE FROM llx_product WHERE entity = ? AND rowid = ?",
        (entity, rowid),
    )?;
    Ok(())
}

/// Met a jour les champs editables d'un produit. La reference ne bouge pas :
/// elle sert de cle dans les documents deja emis.
#[allow(clippy::too_many_arguments)]
pub fn update(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    label: &str,
    price: Decimal,
    tva_tx: Decimal,
    description: Option<&str>,
    on_sell: bool,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_product SET label = :label, price = :prix, price_ttc = :prix_ttc, \
          tva_tx = :tva, description = :description, tosell = :vente, tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "label" => label,
            "prix" => price.to_string(),
            "prix_ttc" => money::round_unit_price(price * (Decimal::ONE + tva_tx / Decimal::from(100))).to_string(),
            "tva" => tva_tx.to_string(),
            "description" => description,
            "vente" => if on_sell { 1 } else { 0 },
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}
