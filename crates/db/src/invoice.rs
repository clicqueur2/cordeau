//! Factures (`llx_facture` + `llx_facturedet`).
//!
//! Meme discipline que pour les devis : colonnes explicites, filtre `entity`,
//! montants lus via `CAST(... AS CHAR)`, numerotation dans la transaction.
//!
//! Numerotation reprise de `mod_facture_terre` : `FA` + `AAAMM` + `-` + 4 chiffres,
//! sans remise a zero mensuelle. Les avoirs utilisent `AV`, les acomptes `AC`.

use std::error::Error;
use std::str::FromStr;

use money::{line_totals, sum, LineInput};
use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

pub const PREFIX_INVOICE: &str = "FA";
pub const PREFIX_CREDIT_NOTE: &str = "AV";
pub const PREFIX_DEPOSIT: &str = "AC";

/// Ligne de structure (titre de section), meme convention que pour les devis.
pub const PRODUCT_TYPE_SECTION: i16 = 9;
pub const SPECIAL_CODE_SECTION: i32 = 104777;

/// `llx_facture.type` : 0 facture, 2 avoir, 3 acompte, 5 situation (Dolibarr).
pub const TYPE_FACTURE: i16 = 0;
pub const TYPE_AVOIR: i16 = 2;
pub const TYPE_ACOMPTE: i16 = 3;
pub const TYPE_SITUATION: i16 = 5;
pub const STATUT_BROUILLON: i16 = 0;
pub const STATUT_VALIDEE: i16 = 1;
pub const STATUT_PAYEE: i16 = 2;
pub const STATUT_ABANDONNEE: i16 = 3;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Invoice {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub fk_soc: Option<i64>,
    pub type_: i16,
    pub datec: Option<String>,
    pub datef: Option<String>,
    pub date_lim_reglement: Option<String>,
    pub fk_statut: i16,
    pub paye: i16,
    pub total_ht: Option<Decimal>,
    pub total_tva: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    /// Facture d'origine, pour un avoir.
    pub fk_facture_source: Option<i64>,
    /// Bon de commande du client (mention obligatoire si établi).
    pub ref_client: Option<String>,
    /// Condition de règlement (`llx_c_payment_term`).
    pub fk_cond_reglement: Option<i64>,
    /// Numéro d'ordre dans un cycle de factures de situation (type 5).
    pub situation_counter: Option<i32>,
    /// Dernière facture du cycle de situation.
    pub situation_final: bool,
}

