//! Import d'un releve bancaire (CSV) dans `llx_bank`.
//!
//! Dolibarr a un module `bankimport` ; on en fait une version qui **dit ce
//! qu'elle ne comprend pas** au lieu de deviner. Un releve mal lu produit des
//! ecritures fausses, et une ecriture fausse se decouvre au bilan.
//!
//! # Les formats acceptes
//!
//! Deux formes, parce que ce sont les deux que les banques francaises
//! produisent :
//!
//! | Colonnes | Sens |
//! |---|---|
//! | `date ; libelle ; montant` | montant signe (negatif = debit) |
//! | `date ; libelle ; debit ; credit` | deux colonnes, l'une vide |
//!
//! Le separateur se deduit de la premiere ligne (`;`, `,` ou tabulation). Les
//! dates acceptent `JJ/MM/AAAA` et `AAAA-MM-JJ`. Les montants acceptent la
//! virgule decimale, l'espace des milliers, et le signe moins avant ou apres.
//!
//! # Le doublon
//!
//! Importer deux fois le meme releve ne doit pas doubler les ecritures. Une
//! ligne est reconnue par **date + montant + libelle** : meme jour, meme somme,
//! meme texte. C'est imparfait (deux cafes a 2,50 le meme jour se ressemblent),
//! donc l'ecran dit combien de lignes ont ete ecartees.

use std::collections::HashSet;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Une ligne lue dans le fichier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneReleve {
    /// `AAAA-MM-JJ`, deja normalisee.
    pub date: String,
    pub libelle: String,
    pub montant: Decimal,
}

/// Ce que l'import a fait.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rapport {
    pub ajoutees: usize,
    pub doublons: usize,
    /// Les lignes qu'on n'a pas su lire, avec leur numero et la raison.
    pub rejets: Vec<(usize, String)>,
}

/// Le separateur le plus frequent de la premiere ligne.
pub fn separateur(ligne: &str) -> char {
    let essais = [
        (';', ligne.matches(';').count()),
        (',', ligne.matches(',').count()),
        ('\t', ligne.matches('\t').count()),
    ];
    essais
        .iter()
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(_, n)| *n)
        .map(|(c, _)| *c)
        .unwrap_or(';')
}

/// `JJ/MM/AAAA` ou `AAAA-MM-JJ` vers `AAAA-MM-JJ`.
///
/// Rend `None` si la date n'existe pas : « 31/02 » n'est pas une date.
pub fn date(valeur: &str) -> Option<String> {
    let v = valeur.trim();
    let (jour, mois, annee) = if let Some((j, reste)) = v.split_once('/') {
        let (m, a) = reste.split_once('/')?;
        (j, m, a)
    } else if let Some((a, reste)) = v.split_once('-') {
        let (m, j) = reste.split_once('-')?;
        (j, m, a)
    } else {
        return None;
    };
    let jour: u32 = jour.parse().ok()?;
    let mois: u32 = mois.parse().ok()?;
    let annee: i32 = annee.get(..4)?.parse().ok()?;
    if !(1..=12).contains(&mois) || jour == 0 {
        return None;
    }
    let bissextile = (annee % 4 == 0 && annee % 100 != 0) || annee % 400 == 0;
    let jours = match mois {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if bissextile {
                29
            } else {
                28
            }
        }
    };
    if jour > jours {
        return None;
    }
    Some(format!("{annee:04}-{mois:02}-{jour:02}"))
}

/// Un montant ecrit a la francaise : `1 234,56`, `-12,00`, `12,00-`, `12.00`.
pub fn montant(valeur: &str) -> Option<Decimal> {
    let brut: String = valeur
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{00a0}' && *c != '€')
        .collect();
    if brut.is_empty() {
        return None;
    }
    // Le signe peut etre apres le nombre (releve anglo-saxon).
    let (signe, corps) = if let Some(c) = brut.strip_suffix('-') {
        (-1, c.to_string())
    } else {
        (1, brut.clone())
    };
    let corps = corps.replace(',', ".");
    let d = Decimal::from_str(&corps).ok()?;
    Some(d * Decimal::from(signe))
}

/// Lit un releve. Les lignes illisibles ne sont pas jetees : elles reviennent
/// dans le rapport, avec leur numero.
pub fn lire(texte: &str) -> (Vec<LigneReleve>, Vec<(usize, String)>) {
    let mut lignes: Vec<LigneReleve> = Vec::new();
    let mut rejets: Vec<(usize, String)> = Vec::new();
    let mut sep = ';';
    let mut entete_vue = false;

    for (i, brute) in texte.lines().enumerate() {
        let numero = i + 1;
        let l = brute.trim();
        if l.is_empty() {
            continue;
        }
        if !entete_vue {
            sep = separateur(l);
            entete_vue = true;
            // Une premiere ligne sans date est un entete : on la saute.
            let champs: Vec<&str> = l.split(sep).map(str::trim).collect();
            if champs.first().and_then(|c| date(c)).is_none() {
                continue;
            }
        }

        // Un champ peut etre entoure de guillemets.
        let champs: Vec<String> = l
            .split(sep)
            .map(|c| c.trim().trim_matches('"').trim().to_string())
            .collect();
        if champs.len() < 3 {
            rejets.push((numero, format!("{} colonne(s) au lieu de 3", champs.len())));
            continue;
        }

        let Some(jour) = date(&champs[0]) else {
            rejets.push((numero, format!("date illisible : « {} »", champs[0])));
            continue;
        };
        let libelle = champs[1].clone();
        if libelle.is_empty() {
            rejets.push((numero, "libelle vide".into()));
            continue;
        }

        let somme = if champs.len() >= 4 {
            // deux colonnes : debit puis credit, l'une des deux vide.
            let debit = montant(&champs[2]).unwrap_or_default();
            let credit = montant(&champs[3]).unwrap_or_default();
            if debit.is_zero() && credit.is_zero() {
                rejets.push((numero, "debit et credit vides".into()));
                continue;
            }
            if debit.is_zero() {
                credit.abs()
            } else {
                -debit.abs()
            }
        } else {
            match montant(&champs[2]) {
                Some(m) => m,
                None => {
                    rejets.push((numero, format!("montant illisible : « {} »", champs[2])));
                    continue;
                }
            }
        };

        lignes.push(LigneReleve {
            date: jour,
            libelle,
            montant: somme,
        });
    }

    (lignes, rejets)
}

