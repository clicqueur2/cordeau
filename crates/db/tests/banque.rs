//! Comptes bancaires : creation, reference unique, solde deduit des ecritures.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

const ENTITY: i64 = 1;
const REF: &str = "TSTBQ";

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

fn nettoyer(pool: &Pool) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_bank_account WHERE entity = :entity AND ref = :ref",
        params! { "entity" => ENTITY, "ref" => REF },
    )?;
    Ok(())
}

#[test]
fn cree_un_compte_et_calcule_son_solde() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;

    let id = db::banque::creer_compte(
        &pool,
        ENTITY,
        REF,
        "Compte de test",
        Some("Banque de test"),
        Some("FR7600000000000000000000000"),
        Some("TESTFRPP"),
    )?;

    let comptes = db::banque::comptes(&pool, ENTITY)?;
    let compte = comptes
        .iter()
        .find(|c| c.rowid == id)
        .expect("le compte cree est relu");
    assert_eq!(compte.ref_, REF);
    assert!(!compte.clos);

    // Un compte neuf n'a aucune ecriture : le solde est nul, pas absent.
    assert_eq!(db::banque::solde(&pool, ENTITY, id)?, Decimal::ZERO);

    // La reference est unique par entite.
    assert!(
        db::banque::creer_compte(&pool, ENTITY, REF, "Doublon", None, None, None).is_err(),
        "deux comptes de meme reference doivent etre refuses"
    );

    nettoyer(&pool)?;
    Ok(())
}