impl Invoice {
    pub fn est_brouillon(&self) -> bool {
        self.fk_statut == STATUT_BROUILLON
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InvoiceLine {
    pub rowid: i64,
    pub fk_facture: i64,
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
    /// Date de la prestation portée par la ligne (`date_start`).
    pub date_start: Option<String>,
    /// Unité de la ligne (`llx_c_units`).
    pub fk_unit: Option<i64>,
}

impl InvoiceLine {
    pub fn is_section(&self) -> bool {
        self.product_type == PRODUCT_TYPE_SECTION
    }
}

#[derive(Debug, Clone, Default)]
pub struct NewInvoiceLine {
    pub fk_product: Option<i64>,
    pub label: String,
    pub description: Option<String>,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub tva_tx: Decimal,
    pub remise_percent: Decimal,
    pub section: Option<String>,
}

impl NewInvoiceLine {
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

#[derive(Debug, Clone, Default)]
pub struct NewInvoice {
    pub fk_soc: i64,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub datef: Option<String>,
    pub date_lim_reglement: Option<String>,
    /// Bon de commande du client.
    pub ref_client: Option<String>,
    /// Date de la prestation (écrite sur chaque ligne, colonne `date_start`).
    pub date_prestation: Option<String>,
    pub fk_cond_reglement: Option<i64>,
    /// Chantier auquel la facture se rattache.
    pub fk_projet: Option<i64>,
    pub lines: Vec<NewInvoiceLine>,
}

impl NewInvoice {
    pub fn new(fk_soc: i64) -> Self {
        Self {
            fk_soc,
            ..Default::default()
        }
    }
}

/// Ligne prete a etre ecrite.
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

const SELECT_COLS: &str = "rowid, entity, ref, fk_soc, type, \
     DATE_FORMAT(datec, '%Y-%m-%d %H:%i:%s'), DATE_FORMAT(datef, '%Y-%m-%d'), \
     DATE_FORMAT(date_lim_reglement, '%Y-%m-%d'), fk_statut, paye, \
     CAST(total_ht AS CHAR), CAST(total_tva AS CHAR), CAST(total_ttc AS CHAR), \
     note_private, note_public, fk_facture_source, ref_client, fk_cond_reglement, \
     situation_counter, situation_final";
const LINE_COLS: &str = "d.rowid, d.fk_facture, d.fk_product, d.label, d.description, \
     CAST(d.tva_tx AS CHAR), CAST(d.qty AS CHAR), CAST(d.remise_percent AS CHAR), \
     CAST(d.subprice AS CHAR), CAST(d.price AS CHAR), CAST(d.total_ht AS CHAR), \
     CAST(d.total_tva AS CHAR), CAST(d.total_ttc AS CHAR), d.product_type, d.rang, \
     d.special_code, DATE_FORMAT(d.date_start, '%Y-%m-%d'), d.fk_unit";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Invoice {
    Invoice {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        fk_soc: opt_i(&row, 3),
        type_: opt_i(&row, 4).unwrap_or(0) as i16,
        datec: opt_s(&row, 5),
        datef: opt_s(&row, 6),
        date_lim_reglement: opt_s(&row, 7),
        fk_statut: opt_i(&row, 8).unwrap_or(0) as i16,
        paye: opt_i(&row, 9).unwrap_or(0) as i16,
        total_ht: opt_dec(&row, 10),
        total_tva: opt_dec(&row, 11),
        total_ttc: opt_dec(&row, 12),
        note_private: opt_s(&row, 13),
        note_public: opt_s(&row, 14),
        fk_facture_source: opt_i(&row, 15),
        ref_client: opt_s(&row, 16),
        fk_cond_reglement: opt_i(&row, 17),
        situation_counter: opt_i(&row, 18).map(|v| v as i32),
        situation_final: opt_i(&row, 19).unwrap_or(0) != 0,
    }
}

fn map_line(row: Row) -> InvoiceLine {
    InvoiceLine {
        rowid: opt_i(&row, 0).unwrap_or(0),
        fk_facture: opt_i(&row, 1).unwrap_or(0),
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
        date_start: opt_s(&row, 16),
        fk_unit: opt_i(&row, 17),
    }
}

/// Compte bancaire designe par la facture (`llx_facture.fk_account`).
///
/// Dolibarr imprime le RIB du compte de la facture, pas forcement celui de la
/// societe : c'est ce qui permet d'avoir plusieurs comptes.
pub fn compte_bancaire(
    pool: &Pool,
    entity: i64,
    rowid: i64,
) -> Result<Option<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // `fk_account` est nullable : demander `Option<Option<i64>>`, sinon le
    // pilote tente de convertir NULL en i64 et panique.
    let valeur: Option<Option<i64>> = conn.exec_first(
        "SELECT fk_account FROM llx_facture WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    Ok(valeur.flatten())
}

pub fn count(pool: &Pool, entity: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_facture WHERE entity = ?",
        (entity,),
    )?;
    Ok(n.unwrap_or(0))
}

pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Invoice>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_facture WHERE entity = ? \
         ORDER BY rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limit, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Invoice>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} FROM llx_facture WHERE entity = ? AND rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

pub fn lines(
    pool: &Pool,
    entity: i64,
    fk_facture: i64,
) -> Result<Vec<InvoiceLine>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {LINE_COLS} FROM llx_facturedet d \
         INNER JOIN llx_facture f ON f.rowid = d.fk_facture \
         WHERE f.entity = :entity AND d.fk_facture = :facture \
         ORDER BY d.rang ASC, d.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "facture" => fk_facture })?;
    Ok(rows.into_iter().map(map_line).collect())
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

