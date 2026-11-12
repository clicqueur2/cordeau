//! Factures de situation (`llx_facture` de type 5).
//!
//! Un devis signe se facture par tranches : 40 %, 25 %, 15 %, 10 %... Chaque
//! situation est une **vraie facture**, numerotee, valable, et la somme des
//! situations egale le devis au centime (la derniere prend le reste).
//!
//! Colonnes Dolibarr utilisees : `type = 5`, `fk_facture_source` = le devis,
//! `situation_cycle_ref` = le `rowid` de la premiere situation du cycle,
//! `situation_counter` = 1, 2, 3..., `situation_final` = 1 sur la derniere.
//!
//! Le pourcentage n'est pas stocke : il se deduit du TTC de la situation
//! rapportee au TTC du devis. Une seule source de verite, pas de copie.

use std::collections::BTreeMap;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, TxOpts};
use rust_decimal::Decimal;

use crate::invoice::{self, STATUT_BROUILLON, TYPE_SITUATION};

/// Une situation existante sur un devis.
#[derive(Debug, Clone, PartialEq)]
pub struct Situation {
    pub rowid: i64,
    pub ref_: String,
    pub datef: Option<String>,
    /// Reference du cycle (rowid de la premiere situation).
    pub cycle_ref: Option<i64>,
    pub counter: i32,
    pub finale: bool,
    pub total_ht: Decimal,
    pub total_ttc: Decimal,
    /// Part du devis deja facturee par cette situation, en pourcentage.
    pub pourcentage: Decimal,
}

fn opt_s(row: &mysql::Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &mysql::Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &mysql::Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn arrondi(d: Decimal) -> Decimal {
    money::round_money(d)
}

/// Total TTC d'un devis, hors lignes de section.
pub fn total_devis(pool: &Pool, entity: i64, fk_propal: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let brut: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(d.total_ttc), 0) AS CHAR) FROM llx_propaldet d \
         JOIN llx_propal p ON p.rowid = d.fk_propal \
         WHERE p.entity = :entity AND p.rowid = :propal AND d.product_type <> 9",
        params! { "entity" => entity, "propal" => fk_propal },
    )?;
    Ok(brut
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .map(arrondi)
        .unwrap_or(Decimal::ZERO))
}

/// Situations d'un devis, dans l'ordre du cycle.
pub fn situations(
    pool: &Pool,
    entity: i64,
    fk_propal: i64,
) -> Result<Vec<Situation>, Box<dyn Error>> {
    let total = total_devis(pool, entity, fk_propal)?;
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT f.rowid, f.ref, DATE_FORMAT(f.datef, '%Y-%m-%d'), \
                f.situation_cycle_ref, f.situation_counter, f.situation_final, \
                CAST(f.total_ht AS CHAR), CAST(f.total_ttc AS CHAR) \
         FROM llx_facture f \
         JOIN llx_element_element e ON e.fk_target = f.rowid AND e.targettype = 'facture' \
         WHERE f.entity = :entity AND e.sourcetype = 'propal' \
           AND e.fk_source = :propal AND f.type = :type \
         ORDER BY f.situation_counter ASC, f.rowid ASC",
        params! { "entity" => entity, "propal" => fk_propal, "type" => TYPE_SITUATION },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let ttc = opt_dec(&r, 7).unwrap_or_default();
            Situation {
                rowid: opt_i(&r, 0).unwrap_or(0),
                ref_: opt_s(&r, 1).unwrap_or_default(),
                datef: opt_s(&r, 2),
                cycle_ref: opt_i(&r, 3),
                counter: opt_i(&r, 4).unwrap_or(0) as i32,
                finale: opt_i(&r, 5).unwrap_or(0) != 0,
                total_ht: opt_dec(&r, 6).unwrap_or_default(),
                total_ttc: ttc,
                pourcentage: if total > Decimal::ZERO {
                    arrondi(ttc * Decimal::from(100) / total)
                } else {
                    Decimal::ZERO
                },
            }
        })
        .collect())
}

/// Somme HT et TVA du devis, par taux de TVA.
fn totaux_par_taux(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_propal: i64,
) -> Result<BTreeMap<String, (Decimal, Decimal)>, Box<dyn Error>> {
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT CAST(d.tva_tx AS CHAR), CAST(SUM(d.total_ht) AS CHAR), \
                CAST(SUM(d.total_tva) AS CHAR) \
         FROM llx_propaldet d JOIN llx_propal p ON p.rowid = d.fk_propal \
         WHERE p.entity = :entity AND p.rowid = :propal AND d.product_type <> 9 \
         GROUP BY d.tva_tx ORDER BY d.tva_tx",
        params! { "entity" => entity, "propal" => fk_propal },
    )?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let taux = opt_s(&r, 0)?;
            Some((
                taux,
                (
                    opt_dec(&r, 1).unwrap_or_default(),
                    opt_dec(&r, 2).unwrap_or_default(),
                ),
            ))
        })
        .collect())
}

