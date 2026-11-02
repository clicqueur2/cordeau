//! Devis (`llx_propal`) et leurs lignes (`llx_propaldet`).
//!
//! La numerotation reprend `mod_propale_marbre` : `PR` + `AAMM` + `-` + numero
//! sur 4 chiffres, le maximum etant cherche sur **toutes** les periodes (le
//! compteur ne repart donc pas a zero au changement de mois).
//!
//! Comme pour les produits, les montants sortent en `CAST(... AS CHAR)` avant
//! `Decimal::from_str` : les colonnes sont des `double(24,8)`, on refuse de les
//! faire transiter par un flottant.

use money::{line_totals, sum, LineInput};
use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;
use std::error::Error;
use std::str::FromStr;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Proposal {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub fk_soc: Option<i64>,
    pub datec: Option<String>,
    pub datep: Option<String>,
    pub fk_statut: i16,
    pub total_ht: Option<Decimal>,
    pub total_tva: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    pub remise_percent: Option<Decimal>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    /// Condition de reglement proposee (`llx_c_payment_term`).
    pub fk_cond_reglement: Option<i64>,
    /// Acompte demande a la commande, en pourcentage du TTC.
    pub deposit_percent: Option<Decimal>,
    /// Date de fin de validite du devis (`fin_validite`).
    pub fin_validite: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProposalLine {
    pub rowid: i64,
    pub fk_propal: i64,
    pub fk_product: Option<i64>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub tva_tx: Option<Decimal>,
    pub qty: Option<Decimal>,
    pub remise_percent: Option<Decimal>,
    pub subprice: Option<Decimal>,
    pub price: Option<Decimal>,
    pub total_ht: Option<Decimal>,
    pub total_tva: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    pub product_type: i16,
    pub rang: i32,
    pub special_code: i32,
    /// Unite de la ligne (`llx_c_units`).
    pub fk_unit: Option<i64>,
}

impl ProposalLine {
    /// Ligne de structure (titre de section), a exclure des totaux et a rendre
    /// comme une bande pleine largeur.
    pub fn is_section(&self) -> bool {
        self.product_type == PRODUCT_TYPE_SECTION
    }
}

#[derive(Debug, Clone, Default)]
pub struct NewProposal {
    pub fk_soc: i64,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    /// Date du devis ; `None` = aujourd'hui.
    pub datep: Option<String>,
    /// Date de fin de validite (facultative).
    pub fin_validite: Option<String>,
    pub fk_cond_reglement: Option<i64>,
    pub deposit_percent: Option<Decimal>,
    pub lines: Vec<NewProposalLine>,
}

impl NewProposal {
    pub fn new(fk_soc: i64) -> Self {
        Self {
            fk_soc,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NewProposalLine {
    pub fk_product: Option<i64>,
    pub label: String,
    pub description: Option<String>,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub tva_tx: Decimal,
    pub remise_percent: Decimal,
    /// Titre de section. Une ligne de structure est ecrite avant la premiere
    /// ligne de chaque section (et seulement a ce moment-la).
    pub section: Option<String>,
}

/// Ligne de structure (titre de lot / section).
///
/// Convention Dolibarr : `core/lib/pdf.lib.php` traite a part les lignes dont
/// `product_type == 9` et dont `special_code` est renseigne. Ces lignes ne
/// comptent pas dans les totaux.
pub const PRODUCT_TYPE_SECTION: i16 = 9;
pub const SPECIAL_CODE_SECTION: i32 = 104777;

impl NewProposalLine {
    pub fn new(label: &str, qty: Decimal, unit_price: Decimal, tva_tx: Decimal) -> Self {
        Self {
            label: label.to_string(),
            qty,
            unit_price,
            tva_tx,
            ..Default::default()
        }
    }

    fn totals(&self) -> money::LineTotals {
        line_totals(&LineInput::new(
            &self.qty.to_string(),
            &self.unit_price.to_string(),
            &self.remise_percent.to_string(),
            &self.tva_tx.to_string(),
        ))
    }
}

const SELECT_COLS: &str = "rowid, entity, ref, fk_soc, \
     DATE_FORMAT(datec, '%Y-%m-%d %H:%i:%s'), DATE_FORMAT(datep, '%Y-%m-%d'), \
     fk_statut, CAST(total_ht AS CHAR), CAST(total_tva AS CHAR), \
     CAST(total_ttc AS CHAR), CAST(remise_percent AS CHAR), note_private, note_public, \
     fk_cond_reglement, CAST(deposit_percent AS CHAR), \
     DATE_FORMAT(fin_validite, '%Y-%m-%d')";

/// `llx_propaldet` n'a pas de colonne `entity` : le filtre passe par la jointure
/// sur `llx_propal`, d'ou les alias.
const LINE_COLS: &str = "d.rowid, d.fk_propal, d.fk_product, d.label, d.description, \
     CAST(d.tva_tx AS CHAR), CAST(d.qty AS CHAR), CAST(d.remise_percent AS CHAR), \
     CAST(d.subprice AS CHAR), CAST(d.price AS CHAR), CAST(d.total_ht AS CHAR), \
     CAST(d.total_tva AS CHAR), CAST(d.total_ttc AS CHAR), d.product_type, d.rang, \
     d.special_code, d.fk_unit";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Proposal {
    Proposal {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        fk_soc: opt_i(&row, 3),
        datec: opt_s(&row, 4),
        datep: opt_s(&row, 5),
        fk_statut: opt_i(&row, 6).unwrap_or(0) as i16,
        total_ht: opt_dec(&row, 7),
        total_tva: opt_dec(&row, 8),
        total_ttc: opt_dec(&row, 9),
        remise_percent: opt_dec(&row, 10),
        note_private: opt_s(&row, 11),
        note_public: opt_s(&row, 12),
        fk_cond_reglement: opt_i(&row, 13),
        deposit_percent: opt_dec(&row, 14),
        fin_validite: opt_s(&row, 15),
    }
}

fn map_line(row: Row) -> ProposalLine {
    ProposalLine {
        rowid: opt_i(&row, 0).unwrap_or(0),
        fk_propal: opt_i(&row, 1).unwrap_or(0),
        fk_product: opt_i(&row, 2),
        label: opt_s(&row, 3),
        description: opt_s(&row, 4),
        tva_tx: opt_dec(&row, 5),
        qty: opt_dec(&row, 6),
        remise_percent: opt_dec(&row, 7),
        subprice: opt_dec(&row, 8),
        price: opt_dec(&row, 9),
        total_ht: opt_dec(&row, 10),
        total_tva: opt_dec(&row, 11),
        total_ttc: opt_dec(&row, 12),
        product_type: opt_i(&row, 13).unwrap_or(0) as i16,
        rang: opt_i(&row, 14).unwrap_or(0) as i32,
        special_code: opt_i(&row, 15).unwrap_or(0) as i32,
        fk_unit: opt_i(&row, 16),
    }
}

pub fn count(pool: &Pool, entity: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_propal WHERE entity = :entity",
        params! { "entity" => entity },
    )?;
    Ok(n.unwrap_or(0))
}

pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Proposal>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_propal WHERE entity = :entity \
         ORDER BY datec DESC, rowid DESC LIMIT :limit OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limit" => limit, "offset" => offset },
    )?;
    Ok(rows.into_iter().map(map_row).collect())
}

pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Proposal>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql =
        format!("SELECT {SELECT_COLS} FROM llx_propal WHERE entity = :entity AND rowid = :rowid");
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.map(map_row))
}

