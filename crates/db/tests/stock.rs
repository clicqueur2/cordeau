//! Stock : un mouvement met a jour le niveau, dans la meme transaction.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;
use std::str::FromStr;

use db::product::NewProduct;

const ENTITY: i64 = 1;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

fn nettoyer(pool: &Pool, prefixe: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let produits: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_product WHERE entity = :entity AND ref LIKE :motif",
        params! { "entity" => ENTITY, "motif" => format!("{prefixe}%") },
    )?;
    for p in produits {
        conn.exec_drop(
            "DELETE FROM llx_stock_mouvement WHERE fk_product = :p",
            params! { "p" => p },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_product_stock WHERE fk_product = :p",
            params! { "p" => p },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_product WHERE rowid = :p",
            params! { "p" => p },
        )?;
    }
    let entrepots: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_entrepot WHERE entity = :entity AND ref LIKE :motif",
        params! { "entity" => ENTITY, "motif" => format!("{prefixe}%") },
    )?;
    for e in entrepots {
        conn.exec_drop(
            "DELETE FROM llx_entrepot WHERE rowid = :e",
            params! { "e" => e },
        )?;
    }
    Ok(())
}

#[test]
fn les_mouvements_mettent_a_jour_le_niveau() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "ZZSTOCKA")?;

    let produit = db::product::create(
        &pool,
        ENTITY,
        &NewProduct::new("ZZSTOCKA1", "Sac de ciment"),
    )?;
    let entrepot = db::stock::creer_entrepot(&pool, ENTITY, "ZZSTOCKA-ATELIER", Some("Charly"))?;

    // Deux entrees, une sortie.
    db::stock::mouvement(
        &pool,
        ENTITY,
        produit,
        entrepot,
        dec("10"),
        db::stock::TYPE_ENTREE,
        Some("achat"),
    )?;
    db::stock::mouvement(
        &pool,
        ENTITY,
        produit,
        entrepot,
        dec("5"),
        db::stock::TYPE_ENTREE,
        Some("retour"),
    )?;
    db::stock::mouvement(
        &pool,
        ENTITY,
        produit,
        entrepot,
        dec("3"),
        db::stock::TYPE_SORTIE,
        Some("chantier"),
    )?;

    assert_eq!(db::stock::quantite(&pool, produit, entrepot)?, dec("12"));

    let niveau = db::stock::niveaux(&pool, ENTITY)?
        .into_iter()
        .find(|n| n.fk_product == produit)
        .expect("niveau present");
    assert_eq!(niveau.quantite, dec("12"));
    assert_eq!(niveau.entrepot, "ZZSTOCKA-ATELIER");

    let mouvements = db::stock::mouvements(&pool, ENTITY, 10)?;
    assert!(
        mouvements
            .iter()
            .filter(|m| m.ref_produit == "ZZSTOCKA1")
            .count()
            >= 3,
        "les trois mouvements sont dans l'historique"
    );

    // Une quantite nulle ou negative est refusee.
    assert!(
        db::stock::mouvement(&pool, ENTITY, produit, entrepot, Decimal::ZERO, 0, None).is_err()
    );
    // Un entrepot inconnu aussi.
    assert!(db::stock::mouvement(&pool, ENTITY, produit, 999_999, dec("1"), 0, None).is_err());

    nettoyer(&pool, "ZZSTOCKA")?;
    Ok(())
}

#[test]
fn une_sortie_peut_rendre_le_stock_negatif_mais_le_mouvement_reste_trace(
) -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "ZZSTOCKB")?;

    let produit = db::product::create(&pool, ENTITY, &NewProduct::new("ZZSTOCKB1", "Pierre"))?;
    let entrepot = db::stock::creer_entrepot(&pool, ENTITY, "ZZSTOCKB-DEPOT", None)?;

    db::stock::mouvement(
        &pool,
        ENTITY,
        produit,
        entrepot,
        dec("4"),
        db::stock::TYPE_ENTREE,
        None,
    )?;
    db::stock::mouvement(
        &pool,
        ENTITY,
        produit,
        entrepot,
        dec("6"),
        db::stock::TYPE_SORTIE,
        None,
    )?;

    // Dolibarr autorise le negatif : c'est un signal, pas une erreur technique.
    assert_eq!(db::stock::quantite(&pool, produit, entrepot)?, dec("-2"));

    nettoyer(&pool, "ZZSTOCKB")?;
    Ok(())
}
