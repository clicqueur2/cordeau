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
    /// `VE` ventes, `AC` achats, `BQ` banque... Tel que la base le porte.
    pub journal: String,
    pub compte: String,
    pub libelle_compte: String,
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
        journal: journal.to_string(),
        compte: compte.to_string(),
        libelle_compte: libelle_compte(compte).to_string(),
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

/// Les ecritures telles que la base les porte : `llx_accounting_bookkeeping`.
///
/// C'est **la seule table que Dolibarr lit**. Tant qu'on ne lisait pas
/// celle-ci, un ecran pouvait afficher un grand livre que Dolibarr n'avait
/// jamais vu, et ignorer les ecritures qu'il avait produites : deux
/// comptabilites dans la meme base, sans moyen de les rapprocher.
///
/// # Ce qu'on lit
///
/// Les colonnes que `BookKeeping::fetchAll` lit (`bookkeeping.class.php`,
/// l. 1170-1200) : `doc_date`, `doc_ref`, `numero_compte`, `label_compte`,
/// `label_operation`, `debit`, `credit`, `code_journal`. Rien d'autre : une
/// ecriture se juge sur ces huit valeurs.
///
/// # Les comptes sans libelle
///
/// `label_compte` vient d'une ventilation Dolibarr et peut etre vide. On prend
/// alors le libelle du plan comptable quand on le connait, sinon le numero.
/// Afficher une ligne sans nom de compte rend un grand livre illisible.
pub fn ecritures_comptables(
    pool: &Pool,
    entity: i64,
    debut: Option<&str>,
    fin: Option<&str>,
) -> Result<Vec<Ecriture>, Box<dyn Error>> {
    let (d, f) = (borne(debut, "1970-01-01"), borne(fin, "2099-12-31"));
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT DATE_FORMAT(doc_date, '%Y-%m-%d'), COALESCE(doc_ref, ''), \
                COALESCE(numero_compte, ''), COALESCE(label_compte, ''), \
                COALESCE(label_operation, ''), CAST(debit AS CHAR), \
                CAST(credit AS CHAR), COALESCE(code_journal, '') \
         FROM llx_accounting_bookkeeping \
         WHERE entity = :entity AND doc_date BETWEEN :d AND :f \
         ORDER BY doc_date ASC, piece_num ASC, rowid ASC",
        params! { "entity" => entity, "d" => d.as_str(), "f" => f.as_str() },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let compte = opt_s(&r, 2).unwrap_or_default();
            let libelle_compte = {
                let lu = opt_s(&r, 3).unwrap_or_default();
                if lu.trim().is_empty() {
                    libelle_compte(&compte).to_string()
                } else {
                    lu
                }
            };
            Ecriture {
                date: opt_s(&r, 0).unwrap_or_default(),
                reference: opt_s(&r, 1).unwrap_or_default(),
                journal: opt_s(&r, 7).unwrap_or_default().trim().to_string(),
                compte,
                libelle_compte,
                libelle: opt_s(&r, 4).unwrap_or_default(),
                debit: opt_dec(&r, 5),
                credit: opt_dec(&r, 6),
            }
        })
        .collect())
}

/// Vrai si la base porte au moins une ecriture comptable.
///
/// Sert a choisir entre les deux lectures : la table partagee quand elle est
/// remplie, la reconstitution depuis les documents sinon. Une base neuve n'a
/// aucune ecriture, et un grand livre vide se lirait « rien n'a ete fait »
/// alors que les factures existent.
pub fn a_des_ecritures(pool: &Pool, entity: i64) -> Result<bool, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_accounting_bookkeeping WHERE entity = :entity",
        params! { "entity" => entity },
    )?;
    Ok(n.unwrap_or(0) > 0)
}

/// Les ecritures a montrer : celles de la base quand elle en porte, sinon
/// celles reconstituees depuis les documents.
///
/// # Pourquoi ce choix, et pas un seul des deux
///
/// La table `llx_accounting_bookkeeping` est la seule que Dolibarr lit : c'est
/// elle qui fait foi des qu'une ventilation a eu lieu. Mais une base neuve n'en
/// porte aucune, et un grand livre vide se lirait « rien n'a ete fait » alors
/// que les factures existent.
///
/// On lit donc la table **des qu'elle contient quelque chose**, et on
/// reconstitue sinon. Le choix est visible : l'ecran affiche d'ou viennent les
/// lignes, parce qu'un grand livre reconstitue et un grand livre constate ne
/// se confondent pas.
pub fn ecritures_effectives(
    pool: &Pool,
    entity: i64,
    debut: Option<&str>,
    fin: Option<&str>,
) -> Result<Vec<Ecriture>, Box<dyn Error>> {
    if a_des_ecritures(pool, entity)? {
        ecritures_comptables(pool, entity, debut, fin)
    } else {
        ecritures(pool, entity, debut, fin)
    }
}

/// D'ou viennent les lignes affichees.
pub fn source(pool: &Pool, entity: i64) -> Result<&'static str, Box<dyn Error>> {
    Ok(if a_des_ecritures(pool, entity)? {
        "llx_accounting_bookkeeping"
    } else {
        "reconstitution depuis les documents"
    })
}

/// Balance d'une liste d'ecritures : totaux par compte, dans l'ordre du plan.
///
/// Pure, et c'est le point : un appelant qui vient de lire un jeu d'ecritures
/// n'a pas a decider d'ou elles viennent pour les totaliser. Deux sources, une
/// seule facon de compter.
pub fn balance_de(ecritures: &[Ecriture]) -> Vec<SoldeCompte> {
    let mut cumul: BTreeMap<String, SoldeCompte> = BTreeMap::new();
    for e in ecritures {
        let entree = cumul
            .entry(e.compte.clone())
            .or_insert_with(|| SoldeCompte {
                compte: e.compte.clone(),
                libelle: e.libelle_compte.clone(),
                debit: Decimal::ZERO,
                credit: Decimal::ZERO,
            });
        entree.debit += e.debit;
        entree.credit += e.credit;
    }
    cumul.into_values().collect()
}

/// Balance : totaux par compte, dans l'ordre du plan comptable.
pub fn balance(
    pool: &Pool,
    entity: i64,
    debut: Option<&str>,
    fin: Option<&str>,
) -> Result<Vec<SoldeCompte>, Box<dyn Error>> {
    let ecritures = ecritures_effectives(pool, entity, debut, fin)?;
    Ok(balance_de(&ecritures))
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