/// Deja facture (HT) sur ce devis, par taux de TVA.
fn deja_par_taux(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_propal: i64,
) -> Result<BTreeMap<String, Decimal>, Box<dyn Error>> {
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT CAST(d.tva_tx AS CHAR), CAST(SUM(d.total_ht) AS CHAR) \
         FROM llx_facturedet d \
         JOIN llx_facture f ON f.rowid = d.fk_facture \
         JOIN llx_element_element e ON e.fk_target = f.rowid AND e.targettype = 'facture' \
         WHERE f.entity = :entity AND e.sourcetype = 'propal' \
           AND e.fk_source = :propal AND f.type = :type \
         GROUP BY d.tva_tx",
        params! { "entity" => entity, "propal" => fk_propal, "type" => TYPE_SITUATION },
    )?;
    Ok(rows
        .into_iter()
        .filter_map(|r| Some((opt_s(&r, 0)?, opt_dec(&r, 1).unwrap_or_default())))
        .collect())
}

/// Cree une facture de situation pour `pourcentage` du devis.
///
/// Refuse si le devis n'est pas valide, si le pourcentage est nul ou si le
/// cumul depasse 100 %. Le pourcentage est arrondi a 2 decimales.
pub fn creer(
    pool: &Pool,
    entity: i64,
    fk_propal: i64,
    pourcentage: Decimal,
    datef: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    if pourcentage <= Decimal::ZERO {
        return Err("le pourcentage de la situation doit etre positif".into());
    }
    let devis = crate::proposal::get(pool, entity, fk_propal)?
        .ok_or_else(|| format!("devis {fk_propal} introuvable"))?;
    if devis.fk_statut < 1 {
        return Err("un devis non valide ne se facture pas par situation".into());
    }

    // Un devis deja facture **en une fois** ne se facture pas en situations : ce
    // serait la meme prestation comptee deux fois. Le controle ne regardait que
    // les situations, donc une facture unique liee au devis passait inapercue —
    // c'est le sens unique que la revue a trouve. Les acomptes (type 3) sont
    // exclus : ils n'ont pas facture le devis, ils ont encaisse une part.
    {
        let mut conn = pool.get_conn()?;
        let deja_facture: Option<(i64, String)> = conn.exec_first(
            "SELECT f.rowid, f.ref FROM llx_facture f \
             JOIN llx_element_element e ON e.fk_target = f.rowid AND e.targettype = 'facture' \
             WHERE f.entity = :entity AND e.sourcetype = 'propal' AND e.fk_source = :propal \
               AND f.type NOT IN (:situation, :acompte) AND f.fk_statut >= 1 \
             ORDER BY f.rowid LIMIT 1",
            params! {
                "entity" => entity,
                "propal" => fk_propal,
                "situation" => TYPE_SITUATION,
                "acompte" => crate::invoice::TYPE_ACOMPTE,
            },
        )?;
        if let Some((id, reference)) = deja_facture {
            return Err(format!(
                "ce devis est deja facture en une fois (facture {reference}, n° {id}) : \
                 des situations s'ajouteraient par-dessus et compteraient la prestation deux fois"
            )
            .into());
        }
    }

    let deja = situations(pool, entity, fk_propal)?;
    let cumul: Decimal = deja.iter().map(|s| s.pourcentage).sum();
    let total_pct = cumul + pourcentage;
    if total_pct > Decimal::from(100) {
        return Err(format!(
            "le cumul depasse 100 % : deja {} %, demande {} %",
            cumul, pourcentage
        )
        .into());
    }

    let mut conn = pool.get_conn()?;
    let verrou = format!("dolibarr_rs_facture_{entity}");
    let pris: Option<i64> = conn.exec_first("SELECT GET_LOCK(?, 10)", (verrou.as_str(),))?;
    if pris != Some(1) {
        return Err("verrou de numerotation indisponible (10 s)".into());
    }
    let resultat = creer_sous_verrou(
        &mut conn,
        entity,
        fk_propal,
        &devis,
        &deja,
        pourcentage,
        datef,
    );
    let _: Option<i64> = conn.exec_first("SELECT RELEASE_LOCK(?)", (verrou.as_str(),))?;
    resultat
}

