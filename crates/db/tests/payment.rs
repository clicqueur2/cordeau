use std::error::Error;
use std::str::FromStr;

use db::invoice::{NewInvoice, NewInvoiceLine};
use db::payment::{NewPayment, MODE_CHEQUE, MODE_VIREMENT};
use mysql::Pool;
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

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn nettoyer(pool: &Pool, nom: &str) -> Result<(), Box<dyn Error>> {
    let appartient = |fk: Option<i64>, pool: &Pool| -> bool {
        match fk.and_then(|id| db::party::get(pool, ENTITY, id).ok().flatten()) {
            Some(s) => s.nom == nom,
            None => false,
        }
    };
    for f in db::invoice::list(pool, ENTITY, 1000, 0)? {
        if appartient(f.fk_soc, pool) {
            // Les reglements partent avec la facture (cascade applicative).
            for p in db::payment::list_for_invoice(pool, ENTITY, f.rowid)? {
                db::payment::delete(pool, ENTITY, p.rowid)?;
            }
            menage::facture(pool, ENTITY, f.rowid)?;
        }
    }
    for p in db::party::list(pool, ENTITY, 10_000, 0)? {
        if p.nom == nom {
            db::party::delete(pool, ENTITY, p.rowid)?;
        }
    }
    Ok(())
}

/// Facture de 1 000 € HT a 20 % => 1 200 € TTC.
fn facture_de_1200(pool: &Pool, nom: &str) -> Result<i64, Box<dyn Error>> {
    let fk_soc = db::party::create(pool, ENTITY, &db::party::NewParty::client(nom))?;
    let mut new = NewInvoice::new(fk_soc);
    new.lines = vec![NewInvoiceLine::new(
        "Travaux",
        dec("1"),
        dec("1000"),
        dec("20"),
    )];
    let id = db::invoice::create(pool, ENTITY, &new)?;
    db::invoice::validate(pool, ENTITY, id)?;
    Ok(id)
}

mod menage;

