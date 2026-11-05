//! Statistiques et echeancier : ce que les documents racontent ensemble.
//!
//! Deux ecrans qui manquaient : « combien j'ai vendu, a qui, sur quel
//! chantier » et « qu'est-ce qui rentre et sort dans les trois mois ». Les
//! donnees existent deja dans les pieces ; il ne manquait que de les agreger.
//!
//! Regles de calcul, ecrites une fois :
//!
//! - une vente compte quand la facture est **validee** (statut >= 1) et non
//!   abandonnee ; une facture en brouillon n'est pas un chiffre d'affaires ;
//! - les avoirs sont deduits : le chiffre d'affaires est net ;
//! - l'encours est le reste du, somme des `total_ttc - paye` ;
//! - les mois sans vente sortent a zero plutot que d'etre omis : une courbe a
//!   trous se lit mal.

use std::collections::BTreeMap;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Une valeur par mois, dans l'ordre chronologique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mois {
    /// `AAAA-MM`.
    pub mois: String,
    pub ventes: Decimal,
    pub achats: Decimal,
}

/// Un client et ce qu'il a achete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneClassement {
    pub libelle: String,
    pub montant: Decimal,
    pub nombre: i64,
}

/// Les encaissements et decaissements attendus d'un mois.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Echeance {
    pub mois: String,
    /// Ce que les clients doivent encore payer.
    pub encaissements: Decimal,
    /// Ce qu'il reste a payer aux fournisseurs.
    pub decaissements: Decimal,
}

impl Echeance {
    pub fn solde(&self) -> Decimal {
        self.encaissements - self.decaissements
    }
}

fn dec(v: Option<String>) -> Decimal {
    v.as_deref()
        .map(str::trim)
        .and_then(|s| Decimal::from_str(s).ok())
        .unwrap_or_default()
}

/// Les `n` derniers mois, du plus ancien au plus recent, a partir d'aujourd'hui.
fn mois_glissants(aujourd_hui: &str, n: usize) -> Vec<String> {
    let mut annee: i32 = aujourd_hui
        .get(0..4)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2026);
    let mut mois: i32 = aujourd_hui
        .get(5..7)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(format!("{annee:04}-{mois:02}"));
        mois -= 1;
        if mois == 0 {
            mois = 12;
            annee -= 1;
        }
    }
    out.reverse();
    out
}

/// Ventes et achats par mois sur les `n` derniers mois.
///
/// Les avoirs (type 2 sur une facture, statut >= 1) sont retires des ventes : le
/// chiffre d'affaires est net, pas brut.
pub fn par_mois(
    pool: &Pool,
    entity: i64,
    aujourd_hui: &str,
    n: usize,
) -> Result<Vec<Mois>, Box<dyn Error>> {
    let mois = mois_glissants(aujourd_hui, n);
    let debut = format!("{}-01", mois.first().cloned().unwrap_or_default());

    let mut ventes: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut achats: BTreeMap<String, Decimal> = BTreeMap::new();

    let mut conn = pool.get_conn()?;
    let v: Vec<(String, Option<String>)> = conn.exec(
        "SELECT DATE_FORMAT(datef, '%Y-%m'), CAST(SUM(total_ttc) AS CHAR) \
         FROM llx_facture \
         WHERE entity = :entity AND fk_statut >= 1 AND datef >= :debut \
           AND (type IS NULL OR type <> 2) \
         GROUP BY DATE_FORMAT(datef, '%Y-%m')",
        params! { "entity" => entity, "debut" => debut.as_str() },
    )?;
    for (m, s) in v {
        ventes.insert(m, dec(s));
    }

    let a: Vec<(String, Option<String>)> = conn.exec(
        "SELECT DATE_FORMAT(datef, '%Y-%m'), CAST(SUM(total_ttc) AS CHAR) \
         FROM llx_facture \
         WHERE entity = :entity AND fk_statut >= 1 AND datef >= :debut AND type = 2 \
         GROUP BY DATE_FORMAT(datef, '%Y-%m')",
        params! { "entity" => entity, "debut" => debut.as_str() },
    )?;
    for (m, s) in a {
        // Un avoir se deduit : il porte le meme signe dans la base, c'est nous
        // qui le retirons.
        let entree = ventes.entry(m).or_insert(Decimal::ZERO);
        *entree -= dec(s);
    }

    let f: Vec<(String, Option<String>)> = conn.exec(
        "SELECT DATE_FORMAT(datef, '%Y-%m'), CAST(SUM(total_ttc) AS CHAR) \
         FROM llx_facture_fourn \
         WHERE entity = :entity AND fk_statut >= 1 AND datef >= :debut \
         GROUP BY DATE_FORMAT(datef, '%Y-%m')",
        params! { "entity" => entity, "debut" => debut.as_str() },
    )?;
    for (m, s) in f {
        achats.insert(m, dec(s));
    }

    Ok(mois
        .into_iter()
        .map(|m| Mois {
            ventes: ventes.get(&m).copied().unwrap_or_default(),
            achats: achats.get(&m).copied().unwrap_or_default(),
            mois: m,
        })
        .collect())
}