pub fn lines(
    pool: &Pool,
    entity: i64,
    fk_propal: i64,
) -> Result<Vec<ProposalLine>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {LINE_COLS} FROM llx_propaldet d \
         INNER JOIN llx_propal p ON p.rowid = d.fk_propal \
         WHERE p.entity = :entity AND d.fk_propal = :fk_propal \
         ORDER BY d.rang ASC, d.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "fk_propal" => fk_propal },
    )?;
    Ok(rows.into_iter().map(map_line).collect())
}

/// Numero suivant au format `PR` + `AAMM` + `-` + numero sur 4 chiffres.
///
/// Reprise fidele de `mod_propale_marbre::getNextValue` : le maximum est cherche
/// sur toutes les periodes (`LIKE 'PR____-%'`, sans filtre sur l'annee ni le
/// mois), donc le compteur continue d'augmenter d'un mois sur l'autre.
/// `$posindice = strlen("PR") + 6 = 8` : le premier chiffre est juste apres le
/// tiret (`PR` = 2, `AAMM` = 4, `-` = 1).
///
/// L'`offset` sert au reessai en cas de collision sur `uk_propal_ref`.
fn next_ref<Q: Queryable>(
    conn: &mut Q,
    entity: i64,
    offset: u32,
) -> Result<String, Box<dyn Error>> {
    let yymm: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%y%m')")?;
    let yymm = yymm.unwrap_or_else(|| "0001".to_string());

    let max: Option<Option<i64>> = conn.exec_first(
        "SELECT MAX(CAST(SUBSTRING(ref, 8) AS SIGNED)) FROM llx_propal \
         WHERE entity = :entity AND ref LIKE 'PR____-%'",
        params! { "entity" => entity },
    )?;
    let max = max.flatten().unwrap_or(0);

    let suivant = max + 1 + i64::from(offset);
    // Au-dela de 9999, `sprintf("%04d")` n'ajoute plus rien : le nombre reste tel quel.
    let num = if max >= 9999 {
        suivant.to_string()
    } else {
        format!("{suivant:04}")
    };
    Ok(format!("PR{yymm}-{num}"))
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

/// Interblocage ou attente de verrou depassee : l'ecriture doit etre rejouee.
fn is_deadlock(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1213 || me.code == 1205)
}

