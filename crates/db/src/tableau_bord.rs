//! Chiffres de pilotage : ce qui se mesure d'un coup d'oeil.
//!
//! Une seule question ici : **combien ai-je facture chaque mois**. La serie est
//! celle des factures validees (statut >= 1), hors avoirs : un avoir n'est pas
//! du chiffre d'affaires, il l'annule.

use std::error::Error;

use mysql::params;
use mysql::prelude::*;
use mysql::Pool;

/// Chiffre d'affaires hors taxes par mois, du plus ancien au plus recent.
///
/// Rend `(AAAA-MM, montant HT)`. Les mois sans facture sont rendus a zero : une
/// courbe qui saute les mois creux ment sur la regularite de l'activite.
pub fn ca_mensuel(
    pool: &Pool,
    entity: i64,
    mois: u32,
) -> Result<Vec<(String, f64)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(String, Option<f64>)> = conn.exec(
        "SELECT DATE_FORMAT(datef, '%Y-%m'), SUM(total_ht) \
         FROM llx_facture \
         WHERE entity = :entity AND fk_statut >= 1 AND type <> 2 \
           AND datef >= DATE_SUB(DATE_FORMAT(CURDATE(), '%Y-%m-01'), INTERVAL :mois MONTH) \
         GROUP BY DATE_FORMAT(datef, '%Y-%m') \
         ORDER BY 1 ASC",
        params! { "entity" => entity, "mois" => mois.saturating_sub(1) },
    )?;

    let connu: std::collections::BTreeMap<String, f64> = rows
        .into_iter()
        .map(|(m, v)| (m, v.unwrap_or(0.0)))
        .collect();

    // Les douze derniers mois, trous compris.
    let debut: Option<String> = conn.query_first(
        "SELECT DATE_FORMAT(DATE_SUB(DATE_FORMAT(CURDATE(), '%Y-%m-01'), INTERVAL 11 MONTH), '%Y-%m')",
    )?;
    let Some(debut) = debut else {
        return Ok(Vec::new());
    };
    let (mut annee, mut m) = (debut[0..4].parse::<i32>()?, debut[5..7].parse::<u32>()?);
    let mut serie = Vec::with_capacity(mois as usize);
    for _ in 0..mois {
        let cle = format!("{annee:04}-{m:02}");
        serie.push((cle.clone(), *connu.get(&cle).unwrap_or(&0.0)));
        m += 1;
        if m > 12 {
            m = 1;
            annee += 1;
        }
    }
    Ok(serie)
}

/// Delai moyen entre la date de facture et la date d'encaissement, en jours.
///
/// Rend 0.0 s'il n'y a aucun reglement rattache : mieux vaut zero affiche que
/// « NaN » sur un tableau de bord.
pub fn delai_moyen(pool: &Pool, entity: i64) -> Result<f64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // `AVG()` sur zero ligne rend **NULL**, et `NULL` ne se decode pas en `f64` :
    // le pilote leve avant qu'on puisse voir l'`Option`, et le fil de travail
    // meurt — c'est ce qui arrivait sur une base neuve, ou la page d'accueil
    // tombait en 000. On decode donc `Option<f64>` **dans** la ligne.
    let jours: Option<Option<f64>> = conn.exec_first(
        "SELECT AVG(DATEDIFF(p.datep, f.datef)) \
         FROM llx_paiement_facture pf \
         JOIN llx_paiement p ON p.rowid = pf.fk_paiement \
         JOIN llx_facture f ON f.rowid = pf.fk_facture \
         WHERE f.entity = :entity AND p.datep IS NOT NULL AND f.datef IS NOT NULL",
        params! { "entity" => entity },
    )?;
    Ok(jours.flatten().unwrap_or(0.0))
}

/// Les cinq premiers clients par chiffre d'affaires facture (HT, hors avoirs).
///
/// Le nom vient du tiers ; un tiers supprime laisse la ligne sans nom plutot que
/// de disparaitre, sinon le total des barres ne ferait plus le total facture.
pub fn top_clients(pool: &Pool, entity: i64, n: u32) -> Result<Vec<(String, f64)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(Option<String>, Option<f64>)> = conn.exec(
        "SELECT s.nom, SUM(f.total_ht) \
         FROM llx_facture f LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE f.entity = :entity AND f.fk_statut >= 1 AND f.type <> 2 \
         GROUP BY f.fk_soc, s.nom \
         ORDER BY 2 DESC LIMIT :n",
        params! { "entity" => entity, "n" => n },
    )?;
    Ok(rows
        .into_iter()
        .map(|(nom, ca)| {
            (
                nom.unwrap_or_else(|| "(client supprime)".to_string()),
                ca.unwrap_or(0.0),
            )
        })
        .collect())
}