/// Interblocage ou attente de verrou depassee : l'ecriture doit etre rejouee.
fn is_deadlock(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1213 || me.code == 1205)
}

pub(crate) fn next_ref<Q>(
    conn: &mut Q,
    entity: i64,
    prefix: &str,
    offset: u32,
) -> Result<String, Box<dyn Error>>
where
    Q: mysql::prelude::Queryable,
{
    let period: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%y%m')")?;
    let period = period.unwrap_or_else(|| "0001".to_string());
    let prefixe = format!("{prefix}{period}-");

    let posindice = prefix.len() + 6;
    let sql = format!(
        "SELECT MAX(CAST(SUBSTRING(ref FROM {posindice}) AS SIGNED)) \
         FROM llx_facture WHERE entity = :entity AND ref LIKE :motif"
    );
    let dernier: Option<Option<i64>> = conn.exec_first(
        sql,
        params! { "entity" => entity, "motif" => format!("{prefix}____-%") },
    )?;
    let suivant = dernier.flatten().unwrap_or(0) + 1 + i64::from(offset);
    if suivant > 9999 {
        Ok(format!("{prefixe}{suivant}"))
    } else {
        Ok(format!("{prefixe}{suivant:04}"))
    }
}

/// Ecrit une facture et ses lignes. Renvoie le `rowid` de la facture.
///
/// La numerotation est serialisee par un verrou consultatif MySQL (`GET_LOCK`) :
/// deux processus du noyau Rust ne peuvent pas tirer le meme numero. Aucun
/// changement de schema, et Dolibarr n'est pas impacte.
pub fn create(pool: &Pool, entity: i64, new: &NewInvoice) -> Result<i64, Box<dyn Error>> {
    if new.fk_soc <= 0 {
        return Err("le tiers (fk_soc) est obligatoire".into());
    }
    if new.lines.is_empty() {
        return Err("une facture doit avoir au moins une ligne".into());
    }
    for l in &new.lines {
        if l.qty <= Decimal::ZERO {
            return Err(format!("quantite invalide sur la ligne « {} »", l.label).into());
        }
    }

    let mut conn = pool.get_conn()?;
    let verrou = format!("dolibarr_rs_facture_{entity}");
    let pris: Option<i64> = conn.exec_first("SELECT GET_LOCK(?, 10)", (verrou.as_str(),))?;
    if pris != Some(1) {
        return Err("verrou de numerotation indisponible (10 s)".into());
    }

    let resultat = create_sous_verrou(&mut conn, entity, new, PREFIX_INVOICE, TYPE_FACTURE, None);
    let _: Option<i64> = conn.exec_first("SELECT RELEASE_LOCK(?)", (verrou.as_str(),))?;
    resultat
}

