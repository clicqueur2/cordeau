//! Contrats (`llx_contrat` + `llx_contratdet`).
//!
//! Le contrat etait le seul module Dolibarr absent du noyau : 0 action sur les
//! 45 de `htdocs/contrat/card.php` (cf. `EXTRACTIONS/contrat.md`). Ce module ne
//! porte que la **lecture** et le **cycle de vie** ; il n'y a volontairement ni
//! facturation, ni echeancier, ni PDF.
//!
//! ## Statuts
//!
//! Contrat : la colonne reelle est **`llx_contrat.statut`** (`smallint(6)`,
//! defaut `0`) et non `fk_statut` comme sur les devis, commandes et factures.
//! Trois valeurs : `0` brouillon, `1` valide, `2` clos. La reference d'un
//! brouillon est `(PROV<rowid>)`, comme Dolibarr ; [`valider`] la renumerote en
//! `CT` + AAAMM + `-` + 4 chiffres (meme motif que `PR`/`CO` du noyau).
//!
//! Ligne (`llx_contratdet.statut`) : `0` en attente, `4` active (en service),
//! `5` close. Dolibarr n'emploie que ces trois valeurs pour les services d'un
//! contrat — pas de `1`/`2`/`3` (cf. `card.php`, `ContratLigne::STATUS_CLOSED`).
//!
//! ## Total du contrat : lignes ACTIVES uniquement
//!
//! **`total_ht`, `total_tva` et `total_ttc` sont la somme des lignes actives
//! (statut `4`) et d'elles seules.** Une ligne en attente (`0`) ou close (`5`)
//! ne compte pas. C'est un ecart assume avec Dolibarr, dont `update_price()`
//! additionne toutes les lignes quel que soit leur statut : ici le total dit ce
//! qui est facturable maintenant, et il baisse quand un service est desactive.
//! Il est recalcule a chaque ecriture qui touche une ligne ([`add_line`],
//! [`activer_ligne`], [`desactiver_ligne`]) et a la validation.
//!
//! ## Choix de cloture
//!
//! Dans Dolibarr, `card.php` ne pose **jamais** `statut = 2` : l'action `close`
//! appelle `closeAll()`, qui ferme les *services*. Ici [`clore`] fait l'inverse
//! et de facon explicite : le contrat passe a `2` et porte sa `date_cloture`,
//! les lignes ne sont pas touchees (c'est [`desactiver_ligne`] qui les ferme).
//! Sans cela, clore un contrat remettrait son total a zero et effacerait la
//! valeur du contrat.

use std::error::Error;
use std::str::FromStr;

use money::{line_totals, round_money, LineInput};
use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

pub const PREFIXE: &str = "CT";
pub const PREVISIONNEL: &str = "PROV";

/// Contrat brouillon : reference `(PROV<rowid>)`, non facturable.
pub const STATUT_BROUILLON: i16 = 0;
/// Contrat valide : reference definitive.
pub const STATUT_VALIDE: i16 = 1;
/// Contrat clos.
pub const STATUT_CLOS: i16 = 2;

/// Service en attente (valeur posee a la creation de la ligne).
pub const LIGNE_ATTENTE: i16 = 0;
/// Service actif, en service : seule valeur comptee dans le total.
pub const LIGNE_ACTIVE: i16 = 4;
/// Service clos.
pub const LIGNE_CLOSE: i16 = 5;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contrat {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub fk_soc: i64,
    pub datec: Option<String>,
    pub date_contrat: Option<String>,
    pub fin_validite: Option<String>,
    /// Colonne `statut` (et non `fk_statut`).
    pub statut: i16,
    pub total_ht: Option<Decimal>,
    pub total_tva: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub ref_customer: Option<String>,
}

impl Contrat {
    pub fn est_brouillon(&self) -> bool {
        self.statut == STATUT_BROUILLON
    }

