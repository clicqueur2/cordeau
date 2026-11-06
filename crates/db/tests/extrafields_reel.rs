//! Champs libres : lire et ecrire la vraie table de Dolibarr, pas une imitation.
//!
//! Ces tests sont nes d'une erreur trouvee sur la base de production. Le module
//! cherchait la ligne d'un objet par `rowid` et la colonne sous le nom
//! `options_<champ>`. Les deux sont faux :
//!
//! - la table `<objet>_extrafields` a son propre `rowid` et lie l'objet par
//!   `fk_object` (`UNIQUE KEY uk_<table>_extrafields (fk_object)`) ;
//! - la colonne porte le nom du champ, sans prefixe (`DDLAddField($table,
//!   $attrname, â€¦)`). Le prefixe `options_` n'existe que cote PHP.
//!
//! Les tests unitaires ne pouvaient pas le voir : ils fabriquaient leurs propres
//! tables. Ceux-ci passent par le schema reel.

use std::error::Error;

use mysql::prelude::Queryable;
use mysql::{params, Pool};

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

const ENTITY: i64 = 1;
const CHAMP: &str = "zz_test_champ";
const TABLE: &str = "llx_propal_extrafields";

/// Remet la base dans l'etat d'avant, meme apres un test interrompu.
fn nettoyer(pool: &Pool, conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
    conn.exec_drop(
        "DELETE FROM llx_extrafields WHERE name = :nom",
        params! { "nom" => CHAMP },
    )?;
    let colonnes = db::extrafields::colonnes_presentes(pool, TABLE)?;
    if colonnes.contains(&CHAMP.to_string()) {
        conn.exec_drop(format!("ALTER TABLE `{TABLE}` DROP COLUMN `{CHAMP}`"), ())?;
    }
    Ok(())
}

/// Le cycle complet : definition, colonne, valeur, relecture, ecriture.
#[test]
fn un_champ_libre_se_lit_et_s_ecrit_sur_la_vraie_table() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let mut conn = pool.get_conn()?;

    // Une colonne peut rester d'un essai precedent.
    nettoyer(&pool, &mut conn)?;

    // Un devis de l'entite de test, pour porter le champ.
    let propal: Option<i64> = conn.query_first("SELECT rowid FROM llx_propal LIMIT 1")?;
    let Some(propal) = propal else {
        eprintln!("aucun devis dans la base : test ignore");
        return Ok(());
    };

    conn.exec_drop(
        format!("ALTER TABLE `{TABLE}` ADD COLUMN `{CHAMP}` varchar(64) DEFAULT NULL"),
        (),
    )?;
    conn.exec_drop(
        "INSERT INTO llx_extrafields (name, entity, elementtype, label, type, size, \
                                      fieldrequired, enabled, pos) \
         VALUES (:nom, :entity, 'propal', 'Test', 'varchar', '64', 0, '1', 0)",
        params! { "nom" => CHAMP, "entity" => ENTITY },
    )?;

    // La ligne de l'objet porte son propre `rowid`, tres different de l'id.
    conn.exec_drop(
        format!("DELETE FROM `{TABLE}` WHERE fk_object = :id"),
        params! { "id" => propal },
    )?;
    conn.exec_drop(
        format!("INSERT INTO `{TABLE}` (fk_object, `{CHAMP}`) VALUES (:id, 'bonjour')"),
        params! { "id" => propal },
    )?;
    let rowid: Option<i64> = conn.exec_first(
        format!("SELECT rowid FROM `{TABLE}` WHERE fk_object = :id"),
        params! { "id" => propal },
    )?;
    assert_ne!(
        rowid,
        Some(propal),
        "le test n'a de sens que si rowid differe de fk_object"
    );

    // Lecture : la valeur doit sortir.
    let valeurs = db::extrafields::valeurs(&pool, "propal", ENTITY, propal)?;
    let lue = valeurs
        .iter()
        .find(|(d, _, _)| d.nom == CHAMP)
        .ok_or("champ absent des definitions")?;
    assert_eq!(lue.1.as_deref(), Some("bonjour"), "valeur non relue");
    assert!(lue.2, "la colonne devrait etre vue comme existante");

    // Ecriture : elle doit atteindre la bonne ligne.
    db::extrafields::enregistrer(
        &pool,
        "propal",
        ENTITY,
        propal,
        &[(CHAMP.to_string(), "soir".to_string())],
    )?;
    let relu: Option<String> = conn.exec_first(
        format!("SELECT `{CHAMP}` FROM `{TABLE}` WHERE fk_object = :id"),
        params! { "id" => propal },
    )?;
    assert_eq!(relu.as_deref(), Some("soir"));

    conn.exec_drop(
        format!("DELETE FROM `{TABLE}` WHERE fk_object = :id"),
        params! { "id" => propal },
    )?;
    nettoyer(&pool, &mut conn)?;
    Ok(())
}
