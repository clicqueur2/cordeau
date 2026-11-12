//! Menage des tests d'integration.
//!
//! `db::invoice::delete` refuse les factures validees — c'est la regle de
//! production, et un test ne doit pas la contourner. Ces fonctions effacent
//! donc directement les lignes creees par le test.

#![allow(dead_code)]
use mysql::params;
use mysql::prelude::Queryable;

/// Efface une facture (ou un avoir) et ses dependances.
pub fn facture(pool: &db::Pool, entity: i64, id: i64) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = pool.get_conn()?;
    let paiements: Vec<i64> = conn.exec(
        "SELECT fk_paiement FROM llx_paiement_facture WHERE fk_facture = :f",
        params! { "f" => id },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_paiement_facture WHERE fk_facture = :f",
        params! { "f" => id },
    )?;
    for p in paiements {
        conn.exec_drop(
            "DELETE FROM llx_paiement WHERE entity = :e AND rowid = :p",
            params! { "e" => entity, "p" => p },
        )?;
    }
    conn.exec_drop(
        "DELETE FROM llx_facturedet WHERE fk_facture = :f",
        params! { "f" => id },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_element_element WHERE targettype = 'facture' AND fk_target = :f",
        params! { "f" => id },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_accounting_bookkeeping WHERE entity = :e AND fk_doc = :f",
        params! { "e" => entity, "f" => id },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_facture WHERE entity = :e AND rowid = :f",
        params! { "e" => entity, "f" => id },
    )?;
    Ok(())
}
