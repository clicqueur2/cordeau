//! Groupes et droits herites : le modele de Dolibarr.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

const ENTITY: i64 = 1;
const GROUPE: &str = "ZZ Groupe Test";

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

fn nettoyer(pool: &Pool) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let groupes: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_usergroup WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => GROUPE },
    )?;
    for g in groupes {
        conn.exec_drop(
            "DELETE FROM llx_usergroup_user WHERE fk_usergroup = :g",
            params! { "g" => g },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_usergroup_rights WHERE fk_usergroup = :g",
            params! { "g" => g },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_usergroup WHERE rowid = :g",
            params! { "g" => g },
        )?;
    }
    // Retire les droits directs poses par le test sur le compte de service.
    conn.exec_drop(
        "DELETE FROM llx_user_rights WHERE fk_user = :u",
        params! { "u" => utilisateur_test(pool)? },
    )?;
    Ok(())
}

/// Un compte de test, cree au besoin (jamais l'administrateur reel).
fn utilisateur_test(pool: &Pool) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existant: Option<i64> =
        conn.exec_first("SELECT rowid FROM llx_user WHERE login = 'zz.droits'", ())?;
    if let Some(id) = existant {
        return Ok(id);
    }
    conn.exec_drop(
        "INSERT INTO llx_user (entity, login, lastname, pass, admin, employee, statut, datec) \
         VALUES (:entity, 'zz.droits', 'Test Droits', '', 0, 0, 1, NOW())",
        params! { "entity" => ENTITY },
    )?;
    Ok(conn.last_insert_id() as i64)
}

#[test]
fn un_groupe_donne_ses_droits_a_ses_membres() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    db::droit::assurer_definitions(&pool, ENTITY)?;
    nettoyer(&pool)?;

    let groupe = db::groupe::creer(&pool, ENTITY, GROUPE, Some("test"))?;
    assert!(groupe > 0);

    // Un nom en double est refuse.
    assert!(
        db::groupe::creer(&pool, ENTITY, GROUPE, None).is_err(),
        "deux groupes de meme nom doivent etre refuses"
    );

    let user = utilisateur_test(&pool)?;
    db::groupe::definir_membre(&pool, ENTITY, groupe, user, true)?;
    assert_eq!(db::groupe::membres(&pool, ENTITY, groupe)?.len(), 1);

    // Le groupe porte les droits 9002 (devis) et 9004 (factures).
    db::groupe::definir_droit(&pool, ENTITY, groupe, 9002, true)?;
    db::groupe::definir_droit(&pool, ENTITY, groupe, 9004, true)?;
    let droits = db::groupe::droits(&pool, groupe)?;
    assert!(droits.contains(&9002) && droits.contains(&9004));

    // Droits effectifs = droits directs + droits des groupes.
    db::droit::definir(&pool, ENTITY, user, 9001, true)?;
    let effectifs = db::groupe::droits_effectifs(&pool, ENTITY, user)?;
    assert!(effectifs.contains(&9001), "droit direct");
    assert!(effectifs.contains(&9002), "droit herite");
    assert!(effectifs.contains(&9004), "droit herite");
    assert!(!effectifs.contains(&9007), "banque non accordee");

    // Retirer le membre retire ses droits herites, pas ses droits directs.
    db::groupe::definir_membre(&pool, ENTITY, groupe, user, false)?;
    let apres = db::groupe::droits_effectifs(&pool, ENTITY, user)?;
    assert!(apres.contains(&9001), "le droit direct reste");
    assert!(!apres.contains(&9002), "le droit du groupe disparait");

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn un_droit_peut_etre_retire_du_groupe() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    db::droit::assurer_definitions(&pool, ENTITY)?;
    nettoyer(&pool)?;

    let groupe = db::groupe::creer(&pool, ENTITY, GROUPE, None)?;
    db::groupe::definir_droit(&pool, ENTITY, groupe, 9003, true)?;
    assert!(db::groupe::droits(&pool, groupe)?.contains(&9003));

    db::groupe::definir_droit(&pool, ENTITY, groupe, 9003, false)?;
    assert!(!db::groupe::droits(&pool, groupe)?.contains(&9003));

    nettoyer(&pool)?;
    Ok(())
}
