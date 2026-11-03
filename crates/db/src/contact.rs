//! Contacts et adresses (`llx_socpeople`), rattaches ou non a un tiers.
//!
//! Dolibarr distingue le tiers (la societe) du contact (la personne). Un contact
//! peut exister sans societe : c'est le cas des particuliers et des prospects
//! pas encore qualifies.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contact {
    pub rowid: i64,
    pub fk_soc: Option<i64>,
    pub tiers: Option<String>,
    pub civilite: Option<String>,
    pub nom: String,
    pub prenom: Option<String>,
    pub poste: Option<String>,
    pub email: Option<String>,
    pub telephone: Option<String>,
    pub ville: Option<String>,
    pub actif: bool,
}

impl Contact {
    /// Nom affichable : « M. Dupont Jean » ou « Dupont Jean ».
    pub fn nom_complet(&self) -> String {
        let mut morceaux = Vec::new();
        if let Some(c) = self.civilite.as_deref().filter(|c| !c.trim().is_empty()) {
            morceaux.push(c.trim());
        }
        if !self.nom.trim().is_empty() {
            morceaux.push(self.nom.trim());
        }
        if let Some(p) = self.prenom.as_deref().filter(|p| !p.trim().is_empty()) {
            morceaux.push(p.trim());
        }
        morceaux.join(" ")
    }
}

const SELECT_COLS: &str = "c.rowid, c.fk_soc, s.nom, c.civility, c.lastname, c.firstname, \
     c.poste, c.email, c.phone, c.town, c.statut";

fn map_row(r: &mysql::Row) -> Contact {
    let s = |i: usize| r.get_opt::<String, _>(i).and_then(|v| v.ok());
    let i = |i: usize| r.get_opt::<i64, _>(i).and_then(|v| v.ok());
    Contact {
        rowid: i(0).unwrap_or(0),
        fk_soc: i(1),
        tiers: s(2),
        civilite: s(3),
        nom: s(4).unwrap_or_default(),
        prenom: s(5),
        poste: s(6),
        email: s(7),
        telephone: s(8),
        ville: s(9),
        actif: i(10).unwrap_or(1) != 0,
    }
}

/// Contacts de l'entite, du plus recent au plus ancien.
pub fn list(pool: &Pool, entity: i64, limit: u32) -> Result<Vec<Contact>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_socpeople c \
         LEFT JOIN llx_societe s ON s.rowid = c.fk_soc \
         WHERE c.entity = :entity ORDER BY c.rowid DESC LIMIT :limit"
    );
    let rows: Vec<mysql::Row> = conn.exec(sql, params! { "entity" => entity, "limit" => limit })?;
    Ok(rows.iter().map(map_row).collect())
}

/// Contacts d'un tiers.
pub fn par_tiers(pool: &Pool, entity: i64, fk_soc: i64) -> Result<Vec<Contact>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_socpeople c \
         LEFT JOIN llx_societe s ON s.rowid = c.fk_soc \
         WHERE c.entity = :entity AND c.fk_soc = :soc ORDER BY c.lastname ASC"
    );
    let rows: Vec<mysql::Row> = conn.exec(sql, params! { "entity" => entity, "soc" => fk_soc })?;
    Ok(rows.iter().map(map_row).collect())
}

/// Cree un contact. Le nom est obligatoire, le reste facultatif.
#[allow(clippy::too_many_arguments)]
pub fn creer(
    pool: &Pool,
    entity: i64,
    fk_soc: Option<i64>,
    civilite: Option<&str>,
    nom: &str,
    prenom: Option<&str>,
    email: Option<&str>,
    telephone: Option<&str>,
    poste: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let nom = nom.trim();
    if nom.is_empty() {
        return Err("le nom du contact est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    if let Some(soc) = fk_soc {
        let existe: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :id",
            params! { "entity" => entity, "id" => soc },
        )?;
        if existe.is_none() {
            return Err(format!("le tiers {soc} n'existe pas dans l'entite {entity}").into());
        }
    }
    conn.exec_drop(
        // `fk_user_creat` a une vraie cle etrangere vers `llx_user` : sa valeur
        // par defaut (0) la fait echouer. On met l'utilisateur le plus ancien.
        "INSERT INTO llx_socpeople \
         (datec, fk_soc, entity, civility, lastname, firstname, poste, email, phone, statut, \
          fk_user_creat) \
         VALUES (NOW(), :soc, :entity, :civilite, :nom, :prenom, :poste, :email, :tel, 1, \
          (SELECT MIN(rowid) FROM llx_user))",
        params! {
            "soc" => fk_soc,
            "entity" => entity,
            "civilite" => civilite,
            "nom" => nom,
            "prenom" => prenom,
            "poste" => poste,
            "email" => email,
            "tel" => telephone,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Desactive un contact (on ne supprime pas une fiche qui a un historique).
pub fn desactiver(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_socpeople SET statut = 0 WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => rowid },
    )?;
    Ok(())
}

/// Un contact precis.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Contact>, Box<dyn Error>> {
    Ok(list(pool, entity, 10_000)?
        .into_iter()
        .find(|c| c.rowid == rowid))
}

/// Met a jour un contact. Le nom reste obligatoire.
#[allow(clippy::too_many_arguments)]
pub fn modifier(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    nom: &str,
    prenom: Option<&str>,
    email: Option<&str>,
    telephone: Option<&str>,
    poste: Option<&str>,
    ville: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let nom = nom.trim();
    if nom.is_empty() {
        return Err("le nom du contact est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_socpeople SET lastname = :nom, firstname = :prenom, email = :email, \
          phone = :tel, poste = :poste, town = :ville, tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "nom" => nom,
            "prenom" => prenom,
            "email" => email,
            "tel" => telephone,
            "poste" => poste,
            "ville" => ville,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}
