//! Dictionnaires de Dolibarr (`llx_c_*`).
//!
//! Ces tables sont installees avec le schema et alimentees par l'installeur.
//! On les **lit** ; quand un module en ajoute, le noyau les voit sans rien changer.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Ligne brute d'un dictionnaire : identifiant, code, libelle court, libelle long, actif.
type LigneDico = (i64, String, Option<String>, Option<String>, i8);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entree {
    pub id: i64,
    pub code: String,
    pub libelle: String,
    pub actif: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TauxTva {
    pub taux: Decimal,
    pub code: String,
    pub libelle: String,
}

fn opt_s(v: Option<String>) -> String {
    v.unwrap_or_default().trim().to_string()
}

/// Unites de mesure (`llx_c_units`). `label` est une cle de traduction, on
/// privilegie `short_label` qui est lisible tel quel.
pub fn unites(pool: &Pool) -> Result<Vec<Entree>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<LigneDico> = conn.query(
        "SELECT rowid, code, short_label, label, active FROM llx_c_units \
         WHERE active = 1 ORDER BY sortorder ASC, code ASC",
    )?;
    Ok(lignes
        .into_iter()
        .map(|(id, code, court, long, actif)| Entree {
            id,
            libelle: {
                let c = opt_s(court);
                if c.is_empty() {
                    opt_s(long)
                } else {
                    c
                }
            },
            code,
            actif: actif != 0,
        })
        .collect())
}

/// Taux de TVA actifs pour un pays et une entite (`llx_c_tva`).
///
/// Dolibarr stocke un jeu de taux par pays ; `fk_pays` peut etre nul pour les
/// taux communs. On dedoublonne par taux, du plus eleve au plus bas.
pub fn taux_tva(pool: &Pool, entity: i64, fk_pays: i64) -> Result<Vec<TauxTva>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<(Option<String>, String, Option<String>)> = conn.exec(
        "SELECT CAST(taux AS CHAR), code, note FROM llx_c_tva \
         WHERE active = 1 AND (fk_pays IS NULL OR fk_pays = :pays) \
           AND (entity = :entity OR entity = 0) \
         ORDER BY taux DESC",
        params! { "pays" => fk_pays, "entity" => entity },
    )?;

    let mut out: Vec<TauxTva> = Vec::new();
    for (taux, code, note) in lignes {
        let taux = match taux
            .as_deref()
            .and_then(|s| Decimal::from_str(s.trim()).ok())
        {
            Some(t) => t,
            None => continue,
        };
        if out.iter().any(|x| x.taux == taux) {
            continue;
        }
        out.push(TauxTva {
            taux,
            code: code.trim().to_string(),
            libelle: opt_s(note),
        });
    }
    Ok(out)
}

/// Modes de paiement (`llx_c_paiement`). La colonne d'identifiant est `id`.
pub fn modes_paiement(pool: &Pool) -> Result<Vec<Entree>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<(i64, String, String, i8)> = conn.query(
        "SELECT id, code, libelle, active FROM llx_c_paiement \
         WHERE active = 1 ORDER BY id ASC",
    )?;
    Ok(lignes
        .into_iter()
        .map(|(id, code, libelle, actif)| Entree {
            id,
            code,
            libelle: libelle.trim().to_string(),
            actif: actif != 0,
        })
        .collect())
}

/// Rend une unite lisible : `M2` devient `m²`, `P` devient `u`.
///
/// Dolibarr stocke `short_label` sur 5 caracteres (« forfait » y devient
/// « forfa ») et `label` est une cle de traduction. On refait donc l'affichage
/// a partir du code, qui est stable.
pub fn unite_lisible(code: &str, court: &str) -> String {
    let c = code.trim().to_ascii_uppercase();
    let lisible = match c.as_str() {
        "T" => "t",
        "KG" => "kg",
        "G" => "g",
        "MG" => "mg",
        "OZ" => "oz",
        "LB" => "lb",
        "M" => "m",
        "DM" => "dm",
        "CM" => "cm",
        "MM" => "mm",
        "FT" => "ft",
        "IN" => "in",
        "M2" => "m²",
        "DM2" => "dm²",
        "CM2" => "cm²",
        "MM2" => "mm²",
        "FT2" => "ft²",
        "IN2" => "in²",
        "M3" => "m³",
        "DM3" => "dm³",
        "CM3" => "cm³",
        "MM3" => "mm³",
        "FT3" => "ft³",
        "IN3" => "in³",
        "L" => "L",
        "GAL" => "gal",
        "P" => "u",
        "SET" => "ens.",
        "S" => "s",
        "MI" => "min",
        "H" => "h",
        "D" => "j",
        "W" => "sem.",
        "MO" => "mois",
        "Y" => "an",
        _ => "",
    };
    if !lisible.is_empty() {
        return lisible.to_string();
    }
    // Unite propre au client (ex. « forfait ») : le code sert de libelle.
    let c = code.trim();
    if !c.is_empty() {
        c.to_string()
    } else {
        court.trim().to_string()
    }
}

