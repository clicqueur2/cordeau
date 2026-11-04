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

/// L'archive se verifie **sans la base** : c'est la propriete d'archivage.
///
/// Trois falsifications testees sur le fichier seul : montant change, ligne
/// retiree, ligne ajoutee.
#[test]
fn l_archive_se_verifie_hors_de_la_base() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool);
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client("ZZ-ARCHIVE"))?;

    let mut conn = pool.get_conn()?;
    for (i, total) in [(1, "10.00"), (2, "20.00"), (3, "5.50")] {
        conn.exec_drop(
            "INSERT INTO llx_facture \
             (entity, ref, fk_soc, type, datec, datef, fk_statut, paye, \
              total_ht, total_tva, total_ttc, ref_ext, fk_cond_reglement) \
             VALUES (:entity, :ref, :soc, 0, NOW(), :jour, 2, 1, \
                     :total, 0, :total, :ref_ext, 1)",
            params! {
                "entity" => ENTITY,
                "ref" => format!("ZZARCHIVE{i}"),
                "soc" => fk_soc,
                "jour" => JOUR,
                "total" => total,
                "ref_ext" => format!("POS-20261104-{i}"),
            },
        )?;
    }
    drop(conn);

    // 1. Archive produite depuis la base, puis verifiee deux fois sans elle.
    let fichier = db::caisse_scelle::archive(&pool, ENTITY, JOUR)?;
    assert!(
        fichier.contains("SCELLE|2026-11-04|"),
        "ligne SCELLE absente"
    );
    let v = db::caisse_scelle::verifier_archive(&fichier)?;
    assert!(v.intact, "l'archive neuve doit etre intacte");
    assert_eq!(v.ventes, 3);
    assert_eq!(v.total, rust_decimal::Decimal::new(3550, 2));
    assert_eq!(v.date, JOUR);

    // 2. Un montant change dans le fichier : detecte.
    //    Le montant vient de MySQL en `double(24,8)`, donc « 10.00000000 » :
    //    on reecrit le champ plutot que de chercher une chaine exacte.
    let mut champs: Vec<String> = fichier
        .lines()
        .next()
        .unwrap()
        .split('|')
        .map(String::from)
        .collect();
    champs[3] = "10.01000000".to_string();
    let premiere = champs.join("|");
    let falsifie = fichier.replacen(fichier.lines().next().unwrap(), &premiere, 1);
    let v2 = db::caisse_scelle::verifier_archive(&falsifie)?;
    assert!(!v2.intact, "un montant change doit invalider l'archive");

    // 3. Une ligne retiree : detecte (le SCELLE annonce 3 ventes).
    let lignes: Vec<&str> = fichier.lines().collect();
    let ampute = format!("{}\n{}\n", lignes[0], lignes[3]);
    let v3 = db::caisse_scelle::verifier_archive(&ampute)?;
    assert!(!v3.intact, "une ligne retiree doit invalider l'archive");

    // 4. Une ligne ajoutee : detectee.
    let ajoute = format!("{}\n{}\n", fichier.trim_end(), lignes[0]);
    let v4 = db::caisse_scelle::verifier_archive(&ajoute);
    assert!(
        v4.map(|r| !r.intact).unwrap_or(true),
        "une ligne ajoutee doit invalider l'archive"
    );

    // 5. Archive vide ou sans SCELLE : erreur explicite, pas un faux intact.
    assert!(db::caisse_scelle::verifier_archive("").is_err());

    let _ = db::party::delete(&pool, ENTITY, fk_soc);
    menage(&pool);
    println!("archive : 4 falsifications detectees hors base");
    Ok(())
}
