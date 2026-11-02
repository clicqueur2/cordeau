use std::error::Error;

use db::user::{NouvelUtilisateur, STATUT_DESACTIVE};
use mysql::Pool;

const ENTITY: i64 = 1;

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test ignore");
            None
        }
    }
}

/// Un identifiant par test : les fichiers de test tournent en parallele.
fn nettoyer(pool: &Pool, login: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    use mysql::prelude::Queryable;
    conn.exec_drop(
        "DELETE FROM llx_usergroup_user WHERE fk_user IN \
         (SELECT rowid FROM llx_user WHERE login = ?)",
        (login,),
    )?;
    conn.exec_drop(
        "DELETE FROM llx_user_rights WHERE fk_user IN \
         (SELECT rowid FROM llx_user WHERE login = ?)",
        (login,),
    )?;
    conn.exec_drop("DELETE FROM llx_user WHERE login = ?", (login,))?;
    Ok(())
}

fn nouveau(login: &str) -> NouvelUtilisateur {
    NouvelUtilisateur {
        login: login.into(),
        mot_de_passe: "MotDePasse!2026".into(),
        nom: "Rigot".into(),
        prenom: "Maxime".into(),
        email: Some("maxime@exemple.test".into()),
        admin: true,
    }
}

#[test]
fn cree_un_utilisateur_et_verifie_son_mot_de_passe() -> Result<(), Box<dyn Error>> {
    let login = "test.socle.un";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, login)?;

    let id = db::user::creer(&pool, ENTITY, &nouveau(login))?;
    assert!(id > 0);

    let u = db::user::get(&pool, id)?.expect("utilisateur");
    assert_eq!(u.login, login);
    assert_eq!(u.nom_complet(), "Maxime Rigot");
    assert!(u.admin);
    assert!(u.actif());

    let connecte = db::user::authentifier(&pool, login, "MotDePasse!2026")?;
    assert!(connecte.is_some(), "le bon mot de passe doit passer");
    assert!(
        connecte.unwrap().datelastlogin.is_some(),
        "date de connexion mise a jour"
    );

    assert!(db::user::authentifier(&pool, login, "mauvais")?.is_none());

    // Un compte desactive ne se connecte plus.
    db::user::activer(&pool, id, false)?;
    assert_eq!(db::user::get(&pool, id)?.unwrap().statut, STATUT_DESACTIVE);
    assert!(db::user::authentifier(&pool, login, "MotDePasse!2026")?.is_none());

    nettoyer(&pool, login)?;
    Ok(())
}

#[test]
fn refuse_un_login_deja_pris_et_un_mot_de_passe_court() -> Result<(), Box<dyn Error>> {
    let login = "test.socle.deux";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, login)?;
    db::user::creer(&pool, ENTITY, &nouveau(login))?;

    let err = db::user::creer(&pool, ENTITY, &nouveau(login)).expect_err("doublon refuse");
    assert!(err.to_string().contains("existe deja"), "{err}");

    let mut court = nouveau("test.socle.court");
    court.mot_de_passe = "abc".into();
    assert!(db::user::creer(&pool, ENTITY, &court).is_err());

    nettoyer(&pool, login)?;
    nettoyer(&pool, "test.socle.court")?;
    Ok(())
}

#[test]
fn change_le_mot_de_passe() -> Result<(), Box<dyn Error>> {
    let login = "test.socle.trois";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, login)?;
    let id = db::user::creer(&pool, ENTITY, &nouveau(login))?;

    db::user::definir_mot_de_passe(&pool, id, "NouveauSecret2026")?;
    assert!(db::user::authentifier(&pool, login, "MotDePasse!2026")?.is_none());
    assert!(db::user::authentifier(&pool, login, "NouveauSecret2026")?.is_some());

    nettoyer(&pool, login)?;
    Ok(())
}

#[test]
fn les_groupes_et_les_droits_se_lisent() -> Result<(), Box<dyn Error>> {
    let login = "test.socle.quatre";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, login)?;
    let id = db::user::creer(&pool, ENTITY, &nouveau(login))?;

    let mut conn = pool.get_conn()?;
    use mysql::prelude::Queryable;
    // Restes d'une execution precedente interrompue.
    conn.exec_drop(
        "DELETE FROM llx_usergroup_user WHERE fk_usergroup IN \
         (SELECT rowid FROM llx_usergroup WHERE nom = 'Test Groupe')",
        (),
    )?;
    conn.exec_drop("DELETE FROM llx_usergroup WHERE nom = 'Test Groupe'", ())?;
    conn.exec_drop(
        "INSERT INTO llx_usergroup (nom, entity, datec) VALUES ('Test Groupe', ?, NOW())",
        (ENTITY,),
    )?;
    let groupe = conn.last_insert_id() as i64;
    conn.exec_drop(
        "INSERT INTO llx_usergroup_user (entity, fk_user, fk_usergroup) VALUES (?, ?, ?)",
        (ENTITY, id, groupe),
    )?;

    let groupes = db::user::groupes(&pool, ENTITY, id)?;
    assert_eq!(groupes.len(), 1);
    assert_eq!(groupes[0].1, "Test Groupe");

    assert!(!db::user::a_le_droit(&pool, id, 1)?);

    conn.exec_drop(
        "DELETE FROM llx_usergroup_user WHERE fk_usergroup = ?",
        (groupe,),
    )?;
    conn.exec_drop("DELETE FROM llx_usergroup WHERE rowid = ?", (groupe,))?;
    nettoyer(&pool, login)?;
    Ok(())
}
