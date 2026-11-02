use std::error::Error;
use std::str::FromStr;

use db::invoice::{NewInvoice, NewInvoiceLine};
use mysql::Pool;
use rust_decimal::Decimal;

const ENTITY: i64 = 1;

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

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn nettoyer(pool: &Pool, nom: &str) -> Result<(), Box<dyn Error>> {
    let appartient = |fk: Option<i64>, pool: &Pool| -> bool {
        match fk.and_then(|id| db::party::get(pool, ENTITY, id).ok().flatten()) {
            Some(s) => s.nom == nom,
            None => false,
        }
    };
    for f in db::invoice::list(pool, ENTITY, 1000, 0)? {
        if appartient(f.fk_soc, pool) {
            db::invoice::delete(pool, ENTITY, f.rowid)?;
        }
    }
    for p in db::party::list(pool, ENTITY, 10_000, 0)? {
        if p.nom == nom {
            db::party::delete(pool, ENTITY, p.rowid)?;
        }
    }
    Ok(())
}

#[test]
fn un_avoir_annule_une_facture_validee() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Avoir Test";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;

    let mut new = NewInvoice::new(fk_soc);
    new.lines = vec![NewInvoiceLine::new(
        "Travaux",
        dec("2"),
        dec("500"),
        dec("20"),
    )];
    let facture = db::invoice::create(&pool, ENTITY, &new)?;
    db::invoice::validate(&pool, ENTITY, facture)?;
    let f = db::invoice::get(&pool, ENTITY, facture)?.expect("facture");
    assert_eq!(f.total_ttc, Some(dec("1200")));

    let avoir = db::invoice::create_avoir(&pool, ENTITY, facture)?;
    let a = db::invoice::get(&pool, ENTITY, avoir)?.expect("avoir");

    assert!(
        a.ref_.starts_with("AV"),
        "reference d'avoir attendue: {}",
        a.ref_
    );
    assert_eq!(a.type_, db::invoice::TYPE_AVOIR);
    assert_eq!(
        a.fk_facture_source,
        Some(facture),
        "lien vers la facture d'origine"
    );
    assert_eq!(a.fk_soc, Some(fk_soc));
    assert_eq!(a.total_ttc, Some(dec("-1200")), "montants negatifs");
    assert_eq!(a.total_ht, Some(dec("-1000")));

    let lignes = db::invoice::lines(&pool, ENTITY, avoir)?;
    assert_eq!(lignes.len(), 1);
    assert_eq!(lignes[0].qty, Some(dec("-2")), "quantite negative");

    // Un seul avoir par facture.
    let err = db::invoice::create_avoir(&pool, ENTITY, facture)
        .expect_err("un second avoir doit etre refuse");
    assert!(
        err.to_string().contains("deja un avoir"),
        "message inattendu : {err}"
    );

    db::invoice::delete(&pool, ENTITY, avoir)?;
    db::invoice::delete(&pool, ENTITY, facture)?;
    db::party::delete(&pool, ENTITY, fk_soc)?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn pas_d_avoir_sur_un_brouillon_ni_sur_un_avoir() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Avoir Refus";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;

    let mut new = NewInvoice::new(fk_soc);
    new.lines = vec![NewInvoiceLine::new(
        "Travaux",
        dec("1"),
        dec("100"),
        dec("20"),
    )];
    let brouillon = db::invoice::create(&pool, ENTITY, &new)?;

    let err = db::invoice::create_avoir(&pool, ENTITY, brouillon)
        .expect_err("un brouillon ne doit pas recevoir d'avoir");
    assert!(err.to_string().contains("validee"), "message : {err}");

    db::invoice::validate(&pool, ENTITY, brouillon)?;
    let avoir = db::invoice::create_avoir(&pool, ENTITY, brouillon)?;
    db::invoice::validate(&pool, ENTITY, avoir)?;
    let err = db::invoice::create_avoir(&pool, ENTITY, avoir)
        .expect_err("un avoir ne doit pas recevoir d'avoir");
    assert!(err.to_string().contains("avoir"), "message : {err}");

    db::invoice::delete(&pool, ENTITY, avoir)?;
    db::invoice::delete(&pool, ENTITY, brouillon)?;
    db::party::delete(&pool, ENTITY, fk_soc)?;
    nettoyer(&pool, nom)?;
    Ok(())
}
