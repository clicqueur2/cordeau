//! Factures de situation : le devis se facture par tranches, la somme tombe
//! juste, et le cycle est coherent.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;
use std::str::FromStr;

use db::proposal::{NewProposal, NewProposalLine};

const ENTITY: i64 = 1;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

/// Un devis de test : 1 000 HT a 10 % + 2 000 HT a 20 % = 3 500 TTC.
fn devis(pool: &Pool, nom: &str, valide: bool) -> Result<i64, Box<dyn Error>> {
    let fk_soc = db::party::create(pool, ENTITY, &db::party::NewParty::client(nom))?;
    let mut new = NewProposal::new(fk_soc);
    new.lines = vec![
        NewProposalLine::new("Lot maconnerie", dec("1"), dec("1000"), dec("10")),
        NewProposalLine::new("Lot pierre", dec("1"), dec("2000"), dec("20")),
    ];
    let id = db::proposal::create(pool, ENTITY, &new)?;
    if valide {
        db::proposal::validate(pool, ENTITY, id)?;
    }
    Ok(id)
}

fn nettoyer(pool: &Pool, nom: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    for id in ids {
        // Les situations se chainent par `fk_facture_source` : on coupe le
        // chainage avant de supprimer, sinon la FK bloque.
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
                "DELETE FROM llx_facturedet WHERE fk_facture = :f",
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

#[test]
fn quatre_situations_font_exactement_le_devis() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Situation Quatre";
    nettoyer(&pool, nom)?;

    let devis_id = devis(&pool, nom, true)?;
    let total_ttc = db::situation::total_devis(&pool, ENTITY, devis_id)?;
    assert_eq!(total_ttc, dec("3500"), "1000 a 10 % + 2000 a 20 %");

    // 40 + 25 + 15 + 20 = 100 % : la derniere situation solde le devis.
    let parts = ["40", "25", "15", "20"];
    let mut ids = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let id = db::situation::creer(&pool, ENTITY, devis_id, dec(p), Some("2026-03-01"))?;
        assert!(id > 0, "situation {} creee", i + 1);
        ids.push(id);
    }

    let liste = db::situation::situations(&pool, ENTITY, devis_id)?;
    assert_eq!(liste.len(), 4);
    for (i, s) in liste.iter().enumerate() {
        assert_eq!(s.counter, i as i32 + 1);
        assert_eq!(s.finale, i == 3, "seule la derniere est finale");
        assert_eq!(s.datef.as_deref(), Some("2026-03-01"));
    }

    // Le cycle est unique et porte sur la premiere facture.
    let cycle = liste[0].cycle_ref.expect("cycle renseigne");
    assert_eq!(cycle, ids[0]);
    assert!(liste.iter().all(|s| s.cycle_ref == Some(cycle)));

    // Le compte est bon : la somme des situations egale le devis.
    let somme_ht: Decimal = liste.iter().map(|s| s.total_ht).sum();
    let somme_ttc: Decimal = liste.iter().map(|s| s.total_ttc).sum();
    assert_eq!(
        db::situation::total_devis(&pool, ENTITY, devis_id)?,
        somme_ttc,
        "somme des situations = TTC du devis"
    );

    // Les references sont uniques.
    let refs: std::collections::BTreeSet<&String> = liste.iter().map(|s| &s.ref_).collect();
    assert_eq!(refs.len(), 4);
    assert!(liste.iter().all(|s| s.ref_.starts_with("FA")));

    // Les factures sont en brouillon, type situation, rattachees au devis.
    let mut conn = pool.get_conn()?;
    let (typ, statut): (i16, i16) = conn
        .exec_first(
            "SELECT type, fk_statut FROM llx_facture WHERE rowid = :id",
            params! { "id" => ids[0] },
        )?
        .expect("facture relue");
    assert_eq!(typ, db::invoice::TYPE_SITUATION);
    assert_eq!(statut, db::invoice::STATUT_BROUILLON);

    // Le lien au devis passe par llx_element_element, pas par fk_facture_source
    // (qui porte une FK vers llx_facture : la situation precedente).
    let lien: Option<i64> = conn.exec_first(
        "SELECT fk_source FROM llx_element_element \
         WHERE sourcetype = 'propal' AND targettype = 'facture' AND fk_target = :id",
        params! { "id" => ids[0] },
    )?;
    assert_eq!(lien, Some(devis_id));

    assert!(somme_ht > Decimal::ZERO);
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn refuse_les_situations_incoherentes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Situation Refus";
    nettoyer(&pool, nom)?;

    // Devis en brouillon : on ne facture pas.
    let brouillon = devis(&pool, nom, false)?;
    assert!(
        db::situation::creer(&pool, ENTITY, brouillon, dec("10"), None).is_err(),
        "un devis non valide ne se facture pas"
    );

    // Devis valide : pourcentage nul ou cumul > 100 % refuses.
    let valide = devis(&pool, nom, true)?;
    assert!(db::situation::creer(&pool, ENTITY, valide, Decimal::ZERO, None).is_err());
    db::situation::creer(&pool, ENTITY, valide, dec("70"), None)?;
    assert!(
        db::situation::creer(&pool, ENTITY, valide, dec("40"), None).is_err(),
        "70 + 40 depasse 100 %"
    );
    // Le solde exact reste possible.
    db::situation::creer(&pool, ENTITY, valide, dec("30"), None)?;
    let liste = db::situation::situations(&pool, ENTITY, valide)?;
    assert_eq!(liste.len(), 2);
    assert!(liste[1].finale, "100 % atteint : la derniere est finale");

    nettoyer(&pool, nom)?;
    Ok(())
}
