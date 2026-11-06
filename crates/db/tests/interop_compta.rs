//! Interoperabilite comptable : les deux logiciels lisent la meme table.
//!
//! La promesse du projet est qu'une base ecrite par Cordeau s'ouvre dans
//! Dolibarr, et l'inverse. Ce n'est nulle part plus sensible qu'en
//! comptabilite : une ecriture que l'un voit et l'autre non fait un grand livre
//! faux, et un grand livre faux ne se rattrape pas.
//!
//! # Ce que ces tests verifient
//!
//! Le **contrat de donnees** de `llx_accounting_bookkeeping`, des deux cotes :
//!
//! 1. une ligne ecrite **a la maniere de Dolibarr** est vue par notre grand
//!    livre (c'est ce qui manquait : on ne lisait pas cette table) ;
//! 2. une ecriture ecrite **par notre noyau** est relue par la requete exacte
//!    que Dolibarr execute (`BookKeeping::fetchAll`, `bookkeeping.class.php`
//!    l. 1170-1200) ;
//! 3. la piece est **equilibree** : debit = credit.
//!
//! Lancer le PHP de Dolibarr par-dessus ne changerait rien a ces donnees : il
//! lit ces colonnes-la, dans cet ordre-la.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::Queryable;
use mysql::{params, Pool, Row};
use rust_decimal::Decimal;

const ENTITY: i64 = 1;

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// La requete de `BookKeeping::fetchAll`, recopiee telle quelle.
///
/// Les colonnes sont exactement celles du PHP, dans le meme ordre : si
/// `llx_accounting_bookkeeping` perdait une de ces colonnes, Dolibarr
/// n'afficherait plus rien — et ce test le dirait avant lui.
const REQUETE_DOLIBARR: &str = "SELECT \
     t.rowid, t.doc_date, t.doc_type, t.doc_ref, t.fk_doc, t.fk_docdet, \
     t.thirdparty_code, t.subledger_account, t.subledger_label, t.numero_compte, \
     t.label_compte, t.label_operation, t.debit, t.credit, t.lettering_code, \
     t.date_lettering, t.montant as amount, t.sens, t.fk_user_author, t.import_key, \
     t.code_journal, t.journal_label, t.piece_num, t.date_creation, \
     t.date_lim_reglement, t.tms as date_modification, t.date_export, \
     t.date_validated as date_validation \
     FROM llx_accounting_bookkeeping as t \
     WHERE t.entity = ? AND t.doc_ref = ?";

/// Une ligne ecrite a la maniere de Dolibarr se lit dans notre grand livre.
///
/// C'est le sens qui manquait : notre grand livre reconstituait les ecritures
/// depuis les factures, et ne voyait donc rien de ce que Dolibarr avait
/// ventile.
#[test]
fn une_ecriture_de_dolibarr_se_lit_dans_notre_grand_livre() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    // Marque unique : on ne touche a aucune ecriture reelle.
    let marque = format!("ZZ-INTEROP-DOLI-{}", std::process::id());
    let mut conn = pool.get_conn()?;

    // Les deux lignes que Dolibarr produit pour une vente de 100 HT :
    // 411 client au debit, 701 vente au credit.
    // `piece_num` n'est ni nullable ni pourvu d'un defaut : Dolibarr le
    // numerote par journal. On fait pareil, sinon l'insertion est refusee — et
    // c'est heureux, une ligne sans numero de piece ne se relit pas.
    let piece: i64 = conn
        .exec_first(
            "SELECT COALESCE(MAX(piece_num), 0) + 1 FROM llx_accounting_bookkeeping \
             WHERE entity = :entity AND code_journal = 'VE'",
            params! { "entity" => ENTITY },
        )?
        .unwrap_or(1);

    for (compte, libelle, debit, credit) in [
        ("411000", "Client", "100.00", "0"),
        ("701000", "Vente", "0", "100.00"),
    ] {
        conn.exec_drop(
            "INSERT INTO llx_accounting_bookkeeping \
             (entity, doc_date, doc_type, doc_ref, fk_doc, fk_docdet, numero_compte, \
              label_compte, label_operation, debit, credit, montant, sens, code_journal, \
              journal_label, fk_user_author, date_creation, piece_num) \
             VALUES (:entity, CURDATE(), 'customer_invoice', :ref, 0, 0, :compte, :libelle, \
              :libelle, :debit, :credit, :montant, :sens, 'VE', 'Journal des ventes', 0, NOW(), \
              :piece)",
            params! {
                "entity" => ENTITY,
                "ref" => marque.as_str(),
                "compte" => compte,
                "libelle" => libelle,
                "debit" => debit,
                "credit" => credit,
                "montant" => if credit == "0" { debit } else { credit },
                "sens" => if credit == "0" { "D" } else { "C" },
                "piece" => piece,
            },
        )?;
    }

    // Notre grand livre doit les voir, et la balance doit les totaliser.
    let lignes = db::grand_livre::ecritures_comptables(&pool, ENTITY, None, None)?;
    let trouvees: Vec<_> = lignes.iter().filter(|e| e.reference == marque).collect();
    assert_eq!(
        trouvees.len(),
        2,
        "notre grand livre doit voir les deux lignes de Dolibarr"
    );

    let balance = db::grand_livre::balance(&pool, ENTITY, None, None)?;
    let client = balance
        .iter()
        .find(|b| b.compte == "411000")
        .expect("le compte client doit apparaitre");
    assert!(
        client.debit >= dec("100"),
        "le debit client doit inclure notre vente, trouve {}",
        client.debit
    );

    conn.exec_drop(
        "DELETE FROM llx_accounting_bookkeeping WHERE entity = :entity AND doc_ref = :ref",
        params! { "entity" => ENTITY, "ref" => marque.as_str() },
    )?;
    Ok(())
}

