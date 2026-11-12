//! Un acompte encaisse avant des situations doit etre deduit : sans cela, le
//! devis se facture deux fois pour la part versee.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::invoice::{NewInvoice, NewInvoiceLine};
use db::proposal::{NewProposal, NewProposalLine};

const ENTITY: i64 = 1;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

fn nettoyer(pool: &Pool, nom: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    for id in ids {
        conn.exec_drop(
            "UPDATE llx_facture SET fk_facture_source = NULL WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        let factures: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_facture WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for f in factures {
            conn.exec_drop(
                "DELETE FROM llx_paiement_facture WHERE fk_facture = :f",
                params! { "f" => f },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_facturedet WHERE fk_facture = :f",
                params! { "f" => f },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_element_element WHERE fk_target = :f",
                params! { "f" => f },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_facture WHERE rowid = :f",
                params! { "f" => f },
            )?;
        }
        let devis: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_propal WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for d in devis {
            conn.exec_drop(
                "DELETE FROM llx_propaldet WHERE fk_propal = :d",
                params! { "d" => d },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_propal WHERE rowid = :d",
                params! { "d" => d },
            )?;
        }
        db::party::delete(pool, ENTITY, id)?;
    }
    Ok(())
}

/// Devis 1 000 HT a 10 % + 2 000 HT a 20 % : 3 500 TTC.
fn devis(pool: &Pool, nom: &str) -> Result<(i64, i64), Box<dyn Error>> {
    let fk_soc = db::party::create(pool, ENTITY, &db::party::NewParty::client(nom))?;
    let mut new = NewProposal::new(fk_soc);
    new.lines = vec![
        NewProposalLine::new("Lot maconnerie", dec("1"), dec("1000"), dec("10")),
        NewProposalLine::new("Lot pierre", dec("1"), dec("2000"), dec("20")),
    ];
    let id = db::proposal::create(pool, ENTITY, &new)?;
    db::proposal::validate(pool, ENTITY, id)?;
    db::proposal::cloturer(pool, ENTITY, id, 2)?;
    Ok((fk_soc, id))
}

/// Un acompte de 300 HT (360 TTC a 20 %), marque comme le fait n8n.
fn acompte(pool: &Pool, fk_soc: i64, devis_id: i64) -> Result<i64, Box<dyn Error>> {
    let mut new = NewInvoice::new(fk_soc);
    new.lines = vec![NewInvoiceLine::new(
        "Acompte 10 %",
        dec("1"),
        dec("300.00"),
        dec("20"),
    )];
    let id = db::invoice::create(pool, ENTITY, &new)?;
    db::invoice::validate(pool, ENTITY, id)?;
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_facture SET type = 3, ref_ext = :marque WHERE rowid = :id",
        params! { "marque" => format!("PA-ACOMPTE-{devis_id}"), "id" => id },
    )?;
    conn.exec_drop(
        "INSERT INTO llx_element_element (fk_source, sourcetype, fk_target, targettype) \
         VALUES (:devis, 'propal', :facture, 'facture')",
        params! { "devis" => devis_id, "facture" => id },
    )?;
    Ok(id)
}

#[test]
fn un_acompte_encaisse_est_deduit_des_situations() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Situation Acompte";
    nettoyer(&pool, nom)?;

    let (fk_soc, devis_id) = devis(&pool, nom)?;
    let acompte_id = acompte(&pool, fk_soc, devis_id)?;

    // 50 % : 1 750 TTC. Rien a deduire, l'acompte ne couvre pas encore.
    let s1 = db::situation::creer(&pool, ENTITY, devis_id, dec("50"), Some("2026-03-01"))?;
    let f1 = db::invoice::get(&pool, ENTITY, s1)?.expect("situation 1");
    assert_eq!(f1.total_ttc, Some(dec("1750")));

    // 50 % de plus : la situation finale solde, en retirant l'acompte.
    let s2 = db::situation::creer(&pool, ENTITY, devis_id, dec("50"), Some("2026-04-01"))?;
    let f2 = db::invoice::get(&pool, ENTITY, s2)?.expect("situation 2");
    assert_eq!(
        f2.total_ttc,
        Some(dec("1390")),
        "3 500 - 1 750 deja - 360 d'acompte (300 HT a 20 %)"
    );

    // Le compte est bon : situations + acompte = devis, au centime.
    let a = db::invoice::get(&pool, ENTITY, acompte_id)?.expect("acompte");
    let total = f1.total_ttc.unwrap_or_default()
        + f2.total_ttc.unwrap_or_default()
        + a.total_ttc.unwrap_or_default();
    assert_eq!(total, dec("3500"), "le devis n'est facture qu'une fois");

    // Une situation de plus depasserait le devis : refusee.
    let err = db::situation::creer(&pool, ENTITY, devis_id, dec("10"), Some("2026-05-01"))
        .expect_err("le devis est deja facture en entier");
    assert!(
        err.to_string().contains("depasserait") || err.to_string().contains("cumul"),
        "message inattendu : {err}"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}
