//! Grand livre derive : la partie double doit tomber juste.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;
use std::str::FromStr;

use db::invoice::{NewInvoice, NewInvoiceLine};

const ENTITY: i64 = 1;
const NOM: &str = "Tiers Grand Livre Test";

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

fn nettoyer(pool: &Pool) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => NOM },
    )?;
    for id in ids {
        let factures: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_facture WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for f in factures {
            // Purger aussi la comptabilite : une facture supprimee dont
            // l'ecriture reste laisse un grand livre faux, et la reference
            // reattribuee plus tard ferait croire l'ecriture deja ecrite.
            let ref_facture: Option<String> = conn.exec_first(
                "SELECT ref FROM llx_facture WHERE rowid = :f",
                params! { "f" => f },
            )?;
            let paiements: Vec<i64> = conn.exec(
                "SELECT fk_paiement FROM llx_paiement_facture WHERE fk_facture = :f",
                params! { "f" => f },
            )?;
            for p in paiements {
                let ref_paiement: Option<String> = conn.exec_first(
                    "SELECT ref FROM llx_paiement WHERE rowid = :p",
                    params! { "p" => p },
                )?;
                if let Some(r) = ref_paiement {
                    conn.exec_drop(
                        "DELETE FROM llx_accounting_bookkeeping \
                         WHERE entity = :entity AND doc_ref = :ref",
                        params! { "entity" => ENTITY, "ref" => r },
                    )?;
                }
                conn.exec_drop(
                    "DELETE FROM llx_paiement_facture WHERE fk_paiement = :p",
                    params! { "p" => p },
                )?;
                conn.exec_drop(
                    "DELETE FROM llx_bank WHERE fk_bordereau = :p",
                    params! { "p" => p },
                )?;
                conn.exec_drop(
                    "DELETE FROM llx_paiement WHERE rowid = :p",
                    params! { "p" => p },
                )?;
            }
            if let Some(r) = ref_facture {
                conn.exec_drop(
                    "DELETE FROM llx_accounting_bookkeeping \
                     WHERE entity = :entity AND doc_ref = :ref",
                    params! { "entity" => ENTITY, "ref" => r },
                )?;
            }
            conn.exec_drop(
                "DELETE FROM llx_facturedet WHERE fk_facture = :f",
                params! { "f" => f },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_facture WHERE rowid = :f",
                params! { "f" => f },
            )?;
        }
        db::party::delete(pool, ENTITY, id)?;
    }
    Ok(())
}

#[test]
fn la_partie_double_tombe_juste() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;

    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(NOM))?;
    let mut facture = NewInvoice::new(fk_soc);
    facture.datef = Some("2026-06-15".into());
    facture.lines = vec![
        NewInvoiceLine::new("Maconnerie", dec("1000"), dec("1"), dec("10")),
        NewInvoiceLine::new("Fourniture", dec("500"), dec("1"), dec("20")),
    ];
    let id = db::invoice::create(&pool, ENTITY, &facture)?;
    db::invoice::validate(&pool, ENTITY, id)?;

    let ecritures =
        db::grand_livre::ecritures(&pool, ENTITY, Some("2026-06-01"), Some("2026-06-30"))?;
    let debit: Decimal = ecritures.iter().map(|e| e.debit).sum();
    let credit: Decimal = ecritures.iter().map(|e| e.credit).sum();
    assert_eq!(
        debit, credit,
        "debit = credit, sinon ce n'est pas de la comptabilite"
    );
    assert_eq!(debit, dec("1700"), "1000 + 100 + 500 + 500 + 100 = 1700");

    // La balance se calcule sur les ecritures qu'on vient de lire : melanger
    // une lecture derivee et une balance qui lit la table comparerait deux
    // choses differentes.
    let balance = db::grand_livre::balance_de(&ecritures);
    let clients = balance
        .iter()
        .find(|b| b.compte == "411000")
        .expect("compte client present");
    assert_eq!(clients.debit, dec("1700"));
    assert_eq!(clients.credit, Decimal::ZERO);
    assert_eq!(clients.solde(), dec("1700"), "le client doit 1700");

    let tva = balance
        .iter()
        .find(|b| b.compte == "445710")
        .expect("compte de TVA collectee");
    assert_eq!(tva.credit, dec("200"), "100 de TVA a 10 % + 100 a 20 %");

    // Un reglement solde le compte client.
    let mut paiement = db::payment::NewPayment {
        fk_facture: id,
        montant: dec("1700"),
        fk_paiement: db::payment::MODE_VIREMENT,
        fk_compte: Some(-1),
        ..Default::default()
    };
    paiement.date = Some("2026-06-20".into());
    db::payment::create(&pool, ENTITY, &paiement)?;

    let balance = db::grand_livre::balance(&pool, ENTITY, Some("2026-06-01"), Some("2026-06-30"))?;
    let clients = balance
        .iter()
        .find(|b| b.compte == "411000")
        .expect("compte client present");
    assert_eq!(
        clients.solde(),
        Decimal::ZERO,
        "client solde apres reglement"
    );
    let banque = balance
        .iter()
        .find(|b| b.compte == "512000")
        .expect("compte banque present");
    assert_eq!(banque.solde(), dec("1700"), "la banque a encaisse 1700");

    let debit: Decimal = balance.iter().map(|b| b.debit).sum();
    let credit: Decimal = balance.iter().map(|b| b.credit).sum();
    assert_eq!(debit, credit, "la balance reste equilibree");

    nettoyer(&pool)?;
    Ok(())
}
