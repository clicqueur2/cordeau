//! Saisie d'une facture fournisseur : en-tete `llx_facture_fourn` et lignes
//! `llx_facture_fourn_det`.
//!
//! `db::achat` reste la lecture (fournisseurs, liste des factures, total des
//! achats) ; ce module-ci ecrit. Le schema Dolibarr n'est pas touche : aucune
//! table, aucune colonne ajoutee, et les conventions de `mod_facture_fourn`
//! sont reprises telles quelles.
//!
//! Pieges du schema, verifies sur la base 24.0.1 (`schema-24.0.1.json` et
//! `information_schema`) :
//!
//! - `ref_supplier` est `NOT NULL` **et** entre dans l'index unique
//!   `uk_facture_fourn_ref_supplier (ref_supplier, fk_soc, entity)`. Deux
//!   factures du meme fournisseur sans reference fournisseur se heurtent donc a
//!   une cle dupliquee : une chaine vide est une valeur comme une autre. Le
//!   refus est fait ici par une verification explicite, pour rendre un message
//!   lisible plutot qu'une erreur 1062 brute.
//! - `ref` est `NOT NULL` : on ecrit directement la reference definitive
//!   (`FF` + AAMM + `-` + numero sur 4 chiffres), comme le fait deja
//!   `db::invoice` pour les factures clients. Dolibarr, lui, cree le brouillon
//!   avec `(PROV<id>)` et ne renumerote a la validation que si la reference
//!   commence par `PROV` : une facture ecrite ici garde donc son numero si elle
//!   est validee depuis l'interface Dolibarr.
//! - le numero est cherche sur **toutes** les periodes (`LIKE 'FF____-%'`),
//!   comme `mod_facture_fournisseur_cactus`, et le AAMM est celui de la **date
//!   de facture** (et non celle du jour) : c'est ce que fait le modele Dolibarr.
//! - `llx_facture_fourn_det` n'a pas de colonne `entity` : le filtre passe par
//!   la jointure sur l'en-tete.
//! - `fk_code_ventilation` est `NOT NULL` (defaut 0) : il est ecrit
//!   explicitement plutot que laisse a un defaut implicite.
//! - la base porte des cles etrangeres que le manifeste n'indexe pas
//!   (`fk_facture_fourn_fk_soc`, `fk_facture_fourn_det_fk_facture`) : un tiers
//!   inexistant est de toute facon refuse par la base, mais il vaut mieux le
//!   refuser avant d'ouvrir une transaction.

use money::{line_totals, sum, LineInput};
use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;
use std::error::Error;
use std::str::FromStr;

pub use crate::achat::{STATUT_BROUILLON, STATUT_VALIDEE};

/// Prefixe des references internes de facture fournisseur.
pub const PREFIXE: &str = "FF";

/// Une ligne de facture fournisseur, telle qu'on la saisit et telle qu'on la
/// relit.
///
/// Le prix unitaire est hors taxes : c'est la base de saisie de Dolibarr pour
/// une facture fournisseur (`price_base_type = 'HT'`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LigneAchat {
    pub label: String,
    pub qty: Decimal,
    pub unit_price_ht: Decimal,
    /// Taux de TVA en pourcentage (20, 10, 5.5, 2.1 ou 0).
    pub tva_tx: Decimal,
}

impl LigneAchat {
    pub fn new(label: &str, qty: Decimal, unit_price_ht: Decimal, tva_tx: Decimal) -> Self {
        Self {
            label: label.to_string(),
            qty,
            unit_price_ht,
            tva_tx,
        }
    }

    /// Totaux de la ligne, arrondis comme Dolibarr (`price.lib.php`).
    fn totaux(&self) -> money::LineTotals {
        line_totals(&LineInput::new(
            &self.qty.to_string(),
            &self.unit_price_ht.to_string(),
            "0",
            &self.tva_tx.to_string(),
        ))
    }

    /// Prix unitaire TTC, que Dolibarr stocke aussi dans `pu_ttc`.
    fn prix_unitaire_ttc(&self) -> Decimal {
        let cent = Decimal::from(100);
        money::round_unit_price(self.unit_price_ht * (Decimal::ONE + self.tva_tx / cent))
    }
}

/// Une facture fournisseur a saisir.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NouvelleFactureAchat {
    /// Tiers fournisseur (`llx_societe.fournisseur = 1`).
    pub fk_soc: i64,
    /// Reference du document chez le fournisseur. Facultative, mais unique par
    /// fournisseur (voir la note du module).
    pub ref_supplier: Option<String>,
    /// Date de facture, `AAAA-MM-JJ`. `None` = aujourd'hui.
    pub datef: Option<String>,
    /// Note interne (`note_private`).
    pub note: Option<String>,
    pub lines: Vec<LigneAchat>,
}

