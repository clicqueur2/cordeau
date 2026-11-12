use std::error::Error;
use std::str::FromStr;

use db::invoice::{NewInvoice, NewInvoiceLine};
use mysql::prelude::Queryable;
use mysql::{params, Pool};
use rust_decimal::Decimal;

const ENTITY: i64 = 1;

mod menage;

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
            menage::facture(pool, ENTITY, f.rowid)?;
        }
    }
    for d in db::proposal::list(pool, ENTITY, 1000, 0)? {
        if appartient(d.fk_soc, pool) {
            db::proposal::delete(pool, ENTITY, d.rowid)?;
        }
    }
    for p in db::party::list(pool, ENTITY, 10_000, 0)? {
        if p.nom == nom {
            db::party::delete(pool, ENTITY, p.rowid)?;
        }
    }
    Ok(())
}

/// Un acompte encaisse sur un devis ne doit pas etre facture deux fois, et sa
/// presence ne doit plus empecher la facture finale de sortir.
///
/// Avant ce correctif : le lien devis -> facture de l'acompte faisait croire au
/// garde que le devis etait deja facture (« ce devis a deja ete facture »), et
/// la deduction n'existait que sur le chemin commande.
#[test]
fn un_acompte_de_devis_se_deduit_de_la_facture_finale() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Acompte Devis Test";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;

    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;

    // Devis : 2 x 500 HT a 20 % = 1 200 TTC.
    let mut devis = db::proposal::NewProposal::new(fk_soc);
    devis.lines = vec![db::proposal::NewProposalLine::new(
        "Travaux",
        dec("2"),
        dec("500"),
        dec("20"),
    )];
    let devis_id = db::proposal::create(&pool, ENTITY, &devis)?;
    db::proposal::validate(&pool, ENTITY, devis_id)?;
    // 2 = signe : seul un devis signe se facture.
    db::proposal::cloturer(&pool, ENTITY, devis_id, 2)?;

    // L'acompte de 30 %, tel que l'ecrit l'automatisation n8n : une facture
    // type acompte, marquee `PA-ACOMPTE-<devis>` et liee au devis.
    let mut acompte = NewInvoice::new(fk_soc);
    acompte.lines = vec![NewInvoiceLine::new(
        "Acompte 30 %",
        dec("1"),
        dec("300"),
        dec("20"),
    )];
    let acompte_id = db::invoice::create(&pool, ENTITY, &acompte)?;
    db::invoice::validate(&pool, ENTITY, acompte_id)?;
    {
        let mut conn = pool.get_conn()?;
        conn.exec_drop(
            "UPDATE llx_facture SET type = 3, ref_ext = :marque WHERE rowid = :id",
            params! { "marque" => format!("PA-ACOMPTE-{devis_id}"), "id" => acompte_id },
        )?;
        conn.exec_drop(
            "INSERT INTO llx_element_element (fk_source, sourcetype, fk_target, targettype) \
             VALUES (:devis, 'propal', :facture, 'facture')",
            params! { "devis" => devis_id, "facture" => acompte_id },
        )?;
    }

    // La facture finale doit sortir, et deduire l'acompte.
    let facture_id = db::invoice::create_from_proposal(&pool, ENTITY, devis_id)?;
    let facture = db::invoice::get(&pool, ENTITY, facture_id)?.expect("facture creee");
    let lignes = db::invoice::lines(&pool, ENTITY, facture_id)?;

    let deduction = lignes
        .iter()
        .find(|l| {
            l.label
                .as_deref()
                .unwrap_or("")
                .contains("Acompte déjà facturé")
        })
        .expect("la facture doit porter la ligne de deduction");
    assert_eq!(
        deduction.total_ht,
        Some(dec("-300")),
        "l'acompte de 300 HT doit etre deduit"
    );

    assert_eq!(
        facture.total_ht,
        Some(dec("700")),
        "1 000 HT de travaux - 300 HT d'acompte"
    );
    assert_eq!(facture.total_ttc, Some(dec("840")), "700 HT + 20 % de TVA");

    // Menage : la facture finale (validee ou non) et les pieces du test.
    menage::facture(&pool, ENTITY, facture_id)?;
    menage::facture(&pool, ENTITY, acompte_id)?;
    db::proposal::delete(&pool, ENTITY, devis_id)?;
    db::party::delete(&pool, ENTITY, fk_soc)?;
    Ok(())
}
