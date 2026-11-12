//! Cloture d'exercice : solder les comptes de resultat, et garder l'instantane.
//!
//! A la fin d'un exercice, les comptes de charges (classe 6) et de produits
//! (classe 7) doivent etre remis a zero, et leur difference portee au compte
//! **12 « Resultat de l'exercice »**. C'est ce que font tous les logiciels
//! comptables, et c'est ce que Dolibarr appelle « cloture ».
//!
//! Trois decisions, prises et ecrites ici pour ne pas les rouvrir :
//!
//! 1. **Compte 12**, et non 120/129. La repartition entre benefice et pertes
//!    anterieures releve de l'assemblee, pas du logiciel. Le signe du solde de
//!    12 dit deja s'il y a benefice (credit) ou perte (debit).
//! 2. **Une ecriture, un numero** : `PA-CLOTURE-<AAAA>`. Comme l'ecriture est
//!    reecrite par `ref`, rejouer la cloture ne la duplique pas.
//! 3. **Une cloture ne se solde pas elle-meme.** Les ecritures de cloture sont
//!    exclues du calcul : sans cela, une seconde cloture annulerait la
//!    premiere et produirait un resultat de zero.
//!
//! L'instantane (le « bilan comparatif » que Dolibarr 24.0 ajoute) est range
//! dans `llx_const`, en JSON : c'est de la donnee, pas du schema, et la table
//! des constantes est faite pour ca.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use comptabilite::{Ecriture, Ligne};

/// Le compte de resultat du plan comptable general.
pub const COMPTE_RESULTAT: &str = "120000";
/// Le journal des ecritures de cloture.
pub const JOURNAL_CLOTURE: &str = "CL";
/// Prefixe des numeros d'ecriture de cloture.
pub const PREFIXE: &str = "PA-CLOTURE-";

/// Le solde d'un compte sur la periode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Solde {
    pub compte: String,
    pub libelle: String,
    pub debit: Decimal,
    pub credit: Decimal,
}

impl Solde {
    /// Solde debiteur positif : `debit - credit`.
    pub fn solde(&self) -> Decimal {
        self.debit - self.credit
    }
}

/// Ce qu'une cloture produit : les lignes, le resultat, et de quoi le montrer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apercu {
    pub debut: String,
    pub fin: String,
    pub date_cloture: String,
    /// Charges de la periode (classe 6), en positif.
    pub charges: Decimal,
    /// Produits de la periode (classe 7), en positif.
    pub produits: Decimal,
    /// Produits moins charges : positif = benefice, negatif = perte.
    pub resultat: Decimal,
    pub lignes: Vec<Ligne>,
    /// Tous les soldes de la periode, pour l'instantane.
    pub soldes: Vec<Solde>,
}

impl Apercu {
    /// Y a-t-il quelque chose a solder ?
    pub fn a_solder(&self) -> bool {
        !self.lignes.is_empty()
    }
}

/// Un compte porte-t-il un resultat ? Classes 6 (charges) et 7 (produits).
fn is_resultat(compte: &str) -> bool {
    compte.starts_with('6') || compte.starts_with('7')
}

/// Construit les lignes de la cloture a partir des soldes.
///
/// Pure : ni base ni horloge, donc verifiable. Chaque compte de resultat est
/// ramene a zero, et la difference va au compte 12.
pub fn construire(soldes: &[Solde], date: &str, journal: &str) -> (Vec<Ligne>, Decimal) {
    let mut lignes = Vec::new();
    let mut charges = Decimal::ZERO;
    let mut produits = Decimal::ZERO;

    for s in soldes.iter().filter(|s| is_resultat(&s.compte)) {
        let solde = s.solde();
        if solde == Decimal::ZERO {
            continue;
        }
        if s.compte.starts_with('6') {
            charges += solde;
        } else {
            produits -= solde; // le credit rend le solde negatif
        }
        // On contre-passe : un compte debiteur est credite, et inversement.
        lignes.push(Ligne {
            compte: s.compte.clone(),
            libelle: format!("Cloture — {}", s.libelle),
            debit: if solde < Decimal::ZERO {
                -solde
            } else {
                Decimal::ZERO
            },
            credit: if solde > Decimal::ZERO {
                solde
            } else {
                Decimal::ZERO
            },
            journal: journal.to_string(),
            date: date.to_string(),
        });
    }

    let resultat = produits - charges;

    // L'equilibrage. Les lignes ci-dessus ne tombent pas juste : ce qui manque
    // est le resultat, et il va au compte 12.
    if !lignes.is_empty() {
        let debit: Decimal = lignes.iter().map(|l| l.debit).sum();
        let credit: Decimal = lignes.iter().map(|l| l.credit).sum();
        let ecart = debit - credit;
        if ecart != Decimal::ZERO {
            lignes.push(Ligne {
                compte: COMPTE_RESULTAT.to_string(),
                libelle: if resultat >= Decimal::ZERO {
                    "Resultat de l'exercice — benefice".to_string()
                } else {
                    "Resultat de l'exercice — perte".to_string()
                },
                debit: if ecart < Decimal::ZERO {
                    -ecart
                } else {
                    Decimal::ZERO
                },
                credit: if ecart > Decimal::ZERO {
                    ecart
                } else {
                    Decimal::ZERO
                },
                journal: journal.to_string(),
                date: date.to_string(),
            });
        }
    }

    (lignes, resultat)
}