fn creer_sous_verrou(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_propal: i64,
    devis: &crate::proposal::Proposal,
    deja: &[Situation],
    pourcentage: Decimal,
    datef: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let taux = totaux_par_taux(conn, entity, fk_propal)?;
    if taux.is_empty() {
        return Err("le devis n'a aucune ligne facturable".into());
    }
    let deja_ht = deja_par_taux(conn, entity, fk_propal)?;
    // L'acompte deja encaisse se deduit du solde, et compte dans le plafond :
    // sans lui, 30 % d'acompte puis quatre situations de 25 % facturaient 130 %
    // du devis.
    let acompte_ht = crate::invoice::acomptes_par_taux(conn, entity, fk_propal, &devis.ref_)?;

    let counter = deja.len() as i32 + 1;
    // La derniere situation prend le reste : la somme des situations egale
    // le devis, au centime.
    let finale =
        deja.iter().map(|s| s.pourcentage).sum::<Decimal>() + pourcentage >= Decimal::from(100);
    let pct = pourcentage / Decimal::from(100);

    let mut lignes: Vec<(String, Decimal, Decimal)> = Vec::new(); // (libelle, ht, taux)
    for (taux_txt, (ht, _tva)) in &taux {
        let deja_ht_taux = deja_ht.get(taux_txt).copied().unwrap_or_default();
        let acompte_taux = acompte_ht.get(taux_txt).copied().unwrap_or_default();
        let montant = if finale {
            // Le solde : ce que le devis demande, moins les situations deja
            // facturees, moins l'acompte deja encaisse.
            arrondi(*ht - deja_ht_taux - acompte_taux)
        } else {
            arrondi(*ht * pct)
        };
        if montant <= Decimal::ZERO {
            continue;
        }
        let taux_fmt = Decimal::from_str(taux_txt.trim())
            .map(|d| d.normalize().to_string())
            .unwrap_or_else(|_| taux_txt.clone());
        let libelle = if finale {
            format!(
                "Situation {} - solde du devis {} (TVA {} %)",
                counter, devis.ref_, taux_fmt
            )
        } else {
            format!(
                "Situation {} - {} % du devis {} (TVA {} %)",
                counter, pourcentage, devis.ref_, taux_fmt
            )
        };
        lignes.push((
            libelle,
            montant,
            Decimal::from_str(taux_txt.trim()).unwrap_or_default(),
        ));
    }
    if lignes.is_empty() {
        return Err(format!(
            "cette situation ne produit aucun montant : les {} % demandes sont deja \
             couverts par l'acompte et les situations precedentes",
            pourcentage
        )
        .into());
    }

    // Le plafond : le TTC du devis. Situations + acomptes + cette situation ne
    // peuvent pas le depasser.
    let plafond: Decimal = taux.iter().map(|(_, (ht, tva))| ht + tva).sum();
    let nouveau: Decimal = lignes
        .iter()
        .map(|(_, ht, taux_ligne)| ht + arrondi(*ht * *taux_ligne / Decimal::from(100)))
        .sum();
    let situations_ttc: Decimal = deja.iter().map(|s| s.total_ttc).sum();
    let acomptes_ttc: Decimal = acompte_ht
        .iter()
        .map(|(taux_txt, ht)| {
            let t = Decimal::from_str(taux_txt.trim()).unwrap_or_default();
            ht + arrondi(*ht * t / Decimal::from(100))
        })
        .sum();
    if situations_ttc + acomptes_ttc + nouveau
        > plafond + Decimal::from_str("0.02").unwrap_or_default()
    {
        return Err(format!(
            "cette situation depasserait le devis : deja facture {} (situations {} + \
             acomptes {}), demande {}, total du devis {}",
            arrondi(situations_ttc + acomptes_ttc),
            arrondi(situations_ttc),
            arrondi(acomptes_ttc),
            arrondi(nouveau),
            arrondi(plafond)
        )
        .into());
    }

    // Totaux, avec la meme arithmetique que le reste du noyau.
    let mut total_ht = Decimal::ZERO;
    let mut total_tva = Decimal::ZERO;
    let mut total_ttc = Decimal::ZERO;
    let mut calculees = Vec::with_capacity(lignes.len());
    for (libelle, montant, tva_tx) in &lignes {
        let t = money::line_totals(&money::LineInput {
            qty: Decimal::ONE,
            unit_price: *montant,
            discount_percent: Decimal::ZERO,
            vat_rate: *tva_tx,
            base: money::PriceBase::Ht,
            vat_mode: money::VatMode::Normal,
        });
        total_ht += t.total_ht;
        total_tva += t.total_vat;
        total_ttc += t.total_ttc;
        calculees.push((libelle.clone(), *montant, *tva_tx, t));
    }

    let cond: i64 = match devis.fk_cond_reglement {
        Some(c) => c,
        None => conn
            .exec_first(
                "SELECT MIN(rowid) FROM llx_c_payment_term \
                 WHERE active = 1 AND (entity = :entity OR entity = 0)",
                params! { "entity" => entity },
            )?
            .unwrap_or(1),
    };
    // Unite des lignes : « forfait » si le dictionnaire la propose, sinon la
    // piece. Coder 2 en dur visait KG : c'etait faux.
    let unite: Option<i64> = crate::dict::unite_par_code(conn, "FORFAIT")
        .ok()
        .flatten()
        .or_else(|| crate::dict::unite_par_code(conn, "P").ok().flatten());
    let date_facture: String = match datef.map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => d.to_string(),
        None => conn
            .exec_first("SELECT DATE_FORMAT(CURDATE(), '%Y-%m-%d')", ())?
            .unwrap_or_default(),
    };
    let echeance = {
        let regle: Option<(Option<String>, Option<i16>, Option<i16>)> = conn.exec_first(
            "SELECT code, nbjour, decalage FROM llx_c_payment_term WHERE rowid = :id",
            params! { "id" => cond },
        )?;
        match regle {
            Some((code, nbjour, decalage)) => crate::reglement::echeance(
                &date_facture,
                code.as_deref().unwrap_or(""),
                nbjour.unwrap_or(0) as i64,
                decalage.unwrap_or(0) as i64,
            )
            .unwrap_or_else(|| date_facture.clone()),
            None => date_facture.clone(),
        }
    };

    for attempt in 0..5u32 {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let reference = invoice::next_ref(&mut tx, entity, invoice::PREFIX_INVOICE, attempt)?;

        let insere = tx.exec_drop(
            "INSERT INTO llx_facture \
             (entity, ref, fk_soc, type, fk_facture_source, datec, datef, \
              date_lim_reglement, fk_statut, paye, total_ht, total_tva, total_ttc, \
              remise_percent, note_private, note_public, fk_cond_reglement, \
              situation_cycle_ref, situation_counter, situation_final) \
             VALUES (:entity, :ref, :fk_soc, :type, :source, NOW(), :datef, \
              :echeance, :statut, 0, :total_ht, :total_tva, :total_ttc, \
              0, :note, NULL, :cond, NULL, :counter, :finale)",
            params! {
                "entity" => entity,
                "ref" => reference.as_str(),
                "fk_soc" => devis.fk_soc,
                "type" => TYPE_SITUATION,
                // `fk_facture_source` porte une FK vers llx_facture : c'est la
                // situation precedente. Le lien au devis passe par
                // llx_element_element, comme pour une facture classique.
                "source" => deja.last().map(|s| s.rowid),
                "datef" => date_facture.as_str(),
                "echeance" => echeance.as_str(),
                "statut" => STATUT_BROUILLON,
                "total_ht" => total_ht.to_string(),
                "total_tva" => total_tva.to_string(),
                "total_ttc" => total_ttc.to_string(),
                "note" => devis.note_private.as_deref(),
                "cond" => cond,
                "counter" => counter,
                "finale" => if finale { 1 } else { 0 },
            },
        );

        match insere {
            Ok(()) => {
                let rowid = tx.last_insert_id().unwrap_or(0) as i64;
                // Le cycle porte le rowid de sa premiere situation : c'est la
                // convention Dolibarr (`situation_cycle_ref`).
                let cycle = deja.first().and_then(|s| s.cycle_ref).unwrap_or(rowid);
                tx.exec_drop(
                    "UPDATE llx_facture SET situation_cycle_ref = :cycle WHERE rowid = :rowid",
                    params! { "cycle" => cycle, "rowid" => rowid },
                )?;
                tx.exec_drop(
                    "INSERT INTO llx_element_element \
                     (fk_source, sourcetype, fk_target, targettype) \
                     VALUES (:propal, 'propal', :facture, 'facture')",
                    params! { "propal" => fk_propal, "facture" => rowid },
                )?;
                for (rang, (libelle, montant, tva_tx, t)) in calculees.iter().enumerate() {
                    tx.exec_drop(
                        "INSERT INTO llx_facturedet \
                         (fk_facture, label, description, tva_tx, qty, remise_percent, \
                          subprice, price, total_ht, total_tva, total_ttc, \
                          product_type, rang, special_code, fk_unit) \
                         VALUES (:fk_facture, :label, NULL, :tva_tx, 1, 0, \
                          :subprice, :subprice, :total_ht, :total_tva, :total_ttc, \
                          0, :rang, 0, :unite)",
                        params! {
                            "fk_facture" => rowid,
                            "label" => libelle.as_str(),
                            "tva_tx" => tva_tx.to_string(),
                            "subprice" => montant.to_string(),
                            "total_ht" => t.total_ht.to_string(),
                            "total_tva" => t.total_vat.to_string(),
                            "total_ttc" => t.total_ttc.to_string(),
                            "rang" => rang as i32,
                            "unite" => unite,
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

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

fn is_deadlock(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1213 || s.code == 1205)
}