/// Les `n` meilleurs clients par chiffre d'affaires.
pub fn top_clients(
    pool: &Pool,
    entity: i64,
    depuis: &str,
    n: u32,
) -> Result<Vec<LigneClassement>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(Option<String>, Option<String>, Option<i64>)> = conn.exec(
        "SELECT s.nom, CAST(SUM(f.total_ttc) AS CHAR), COUNT(*) \
         FROM llx_facture f \
         JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE f.entity = :entity AND f.fk_statut >= 1 AND f.datef >= :depuis \
           AND (f.type IS NULL OR f.type <> 2) \
         GROUP BY s.rowid, s.nom \
         ORDER BY SUM(f.total_ttc) DESC \
         LIMIT :n",
        params! { "entity" => entity, "depuis" => depuis, "n" => n.clamp(1, 50) },
    )?;

    Ok(rows
        .into_iter()
        .map(|(nom, montant, nb)| LigneClassement {
            libelle: nom.unwrap_or_default(),
            montant: dec(montant),
            nombre: nb.unwrap_or(0),
        })
        .collect())
}

/// Les `n` meilleurs chantiers, par chiffre d'affaires facture.
pub fn top_chantiers(
    pool: &Pool,
    entity: i64,
    depuis: &str,
    n: u32,
) -> Result<Vec<LigneClassement>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(Option<String>, Option<String>, Option<i64>)> = conn.exec(
        "SELECT p.title, CAST(SUM(f.total_ttc) AS CHAR), COUNT(*) \
         FROM llx_facture f \
         JOIN llx_projet p ON p.rowid = f.fk_projet \
         WHERE f.entity = :entity AND f.fk_statut >= 1 AND f.datef >= :depuis \
           AND (f.type IS NULL OR f.type <> 2) \
         GROUP BY p.rowid, p.title \
         ORDER BY SUM(f.total_ttc) DESC \
         LIMIT :n",
        params! { "entity" => entity, "depuis" => depuis, "n" => n.clamp(1, 50) },
    )?;

    Ok(rows
        .into_iter()
        .map(|(titre, montant, nb)| LigneClassement {
            libelle: titre.unwrap_or_else(|| "(sans titre)".into()),
            montant: dec(montant),
            nombre: nb.unwrap_or(0),
        })
        .collect())
}

