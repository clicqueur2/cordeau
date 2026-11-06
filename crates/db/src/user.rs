//! Utilisateurs (`llx_user`) et groupes.
//!
//! Les mots de passe sont ceux de Dolibarr : `pass` porte l'empreinte, verifiee
//! par le crate `auth` (bcrypt `$2y$`, md5 historique). Les comptes crees ici
//! fonctionnent donc aussi dans Dolibarr, et inversement.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Utilisateur {
    pub rowid: i64,
    pub entity: i64,
    pub login: String,
    pub nom: String,
    pub prenom: String,
    pub email: Option<String>,
    pub admin: bool,
    pub statut: i16,
    pub lang: Option<String>,
    pub datelastlogin: Option<String>,
}

impl Utilisateur {
    pub fn nom_complet(&self) -> String {
        let n = format!("{} {}", self.prenom.trim(), self.nom.trim());
        if n.trim().is_empty() {
            self.login.clone()
        } else {
            n.trim().to_string()
        }
    }

    pub fn actif(&self) -> bool {
        self.statut == 1
    }
}

#[derive(Debug, Clone, Default)]
pub struct NouvelUtilisateur {
    pub login: String,
    pub mot_de_passe: String,
    pub nom: String,
    pub prenom: String,
    pub email: Option<String>,
    pub admin: bool,
}

pub const STATUT_ACTIF: i16 = 1;
pub const STATUT_DESACTIVE: i16 = 0;

fn map_row(row: &mysql::Row) -> Utilisateur {
    let s = |i: usize| row.get_opt::<String, _>(i).and_then(|r| r.ok());
    let n = |i: usize| row.get_opt::<i64, _>(i).and_then(|r| r.ok());
    Utilisateur {
        rowid: n(0).unwrap_or(0),
        entity: n(1).unwrap_or(1),
        login: s(2).unwrap_or_default(),
        nom: s(3).unwrap_or_default(),
        prenom: s(4).unwrap_or_default(),
        email: s(5),
        admin: n(6).unwrap_or(0) == 1,
        statut: n(7).unwrap_or(0) as i16,
        lang: s(8),
        datelastlogin: s(9),
    }
}

const COLS: &str = "rowid, entity, login, lastname, firstname, email, admin, statut, lang, \
     DATE_FORMAT(datelastlogin, '%Y-%m-%d %H:%i')";

pub fn list(pool: &Pool, entity: i64) -> Result<Vec<Utilisateur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {COLS} FROM llx_user WHERE entity IN (0, :entity) \
         ORDER BY statut DESC, lastname ASC, login ASC"
    );
    let rows: Vec<mysql::Row> = conn.exec(sql, params! { "entity" => entity })?;
    Ok(rows.iter().map(map_row).collect())
}

pub fn get(pool: &Pool, rowid: i64) -> Result<Option<Utilisateur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {COLS} FROM llx_user WHERE rowid = ?");
    let row: Option<mysql::Row> = conn.exec_first(sql, (rowid,))?;
    Ok(row.as_ref().map(map_row))
}

pub fn par_login(pool: &Pool, login: &str) -> Result<Option<Utilisateur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {COLS} FROM llx_user WHERE login = ? LIMIT 1");
    let row: Option<mysql::Row> = conn.exec_first(sql, (login,))?;
    Ok(row.as_ref().map(map_row))
}