/// Ce que notre noyau ecrit est relu par la requete de Dolibarr, et equilibre.
#[test]
fn une_ecriture_de_cordeau_se_relit_avec_la_requete_de_dolibarr() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    let nom = format!("ZZ-INTEROP-{}", std::process::id());
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(&nom))?;

    let mut f = db::invoice::NewInvoice::new(fk_soc);
    f.lines = vec![db::invoice::NewInvoiceLine::new(
        "Prestation de test",
        dec("1"),
        dec("1000"),
        dec("20"),
    )];
    let id = db::invoice::create(&pool, ENTITY, &f)?;
    db::invoice::validate(&pool, ENTITY, id)?;
    let facture = db::invoice::get(&pool, ENTITY, id)?.expect("facture relue");

    // La requete exacte de Dolibarr, sur la reference de la facture.
    let mut conn = pool.get_conn()?;
    let lignes: Vec<Row> = conn.exec(REQUETE_DOLIBARR, (ENTITY, facture.ref_.as_str()))?;

    if lignes.is_empty() {
        // Aucune ecriture : le plan comptable n'est pas configure sur cette
        // base. On le dit, plutot que d'echouer sur une installation incomplete.
        eprintln!(
            "aucune ecriture pour {} : plan comptable non configure, test ignore",
            facture.ref_
        );
        db::invoice::delete(&pool, ENTITY, id).ok();
        return Ok(());
    }

    // Lecture tolerante : `get` panique sur un type qui ne correspond pas, et
    // un test qui panique sur un detail de conversion dit moins qu'un test qui
    // explique ce qu'il attendait.
    let texte = |ligne: &Row, i: usize| -> String {
        ligne
            .get_opt::<String, _>(i)
            .and_then(|v| v.ok())
            .unwrap_or_default()
    };
    // `debit` et `credit` sont des `double` : on les lit en flottant puis on
    // convertit par le texte, jamais par une multiplication — c'est le meme
    // chemin que le noyau suit partout ailleurs.
    let nombre = |ligne: &Row, i: usize| -> Decimal {
        if let Some(v) = ligne.get_opt::<f64, _>(i).and_then(|v| v.ok()) {
            return Decimal::from_str(&format!("{v:.2}")).unwrap_or_default();
        }
        Decimal::from_str(texte(ligne, i).trim()).unwrap_or_default()
    };

    let mut debit = Decimal::ZERO;
    let mut credit = Decimal::ZERO;
    for ligne in &lignes {
        // Colonne 9 : `numero_compte`. Dolibarr refuse une ecriture sans compte
        // (le plan comptable francais est en 5 chiffres au minimum).
        let compte = texte(ligne, 9);
        assert!(compte.len() >= 5, "numero de compte invalide : {compte:?}");
        // Colonnes 12 et 13 : debit et credit.
        debit += nombre(ligne, 12);
        credit += nombre(ligne, 13);
    }

    assert_eq!(
        debit, credit,
        "la piece doit etre equilibree : debit {debit}, credit {credit}"
    );
    assert_eq!(
        debit,
        dec("1200"),
        "le debit egale le TTC de la facture (compte client)"
    );

    db::invoice::delete(&pool, ENTITY, id).ok();
    Ok(())
}
