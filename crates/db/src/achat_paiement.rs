//! Paiements fournisseur : `llx_paiementfourn` et sa table de liaison
//! `llx_paiementfourn_facturefourn`.
//!
//! Sans ce module, une facture d'achat validee reste indefiniment a
//! `fk_statut = 1` : l'argent sort de la banque et rien ne le dit. On ecrit
//! donc les trois choses que Dolibarr attend :
//!
//! 1. l'en-tete du paiement (`llx_paiementfourn`) ;
//! 2. la ligne qui l'affecte a la facture (`llx_paiementfourn_facturefourn`) ;
//! 3. la bascule de la facture en « payee » (`llx_facture_fourn.paye = 1`,
//!    `fk_statut = 2`) **quand le solde tombe a zero**, jamais avant.
//!
//! La mecanique reprend celle du reglement client ([`crate::payment`]) plutot
//! que d'en inventer une : meme verrou consultatif par entite, meme reference
//! `PAYF` + `AAMM` + `-` + 4 chiffres, meme reessai sur collision, et surtout
//! meme principe de calcul — on additionne **les affectations reelles** de la
//! facture, on ne fait pas confiance au drapeau `paye`.
//!
//! Trois ecarts avec le reglement client, tous imposes par le schema :
//!
//! - **les tables sont differentes** : Dolibarr range l'argent sortant dans
//!   `llx_paiementfourn`, l'argent entrant dans `llx_paiement`. Les references
//!   sont donc numerotees separement ;
//! - **`statut`** : `0` est un brouillon et n'entre pas dans le solde (c'est la
//!   convention deja suivie par [`crate::sepa`]), `1` est un paiement reel. Ce
//!   module ecrit toujours `1` : il n'y a pas d'etape de validation ;
//! - **`fk_bank` est `NOT NULL`** mais reste a `0` : voir ci-dessous.
//!
//! ## Ce qui n'est pas fait ici
//!
//! - **Aucune ligne `llx_bank`** n'est ecrite : `fk_bank` vaut `0`, donc
//!   Dolibarr ne voit pas encore le debit sur le releve. Le rapprochement
//!   bancaire est un autre sujet (il demande de choisir un compte, et le faire
//!   au hasard — « le premier compte ouvert » — provoque des collisions, cf. le
//!   commentaire de `payment.rs` sur ce point) ;
//! - **aucune ecriture comptable** : ni `llx_accounting_bookkeeping`, ni
//!   ventilation sur un journal d'achat. Le paiement existe en gestion, pas en
//!   comptabilite generale.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

use crate::achat::{STATUT_ABANDONNEE, STATUT_BROUILLON, STATUT_PAYEE};

/// Prefixe des references de paiement fournisseur.
///
/// Distinct de celui des reglements clients (`PAY`) : une reference lue a
/// l'ecran doit designer un seul paiement, pas deux selon la table.
pub const PREFIXE_PAIEMENT: &str = "PAYF";

/// `llx_paiementfourn.statut` : paiement enregistre mais pas encore parti.
pub const STATUT_PAIEMENT_BROUILLON: i16 = 0;
/// `llx_paiementfourn.statut` : paiement reel, compte dans le solde.
pub const STATUT_PAIEMENT_VALIDE: i16 = 1;

/// Un paiement fournisseur, vu depuis une facture.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PaiementFourn {
    pub rowid: i64,
    /// `llx_paiementfourn.ref`.
    pub ref_: String,
    /// Date de paiement (`datep`), `AAAA-MM-JJ`.
    pub datep: Option<String>,
    /// Montant total du paiement (`llx_paiementfourn.amount`).
    pub montant: Decimal,
    /// Part affectee a **cette** facture
    /// (`llx_paiementfourn_facturefourn.amount`).
    pub montant_affecte: Decimal,
    /// Code du mode de paiement (`llx_c_paiement.code` : `VIR`, `CHQ`...).
    pub mode: Option<String>,
    /// Numero externe (cheque, virement), quand il y en a un.
    pub num_paiement: Option<String>,
    /// `llx_paiementfourn.statut` : `0` brouillon, `1` valide.
    pub statut: i16,
}

const SELECT_COLS: &str = "p.rowid, p.ref, DATE_FORMAT(p.datep, '%Y-%m-%d'), \
     CAST(p.amount AS CHAR), CAST(pf.amount AS CHAR), c.code, p.num_paiement, p.statut";