/// Ce qui doit rentrer et sortir, mois par mois, sur les `n` prochains mois.
///
/// Un encaissement est compte a l'**echeance**, pas a la date de facture : c'est
/// la date a laquelle l'argent arrive.
pub fn echeancier(
    pool: &Pool,
    entity: i64,
    aujourd_hui: &str,
    n: usize,
) -> Result<Vec<Echeance>, Box<dyn Error>> {
    let mut mois = mois_glissants(aujourd_hui, 1);
    // On part du mois courant et on avance.
    let depart = mois.pop().unwrap_or_default();
    let mut liste = Vec::with_capacity(n);
    let mut annee: i32 = depart
        .get(0..4)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2026);
    let mut m: i32 = depart.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(1);
    for _ in 0..n {
        liste.push(format!("{annee:04}-{m:02}"));
        m += 1;
        if m == 13 {
            m = 1;
            annee += 1;
        }
    }

    let mut conn = pool.get_conn()?;
    let entrees: Vec<(String, Option<String>)> = conn.exec(
        "SELECT DATE_FORMAT(COALESCE(date_lim_reglement, datef), '%Y-%m'), \
                CAST(SUM(ttc_a_payer) AS CHAR) \
         FROM ( \
           SELECT f.date_lim_reglement, f.datef, \
                  f.total_ttc - COALESCE(( \
                    SELECT SUM(pf.amount) FROM llx_paiement_facture pf \
                    WHERE pf.fk_facture = f.rowid), 0) AS ttc_a_payer \
           FROM llx_facture f \
           WHERE f.entity = :entity AND f.fk_statut >= 1 AND f.paye = 0 \
         ) x \
         GROUP BY DATE_FORMAT(COALESCE(date_lim_reglement, datef), '%Y-%m')",
        params! { "entity" => entity },
    )?;
    let sorties: Vec<(String, Option<String>)> = conn.exec(
        "SELECT DATE_FORMAT(COALESCE(date_lim_reglement, datef), '%Y-%m'), \
                CAST(SUM(total_ttc) AS CHAR) \
         FROM llx_facture_fourn \
         WHERE entity = :entity AND fk_statut >= 1 AND paye = 0 \
         GROUP BY DATE_FORMAT(COALESCE(date_lim_reglement, datef), '%Y-%m')",
        params! { "entity" => entity },
    )?;

    let enc: BTreeMap<String, Decimal> = entrees.into_iter().map(|(m, v)| (m, dec(v))).collect();
    let dec_: BTreeMap<String, Decimal> = sorties.into_iter().map(|(m, v)| (m, dec(v))).collect();

    Ok(liste
        .into_iter()
        .map(|m| Echeance {
            encaissements: enc.get(&m).copied().unwrap_or_default(),
            decaissements: dec_.get(&m).copied().unwrap_or_default(),
            mois: m,
        })
        .collect())
}

/// L'encours client total : ce qu'on doit encore encaisser.
pub fn encours_client(pool: &Pool, entity: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let v: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(f.total_ttc - COALESCE(pe.regle, 0)), 0) AS CHAR) \
         FROM llx_facture f \
         LEFT JOIN (SELECT fk_facture, SUM(amount) AS regle \
                    FROM llx_paiement_facture GROUP BY fk_facture) pe \
                ON pe.fk_facture = f.rowid \
         WHERE f.entity = :entity AND f.fk_statut >= 1 AND f.paye = 0",
        params! { "entity" => entity },
    )?;
    Ok(dec(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_mois_glissants_sont_dans_l_ordre_et_contigus() {
        let m = mois_glissants("2026-11-05", 3);
        assert_eq!(m, vec!["2026-09", "2026-10", "2026-11"]);
    }

    #[test]
    fn les_mois_traversent_le_changement_d_annee() {
        let m = mois_glissants("2026-02-10", 4);
        assert_eq!(m, vec!["2025-11", "2025-12", "2026-01", "2026-02"]);
    }

    #[test]
    fn un_solde_d_echeance_est_la_difference() {
        let e = Echeance {
            mois: "2026-11".into(),
            encaissements: Decimal::from(1000),
            decaissements: Decimal::from(400),
        };
        assert_eq!(e.solde(), Decimal::from(600));
    }

    #[test]
    fn un_montant_illisible_vaut_zero_plutot_que_de_planter() {
        assert_eq!(dec(Some("".into())), Decimal::ZERO);
        assert_eq!(dec(None), Decimal::ZERO);
        assert_eq!(dec(Some("abc".into())), Decimal::ZERO);
        assert_eq!(dec(Some("12.5".into())), Decimal::from_str("12.5").unwrap());
    }
}