fn create_sous_verrou(
    conn: &mut mysql::PooledConn,
    entity: i64,
    new: &NewInvoice,
    prefixe: &str,
    type_document: i16,
    source: Option<i64>,
) -> Result<i64, Box<dyn Error>> {
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
                    // `fk_product = 0` n'est pas un produit absent : c'est une
                    // **ligne libre**, la facon dont Dolibarr ecrit une
                    // prestation sans fiche article. La refuser rendrait toute
                    // facture sans catalogue impossible.
                    None if id <= 0 => 0i16,
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

    let utiles = |l: &&Ligne| l.product_type != PRODUCT_TYPE_SECTION;
    let hts: Vec<Decimal> = lignes.iter().filter(utiles).map(|l| l.total_ht).collect();
    let tvas: Vec<Decimal> = lignes.iter().filter(utiles).map(|l| l.total_tva).collect();
    let ttcs: Vec<Decimal> = lignes.iter().filter(utiles).map(|l| l.total_ttc).collect();
    let total_ht = sum(&hts);
    let total_tva = sum(&tvas);
    let total_ttc = sum(&ttcs);

    // Condition de reglement et echeance : resolues avant la transaction, pour
    // que l'INSERT n'ait aucune valeur par defaut cachee. L'echeance se deduit
    // de la date de facture et de la condition (voir `reglement::echeance`).
    let datef: String = match new.datef.as_deref() {
        Some(d) if !d.trim().is_empty() => d.to_string(),
        _ => conn
            .exec_first("SELECT DATE_FORMAT(CURDATE(), '%Y-%m-%d')", ())?
            .unwrap_or_default(),
    };
    // `MIN(rowid)` rend NULL quand la table est vide : decoder en i64 ferait
    // paniquer le pilote MySQL (`exec_first::<i64>` sur NULL). On lit donc un
    // `Option<Option<i64>>` et on traite l'absence comme une erreur lisible.
    // Les conditions de reglement sont un dictionnaire : Dolibarr les seme sur
    // une entite, et les entites creees ensuite n'en ont pas de copie. On
    // cherche donc celle de l'entite, puis celle de l'entite 0, puis n'importe
    // laquelle — sans quoi la premiere facture d'une entite secondaire echoue.
    let termine: Option<Option<i64>> = conn.exec_first(
        "SELECT COALESCE( \
            (SELECT MIN(rowid) FROM llx_c_payment_term \
              WHERE active = 1 AND entity = :entity), \
            (SELECT MIN(rowid) FROM llx_c_payment_term \
              WHERE active = 1 AND entity = 0), \
            (SELECT MIN(rowid) FROM llx_c_payment_term WHERE active = 1))",
        params! { "entity" => entity },
    )?;
    let cond: i64 = match new.fk_cond_reglement {
        Some(c) => c,
        None => termine
            .flatten()
            .ok_or("aucune condition de reglement dans llx_c_payment_term")?,
    };
    let echeance: String = match new.date_lim_reglement.as_deref() {
        Some(d) if !d.trim().is_empty() => d.to_string(),
        _ => {
            let regle: Option<(Option<String>, Option<i16>, Option<i16>)> = conn.exec_first(
                "SELECT code, nbjour, decalage FROM llx_c_payment_term WHERE rowid = :id",
                params! { "id" => cond },
            )?;
            let (code, nbjour, decalage) =
                regle.ok_or_else(|| format!("condition de reglement {cond} introuvable"))?;
            crate::reglement::echeance(
                &datef,
                code.as_deref().unwrap_or(""),
                nbjour.unwrap_or(0) as i64,
                decalage.unwrap_or(0) as i64,
            )
            .ok_or_else(|| format!("date de facture invalide : {datef}"))?
        }
    };

    for attempt in 0..5u32 {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let reference = next_ref(&mut tx, entity, prefixe, attempt)?;

        let inserted = tx.exec_drop(
            "INSERT INTO llx_facture \
             (entity, ref, fk_soc, type, fk_facture_source, ref_client, datec, datef, \
              date_lim_reglement, fk_statut, paye, total_ht, total_tva, total_ttc, \
              remise_percent, note_private, note_public, fk_cond_reglement, fk_projet) \
             VALUES (:entity, :ref, :fk_soc, :type, :source, :ref_client, NOW(), \
              :datef, \
              :echeance, :statut, 0, \
              :total_ht, :total_tva, :total_ttc, 0, :note_private, :note_public, \
              :cond, :projet)",
            params! {
                "entity" => entity,
                "ref" => reference.as_str(),
                "fk_soc" => new.fk_soc,
                "type" => type_document,
                "source" => source,
                "ref_client" => new.ref_client.as_deref(),
                "cond" => cond,
            "projet" => new.fk_projet,
                "datef" => datef.as_str(),
                "echeance" => echeance.as_str(),
                "statut" => STATUT_BROUILLON,
                "total_ht" => total_ht.to_string(),
                "total_tva" => total_tva.to_string(),
                "total_ttc" => total_ttc.to_string(),
                "note_private" => new.note_private.as_deref(),
                "note_public" => new.note_public.as_deref(),
            },
        );

        match inserted {
            Ok(()) => {
                let rowid = tx.last_insert_id().unwrap_or(0) as i64;
                for (i, l) in lignes.iter().enumerate() {
                    tx.exec_drop(
                        "INSERT INTO llx_facturedet \
                         (fk_facture, fk_product, label, description, tva_tx, qty, \
                          remise_percent, subprice, price, total_ht, total_tva, \
                          total_ttc, product_type, rang, special_code, date_start) \
                         VALUES (:fk_facture, :fk_product, :label, :description, :tva_tx, \
                          :qty, :remise_percent, :subprice, :subprice, :total_ht, \
                          :total_tva, :total_ttc, :product_type, :rang, :special_code, \
                          :date_start)",
                        params! {
                            "fk_facture" => rowid,
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
                            "date_start" => new.date_prestation.as_deref(),
                        },
                    )?;
                }
                tx.commit()?;
                return Ok(rowid);
            }
            Err(e) if (is_duplicate(&e) || is_deadlock(&e)) && attempt < 4 => {
                tx.rollback()?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une reference de facture unique".into())
}

/// Cree une facture a partir d'un devis et enregistre le lien dans
/// `llx_element_element` (sourcetype `propal`, targettype `facture`).
pub fn create_from_proposal(
    pool: &Pool,
    entity: i64,
    fk_propal: i64,
) -> Result<i64, Box<dyn Error>> {
    let devis = crate::proposal::get(pool, entity, fk_propal)?
        .ok_or_else(|| format!("devis {fk_propal} introuvable dans l'entite {entity}"))?;
    // Chez Dolibarr, seul un devis SIGNE se facture : `STATUS_SIGNED` ouvre les
    // conversions dans `htdocs/comm/propal/card.php`. Sans ce garde-fou, notre
    // ecran facturait un brouillon (cf. EXTRACTIONS/devis.md).
    // 2 = signe, 4 = facture (reste convertible).
    match devis.fk_statut {
        2 | 4 => {}
        autre => {
            return Err(format!(
                "devis {} en statut {autre} : seul un devis signe peut etre facture",
                devis.ref_
            )
            .into())
        }
    }
    let fk_soc = devis.fk_soc.ok_or("le devis n'a pas de tiers")?;
    let lignes = crate::proposal::lines(pool, entity, fk_propal)?;
    if lignes.is_empty() {
        return Err("le devis n'a aucune ligne".into());
    }

    let mut new = NewInvoice::new(fk_soc);
    new.note_private = devis.note_private.clone();
    new.note_public = devis.note_public.clone();
    // La facture herite de la condition de reglement negociee sur le devis.
    new.fk_cond_reglement = devis.fk_cond_reglement;
    new.lines = lignes
        .iter()
        .filter(|l| !l.is_section())
        .map(|l| NewInvoiceLine {
            fk_product: l.fk_product,
            label: l.label.clone().unwrap_or_default(),
            description: l.description.clone(),
            qty: l.qty.unwrap_or_default(),
            unit_price: l.subprice.unwrap_or_default(),
            tva_tx: l.tva_tx.unwrap_or_default(),
            remise_percent: l.remise_percent.unwrap_or_default(),
            section: None,
        })
        .collect();

    // Un devis ne se facture qu'une fois : deux factures pour le meme devis,
    // c'est une double facturation.
    {
        let mut conn = pool.get_conn()?;
        let deja: Option<i64> = conn.exec_first(
            "SELECT fk_target FROM llx_element_element \
             WHERE sourcetype = 'propal' AND fk_source = :devis \
               AND targettype = 'facture' LIMIT 1",
            params! { "devis" => fk_propal },
        )?;
        if let Some(existante) = deja {
            return Err(format!("ce devis a deja ete facture (facture n° {existante})").into());
        }
    }

    let id = create(pool, entity, &new)?;

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_element_element \
         (fk_source, sourcetype, fk_target, targettype) \
         VALUES (:source, 'propal', :cible, 'facture')",
        params! { "source" => fk_propal, "cible" => id },
    )?;
    Ok(id)
}

/// Cree une facture a partir d'une commande validee.
///
/// Le lien est pose dans `llx_element_element` (`commande` -> `facture`) et la
/// commande n'est facturable qu'une fois : deux factures pour la meme commande,
/// c'est une double facturation.
/// Cree la facture d'acompte d'une commande, si elle en demande un.
///
/// Dolibarr le fait dans `Facture::createDepositFromOrigin` : une facture de
/// type 3 (acompte), **une ligne par taux de TVA**, valant `deposit_percent` du
/// TTC de chaque taux. Sans cela, `deposit_percent` s'imprime sur le PDF sans
/// qu'aucune facture ne le réclame — c'est l'écart relevé dans
/// `EXTRACTIONS/commande.md`.
///
/// Garde anti-doublon : la référence externe `AC-<ref commande>` est unique.
/// Une facture d'acompte existe-t-elle deja pour cette commande ?
///
/// `web` n'a pas acces a MySQL directement : la verification se fait ici.
pub fn acompte_existe(
    pool: &Pool,
    entity: i64,
    ref_commande: &str,
) -> Result<bool, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let marque = format!("AC-{ref_commande}");
    let deja: Option<String> = conn.exec_first(
        "SELECT ref_ext FROM llx_facture WHERE entity = :entity AND ref_ext = :marque LIMIT 1",
        params! { "entity" => entity, "marque" => marque.as_str() },
    )?;
    Ok(deja.is_some())
}

pub fn create_deposit_from_commande(
    pool: &Pool,
    entity: i64,
    fk_commande: i64,
) -> Result<i64, Box<dyn Error>> {
    use std::collections::BTreeMap;

    let commande = crate::commande::get(pool, entity, fk_commande)?
        .ok_or_else(|| format!("commande {fk_commande} introuvable dans l'entite {entity}"))?;
    let pct = commande
        .deposit_percent
        .filter(|p| *p > Decimal::ZERO)
        .ok_or("cette commande ne demande pas d'acompte")?;
    let fk_soc = commande.fk_soc;

    let marque = format!("AC-{}", commande.ref_);
    let mut conn = pool.get_conn()?;
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_facture WHERE entity = :entity AND ref_ext = :marque LIMIT 1",
        params! { "entity" => entity, "marque" => marque.as_str() },
    )?;
    if let Some(id) = deja {
        return Err(format!("un acompte existe deja pour cette commande (facture {id})").into());
    }

    // On ventile le TTC par taux : l'acompte se demande sur chaque taux, pas
    // sur un total unique (sinon la TVA de l'acompte serait fausse).
    let lignes = crate::commande::lines(pool, entity, fk_commande)?;
    let mut par_taux: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();
    for l in &lignes {
        if l.product_type == 9 {
            continue; // titre de section : sans montant
        }
        let ttc = l.total_ttc.unwrap_or_default();
        let taux = l.tva_tx.unwrap_or_default();
        let e = par_taux
            .entry(taux.normalize().to_string())
            .or_insert((taux, Decimal::ZERO));
        e.1 += ttc;
    }
    if par_taux.is_empty() {
        return Err("la commande n'a aucune ligne facturable".into());
    }

    let mut new = NewInvoice::new(fk_soc);
    new.fk_cond_reglement = commande.fk_cond_reglement;
    for (libelle_taux, (taux, ttc)) in &par_taux {
        let montant = money::round_money(*ttc * pct / Decimal::from(100));
        if montant <= Decimal::ZERO {
            continue;
        }
        // Le prix unitaire est un HT : on retire la TVA du montant TTC calcule.
        let ht = money::round_money(montant / (Decimal::ONE + *taux / Decimal::from(100)));
        let mut ligne = NewInvoiceLine::new(
            &format!("Acompte {} % - TVA {} %", pct.normalize(), libelle_taux),
            Decimal::ONE,
            ht,
            *taux,
        );
        ligne.description = Some(format!("Commande {}", commande.ref_));
        new.lines.push(ligne);
    }
    if new.lines.is_empty() {
        return Err("l'acompte calcule est nul".into());
    }

    let id = create(pool, entity, &new)?;
    conn.exec_drop(
        "UPDATE llx_facture SET type = :type, ref_ext = :marque, tms = NOW() \
         WHERE entity = :entity AND rowid = :id",
        params! {
            "type" => TYPE_ACOMPTE,
            "marque" => marque.as_str(),
            "entity" => entity,
            "id" => id,
        },
    )?;
    Ok(id)
}

