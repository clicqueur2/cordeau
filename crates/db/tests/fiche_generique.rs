use std::error::Error;

use mysql::Pool;

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

/// La fiche generique : creer, relire, modifier, supprimer.
///
/// Ce chemin est celui des cinq ecrans de saisie. S'il casse, un utilisateur
/// ouvre une fiche vide ou supprime la mauvaise ligne.
#[test]
fn fiche_generique_aller_retour() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    // Un pret de test, unique par processus pour ne pas se marcher dessus.
    let etiquette = format!("RS-FICHE-{}", std::process::id());
    let valeurs = vec![
        ("label".to_string(), Some(etiquette.clone())),
        ("capital".to_string(), Some("1234,50".replace(',', "."))),
        ("rate".to_string(), Some("2.5".to_string())),
    ];
    let id = db::divers::inserer(
        &pool,
        "llx_loan",
        ENTITY,
        &valeurs,
        &[("datec", "NOW()"), ("paid", "0"), ("active", "1")],
    )? as i64;

    // Relecture : tout revient en texte, y compris les doubles et les dates.
    let lu = db::divers::lire_ligne_par(
        &pool,
        "llx_loan",
        "rowid",
        ENTITY,
        id,
        &["label", "capital", "rate", "paid"],
    )?
    .ok_or("ligne non relue")?;
    assert_eq!(lu[0].as_deref(), Some(etiquette.as_str()));
    assert_eq!(lu[1].as_deref(), Some("1234.50000000"));
    assert_eq!(lu[3].as_deref(), Some("0"));

    // Modification : le capital change, le libelle reste.
    let maj = db::divers::mettre_a_jour_par(
        &pool,
        "llx_loan",
        "rowid",
        ENTITY,
        id,
        &[
            ("label".to_string(), Some(etiquette.clone())),
            ("capital".to_string(), Some("2000".to_string())),
        ],
        &[],
    )?;
    assert_eq!(maj, 1);
    let relu = db::divers::lire_ligne_par(&pool, "llx_loan", "rowid", ENTITY, id, &["capital"])?
        .ok_or("ligne disparue apres modification")?;
    assert_eq!(relu[0].as_deref(), Some("2000.00000000"));

    // Suppression.
    let partis = db::divers::supprimer_ligne_par(&pool, "llx_loan", "rowid", ENTITY, id)?;
    assert_eq!(partis, 1);
    assert!(
        db::divers::lire_ligne_par(&pool, "llx_loan", "rowid", ENTITY, id, &["label"])?.is_none(),
        "la ligne survit a la suppression"
    );
    Ok(())
}

/// Une autre entite ne doit jamais voir la ligne.
#[test]
fn fiche_generique_respecte_l_entite() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let etiquette = format!("RS-ENTITE-{}", std::process::id());
    let id = db::divers::inserer(
        &pool,
        "llx_loan",
        ENTITY,
        &[("label".to_string(), Some(etiquette.clone()))],
        &[("datec", "NOW()"), ("rate", "1")],
    )? as i64;

    let ailleurs =
        db::divers::lire_ligne_par(&pool, "llx_loan", "rowid", ENTITY + 1, id, &["label"])?;
    assert!(ailleurs.is_none(), "la ligne fuit hors de son entite");

    db::divers::supprimer_ligne_par(&pool, "llx_loan", "rowid", ENTITY, id)?;
    Ok(())
}

/// Les garde-fous SQL : un nom de table ou de colonne bricole est refuse.
#[test]
fn fiche_generique_refuse_les_identifiants_bizarres() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    for table in ["llx_loan; DROP TABLE llx_loan", "llx loan", ""] {
        assert!(
            db::divers::supprimer_ligne_par(&pool, table, "rowid", ENTITY, 1).is_err(),
            "table acceptee a tort : {table}"
        );
    }
    assert!(db::divers::lire_ligne_par(
        &pool,
        "llx_loan",
        "rowid",
        ENTITY,
        1,
        &["label); DROP TABLE llx_user"]
    )
    .is_err());
    assert!(db::divers::mettre_a_jour_par(
        &pool,
        "llx_loan",
        "rowid",
        ENTITY,
        1,
        &[("label = 1, capital".to_string(), Some("x".to_string()))],
        &[]
    )
    .is_err());
    // Une expression d'office non prevue est refusee elle aussi.
    assert!(db::divers::mettre_a_jour_par(
        &pool,
        "llx_loan",
        "rowid",
        ENTITY,
        1,
        &[],
        &[("capital", "SLEEP(60)")]
    )
    .is_err());
}
