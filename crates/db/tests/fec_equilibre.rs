use std::collections::BTreeMap;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::Queryable;
use mysql::{params, Pool};
use rust_decimal::Decimal;

const ENTITY: i64 = 1;

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test ignore");
            None
        }
    }
}

/// Le FEC doit s'equilibrer **par ecriture**, pas seulement au total.
///
/// Art. L. 47 A-I du LPF : un fichier ou chaque ligne est sa propre ecriture
/// est desagrege, donc non probant. On verifie ici que toutes les lignes d'une
/// meme piece portent le meme numero et que chaque numero s'equilibre.
#[test]
fn chaque_ecriture_du_fec_s_equilibre() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    // Une piece de test : 2 lignes equilibrees, dans un journal a part.
    let mut conn = pool.get_conn()?;
    let marque = format!("ZZFEC-{}", std::process::id());
    for (compte, libelle, debit, credit) in [
        ("411000", "Client test", "120.00", "0"),
        ("706000", "Vente test", "0", "100.00"),
        ("445710", "TVA test", "0", "20.00"),
    ] {
        conn.exec_drop(
            "INSERT INTO llx_accounting_bookkeeping \
             (entity, piece_num, doc_date, doc_type, doc_ref, fk_doc, fk_docdet, \
              numero_compte, label_compte, label_operation, debit, credit, montant, \
              sens, code_journal, journal_label, fk_user_author) \
             VALUES (:entity, 1, :jour, 'dolibarr_rs_test', :ref, 0, 0, \
                     :compte, :libelle, :libelle, :debit, :credit, :montant, \
                     :sens, 'ZZ', 'Journal de test', 0)",
            params! {
                "entity" => ENTITY,
                "jour" => "2026-11-04",
                "ref" => marque.as_str(),
                "compte" => compte,
                "libelle" => libelle,
                "debit" => debit,
                "credit" => credit,
                "montant" => if credit == "0" { debit } else { credit },
                "sens" => if credit == "0" { "D" } else { "C" },
            },
        )?;
    }
    drop(conn);

    let fec = db::fec::export(&pool, ENTITY, "2026-11-04", "2026-11-04")?;
    let mut par_numero: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();
    for ligne in fec.lines().skip(1) {
        let c: Vec<&str> = ligne.split('\t').collect();
        assert_eq!(c.len(), 18, "le FEC doit avoir 18 colonnes : {ligne}");
        let numero = c[2].to_string();
        let d = Decimal::from_str(c[11]).unwrap_or_default();
        let cr = Decimal::from_str(c[12]).unwrap_or_default();
        let e = par_numero.entry(numero).or_default();
        e.0 += d;
        e.1 += cr;
    }

    assert!(!par_numero.is_empty(), "aucune ecriture exportee");
    for (numero, (debit, credit)) in &par_numero {
        assert_eq!(
            debit, credit,
            "l'ecriture {numero} ne s'equilibre pas : {debit} != {credit}"
        );
    }

    // Menage.
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_accounting_bookkeeping WHERE doc_ref = :ref",
        params! { "ref" => marque.as_str() },
    )?;
    println!("FEC : {} ecriture(s), toutes equilibrees", par_numero.len());
    Ok(())
}
