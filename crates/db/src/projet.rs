//! Projets / chantiers (`llx_projet`) et leurs taches (`llx_projet_task`).
//!
//! Numerotation reprise du style des autres modules : `PRJ` + `AAAMM` + `-` +
//! numero sur 4 chiffres. Comme pour les devis et les factures, le maximum est
//! cherche sur **toutes** les periodes (`LIKE 'PRJ____-%'`) : le compteur ne
//! repart donc pas a zero au changement de mois, ce qui evite de reattribuer une
//! reference deja imprimee sur un planning.
//!
//! Les montants (`budget_amount`, `double(24,8)`) sortent en `CAST(... AS CHAR)`
//! avant `Decimal::from_str`, comme partout ailleurs : aucune somme ne transite
//! par un flottant.
//!
//! Pieges de schema verifies sur `schema-24.0.1.json` et sur le DDL de
//! `install/mysql/tables/` :
//! - `llx_projet` n'a **pas** de `date_start` / `date_end` : les dates du
//!   chantier sont `dateo` (debut) et `datee` (fin), toutes deux de type `date` ;
//! - `llx_projet.fk_user_creat` est `NOT NULL` **sans valeur par defaut** ;
//! - `llx_projet_task.ref` est *nullable* mais porte l'index unique
//!   `uk_projet_task_ref (ref, entity)` : on ecrit `NULL` puis on renseigne
//!   `T<rowid>`, un `''` en dur entrerait en collision des la deuxieme tache.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

/// Prefixe des references de projet.
pub const PREFIX_PROJET: &str = "PRJ";

/// Statut d'un projet (`llx_projet.fk_statut`) : 0 ouvert, 2 clos.
pub const STATUT_OUVERT: i16 = 0;
pub const STATUT_CLOS: i16 = 2;

/// `llx_projet.fk_user_creat` est `NOT NULL` sans valeur par defaut : un `INSERT`
/// qui l'omet est refuse en mode strict. Le noyau n'a pas de session utilisateur
/// ouverte, on ecrit donc l'identifiant systeme — cette colonne n'a **aucune cle
/// etrangere** vers `llx_user`, Dolibarr affiche simplement « aucun
/// utilisateur » sur la fiche.
///
/// `llx_projet_task.fk_user_creat`, elle, porte `fk_projet_task_fk_user_creat`
/// vers `llx_user` : 0 y serait refuse. La colonne etant *nullable*, une tache
/// creee par le noyau porte `NULL` (voir `creer_tache`).
pub const FK_USER_CREAT_DEFAUT: i64 = 0;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Projet {
    pub rowid: i64,
    pub ref_: String,
    pub title: String,
    pub fk_soc: Option<i64>,
    /// Debut du chantier (colonne `dateo`, de type `date`).
    pub date_start: Option<String>,
    /// Fin prevue du chantier (colonne `datee`, de type `date`).
    pub date_end: Option<String>,
    pub fk_statut: i16,
    /// Budget previsionnel (colonne `budget_amount`), arrondi au centime.
    pub budget: Option<Decimal>,
    pub description: Option<String>,
    pub entity: i64,
}

impl Projet {
    pub fn est_ouvert(&self) -> bool {
        self.fk_statut == STATUT_OUVERT
    }
}

#[derive(Debug, Clone, Default)]
pub struct NewProjet {
    /// Tiers commanditaire, facultatif : un chantier interne n'en a pas.
    pub fk_soc: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    /// `None` = non renseignee (colonne laissee a `NULL`, jamais `0000-00-00`).
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    /// Budget previsionnel. Arrondi au centime a l'ecriture.
    pub budget: Decimal,
}

