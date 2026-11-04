use std::error::Error;

use mysql::Pool;

fn dsn() -> Option<String> {
    std::env::var("DOLIBARR_DSN").ok()
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = match dsn() {
        Some(d) => d,
        None => {
            eprintln!("DOLIBARR_DSN absent : test d'integration ignore");
            return None;
        }
    };
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test d'integration ignore");
            None
        }
    }
}

const ENTITY: i64 = 1;

/// Supprime les restes d'un test precedent interrompu, pour que la suite soit
/// rejouable sans intervention manuelle.
fn nettoyer(pool: &Pool, noms: &[&str]) -> Result<(), Box<dyn Error>> {
    let listes = db::party::list(pool, ENTITY, 10_000, 0)?;
    for p in listes {
        if noms.contains(&p.nom.as_str()) {
            db::party::delete(pool, ENTITY, p.rowid)?;
        }
    }
    Ok(())
}

#[test]
fn cree_relit_liste_et_supprime_un_tiers() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &["SARL Test Dolibarr-RS", "Autre Test"])?;

    let mut new = db::party::NewParty::client("SARL Test Dolibarr-RS");
    new.zip = Some("69390".into());
    new.town = Some("Charly".into());
    new.email = Some("contact@exemple.test".into());

    let id = db::party::create(&pool, ENTITY, &new)?;
    assert!(id > 0);

    let lu = db::party::get(&pool, ENTITY, id)?.expect("tiers relu");
    assert_eq!(lu.nom, "SARL Test Dolibarr-RS");
    assert_eq!(lu.entity, ENTITY);
    // Un client Dolibarr vaut 3 (client + prospect) : card.php ecrit
    // client = (customer?1:0) + (prospect?2:0). Verifie dans EXTRACTIONS/tiers.md.
    assert_eq!(lu.client, 3);
    assert_eq!(lu.town.as_deref(), Some("Charly"));
    assert_eq!(lu.statut, 1);

    let code = lu.code_client.clone().expect("code client genere");
    assert!(
        code.starts_with("CU") && code.len() == 12 && code.as_bytes()[6] == b'-',
        "format de code client inattendu: {code}"
    );

    let listes = db::party::list(&pool, ENTITY, 1000, 0)?;
    assert!(listes.iter().any(|p| p.rowid == id));

    let autre = db::party::create(&pool, ENTITY, &db::party::NewParty::client("Autre Test"))?;
    let lu2 = db::party::get(&pool, ENTITY, autre)?.expect("second tiers");
    assert_ne!(lu2.code_client, lu.code_client, "codes clients distincts");

    db::party::set_statut(&pool, ENTITY, id, 0)?;
    assert_eq!(db::party::get(&pool, ENTITY, id)?.unwrap().statut, 0);

    db::party::delete(&pool, ENTITY, id)?;
    db::party::delete(&pool, ENTITY, autre)?;
    assert!(db::party::get(&pool, ENTITY, id)?.is_none());

    Ok(())
}

#[test]
fn refuse_un_tiers_sans_nom() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let vide = db::party::NewParty::client("   ");
    assert!(db::party::create(&pool, ENTITY, &vide).is_err());
    Ok(())
}

#[test]
fn isole_les_entites() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let id = db::party::create(
        &pool,
        ENTITY,
        &db::party::NewParty::client("Tiers Entite 1"),
    )?;
    assert!(
        db::party::get(&pool, 2, id)?.is_none(),
        "un tiers ne doit pas etre visible depuis une autre entite"
    );
    db::party::delete(&pool, ENTITY, id)?;
    Ok(())
}