impl NouvelleFactureAchat {
    pub fn new(fk_soc: i64) -> Self {
        Self {
            fk_soc,
            ..Default::default()
        }
    }
}

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
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
    label: String,
    qty: Decimal,
    unit_price_ht: Decimal,
    pu_ttc: Decimal,
    tva_tx: Decimal,
    total_ht: Decimal,
    total_tva: Decimal,
    total_ttc: Decimal,
}

fn preparer_lignes(new: &NouvelleFactureAchat) -> Vec<Ligne> {
    new.lines
        .iter()
        .map(|l| {
            let t = l.totaux();
            Ligne {
                label: l.label.trim().to_string(),
                qty: l.qty,
                unit_price_ht: t.unit_price_ht,
                pu_ttc: l.prix_unitaire_ttc(),
                tva_tx: l.tva_tx,
                total_ht: t.total_ht,
                total_tva: t.total_vat,
                total_ttc: t.total_ttc,
            }
        })
        .collect()
}

/// Totaux d'en-tete : somme des lignes, arrondie une seule fois.
fn totaux_lignes(lignes: &[Ligne]) -> (Decimal, Decimal, Decimal) {
    let hts: Vec<Decimal> = lignes.iter().map(|l| l.total_ht).collect();
    let tvas: Vec<Decimal> = lignes.iter().map(|l| l.total_tva).collect();
    let ttcs: Vec<Decimal> = lignes.iter().map(|l| l.total_ttc).collect();
    (sum(&hts), sum(&tvas), sum(&ttcs))
}

