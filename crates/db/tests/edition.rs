use std::error::Error;
use std::str::FromStr;

use db::proposal::{NewProposal, NewProposalLine};
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
            db::invoice::delete(pool, ENTITY, f.rowid)?;
        }
    }
    for p in db::proposal::list(pool, ENTITY, 1000, 0)? {
        if appartient(p.fk_soc, pool) {
            db::proposal::delete(pool, ENTITY, p.rowid)?;
        }
    }
    for p in db::party::list(pool, ENTITY, 10_000, 0)? {
        if p.nom == nom {
            db::party::delete(pool, ENTITY, p.rowid)?;
        }
    }
    Ok(())
}

#[test]
fn modifie_un_devis_brouillon_et_conserve_la_reference() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Modification Devis";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;

    let mut new = NewProposal::new(fk_soc);
    new.lines = vec![NewProposalLine::new(
        "Ligne initiale",
        dec("1"),
        dec("100"),
        dec("20"),
    )];
    let id = db::proposal::create(&pool, ENTITY, &new)?;
    let avant = db::proposal::get(&pool, ENTITY, id)?.expect("devis");
    assert_eq!(avant.total_ht, Some(dec("100")));

    // On remplace par deux lignes et une section.
    let mut a = NewProposalLine::new("Depose", dec("2"), dec("50"), dec("10"));
    a.section = Some("Section A".into());
    let mut b = NewProposalLine::new("Pose", dec("1"), dec("300"), dec("20"));
    b.section = Some("Section A".into());
    let mut modif = NewProposal::new(fk_soc);
    modif.note_private = Some("modifie".into());
    modif.lines = vec![a, b];
    db::proposal::update(&pool, ENTITY, id, &modif)?;

    let apres = db::proposal::get(&pool, ENTITY, id)?.expect("devis modifie");
    assert_eq!(apres.ref_, avant.ref_, "la reference ne change pas");
    assert_eq!(apres.total_ht, Some(dec("400")), "100 + 300");
    assert_eq!(apres.note_private.as_deref(), Some("modifie"));

    let lignes = db::proposal::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 3, "1 titre de section + 2 prestations");
    assert!(lignes[0].is_section());
    assert_eq!(lignes[1].label.as_deref(), Some("Depose"));

    db::proposal::delete(&pool, ENTITY, id)?;
    db::party::delete(&pool, ENTITY, fk_soc)?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn un_devis_valide_ne_se_modifie_plus() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Devis Verrouille";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;

    let mut new = NewProposal::new(fk_soc);
    new.lines = vec![NewProposalLine::new(
        "Travaux",
        dec("1"),
        dec("500"),
        dec("20"),
    )];
    let id = db::proposal::create(&pool, ENTITY, &new)?;

    db::proposal::validate(&pool, ENTITY, id)?;
    assert_eq!(db::proposal::get(&pool, ENTITY, id)?.unwrap().fk_statut, 1);
    assert!(
        db::proposal::validate(&pool, ENTITY, id).is_err(),
        "un devis deja valide ne se revalide pas"
    );

    let mut modif = NewProposal::new(fk_soc);
    modif.lines = vec![NewProposalLine::new("Autre", dec("1"), dec("1"), dec("20"))];
    let err = db::proposal::update(&pool, ENTITY, id, &modif).expect_err("doit refuser");
    assert!(
        err.to_string().contains("brouillon"),
        "message inattendu : {err}"
    );

    db::proposal::delete(&pool, ENTITY, id)?;
    db::party::delete(&pool, ENTITY, fk_soc)?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn un_devis_ne_se_facture_qu_une_fois() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Devis Double Facturation";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;

    let mut new = NewProposal::new(fk_soc);
    new.lines = vec![NewProposalLine::new(
        "Travaux",
        dec("1"),
        dec("1000"),
        dec("20"),
    )];
    let devis = db::proposal::create(&pool, ENTITY, &new)?;

    // Un devis doit etre SIGNE pour se facturer : c'est la regle de Dolibarr
    // (STATUS_SIGNED ouvre les conversions), verifiee dans EXTRACTIONS/devis.md.
    db::proposal::validate(&pool, ENTITY, devis)?;
    db::proposal::cloturer(&pool, ENTITY, devis, 2)?;

    let facture = db::invoice::create_from_proposal(&pool, ENTITY, devis)?;
    let err = db::invoice::create_from_proposal(&pool, ENTITY, devis)
        .expect_err("la seconde facturation doit etre refusee");
    assert!(
        err.to_string().contains("deja ete facture"),
        "message inattendu : {err}"
    );

    db::invoice::delete(&pool, ENTITY, facture)?;
    db::proposal::delete(&pool, ENTITY, devis)?;
    db::party::delete(&pool, ENTITY, fk_soc)?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn modifie_une_fiche_tiers() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Fiche Modifiable";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let id = db::party::create(&pool, ENTITY, &db::party::NewParty::client(nom))?;

    let mut modif = db::party::NewParty::client("Tiers Fiche Renommee");
    modif.zip = Some("69005".into());
    modif.town = Some("Saint-Cyr".into());
    db::party::update(&pool, ENTITY, id, &modif)?;

    let lu = db::party::get(&pool, ENTITY, id)?.expect("tiers");
    assert_eq!(lu.nom, "Tiers Fiche Renommee");
    assert_eq!(lu.town.as_deref(), Some("Saint-Cyr"));

    db::party::delete(&pool, ENTITY, id)?;
    nettoyer(&pool, nom)?;
    nettoyer(&pool, "Tiers Fiche Renommee")?;
    Ok(())
}
