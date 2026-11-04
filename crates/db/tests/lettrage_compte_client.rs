use std::error::Error;
use std::str::FromStr;

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

/// Le compte client s'eteint : facture (debit 411) puis encaissement
/// (credit 411) donnent un solde nul et **une paire de lettrage**.
///
/// C'est le chainon qui manquait a l'audit du FEC : sans l'ecriture de
/// reglement, le 411 ne s'epuise jamais et le solde est inexpliquable.
#[test]
fn une_facture_payee_solde_le_compte_client() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = format!("ZZ-LETTRAGE-{}", std::process::id());
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(&nom))?;

    // 1. Une facture de 1 000 HT a 20 % : 1 200 TTC.
    let mut new = db::invoice::NewInvoice::new(fk_soc);
    new.lines = vec![db::invoice::NewInvoiceLine::new(
        "Travaux",
        dec("1"),
        dec("1000"),
        dec("20"),
    )];
    let facture = db::invoice::create(&pool, ENTITY, &new)?;
    // Une facture doit etre validee pour s'ecrire : un brouillon n'a pas
    // d'ecriture (regle du pont ecritures).
    db::invoice::validate(&pool, ENTITY, facture)?;

    // 2. Son ecriture de vente : debit 411 de 1 200.
    let ecriture_vente = db::ecritures::facture_vente(&pool, ENTITY, facture)?;
    assert!(
        ecriture_vente.numero.starts_with("VE-"),
        "numero : {}",
        ecriture_vente.numero
    );

    // 3. Le reglement, puis son ecriture : credit 411 de 1 200.
    let paiement = db::payment::create(
        &pool,
        ENTITY,
        &db::payment::NewPayment {
            fk_facture: facture,
            montant: dec("1200"),
            fk_paiement: 2, // VIR
            num_paiement: None,
            note: None,
            date: Some("2026-11-04".to_string()),
            fk_compte: None,
        },
    )?;
    let ecriture_reglement = db::ecritures::reglement_client(&pool, ENTITY, paiement)?;

    // 4. On porte les deux au grand livre : le 411 doit tomber a zero.
    let mut livre = comptabilite::GrandLivre::nouveau();
    livre.ajouter(&ecriture_vente)?;
    livre.ajouter(&ecriture_reglement)?;
    assert_eq!(
        livre.solde(db::ecritures::COMPTE_CLIENTS),
        Decimal::ZERO,
        "le compte client doit etre solde"
    );

    // 5. Et le lettrage doit apparier les deux mouvements.
    let mut lettrage = comptabilite::Lettrage::nouveau();
    let paires = lettrage.lettrer(&livre, db::ecritures::COMPTE_CLIENTS)?;
    assert_eq!(paires, 1, "une paire de lettrage attendue");
    assert_eq!(
        lettrage.solde_restant(db::ecritures::COMPTE_CLIENTS),
        Decimal::ZERO
    );

    // Menage.
    db::invoice::delete(&pool, ENTITY, facture)?;
    db::party::delete(&pool, ENTITY, fk_soc)?;
    println!("lettrage : 411 solde par 1 paire, reste 0.00");
    Ok(())
}