/// Cree un compte. Le mot de passe est stocke au format de Dolibarr (`$2y$`).
pub fn creer(pool: &Pool, entity: i64, nouveau: &NouvelUtilisateur) -> Result<i64, Box<dyn Error>> {
    if nouveau.login.trim().is_empty() {
        return Err("l'identifiant est obligatoire".into());
    }
    if nouveau.mot_de_passe.len() < 8 {
        return Err("le mot de passe doit faire au moins 8 caracteres".into());
    }
    let mut conn = pool.get_conn()?;

    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_user WHERE login = ? LIMIT 1",
        (nouveau.login.trim(),),
    )?;
    if existe.is_some() {
        return Err(format!("l'identifiant « {} » existe deja", nouveau.login).into());
    }

    let empreinte = auth::hash(&nouveau.mot_de_passe);
    // Dolibarr 24 ecrit l'empreinte dans `pass_crypted` et laisse `pass` a NULL
    // (`User::setPassword()` : `pass_crypted = dol_hash(...)` puis `pass = null`
    // quand `DATABASE_PWD_ENCRYPTED`). Ecrire dans `pass` creerait un compte que
    // Dolibarr refuserait.
    conn.exec_drop(
        "INSERT INTO llx_user \
         (entity, login, pass_crypted, pass_encoding, lastname, firstname, email, admin, \
          employee, statut, datec, lang, datelastpassvalidation) \
         VALUES (:entity, :login, :pass, 'bcrypt', :nom, :prenom, :email, :admin, \
          1, :statut, NOW(), 'fr_FR', NOW())",
        params! {
            "entity" => entity,
            "login" => nouveau.login.trim(),
            "pass" => empreinte.as_str(),
            "nom" => nouveau.nom.trim(),
            "prenom" => nouveau.prenom.trim(),
            "email" => nouveau.email.as_deref(),
            "admin" => if nouveau.admin { 1 } else { 0 },
            "statut" => STATUT_ACTIF,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Verifie un couple identifiant / mot de passe. Met a jour la date de connexion.
///
/// L'empreinte vit dans `pass_crypted` : c'est la colonne que Dolibarr lit
/// (`User::setPassword()`, puis `dol_verifyHash($password,
/// $this->pass_indatabase_crypted)`). `pass` ne porte plus rien depuis la 13 —
/// il n'est rempli qu'en clair quand `DATABASE_PWD_ENCRYPTED` est desactive, et
/// comparer a lui reviendrait a accepter un mot de passe en clair.
///
/// Lire `pass` seul refusait **tous les comptes reels** : sur la copie de
/// production, les dix-sept comptes ont `pass_crypted` et `pass` a NULL. Seul le
/// compte de test, ecrit par Cordeau a l'ancienne, pouvait entrer.
pub fn authentifier(
    pool: &Pool,
    login: &str,
    mot_de_passe: &str,
) -> Result<Option<Utilisateur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ligne: Option<(i64, Option<String>, Option<String>, String)> = conn.exec_first(
        "SELECT rowid, pass_crypted, pass, IFNULL(pass_encoding, '') FROM llx_user \
         WHERE login = :login AND statut = 1 LIMIT 1",
        params! { "login" => login.trim() },
    )?;
    let Some((rowid, crypte, herite, encodage)) = ligne else {
        return Ok(None);
    };

    // `pass_crypted` d'abord ; `pass` seulement s'il porte une empreinte
    // reconnaissable (md5, sha1, bcrypt). Un mot de passe en clair n'authentifie
    // personne.
    let accepte = [crypte, herite]
        .iter()
        .filter_map(|e| e.as_deref())
        .map(str::trim)
        .filter(|e| {
            !e.is_empty()
                && (e.starts_with("$2")
                    || e.starts_with("$argon")
                    || e.len() == 32
                    || e.len() == 40)
        })
        .any(|e| auth::verify(mot_de_passe, e, Some(&encodage)));

    if !accepte {
        return Ok(None);
    }
    conn.exec_drop(
        "UPDATE llx_user SET datelastlogin = datepreviouslogin, \
          datepreviouslogin = datelastlogin, datelastlogin = NOW() WHERE rowid = :rowid",
        params! { "rowid" => rowid },
    )?;
    get(pool, rowid)
}

pub fn definir_mot_de_passe(
    pool: &Pool,
    rowid: i64,
    mot_de_passe: &str,
) -> Result<(), Box<dyn Error>> {
    if mot_de_passe.len() < 8 {
        return Err("le mot de passe doit faire au moins 8 caracteres".into());
    }
    let mut conn = pool.get_conn()?;
    // Meme colonne que Dolibarr, et `pass` vide comme il le fait : un mot de
    // passe change ici doit ouvrir la porte des deux cotes.
    conn.exec_drop(
        "UPDATE llx_user SET pass_crypted = :pass, pass = NULL, pass_encoding = 'bcrypt', \
          datelastpassvalidation = NOW() WHERE rowid = :rowid",
        params! { "pass" => auth::hash(mot_de_passe), "rowid" => rowid },
    )?;
    Ok(())
}

pub fn activer(pool: &Pool, rowid: i64, actif: bool) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_user SET statut = :statut, tms = NOW() WHERE rowid = :rowid",
        params! {
            "statut" => if actif { STATUT_ACTIF } else { STATUT_DESACTIVE },
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Groupes d'un utilisateur (`llx_usergroup` + `llx_usergroup_user`).
pub fn groupes(
    pool: &Pool,
    entity: i64,
    fk_user: i64,
) -> Result<Vec<(i64, String)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<(i64, String)> = conn.exec(
        "SELECT g.rowid, g.nom FROM llx_usergroup g \
         INNER JOIN llx_usergroup_user gu ON gu.fk_usergroup = g.rowid \
         WHERE gu.fk_user = :user AND g.entity IN (0, :entity) ORDER BY g.nom",
        params! { "user" => fk_user, "entity" => entity },
    )?;
    Ok(lignes)
}

/// `true` si le compte detient le droit (`llx_rights_def.id`).
pub fn a_le_droit(pool: &Pool, fk_user: i64, id_droit: i64) -> Result<bool, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_user_rights WHERE fk_user = :user AND fk_id = :droit",
        params! { "user" => fk_user, "droit" => id_droit },
    )?;
    Ok(n.unwrap_or(0) > 0)
}
