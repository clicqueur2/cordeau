//! Audit en lecture seule : la garantie, testee.

use std::error::Error;

use mysql::prelude::Queryable;
use mysql::{params, Pool};

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

/// L'audit lit une base Dolibarr reelle et n'invente rien.
#[test]
fn l_audit_lit_une_base_reelle() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let rapport = db::audit::lire(&pool)?;

    // La base de reference porte les 410 tables du noyau. Si ce test tombe,
    // c'est la base qui a un probleme, pas l'audit — et c'est utile a savoir.
    assert!(
        rapport.schema_complet(),
        "schema incomplet : {} tables sur {}",
        rapport.tables,
        rapport.tables_attendues
    );

    // Les objets surveilles sont tous comptes, dans l'ordre de la liste.
    assert_eq!(rapport.comptes.len(), db::audit::SURVEILLEES.len());
    for (compte, (table, libelle)) in rapport.comptes.iter().zip(db::audit::SURVEILLEES) {
        assert_eq!(&compte.table, table);
        assert_eq!(&compte.libelle, libelle);
        assert!(compte.lignes >= 0);
    }

    // Les utilisateurs sont comptes : une base sans aucun compte actif serait
    // une base ou personne ne peut entrer.
    assert!(rapport.utilisateurs > 0, "aucun utilisateur actif");
    assert!(rapport.total_objets() > 0, "aucun objet commercial compte");
    Ok(())
}

/// Le mecanisme de la garantie : une transaction en lecture seule refuse
/// l'ecriture, y compris quand le code se trompe.
///
/// C'est ce que `audit::lire` utilise. Le tester separement a un interet : si
/// MariaDB changeait ce comportement, l'audit deviendrait une promesse au lieu
/// d'une contrainte, et ce test le dirait.
#[test]
fn une_transaction_en_lecture_seule_refuse_l_ecriture() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let mut conn = pool.get_conn()?;

    // On ne touche a rien : on ecrit dans `llx_const` avec un nom qu'on
    // supprimerait de toute facon, et on verifie que la base refuse AVANT.
    conn.query_drop("START TRANSACTION READ ONLY")?;
    let ecriture = conn.exec_drop(
        "INSERT INTO llx_const (name, entity, value, type, visible) \
         VALUES (:nom, 1, 'x', 'chaine', 0)",
        params! { "nom" => "ZZ_AUDIT_LECTURE_SEULE" },
    );
    let _ = conn.query_drop("ROLLBACK");

    let erreur = ecriture.expect_err("une transaction READ ONLY doit refuser un INSERT");
    let message = erreur.to_string();
    assert!(
        message.contains("read only") || message.contains("READ ONLY") || message.contains("1290"),
        "le refus doit venir de la base, trouve : {message}"
    );

    // Et rien n'a ete ecrit.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_const WHERE name = 'ZZ_AUDIT_LECTURE_SEULE'",
        (),
    )?;
    assert_eq!(restes, Some(0), "l'ecriture refusee ne doit rien laisser");
    Ok(())
}

/// Deux lectures de suite donnent le meme resultat : l'audit ne modifie pas ce
/// qu'il observe.
#[test]
fn l_audit_est_stable_entre_deux_passages() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let premier = db::audit::lire(&pool)?;
    let second = db::audit::lire(&pool)?;
    assert_eq!(
        premier.comptes, second.comptes,
        "l'audit ne doit rien changer a la base qu'il lit"
    );
    Ok(())
}