/// Somme des paiements **valides** affectes a une facture.
///
/// Le filtre `entity` porte sur l'en-tete du paiement : la table de liaison n'a
/// pas de colonne `entity` (comme `llx_facture_fourn_det`). Un brouillon
/// (`statut = 0`) est exclu ; un `statut` NULL est traite comme valide, ce qu'il
/// est en pratique pour les paiements ecrits par Dolibarr avant cette colonne.
const REGLE_SQL: &str = "SELECT CAST(COALESCE(SUM(pf.amount), 0) AS CHAR) \
     FROM llx_paiementfourn_facturefourn pf \
     INNER JOIN llx_paiementfourn p ON p.rowid = pf.fk_paiementfourn \
     WHERE pf.fk_facturefourn = :facture AND p.entity = :entity \
       AND COALESCE(p.statut, 1) <> 0";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> PaiementFourn {
    PaiementFourn {
        rowid: opt_i(&row, 0).unwrap_or(0),
        ref_: opt_s(&row, 1).unwrap_or_default(),
        datep: opt_s(&row, 2),
        montant: opt_dec(&row, 3).unwrap_or_default(),
        montant_affecte: opt_dec(&row, 4).unwrap_or_default(),
        mode: opt_s(&row, 5),
        num_paiement: opt_s(&row, 6),
        statut: opt_i(&row, 7).unwrap_or(i64::from(STATUT_PAIEMENT_VALIDE)) as i16,
    }
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

/// Interblocage ou attente de verrou depassee : l'ecriture doit etre rejouee.
fn is_deadlock(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1213 || me.code == 1205)
}

/// Montant lu dans une colonne `double(24,8)` via `CAST(... AS CHAR)`.
fn lire_montant(s: Option<String>) -> Decimal {
    s.and_then(|v| Decimal::from_str(v.trim()).ok())
        .map(money::round_money)
        .unwrap_or_default()
}

/// Somme des paiements valides affectes a la facture.
fn deja_paye<Q>(conn: &mut Q, entity: i64, facture_id: i64) -> Result<Decimal, Box<dyn Error>>
where
    Q: mysql::prelude::Queryable,
{
    let brut: Option<String> = conn.exec_first(
        REGLE_SQL,
        params! { "facture" => facture_id, "entity" => entity },
    )?;
    Ok(lire_montant(brut))
}

/// Identifiant du mode de paiement dans `llx_c_paiement`.
///
/// Le code (`VIR`, `CHQ`, `ESP`...) est la forme attendue ; un identifiant
/// numerique est accepte pour les appelants qui n'ont que l'`id` du dictionnaire
/// sous la main. Un mode inconnu est refuse : `llx_paiementfourn.fk_paiement`
/// est `NOT NULL` et une valeur fantaisiste rendrait le paiement illisible.
fn id_mode<Q>(conn: &mut Q, moyen: &str) -> Result<i64, Box<dyn Error>>
where
    Q: mysql::prelude::Queryable,
{
    let moyen = moyen.trim();
    if moyen.is_empty() {
        return Err(
            "le moyen de paiement est obligatoire (code llx_c_paiement, ex. « VIR »)".into(),
        );
    }

    let trouve: Option<i64> = if moyen.bytes().all(|b| b.is_ascii_digit()) {
        let id: i64 = moyen.parse()?;
        conn.exec_first(
            "SELECT id FROM llx_c_paiement WHERE id = :id",
            params! { "id" => id },
        )?
    } else {
        conn.exec_first(
            "SELECT id FROM llx_c_paiement WHERE UPPER(code) = UPPER(:code) LIMIT 1",
            params! { "code" => moyen },
        )?
    };

    trouve.ok_or_else(|| {
        format!(
            "le moyen de paiement « {moyen} » n'existe pas dans llx_c_paiement \
             (codes courants : VIR, CHQ, ESP, CB)"
        )
        .into()
    })
}

/// Date de paiement au format `AAAA-MM-JJ`.
///
/// La validation passe par `DATE_FORMAT` : une date illisible rend NULL et on
/// la refuse ici plutot que par un `Incorrect date value` au milieu de la
/// transaction.
fn date_paiement<Q>(conn: &mut Q, date: &str) -> Result<String, Box<dyn Error>>
where
    Q: mysql::prelude::Queryable,
{
    let date = date.trim();
    if date.is_empty() {
        return Err("la date de paiement est obligatoire (format AAAA-MM-JJ)".into());
    }
    let lue: Option<Option<String>> =
        conn.exec_first("SELECT DATE_FORMAT(?, '%Y-%m-%d')", (date,))?;
    match lue.flatten() {
        Some(v) => Ok(v),
        None => Err(
            format!("date de paiement invalide : « {date} » (format attendu AAAA-MM-JJ)").into(),
        ),
    }
}

