//! Vue comptable : ventes, achats et TVA, mois par mois.
//!
//! Ce module ne tient pas de grand livre : Dolibarr le fait dans
//! `llx_accounting_bookkeeping`, qui demande une saisie par journal. Ce qui est
//! calcule ici est ce qu'un artisan regarde tous les mois : le chiffre
//! d'affaires, la TVA collectee, la TVA deductible et le solde a payer.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Une ligne de synthese mensuelle.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mois {
    /// `AAAA-MM`.
    pub periode: String,
    pub total_ht: Decimal,
    pub total_tva: Decimal,
    pub total_ttc: Decimal,
    /// Nombre de documents pris en compte.
    pub documents: i64,
}

fn lire(row: &mysql::Row, i: usize) -> Decimal {
    row.get_opt::<String, _>(i)
        .and_then(|r| r.ok())
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .map(money::round_money)
        .unwrap_or_default()
}

/// Ventes par mois : factures validees, avoirs deduits.
///
/// Les brouillons et les factures abandonnees sont exclus : ils ne sont pas
/// encore du chiffre d'affaires.
pub fn ventes_par_mois(pool: &Pool, entity: i64, limite: u32) -> Result<Vec<Mois>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT DATE_FORMAT(datef, '%Y-%m'), \
                CAST(SUM(CASE WHEN type = 2 THEN -total_ht ELSE total_ht END) AS CHAR), \
                CAST(SUM(CASE WHEN type = 2 THEN -total_tva ELSE total_tva END) AS CHAR), \
                CAST(SUM(CASE WHEN type = 2 THEN -total_ttc ELSE total_ttc END) AS CHAR), \
                COUNT(*) \
         FROM llx_facture \
         WHERE entity = :entity AND fk_statut >= 1 AND type <> 3 \
         GROUP BY DATE_FORMAT(datef, '%Y-%m') \
         ORDER BY 1 DESC LIMIT :limite",
        params! { "entity" => entity, "limite" => limite },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Mois {
            periode: r
                .get_opt::<String, _>(0)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            total_ht: lire(&r, 1),
            total_tva: lire(&r, 2),
            total_ttc: lire(&r, 3),
            documents: r.get_opt::<i64, _>(4).and_then(|v| v.ok()).unwrap_or(0),
        })
        .collect())
}

/// Achats par mois : factures fournisseurs validees.
pub fn achats_par_mois(pool: &Pool, entity: i64, limite: u32) -> Result<Vec<Mois>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT DATE_FORMAT(datef, '%Y-%m'), CAST(SUM(total_ht) AS CHAR), \
                CAST(SUM(total_tva) AS CHAR), CAST(SUM(total_ttc) AS CHAR), COUNT(*) \
         FROM llx_facture_fourn \
         WHERE entity = :entity AND fk_statut >= 1 \
         GROUP BY DATE_FORMAT(datef, '%Y-%m') \
         ORDER BY 1 DESC LIMIT :limite",
        params! { "entity" => entity, "limite" => limite },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Mois {
            periode: r
                .get_opt::<String, _>(0)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            total_ht: lire(&r, 1),
            total_tva: lire(&r, 2),
            total_ttc: lire(&r, 3),
            documents: r.get_opt::<i64, _>(4).and_then(|v| v.ok()).unwrap_or(0),
        })
        .collect())
}

/// Solde de TVA : collectee (ventes) moins deductible (achats).
///
/// Positif = TVA a payer. Ce n'est pas une declaration : les taux, les
/// autoliquidations et les encaissements doivent etre verifies avant depot.
pub fn solde_tva(pool: &Pool, entity: i64) -> Result<(Decimal, Decimal, Decimal), Box<dyn Error>> {
    let ventes = ventes_par_mois(pool, entity, 10_000)?;
    let achats = achats_par_mois(pool, entity, 10_000)?;
    let collectee: Decimal = ventes.iter().map(|m| m.total_tva).sum();
    let deductible: Decimal = achats.iter().map(|m| m.total_tva).sum();
    Ok((
        money::round_money(collectee),
        money::round_money(deductible),
        money::round_money(collectee - deductible),
    ))
}

/// Une ligne de journal comptable, deja formatee pour un tableur.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LigneJournal {
    pub date: String,
    pub reference: String,
    pub type_document: String,
    pub tiers: String,
    pub total_ht: Decimal,
    pub total_tva: Decimal,
    pub total_ttc: Decimal,
    pub complement: String,
}

fn opt_s(row: &mysql::Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &mysql::Row, i: usize) -> Decimal {
    opt_s(row, i)
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .map(money::round_money)
        .unwrap_or_default()
}

fn borne(date: Option<&str>, defaut: &str) -> String {
    date.map(str::trim)
        .filter(|d| d.len() >= 10)
        .map(|d| d[0..10].to_string())
        .unwrap_or_else(|| defaut.to_string())
}

