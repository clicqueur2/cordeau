//! Chiffres de pilotage : ce qui se mesure d'un coup d'oeil.
//!
//! Une seule question ici : **combien ai-je facture chaque mois**. La serie est
//! celle des factures validees (statut >= 1), hors avoirs : un avoir n'est pas
//! du chiffre d'affaires, il l'annule.

use std::error::Error;

use mysql::prelude::*;
use mysql::params;
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

    let connu: std::collections::BTreeMap<String, f64> =
        rows.into_iter().map(|(m, v)| (m, v.unwrap_or(0.0))).collect();

    // Les douze derniers mois, trous compris.
    let debut: Option<String> = conn.query_first(
        "SELECT DATE_FORMAT(DATE_SUB(DATE_FORMAT(CURDATE(), '%Y-%m-01'), INTERVAL 11 MONTH), '%Y-%m')",
    )?;
    let Some(debut) = debut else {
        return Ok(Vec::new());
    };
    let (mut annee, mut m) = (
        debut[0..4].parse::<i32>()?,
        debut[5..7].parse::<u32>()?,
    );
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