/// Numero suivant : `PAYF` + `AAMM` + `-` + 4 chiffres.
///
/// L'`offset` sert au reessai quand deux paiements tires en meme temps tombent
/// sur le meme numero.
fn next_ref<Q>(conn: &mut Q, entity: i64, offset: u32) -> Result<String, Box<dyn Error>>
where
    Q: mysql::prelude::Queryable,
{
    let period: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%y%m')")?;
    let period = period.unwrap_or_else(|| "0001".to_string());
    let prefixe = format!("{PREFIXE_PAIEMENT}{period}-");

    // Les chiffres commencent au 10e caractere : `PAYF` (4) + `AAMM` (4) + `-`.
    let posindice = PREFIXE_PAIEMENT.len() + 6;
    let sql = format!(
        "SELECT MAX(CAST(SUBSTRING(ref FROM {posindice}) AS SIGNED)) \
         FROM llx_paiementfourn WHERE entity = :entity AND ref LIKE :motif"
    );
    let dernier: Option<Option<i64>> = conn.exec_first(
        sql,
        params! { "entity" => entity, "motif" => format!("{PREFIXE_PAIEMENT}____-%") },
    )?;
    let suivant = dernier.flatten().unwrap_or(0) + 1 + i64::from(offset);
    Ok(format!("{prefixe}{suivant:04}"))
}

/// Reste a payer d'une facture fournisseur : TTC moins la somme des paiements
/// **lies** a cette facture.
///
/// On lit les vraies affectations (`llx_paiementfourn_facturefourn`) et pas la
/// colonne `llx_facture_fourn.paye` : cette derniere est un drapeau tout ou
/// rien, elle ne dit pas combien reste du.
pub fn reste_a_payer(pool: &Pool, entity: i64, facture_id: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let total: Option<String> = conn.exec_first(
        "SELECT CAST(total_ttc AS CHAR) FROM llx_facture_fourn \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => facture_id },
    )?;
    let Some(total) = total else {
        return Err(format!(
            "la facture fournisseur {facture_id} n'existe pas dans l'entite {entity}"
        )
        .into());
    };

    let total = lire_montant(Some(total));
    let regle = deja_paye(&mut conn, entity, facture_id)?;
    let reste = total - regle;
    Ok(if reste <= Decimal::ZERO {
        Decimal::ZERO
    } else {
        money::round_money(reste)
    })
}

/// Paiements lies a une facture fournisseur, du plus ancien au plus recent.
///
/// Les brouillons sont rendus eux aussi (avec `statut = 0`) : c'est a l'appelant
/// de les distinguer, alors que [`reste_a_payer`] les exclut.
pub fn paiements(
    pool: &Pool,
    entity: i64,
    facture_id: i64,
) -> Result<Vec<PaiementFourn>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_paiementfourn p \
         INNER JOIN llx_paiementfourn_facturefourn pf ON pf.fk_paiementfourn = p.rowid \
         LEFT JOIN llx_c_paiement c ON c.id = p.fk_paiement \
         WHERE p.entity = :entity AND pf.fk_facturefourn = :facture \
         ORDER BY p.datep ASC, p.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "facture" => facture_id })?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Enregistre le paiement d'une facture fournisseur et renvoie son `rowid`.
///
/// Refuse, sans rien ecrire :
///
/// - un montant nul ou negatif (et un montant qui s'arrondit a zero centime) ;
/// - une facture en brouillon ou abandonnee ;
/// - une facture deja soldee, ou un montant superieur au reste du ;
/// - une facture qui n'existe pas dans l'entite, un moyen de paiement inconnu,
///   une date illisible.
///
/// Un paiement partiel est normal : la facture reste a `fk_statut = 1` et
/// `paye = 0` tant que la somme des affectations n'atteint pas le TTC.
pub fn payer(
    pool: &Pool,
    entity: i64,
    facture_id: i64,
    montant: Decimal,
    moyen: &str,
    date: &str,
) -> Result<i64, Box<dyn Error>> {
    if entity <= 0 {
        return Err("l'entite est obligatoire".into());
    }
    if facture_id <= 0 {
        return Err("la facture fournisseur (facture_id) est obligatoire".into());
    }
    if montant <= Decimal::ZERO {
        return Err("le montant du paiement doit etre superieur a zero".into());
    }
    // Arrondi au centime avant tout : un montant de 0,004 ne doit pas creer un
    // paiement de zero euro.
    let montant = money::round_money(montant);
    if montant <= Decimal::ZERO {
        return Err("le montant du paiement doit etre superieur a zero".into());
    }

    let mut conn = pool.get_conn()?;
    let verrou = format!("dolibarr_rs_paiement_fourn_{entity}");
    let pris: Option<i64> = conn.exec_first("SELECT GET_LOCK(?, 10)", (verrou.as_str(),))?;
    if pris != Some(1) {
        return Err("verrou de numerotation indisponible (10 s)".into());
    }
    let resultat = payer_sous_verrou(&mut conn, entity, facture_id, montant, moyen, date);
    let _: Option<i64> = conn.exec_first("SELECT RELEASE_LOCK(?)", (verrou.as_str(),))?;
    resultat
}