    pub fn est_clos(&self) -> bool {
        self.statut == STATUT_CLOS
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContratLigne {
    pub rowid: i64,
    pub fk_contrat: i64,
    pub fk_product: Option<i64>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub tva_tx: Option<Decimal>,
    pub qty: Option<Decimal>,
    pub subprice: Option<Decimal>,
    pub total_ht: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    pub statut: i16,
    pub date_ouverture_prevue: Option<String>,
    pub date_ouverture: Option<String>,
    pub date_fin_validite: Option<String>,
}

impl ContratLigne {
    /// Un service est actif quand son statut vaut `4` ; une ligne active dont la
    /// `date_fin_validite` est passee est « expiree » a l'affichage, mais garde
    /// le statut `4` (Dolibarr ne la modifie pas : c'est un drapeau calcule).
    pub fn est_active(&self) -> bool {
        self.statut == LIGNE_ACTIVE
    }
}

/// Contrat a creer. Les totaux partent a zero : ils suivent les lignes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewContrat {
    pub fk_soc: i64,
    pub date_contrat: Option<String>,
    pub fin_validite: Option<String>,
    pub ref_customer: Option<String>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    /// Auteur. Laisser vide pour prendre le premier utilisateur de l'entite :
    /// `llx_contrat.fk_user_author` porte une **cle etrangere reelle** vers
    /// `llx_user`, donc la valeur `0` de Dolibarr est refusee par le schema.
    pub fk_user_author: Option<i64>,
}

impl NewContrat {
    pub fn new(fk_soc: i64) -> Self {
        Self {
            fk_soc,
            ..Self::default()
        }
    }
}

/// Service a ajouter. Il nait en attente (statut `0`) : c'est
/// [`activer_ligne`] qui le met en service.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewContratLine {
    pub fk_product: Option<i64>,
    pub label: String,
    pub description: Option<String>,
    pub qty: Decimal,
    pub subprice: Decimal,
    pub remise_percent: Decimal,
    pub tva_tx: Decimal,
    pub date_ouverture_prevue: Option<String>,
    pub date_fin_validite: Option<String>,
}

impl NewContratLine {
    pub fn new(label: &str, qty: Decimal, subprice: Decimal, tva_tx: Decimal) -> Self {
        Self {
            label: label.to_string(),
            qty,
            subprice,
            tva_tx,
            ..Self::default()
        }
    }
}

const SELECT_COLS: &str = "rowid, entity, ref, fk_soc, \
     DATE_FORMAT(datec, '%Y-%m-%d'), DATE_FORMAT(date_contrat, '%Y-%m-%d'), \
     DATE_FORMAT(fin_validite, '%Y-%m-%d'), statut, \
     CAST(total_ht AS CHAR), CAST(total_tva AS CHAR), CAST(total_ttc AS CHAR), \
     note_private, note_public, ref_customer";

const LINE_COLS: &str = "d.rowid, d.fk_contrat, d.fk_product, d.label, d.description, \
     CAST(d.tva_tx AS CHAR), CAST(d.qty AS CHAR), CAST(d.subprice AS CHAR), \
     CAST(d.total_ht AS CHAR), CAST(d.total_ttc AS CHAR), d.statut, \
     DATE_FORMAT(d.date_ouverture_prevue, '%Y-%m-%d'), \
     DATE_FORMAT(d.date_ouverture, '%Y-%m-%d'), \
     DATE_FORMAT(d.date_fin_validite, '%Y-%m-%d')";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Contrat {
    Contrat {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        fk_soc: opt_i(&row, 3).unwrap_or(0),
        datec: opt_s(&row, 4),
        date_contrat: opt_s(&row, 5),
        fin_validite: opt_s(&row, 6),
        statut: opt_i(&row, 7).unwrap_or(0) as i16,
        total_ht: opt_dec(&row, 8),
        total_tva: opt_dec(&row, 9),
        total_ttc: opt_dec(&row, 10),
        note_private: opt_s(&row, 11),
        note_public: opt_s(&row, 12),
        ref_customer: opt_s(&row, 13),
    }
}