pub fn create_from_commande(
    pool: &Pool,
    entity: i64,
    fk_commande: i64,
) -> Result<i64, Box<dyn Error>> {
    let commande = crate::commande::get(pool, entity, fk_commande)?
        .ok_or_else(|| format!("commande {fk_commande} introuvable"))?;
    if commande.fk_statut < crate::commande::STATUT_VALIDEE {
        return Err("une commande non validee ne se facture pas".into());
    }
    let lignes = crate::commande::lines(pool, entity, fk_commande)?;
    if lignes.is_empty() {
        return Err("la commande n'a aucune ligne".into());
    }

    {
        let mut conn = pool.get_conn()?;
        let deja: Option<i64> = conn.exec_first(
            "SELECT fk_target FROM llx_element_element \
             WHERE sourcetype = 'commande' AND fk_source = :commande \
               AND targettype = 'facture' LIMIT 1",
            params! { "commande" => fk_commande },
        )?;
        if let Some(existante) = deja {
            return Err(
                format!("cette commande a deja ete facturee (facture n° {existante})").into(),
            );
        }
    }

    let mut new = NewInvoice::new(commande.fk_soc);
    new.note_private = commande.note_private.clone();
    new.note_public = commande.note_public.clone();
    new.fk_cond_reglement = commande.fk_cond_reglement;
    new.lines = lignes
        .iter()
        .filter(|l| !l.is_section())
        .map(|l| NewInvoiceLine {
            fk_product: l.fk_product,
            label: l.label.clone().unwrap_or_default(),
            description: l.description.clone(),
            qty: l.qty.unwrap_or_default(),
            unit_price: l.subprice.unwrap_or_default(),
            tva_tx: l.tva_tx.unwrap_or_default(),
            remise_percent: l.remise_percent.unwrap_or_default(),
            section: None,
        })
        .collect();

    let id = create(pool, entity, &new)?;

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_element_element \
         (fk_source, sourcetype, fk_target, targettype) \
         VALUES (:source, 'commande', :cible, 'facture')",
        params! { "source" => fk_commande, "cible" => id },
    )?;
    Ok(id)
}

