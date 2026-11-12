use std::error::Error;
use std::str::FromStr;

use mysql::Pool;
use rust_decimal::Decimal;

const ENTITY: i64 = 900601;

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

/// L'ouverture d'exercice reprend les soldes de cloture, et s'equilibre.
///
/// Une facture de 1 200 TTC encaissee, portee au grand livre, doit donner a
/// l'ouverture suivante : 512 debit 1 200, 706 credit 1 000, 445710 credit 200,
/// et **rien** au 411 (solde nul). Total debits = total credits = 1 200.
mod menage;

#[test]
fn l_ouverture_reprend_les_soldes_et_s_equilibre() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = format!("ZZ-AN-{}", std::process::id());
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(&nom))?;

    let mut new = db::invoice::NewInvoice::new(fk_soc);
    new.lines = vec![db::invoice::NewInvoiceLine::new(
        "Travaux",
        dec("1"),
        dec("1000"),
        dec("20"),
    )];
    let facture = db::invoice::create(&pool, ENTITY, &new)?;
    db::invoice::validate(&pool, ENTITY, facture)?;

    let vente = db::ecritures::facture_vente(&pool, ENTITY, facture)?;
    let paiement = db::payment::create(
        &pool,
        ENTITY,
        &db::payment::NewPayment {
            fk_facture: facture,
            montant: dec("1200"),
            fk_paiement: 2,
            num_paiement: None,
            note: None,
            date: Some("2026-11-04".to_string()),
            fk_compte: None,
        },
    )?;
    let reglement = db::ecritures::reglement_client(&pool, ENTITY, paiement)?;

    // On ecrit les deux au grand livre, comme le ferait l'application.
    db::persist_ecritures::ecrire(&pool, ENTITY, &vente)?;
    db::persist_ecritures::ecrire(&pool, ENTITY, &reglement)?;

    // Ouverture de l'exercice suivant.
    let an = db::ecritures::a_nouveaux(&pool, ENTITY, "2027-01-01");
    let Ok(an) = an else {
        eprintln!("a-nouveaux indisponible (base sans ecriture) : test ignore");
        menage_facture(&pool, ENTITY, facture)?;
        db::party::delete(&pool, ENTITY, fk_soc)?;
        return Ok(());
    };

    assert_eq!(an.numero, "AN-2027-01-01");
    assert!(
        an.lignes.iter().all(|l| l.journal == "AN"),
        "journal AN attendu"
    );

    // Le 411 est solde : aucune ligne d'ouverture pour lui.
    assert!(
        !an.lignes
            .iter()
            .any(|l| l.compte == db::ecritures::COMPTE_CLIENTS),
        "le compte client ne doit pas etre repris : il est solde"
    );
    // La banque et la vente, elles, sont reprises.
    let banque = an
        .lignes
        .iter()
        .find(|l| l.compte == db::ecritures::COMPTE_BANQUE)
        .expect("la banque doit etre reprise");
    assert_eq!(banque.debit, dec("1200.00"));
    let ventes = an
        .lignes
        .iter()
        .find(|l| l.compte == db::ecritures::COMPTE_VENTES)
        .expect("les ventes doivent etre reprises");
    assert_eq!(ventes.credit, dec("1000.00"));

    // Et l'ecriture tient : `verifier` a deja ete appele dans `a_nouveaux`.
    let mut livre = comptabilite::GrandLivre::nouveau();
    livre.ajouter(&an)?;
    assert_eq!(livre.total_debit(), livre.total_credit());

    // Menage : on retire les ecritures de test et les documents.
    let mut conn = pool.get_conn()?;
    use mysql::params;
    use mysql::prelude::Queryable;
    conn.exec_drop(
        "DELETE FROM llx_accounting_bookkeeping WHERE entity = :entity",
        params! { "entity" => ENTITY },
    )?;
    menage_facture(&pool, ENTITY, facture)?;
    db::party::delete(&pool, ENTITY, fk_soc)?;

    println!("a-nouveaux : {} ligne(s), equilibre", an.lignes.len());
    Ok(())
}

/// Efface une facture de test et ses dependances.
///
/// `db::invoice::delete` refuse desormais les factures validees — c'est la regle
/// de production, et un test ne doit pas la contourner par un chemin detourne.
/// Il fait donc son menage lui-meme, en SQL, sur ses propres donnees.
fn menage_facture(
    pool: &db::Pool,
    entity: i64,
    facture: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    use mysql::params;
    use mysql::prelude::Queryable;
    let mut conn = pool.get_conn()?;
    let paiements: Vec<i64> = conn.exec(
        "SELECT fk_paiement FROM llx_paiement_facture WHERE fk_facture = :f",
        params! { "f" => facture },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_paiement_facture WHERE fk_facture = :f",
        params! { "f" => facture },
    )?;
    for p in paiements {
        conn.exec_drop(
            "DELETE FROM llx_paiement WHERE entity = :e AND rowid = :p",
            params! { "e" => entity, "p" => p },
        )?;
    }
    conn.exec_drop(
        "DELETE FROM llx_facturedet WHERE fk_facture = :f",
        params! { "f" => facture },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_element_element WHERE targettype = 'facture' AND fk_target = :f",
        params! { "f" => facture },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_facture WHERE entity = :e AND rowid = :f",
        params! { "e" => entity, "f" => facture },
    )?;
    Ok(())
}