impl NewProjet {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tache {
    pub rowid: i64,
    pub fk_projet: i64,
    /// Reference `T<rowid>`, posee juste apres l'insertion. Le suffixe `_` est
    /// impose : `ref` est un mot-cle Rust.
    pub ref_: Option<String>,
    pub label: String,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    /// Avancement en pourcentage (colonne `progress`).
    pub progress: i32,
    /// Createur de la tache (`fk_user_creat`).
    pub fk_user: Option<i64>,
}

const SELECT_COLS: &str = "rowid, entity, ref, title, fk_soc, \
     DATE_FORMAT(dateo, '%Y-%m-%d'), DATE_FORMAT(datee, '%Y-%m-%d'), \
     fk_statut, CAST(budget_amount AS CHAR), description";

const TASK_COLS: &str = "t.rowid, t.fk_projet, t.ref, t.label, \
     DATE_FORMAT(t.dateo, '%Y-%m-%d %H:%i:%s'), DATE_FORMAT(t.datee, '%Y-%m-%d %H:%i:%s'), \
     t.progress, t.fk_user_creat";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Projet {
    Projet {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        title: opt_s(&row, 3).unwrap_or_default(),
        fk_soc: opt_i(&row, 4),
        date_start: opt_s(&row, 5),
        date_end: opt_s(&row, 6),
        fk_statut: opt_i(&row, 7).unwrap_or(0) as i16,
        budget: opt_dec(&row, 8),
        description: opt_s(&row, 9),
    }
}

fn map_tache(row: Row) -> Tache {
    Tache {
        rowid: opt_i(&row, 0).unwrap_or(0),
        fk_projet: opt_i(&row, 1).unwrap_or(0),
        ref_: opt_s(&row, 2),
        label: opt_s(&row, 3).unwrap_or_default(),
        date_start: opt_s(&row, 4),
        date_end: opt_s(&row, 5),
        progress: opt_i(&row, 6).unwrap_or(0) as i32,
        fk_user: opt_i(&row, 7),
    }
}

/// Derniers projets de l'entite.
pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Projet>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_projet WHERE entity = :entity \
         ORDER BY rowid DESC LIMIT :limit OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limit" => limit, "offset" => offset },
    )?;
    Ok(rows.into_iter().map(map_row).collect())
}

pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Projet>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql =
        format!("SELECT {SELECT_COLS} FROM llx_projet WHERE entity = :entity AND rowid = :rowid");
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.map(map_row))
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

/// Interblocage ou attente de verrou depassee : l'ecriture doit etre rejouee.
fn is_deadlock(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1213 || me.code == 1205)
}

/// Normalise une date saisie : `None` ou vide reste `NULL`, sinon la valeur est
/// transmise telle quelle (colonnes `date` / `datetime`). Une chaine vide dans
/// une colonne `date` donne `0000-00-00` ou une erreur en mode strict : on
/// n'ecrit donc jamais que `NULL` ou une date.
fn date_ou_none(d: Option<&str>) -> Option<&str> {
    d.map(str::trim).filter(|s| !s.is_empty())
}

/// Reference suivante : `PRJ` + `AAAMM` + `-` + numero sur 4 chiffres.
///
/// `$posindice` equivalent = longueur du prefixe + 6 (`PRJ` = 3, `AAAMM` = 4,
/// `-` = 1) : le premier chiffre commence donc au 9e caractere. L'`offset` sert
/// au reessai en cas de collision sur `uk_projet_ref`.
fn next_ref<Q: Queryable>(
    conn: &mut Q,
    entity: i64,
    offset: u32,
) -> Result<String, Box<dyn Error>> {
    let period: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%y%m')")?;
    let period = period.unwrap_or_else(|| "0001".to_string());
    let prefixe = format!("{PREFIX_PROJET}{period}-");
    let posindice = PREFIX_PROJET.len() + 6;

    let sql = format!(
        "SELECT MAX(CAST(SUBSTRING(ref FROM {posindice}) AS SIGNED)) \
         FROM llx_projet WHERE entity = :entity AND ref LIKE :motif"
    );
    let dernier: Option<Option<i64>> = conn.exec_first(
        sql,
        params! {
            "entity" => entity,
            "motif" => format!("{PREFIX_PROJET}____-%"),
        },
    )?;
    let suivant = dernier.flatten().unwrap_or(0) + 1 + i64::from(offset);
    if suivant > 9999 {
        Ok(format!("{prefixe}{suivant}"))
    } else {
        Ok(format!("{prefixe}{suivant:04}"))
    }
}

