//! Ce que la base de production a appris au noyau.
//!
//! Deux faits, mesures sur la copie de la base de Pierre Angulaire :
//!
//! - Dolibarr n'ecrit jamais `NULL` dans `siret` / `tva_intra` : il ecrit la
//!   chaine vide. 912 tiers sur 932 sont dans ce cas.
//! - Six chantiers d'une societe peuvent pointer sur un tiers d'une autre.
//!
//! Ces deux faits ont casse quelque chose de precis : le premier faisait passer
//! tout particulier pour un professionnel, et refusait ses factures ; le second
//! affichait « absent » pour une donnee qui existait.

use std::error::Error;

use mysql::prelude::Queryable;
use mysql::{params, Pool};

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

const ENTITY: i64 = 1;

/// Une chaine vide en base n'est pas une valeur : elle ne doit pas ressortir
/// comme un SIRET existant.
#[test]
fn un_siret_vide_ressort_comme_absent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let mut conn = pool.get_conn()?;

    let nom = "ZZ-TEST-SIRET-VIDE";
    conn.exec_drop(
        "DELETE FROM llx_societe WHERE nom = :nom",
        params! { "nom" => nom },
    )?;
    conn.exec_drop(
        "INSERT INTO llx_societe (entity, nom, siret, tva_intra, siren, client, statut, datec) \
         VALUES (:entity, :nom, '', '  ', '', 3, 1, NOW())",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    let id: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE nom = :nom ORDER BY rowid DESC LIMIT 1",
        params! { "nom" => nom },
    )?;
    let id = id.ok_or("tiers de test non cree")?;

    let p = db::party::get(&pool, ENTITY, id)?.ok_or("tiers de test illisible")?;
    assert_eq!(
        p.siret, None,
        "un SIRET vide ne doit pas ressortir comme une valeur"
    );
    assert_eq!(
        p.tva_intra, None,
        "un numero de TVA en espaces est une absence"
    );
    assert_eq!(p.siren, None);

    // Et l'ecriture d'un vrai SIRET ressort bien.
    conn.exec_drop(
        "UPDATE llx_societe SET siret = '12345678900012' WHERE rowid = :id",
        params! { "id" => id },
    )?;
    let p = db::party::get(&pool, ENTITY, id)?.ok_or("tiers de test illisible")?;
    assert_eq!(p.siret.as_deref(), Some("12345678900012"));

    conn.exec_drop(
        "DELETE FROM llx_societe WHERE rowid = :id",
        params! { "id" => id },
    )?;
    Ok(())
}

/// Un tiers rattache a une autre societe est signale comme tel, pas comme
/// absent : ce n'est pas une donnee manquante, c'est un rattachement a
/// corriger.
#[test]
fn un_tiers_d_une_autre_societe_se_distingue_d_un_tiers_absent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let mut conn = pool.get_conn()?;

    let nom = "ZZ-TEST-AUTRE-SOCIETE";
    conn.exec_drop(
        "DELETE FROM llx_societe WHERE nom = :nom",
        params! { "nom" => nom },
    )?;
    conn.exec_drop(
        "INSERT INTO llx_societe (entity, nom, client, statut, datec) \
         VALUES (7, :nom, 3, 1, NOW())",
        params! { "nom" => nom },
    )?;
    let id: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE nom = :nom ORDER BY rowid DESC LIMIT 1",
        params! { "nom" => nom },
    )?;
    let id = id.ok_or("tiers de test non cree")?;

    // Introuvable dans l'entite de travail...
    assert!(db::party::get(&pool, ENTITY, id)?.is_none());
    // ...mais la societe qui le detient est connue.
    assert_eq!(db::party::entity_du_tiers(&pool, id)?, Some(7));
    // Un identifiant qui n'existe nulle part ne rend rien.
    assert_eq!(db::party::entity_du_tiers(&pool, 99_999_999)?, None);

    conn.exec_drop(
        "DELETE FROM llx_societe WHERE rowid = :id",
        params! { "id" => id },
    )?;
    Ok(())
}