/// Le numero d'ecriture d'une cloture, a partir de l'exercice.
pub fn numero(exercice: &str) -> String {
    format!("{PREFIXE}{exercice}")
}

/// Le numero de l'exercice clos par cette ecriture, si elle en est une.
pub fn exercice_de(numero: &str) -> Option<String> {
    numero
        .strip_prefix(PREFIXE)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Les soldes d'une periode, lus dans le livre tenu.
///
/// Les ecritures de cloture sont exclues : elles appartiennent au resultat de
/// l'exercice, pas a son exploitation.
pub fn soldes(
    pool: &Pool,
    entity: i64,
    debut: &str,
    fin: &str,
) -> Result<Vec<Solde>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(String, Option<String>, String, String)> = conn.exec(
        "SELECT numero_compte, MAX(label_compte), \
                CAST(COALESCE(SUM(debit), 0) AS CHAR), \
                CAST(COALESCE(SUM(credit), 0) AS CHAR) \
         FROM llx_accounting_bookkeeping \
         WHERE entity = :entity AND doc_date BETWEEN :debut AND :fin \
           AND (ref IS NULL OR ref NOT LIKE 'PA-CLOTURE-%') \
         GROUP BY numero_compte \
         ORDER BY numero_compte ASC",
        params! { "entity" => entity, "debut" => debut, "fin" => fin },
    )?;

    let mut out = Vec::with_capacity(rows.len());
    for (compte, libelle, debit, credit) in rows {
        out.push(Solde {
            compte: compte.trim().to_string(),
            libelle: libelle.unwrap_or_default(),
            // Un solde illisible a zero passe inapercu : `construire()`
            // auto-equilibre la difference sur 120000, donc la balance reste
            // juste et l'erreur est indetectable. On refuse la ligne.
            debit: Decimal::from_str(debit.trim())
                .map_err(|_| format!("solde debit illisible : « {debit} »"))?,
            credit: Decimal::from_str(credit.trim())
                .map_err(|_| format!("solde credit illisible : « {credit} »"))?,
        });
    }
    Ok(out)
}

/// Prepare la cloture sans rien ecrire : ce que l'ecran montre avant d'agir.
pub fn apercu(
    pool: &Pool,
    entity: i64,
    debut: &str,
    fin: &str,
    date_cloture: &str,
) -> Result<Apercu, Box<dyn Error>> {
    let soldes = soldes(pool, entity, debut, fin)?;
    let (lignes, resultat) = construire(&soldes, date_cloture, JOURNAL_CLOTURE);

    let charges: Decimal = soldes
        .iter()
        .filter(|s| s.compte.starts_with('6'))
        .map(|s| s.solde())
        .sum();
    let produits: Decimal = soldes
        .iter()
        .filter(|s| s.compte.starts_with('7'))
        .map(|s| -s.solde())
        .sum();

    Ok(Apercu {
        debut: debut.to_string(),
        fin: fin.to_string(),
        date_cloture: date_cloture.to_string(),
        charges,
        produits,
        resultat,
        lignes,
        soldes,
    })
}

/// La cloture de cet exercice existe-t-elle deja ?
pub fn deja_cloture(pool: &Pool, entity: i64, exercice: &str) -> Result<bool, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_accounting_bookkeeping \
         WHERE entity = :entity AND ref = :ref",
        params! { "entity" => entity, "ref" => numero(exercice) },
    )?;
    Ok(n.unwrap_or(0) > 0)
}