fn verifier_entree(new: &NewProjet) -> Result<(), Box<dyn Error>> {
    if new.title.trim().is_empty() {
        return Err("le titre du projet est obligatoire".into());
    }
    if matches!(new.fk_soc, Some(id) if id <= 0) {
        return Err("le tiers (fk_soc) est invalide".into());
    }
    Ok(())
}

/// Verifie que le tiers existe bien dans l'entite. `None` est accepte : un
/// chantier interne n'a pas de commanditaire.
fn verifier_tiers(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_soc: Option<i64>,
) -> Result<(), Box<dyn Error>> {
    let Some(id) = fk_soc else {
        return Ok(());
    };
    let tiers: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => id },
    )?;
    if tiers.is_none() {
        return Err(format!("le tiers {id} n'existe pas dans l'entite {entity}").into());
    }
    Ok(())
}

/// Cree un projet et renvoie son `rowid`.
///
/// La numerotation est serialisee par un verrou consultatif MySQL (`GET_LOCK`) :
/// deux processus du noyau ne peuvent pas tirer la meme reference. Aucun
/// changement de schema, Dolibarr n'est pas impacte. Le verrou est relache dans
/// tous les cas, y compris quand l'ecriture echoue.
pub fn create(pool: &Pool, entity: i64, new: &NewProjet) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;

    let mut conn = pool.get_conn()?;
    let verrou = format!("dolibarr_rs_projet_{entity}");
    let pris: Option<i64> = conn.exec_first("SELECT GET_LOCK(?, 10)", (verrou.as_str(),))?;
    if pris != Some(1) {
        return Err("verrou de numerotation indisponible (10 s)".into());
    }

    let resultat = create_sous_verrou(&mut conn, entity, new);
    let _: Option<i64> = conn.exec_first("SELECT RELEASE_LOCK(?)", (verrou.as_str(),))?;
    resultat
}