fn verifier_entree(new: &NouvelleFactureAchat) -> Result<(), Box<dyn Error>> {
    if new.fk_soc <= 0 {
        return Err("le fournisseur (fk_soc) est obligatoire".into());
    }
    if new.lines.is_empty() {
        return Err("une facture fournisseur doit comporter au moins une ligne".into());
    }
    for (i, l) in new.lines.iter().enumerate() {
        if l.label.trim().is_empty() {
            return Err(format!("ligne {} : le libelle est obligatoire", i + 1).into());
        }
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

/// Numero suivant : `FF` + `AAMM` + `-` + numero sur 4 chiffres.
///
/// Le maximum est cherche sur toutes les periodes, comme
/// `mod_facture_fournisseur_cactus::getNextValue` : le compteur ne repart pas a
/// zero au changement de mois. Le AAMM est celui de la date de facture. Les
/// chiffres commencent au 8e caractere (`FF` = 2, `AAMM` = 4, `-` = 1).
///
/// L'`offset` sert au reessai en cas de collision sur `uk_facture_fourn_ref`.
fn next_ref<Q: mysql::prelude::Queryable>(
    conn: &mut Q,
    entity: i64,
    datef: &str,
    offset: u32,
) -> Result<String, Box<dyn Error>> {
    let yymm: Option<Option<String>> =
        conn.exec_first("SELECT DATE_FORMAT(?, '%y%m')", (datef,))?;
    let yymm = yymm.flatten().unwrap_or_else(|| "0001".to_string());

    let dernier: Option<Option<i64>> = conn.exec_first(
        "SELECT MAX(CAST(SUBSTRING(ref, 8) AS SIGNED)) FROM llx_facture_fourn \
         WHERE entity = :entity AND ref LIKE 'FF____-%'",
        params! { "entity" => entity },
    )?;
    let suivant = dernier.flatten().unwrap_or(0) + 1 + i64::from(offset);
    // Au-dela de 9999, Dolibarr ne formate plus sur 4 caracteres : le nombre
    // reste tel quel.
    let num = if suivant > 9999 {
        suivant.to_string()
    } else {
        format!("{suivant:04}")
    };
    Ok(format!("{PREFIXE}{yymm}-{num}"))
}

fn inserer_lignes(
    tx: &mut mysql::Transaction<'_>,
    fk_facture: i64,
    lignes: &[Ligne],
) -> Result<(), Box<dyn Error>> {
    for (i, l) in lignes.iter().enumerate() {
        tx.exec_drop(
            "INSERT INTO llx_facture_fourn_det \
             (fk_facture_fourn, label, description, pu_ht, pu_ttc, qty, \
              remise_percent, vat_src_code, tva_tx, total_ht, tva, total_ttc, \
              product_type, info_bits, fk_code_ventilation, special_code, rang) \
             VALUES (:fk_facture, NULL, :description, :pu_ht, :pu_ttc, :qty, \
              0, '', :tva_tx, :total_ht, :tva, :total_ttc, 0, 0, 0, 0, :rang)",
            params! {
                "fk_facture" => fk_facture,
                "description" => l.label.as_str(),
                "pu_ht" => l.unit_price_ht.to_string(),
                "pu_ttc" => l.pu_ttc.to_string(),
                "qty" => l.qty.to_string(),
                "tva_tx" => l.tva_tx.to_string(),
                "total_ht" => l.total_ht.to_string(),
                "tva" => l.total_tva.to_string(),
                "total_ttc" => l.total_ttc.to_string(),
                "rang" => (i + 1) as i32,
            },
        )?;
    }
    Ok(())
}

/// Insere l'en-tete puis les lignes, dans la meme transaction.
fn inserer(
    conn: &mut mysql::PooledConn,
    entity: i64,
    new: &NouvelleFactureAchat,
    lignes: &[Ligne],
    datef: &str,
    ref_supplier: &str,
) -> Result<i64, Box<dyn Error>> {
    let (total_ht, total_tva, total_ttc) = totaux_lignes(lignes);

    for attempt in 0..5u32 {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let reference = next_ref(&mut tx, entity, datef, attempt)?;

        let inserted = tx.exec_drop(
            "INSERT INTO llx_facture_fourn \
             (ref, ref_supplier, entity, type, fk_soc, datec, datef, fk_statut, \
              total_ht, total_tva, total_ttc, note_private) \
             VALUES (:ref, :ref_supplier, :entity, 0, :fk_soc, NOW(), :datef, 0, \
              :total_ht, :total_tva, :total_ttc, :note_private)",
            params! {
                "ref" => reference.as_str(),
                "ref_supplier" => ref_supplier,
                "entity" => entity,
                "fk_soc" => new.fk_soc,
                "datef" => datef,
                "total_ht" => total_ht.to_string(),
                "total_tva" => total_tva.to_string(),
                "total_ttc" => total_ttc.to_string(),
                "note_private" => new.note.as_deref(),
            },
        );

        match inserted {
            Ok(()) => {
                let rowid = tx.last_insert_id().unwrap_or(0) as i64;
                inserer_lignes(&mut tx, rowid, lignes)?;
                tx.commit()?;
                return Ok(rowid);
            }
            Err(e) if (is_duplicate(&e) || is_deadlock(&e)) && attempt < 4 => {
                tx.rollback()?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une reference de facture fournisseur unique".into())
}

/// Cree une facture fournisseur en brouillon (`fk_statut = 0`) et renvoie son
/// `rowid`.
///
/// Le tiers doit exister dans l'entite **et** etre un fournisseur
/// (`llx_societe.fournisseur = 1`). La numerotation est serialisee par un
/// verrou consultatif MySQL (`GET_LOCK`), relache dans tous les cas : deux
/// processus du noyau Rust ne peuvent pas tirer le meme numero.
pub fn creer(pool: &Pool, entity: i64, new: &NouvelleFactureAchat) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;

    let mut conn = pool.get_conn()?;

    let tiers: Option<(i64, i64)> = conn.exec_first(
        "SELECT rowid, fournisseur FROM llx_societe \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => new.fk_soc },
    )?;
    let Some((_, fournisseur)) = tiers else {
        return Err(format!(
            "le tiers {} n'existe pas dans l'entite {}",
            new.fk_soc, entity
        )
        .into());
    };
    if fournisseur != 1 {
        return Err(format!(
            "le tiers {} n'est pas un fournisseur : la fiche doit avoir la case \
             fournisseur cochee (llx_societe.fournisseur = 1)",
            new.fk_soc
        )
        .into());
    }

    // `uk_facture_fourn_ref_supplier` porte sur (ref_supplier, fk_soc, entity) :
    // l'unicite est donc par fournisseur, chaine vide comprise.
    let ref_supplier = new
        .ref_supplier
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_facture_fourn \
         WHERE entity = :entity AND fk_soc = :fk_soc AND ref_supplier = :ref_supplier",
        params! {
            "entity" => entity,
            "fk_soc" => new.fk_soc,
            "ref_supplier" => ref_supplier.as_str(),
        },
    )?;
    if let Some(existante) = deja {
        return Err(if ref_supplier.is_empty() {
            format!(
                "le fournisseur {} a deja la facture fournisseur {} sans reference \
                 fournisseur : renseignez la reference du document",
                new.fk_soc, existante
            )
        } else {
            format!(
                "le fournisseur {} a deja une facture avec la reference fournisseur « {} »",
                new.fk_soc, ref_supplier
            )
        }
        .into());
    }

    // Date de facture : elle fixe le AAMM de la reference. Une date illisible est
    // refusee ici plutot que par un `Incorrect date value` au milieu de la
    // transaction.
    let datef: String = match new.datef.as_deref().map(str::trim) {
        Some(d) if !d.is_empty() => {
            // `DATE_FORMAT` renvoie NULL sur une date illisible : le type
            // `Option<String>` est obligatoire, sinon le pilote panique sur le
            // NULL au lieu de rendre la main.
            let lue: Option<Option<String>> =
                conn.exec_first("SELECT DATE_FORMAT(?, '%Y-%m-%d')", (d,))?;
            match lue.flatten() {
                Some(v) => v,
                None => {
                    return Err(format!(
                        "date de facture invalide : « {d} » (format attendu AAAA-MM-JJ)"
                    )
                    .into())
                }
            }
        }
        _ => conn
            .exec_first("SELECT DATE_FORMAT(CURDATE(), '%Y-%m-%d')", ())?
            .unwrap_or_default(),
    };

    let lignes = preparer_lignes(new);

    let verrou = format!("dolibarr_rs_facture_fourn_{entity}");
    let pris: Option<i64> = conn.exec_first("SELECT GET_LOCK(?, 10)", (verrou.as_str(),))?;
    if pris != Some(1) {
        return Err("verrou de numerotation indisponible (10 s)".into());
    }
    let resultat = inserer(&mut conn, entity, new, &lignes, &datef, &ref_supplier);
    let _: Option<i64> = conn.exec_first("SELECT RELEASE_LOCK(?)", (verrou.as_str(),))?;
    resultat
}

/// Relit les lignes d'une facture fournisseur, dans l'ordre de saisie.
///
/// Le libelle vient de `description` (c'est la que Dolibarr met le texte d'une
/// ligne libre, et c'est ce que le PDF imprime) ; `label` sert de repli quand la
/// ligne vient d'un produit.
pub fn lignes(
    pool: &Pool,
    entity: i64,
    fk_facture: i64,
) -> Result<Vec<LigneAchat>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT COALESCE(NULLIF(d.description, ''), d.label, ''), \
                CAST(d.qty AS CHAR), CAST(d.pu_ht AS CHAR), CAST(d.tva_tx AS CHAR) \
         FROM llx_facture_fourn_det d \
         INNER JOIN llx_facture_fourn f ON f.rowid = d.fk_facture_fourn \
         WHERE f.entity = :entity AND d.fk_facture_fourn = :fk_facture \
         ORDER BY d.rang ASC, d.rowid ASC",
        params! { "entity" => entity, "fk_facture" => fk_facture },
    )?;
    Ok(rows
        .iter()
        .map(|r| LigneAchat {
            label: opt_s(r, 0).unwrap_or_default(),
            qty: opt_dec(r, 1).unwrap_or_default(),
            unit_price_ht: opt_dec(r, 2).unwrap_or_default(),
            tva_tx: opt_dec(r, 3).unwrap_or_default(),
        })
        .collect())
}

/// Passe une facture fournisseur de brouillon (0) a validee (1).
///
/// Refuse une facture deja validee (ou cloturee) et une facture sans ligne : une
/// facture validee sans ligne serait un document vide et definitif.
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_facture_fourn WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => return Err(format!("facture fournisseur {rowid} introuvable").into()),
        Some(s) if s != i64::from(STATUT_BROUILLON) => {
            return Err("cette facture fournisseur n'est pas en brouillon".into())
        }
        _ => {}
    }

    let nb: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_facture_fourn_det WHERE fk_facture_fourn = :rowid",
        params! { "rowid" => rowid },
    )?;
    if nb.unwrap_or(0) == 0 {
        return Err("une facture fournisseur sans ligne ne peut pas etre validee".into());
    }

    conn.exec_drop(
        "UPDATE llx_facture_fourn SET fk_statut = :validee, date_valid = CURDATE(), tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "validee" => STATUT_VALIDEE,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Totaux (HT, TVA, TTC) relus sur la facture, au centime.
///
/// Ce sont les totaux d'en-tete : ceux que Dolibarr affiche et imprime, calcules
/// une seule fois a partir des lignes (comme `FactureFournisseur::update_price`).
pub fn total(
    pool: &Pool,
    entity: i64,
    rowid: i64,
) -> Result<(Decimal, Decimal, Decimal), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let row: Option<Row> = conn.exec_first(
        "SELECT CAST(total_ht AS CHAR), CAST(total_tva AS CHAR), CAST(total_ttc AS CHAR) \
         FROM llx_facture_fourn WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(row) = row else {
        return Err(format!("facture fournisseur {rowid} introuvable").into());
    };
    Ok((
        opt_dec(&row, 0).unwrap_or_default().normalize(),
        opt_dec(&row, 1).unwrap_or_default().normalize(),
        opt_dec(&row, 2).unwrap_or_default().normalize(),
    ))
}