/// Passe la facture en validee (statut 1). Le numero est deja attribue a la creation,
/// comme le fait Dolibarr.
/// Cree un avoir sur une facture validee.
///
/// Convention Dolibarr : `type = 2`, lignes a quantite **negative**, reference
/// prefixee `AV`, et `fk_facture_source` pointant sur la facture d'origine.
/// On ne supprime jamais une facture : on l'annule par un avoir.
pub fn create_avoir(pool: &Pool, entity: i64, fk_facture: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let verrou = format!("dolibarr_rs_facture_{entity}");
    let pris: Option<i64> = conn.exec_first("SELECT GET_LOCK(?, 10)", (verrou.as_str(),))?;
    if pris != Some(1) {
        return Err("verrou de numerotation indisponible (10 s)".into());
    }
    let resultat = avoir_sous_verrou(&mut conn, entity, fk_facture);
    let _: Option<i64> = conn.exec_first("SELECT RELEASE_LOCK(?)", (verrou.as_str(),))?;
    resultat
}

fn avoir_sous_verrou(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_facture: i64,
) -> Result<i64, Box<dyn Error>> {
    let facture = get_par_id(conn, entity, fk_facture)?;

    if facture.type_ == TYPE_AVOIR {
        return Err("un avoir ne peut pas recevoir d'avoir".into());
    }
    if facture.fk_statut < STATUT_VALIDEE {
        return Err("seule une facture validee peut etre annulee par un avoir".into());
    }
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_facture WHERE entity = :entity AND fk_facture_source = :source \
         AND type = :type LIMIT 1",
        params! { "entity" => entity, "source" => fk_facture, "type" => TYPE_AVOIR },
    )?;
    if let Some(a) = deja {
        return Err(format!("cette facture a deja un avoir (n° {a})").into());
    }
    let fk_soc = facture.fk_soc.ok_or("la facture n'a pas de tiers")?;

    let lignes_src = lines_par_id(conn, fk_facture)?;
    if lignes_src.is_empty() {
        return Err("la facture n'a aucune ligne".into());
    }

    // Quantites negativées : l'avoir annule la facture sans la modifier.
    let mut new = NewInvoice::new(fk_soc);
    new.note_private = Some(format!("Avoir sur la facture {}", facture.ref_));
    new.lines = lignes_src
        .iter()
        .filter(|l| !l.is_section())
        .map(|l| NewInvoiceLine {
            fk_product: l.fk_product,
            label: l.label.clone().unwrap_or_default(),
            description: l.description.clone(),
            qty: -(l.qty.unwrap_or_default()),
            unit_price: l.subprice.unwrap_or_default(),
            tva_tx: l.tva_tx.unwrap_or_default(),
            remise_percent: l.remise_percent.unwrap_or_default(),
            section: None,
        })
        .collect();

    let id = create_sous_verrou(
        conn,
        entity,
        &new,
        PREFIX_CREDIT_NOTE,
        TYPE_AVOIR,
        Some(fk_facture),
    )?;
    Ok(id)
}

