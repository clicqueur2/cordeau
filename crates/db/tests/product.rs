use std::error::Error;

use mysql::Pool;
use rust_decimal::Decimal;
use std::str::FromStr;

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test ignore");
            None
        }
    }
}

const ENTITY: i64 = 1;

fn nettoyer(pool: &Pool, refs: &[&str]) -> Result<(), Box<dyn Error>> {
    for r in refs {
        if let Some(p) = db::product::by_ref(pool, ENTITY, r)? {
            db::product::delete(pool, ENTITY, p.rowid)?;
        }
    }
    Ok(())
}

#[test]
fn cree_un_produit_et_relit_son_prix_sans_flottant() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &["TEST-PROD-1"])?;

    let mut new = db::product::NewProduct::new("TEST-PROD-1", "Bordure granit 30 cm");
    new.price = Some(Decimal::from_str("125.50").unwrap());
    new.tva_tx = Some(Decimal::from_str("20").unwrap());

    let id = db::product::create(&pool, ENTITY, &new)?;
    assert!(id > 0);

    let lu = db::product::get(&pool, ENTITY, id)?.expect("produit relu");
    assert_eq!(lu.ref_, "TEST-PROD-1");
    assert_eq!(lu.label, "Bordure granit 30 cm");
    assert_eq!(lu.price, Some(Decimal::from_str("125.5").unwrap()));
    assert_eq!(lu.price_ttc, Some(Decimal::from_str("150.6").unwrap()));
    assert_eq!(lu.price_base_type, "HT");
    assert_eq!(lu.hidden, 0);
    assert!(!lu.est_service());

    db::product::set_hidden(&pool, ENTITY, id, 1)?;
    assert_eq!(db::product::get(&pool, ENTITY, id)?.unwrap().hidden, 1);

    db::product::delete(&pool, ENTITY, id)?;
    assert!(db::product::get(&pool, ENTITY, id)?.is_none());
    Ok(())
}

#[test]
fn cree_un_service_avec_unite() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &["TEST-SERV-1"])?;

    let mut new = db::product::NewProduct::new("TEST-SERV-1", "Main d'oeuvre macon");
    new.fk_product_type = db::product::TYPE_SERVICE;
    new.price = Some(Decimal::from_str("48.00").unwrap());
    new.tva_tx = Some(Decimal::from_str("10").unwrap());
    new.fk_unit = db::product::unit_id(&pool, "H")?;

    let id = db::product::create(&pool, ENTITY, &new)?;
    let lu = db::product::get(&pool, ENTITY, id)?.expect("service relu");
    assert!(lu.est_service());
    assert_eq!(lu.price, Some(Decimal::from_str("48").unwrap()));
    assert_eq!(lu.price_ttc, Some(Decimal::from_str("52.8").unwrap()));

    db::product::delete(&pool, ENTITY, id)?;
    Ok(())
}

#[test]
fn refuse_une_reference_ou_un_libelle_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    assert!(db::product::create(&pool, ENTITY, &db::product::NewProduct::new("  ", "x")).is_err());
    assert!(
        db::product::create(&pool, ENTITY, &db::product::NewProduct::new("REF", "  ")).is_err()
    );
    Ok(())
}

#[test]
fn reference_unique_par_entite() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &["TEST-DOUBLON"])?;
    let new = db::product::NewProduct::new("TEST-DOUBLON", "x");
    let id = db::product::create(&pool, ENTITY, &new)?;
    assert!(
        db::product::create(&pool, ENTITY, &new).is_err(),
        "uk_product_ref doit refuser un doublon dans la meme entite"
    );
    db::product::delete(&pool, ENTITY, id)?;
    Ok(())
}