fn payer_sous_verrou(
    conn: &mut mysql::PooledConn,
    entity: i64,
    facture_id: i64,
    montant: Decimal,
    moyen: &str,
    date: &str,
) -> Result<i64, Box<dyn Error>> {
    let mode = id_mode(conn, moyen)?;
    let datep = date_paiement(conn, date)?;

    let facture: Option<(i64, Option<String>)> = conn.exec_first(
        "SELECT fk_statut, CAST(total_ttc AS CHAR) FROM llx_facture_fourn \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => facture_id },
    )?;
    let Some((statut, ttc)) = facture else {
        return Err(format!(
            "la facture fournisseur {facture_id} n'existe pas dans l'entite {entity}"
        )
        .into());
    };
    if statut == i64::from(STATUT_BROUILLON) {
        return Err(
            "une facture fournisseur en brouillon ne peut pas etre payee : validez-la d'abord"
                .into(),
        );
    }
    if statut == i64::from(STATUT_ABANDONNEE) {
        return Err("une facture fournisseur abandonnee ne peut pas etre payee".into());
    }

    let total_ttc = lire_montant(ttc);
    let regle = deja_paye(conn, entity, facture_id)?;
    let reste = if total_ttc > regle {
        money::round_money(total_ttc - regle)
    } else {
        Decimal::ZERO
    };
    if reste <= Decimal::ZERO {
        return Err(format!(
            "la facture fournisseur {facture_id} est deja soldee, il n'y a rien a payer"
        )
        .into());
    }
    if montant > reste {
        return Err(format!(
            "le montant {montant} depasse le reste a payer {reste} de la facture fournisseur {facture_id}"
        )
        .into());
    }

    for attempt in 0..5u32 {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let reference = next_ref(&mut tx, entity, attempt)?;

        let insere = tx.exec_drop(
            "INSERT INTO llx_paiementfourn \
             (ref, entity, datec, datep, amount, fk_paiement, num_paiement, note, \
              fk_bank, statut) \
             VALUES (:ref, :entity, NOW(), :datep, :amount, :mode, NULL, NULL, 0, :statut)",
            params! {
                "ref" => reference.as_str(),
                "entity" => entity,
                "datep" => datep.as_str(),
                "amount" => montant.to_string(),
                "mode" => mode,
                "statut" => STATUT_PAIEMENT_VALIDE,
            },
        );

        match insere {
            Ok(()) => {
                let rowid = tx.last_insert_id().unwrap_or(0) as i64;
                tx.exec_drop(
                    "INSERT INTO llx_paiementfourn_facturefourn \
                     (fk_paiementfourn, fk_facturefourn, amount, multicurrency_code, \
                      multicurrency_tx, multicurrency_amount) \
                     VALUES (:paiement, :facture, :amount, NULL, 1, :amount)",
                    params! {
                        "paiement" => rowid,
                        "facture" => facture_id,
                        "amount" => montant.to_string(),
                    },
                )?;

                // Lettrage : on additionne ce qui est reellement affecte a la
                // facture, brouillons exclus, et on ne bascule qu'au solde.
                let regle = deja_paye(&mut tx, entity, facture_id)?;
                if regle >= total_ttc {
                    tx.exec_drop(
                        "UPDATE llx_facture_fourn SET paye = 1, fk_statut = :payee, tms = NOW() \
                         WHERE entity = :entity AND rowid = :rowid",
                        params! {
                            "payee" => STATUT_PAYEE,
                            "entity" => entity,
                            "rowid" => facture_id,
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
    Err("impossible d'attribuer une reference de paiement fournisseur unique".into())
}
