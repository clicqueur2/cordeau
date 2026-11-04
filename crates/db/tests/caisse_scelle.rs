use std::error::Error;

use mysql::prelude::Queryable;
use mysql::{params, Pool};

const ENTITY: i64 = 1;
const JOUR: &str = "2026-11-04";

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

fn menage(pool: &Pool) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE FROM llx_const WHERE name LIKE 'DOLIBARR_RS_SCELLE_%'",
        (),
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_facture WHERE ref_ext LIKE 'POS-20261104-%'",
        (),
    );
}

/// Le scellé détecte une modification de montant et une vente supprimée.
///
/// C'est la seule propriété que ce module revendique : rendre une
/// falsification **visible**. Il ne l'empêche pas.
#[test]
fn le_scelle_rend_une_falsification_visible() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool);

    // La base impose les clés étrangères Dolibarr : fk_soc doit désigner un
    // tiers réel. On en crée un, supprimé à la fin.
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client("ZZ-SCELLE"))?;

    let mut conn = pool.get_conn()?;
    // Deux ventes de caisse du jour, posées directement : le scellé ne dépend
    // pas de la façon dont la vente a été créée, seulement de ce qui est en base.
    for (i, total) in [(1, "10.00"), (2, "20.00")] {
        conn.exec_drop(
            "INSERT INTO llx_facture \
             (entity, ref, fk_soc, type, datec, datef, fk_statut, paye, \
              total_ht, total_tva, total_ttc, ref_ext, fk_cond_reglement) \
             VALUES (:entity, :ref, :soc, 0, NOW(), :jour, 2, 1, \
                     :total, 0, :total, :ref_ext, 1)",
            params! {
                "entity" => ENTITY,
                "soc" => fk_soc,
                "ref" => format!("ZZSCELLE{i}"),
                "jour" => JOUR,
                "total" => total,
                "ref_ext" => format!("POS-20261104-{i}"),
            },
        )?;
    }

    // 1. Calcul : deux ventes, 30,00 au total.
    let avant = db::caisse_scelle::calculer(&pool, ENTITY, JOUR)?;
    assert_eq!(avant.ventes, 2);
    assert_eq!(avant.total, rust_decimal::Decimal::new(3000, 2));
    assert_eq!(
        avant.intact(),
        None,
        "aucun scelle pose : on ne compare pas"
    );

    // 2. Scellé posé.
    let pose = db::caisse_scelle::sceller(&pool, ENTITY, JOUR)?;
    assert_eq!(
        pose.empreinte_enregistree, None,
        "le scelle rendu est celui d'avant la pose"
    );
    let apres = db::caisse_scelle::calculer(&pool, ENTITY, JOUR)?;
    assert_eq!(apres.intact(), Some(true), "le scelle doit etre intact");

    // 3. On modifie un centime : le scellé doit crier.
    conn.exec_drop(
        "UPDATE llx_facture SET total_ttc = 10.01 WHERE ref_ext = 'POS-20261104-1'",
        (),
    )?;
    let falsifie = db::caisse_scelle::calculer(&pool, ENTITY, JOUR)?;
    assert_eq!(
        falsifie.intact(),
        Some(false),
        "un centime modifie doit invalider le scelle"
    );

    // 4. On restaure, puis on supprime une vente : le scellé doit crier aussi.
    conn.exec_drop(
        "UPDATE llx_facture SET total_ttc = 10.00 WHERE ref_ext = 'POS-20261104-1'",
        (),
    )?;
    assert_eq!(
        db::caisse_scelle::calculer(&pool, ENTITY, JOUR)?.intact(),
        Some(true),
        "restaure, le scelle redevient valide"
    );
    conn.exec_drop(
        "DELETE FROM llx_facture WHERE ref_ext = 'POS-20261104-2'",
        (),
    )?;
    assert_eq!(
        db::caisse_scelle::calculer(&pool, ENTITY, JOUR)?.intact(),
        Some(false),
        "une vente supprimee doit invalider le scelle"
    );

    // 5. Un scellé déjà posé n'est jamais écrasé : resceller ne répare pas.
    let rescelle = db::caisse_scelle::sceller(&pool, ENTITY, JOUR)?;
    assert_eq!(
        rescelle.empreinte_enregistree.is_some(),
        true,
        "un scelle existant est relu, pas remplace"
    );
    assert_eq!(rescelle.intact(), Some(false));

    let _ = db::party::delete(&pool, ENTITY, fk_soc);
    menage(&pool);
    println!("scelle : falsification detectee sur 3 cas");
    Ok(())
}