/// Ligne prete a etre ecrite, totaux deja calcules par `money`.
struct Ligne {
    fk_product: Option<i64>,
    label: String,
    description: Option<String>,
    tva_tx: Decimal,
    qty: Decimal,
    remise_percent: Decimal,
    unit_price_ht: Decimal,
    total_ht: Decimal,
    total_tva: Decimal,
    total_ttc: Decimal,
    product_type: i16,
    special_code: i32,
}

fn verifier_entree(new: &NewProposal) -> Result<(), Box<dyn Error>> {
    if new.fk_soc <= 0 {
        return Err("le tiers (fk_soc) est obligatoire".into());
    }
    if new.lines.is_empty() {
        return Err("un devis doit comporter au moins une ligne".into());
    }
    for (i, l) in new.lines.iter().enumerate() {
        if l.qty <= Decimal::ZERO {
            return Err(format!(
                "ligne {} : la quantite doit etre strictement positive",
                i + 1
            )
            .into());
        }
    }
    Ok(())
}

/// Transforme les lignes demandees en lignes a ecrire, en inserant un titre de
/// section a chaque changement de section.
fn preparer_lignes(
    conn: &mut mysql::PooledConn,
    entity: i64,
    new: &NewProposal,
) -> Result<Vec<Ligne>, Box<dyn Error>> {
    let mut lignes: Vec<Ligne> = Vec::with_capacity(new.lines.len());
    let mut section_courante: Option<String> = None;

    for l in &new.lines {
        if let Some(titre) = l.section.as_deref() {
            if section_courante.as_deref() != Some(titre) {
                lignes.push(Ligne {
                    fk_product: None,
                    label: titre.to_string(),
                    description: None,
                    tva_tx: Decimal::ZERO,
                    qty: Decimal::ZERO,
                    remise_percent: Decimal::ZERO,
                    unit_price_ht: Decimal::ZERO,
                    total_ht: Decimal::ZERO,
                    total_tva: Decimal::ZERO,
                    total_ttc: Decimal::ZERO,
                    product_type: PRODUCT_TYPE_SECTION,
                    special_code: SPECIAL_CODE_SECTION,
                });
                section_courante = Some(titre.to_string());
            }
        } else {
            section_courante = None;
        }

        let t = l.totals();
        let product_type = match l.fk_product {
            None => 0i16,
            Some(id) => {
                let pt: Option<i64> = conn.exec_first(
                    "SELECT fk_product_type FROM llx_product \
                     WHERE entity = :entity AND rowid = :rowid",
                    params! { "entity" => entity, "rowid" => id },
                )?;
                match pt {
                    Some(v) => v as i16,
                    None => {
                        return Err(format!(
                            "le produit {} n'existe pas dans l'entite {}",
                            id, entity
                        )
                        .into())
                    }
                }
            }
        };
        lignes.push(Ligne {
            fk_product: l.fk_product,
            label: l.label.clone(),
            description: l.description.clone(),
            tva_tx: l.tva_tx,
            qty: l.qty,
            remise_percent: l.remise_percent,
            unit_price_ht: t.unit_price_ht,
            total_ht: t.total_ht,
            total_tva: t.total_vat,
            total_ttc: t.total_ttc,
            product_type,
            special_code: 0,
        });
    }
    Ok(lignes)
}

