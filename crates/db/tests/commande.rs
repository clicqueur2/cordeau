//! Commandes clients : creation depuis un devis, numerotation, validation.

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

fn nettoyer(pool: &Pool, nom: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    for id in ids {
        let commandes: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_commande WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for c in commandes {
            conn.exec_drop(
                "DELETE FROM llx_commandedet WHERE fk_commande = :c",
                params! { "c" => c },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_element_element WHERE targettype = 'commande' AND fk_target = :c",
                params! { "c" => c },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_commande WHERE rowid = :c",
                params! { "c" => c },
            )?;
        }
        let propales: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_propal WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for p in propales {
            conn.exec_drop(
                "DELETE FROM llx_propaldet WHERE fk_propal = :p",
                params! { "p" => p },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_propal WHERE rowid = :p",
                params! { "p" => p },
            )?;
        }
        db::party::delete(pool, ENTITY, id)?;
    }
    Ok(())
}

#[test]
fn une_commande_reprend_le_devis_et_se_valide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Commande Test";
    nettoyer(&pool, nom)?;

    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;
    let mut devis = NewProposal::new(fk_soc);
    devis.lines = vec![
        NewProposalLine::new("Lot maconnerie", dec("2"), dec("100"), dec("10")),
        NewProposalLine::new("Lot pierre", dec("1"), dec("500"), dec("20")),
    ];
    let propal = db::proposal::create(&pool, ENTITY, &devis)?;

    // Un devis en brouillon ne se commande pas.
    assert!(
        db::commande::create_from_proposal(&pool, ENTITY, propal).is_err(),
        "un devis non valide ne se commande pas"
    );

    db::proposal::validate(&pool, ENTITY, propal)?;
    let commande = db::commande::create_from_proposal(&pool, ENTITY, propal)?;

    let c = db::commande::get(&pool, ENTITY, commande)?.expect("commande relue");
    assert!(c.ref_.starts_with("CO"), "reference inattendue: {}", c.ref_);
    assert_eq!(c.ref_.len(), 11, "CO + AAAMM + - + 4 chiffres");
    assert_eq!(c.fk_statut, db::commande::STATUT_BROUILLON);
    assert_eq!(c.total_ht, Some(dec("700")), "2 x 100 + 500");
    assert_eq!(c.total_ttc, Some(dec("820")), "220 + 600");

    let lignes = db::commande::lines(&pool, ENTITY, commande)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(lignes[0].label.as_deref(), Some("Lot maconnerie"));

    // Le lien devis -> commande est pose.
    let mut conn = pool.get_conn()?;
    let lien: Option<i64> = conn.exec_first(
        "SELECT fk_source FROM llx_element_element \
         WHERE sourcetype = 'propal' AND targettype = 'commande' AND fk_target = :c",
        params! { "c" => commande },
    )?;
    assert_eq!(lien, Some(propal));

    db::commande::validate(&pool, ENTITY, commande)?;
    assert_eq!(
        db::commande::get(&pool, ENTITY, commande)?
            .unwrap()
            .fk_statut,
        db::commande::STATUT_VALIDEE
    );
    assert!(
        db::commande::validate(&pool, ENTITY, commande).is_err(),
        "une commande ne se valide qu'une fois"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn les_commandes_sont_numerotees_sans_doublon() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Commande Numeros";
    nettoyer(&pool, nom)?;

    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;
    let mut devis = NewProposal::new(fk_soc);
    devis.lines = vec![NewProposalLine::new(
        "Prestation",
        dec("1"),
        dec("100"),
        dec("20"),
    )];
    let propal = db::proposal::create(&pool, ENTITY, &devis)?;
    db::proposal::validate(&pool, ENTITY, propal)?;

    let mut refs = Vec::new();
    for _ in 0..3 {
        let id = db::commande::create_from_proposal(&pool, ENTITY, propal)?;
        refs.push(db::commande::get(&pool, ENTITY, id)?.unwrap().ref_);
    }
    let uniques: std::collections::BTreeSet<&String> = refs.iter().collect();
    assert_eq!(uniques.len(), 3, "references dupliquees: {refs:?}");

    nettoyer(&pool, nom)?;
    Ok(())
}