/// Journal des ventes : factures et avoirs valides, entre deux dates.
pub fn journal_ventes(
    pool: &Pool,
    entity: i64,
    debut: Option<&str>,
    fin: Option<&str>,
) -> Result<Vec<LigneJournal>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT DATE_FORMAT(f.datef, '%Y-%m-%d'), f.ref, f.type, COALESCE(s.nom, ''), \
                CAST(SUM(d.total_ht) AS CHAR), CAST(SUM(d.total_tva) AS CHAR), \
                CAST(SUM(d.total_ttc) AS CHAR) \
         FROM llx_facture f \
         LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         LEFT JOIN llx_facturedet d ON d.fk_facture = f.rowid AND d.product_type <> 9 \
         WHERE f.entity = :entity AND f.fk_statut >= 1 \
           AND f.datef BETWEEN :debut AND :fin \
         GROUP BY f.rowid, f.datef, f.ref, f.type, s.nom \
         ORDER BY f.datef ASC, f.ref ASC",
        params! {
            "entity" => entity,
            "debut" => borne(debut, "1970-01-01"),
            "fin" => borne(fin, "2099-12-31"),
        },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| LigneJournal {
            date: opt_s(&r, 0).unwrap_or_default(),
            reference: opt_s(&r, 1).unwrap_or_default(),
            type_document: match opt_s(&r, 2).as_deref() {
                Some("2") => "Avoir".to_string(),
                Some("3") => "Acompte".to_string(),
                Some("5") => "Situation".to_string(),
                _ => "Facture".to_string(),
            },
            tiers: opt_s(&r, 3).unwrap_or_default(),
            total_ht: opt_dec(&r, 4),
            total_tva: opt_dec(&r, 5),
            total_ttc: opt_dec(&r, 6),
            complement: String::new(),
        })
        .collect())
}

/// Journal des achats : factures fournisseurs validees.
pub fn journal_achats(
    pool: &Pool,
    entity: i64,
    debut: Option<&str>,
    fin: Option<&str>,
) -> Result<Vec<LigneJournal>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT DATE_FORMAT(f.datef, '%Y-%m-%d'), f.ref, COALESCE(f.ref_supplier, ''), \
                COALESCE(s.nom, ''), CAST(f.total_ht AS CHAR), CAST(f.total_tva AS CHAR), \
                CAST(f.total_ttc AS CHAR) \
         FROM llx_facture_fourn f LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE f.entity = :entity AND f.fk_statut >= 1 \
           AND f.datef BETWEEN :debut AND :fin \
         ORDER BY f.datef ASC, f.ref ASC",
        params! {
            "entity" => entity,
            "debut" => borne(debut, "1970-01-01"),
            "fin" => borne(fin, "2099-12-31"),
        },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| LigneJournal {
            date: opt_s(&r, 0).unwrap_or_default(),
            reference: opt_s(&r, 1).unwrap_or_default(),
            type_document: "Facture fournisseur".to_string(),
            tiers: opt_s(&r, 3).unwrap_or_default(),
            total_ht: opt_dec(&r, 4),
            total_tva: opt_dec(&r, 5),
            total_ttc: opt_dec(&r, 6),
            complement: opt_s(&r, 2).unwrap_or_default(),
        })
        .collect())
}

/// Une ligne de TVA : mois, sens, taux, base HT, montant.
pub type LigneTva = (String, String, String, Decimal, Decimal);

/// TVA par mois et par taux, ventes et achats separes.
pub fn tva_par_taux(
    pool: &Pool,
    entity: i64,
    debut: Option<&str>,
    fin: Option<&str>,
) -> Result<Vec<LigneTva>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT DATE_FORMAT(f.datef, '%Y-%m'), 'Vente', CAST(d.tva_tx AS CHAR), \
                CAST(SUM(d.total_ht) AS CHAR), CAST(SUM(d.total_tva) AS CHAR) \
         FROM llx_facture f JOIN llx_facturedet d ON d.fk_facture = f.rowid \
         WHERE f.entity = :entity AND f.fk_statut >= 1 AND d.product_type <> 9 \
           AND f.datef BETWEEN :debut AND :fin \
         GROUP BY 1, 2, 3 \
         UNION ALL \
         SELECT DATE_FORMAT(f.datef, '%Y-%m'), 'Achat', CAST(d.tva_tx AS CHAR), \
                CAST(SUM(d.total_ht) AS CHAR), CAST(SUM(d.tva) AS CHAR) \
         FROM llx_facture_fourn f JOIN llx_facture_fourn_det d ON d.fk_facture_fourn = f.rowid \
         WHERE f.entity = :entity AND f.fk_statut >= 1 \
           AND f.datef BETWEEN :debut AND :fin \
         GROUP BY 1, 2, 3 \
         ORDER BY 1 ASC, 2 ASC, 3 ASC",
        params! {
            "entity" => entity,
            "debut" => borne(debut, "1970-01-01"),
            "fin" => borne(fin, "2099-12-31"),
        },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                opt_s(&r, 0).unwrap_or_default(),
                opt_s(&r, 1).unwrap_or_default(),
                opt_s(&r, 2).unwrap_or_default(),
                opt_dec(&r, 3),
                opt_dec(&r, 4),
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_solde_est_collectee_moins_deductible() {
        let a = Decimal::from_str("120.5").unwrap();
        let b = Decimal::from_str("20.25").unwrap();
        assert_eq!(
            money::round_money(a - b),
            Decimal::from_str("100.25").unwrap()
        );
    }
}
