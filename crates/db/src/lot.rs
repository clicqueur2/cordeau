//! Lots et series (`llx_product_lot`) : la tracabilite d'un produit.
//!
//! Un lot porte un numero, deux dates qui engagent (a consommer avant, a vendre
//! avant) et un compteur de qualite. Il ne porte **pas** de quantite : le stock
//! d'un lot est la somme des mouvements qui le visent, comme tout le reste. On
//! ne stocke donc pas de doublon de verite.
//!
//! Une date illisible est refusee : « 31/02/2026 » ne doit pas entrer en base
//! sous pretexte qu'elle ressemble a une date.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lot {
    pub rowid: i64,
    /// Reference du produit porte par le lot.
    pub ref_produit: String,
    pub libelle_produit: String,
    pub batch: Option<String>,
    pub eatby: Option<String>,
    pub sellby: Option<String>,
    pub note_public: Option<String>,
    pub datec: Option<String>,
}

/// Un lot a creer.
#[derive(Debug, Clone, Default)]
pub struct NouveauLot {
    pub fk_product: i64,
    pub batch: String,
    pub eatby: Option<String>,
    pub sellby: Option<String>,
    pub note_public: Option<String>,
}

/// Une date `AAAA-MM-JJ` qui existe vraiment.
pub fn date_valide(valeur: &str) -> bool {
    let v = valeur.trim();
    if v.len() != 10 {
        return false;
    }
    let octets = v.as_bytes();
    if octets[4] != b'-' || octets[7] != b'-' {
        return false;
    }
    let (annee, mois, jour) = (
        v[..4].parse::<i32>().ok(),
        v[5..7].parse::<u32>().ok(),
        v[8..].parse::<u32>().ok(),
    );
    let (Some(annee), Some(mois), Some(jour)) = (annee, mois, jour) else {
        return false;
    };
    if !(1..=12).contains(&mois) || jour == 0 {
        return false;
    }
    let bissextile = (annee % 4 == 0 && annee % 100 != 0) || annee % 400 == 0;
    let jours = match mois {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if bissextile {
                29
            } else {
                28
            }
        }
    };
    jour <= jours
}

/// Les lots de l'entite, du plus recent au plus ancien.
pub fn list(pool: &Pool, entity: i64, limite: u32) -> Result<Vec<Lot>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT l.rowid, COALESCE(p.ref, ''), COALESCE(p.label, ''), l.batch, \
                DATE_FORMAT(l.eatby, '%Y-%m-%d'), DATE_FORMAT(l.sellby, '%Y-%m-%d'), \
                l.note_public, DATE_FORMAT(l.datec, '%Y-%m-%d') \
         FROM llx_product_lot l \
         LEFT JOIN llx_product p ON p.rowid = l.fk_product \
         WHERE l.entity = :entity \
         ORDER BY l.rowid DESC LIMIT :limite",
        params! { "entity" => entity, "limite" => limite.clamp(1, 1000) },
    )?;
    let s = |r: &mysql::Row, i: usize| r.get_opt::<String, _>(i).and_then(|v| v.ok());
    Ok(rows
        .into_iter()
        .map(|r| Lot {
            rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
            ref_produit: s(&r, 1).unwrap_or_default(),
            libelle_produit: s(&r, 2).unwrap_or_default(),
            batch: s(&r, 3),
            eatby: s(&r, 4),
            sellby: s(&r, 5),
            note_public: s(&r, 6),
            datec: s(&r, 7),
        })
        .collect())
}

/// Cree un lot. Refuse un produit absent, un numero vide et une date impossible.
pub fn creer(pool: &Pool, entity: i64, nouveau: &NouveauLot) -> Result<i64, Box<dyn Error>> {
    let batch = nouveau.batch.trim();
    if batch.is_empty() {
        return Err("le numero de lot est obligatoire".into());
    }
    for (nom, valeur) in [("eatby", &nouveau.eatby), ("sellby", &nouveau.sellby)] {
        if let Some(v) = valeur.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            if !date_valide(v) {
                return Err(
                    format!("{nom} : « {v} » n'est pas une date valide (AAAA-MM-JJ)").into(),
                );
            }
        }
    }

    let mut conn = pool.get_conn()?;
    let produit: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_product WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => nouveau.fk_product },
    )?;
    if produit.is_none() {
        return Err(format!(
            "le produit {} n'existe pas dans l'entite {entity}",
            nouveau.fk_product
        )
        .into());
    }

    // Deux lots du meme numero pour le meme produit rendraient la tracabilite
    // inutile : le numero est ce qui distingue.
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_product_lot \
         WHERE entity = :entity AND fk_product = :produit AND batch = :batch",
        params! { "entity" => entity, "produit" => nouveau.fk_product, "batch" => batch },
    )?;
    if deja.is_some() {
        return Err(format!("le lot « {batch} » existe deja pour ce produit").into());
    }

    conn.exec_drop(
        "INSERT INTO llx_product_lot \
         (entity, fk_product, batch, eatby, sellby, note_public, datec, fk_user_creat) \
         VALUES (:entity, :produit, :batch, :eatby, :sellby, :note, NOW(), \
          (SELECT MIN(rowid) FROM llx_user))",
        params! {
            "entity" => entity,
            "produit" => nouveau.fk_product,
            "batch" => batch,
            "eatby" => nouveau.eatby.as_deref().map(str::trim).filter(|v| !v.is_empty()),
            "sellby" => nouveau.sellby.as_deref().map(str::trim).filter(|v| !v.is_empty()),
            "note" => nouveau.note_public.as_deref(),
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_dates_impossibles_sont_refusees() {
        assert!(date_valide("2026-11-10"));
        assert!(date_valide("2024-02-29"), "2024 est bissextile");
        assert!(!date_valide("2026-02-29"), "2026 ne l'est pas");
        assert!(!date_valide("2026-13-01"));
        assert!(!date_valide("2026-00-10"));
        assert!(!date_valide("2026-11-31"));
        assert!(!date_valide("10/11/2026"));
        assert!(!date_valide(""));
    }
}