#[test]
fn un_reglement_partiel_ne_solde_pas_la_facture() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Paiement Partiel";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let id = facture_de_1200(&pool, nom)?;

    let p = db::payment::create(
        &pool,
        ENTITY,
        &NewPayment {
            fk_facture: id,
            montant: dec("500"),
            fk_paiement: MODE_VIREMENT,
            num_paiement: Some("VIR-001".into()),
            // Ce test ne parle pas de banque : aucun compte ne correspond,
            // donc aucune ligne bancaire n'est ecrite et il n'y a pas de contention.
            fk_compte: Some(-1),
            ..Default::default()
        },
    )?;
    assert!(p > 0);

    let b = db::payment::balance(&pool, ENTITY, id)?;
    assert_eq!(b.total_ttc, dec("1200"));
    assert_eq!(b.regle, dec("500"));
    assert_eq!(b.reste, dec("700"));
    assert!(!b.soldee);

    let f = db::invoice::get(&pool, ENTITY, id)?.unwrap();
    assert_eq!(f.paye, 0, "facture non soldee");
    assert_eq!(f.fk_statut, db::invoice::STATUT_VALIDEE);

    db::payment::delete(&pool, ENTITY, p)?;
    menage::facture(&pool, ENTITY, id)?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn deux_reglements_soldent_la_facture() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Paiement Complet";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let id = facture_de_1200(&pool, nom)?;

    let a = db::payment::create(
        &pool,
        ENTITY,
        &NewPayment {
            fk_facture: id,
            montant: dec("700"),
            fk_paiement: MODE_VIREMENT,
            // Ce test ne parle pas de banque : aucun compte ne correspond,
            // donc aucune ligne bancaire n'est ecrite et il n'y a pas de contention.
            fk_compte: Some(-1),
            ..Default::default()
        },
    )?;
    let b = db::payment::create(
        &pool,
        ENTITY,
        &NewPayment {
            fk_facture: id,
            montant: dec("500"),
            fk_paiement: MODE_CHEQUE,
            num_paiement: Some("CHQ-42".into()),
            // Ce test ne parle pas de banque : aucun compte ne correspond,
            // donc aucune ligne bancaire n'est ecrite et il n'y a pas de contention.
            fk_compte: Some(-1),
            ..Default::default()
        },
    )?;

    let f = db::invoice::get(&pool, ENTITY, id)?.unwrap();
    assert_eq!(f.paye, 1);
    assert_eq!(f.fk_statut, db::invoice::STATUT_PAYEE);

    let b2 = db::payment::balance(&pool, ENTITY, id)?;
    assert!(b2.soldee);
    assert_eq!(b2.reste, Decimal::ZERO);

    let listes = db::payment::list_for_invoice(&pool, ENTITY, id)?;
    assert_eq!(listes.len(), 2);
    assert!(listes.iter().all(|p| p.ref_.starts_with("PAY")));

    // Supprimer un reglement resilde... non : il rouvre la facture.
    db::payment::delete(&pool, ENTITY, a)?;
    let f = db::invoice::get(&pool, ENTITY, id)?.unwrap();
    assert_eq!(
        f.paye, 0,
        "suppression d'un reglement : la facture se rouvre"
    );

    db::payment::delete(&pool, ENTITY, b)?;
    menage::facture(&pool, ENTITY, id)?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn un_trop_percu_solde_aussi_la_facture() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Paiement Trop Perçu";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let id = facture_de_1200(&pool, nom)?;

    let p = db::payment::create(
        &pool,
        ENTITY,
        &NewPayment {
            fk_facture: id,
            montant: dec("1500"),
            fk_paiement: MODE_VIREMENT,
            // Ce test ne parle pas de banque : aucun compte ne correspond,
            // donc aucune ligne bancaire n'est ecrite et il n'y a pas de contention.
            fk_compte: Some(-1),
            ..Default::default()
        },
    )?;
    assert_eq!(db::invoice::get(&pool, ENTITY, id)?.unwrap().paye, 1);
    let b = db::payment::balance(&pool, ENTITY, id)?;
    assert_eq!(
        b.reste,
        Decimal::ZERO,
        "le reste du ne descend pas sous zero"
    );

    db::payment::delete(&pool, ENTITY, p)?;
    menage::facture(&pool, ENTITY, id)?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn le_reglement_ecrit_une_ligne_bancaire() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Paiement Bancaire";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;

    // Sans compte bancaire declare, Dolibarr ne peut rien rapprocher : on en cree un
    // pour la duree du test.
    let mut conn = pool.get_conn()?;
    use mysql::prelude::Queryable;
    // Restes d'une execution precedente interrompue.
    conn.exec_drop(
        "DELETE FROM llx_bank_url WHERE fk_bank IN \
         (SELECT rowid FROM llx_bank WHERE fk_account IN \
           (SELECT rowid FROM llx_bank_account WHERE ref = 'TEST-BQ'))",
        (),
    )?;
    conn.exec_drop(
        "DELETE FROM llx_bank WHERE fk_account IN \
         (SELECT rowid FROM llx_bank_account WHERE ref = 'TEST-BQ')",
        (),
    )?;
    conn.exec_drop("DELETE FROM llx_bank_account WHERE ref = 'TEST-BQ'", ())?;

    conn.exec_drop(
        "INSERT INTO llx_bank_account \
          (datec, ref, label, entity, courant, clos, fk_pays, currency_code, state_id) \
         VALUES (NOW(), 'TEST-BQ', 'Compte de test', ?, 1, 0, 1, 'EUR', 0)",
        (ENTITY,),
    )?;
    let compte = conn.last_insert_id() as i64;

    let id = facture_de_1200(&pool, nom)?;
    let p = db::payment::create(
        &pool,
        ENTITY,
        &NewPayment {
            fk_facture: id,
            montant: dec("1200"),
            fk_paiement: MODE_VIREMENT,
            num_paiement: Some("VIR-BQ".into()),
            fk_compte: Some(compte),
            ..Default::default()
        },
    )?;

    let lignes: Vec<(i64, String)> = conn.exec(
        "SELECT b.rowid, CAST(b.amount AS CHAR) FROM llx_bank b \
         INNER JOIN llx_bank_url u ON u.fk_bank = b.rowid \
         WHERE u.type = 'payment' AND u.url_id = ?",
        (p,),
    )?;
    assert_eq!(
        lignes.len(),
        1,
        "une ligne bancaire attendue, trouve {lignes:?}"
    );
    assert_eq!(lignes[0].1, "1200.00000000");

    // Supprimer le reglement retire aussi la ligne bancaire.
    db::payment::delete(&pool, ENTITY, p)?;
    let restantes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_bank_url WHERE type = 'payment' AND url_id = ?",
        (p,),
    )?;
    assert_eq!(restantes.unwrap_or(-1), 0);

    menage::facture(&pool, ENTITY, id)?;
    // Menage : les lignes bancaires de ce compte avant le compte lui-meme,
    // sinon la cle etrangere bloque si un autre test y a ecrit.
    conn.exec_drop(
        "DELETE FROM llx_bank_url WHERE fk_bank IN (SELECT rowid FROM llx_bank WHERE fk_account = ?)",
        (compte,),
    )?;
    conn.exec_drop("DELETE FROM llx_bank WHERE fk_account = ?", (compte,))?;
    conn.exec_drop("DELETE FROM llx_bank_account WHERE rowid = ?", (compte,))?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn refuse_de_regler_un_brouillon() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Paiement Brouillon";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;

    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;
    let mut new = NewInvoice::new(fk_soc);
    new.lines = vec![NewInvoiceLine::new(
        "Travaux",
        dec("1"),
        dec("1000"),
        dec("20"),
    )];
    // volontairement non validee
    let id = db::invoice::create(&pool, ENTITY, &new)?;

    let err = db::payment::create(
        &pool,
        ENTITY,
        &NewPayment {
            fk_facture: id,
            montant: dec("1200"),
            fk_paiement: MODE_VIREMENT,
            // Ce test ne parle pas de banque : aucun compte ne correspond,
            // donc aucune ligne bancaire n'est ecrite et il n'y a pas de contention.
            fk_compte: Some(-1),
            ..Default::default()
        },
    )
    .expect_err("un brouillon ne doit pas etre reglable");
    assert!(
        err.to_string().contains("brouillon"),
        "message inattendu : {err}"
    );

    menage::facture(&pool, ENTITY, id)?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn refuse_les_reglements_invalides() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Paiement Refus";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let id = facture_de_1200(&pool, nom)?;

    // montant nul
    assert!(db::payment::create(
        &pool,
        ENTITY,
        &NewPayment {
            fk_facture: id,
            montant: Decimal::ZERO,
            fk_paiement: MODE_VIREMENT,
            // Ce test ne parle pas de banque : aucun compte ne correspond,
            // donc aucune ligne bancaire n'est ecrite et il n'y a pas de contention.
            fk_compte: Some(-1),
            ..Default::default()
        }
    )
    .is_err());

    // facture inconnue
    assert!(db::payment::create(
        &pool,
        ENTITY,
        &NewPayment {
            fk_facture: 999_999,
            montant: dec("10"),
            fk_paiement: MODE_VIREMENT,
            // Ce test ne parle pas de banque : aucun compte ne correspond,
            // donc aucune ligne bancaire n'est ecrite et il n'y a pas de contention.
            fk_compte: Some(-1),
            ..Default::default()
        }
    )
    .is_err());

    // facture deja soldee
    let p = db::payment::create(
        &pool,
        ENTITY,
        &NewPayment {
            fk_facture: id,
            montant: dec("1200"),
            fk_paiement: MODE_VIREMENT,
            // Ce test ne parle pas de banque : aucun compte ne correspond,
            // donc aucune ligne bancaire n'est ecrite et il n'y a pas de contention.
            fk_compte: Some(-1),
            ..Default::default()
        },
    )?;
    assert!(db::payment::create(
        &pool,
        ENTITY,
        &NewPayment {
            fk_facture: id,
            montant: dec("10"),
            fk_paiement: MODE_VIREMENT,
            // Ce test ne parle pas de banque : aucun compte ne correspond,
            // donc aucune ligne bancaire n'est ecrite et il n'y a pas de contention.
            fk_compte: Some(-1),
            ..Default::default()
        }
    )
    .is_err());

    db::payment::delete(&pool, ENTITY, p)?;
    menage::facture(&pool, ENTITY, id)?;
    nettoyer(&pool, nom)?;
    Ok(())
}