fn map_line(row: Row) -> ContratLigne {
    ContratLigne {
        rowid: opt_i(&row, 0).unwrap_or(0),
        fk_contrat: opt_i(&row, 1).unwrap_or(0),
        fk_product: opt_i(&row, 2),
        label: opt_s(&row, 3),
        description: opt_s(&row, 4),
        tva_tx: opt_dec(&row, 5),
        qty: opt_dec(&row, 6),
        subprice: opt_dec(&row, 7),
        total_ht: opt_dec(&row, 8),
        total_ttc: opt_dec(&row, 9),
        statut: opt_i(&row, 10).unwrap_or(0) as i16,
        date_ouverture_prevue: opt_s(&row, 11),
        date_ouverture: opt_s(&row, 12),
        date_fin_validite: opt_s(&row, 13),
    }
}

/// Nombre de contrats de l'entite (toutes agences confondues : `entity` filtre).
pub fn compte(pool: &Pool, entity: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_contrat WHERE entity = :entity",
        params! { "entity" => entity },
    )?;
    Ok(n.unwrap_or(0))
}

pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Contrat>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_contrat WHERE entity = ? \
         ORDER BY date_contrat DESC, rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limit, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Contrat>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} FROM llx_contrat WHERE entity = ? AND rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

pub fn lines(
    pool: &Pool,
    entity: i64,
    fk_contrat: i64,
) -> Result<Vec<ContratLigne>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {LINE_COLS} FROM llx_contratdet d \
         INNER JOIN llx_contrat c ON c.rowid = d.fk_contrat \
         WHERE c.entity = :entity AND d.fk_contrat = :fk_contrat \
         ORDER BY d.rang ASC, d.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "fk_contrat" => fk_contrat },
    )?;
    Ok(rows.into_iter().map(map_line).collect())
}