/// Totaux d'en-tete : somme des lignes utiles, arrondie une seule fois.
/// Les titres de section ne comptent pas.
fn totaux_lignes(lignes: &[Ligne]) -> (Decimal, Decimal, Decimal) {
    let utiles = |l: &&Ligne| l.product_type != PRODUCT_TYPE_SECTION;
    let hts: Vec<Decimal> = lignes.iter().filter(utiles).map(|l| l.total_ht).collect();
    let tvas: Vec<Decimal> = lignes.iter().filter(utiles).map(|l| l.total_tva).collect();
    let ttcs: Vec<Decimal> = lignes.iter().filter(utiles).map(|l| l.total_ttc).collect();
    (sum(&hts), sum(&tvas), sum(&ttcs))
}

fn inserer_lignes(
    tx: &mut mysql::Transaction<'_>,
    fk_propal: i64,
    lignes: &[Ligne],
) -> Result<(), Box<dyn Error>> {
    for (i, l) in lignes.iter().enumerate() {
        tx.exec_drop(
            "INSERT INTO llx_propaldet \
             (fk_propal, fk_product, label, description, tva_tx, qty, \
              remise_percent, subprice, price, total_ht, total_tva, \
              total_ttc, product_type, rang, special_code) \
             VALUES (:fk_propal, :fk_product, :label, :description, :tva_tx, \
              :qty, :remise_percent, :subprice, :subprice, :total_ht, \
              :total_tva, :total_ttc, :product_type, :rang, :special_code)",
            params! {
                "fk_propal" => fk_propal,
                "fk_product" => l.fk_product,
                "label" => l.label.as_str(),
                "description" => l.description.as_deref(),
                "tva_tx" => l.tva_tx.to_string(),
                "qty" => l.qty.to_string(),
                "remise_percent" => l.remise_percent.to_string(),
                "subprice" => l.unit_price_ht.to_string(),
                "total_ht" => l.total_ht.to_string(),
                "total_tva" => l.total_tva.to_string(),
                "total_ttc" => l.total_ttc.to_string(),
                "product_type" => l.product_type,
                "rang" => (i + 1) as i32,
                "special_code" => l.special_code,
            },
        )?;
    }
    Ok(())
}

