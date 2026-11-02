//! Contacts et agenda : creation, rattachement, lecture.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

const ENTITY: i64 = 1;

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

fn nettoyer(pool: &Pool, nom: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_socpeople WHERE entity = :entity AND lastname = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_actioncomm WHERE entity = :entity AND label = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    for id in ids {
        db::party::delete(pool, ENTITY, id)?;
    }
    Ok(())
}

#[test]
fn cree_un_contact_rattache_ou_non() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "ZZ Contact Test";
    nettoyer(&pool, nom)?;

    // Contact sans societe : c'est un particulier, Dolibarr l'accepte.
    let seul = db::contact::creer(
        &pool,
        ENTITY,
        None,
        Some("M."),
        nom,
        Some("Jean"),
        Some("jean@exemple.test"),
        None,
        None,
    )?;
    assert!(seul > 0);

    // Contact rattache a un tiers.
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;
    let lie = db::contact::creer(
        &pool,
        ENTITY,
        Some(fk_soc),
        Some("Mme"),
        nom,
        Some("Marie"),
        None,
        Some("0600000000"),
        Some("Conductrice de travaux"),
    )?;

    let liste = db::contact::list(&pool, ENTITY, 500)?;
    let jean = liste
        .iter()
        .find(|c| c.rowid == seul)
        .expect("contact seul");
    assert_eq!(jean.nom_complet(), "M. ZZ Contact Test Jean");
    assert_eq!(jean.fk_soc, None);
    assert!(jean.actif);

    let marie = liste.iter().find(|c| c.rowid == lie).expect("contact lie");
    assert_eq!(marie.fk_soc, Some(fk_soc));
    assert_eq!(marie.tiers.as_deref(), Some(nom));
    assert_eq!(marie.poste.as_deref(), Some("Conductrice de travaux"));

    // Un tiers inconnu est refuse.
    assert!(db::contact::creer(
        &pool,
        ENTITY,
        Some(999_999),
        None,
        nom,
        None,
        None,
        None,
        None
    )
    .is_err());
    // Un nom vide aussi.
    assert!(db::contact::creer(&pool, ENTITY, None, None, "   ", None, None, None, None).is_err());

    // Desactivation : le contact reste, mais inactif.
    db::contact::desactiver(&pool, ENTITY, lie)?;
    let apres = db::contact::list(&pool, ENTITY, 500)?
        .into_iter()
        .find(|c| c.rowid == lie)
        .expect("contact conserve");
    assert!(!apres.actif);

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn cree_un_evenement_d_agenda() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "ZZ Evenement Test";
    nettoyer(&pool, nom)?;

    let id = db::journal::creer_evenement(&pool, ENTITY, nom, Some("2026-11-05"), None)?;
    assert!(id > 0);

    let recents = db::journal::recents(&pool, ENTITY, 200)?;
    let evt = recents
        .iter()
        .find(|e| e.rowid == id)
        .expect("evenement cree");
    assert_eq!(evt.label, nom);
    assert!(evt.datep.is_some(), "la date du rendez-vous est relue");

    nettoyer(&pool, nom)?;
    Ok(())
}
