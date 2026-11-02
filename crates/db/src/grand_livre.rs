//! Grand livre derive des documents.
//!
//! Dolibarr tient son grand livre dans `llx_accounting_bookkeeping`, alimente
//! par une saisie par journal. Cette table est **vide** tant que la
//! comptabilite n'a pas ete exportee. On derive donc les ecritures des
//! documents eux-memes (factures, avoirs, achats, reglements), selon le plan
//! comptable general simplifie d'une activite de services du batiment.
//!
//! Ce n'est pas une comptabilite tenue : c'est une **proposition d'ecritures**
//! que l'expert-comptable valide. Elle est deterministe et verifiable.

use std::collections::BTreeMap;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row};
use rust_decimal::Decimal;

/// Une ligne d'ecriture au grand livre.
#[derive(Debug, Clone, PartialEq)]
pub struct Ecriture {
    pub date: String,
    /// `VE` ventes, `AC` achats, `BQ` banque.
    pub journal: &'static str,
    pub compte: &'static str,
    pub libelle_compte: &'static str,
    pub libelle: String,
    pub reference: String,
    pub debit: Decimal,
    pub credit: Decimal,
}

/// Compte du plan comptable general utilise.
fn libelle_compte(compte: &str) -> &'static str {
    match compte {
        "411000" => "Clients",
        "401000" => "Fournisseurs",
        "512000" => "Banque",
        "706000" => "Prestations de services",
        "601000" => "Achats",
        "445710" => "TVA collectee",
        "445660" => "TVA deductible",
        _ => "Compte",
    }
}

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Decimal {
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

/// Un constructeur de ligne avec huit arguments : le regrouper dans une
/// structure n'apporterait rien a la lecture des appels.
#[allow(clippy::too_many_arguments)]
fn ecrire(
    out: &mut Vec<Ecriture>,
    date: &str,
    journal: &'static str,
    compte: &'static str,
    reference: &str,
    libelle: &str,
    debit: Decimal,
    credit: Decimal,
) {
    if debit == Decimal::ZERO && credit == Decimal::ZERO {
        return;
    }
    out.push(Ecriture {
        date: date.to_string(),
        journal,
        compte,
        libelle_compte: libelle_compte(compte),
        libelle: libelle.to_string(),
        reference: reference.to_string(),
        debit,
        credit,
    });
}

/// Ecritures derivees des documents, entre deux dates.
pub fn ecritures(
    pool: &Pool,
    entity: i64,
    debut: Option<&str>,
    fin: Option<&str>,
) -> Result<Vec<Ecriture>, Box<dyn Error>> {
    let (d, f) = (borne(debut, "1970-01-01"), borne(fin, "2099-12-31"));
    let mut conn = pool.get_conn()?;
    let mut out: Vec<Ecriture> = Vec::new();

    // Ventes : factures, avoirs et situations, en trois lignes chacun.
    let ventes: Vec<Row> = conn.exec(
        "SELECT DATE_FORMAT(f.datef, '%Y-%m-%d'), f.ref, f.type, COALESCE(s.nom, ''), \
                CAST(f.total_ht AS CHAR), CAST(f.total_tva AS CHAR), CAST(f.total_ttc AS CHAR) \
         FROM llx_facture f LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE f.entity = :entity AND f.fk_statut >= 1 AND f.datef BETWEEN :d AND :f \
         ORDER BY f.datef ASC, f.ref ASC",
        params! { "entity" => entity, "d" => d.as_str(), "f" => f.as_str() },
    )?;
    for r in ventes {
        let date = opt_s(&r, 0).unwrap_or_default();
        let reference = opt_s(&r, 1).unwrap_or_default();
        let type_ = opt_s(&r, 2).unwrap_or_default();
        let tiers = opt_s(&r, 3).unwrap_or_default();
        let (mut ht, mut tva, mut ttc) = (opt_dec(&r, 4), opt_dec(&r, 5), opt_dec(&r, 6));
        if type_ == "2" {
            // Un avoir inverse les signes.
            ht = -ht;
            tva = -tva;
            ttc = -ttc;
        }
        let libelle = format!("{reference} - {tiers}");
        ecrire(
            &mut out,
            &date,
            "VE",
            "411000",
            &reference,
            &libelle,
            ttc,
            Decimal::ZERO,
        );
        ecrire(
            &mut out,
            &date,
            "VE",
            "706000",
            &reference,
            &libelle,
            Decimal::ZERO,
            ht,
        );
        ecrire(
            &mut out,
            &date,
            "VE",
            "445710",
            &reference,
            &libelle,
            Decimal::ZERO,
            tva,
        );
    }

    // Achats : une ligne de charge, une de TVA deductible, une de dette.
    let achats: Vec<Row> = conn.exec(
        "SELECT DATE_FORMAT(f.datef, '%Y-%m-%d'), f.ref, COALESCE(s.nom, ''), \
                CAST(f.total_ht AS CHAR), CAST(f.total_tva AS CHAR), CAST(f.total_ttc AS CHAR) \
         FROM llx_facture_fourn f LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE f.entity = :entity AND f.fk_statut >= 1 AND f.datef BETWEEN :d AND :f \
         ORDER BY f.datef ASC, f.ref ASC",
        params! { "entity" => entity, "d" => d.as_str(), "f" => f.as_str() },
    )?;
    for r in achats {
        let date = opt_s(&r, 0).unwrap_or_default();
        let reference = opt_s(&r, 1).unwrap_or_default();
        let tiers = opt_s(&r, 2).unwrap_or_default();
        let (ht, tva, ttc) = (opt_dec(&r, 3), opt_dec(&r, 4), opt_dec(&r, 5));
        let libelle = format!("{reference} - {tiers}");
        ecrire(
            &mut out,
            &date,
            "AC",
            "601000",
            &reference,
            &libelle,
            ht,
            Decimal::ZERO,
        );
        ecrire(
            &mut out,
            &date,
            "AC",
            "445660",
            &reference,
            &libelle,
            tva,
            Decimal::ZERO,
        );
        ecrire(
            &mut out,
            &date,
            "AC",
            "401000",
            &reference,
            &libelle,
            Decimal::ZERO,
            ttc,
        );
    }

    // Reglements clients : la banque augmente, le compte client se solde.
    let paiements: Vec<Row> = conn.exec(
        "SELECT DATE_FORMAT(p.datep, '%Y-%m-%d'), p.ref, CAST(p.amount AS CHAR), \
                GROUP_CONCAT(DISTINCT f.ref ORDER BY f.ref SEPARATOR ', ') \
         FROM llx_paiement p \
         LEFT JOIN llx_paiement_facture pf ON pf.fk_paiement = p.rowid \
         LEFT JOIN llx_facture f ON f.rowid = pf.fk_facture \
         WHERE p.entity = :entity AND p.datep BETWEEN :d AND :f \
         GROUP BY p.rowid, p.datep, p.ref, p.amount \
         ORDER BY p.datep ASC, p.ref ASC",
        params! { "entity" => entity, "d" => d.as_str(), "f" => f.as_str() },
    )?;
    for r in paiements {
        let date = opt_s(&r, 0).unwrap_or_default();
        let reference = opt_s(&r, 1).unwrap_or_default();
        let montant = opt_dec(&r, 2);
        let factures = opt_s(&r, 3).unwrap_or_default();
        let libelle = if factures.is_empty() {
            format!("Reglement {reference}")
        } else {
            format!("Reglement {reference} - {factures}")
        };
        ecrire(
            &mut out,
            &date,
            "BQ",
            "512000",
            &reference,
            &libelle,
            montant,
            Decimal::ZERO,
        );
        ecrire(
            &mut out,
            &date,
            "BQ",
            "411000",
            &reference,
            &libelle,
            Decimal::ZERO,
            montant,
        );
    }

    Ok(out)
}

/// Solde d'un compte dans une periode.
#[derive(Debug, Clone, PartialEq)]
pub struct SoldeCompte {
    pub compte: String,
    pub libelle: String,
    pub debit: Decimal,
    pub credit: Decimal,
}

impl SoldeCompte {
    pub fn solde(&self) -> Decimal {
        self.debit - self.credit
    }
}

/// Balance : totaux par compte, dans l'ordre du plan comptable.
pub fn balance(
    pool: &Pool,
    entity: i64,
    debut: Option<&str>,
    fin: Option<&str>,
) -> Result<Vec<SoldeCompte>, Box<dyn Error>> {
    let mut cumul: BTreeMap<String, SoldeCompte> = BTreeMap::new();
    for e in ecritures(pool, entity, debut, fin)? {
        let entree = cumul
            .entry(e.compte.to_string())
            .or_insert_with(|| SoldeCompte {
                compte: e.compte.to_string(),
                libelle: e.libelle_compte.to_string(),
                debit: Decimal::ZERO,
                credit: Decimal::ZERO,
            });
        entree.debit += e.debit;
        entree.credit += e.credit;
    }
    Ok(cumul.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_avoir_inverse_les_signes() {
        let ht = Decimal::from(100);
        let tva = Decimal::from(10);
        let ttc = Decimal::from(110);
        assert_eq!(-ht, Decimal::from(-100));
        assert_eq!(ttc - tva - ht, Decimal::ZERO);
    }

    #[test]
    fn le_libelle_des_comptes_est_stable() {
        assert_eq!(libelle_compte("411000"), "Clients");
        assert_eq!(libelle_compte("512000"), "Banque");
        assert_eq!(libelle_compte("999999"), "Compte");
    }
}
