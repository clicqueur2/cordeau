//! Relances : factures echues et non reglees.
//!
//! On ne fait pas d'envoi ici : on produit la liste, les jours de retard et le
//! texte du courriel, pret a partir. L'envoi reste une action humaine tant que
//! le SMTP n'est pas branche.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Ligne brute : facture, reference, tiers, nom, courriel, echeance, jours de retard.
type LigneRelance = (
    i64,
    String,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Relance {
    pub fk_facture: i64,
    pub reference: String,
    pub fk_soc: Option<i64>,
    pub client: String,
    pub email: Option<String>,
    pub echeance: Option<String>,
    pub jours_retard: i64,
    pub total_ttc: Decimal,
    pub regle: Decimal,
    pub reste: Decimal,
}

/// Factures validees, non soldees et dont l'echeance est passee.
pub fn en_retard(pool: &Pool, entity: i64) -> Result<Vec<Relance>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<LigneRelance> = conn.exec(
        "SELECT f.rowid, f.ref, f.fk_soc, s.nom, s.email, \
              DATE_FORMAT(f.date_lim_reglement, '%Y-%m-%d'), \
              DATEDIFF(CURDATE(), f.date_lim_reglement) \
             FROM llx_facture f \
             LEFT JOIN llx_societe s ON s.rowid = f.fk_soc AND s.entity = f.entity \
             WHERE f.entity = :entity AND f.fk_statut IN (1, 2) AND f.paye = 0 \
               AND f.date_lim_reglement IS NOT NULL AND f.date_lim_reglement < CURDATE() \
             ORDER BY f.date_lim_reglement ASC, f.rowid ASC",
        params! { "entity" => entity },
    )?;

    let mut out = Vec::with_capacity(lignes.len());
    for (fk_facture, reference, fk_soc, nom, email, echeance, jours) in lignes {
        let solde = crate::payment::balance(pool, entity, fk_facture)?;
        out.push(Relance {
            fk_facture,
            reference,
            fk_soc,
            client: nom.unwrap_or_default(),
            email,
            echeance,
            jours_retard: jours,
            total_ttc: solde.total_ttc,
            regle: solde.regle,
            reste: solde.reste,
        });
    }
    Ok(out)
}

/// Corps du courriel de relance, en francais, sans jargon.
pub fn texte(r: &Relance) -> String {
    // Deux decimales, comme sur une facture : « 4109,60 » et non « 4109,6 ».
    let reste = format!("{:.2}", r.reste).replace('.', ",");
    match (r.echeance.as_deref(), r.jours_retard) {
        (Some(ech), j) if j > 0 => format!(
            "Bonjour,\n\n\
             Sauf erreur de notre part, la facture {} d'un montant de {} EUR TTC, \
             echeance du {}, demeure impayee ({} jours de retard).\n\n\
             Je vous serais reconnaissant de bien vouloir proceder au reglement, \
             ou de me faire savoir si un point bloque.\n\n\
             Bien cordialement,\n",
            r.reference, reste, ech, j
        ),
        _ => format!(
            "Bonjour,\n\n\
             Sauf erreur de notre part, la facture {} d'un montant de {} EUR TTC \
             demeure impayee.\n\n\
             Je vous serais reconnaissant de bien vouloir proceder au reglement, \
             ou de me faire savoir si un point bloque.\n\n\
             Bien cordialement,\n",
            r.reference, reste
        ),
    }
}

/// Montant total en retard.
pub fn total(relances: &[Relance]) -> Decimal {
    relances.iter().map(|r| r.reste).sum()
}

fn _dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_texte_d_une_relance_contient_la_reference_et_le_montant() {
        let r = Relance {
            reference: "FA2611-0001".into(),
            echeance: Some("2026-10-01".into()),
            jours_retard: 12,
            reste: Decimal::from_str("4109.60").unwrap(),
            ..Default::default()
        };
        let t = texte(&r);
        assert!(t.contains("FA2611-0001"));
        assert!(t.contains("4109,60"));
        assert!(t.contains("12 jours"));
    }
}