/// Ecrit les lignes dans `llx_bank`, sans doublon.
pub fn importer(
    pool: &Pool,
    entity: i64,
    fk_account: i64,
    lignes: &[LigneReleve],
    rejets: Vec<(usize, String)>,
) -> Result<Rapport, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let compte: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_bank_account WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => fk_account },
    )?;
    if compte.is_none() {
        return Err(format!("le compte {fk_account} n'existe pas dans l'entite {entity}").into());
    }

    // Les ecritures deja la, sous la meme forme que la cle de doublon.
    let existantes: Vec<(String, String, String)> = conn.exec(
        "SELECT DATE_FORMAT(datev, '%Y-%m-%d'), COALESCE(label, ''), CAST(amount AS CHAR) \
         FROM llx_bank WHERE fk_account = :compte",
        params! { "compte" => fk_account },
    )?;
    let mut connues: HashSet<String> = existantes
        .into_iter()
        .map(|(d, l, m)| format!("{d}|{l}|{}", montant(&m).unwrap_or_default().normalize()))
        .collect();

    let mut rapport = Rapport {
        rejets,
        ..Default::default()
    };
    let mut tx = conn.start_transaction(mysql::TxOpts::default())?;
    for l in lignes {
        let cle = format!("{}|{}|{}", l.date, l.libelle, l.montant.normalize());
        if connues.contains(&cle) {
            rapport.doublons += 1;
            continue;
        }
        tx.exec_drop(
            "INSERT INTO llx_bank \
             (datec, datev, dateo, amount, amount_main_currency, label, fk_account, \
              fk_user_author, rappro, num_releve) \
             VALUES (NOW(), :date, :date, :montant, :montant, :libelle, :compte, \
              (SELECT MIN(rowid) FROM llx_user), 0, 'import')",
            params! {
                "date" => l.date.as_str(),
                "montant" => l.montant.to_string(),
                "libelle" => l.libelle.as_str(),
                "compte" => fk_account,
            },
        )?;
        connues.insert(cle);
        rapport.ajoutees += 1;
    }
    tx.commit()?;
    Ok(rapport)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(v: &str) -> Decimal {
        Decimal::from_str(v).unwrap()
    }

    #[test]
    fn les_dates_francaises_et_iso_donnent_la_meme_chose() {
        assert_eq!(date("05/03/2026").as_deref(), Some("2026-03-05"));
        assert_eq!(date("2026-03-05").as_deref(), Some("2026-03-05"));
        assert_eq!(date("31/02/2026"), None, "fevrier n'a pas 31 jours");
        assert_eq!(date("29/02/2024").as_deref(), Some("2024-02-29"));
    }

    #[test]
    fn les_montants_a_la_francaise_sont_lus() {
        assert_eq!(montant("1 234,56"), Some(d("1234.56")));
        assert_eq!(montant("-12,00"), Some(d("-12")));
        assert_eq!(
            montant("12,00-"),
            Some(d("-12")),
            "signe apres, releve anglo-saxon"
        );
        assert_eq!(montant("1 234,56 €"), Some(d("1234.56")));
        assert_eq!(montant(""), None);
    }

    #[test]
    fn un_releve_a_trois_colonnes_se_lit() {
        let (lignes, rejets) = lire("date;libelle;montant\n05/03/2026;CARREFOUR;-42,10\n06/03/2026;VIREMENT CLIENT;1 200,00\n");
        assert!(rejets.is_empty(), "{rejets:?}");
        assert_eq!(lignes.len(), 2);
        assert_eq!(lignes[0].montant, d("-42.10"));
        assert_eq!(lignes[1].montant, d("1200.00"));
    }

    #[test]
    fn un_releve_debit_credit_se_lit() {
        let (lignes, _) = lire("05/03/2026;CARREFOUR;42,10;\n06/03/2026;VIREMENT;;1200,00\n");
        assert_eq!(lignes[0].montant, d("-42.10"), "debit = negatif");
        assert_eq!(lignes[1].montant, d("1200.00"), "credit = positif");
    }

    #[test]
    fn une_ligne_illisible_revient_dans_le_rapport() {
        let (lignes, rejets) = lire("date;libelle;montant\npas une date;X;10\n05/03/2026;Y;abc\n");
        assert!(lignes.is_empty());
        assert_eq!(rejets.len(), 2, "les deux lignes sont dites, pas jetees");
        assert!(rejets[0].1.contains("date"));
    }
}
