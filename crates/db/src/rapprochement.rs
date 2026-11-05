//! Rapprochement bancaire : pointer ce qui est passe en banque.
//!
//! Le geste est celui du tresorier en fin de mois : on a un releve, on coche ce
//! qui y figure. Ici, ce sont les **encaissements dont la ligne bancaire n'est
//! pas encore pointee** (`llx_bank.rappro = 0`), et on les marque d'un releve.
//!
//! Ce module ne cree rien : le paiement et sa ligne de banque naissent ensemble
//! dans `payment::create`. Inventer une deuxieme ligne pour le meme mouvement
//! ferait un doublon dans le journal de banque — c'est l'erreur classique du
//! rapprochement, et elle se paie en ecart de tresorerie.
//!
//! On renseigne aussi `num_releve` : sans lui, on ne sait plus, six mois apres,
//! avec quel releve un encaissement a ete pointe.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Un encaissement en attente de pointage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encaissement {
    /// `llx_bank.rowid`.
    pub ligne: i64,
    pub date: String,
    pub libelle: String,
    pub montant: Decimal,
    /// Reference de la facture reglee, quand le lien existe.
    pub facture: Option<String>,
    /// Tiers de la facture.
    pub tiers: Option<String>,
    /// Compte bancaire.
    pub compte: i64,
}

/// Une ligne brute : `(ligne, date, libelle, montant, facture, tiers, compte)`.
type LigneBrute = (
    i64,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<i64>,
);

/// Les encaissements dont la ligne de banque n'est pas pointee.
pub fn a_pointer(pool: &Pool, entity: i64) -> Result<Vec<Encaissement>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<LigneBrute> = conn.exec(
        "SELECT b.rowid, DATE_FORMAT(b.datev, '%Y-%m-%d'), b.label, \
                CAST(b.amount AS CHAR), f.ref, s.nom, b.fk_account \
         FROM llx_bank b \
         LEFT JOIN llx_bank_url u ON u.fk_bank = b.rowid AND u.type = 'payment' \
         LEFT JOIN llx_paiement p ON p.rowid = u.url_id \
         LEFT JOIN llx_paiement_facture pf ON pf.fk_paiement = p.rowid \
         LEFT JOIN llx_facture f ON f.rowid = pf.fk_facture \
         LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE b.fk_account IN (SELECT rowid FROM llx_bank_account WHERE entity = :entity) \
           AND COALESCE(b.rappro, 0) = 0 \
         ORDER BY b.datev DESC, b.rowid DESC",
        params! { "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .map(
            |(ligne, date, libelle, montant, facture, tiers, compte)| Encaissement {
                ligne,
                date: date.unwrap_or_default(),
                libelle: libelle.unwrap_or_default(),
                montant: Decimal::from_str(montant.trim()).unwrap_or_default(),
                facture,
                tiers,
                compte: compte.unwrap_or(0),
            },
        )
        .collect())
}

/// Pointe une ligne de banque et retient le releve qui la porte.
///
/// Refuse une ligne d'une autre entite : le rapprochement ne traverse pas les
/// societes.
pub fn pointer(pool: &Pool, entity: i64, ligne: i64, releve: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let appartient: Option<i64> = conn.exec_first(
        "SELECT b.rowid FROM llx_bank b \
         JOIN llx_bank_account a ON a.rowid = b.fk_account \
         WHERE b.rowid = :id AND a.entity = :entity",
        params! { "id" => ligne, "entity" => entity },
    )?;
    if appartient.is_none() {
        return Err(format!("ligne de banque {ligne} introuvable dans cette entité").into());
    }

    let releve = releve.trim();
    conn.exec_drop(
        "UPDATE llx_bank SET rappro = 1, num_releve = :releve WHERE rowid = :id",
        params! {
            "id" => ligne,
            "releve" => if releve.is_empty() { None } else { Some(releve) },
        },
    )?;
    Ok(())
}

/// Le nombre de lignes pointees, et le total, pour l'entete de l'ecran.
pub fn total(rows: &[Encaissement]) -> Decimal {
    rows.iter().map(|e| e.montant).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(montant: &str) -> Encaissement {
        Encaissement {
            ligne: 1,
            date: "2026-11-01".into(),
            libelle: "VIR".into(),
            montant: Decimal::from_str(montant).unwrap(),
            facture: None,
            tiers: None,
            compte: 1,
        }
    }

    #[test]
    fn le_total_additionne_les_montants() {
        assert_eq!(
            total(&[e("100"), e("20.50")]),
            Decimal::from_str("120.50").unwrap()
        );
        assert_eq!(total(&[]), Decimal::ZERO);
    }
}