pub fn create(pool: &Pool, entity: i64, new: &NewProposal) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;

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

    let lignes = preparer_lignes(&mut conn, entity, new)?;
    let (total_ht, total_tva, total_ttc) = totaux_lignes(&lignes);

    for attempt in 0..5u32 {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let reference = next_ref(&mut tx, entity, attempt)?;

        let inserted = tx.exec_drop(
            "INSERT INTO llx_propal \
             (entity, ref, fk_soc, datec, datep, fk_statut, total_ht, total_tva, \
              total_ttc, remise_percent, note_private, note_public, \
              fk_cond_reglement, deposit_percent, fin_validite) \
             VALUES (:entity, :ref, :fk_soc, NOW(), COALESCE(:datep, CURDATE()), 0, \
              :total_ht, :total_tva, :total_ttc, 0, :note_private, :note_public, \
              :cond, :deposit, :fin)",
            params! {
                "entity" => entity,
                "ref" => reference.as_str(),
                "fk_soc" => new.fk_soc,
                "datep" => new.datep.as_deref(),
                "total_ht" => total_ht.to_string(),
                "total_tva" => total_tva.to_string(),
                "total_ttc" => total_ttc.to_string(),
                "note_private" => new.note_private.as_deref(),
                "note_public" => new.note_public.as_deref(),
                "cond" => new.fk_cond_reglement,
                "deposit" => new.deposit_percent.map(|d| d.to_string()),
                "fin" => new.fin_validite.as_deref(),
            },
        );

        match inserted {
            Ok(()) => {
                let rowid = tx.last_insert_id().unwrap_or(0) as i64;
                inserer_lignes(&mut tx, rowid, &lignes)?;
                tx.commit()?;
                return Ok(rowid);
            }
            Err(e) if (is_duplicate(&e) || is_deadlock(&e)) && attempt < 4 => {
                tx.rollback()?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une reference de devis unique".into())
}

/// Remplace le contenu d'un devis **brouillon**. La reference est conservee.
pub fn update(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    new: &NewProposal,
) -> Result<(), Box<dyn Error>> {
    verifier_entree(new)?;
    let mut conn = pool.get_conn()?;

    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_propal WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => return Err(format!("devis {rowid} introuvable").into()),
        Some(s) if s != 0 => return Err("seul un devis en brouillon peut etre modifie".into()),
        _ => {}
    }

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

    let lignes = preparer_lignes(&mut conn, entity, new)?;
    let (total_ht, total_tva, total_ttc) = totaux_lignes(&lignes);

    let mut tx = conn.start_transaction(TxOpts::default())?;
    tx.exec_drop(
        "UPDATE llx_propal SET fk_soc = :fk_soc, \
          datep = COALESCE(:datep, datep), total_ht = :total_ht, \
          total_tva = :total_tva, total_ttc = :total_ttc, \
          note_private = :note_private, note_public = :note_public, \
          fk_cond_reglement = :cond, deposit_percent = :deposit, \
          fin_validite = COALESCE(:fin, fin_validite), tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "fk_soc" => new.fk_soc,
            "datep" => new.datep.as_deref(),
            "total_ht" => total_ht.to_string(),
            "total_tva" => total_tva.to_string(),
            "total_ttc" => total_ttc.to_string(),
            "note_private" => new.note_private.as_deref(),
            "note_public" => new.note_public.as_deref(),
            "cond" => new.fk_cond_reglement,
            "deposit" => new.deposit_percent.map(|d| d.to_string()),
            "fin" => new.fin_validite.as_deref(),
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_propaldet WHERE fk_propal = :rowid",
        params! { "rowid" => rowid },
    )?;
    inserer_lignes(&mut tx, rowid, &lignes)?;
    tx.commit()?;
    Ok(())
}

/// Passe un devis de brouillon (0) a valide (1).
pub fn validate(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_propal WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => return Err(format!("devis {rowid} introuvable").into()),
        Some(s) if s != 0 => return Err("ce devis n'est pas en brouillon".into()),
        _ => {}
    }
    let lignes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_propaldet WHERE fk_propal = :rowid",
        params! { "rowid" => rowid },
    )?;
    if lignes.unwrap_or(0) == 0 {
        return Err("un devis sans ligne ne peut pas etre valide".into());
    }
    conn.exec_drop(
        "UPDATE llx_propal SET fk_statut = 1, date_valid = NOW(), tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}
pub fn delete(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_propal WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    if existe.is_none() {
        return Ok(());
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    tx.exec_drop(
        "DELETE FROM llx_propaldet WHERE fk_propal = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_propal WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}
