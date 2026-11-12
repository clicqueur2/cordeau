//! Connexion : les comptes reels de Dolibarr doivent pouvoir entrer.
//!
//! Trouve sur la copie de production : les dix-sept comptes portent leur
//! empreinte dans `pass_crypted` et ont `pass` a NULL. `authentifier` ne lisait
//! que `pass` : personne ne pouvait se connecter, sauf le compte de test ecrit
//! par Cordeau a l'ancienne.

use std::error::Error;

use mysql::prelude::Queryable;
use mysql::{params, Pool};

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

const ENTITY: i64 = 1;
const LOGIN: &str = "zz_test_connexion";
const MDP: &str = "Copie2026!";

/// Ces tests partagent un identifiant et la meme base MySQL : ils doivent
/// s'executer l'un apres l'autre. Deux tests simultanes effacaient et
/// reinseraient le compte de l'autre.
fn verrou_test() -> std::sync::MutexGuard<'static, ()> {
    static VERROU: std::sync::Mutex<()> = std::sync::Mutex::new(());
    VERROU.lock().unwrap_or_else(|e| e.into_inner())
}

fn oublier(pool: &Pool) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_user WHERE login = :login",
        params! { "login" => LOGIN },
    )?;
    Ok(())
}

/// Le cas de la production : empreinte dans `pass_crypted`, `pass` a NULL.
#[test]
fn un_compte_dolibarr_reel_peut_se_connecter() -> Result<(), Box<dyn Error>> {
    let _verrou = verrou_test();
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    oublier(&pool)?;
    let mut conn = pool.get_conn()?;

    let empreinte = auth::hash(MDP);
    conn.exec_drop(
        "INSERT INTO llx_user (entity, login, pass_crypted, pass_encoding, admin, statut, \
                               lastname, datec) \
         VALUES (:entity, :login, :pass, 'bcrypt', 1, 1, 'Test', NOW())",
        params! { "entity" => ENTITY, "login" => LOGIN, "pass" => empreinte },
    )?;

    // Le bon mot de passe ouvre.
    let u = db::user::authentifier(&pool, LOGIN, MDP)?;
    assert!(u.is_some(), "un compte a pass_crypted doit pouvoir entrer");
    assert_eq!(u.map(|u| u.login), Some(LOGIN.to_string()));

    // Un mauvais mot de passe ne passe pas.
    assert!(db::user::authentifier(&pool, LOGIN, "mauvais-mot-de-passe")?.is_none());

    oublier(&pool)?;
    Ok(())
}

/// Un mot de passe en clair dans `pass` n'authentifie personne.
///
/// Dolibarr n'y met une valeur que si `DATABASE_PWD_ENCRYPTED` est desactive ;
/// comparer a cette colonne reviendrait a accepter le clair.
#[test]
fn un_mot_de_passe_en_clair_ne_sert_pas_de_preuve() -> Result<(), Box<dyn Error>> {
    let _verrou = verrou_test();
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    oublier(&pool)?;
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_user (entity, login, pass, admin, statut, lastname, datec) \
         VALUES (:entity, :login, :pass, 1, 1, 'Test', NOW())",
        params! { "entity" => ENTITY, "login" => LOGIN, "pass" => MDP },
    )?;
    assert!(
        db::user::authentifier(&pool, LOGIN, MDP)?.is_none(),
        "le mot de passe en clair ne doit pas ouvrir la porte"
    );
    oublier(&pool)?;
    Ok(())
}

/// Une empreinte heritee (md5 dans `pass`) reste acceptee : les vieux comptes
/// n'ont que celle-la.
#[test]
fn une_empreinte_md5_heritee_passe_encore() -> Result<(), Box<dyn Error>> {
    let _verrou = verrou_test();
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    oublier(&pool)?;
    let mut conn = pool.get_conn()?;
    // md5("Copie2026!") — la forme que Dolibarr ecrivait avant la 13.
    let md5 = "79d1f0e4c2e8a032502b06d4eecabbcf";
    conn.exec_drop(
        "INSERT INTO llx_user (entity, login, pass, admin, statut, lastname, datec) \
         VALUES (:entity, :login, :pass, 1, 1, 'Test', NOW())",
        params! { "entity" => ENTITY, "login" => LOGIN, "pass" => md5 },
    )?;
    assert!(db::user::authentifier(&pool, LOGIN, MDP)?.is_some());
    oublier(&pool)?;
    Ok(())
}

/// Un compte cree par Cordeau doit s'ouvrir avec Dolibarr : l'empreinte va dans
/// `pass_crypted`, `pass` reste vide.
#[test]
fn un_compte_cree_ecrit_l_empreinte_au_bon_endroit() -> Result<(), Box<dyn Error>> {
    let _verrou = verrou_test();
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    oublier(&pool)?;
    let nouveau = db::user::NouvelUtilisateur {
        login: LOGIN.to_string(),
        mot_de_passe: MDP.to_string(),
        nom: "Test".to_string(),
        prenom: "Connexion".to_string(),
        email: None,
        admin: false,
    };
    let id = db::user::creer(&pool, ENTITY, &nouveau)?;

    let mut conn = pool.get_conn()?;
    let (crypte, clair): (Option<String>, Option<String>) = conn
        .exec_first(
            "SELECT pass_crypted, pass FROM llx_user WHERE rowid = :id",
            params! { "id" => id },
        )?
        .ok_or("compte non cree")?;
    assert!(
        crypte.as_deref().unwrap_or("").starts_with("$2"),
        "empreinte absente de pass_crypted"
    );
    assert_eq!(
        clair, None,
        "`pass` doit rester vide, comme le fait Dolibarr"
    );

    assert!(db::user::authentifier(&pool, LOGIN, MDP)?.is_some());
    oublier(&pool)?;
    Ok(())
}