/// Identifiant d'une unite par son code (`FORFAIT`, `M2`, `H`...).
pub fn unite_par_code<Q>(conn: &mut Q, code: &str) -> Result<Option<i64>, Box<dyn Error>>
where
    Q: mysql::prelude::Queryable,
{
    Ok(conn.exec_first(
        "SELECT rowid FROM llx_c_units WHERE UPPER(code) = UPPER(:code) AND active = 1 LIMIT 1",
        params! { "code" => code },
    )?)
}

/// Table `rowid -> libelle lisible` des unites.
pub fn unites_libelle(
    pool: &Pool,
) -> Result<std::collections::BTreeMap<i64, String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<(i64, String, String)> = conn.exec(
        "SELECT rowid, code, short_label FROM llx_c_units \
         WHERE active = 1 ORDER BY rowid",
        (),
    )?;
    Ok(lignes
        .into_iter()
        .map(|(id, code, court)| (id, unite_lisible(&code, &court)))
        .collect())
}

/// Conditions de reglement (`llx_c_payment_term`).
pub fn conditions_reglement(pool: &Pool, entity: i64) -> Result<Vec<Entree>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<LigneDico> = conn.exec(
        "SELECT rowid, code, libelle, libelle_facture, active FROM llx_c_payment_term \
         WHERE active = 1 AND (entity = :entity OR entity = 0) \
         ORDER BY sortorder ASC, rowid ASC",
        params! { "entity" => entity },
    )?;
    Ok(lignes
        .into_iter()
        .map(|(id, code, libelle, facture, actif)| Entree {
            id,
            code,
            libelle: {
                let l = opt_s(libelle);
                if l.is_empty() {
                    opt_s(facture)
                } else {
                    l
                }
            },
            actif: actif != 0,
        })
        .collect())
}

/// Recense les tables de dictionnaire `llx_c_*` et leur nombre de lignes.
pub fn recensement(pool: &Pool) -> Result<Vec<(String, i64)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let tables: Vec<String> = conn.query(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name LIKE 'llx\\_c\\_%' \
         ORDER BY table_name",
    )?;
    let mut out = Vec::with_capacity(tables.len());
    for t in tables {
        // Le nom vient d'information_schema, pas d'une entree utilisateur.
        let n: Option<i64> = conn.query_first(format!("SELECT COUNT(*) FROM `{t}`"))?;
        out.push((t, n.unwrap_or(0)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> Option<Pool> {
        let dsn = std::env::var("DOLIBARR_DSN").ok()?;
        db_pool(&dsn)
    }

    fn db_pool(dsn: &str) -> Option<Pool> {
        match crate::pool(dsn) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("connexion impossible ({e}) : test ignore");
                None
            }
        }
    }

    #[test]
    fn les_unites_sont_lisibles() {
        let Some(p) = pool() else { return };
        let u = unites(&p).expect("unites");
        assert!(!u.is_empty());
        assert!(u.iter().any(|e| e.code == "M"), "le metre doit exister");
        assert!(
            u.iter().all(|e| !e.libelle.is_empty()),
            "aucune unite sans libelle"
        );
    }

    #[test]
    fn les_taux_de_tva_francais_sont_presents() {
        let Some(p) = pool() else { return };
        let t = taux_tva(&p, 1, 1).expect("tva");
        let taux: Vec<String> = t.iter().map(|x| x.taux.to_string()).collect();
        assert!(taux.iter().any(|x| x == "20"), "20 % absent : {taux:?}");
        assert!(taux.iter().any(|x| x == "10"), "10 % absent : {taux:?}");
        // dedoublonnes et tries decroissants
        let mut precedent = Decimal::from(10_000);
        for x in &t {
            assert!(x.taux <= precedent, "tri decroissant attendu");
            precedent = x.taux;
        }
    }

    #[test]
    fn les_modes_de_paiement_sont_presents() {
        let Some(p) = pool() else { return };
        let m = modes_paiement(&p).expect("paiements");
        assert!(m.iter().any(|e| e.code == "VIR"));
        assert!(m.iter().any(|e| e.code == "CHQ"));
        assert_eq!(
            m.iter().find(|e| e.code == "VIR").map(|e| e.id),
            Some(crate::payment::MODE_VIREMENT)
        );
    }

    #[test]
    fn les_conditions_de_reglement_sont_presentes() {
        let Some(p) = pool() else { return };
        let c = conditions_reglement(&p, 1).expect("conditions");
        assert!(!c.is_empty());
        assert!(
            c.iter().all(|e| !e.libelle.is_empty()),
            "aucune condition sans libelle"
        );
    }
}