/// Ecrit la cloture et depose l'instantane.
///
/// Refuse si l'exercice est deja clos, sauf `forcer` : une cloture est un acte,
/// pas un calcul qu'on rejoue par inadvertance. Rejouer avec `forcer` reecrit
/// la meme ecriture (meme numero), ce qui laisse le livre inchange.
pub fn cloturer(
    pool: &Pool,
    entity: i64,
    exercice: &str,
    debut: &str,
    fin: &str,
    date_cloture: &str,
    forcer: bool,
) -> Result<Apercu, Box<dyn Error>> {
    if !forcer && deja_cloture(pool, entity, exercice)? {
        return Err(format!("l'exercice {exercice} est deja clos").into());
    }

    let a = apercu(pool, entity, debut, fin, date_cloture)?;
    let ecriture = Ecriture {
        numero: numero(exercice),
        date: date_cloture.to_string(),
        libelle: format!("Cloture de l'exercice {exercice}"),
        lignes: a.lignes.clone(),
    };
    crate::persist_ecritures::ecrire(pool, entity, &ecriture)?;
    enregistrer_instantane(pool, entity, exercice, &a)?;
    Ok(a)
}

/// Depose l'instantane de cloture dans `llx_const`, en JSON.
///
/// C'est la photographie de l'exercice au moment ou il se ferme : le jour ou
/// quelqu'un demandera « combien faisait le compte 411 au 31/12 », elle repondra
/// sans recalculer sur un livre qui aura bouge.
pub fn enregistrer_instantane(
    pool: &Pool,
    entity: i64,
    exercice: &str,
    a: &Apercu,
) -> Result<(), Box<dyn Error>> {
    let comptes: Vec<serde_json::Value> = a
        .soldes
        .iter()
        .map(|s| {
            serde_json::json!({
                "compte": s.compte,
                "libelle": s.libelle,
                "debit": s.debit.normalize().to_string(),
                "credit": s.credit.normalize().to_string(),
            })
        })
        .collect();

    let valeur = serde_json::json!({
        "exercice": exercice,
        "debut": a.debut,
        "fin": a.fin,
        "date_cloture": a.date_cloture,
        "charges": a.charges.normalize().to_string(),
        "produits": a.produits.normalize().to_string(),
        "resultat": a.resultat.normalize().to_string(),
        "comptes": comptes,
    })
    .to_string();

    let mut conn = pool.get_conn()?;
    let nom = nom_instantane(exercice);
    conn.exec_drop(
        "INSERT INTO llx_const (name, entity, value, type, visible, note) \
         VALUES (:name, :entity, :value, 'chaine', 0, 'Instantane de cloture (noyau Rust)') \
         ON DUPLICATE KEY UPDATE value = VALUES(value), tms = NOW()",
        params! { "name" => nom, "entity" => entity, "value" => valeur },
    )?;
    Ok(())
}

/// Le nom de la constante qui porte l'instantane d'un exercice.
pub fn nom_instantane(exercice: &str) -> String {
    format!("PA_CLOTURE_{exercice}")
}

/// Un instantane de cloture, relu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instantane {
    pub exercice: String,
    pub debut: String,
    pub fin: String,
    pub charges: String,
    pub produits: String,
    pub resultat: String,
    pub comptes: usize,
}

/// Les instantanes deja deposes, du plus recent au plus ancien.
pub fn instantanes(pool: &Pool, entity: i64) -> Result<Vec<Instantane>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(String, String)> = conn.exec(
        "SELECT name, value FROM llx_const \
         WHERE entity = :entity AND name LIKE 'PA\\_CLOTURE\\_%' \
         ORDER BY name DESC",
        params! { "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .filter_map(|(name, valeur)| {
            let v: serde_json::Value = serde_json::from_str(&valeur).ok()?;
            let texte = |cle: &str| {
                v.get(cle)
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            Some(Instantane {
                exercice: name
                    .strip_prefix("PA_CLOTURE_")
                    .unwrap_or(&name)
                    .to_string(),
                debut: texte("debut"),
                fin: texte("fin"),
                charges: texte("charges"),
                produits: texte("produits"),
                resultat: texte("resultat"),
                comptes: v
                    .get("comptes")
                    .and_then(|c| c.as_array())
                    .map(|c| c.len())
                    .unwrap_or(0),
            })
        })
        .collect())
}

