//! Comptabilite tenue : ecritures dans `llx_accounting_bookkeeping` et export
//! au format FEC (fichier des ecritures comptables, art. A47 A-1 du LPF).
//!
//! Les ecritures sont generees a partir des documents valides, avec le meme
//! plan comptable que la balance. Elles sont ecrites en base : Dolibarr les
//! voit comme les siennes, et l'export FEC devient possible.

use std::collections::BTreeMap;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, TxOpts};
use rust_decimal::Decimal;

use crate::grand_livre;

/// Journaux utilises : ventes, achats, banque.
pub const JOURNAUX: &[(&str, &str)] = &[
    ("VE", "Journal des ventes"),
    ("AC", "Journal des achats"),
    ("BQ", "Journal de banque"),
];

fn libelle_journal(code: &str) -> &'static str {
    JOURNAUX
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, l)| *l)
        .unwrap_or("Journal")
}

/// Genere les ecritures d'une periode, en remplacant celles deja produites.
///
/// On ne touche qu'aux journaux VE/AC/BQ : une ecriture saisie dans un autre
/// journal n'est jamais effacee.
pub fn generer(
    pool: &Pool,
    entity: i64,
    debut: Option<&str>,
    fin: Option<&str>,
) -> Result<usize, Box<dyn Error>> {
    let ecritures = grand_livre::ecritures(pool, entity, debut, fin)?;
    let debut = debut.unwrap_or("1970-01-01");
    let fin = fin.unwrap_or("2099-12-31");

    let mut conn = pool.get_conn()?;
    let mut tx = conn.start_transaction(TxOpts::default())?;

    tx.exec_drop(
        "DELETE FROM llx_accounting_bookkeeping \
         WHERE entity = :entity AND code_journal IN ('VE','AC','BQ') \
           AND doc_date BETWEEN :debut AND :fin",
        params! { "entity" => entity, "debut" => debut, "fin" => fin },
    )?;

    let mut numero: BTreeMap<&str, i64> = BTreeMap::new();
    let mut inserees = 0usize;
    for e in &ecritures {
        let compteur = numero.entry(e.journal).or_insert(0);
        *compteur += 1;
        let n = *compteur;
        tx.exec_drop(
            // `piece_num`, `fk_user_author` et `code_journal` sont NOT NULL : on
            // les renseigne explicitement, jamais par defaut implicite.
            "INSERT INTO llx_accounting_bookkeeping \
             (entity, piece_num, doc_date, doc_type, doc_ref, fk_doc, fk_docdet, \
              numero_compte, label_compte, label_operation, debit, credit, \
              code_journal, journal_label, fk_user_author, date_creation) \
             VALUES (:entity, :piece, :date, 'dolibarr_rs', :ref, 0, 0, :compte, \
              :libelle_compte, :libelle, :debit, :credit, :journal, :journal_label, 0, NOW())",
            params! {
                "entity" => entity,
                "piece" => n,
                "date" => e.date.as_str(),
                "ref" => format!("{}#{}", e.reference, n),
                "compte" => e.compte,
                "libelle_compte" => e.libelle_compte,
                "libelle" => e.libelle.as_str(),
                "debit" => e.debit.to_string(),
                "credit" => e.credit.to_string(),
                "journal" => e.journal,
                "journal_label" => libelle_journal(e.journal),
            },
        )?;
        inserees += 1;
    }
    tx.commit()?;
    Ok(inserees)
}

/// Nombre d'ecritures en base pour une periode.
pub fn compter(
    pool: &Pool,
    entity: i64,
    debut: Option<&str>,
    fin: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_accounting_bookkeeping \
         WHERE entity = :entity AND doc_date BETWEEN :debut AND :fin",
        params! {
            "entity" => entity,
            "debut" => debut.unwrap_or("1970-01-01"),
            "fin" => fin.unwrap_or("2099-12-31"),
        },
    )?;
    Ok(n.unwrap_or(0))
}

/// Export FEC sur une periode.
///
/// 18 colonnes separees par des tabulations, dates en `AAAAMMJJ`, montants a
/// deux decimales avec virgule. Trie par journal, date puis numero d'ecriture,
/// comme le demande l'administration.
pub fn export(pool: &Pool, entity: i64, debut: &str, fin: &str) -> Result<String, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT code_journal, doc_ref, DATE_FORMAT(doc_date, '%Y%m%d'), numero_compte, \
                label_compte, label_operation, CAST(debit AS CHAR), CAST(credit AS CHAR) \
         FROM llx_accounting_bookkeeping \
         WHERE entity = :entity AND doc_date BETWEEN :debut AND :fin \
         ORDER BY code_journal ASC, doc_date ASC, rowid ASC",
        params! { "entity" => entity, "debut" => debut, "fin" => fin },
    )?;

    let mut sortie = String::from(
        "JournalCode\tJournalLib\tEcritureNum\tEcritureDate\tCompteNum\tCompteLib\tCompAuxNum\tCompAuxLib\tPieceRef\tPieceDate\tEcritureLib\tDebit\tCredit\tEcritureLet\tDateLet\tValidDate\tMontantdevise\tIdevise\n",
    );

    let mut compteurs: BTreeMap<String, i64> = BTreeMap::new();
    for r in rows {
        let s = |i: usize| {
            r.get_opt::<String, _>(i)
                .and_then(|v| v.ok())
                .unwrap_or_default()
        };
        let journal = s(0);
        let numero_piece = s(1);
        let date = s(2);
        let compte = s(3);
        let libelle_compte = s(4);
        let libelle = s(5).replace(['\t', '\n', '\r'], " ");
        let debit = Decimal::from_str(s(6).trim()).unwrap_or_default();
        let credit = Decimal::from_str(s(7).trim()).unwrap_or_default();

        let n = compteurs.entry(journal.clone()).or_insert(0);
        *n += 1;

        let champs: Vec<String> = vec![
            journal.clone(),
            libelle_journal(&journal).to_string(),
            n.to_string(),
            date.clone(),
            compte,
            libelle_compte,
            String::new(),
            String::new(),
            numero_piece,
            date.clone(),
            libelle,
            montant(debit),
            montant(credit),
            String::new(),
            String::new(),
            date,
            String::new(),
            "EUR".to_string(),
        ];
        sortie.push_str(&champs.join("\t"));
        sortie.push('\n');
    }
    Ok(sortie)
}

fn montant(v: Decimal) -> String {
    format!("{:.2}", v).replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_montants_sortent_a_la_virgule() {
        assert_eq!(montant(Decimal::from_str("1234.5").unwrap()), "1234,50");
        assert_eq!(montant(Decimal::ZERO), "0,00");
    }

    #[test]
    fn les_journaux_ont_un_libelle() {
        assert_eq!(libelle_journal("VE"), "Journal des ventes");
        assert_eq!(libelle_journal("XX"), "Journal");
    }
}
