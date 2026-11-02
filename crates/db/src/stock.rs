//! Stock : entrepots, niveaux et mouvements.
//!
//! Dolibarr tient le stock dans `llx_product_stock` (une ligne par produit et
//! par entrepot) et l'historique dans `llx_stock_mouvement`. Un mouvement
//! met a jour le niveau dans la meme transaction : le stock affiche est
//! toujours la somme des mouvements, jamais une valeur saisie a part.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, TxOpts};
use rust_decimal::Decimal;

/// Entree en stock.
pub const TYPE_ENTREE: i16 = 0;
/// Sortie de stock.
pub const TYPE_SORTIE: i16 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entrepot {
    pub rowid: i64,
    pub ref_: String,
    pub lieu: Option<String>,
    pub ville: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Niveau {
    pub fk_product: i64,
    pub ref_produit: String,
    pub label_produit: String,
    pub fk_entrepot: i64,
    pub entrepot: String,
    pub quantite: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mouvement {
    pub rowid: i64,
    pub date: Option<String>,
    pub ref_produit: String,
    pub entrepot: String,
    pub quantite: Decimal,
    pub type_: i16,
    pub label: Option<String>,
}

fn opt_s(row: &mysql::Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &mysql::Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &mysql::Row, i: usize) -> Decimal {
    opt_s(row, i)
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .map(money::round_money)
        .unwrap_or_default()
}

pub fn entrepots(pool: &Pool, entity: i64) -> Result<Vec<Entrepot>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT rowid, ref, lieu, town FROM llx_entrepot \
         WHERE entity = :entity ORDER BY ref ASC",
        params! { "entity" => entity },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Entrepot {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            lieu: opt_s(&r, 2),
            ville: opt_s(&r, 3),
        })
        .collect())
}

pub fn creer_entrepot(
    pool: &Pool,
    entity: i64,
    ref_: &str,
    lieu: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let ref_ = ref_.trim();
    if ref_.is_empty() {
        return Err("la reference de l'entrepot est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_entrepot WHERE entity = :entity AND ref = :ref",
        params! { "entity" => entity, "ref" => ref_ },
    )?;
    if deja.is_some() {
        return Err(format!("un entrepot porte deja la reference {ref_}").into());
    }
    conn.exec_drop(
        "INSERT INTO llx_entrepot (ref, entity, datec, lieu) VALUES (:ref, :entity, NOW(), :lieu)",
        params! { "ref" => ref_, "entity" => entity, "lieu" => lieu },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Niveaux de stock, produits et entrepots joints.
pub fn niveaux(pool: &Pool, entity: i64) -> Result<Vec<Niveau>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT ps.fk_product, COALESCE(p.ref, ''), COALESCE(p.label, ''), \
                ps.fk_entrepot, COALESCE(e.ref, ''), CAST(ps.reel AS CHAR) \
         FROM llx_product_stock ps \
         JOIN llx_product p ON p.rowid = ps.fk_product \
         JOIN llx_entrepot e ON e.rowid = ps.fk_entrepot \
         WHERE p.entity = :entity \
         ORDER BY p.ref ASC, e.ref ASC",
        params! { "entity" => entity },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Niveau {
            fk_product: opt_i(&r, 0).unwrap_or(0),
            ref_produit: opt_s(&r, 1).unwrap_or_default(),
            label_produit: opt_s(&r, 2).unwrap_or_default(),
            fk_entrepot: opt_i(&r, 3).unwrap_or(0),
            entrepot: opt_s(&r, 4).unwrap_or_default(),
            quantite: opt_dec(&r, 5),
        })
        .collect())
}

/// Derniers mouvements, du plus recent au plus ancien.
pub fn mouvements(pool: &Pool, entity: i64, limit: u32) -> Result<Vec<Mouvement>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT m.rowid, DATE_FORMAT(m.datem, '%Y-%m-%d'), COALESCE(p.ref, ''), \
                COALESCE(e.ref, ''), CAST(m.value AS CHAR), m.type_mouvement, m.label \
         FROM llx_stock_mouvement m \
         JOIN llx_product p ON p.rowid = m.fk_product \
         LEFT JOIN llx_entrepot e ON e.rowid = m.fk_entrepot \
         WHERE p.entity = :entity \
         ORDER BY m.datem DESC, m.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Mouvement {
            rowid: opt_i(&r, 0).unwrap_or(0),
            date: opt_s(&r, 1),
            ref_produit: opt_s(&r, 2).unwrap_or_default(),
            entrepot: opt_s(&r, 3).unwrap_or_default(),
            quantite: opt_dec(&r, 4),
            type_: opt_i(&r, 5).unwrap_or(0) as i16,
            label: opt_s(&r, 6),
        })
        .collect())
}

/// Enregistre un mouvement et met a jour le niveau dans la meme transaction.
///
/// `quantite` est toujours positive : c'est `type_` qui donne le sens.
pub fn mouvement(
    pool: &Pool,
    entity: i64,
    fk_product: i64,
    fk_entrepot: i64,
    quantite: Decimal,
    type_: i16,
    label: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    if quantite <= Decimal::ZERO {
        return Err("la quantite d'un mouvement doit etre positive".into());
    }
    let mut conn = pool.get_conn()?;

    let produit: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_product WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => fk_product },
    )?;
    if produit.is_none() {
        return Err(format!("le produit {fk_product} n'existe pas dans l'entite {entity}").into());
    }
    let entrepot: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_entrepot WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => fk_entrepot },
    )?;
    if entrepot.is_none() {
        return Err(format!("l'entrepot {fk_entrepot} n'existe pas").into());
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    tx.exec_drop(
        "INSERT INTO llx_stock_mouvement \
         (datem, fk_product, fk_entrepot, value, type_mouvement, label) \
         VALUES (NOW(), :produit, :entrepot, :valeur, :type, :label)",
        params! {
            "produit" => fk_product,
            "entrepot" => fk_entrepot,
            "valeur" => quantite.to_string(),
            "type" => type_,
            "label" => label,
        },
    )?;
    let id = tx.last_insert_id().unwrap_or(0) as i64;

    let niveau: Option<i64> = tx.exec_first(
        "SELECT rowid FROM llx_product_stock \
         WHERE fk_product = :produit AND fk_entrepot = :entrepot",
        params! { "produit" => fk_product, "entrepot" => fk_entrepot },
    )?;
    let signe = if type_ == TYPE_ENTREE {
        quantite
    } else {
        -quantite
    };
    match niveau {
        Some(rowid) => {
            tx.exec_drop(
                "UPDATE llx_product_stock SET reel = reel + :delta WHERE rowid = :id",
                params! { "delta" => signe.to_string(), "id" => rowid },
            )?;
        }
        None => {
            tx.exec_drop(
                "INSERT INTO llx_product_stock (fk_product, fk_entrepot, reel) \
                 VALUES (:produit, :entrepot, :reel)",
                params! {
                    "produit" => fk_product,
                    "entrepot" => fk_entrepot,
                    "reel" => signe.to_string(),
                },
            )?;
        }
    }

    tx.commit()?;
    Ok(id)
}

/// Quantite disponible d'un produit dans un entrepot.
pub fn quantite(pool: &Pool, fk_product: i64, fk_entrepot: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let brut: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(reel, 0) AS CHAR) FROM llx_product_stock \
         WHERE fk_product = :produit AND fk_entrepot = :entrepot",
        params! { "produit" => fk_product, "entrepot" => fk_entrepot },
    )?;
    Ok(brut
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .map(money::round_money)
        .unwrap_or(Decimal::ZERO))
}