fn create_sous_verrou(
    conn: &mut mysql::PooledConn,
    entity: i64,
    new: &NewProjet,
) -> Result<i64, Box<dyn Error>> {
    verifier_tiers(conn, entity, new.fk_soc)?;

    let budget = money::round_money(new.budget);

    for attempt in 0..5u32 {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let reference = next_ref(&mut tx, entity, attempt)?;

        let inserted = tx.exec_drop(
            "INSERT INTO llx_projet \
             (entity, ref, fk_soc, title, description, datec, dateo, datee, \
              fk_statut, fk_user_creat, budget_amount) \
             VALUES (:entity, :ref, :fk_soc, :title, :description, NOW(), \
              :date_start, :date_end, :statut, :user_creat, :budget)",
            params! {
                "entity" => entity,
                "ref" => reference.as_str(),
                "fk_soc" => new.fk_soc,
                "title" => new.title.trim(),
                "description" => new.description.as_deref(),
                "date_start" => date_ou_none(new.date_start.as_deref()),
                "date_end" => date_ou_none(new.date_end.as_deref()),
                "statut" => STATUT_OUVERT,
                "user_creat" => FK_USER_CREAT_DEFAUT,
                "budget" => budget.to_string(),
            },
        );

        match inserted {
            Ok(()) => {
                let rowid = tx.last_insert_id().unwrap_or(0) as i64;
                tx.commit()?;
                return Ok(rowid);
            }
            Err(e) if (is_duplicate(&e) || is_deadlock(&e)) && attempt < 4 => {
                tx.rollback()?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une reference de projet unique".into())
}

/// Met a jour un projet existant. **La reference n'est jamais modifiee** : elle
/// est deja imprimee sur les plannings et les situations de travaux.
pub fn update(pool: &Pool, entity: i64, rowid: i64, new: &NewProjet) -> Result<(), Box<dyn Error>> {
    verifier_entree(new)?;
    let mut conn = pool.get_conn()?;

    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_projet WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    if existe.is_none() {
        return Err(format!("projet {rowid} introuvable dans l'entite {entity}").into());
    }
    verifier_tiers(&mut conn, entity, new.fk_soc)?;

    conn.exec_drop(
        "UPDATE llx_projet SET fk_soc = :fk_soc, title = :title, \
          description = :description, dateo = :date_start, datee = :date_end, \
          budget_amount = :budget, fk_user_modif = :user_modif, tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "fk_soc" => new.fk_soc,
            "title" => new.title.trim(),
            "description" => new.description.as_deref(),
            "date_start" => date_ou_none(new.date_start.as_deref()),
            "date_end" => date_ou_none(new.date_end.as_deref()),
            "budget" => money::round_money(new.budget).to_string(),
            "user_modif" => FK_USER_CREAT_DEFAUT,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Taches d'un projet, dans l'ordre de creation.
///
/// `llx_projet_task.entity` existe, mais c'est l'entite du **projet** qui fait
/// foi : la jointure garantit qu'on ne lit pas la tache d'un chantier d'une
/// autre societe meme si sa colonne `entity` a derive.
pub fn taches(pool: &Pool, entity: i64, fk_projet: i64) -> Result<Vec<Tache>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {TASK_COLS} FROM llx_projet_task t \
         INNER JOIN llx_projet p ON p.rowid = t.fk_projet \
         WHERE p.entity = :entity AND t.fk_projet = :projet \
         ORDER BY t.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "projet" => fk_projet })?;
    Ok(rows.into_iter().map(map_tache).collect())
}

/// Ajoute une tache a un projet et renvoie son `rowid`.
///
/// `llx_projet_task.ref` est *nullable* : on insere la reference a `NULL`, puis
/// on la renseigne a `T<rowid>` dans la meme transaction. L'index unique
/// `uk_projet_task_ref (ref, entity)` est ainsi toujours satisfait — un `''` en
/// dur ferait echouer la deuxieme tache du meme projet.
///
/// `fk_user_creat` reste `NULL` : la colonne porte une cle etrangere vers
/// `llx_user`, et le noyau n'a pas d'utilisateur connecte a inscrire. Inventer un
/// identifiant (0) ferait echouer l'insertion en 1452.
pub fn creer_tache(
    pool: &Pool,
    entity: i64,
    fk_projet: i64,
    label: &str,
    date_start: Option<&str>,
    date_end: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let label = label.trim();
    if label.is_empty() {
        return Err("le libelle de la tache est obligatoire".into());
    }

    let mut conn = pool.get_conn()?;
    let projet: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_projet WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_projet },
    )?;
    if projet.is_none() {
        return Err(format!("projet {fk_projet} introuvable dans l'entite {entity}").into());
    }

    for attempt in 0..5u32 {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let inserted = tx.exec_drop(
            "INSERT INTO llx_projet_task \
             (ref, entity, fk_projet, fk_task_parent, datec, dateo, datee, label, \
              progress, fk_statut, fk_user_creat, rang) \
             VALUES (NULL, :entity, :projet, 0, NOW(), :date_start, :date_end, \
              :label, 0, 0, :user_creat, 0)",
            params! {
                "entity" => entity,
                "projet" => fk_projet,
                "date_start" => date_start.map(str::trim).filter(|s| !s.is_empty()),
                "date_end" => date_end.map(str::trim).filter(|s| !s.is_empty()),
                "label" => label,
                "user_creat" => None::<i64>,
            },
        );

        match inserted {
            Ok(()) => {
                let rowid = tx.last_insert_id().unwrap_or(0) as i64;
                tx.exec_drop(
                    "UPDATE llx_projet_task SET ref = CONCAT('T', rowid) \
                     WHERE entity = :entity AND rowid = :rowid",
                    params! { "entity" => entity, "rowid" => rowid },
                )?;
                tx.commit()?;
                return Ok(rowid);
            }
            Err(e) if (is_duplicate(&e) || is_deadlock(&e)) && attempt < 4 => {
                tx.rollback()?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une reference de tache unique".into())
}
