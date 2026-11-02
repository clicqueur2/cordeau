//! Reglements (`llx_paiement` + `llx_paiement_facture`) et lettrage.
//!
//! Un reglement peut etre partiel : on additionne les affectations de la facture
//! et on la bascule en « payee » quand le total atteint le TTC.
//!
//! Contrairement aux factures, aucune numerotation legale n'est imposee aux
//! reglements. On utilise `PAY` + `AAAMM` + `-` + 4 chiffres, lisible et trie.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

pub const PREFIX_PAIEMENT: &str = "PAY";

/// Identifiants de `llx_c_paiement` (dictionnaire Dolibarr).
pub const MODE_VIREMENT: i64 = 2;
pub const MODE_PRELEVEMENT: i64 = 3;
pub const MODE_ESPECES: i64 = 4;
pub const MODE_CARTE: i64 = 6;
pub const MODE_CHEQUE: i64 = 7;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Payment {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub datep: Option<String>,
    pub amount: Option<Decimal>,
    pub fk_paiement: Option<i64>,
    pub num_paiement: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NewPayment {
    pub fk_facture: i64,
    pub montant: Decimal,
    pub fk_paiement: i64,
    pub num_paiement: Option<String>,
    pub note: Option<String>,
    pub date: Option<String>,
    /// Compte bancaire a debiter. `None` = le premier compte ouvert de l'entite.
    pub fk_compte: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InvoiceBalance {
    pub total_ttc: Decimal,
    pub regle: Decimal,
    pub reste: Decimal,
    pub soldee: bool,
}

const SELECT_COLS: &str = "p.rowid, p.entity, p.ref, \
     DATE_FORMAT(p.datep, '%Y-%m-%d'), CAST(p.amount AS CHAR), p.fk_paiement, \
     p.num_paiement, p.note";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Payment {
    Payment {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        datep: opt_s(&row, 3),
        amount: opt_dec(&row, 4),
        fk_paiement: opt_i(&row, 5),
        num_paiement: opt_s(&row, 6),
        note: opt_s(&row, 7),
    }
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

/// Interblocage ou attente de verrou depassee : l'ecriture doit etre rejouee.
fn is_deadlock(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1213 || me.code == 1205)
}

/// Code du mode de paiement (`VIR`, `CHQ`...) attendu par `llx_bank.fk_type`.
fn code_mode<Q: mysql::prelude::Queryable>(conn: &mut Q, id: i64) -> Option<String> {
    conn.exec_first("SELECT code FROM llx_c_paiement WHERE id = ?", (id,))
        .ok()
        .flatten()
}

/// Ecrit la ligne bancaire du reglement.
///
/// Dolibarr ne considere un encaissement comme reel que s'il apparait dans
/// `llx_bank`, relie au paiement par `llx_bank_url`. Sans compte bancaire
/// declare, on ne peut rien ecrire : on le signale a l'appelant.
struct EcritureBancaire<'a> {
    entity: i64,
    fk_paiement: i64,
    montant: Decimal,
    mode: i64,
    num: Option<&'a str>,
    date: Option<&'a str>,
    label: &'a str,
    fk_compte: Option<i64>,
}

fn ecrire_ligne_bancaire(
    conn: &mut mysql::Transaction<'_>,
    e: &EcritureBancaire<'_>,
) -> Result<bool, Box<dyn Error>> {
    let (entity, fk_paiement, montant, mode) = (e.entity, e.fk_paiement, e.montant, e.mode);
    let (num, date, label) = (e.num, e.date, e.label);
    // Compte explicite quand l'appelant le fournit : choisir implicitement
    // « le premier compte ouvert » provoque des collisions entre ecritures
    // concurrentes.
    let compte: Option<i64> = match e.fk_compte {
        Some(c) => conn.exec_first(
            "SELECT rowid FROM llx_bank_account WHERE rowid = :c AND entity = :entity",
            params! { "c" => c, "entity" => entity },
        )?,
        None => conn.exec_first(
            "SELECT rowid FROM llx_bank_account \
             WHERE entity = :entity AND (clos = 0 OR clos IS NULL) ORDER BY rowid ASC LIMIT 1",
            params! { "entity" => entity },
        )?,
    };
    let compte = match compte {
        Some(c) => c,
        None => return Ok(false),
    };

    let code = code_mode(conn, mode).unwrap_or_else(|| "VIR".to_string());

    conn.exec_drop(
        "INSERT INTO llx_bank \
         (datec, dateo, amount, amount_main_currency, label, fk_account, fk_type, \
          num_chq, rappro, origin_id, origin_type) \
         VALUES (NOW(), COALESCE(:dateo, CURDATE()), :amount, :amount, :label, \
          :compte, :type, :num, 0, :paiement, 'payment')",
        params! {
            "dateo" => date,
            "amount" => montant.to_string(),
            "label" => label,
            "compte" => compte,
            "type" => code.as_str(),
            "num" => num,
            "paiement" => fk_paiement,
        },
    )?;
    let fk_bank = conn.last_insert_id().unwrap_or(0);

    conn.exec_drop(
        "INSERT INTO llx_bank_url (fk_bank, url_id, url, label, type) \
         VALUES (:bank, :paiement, :url, :label, 'payment')",
        params! {
            "bank" => fk_bank,
            "paiement" => fk_paiement,
            "url" => "/compta/paiement/card.php?id=",
            "label" => label,
        },
    )?;
    Ok(true)
}

fn next_ref<Q>(conn: &mut Q, entity: i64, offset: u32) -> Result<String, Box<dyn Error>>
where
    Q: mysql::prelude::Queryable,
{
    let period: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%y%m')")?;
    let period = period.unwrap_or_else(|| "0001".to_string());
    let prefixe = format!("{PREFIX_PAIEMENT}{period}-");

    let posindice = PREFIX_PAIEMENT.len() + 6;
    let sql = format!(
        "SELECT MAX(CAST(SUBSTRING(ref FROM {posindice}) AS SIGNED)) \
         FROM llx_paiement WHERE entity = :entity AND ref LIKE :motif"
    );
    let dernier: Option<Option<i64>> = conn.exec_first(
        sql,
        params! { "entity" => entity, "motif" => format!("{PREFIX_PAIEMENT}____-%") },
    )?;
    let suivant = dernier.flatten().unwrap_or(0) + 1 + i64::from(offset);
    Ok(format!("{prefixe}{suivant:04}"))
}

/// Solde d'une facture : TTC, deja regle, reste du.
pub fn balance(
    pool: &Pool,
    entity: i64,
    fk_facture: i64,
) -> Result<InvoiceBalance, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let total: Option<String> = conn.exec_first(
        "SELECT CAST(total_ttc AS CHAR) FROM llx_facture WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_facture },
    )?;
    let total = total.ok_or("facture introuvable")?;
    let total_ttc = Decimal::from_str(total.trim()).unwrap_or_default();

    let regle: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(pf.amount), 0) AS CHAR) \
         FROM llx_paiement_facture pf \
         INNER JOIN llx_paiement p ON p.rowid = pf.fk_paiement \
         WHERE p.entity = :entity AND pf.fk_facture = :facture",
        params! { "entity" => entity, "facture" => fk_facture },
    )?;
    let regle = regle
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .unwrap_or_default();

    let reste = total_ttc - regle;
    Ok(InvoiceBalance {
        total_ttc,
        regle,
        reste: if reste < Decimal::ZERO {
            Decimal::ZERO
        } else {
            reste
        },
        soldee: reste <= Decimal::ZERO,
    })
}

