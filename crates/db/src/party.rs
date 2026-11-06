//! Tiers (`llx_societe`).
//!
//! Regles du plan respectees : liste de colonnes explicite a la lecture comme a
//! l'ecriture, filtre `entity` systematique, aucune dependance a une colonne
//! ajoutee par un module.

use mysql::prelude::*;
use mysql::{params, Pool, Row};
use std::error::Error;

pub const CLIENT_NI: i16 = 0;
pub const CLIENT_CLIENT: i16 = 1;
pub const CLIENT_PROSPECT: i16 = 2;
pub const CLIENT_PROSPECT_CLIENT: i16 = 3;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Party {
    pub rowid: i64,
    pub entity: i64,
    pub nom: String,
    pub name_alias: Option<String>,
    pub client: i16,
    pub fournisseur: i16,
    pub code_client: Option<String>,
    pub code_fournisseur: Option<String>,
    pub address: Option<String>,
    pub zip: Option<String>,
    pub town: Option<String>,
    pub fk_pays: Option<i64>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub siren: Option<String>,
    pub siret: Option<String>,
    pub tva_intra: Option<String>,
    pub statut: i16,
    pub datec: Option<String>,
    /// Condition de reglement par defaut du tiers (`llx_societe.cond_reglement`).
    pub cond_reglement: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct NewParty {
    pub nom: String,
    pub name_alias: Option<String>,
    pub client: i16,
    pub fournisseur: i16,
    pub address: Option<String>,
    pub zip: Option<String>,
    pub town: Option<String>,
    pub fk_pays: Option<i64>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub siren: Option<String>,
    pub siret: Option<String>,
    pub tva_intra: Option<String>,
}

impl NewParty {
    /// Un client cree ici vaut 3, comme dans Dolibarr.
    ///
    /// `htdocs/societe/card.php` ecrit `client = (customer>0?1:0) + (prospect>0?2:0)` :
    /// un client est aussi un prospect, donc 3. Ecrire 1 ferait disparaitre la
    /// fiche des listes qui filtrent sur `client = 3`.
    pub fn client(nom: &str) -> Self {
        Self {
            nom: nom.to_string(),
            client: 3,
            ..Default::default()
        }
    }
}

const SELECT_COLS: &str = "rowid, entity, nom, name_alias, client, fournisseur, \
     code_client, code_fournisseur, address, zip, town, fk_pays, phone, email, \
     siren, siret, tva_intra, statut, \
     DATE_FORMAT(datec, '%Y-%m-%d %H:%i:%s'), cond_reglement";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

/// Un texte lu en base, ou `None` s'il est vide.
///
/// Dolibarr n'ecrit jamais `NULL` dans ces colonnes : il ecrit la chaine vide.
/// Sur la base de production, 912 tiers sur 932 portent `siret = ''`.
/// `Some("")` n'est donc pas une valeur, c'est une absence — et les confondre a
/// un effet mesurable : un particulier sans SIRET passait pour un
/// professionnel, et sa facture etait refusee faute de numero de TVA
/// intracommunautaire. Toutes les factures de particuliers y passaient.
fn opt_txt(row: &Row, i: usize) -> Option<String> {
    opt_s(row, i).filter(|v| !v.trim().is_empty())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_i16(row: &Row, i: usize) -> Option<i16> {
    row.get_opt::<i64, _>(i)
        .and_then(|r| r.ok())
        .map(|v| v as i16)
}

fn map_row(row: Row) -> Party {
    Party {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        nom: opt_s(&row, 2).unwrap_or_default(),
        name_alias: opt_s(&row, 3),
        client: opt_i16(&row, 4).unwrap_or(0),
        fournisseur: opt_i16(&row, 5).unwrap_or(0),
        code_client: opt_s(&row, 6),
        code_fournisseur: opt_s(&row, 7),
        address: opt_s(&row, 8),
        zip: opt_s(&row, 9),
        town: opt_s(&row, 10),
        fk_pays: opt_i(&row, 11),
        phone: opt_s(&row, 12),
        email: opt_s(&row, 13),
        siren: opt_txt(&row, 14),
        siret: opt_txt(&row, 15),
        tva_intra: opt_txt(&row, 16),
        statut: opt_i16(&row, 17).unwrap_or(1),
        datec: opt_s(&row, 18),
        cond_reglement: opt_i(&row, 19),
    }
}

pub fn count(pool: &Pool, entity: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_societe WHERE entity = ?",
        (entity,),
    )?;
    Ok(n.unwrap_or(0))
}

pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Party>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_societe WHERE entity = ? \
         ORDER BY nom ASC, rowid ASC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limit, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Liste filtree par une condition SQL interne (prospects, fournisseurs).
///
/// La condition n'est jamais construite a partir d'une saisie : elle vient
/// d'une liste fermee cote appelant.
pub fn list_filtre(
    pool: &Pool,
    entity: i64,
    condition: &str,
    limit: u32,
) -> Result<Vec<Party>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_societe WHERE entity = :entity AND {condition} \
         ORDER BY nom ASC, rowid ASC LIMIT :limit"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "limit" => limit })?;
    Ok(rows.into_iter().map(map_row).collect())
}

pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Party>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} FROM llx_societe WHERE entity = ? AND rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// La societe qui detient ce tiers, si le tiers existe ailleurs.
///
/// Sert a distinguer deux situations qu'on confond sinon : un tiers supprime,
/// et un tiers qui appartient a une autre societe. La seconde arrive en vrai —
/// un chantier de Pierre Angulaire pointant sur un tiers de 2AR — et dire
/// « absent » envoie chercher une donnee qui existe.
pub fn entity_du_tiers(pool: &Pool, rowid: i64) -> Result<Option<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let e: Option<i64> = conn.exec_first(
        "SELECT entity FROM llx_societe WHERE rowid = ? LIMIT 1",
        (rowid,),
    )?;
    Ok(e)
}

/// Code client au format de `mod_codeclient_leopard` : `CU` + `AAAMM` + `-` + 5 chiffres.
///
/// Le numero suivant vient du plus grand suffixe existant pour la periode, plus
/// un decalage d'essai : en cas de collision (ecriture concurrente), on reessaie
/// sans rejouer le meme numero.
fn next_code_client(
    conn: &mut mysql::PooledConn,
    entity: i64,
    offset: u32,
) -> Result<String, Box<dyn Error>> {
    let period: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%y%m')")?;
    let period = period.unwrap_or_else(|| "0001".to_string());
    let prefix = format!("CU{period}-");

    let dernier: Option<Option<i64>> = conn.exec_first(
        "SELECT MAX(CAST(SUBSTRING(code_client, 8) AS UNSIGNED)) \
         FROM llx_societe WHERE entity = ? AND code_client LIKE ?",
        (entity, format!("{prefix}%")),
    )?;
    let suivant = dernier.flatten().unwrap_or(0) + 1 + i64::from(offset);
    Ok(format!("{prefix}{suivant:05}"))
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

pub fn create(pool: &Pool, entity: i64, new: &NewParty) -> Result<i64, Box<dyn Error>> {
    if new.nom.trim().is_empty() {
        return Err("le nom du tiers est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;

    for attempt in 0..5u32 {
        let code = next_code_client(&mut conn, entity, attempt)?;
        let result = conn.exec_drop(
            "INSERT INTO llx_societe \
             (entity, nom, name_alias, client, fournisseur, code_client, code_fournisseur, \
              address, zip, town, fk_pays, phone, email, siren, siret, tva_intra, \
              statut, datec) \
             VALUES (:entity, :nom, :name_alias, :client, :fournisseur, :code_client, \
              :code_fournisseur, :address, :zip, :town, :fk_pays, :phone, :email, \
              :siren, :siret, :tva_intra, :statut, NOW())",
            params! {
                "entity" => entity,
                "nom" => new.nom.trim(),
                "name_alias" => new.name_alias.as_deref(),
                "client" => new.client,
                "fournisseur" => new.fournisseur,
                "code_client" => code,
                "code_fournisseur" => None::<String>,
                "address" => new.address.as_deref(),
                "zip" => new.zip.as_deref(),
                "town" => new.town.as_deref(),
                "fk_pays" => new.fk_pays,
                "phone" => new.phone.as_deref(),
                "email" => new.email.as_deref(),
                "siren" => new.siren.as_deref(),
                "siret" => new.siret.as_deref(),
                "tva_intra" => new.tva_intra.as_deref(),
                "statut" => 1i16,
            },
        );

        match result {
            Ok(()) => return Ok(conn.last_insert_id() as i64),
            Err(e) if is_duplicate(&e) && attempt < 4 => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer un code client unique".into())
}

/// Met a jour une fiche tiers. Les champs `None` sont laisses tels quels.
pub fn update(pool: &Pool, entity: i64, rowid: i64, new: &NewParty) -> Result<(), Box<dyn Error>> {
    if new.nom.trim().is_empty() {
        return Err("le nom du tiers est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    if existe.is_none() {
        return Err(format!("tiers {rowid} introuvable").into());
    }
    conn.exec_drop(
        "UPDATE llx_societe SET nom = :nom, name_alias = :name_alias, \
          address = :address, zip = :zip, town = :town, email = :email, \
          phone = :phone, siret = :siret, tva_intra = :tva_intra, tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "nom" => new.nom.trim(),
            "name_alias" => new.name_alias.as_deref(),
            "address" => new.address.as_deref(),
            "zip" => new.zip.as_deref(),
            "town" => new.town.as_deref(),
            "email" => new.email.as_deref(),
            "phone" => new.phone.as_deref(),
            "siret" => new.siret.as_deref(),
            "tva_intra" => new.tva_intra.as_deref(),
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

pub fn set_statut(pool: &Pool, entity: i64, rowid: i64, statut: i16) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_societe SET statut = ?, tms = NOW() WHERE entity = ? AND rowid = ?",
        (statut, entity, rowid),
    )?;
    Ok(())
}

/// Suppression physique : reservee aux bases de test.
pub fn delete(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_societe WHERE entity = ? AND rowid = ?",
        (entity, rowid),
    )?;
    Ok(())
}