/// Cree un contrat en brouillon, sans ligne.
///
/// La reference est NULL a l'insertion (l'index unique `uk_contrat_ref` accepte
/// plusieurs NULL), puis renseignee `(PROV<rowid>)` : c'est exactement la forme
/// que Dolibarr donne a un contrat non valide.
pub fn create(pool: &Pool, entity: i64, new: &NewContrat) -> Result<i64, Box<dyn Error>> {
    if new.fk_soc <= 0 {
        return Err("le tiers (fk_soc) est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    let tiers: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => new.fk_soc },
    )?;
    if tiers.is_none() {
        return Err(format!(
            "le tiers {} n'existe pas dans l'entite {}",
            new.fk_soc, entity
        )
        .into());
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    let auteur = match new.fk_user_author {
        Some(u) => u,
        None => resoudre_auteur(&mut tx, entity)?,
    };
    tx.exec_drop(
        "INSERT INTO llx_contrat \
         (entity, ref, fk_soc, datec, date_contrat, statut, \
          total_ht, total_tva, total_ttc, note_private, note_public, \
          ref_customer, fin_validite, fk_user_author) \
         VALUES (:entity, NULL, :fk_soc, NOW(), COALESCE(:date_contrat, CURDATE()), :statut, \
          0, 0, 0, :note_private, :note_public, :ref_customer, :fin, :auteur)",
        params! {
            "entity" => entity,
            "fk_soc" => new.fk_soc,
            "date_contrat" => new.date_contrat.as_deref(),
            "statut" => STATUT_BROUILLON,
            "note_private" => new.note_private.as_deref(),
            "note_public" => new.note_public.as_deref(),
            "ref_customer" => new.ref_customer.as_deref(),
            "fin" => new.fin_validite.as_deref(),
            "auteur" => auteur,
        },
    )?;
    let rowid = tx
        .last_insert_id()
        .ok_or("identifiant du contrat insere indisponible")? as i64;
    tx.exec_drop(
        "UPDATE llx_contrat SET ref = :ref WHERE entity = :entity AND rowid = :rowid",
        params! {
            "ref" => format!("({PREVISIONNEL}{rowid})"),
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    tx.commit()?;
    Ok(rowid)
}

/// Ajoute un service. Le contrat doit exister, ne pas etre clos, et la date de
/// debut prevue ne peut pas depasser la date de fin prevue (seul controle pose
/// par Dolibarr a l'ajout d'une ligne).
pub fn add_line(
    pool: &Pool,
    entity: i64,
    fk_contrat: i64,
    new: &NewContratLine,
) -> Result<i64, Box<dyn Error>> {
    if new.label.trim().is_empty() {
        return Err("le libelle de la ligne est obligatoire".into());
    }
    if new.qty <= Decimal::ZERO {
        return Err("la quantite doit etre strictement positive".into());
    }
    if let (Some(debut), Some(fin)) = (
        new.date_ouverture_prevue.as_deref(),
        new.date_fin_validite.as_deref(),
    ) {
        if debut > fin {
            return Err("la date de debut prevue depasse la date de fin".into());
        }
    }

    let mut conn = pool.get_conn()?;
    let contrat: Option<Row> = conn.exec_first(
        "SELECT statut, fk_user_author FROM llx_contrat WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_contrat },
    )?;
    let Some(contrat) = contrat else {
        return Err(format!("contrat {fk_contrat} introuvable").into());
    };
    if opt_i(&contrat, 0).unwrap_or(0) as i16 == STATUT_CLOS {
        return Err("un contrat clos ne recoit plus de ligne".into());
    }
    let auteur = opt_i(&contrat, 1).unwrap_or(0);

    let totaux = line_totals(&LineInput::new(
        &new.qty.to_string(),
        &new.subprice.to_string(),
        &new.remise_percent.to_string(),
        &new.tva_tx.to_string(),
    ));
    let rang: Option<i64> = conn.exec_first(
        "SELECT COALESCE(MAX(rang), 0) + 1 FROM llx_contratdet WHERE fk_contrat = :rowid",
        params! { "rowid" => fk_contrat },
    )?;

    let mut tx = conn.start_transaction(TxOpts::default())?;
    tx.exec_drop(
        "INSERT INTO llx_contratdet \
         (fk_contrat, fk_product, label, description, tva_tx, qty, remise_percent, \
          subprice, total_ht, total_tva, total_ttc, statut, rang, \
          date_ouverture_prevue, date_fin_validite, fk_user_author) \
         VALUES (:fk_contrat, :fk_product, :label, :description, :tva_tx, :qty, \
          :remise_percent, :subprice, :total_ht, :total_tva, :total_ttc, :statut, \
          :rang, :debut, :fin, :auteur)",
        params! {
            "fk_contrat" => fk_contrat,
            "fk_product" => new.fk_product,
            "label" => new.label.trim(),
            "description" => new.description.as_deref(),
            "tva_tx" => new.tva_tx.to_string(),
            "qty" => new.qty.to_string(),
            "remise_percent" => new.remise_percent.to_string(),
            "subprice" => new.subprice.to_string(),
            "total_ht" => totaux.total_ht.to_string(),
            "total_tva" => totaux.total_vat.to_string(),
            "total_ttc" => totaux.total_ttc.to_string(),
            "statut" => LIGNE_ATTENTE,
            "rang" => rang.unwrap_or(1) as i32,
            "debut" => new.date_ouverture_prevue.as_deref(),
            "fin" => new.date_fin_validite.as_deref(),
            "auteur" => auteur,
        },
    )?;
    let ligne_id = tx
        .last_insert_id()
        .ok_or("identifiant de la ligne inseree indisponible")? as i64;
    recalculer_total(&mut tx, entity, fk_contrat)?;
    tx.commit()?;
    Ok(ligne_id)
}

/// Passe le contrat de brouillon a valide (`0 -> 1`) et renumerote la reference.
///
/// Refus si le contrat est introuvable, deja valide, ou s'il n'a **aucune
/// ligne** : un contrat vide ne veut rien dire.
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT statut FROM llx_contrat WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => return Err(format!("contrat {rowid} introuvable").into()),
        Some(s) if s as i16 != STATUT_BROUILLON => {
            return Err("ce contrat n'est pas en brouillon".into())
        }
        _ => {}
    }
    let lignes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_contratdet WHERE fk_contrat = :rowid",
        params! { "rowid" => rowid },
    )?;
    if lignes.unwrap_or(0) == 0 {
        return Err("un contrat sans ligne ne se valide pas".into());
    }

    for attempt in 0..5u32 {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let reference = next_ref(&mut tx, entity, attempt)?;
        let maj = tx.exec_drop(
            "UPDATE llx_contrat SET ref = :ref, statut = :statut, tms = NOW() \
             WHERE entity = :entity AND rowid = :rowid",
            params! {
                "ref" => reference.as_str(),
                "statut" => STATUT_VALIDE,
                "entity" => entity,
                "rowid" => rowid,
            },
        );
        match maj {
            Ok(()) => {
                recalculer_total(&mut tx, entity, rowid)?;
                tx.commit()?;
                return Ok(());
            }
            Err(e) if (is_duplicate(&e) || is_deadlock(&e)) && attempt < 4 => {
                tx.rollback()?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une reference de contrat unique".into())
}

/// Clot un contrat valide (`1 -> 2`) et date la cloture.
///
/// Les services ne sont pas touches : [`desactiver_ligne`] reste le geste
/// explicite, sinon le total du contrat tomberait a zero en le cloturant.
pub fn clore(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT statut FROM llx_contrat WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => return Err(format!("contrat {rowid} introuvable").into()),
        Some(s) if s as i16 != STATUT_VALIDE => {
            return Err("seul un contrat valide peut etre clos".into())
        }
        _ => {}
    }
    conn.exec_drop(
        "UPDATE llx_contrat SET statut = :statut, date_cloture = NOW(), tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_CLOS, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Met un service en service (`4`) : `date_ouverture` est posee si elle etait
/// vide, et une eventuelle `date_cloture` est effacee (une ligne rouverte).
pub fn activer_ligne(pool: &Pool, entity: i64, ligne_id: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let (fk_contrat, statut_contrat, statut_ligne) = contexte_ligne(&mut conn, entity, ligne_id)?;
    if statut_contrat != STATUT_VALIDE {
        return Err("les services d'un contrat non valide ne s'activent pas".into());
    }
    if statut_ligne == LIGNE_ACTIVE {
        return Err("ce service est deja actif".into());
    }
    conn.exec_drop(
        "UPDATE llx_contratdet \
         SET statut = :statut, date_ouverture = COALESCE(date_ouverture, NOW()), \
             date_cloture = NULL, tms = NOW() \
         WHERE rowid = :rowid",
        params! { "statut" => LIGNE_ACTIVE, "rowid" => ligne_id },
    )?;
    recalculer_total(&mut conn, entity, fk_contrat)?;
    Ok(())
}

/// Ferme un service (`5`) : il sort du total du contrat.
pub fn desactiver_ligne(pool: &Pool, entity: i64, ligne_id: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let (fk_contrat, statut_contrat, statut_ligne) = contexte_ligne(&mut conn, entity, ligne_id)?;
    if statut_contrat < STATUT_VALIDE {
        return Err("un contrat en brouillon n'a pas de service a fermer".into());
    }
    if statut_ligne != LIGNE_ACTIVE {
        return Err("ce service n'est pas actif".into());
    }
    conn.exec_drop(
        "UPDATE llx_contratdet \
         SET statut = :statut, date_cloture = NOW(), tms = NOW() \
         WHERE rowid = :rowid",
        params! { "statut" => LIGNE_CLOSE, "rowid" => ligne_id },
    )?;
    recalculer_total(&mut conn, entity, fk_contrat)?;
    Ok(())
}

/// Auteur des ecritures. `llx_contrat.fk_user_author` porte une **cle etrangere
/// reelle** vers `llx_user` (contrairement a la plupart des tables du noyau), si
/// bien que la valeur `0` employee par Dolibarr est refusee par MySQL 1452.
/// On prend donc le premier utilisateur de l'entite, a defaut n'importe lequel
/// (la contrainte exige seulement que la ligne existe).
fn resoudre_auteur<Q>(conn: &mut Q, entity: i64) -> Result<i64, Box<dyn Error>>
where
    Q: Queryable,
{
    let local: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity ORDER BY rowid ASC LIMIT 1",
        params! { "entity" => entity },
    )?;
    if let Some(u) = local {
        return Ok(u);
    }
    let quiconque: Option<i64> =
        conn.query_first("SELECT rowid FROM llx_user ORDER BY rowid ASC")?;
    quiconque.ok_or_else(|| "aucun utilisateur en base pour fk_user_author".into())
}

/// Retrouve le contrat d'une ligne, en verifiant l'entite : le cloisonnement
/// passe par `llx_contrat.entity`, une ligne n'ayant pas d'entite propre.
fn contexte_ligne<Q>(
    conn: &mut Q,
    entity: i64,
    ligne_id: i64,
) -> Result<(i64, i16, i16), Box<dyn Error>>
where
    Q: Queryable,
{
    let ctx: Option<Row> = conn.exec_first(
        "SELECT d.fk_contrat, c.statut, d.statut FROM llx_contratdet d \
         INNER JOIN llx_contrat c ON c.rowid = d.fk_contrat \
         WHERE c.entity = :entity AND d.rowid = :ligne",
        params! { "entity" => entity, "ligne" => ligne_id },
    )?;
    let row = ctx.ok_or_else(|| format!("ligne de contrat {ligne_id} introuvable"))?;
    Ok((
        opt_i(&row, 0).unwrap_or(0),
        opt_i(&row, 1).unwrap_or(0) as i16,
        opt_i(&row, 2).unwrap_or(0) as i16,
    ))
}

/// Recalcule `total_ht`/`total_tva`/`total_ttc` du contrat : **somme des lignes
/// actives seulement** (statut `4`). Appelee dans la transaction de l'ecriture
/// qui vient de changer une ligne.
fn recalculer_total<Q>(conn: &mut Q, entity: i64, fk_contrat: i64) -> Result<(), Box<dyn Error>>
where
    Q: Queryable,
{
    let somme: Option<(String, String, String)> = conn.exec_first(
        "SELECT COALESCE(CAST(SUM(total_ht) AS CHAR), '0'), \
                COALESCE(CAST(SUM(total_tva) AS CHAR), '0'), \
                COALESCE(CAST(SUM(total_ttc) AS CHAR), '0') \
         FROM llx_contratdet WHERE fk_contrat = :rowid AND statut = :actif",
        params! { "rowid" => fk_contrat, "actif" => LIGNE_ACTIVE },
    )?;
    let (ht, tva, ttc) = somme.unwrap_or_else(|| ("0".into(), "0".into(), "0".into()));
    let ht = round_money(Decimal::from_str(ht.trim()).unwrap_or_default());
    let tva = round_money(Decimal::from_str(tva.trim()).unwrap_or_default());
    let ttc = round_money(Decimal::from_str(ttc.trim()).unwrap_or_default());
    conn.exec_drop(
        "UPDATE llx_contrat SET total_ht = :ht, total_tva = :tva, total_ttc = :ttc, \
         tms = NOW() WHERE entity = :entity AND rowid = :rowid",
        params! {
            "ht" => ht.to_string(),
            "tva" => tva.to_string(),
            "ttc" => ttc.to_string(),
            "entity" => entity,
            "rowid" => fk_contrat,
        },
    )?;
    Ok(())
}

/// Numero suivant au format `CT` + `AAMM` + `-` + numero sur 4 chiffres, meme
/// motif que `PR`/`CO`. Le maximum est cherche sur toutes les periodes
/// (`LIKE 'CT____-%'`), donc le compteur ne repart pas a 1 chaque mois.
///
/// `offset` sert au reessai en cas de collision sur `uk_contrat_ref`.
fn next_ref<Q>(conn: &mut Q, entity: i64, offset: u32) -> Result<String, Box<dyn Error>>
where
    Q: Queryable,
{
    let yymm: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%y%m')")?;
    let yymm = yymm.unwrap_or_else(|| "0001".to_string());
    let max: Option<Option<i64>> = conn.exec_first(
        "SELECT MAX(CAST(SUBSTRING(ref, 8) AS SIGNED)) FROM llx_contrat \
         WHERE entity = :entity AND ref LIKE 'CT____-%'",
        params! { "entity" => entity },
    )?;
    let max = max.flatten().unwrap_or(0);
    let suivant = max + 1 + i64::from(offset);
    let num = if max >= 9999 {
        suivant.to_string()
    } else {
        format!("{suivant:04}")
    };
    Ok(format!("{PREFIXE}{yymm}-{num}"))
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

fn is_deadlock(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1213 || s.code == 1205)
}
