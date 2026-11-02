//! Groupes d'utilisateurs et droits herites, comme Dolibarr.
//!
//! Trois tables : `llx_usergroup` (le groupe), `llx_usergroup_user` (les
//! membres) et `llx_usergroup_rights` (les droits du groupe). Un utilisateur
//! possede les droits de ses groupes **et** ceux qui lui sont donnes en propre.

use std::collections::BTreeSet;
use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Groupe {
    pub rowid: i64,
    pub nom: String,
    pub note: Option<String>,
}

pub fn list(pool: &Pool, entity: i64) -> Result<Vec<Groupe>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<(i64, String, Option<String>)> = conn.exec(
        "SELECT rowid, nom, note FROM llx_usergroup \
         WHERE entity = :entity ORDER BY nom ASC",
        params! { "entity" => entity },
    )?;
    Ok(lignes
        .into_iter()
        .map(|(rowid, nom, note)| Groupe { rowid, nom, note })
        .collect())
}

pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Groupe>, Box<dyn Error>> {
    Ok(list(pool, entity)?.into_iter().find(|g| g.rowid == rowid))
}

pub fn creer(
    pool: &Pool,
    entity: i64,
    nom: &str,
    note: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let nom = nom.trim();
    if nom.is_empty() {
        return Err("le nom du groupe est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_usergroup WHERE entity = :entity AND nom = :nom",
        params! { "entity" => entity, "nom" => nom },
    )?;
    if deja.is_some() {
        return Err(format!("un groupe s'appelle deja « {nom} »").into());
    }
    conn.exec_drop(
        "INSERT INTO llx_usergroup (nom, entity, datec, note) VALUES (:nom, :entity, NOW(), :note)",
        params! { "nom" => nom, "entity" => entity, "note" => note },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Membres d'un groupe : `(rowid utilisateur, identifiant)`.
pub fn membres(
    pool: &Pool,
    entity: i64,
    groupe: i64,
) -> Result<Vec<(i64, String)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    Ok(conn.exec(
        "SELECT u.rowid, u.login FROM llx_usergroup_user m \
         JOIN llx_user u ON u.rowid = m.fk_user \
         WHERE m.entity = :entity AND m.fk_usergroup = :groupe \
         ORDER BY u.login ASC",
        params! { "entity" => entity, "groupe" => groupe },
    )?)
}

/// Ajoute ou retire un membre. Idempotent dans les deux sens.
pub fn definir_membre(
    pool: &Pool,
    entity: i64,
    groupe: i64,
    fk_user: i64,
    membre: bool,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    if membre {
        conn.exec_drop(
            "INSERT IGNORE INTO llx_usergroup_user (entity, fk_user, fk_usergroup) \
             VALUES (:entity, :user, :groupe)",
            params! { "entity" => entity, "user" => fk_user, "groupe" => groupe },
        )?;
    } else {
        conn.exec_drop(
            "DELETE FROM llx_usergroup_user \
             WHERE entity = :entity AND fk_user = :user AND fk_usergroup = :groupe",
            params! { "entity" => entity, "user" => fk_user, "groupe" => groupe },
        )?;
    }
    Ok(())
}

/// Droits portes par un groupe.
pub fn droits(pool: &Pool, groupe: i64) -> Result<BTreeSet<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT fk_id FROM llx_usergroup_rights WHERE fk_usergroup = :groupe",
        params! { "groupe" => groupe },
    )?;
    Ok(ids.into_iter().collect())
}

pub fn definir_droit(
    pool: &Pool,
    entity: i64,
    groupe: i64,
    id_droit: i64,
    actif: bool,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    if actif {
        conn.exec_drop(
            "INSERT IGNORE INTO llx_usergroup_rights (entity, fk_usergroup, fk_id) \
             VALUES (:entity, :groupe, :droit)",
            params! { "entity" => entity, "groupe" => groupe, "droit" => id_droit },
        )?;
    } else {
        conn.exec_drop(
            "DELETE FROM llx_usergroup_rights WHERE fk_usergroup = :groupe AND fk_id = :droit",
            params! { "groupe" => groupe, "droit" => id_droit },
        )?;
    }
    Ok(())
}

/// Groupes dont l'utilisateur est membre.
pub fn groupes_de(pool: &Pool, entity: i64, fk_user: i64) -> Result<Vec<Groupe>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<(i64, String, Option<String>)> = conn.exec(
        "SELECT g.rowid, g.nom, g.note FROM llx_usergroup_user m \
         JOIN llx_usergroup g ON g.rowid = m.fk_usergroup \
         WHERE m.entity = :entity AND m.fk_user = :user ORDER BY g.nom ASC",
        params! { "entity" => entity, "user" => fk_user },
    )?;
    Ok(lignes
        .into_iter()
        .map(|(rowid, nom, note)| Groupe { rowid, nom, note })
        .collect())
}

/// Droits effectifs : ceux du compte, plus ceux de tous ses groupes.
pub fn droits_effectifs(
    pool: &Pool,
    entity: i64,
    fk_user: i64,
) -> Result<BTreeSet<i64>, Box<dyn Error>> {
    let mut effectifs = crate::droit::droits(pool, fk_user)?;
    for g in groupes_de(pool, entity, fk_user)? {
        effectifs.extend(droits(pool, g.rowid)?);
    }
    Ok(effectifs)
}
