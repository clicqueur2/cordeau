//! Journal d'impressions, sur la vraie base.

use std::error::Error;

use db::impression::{self, NouvelleImpression};
use mysql::prelude::Queryable;
use mysql::{params, Pool};

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

/// Efface les impressions d'un module de test : la table n'a pas d'entite, on
/// ne peut donc pas cloisonner — on nettoie par le nom du module.
fn nettoyer(pool: &Pool, module: &str) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE FROM llx_printing WHERE module = :module",
        params! { "module" => module },
    );
}

#[test]
fn une_impression_se_consigne_et_se_relit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let module = format!("zz-{}", std::process::id() % 9000);
    nettoyer(&pool, &module);

    let mut n = NouvelleImpression::nouvelle(&module, "HP LaserJet de recette");
    n.emplacement = "Atelier".to_string();
    n.copies = 3;
    let id = impression::enregistrer(&pool, &n)?;

    let recentes = impression::recentes(&pool, 50)?;
    let ligne = recentes
        .iter()
        .find(|i| i.rowid == id)
        .expect("l'impression consignee doit se relire");
    assert_eq!(ligne.copies, 3);
    assert_eq!(ligne.imprimante, "HP LaserJet de recette");
    assert_eq!(ligne.emplacement, "Atelier");
    assert!(ligne.date.is_some(), "la date est posee par la base");

    nettoyer(&pool, &module);
    Ok(())
}

#[test]
fn le_comptage_par_module_classe_les_plus_frequents() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let a = format!("zza{}", std::process::id() % 9000);
    let b = format!("zzb{}", std::process::id() % 9000);
    nettoyer(&pool, &a);
    nettoyer(&pool, &b);

    for _ in 0..3 {
        impression::enregistrer(&pool, &NouvelleImpression::nouvelle(&a, "HP"))?;
    }
    impression::enregistrer(&pool, &NouvelleImpression::nouvelle(&b, "HP"))?;

    let comptes = impression::par_module(&pool)?;
    let de_a = comptes.iter().find(|(m, _)| m == &a).map(|(_, n)| *n);
    let de_b = comptes.iter().find(|(m, _)| m == &b).map(|(_, n)| *n);
    assert_eq!(de_a, Some(3));
    assert_eq!(de_b, Some(1));

    nettoyer(&pool, &a);
    nettoyer(&pool, &b);
    Ok(())
}

#[test]
fn une_impression_invalide_est_refusee_avant_toute_ecriture() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let avant = impression::total(&pool)?;

    let mut n = NouvelleImpression::nouvelle("facture", "");
    n.imprimante = "  ".to_string();
    assert!(
        impression::enregistrer(&pool, &n).is_err(),
        "un nom vide ne se consigne pas"
    );

    let mut n = NouvelleImpression::nouvelle("facture", "HP");
    n.copies = 0;
    assert!(impression::enregistrer(&pool, &n).is_err());

    assert_eq!(
        impression::total(&pool)?,
        avant,
        "un refus ne doit rien ecrire"
    );
    Ok(())
}