/// Les soldes d'un instantane, pour le bilan comparatif.
pub fn soldes_instantane(
    pool: &Pool,
    entity: i64,
    exercice: &str,
) -> Result<Vec<Solde>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let valeur: Option<String> = conn.exec_first(
        "SELECT value FROM llx_const WHERE entity = :entity AND name = :name",
        params! { "entity" => entity, "name" => nom_instantane(exercice) },
    )?;
    let Some(valeur) = valeur else {
        return Ok(Vec::new());
    };
    let v: serde_json::Value = serde_json::from_str(&valeur)?;
    let mut out = Vec::new();
    if let Some(comptes) = v.get("comptes").and_then(|c| c.as_array()) {
        for c in comptes {
            let texte = |cle: &str| {
                c.get(cle)
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            out.push(Solde {
                compte: texte("compte"),
                libelle: texte("libelle"),
                debit: Decimal::from_str(&texte("debit")).unwrap_or_default(),
                credit: Decimal::from_str(&texte("credit")).unwrap_or_default(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn solde(compte: &str, debit: &str, credit: &str) -> Solde {
        Solde {
            compte: compte.into(),
            libelle: format!("compte {compte}"),
            debit: d(debit),
            credit: d(credit),
        }
    }

    /// Un benefice : les charges sont creditees, les produits debites, et la
    /// difference est creditee au compte 12.
    #[test]
    fn un_benefice_se_solde_par_un_credit_au_compte_12() {
        let soldes = vec![
            solde("601000", "100", "0"), // achats
            solde("706000", "0", "150"), // ventes
        ];
        let (lignes, resultat) = construire(&soldes, "2026-12-31", "CL");

        assert_eq!(resultat, d("50"), "produits 150 - charges 100");
        assert_eq!(lignes.len(), 3);

        let debit: Decimal = lignes.iter().map(|l| l.debit).sum();
        let credit: Decimal = lignes.iter().map(|l| l.credit).sum();
        assert_eq!(debit, credit, "l'ecriture doit tenir");

        let au_12 = lignes.iter().find(|l| l.compte == COMPTE_RESULTAT).unwrap();
        assert_eq!(au_12.credit, d("50"));
        assert_eq!(au_12.debit, Decimal::ZERO, "un benefice est un credit");

        // Chaque compte de resultat est ramene a zero.
        let achat = lignes.iter().find(|l| l.compte == "601000").unwrap();
        assert_eq!(achat.credit, d("100"));
        let vente = lignes.iter().find(|l| l.compte == "706000").unwrap();
        assert_eq!(vente.debit, d("150"));
    }

    #[test]
    fn une_perte_se_solde_par_un_debit_au_compte_12() {
        let soldes = vec![solde("601000", "200", "0"), solde("706000", "0", "150")];
        let (lignes, resultat) = construire(&soldes, "2026-12-31", "CL");
        assert_eq!(resultat, d("-50"));
        let au_12 = lignes.iter().find(|l| l.compte == COMPTE_RESULTAT).unwrap();
        assert_eq!(au_12.debit, d("50"));
        assert_eq!(au_12.credit, Decimal::ZERO, "une perte est un debit");
        let debit: Decimal = lignes.iter().map(|l| l.debit).sum();
        let credit: Decimal = lignes.iter().map(|l| l.credit).sum();
        assert_eq!(debit, credit);
    }

    /// Les comptes de bilan (2 a 5) ne se soldent pas : ils se reportent.
    #[test]
    fn les_comptes_de_bilan_ne_sont_pas_soldes() {
        let soldes = vec![
            solde("411000", "1000", "0"),
            solde("512000", "500", "0"),
            solde("601000", "100", "0"),
        ];
        let (lignes, _) = construire(&soldes, "2026-12-31", "CL");
        assert!(lignes.iter().all(|l| l.compte != "411000"));
        assert!(lignes.iter().all(|l| l.compte != "512000"));
        assert!(lignes.iter().any(|l| l.compte == "601000"));
    }

    #[test]
    fn un_compte_a_solde_nul_est_ignore() {
        let soldes = vec![solde("601000", "100", "100"), solde("706000", "0", "0")];
        let (lignes, resultat) = construire(&soldes, "2026-12-31", "CL");
        assert!(lignes.is_empty(), "{lignes:?}");
        assert_eq!(resultat, Decimal::ZERO);
    }

    /// Charges egales aux produits : rien a porter au compte 12.
    #[test]
    fn un_resultat_nul_ne_cree_pas_de_ligne_au_compte_12() {
        let soldes = vec![solde("601000", "150", "0"), solde("706000", "0", "150")];
        let (lignes, resultat) = construire(&soldes, "2026-12-31", "CL");
        assert_eq!(resultat, Decimal::ZERO);
        assert!(lignes.iter().all(|l| l.compte != COMPTE_RESULTAT));
        let debit: Decimal = lignes.iter().map(|l| l.debit).sum();
        let credit: Decimal = lignes.iter().map(|l| l.credit).sum();
        assert_eq!(debit, credit);
    }

    #[test]
    fn le_numero_porte_l_exercice_et_se_relit() {
        assert_eq!(numero("2026"), "PA-CLOTURE-2026");
        assert_eq!(exercice_de("PA-CLOTURE-2026").as_deref(), Some("2026"));
        assert_eq!(exercice_de("PA-ACOMPTE-12"), None);
        assert_eq!(exercice_de("PA-CLOTURE-"), None);
    }

    #[test]
    fn le_nom_de_l_instantane_est_stable() {
        assert_eq!(nom_instantane("2026"), "PA_CLOTURE_2026");
    }
}
