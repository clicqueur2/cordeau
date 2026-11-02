//! FEC : generation des ecritures et export au format attendu.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;
use std::str::FromStr;

use db::invoice::{NewInvoice, NewInvoiceLine};

const ENTITY: i64 = 1;
const NOM: &str = "Tiers FEC Test";

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

fn nettoyer(pool: &Pool) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_accounting_bookkeeping WHERE doc_ref LIKE 'ZZFEC%'",
        (),
    )?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => NOM },
    )?;
    for id in ids {
        let factures: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_facture WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for f in factures {
            conn.exec_drop(
                "DELETE FROM llx_facturedet WHERE fk_facture = :f",
                params! { "f" => f },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_facture WHERE rowid = :f",
                params! { "f" => f },
            )?;
        }
        db::party::delete(pool, ENTITY, id)?;
    }
    Ok(())
}

#[test]
fn le_fec_sort_avec_ses_dix_huit_colonnes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;

    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(NOM))?;
    let mut facture = NewInvoice::new(fk_soc);
    facture.datef = Some("2026-09-10".into());
    facture.lines = vec![NewInvoiceLine::new(
        "Prestation",
        dec("1"),
        dec("1000"),
        dec("20"),
    )];
    let id = db::invoice::create(&pool, ENTITY, &facture)?;
    db::invoice::validate(&pool, ENTITY, id)?;

    // Generation sur la periode de la facture.
    let n = db::fec::generer(&pool, ENTITY, Some("2026-09-01"), Some("2026-09-30"))?;
    assert_eq!(n, 3, "une vente = trois lignes (client, produit, TVA)");

    // Deux generations de suite ne doublent pas les ecritures.
    let n2 = db::fec::generer(&pool, ENTITY, Some("2026-09-01"), Some("2026-09-30"))?;
    assert_eq!(n2, 3);
    assert_eq!(
        db::fec::compter(&pool, ENTITY, Some("2026-09-01"), Some("2026-09-30"))?,
        3
    );

    let fec = db::fec::export(&pool, ENTITY, "2026-09-01", "2026-09-30")?;
    let lignes: Vec<&str> = fec.lines().collect();
    assert_eq!(lignes.len(), 4, "en-tete + 3 ecritures");
    assert!(
        lignes[0].starts_with("JournalCode\tJournalLib\tEcritureNum"),
        "en-tete FEC"
    );
    assert_eq!(
        lignes[0].split('\t').count(),
        18,
        "le FEC impose 18 colonnes"
    );
    for l in &lignes[1..] {
        assert_eq!(l.split('\t').count(), 18, "ligne FEC mal formee : {l}");
    }

    // La date est en AAAAMMJJ et les montants a la virgule.
    assert!(lignes[1].contains("20260910"), "date FEC : {}", lignes[1]);
    assert!(fec.contains("1200,00"), "montant TTC attendu : {fec}");
    assert!(fec.contains("200,00"), "TVA attendue");

    nettoyer(&pool)?;
    Ok(())
}