pub fn list_for_invoice(
    pool: &Pool,
    entity: i64,
    fk_facture: i64,
) -> Result<Vec<Payment>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_paiement p \
         INNER JOIN llx_paiement_facture pf ON pf.fk_paiement = p.rowid \
         WHERE p.entity = :entity AND pf.fk_facture = :facture \
         ORDER BY p.datep ASC, p.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "facture" => fk_facture })?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Enregistre un reglement et son affectation, puis met la facture a jour.
pub fn create(pool: &Pool, entity: i64, new: &NewPayment) -> Result<i64, Box<dyn Error>> {
    if new.montant <= Decimal::ZERO {
        return Err("le montant du reglement doit etre superieur a zero".into());
    }
    if new.fk_facture <= 0 {
        return Err("la facture (fk_facture) est obligatoire".into());
    }

    let mut conn = pool.get_conn()?;
    let verrou = format!("dolibarr_rs_paiement_{entity}");
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
    new: &NewPayment,
) -> Result<i64, Box<dyn Error>> {
    let facture: Option<(i64, Option<String>, Option<String>)> = conn.exec_first(
        "SELECT fk_statut, CAST(paye AS CHAR), CAST(total_ttc AS CHAR) FROM llx_facture \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => new.fk_facture },
    )?;
    let (statut, paye, ttc) = match facture {
        None => {
            return Err(format!(
                "la facture {} n'existe pas dans l'entite {}",
                new.fk_facture, entity
            )
            .into())
        }
        Some(v) => v,
    };
    if statut == 3 {
        return Err("une facture abandonnee ne peut pas etre reglee".into());
    }
    if statut == 0 {
        return Err("une facture en brouillon ne peut pas etre reglee : validez-la d'abord".into());
    }
    if paye.as_deref() == Some("1") {
        return Err("la facture est deja soldee".into());
    }
    let total_ttc = ttc
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .unwrap_or_default();

    for attempt in 0..5u32 {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let reference = next_ref(&mut tx, entity, attempt)?;

        let insere = tx.exec_drop(
            "INSERT INTO llx_paiement \
             (ref, entity, datec, datep, amount, fk_paiement, num_paiement, note, statut) \
             VALUES (:ref, :entity, NOW(), COALESCE(:datep, CURDATE()), :amount, \
              :mode, :num, :note, 1)",
            params! {
                "ref" => reference.as_str(),
                "entity" => entity,
                "datep" => new.date.as_deref(),
                "amount" => new.montant.to_string(),
                "mode" => new.fk_paiement,
                "num" => new.num_paiement.as_deref(),
                "note" => new.note.as_deref(),
            },
        );

        match insere {
            Ok(()) => {
                let rowid = tx.last_insert_id().unwrap_or(0) as i64;
                tx.exec_drop(
                    "INSERT INTO llx_paiement_facture (fk_paiement, fk_facture, amount) \
                     VALUES (:paiement, :facture, :amount)",
                    params! {
                        "paiement" => rowid,
                        "facture" => new.fk_facture,
                        "amount" => new.montant.to_string(),
                    },
                )?;

                // La ligne bancaire : sans elle, Dolibarr ne voit pas l'encaissement.
                let libelle = format!("Reglement {reference}");
                ecrire_ligne_bancaire(
                    &mut tx,
                    &EcritureBancaire {
                        entity,
                        fk_paiement: rowid,
                        montant: new.montant,
                        mode: new.fk_paiement,
                        num: new.num_paiement.as_deref(),
                        date: new.date.as_deref(),
                        label: &libelle,
                        fk_compte: new.fk_compte,
                    },
                )?;

                // Lettrage : on additionne ce qui est affecte a la facture.
                let regle: Option<String> = tx.exec_first(
                    "SELECT CAST(COALESCE(SUM(amount), 0) AS CHAR) \
                     FROM llx_paiement_facture WHERE fk_facture = :facture",
                    params! { "facture" => new.fk_facture },
                )?;
                let regle = regle
                    .and_then(|s| Decimal::from_str(s.trim()).ok())
                    .unwrap_or_default();

                if regle >= total_ttc {
                    tx.exec_drop(
                        "UPDATE llx_facture SET paye = 1, fk_statut = 2, tms = NOW() \
                         WHERE entity = :entity AND rowid = :rowid",
                        params! { "entity" => entity, "rowid" => new.fk_facture },
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
    Err("impossible d'attribuer une reference de reglement unique".into())
}

/// Supprime un reglement et son affectation, puis recalcule l'etat de la facture.
pub fn delete(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let factures: Vec<i64> = conn.exec(
        "SELECT pf.fk_facture FROM llx_paiement_facture pf \
         INNER JOIN llx_paiement p ON p.rowid = pf.fk_paiement \
         WHERE p.entity = :entity AND p.rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;

    let mut tx = conn.start_transaction(TxOpts::default())?;

    // On retire aussi la ligne bancaire, sinon le releve garde un encaissement fantome.
    let bancaires: Vec<i64> = tx.exec(
        "SELECT fk_bank FROM llx_bank_url WHERE type = 'payment' AND url_id = :rowid",
        params! { "rowid" => rowid },
    )?;
    for b in &bancaires {
        tx.exec_drop(
            "DELETE FROM llx_bank_url WHERE fk_bank = :bank",
            params! { "bank" => *b },
        )?;
        tx.exec_drop(
            "DELETE FROM llx_bank WHERE rowid = :bank",
            params! { "bank" => *b },
        )?;
    }

    tx.exec_drop(
        "DELETE FROM llx_paiement_facture WHERE fk_paiement = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_paiement WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;

    for fk in &factures {
        let total: Option<String> = tx.exec_first(
            "SELECT CAST(total_ttc AS CHAR) FROM llx_facture WHERE rowid = :rowid",
            params! { "rowid" => *fk },
        )?;
        let total = total
            .and_then(|s| Decimal::from_str(s.trim()).ok())
            .unwrap_or_default();
        let regle: Option<String> = tx.exec_first(
            "SELECT CAST(COALESCE(SUM(amount), 0) AS CHAR) \
             FROM llx_paiement_facture WHERE fk_facture = :facture",
            params! { "facture" => *fk },
        )?;
        let regle = regle
            .and_then(|s| Decimal::from_str(s.trim()).ok())
            .unwrap_or_default();
        let soldee = regle >= total;
        tx.exec_drop(
            "UPDATE llx_facture SET paye = :paye, \
              fk_statut = CASE WHEN :soldee THEN 2 ELSE fk_statut END, tms = NOW() \
             WHERE rowid = :rowid",
            params! {
                "paye" => if soldee { 1 } else { 0 },
                "soldee" => soldee,
                "rowid" => *fk,
            },
        )?;
    }
    tx.commit()?;
    Ok(())
}