fn get_par_id(
    conn: &mut mysql::PooledConn,
    entity: i64,
    rowid: i64,
) -> Result<Invoice, Box<dyn Error>> {
    let sql = format!("SELECT {SELECT_COLS} FROM llx_facture WHERE entity = ? AND rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    row.map(map_row)
        .ok_or_else(|| format!("facture {rowid} introuvable").into())
}

fn lines_par_id(
    conn: &mut mysql::PooledConn,
    fk_facture: i64,
) -> Result<Vec<InvoiceLine>, Box<dyn Error>> {
    let sql = format!(
        "SELECT {LINE_COLS} FROM llx_facturedet d WHERE d.fk_facture = ? \
         ORDER BY d.rang ASC, d.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, (fk_facture,))?;
    Ok(rows.into_iter().map(map_line).collect())
}

pub fn validate(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_facture WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => return Err(format!("facture {rowid} introuvable").into()),
        Some(s) if s != i64::from(STATUT_BROUILLON) => {
            return Err(format!("la facture {rowid} n'est pas en brouillon").into())
        }
        _ => {}
    }
    conn.exec_drop(
        "UPDATE llx_facture SET fk_statut = :statut, date_valid = NOW(), tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

pub fn delete(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_facture WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    if existe.is_none() {
        return Ok(());
    }
    let mut tx = conn.start_transaction(TxOpts::default())?;

    // Reglements affectes a cette facture : on retire l'affectation, puis le
    // reglement s'il ne servait qu'a cette facture. Aucune ecriture comptable
    // n'est laissee orpheline.
    let paiements: Vec<i64> = tx.exec(
        "SELECT fk_paiement FROM llx_paiement_facture WHERE fk_facture = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_paiement_facture WHERE fk_facture = :rowid",
        params! { "rowid" => rowid },
    )?;
    for p in paiements {
        let reste: Option<i64> = tx.exec_first(
            "SELECT COUNT(*) FROM llx_paiement_facture WHERE fk_paiement = :paiement",
            params! { "paiement" => p },
        )?;
        if reste.unwrap_or(0) == 0 {
            tx.exec_drop(
                "DELETE FROM llx_paiement WHERE entity = :entity AND rowid = :paiement",
                params! { "entity" => entity, "paiement" => p },
            )?;
        }
    }

    tx.exec_drop(
        "DELETE FROM llx_facturedet WHERE fk_facture = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_element_element WHERE targettype = 'facture' AND fk_target = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_facture WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}
