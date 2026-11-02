use std::error::Error;

use mysql::Pool;
use rust_decimal::Decimal;
use std::str::FromStr;

use db::proposal::{NewProposal, NewProposalLine};

const ENTITY: i64 = 1;

/// Chaque test a son propre tiers et son propre produit : les tests tournent en
/// parallele sur la meme base, un nettoyage commun se marcherait dessus.
const NOM_TIERS_A: &str = "Tiers Test Devis A";
const NOM_TIERS_B: &str = "Tiers Test Devis B";
const NOM_TIERS_D: &str = "Tiers Test Devis D";
const REF_PRODUIT_A: &str = "TEST-PROPAL-PROD-A";

fn pool_or_skip() -> Option<Pool> {
    let dsn = match std::env::var("DOLIBARR_DSN") {
        Ok(d) => d,
        Err(_) => {
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

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal de test valide")
}

/// `^PR\d{4}-\d{4}$` sans dependance a `regex`.
fn ref_conforme(r: &str) -> bool {
    let b = r.as_bytes();
    b.len() == 11
        && &b[0..2] == b"PR"
        && b[2..6].iter().all(u8::is_ascii_digit)
        && b[6] == b'-'
        && b[7..11].iter().all(u8::is_ascii_digit)
}

fn suffixe(r: &str) -> i64 {
    r[7..].parse().expect("suffixe numerique")
}

/// Supprime les devis du tiers de test puis le tiers : la base doit etre
/// rejouable meme apres une execution interrompue.
fn nettoyer(pool: &Pool, noms: &[&str], refs_produit: &[&str]) -> Result<(), Box<dyn Error>> {
    let tiers: Vec<i64> = db::party::list(pool, ENTITY, 10_000, 0)?
        .into_iter()
        .filter(|p| noms.contains(&p.nom.as_str()))
        .map(|p| p.rowid)
        .collect();

    if !tiers.is_empty() {
        let devis: Vec<i64> = db::proposal::list(pool, ENTITY, 10_000, 0)?
            .into_iter()
            .filter(|d| d.fk_soc.is_some_and(|s| tiers.contains(&s)))
            .map(|d| d.rowid)
            .collect();
        for id in devis {
            db::proposal::delete(pool, ENTITY, id)?;
        }
        for id in &tiers {
            db::party::delete(pool, ENTITY, *id)?;
        }
    }

    for r in refs_produit {
        if let Some(p) = db::product::by_ref(pool, ENTITY, r)? {
            db::product::delete(pool, ENTITY, p.rowid)?;
        }
    }
    Ok(())
}

fn tiers_de_test(pool: &Pool, nom: &str) -> Result<i64, Box<dyn Error>> {
    db::party::create(pool, ENTITY, &db::party::NewParty::client(nom))
}

#[test]
fn sections_sans_double_compte_dans_les_totaux() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &[NOM_TIERS_A], &[REF_PRODUIT_A])?;
    let fk_soc = tiers_de_test(&pool, NOM_TIERS_A)?;

    let mut a = NewProposalLine::new(
        "Depose carrelage existant",
        dec("32"),
        dec("28.50"),
        dec("10"),
    );
    a.section = Some("Depose carrelage".into());
    let mut b = NewProposalLine::new("Ragreage du support", dec("32"), dec("42"), dec("10"));
    b.section = Some("Depose carrelage".into());
    let mut c = NewProposalLine::new("Reprise etancheite", dec("1"), dec("1480"), dec("10"));
    c.section = Some("Reprise etancheite".into());

    let mut new = NewProposal::new(fk_soc);
    new.lines = vec![a, b, c];
    let id = db::proposal::create(&pool, ENTITY, &new)?;

    let lignes = db::proposal::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 5, "2 titres de section + 3 prestations");
    assert!(lignes[0].is_section());
    assert_eq!(lignes[0].label.as_deref(), Some("Depose carrelage"));
    assert!(!lignes[1].is_section());
    assert!(lignes[3].is_section());
    assert_eq!(lignes[3].label.as_deref(), Some("Reprise etancheite"));

    // Les titres ne comptent pas : 912 + 1344 + 1480 = 3736.
    let lu = db::proposal::get(&pool, ENTITY, id)?.expect("devis relu");
    assert_eq!(lu.total_ht, Some(dec("3736")));
    assert_eq!(lu.total_ttc, Some(dec("4109.6")));

    db::proposal::delete(&pool, ENTITY, id)?;
    Ok(())
}

#[test]
fn cree_un_devis_deux_lignes_et_relit_les_totaux() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &[NOM_TIERS_A], &[REF_PRODUIT_A])?;

    let fk_soc = tiers_de_test(&pool, NOM_TIERS_A)?;

    let mut prod = db::product::NewProduct::new(REF_PRODUIT_A, "Main d'oeuvre macon");
    prod.fk_product_type = db::product::TYPE_SERVICE;
    prod.price = Some(dec("48"));
    prod.tva_tx = Some(dec("10"));
    let fk_product = db::product::create(&pool, ENTITY, &prod)?;

    let mut ligne_service =
        NewProposalLine::new("Main d'oeuvre macon", dec("2"), dec("48"), dec("10"));
    ligne_service.fk_product = Some(fk_product);
    ligne_service.description = Some("Pose de bordures".into());

    let mut ligne_libre =
        NewProposalLine::new("Bordure granit 30 cm", dec("7"), dec("33.33"), dec("20"));
    ligne_libre.remise_percent = dec("5");
    ligne_libre.description = Some("Fourniture".into());

    let mut new = NewProposal::new(fk_soc);
    new.note_private = Some("note interne".into());
    new.note_public = Some("note client".into());
    // Date de validite : elle doit survivre a l'ecriture et a la relecture.
    new.fin_validite = Some("2026-12-31".into());
    new.lines = vec![ligne_service, ligne_libre];

    let id = db::proposal::create(&pool, ENTITY, &new)?;
    assert!(id > 0);

    // (a) reference au format de `mod_propale_marbre`
    let lu = db::proposal::get(&pool, ENTITY, id)?.expect("devis relu");
    assert_eq!(lu.entity, ENTITY);
    assert_eq!(lu.fk_soc, Some(fk_soc));
    assert_eq!(lu.fk_statut, 0, "un devis nait en brouillon");
    assert_eq!(lu.note_private.as_deref(), Some("note interne"));
    assert_eq!(lu.note_public.as_deref(), Some("note client"));
    assert_eq!(
        lu.fin_validite.as_deref(),
        Some("2026-12-31"),
        "la date de validite est relue"
    );
    assert_eq!(lu.remise_percent, Some(dec("0")));
    assert!(
        ref_conforme(&lu.ref_),
        "reference hors format PR AAMM - 0000 : {}",
        lu.ref_
    );
    assert!(lu.datec.is_some());
    let datec = lu.datec.clone().expect("datec");
    assert_eq!(lu.datep.as_deref(), Some(&datec[..10]));

    // totaux d'en-tete = somme des lignes
    let t1 = money::line_totals(&money::LineInput::new("2", "48", "0", "10"));
    let t2 = money::line_totals(&money::LineInput::new("7", "33.33", "5", "20"));
    assert_eq!(t2.total_ht, dec("221.64"), "valeur de reference attendue");
    let total_ht = money::sum(&[t1.total_ht, t2.total_ht]);
    let total_tva = money::sum(&[t1.total_vat, t2.total_vat]);
    let total_ttc = money::sum(&[t1.total_ttc, t2.total_ttc]);
    assert_eq!(lu.total_ht, Some(total_ht));
    assert_eq!(lu.total_tva, Some(total_tva));
    assert_eq!(lu.total_ttc, Some(total_ttc));

    // lignes relues dans l'ordre
    let lignes = db::proposal::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(lignes[0].rang, 1);
    assert_eq!(lignes[1].rang, 2);
    assert_eq!(lignes[0].label.as_deref(), Some("Main d'oeuvre macon"));
    assert_eq!(lignes[1].label.as_deref(), Some("Bordure granit 30 cm"));
    assert_eq!(lignes[0].description.as_deref(), Some("Pose de bordures"));
    assert_eq!(lignes[0].fk_product, Some(fk_product));
    assert_eq!(lignes[0].product_type, db::product::TYPE_SERVICE);
    assert_eq!(lignes[1].fk_product, None);
    assert_eq!(lignes[1].product_type, 0);

    // TVA de ligne exactement egale au calcul de `money`
    assert_eq!(lignes[0].total_tva, Some(t1.total_vat));
    assert_eq!(lignes[1].total_tva, Some(t2.total_vat));
    assert_eq!(lignes[1].total_ht, Some(t2.total_ht));
    assert_eq!(lignes[1].total_ttc, Some(t2.total_ttc));

    // `subprice` = `price` = prix unitaire HT
    assert_eq!(lignes[0].subprice, Some(t1.unit_price_ht));
    assert_eq!(lignes[0].price, Some(t1.unit_price_ht));
    assert_eq!(lignes[1].subprice, Some(t2.unit_price_ht));
    assert_eq!(lignes[1].remise_percent, Some(dec("5")));
    assert_eq!(lignes[1].qty, Some(dec("7")));

    // relecture isolee par entite
    assert!(
        db::proposal::get(&pool, 2, id)?.is_none(),
        "un devis ne doit pas etre visible depuis une autre entite"
    );
    assert!(db::proposal::lines(&pool, 2, id)?.is_empty());

    // liste : le devis cree y figure
    assert!(db::proposal::list(&pool, ENTITY, 1000, 0)?
        .iter()
        .any(|d| d.rowid == id));

    db::proposal::delete(&pool, ENTITY, id)?;
    db::product::delete(&pool, ENTITY, fk_product)?;
    db::party::delete(&pool, ENTITY, fk_soc)?;
    Ok(())
}

#[test]
fn numerote_sans_remise_a_zero() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &[NOM_TIERS_B], &[])?;
    let fk_soc = tiers_de_test(&pool, NOM_TIERS_B)?;

    let mut new = NewProposal::new(fk_soc);
    new.lines = vec![NewProposalLine::new(
        "Ligne",
        dec("1"),
        dec("10"),
        dec("20"),
    )];

    let id1 = db::proposal::create(&pool, ENTITY, &new)?;
    let id2 = db::proposal::create(&pool, ENTITY, &new)?;

    let r1 = db::proposal::get(&pool, ENTITY, id1)?
        .expect("devis 1")
        .ref_;
    let r2 = db::proposal::get(&pool, ENTITY, id2)?
        .expect("devis 2")
        .ref_;

    assert!(ref_conforme(&r1), "format inattendu : {r1}");
    assert!(ref_conforme(&r2), "format inattendu : {r2}");
    assert_ne!(r1, r2, "deux devis ne peuvent pas porter la meme reference");
    assert!(
        suffixe(&r2) > suffixe(&r1),
        "le compteur doit croitre (aucune remise a zero) : {r1} puis {r2}"
    );

    db::proposal::delete(&pool, ENTITY, id1)?;
    db::proposal::delete(&pool, ENTITY, id2)?;
    db::party::delete(&pool, ENTITY, fk_soc)?;
    Ok(())
}

#[test]
fn refuse_les_devis_invalides() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    // sans ligne
    let sans_ligne = NewProposal::new(1);
    assert!(db::proposal::create(&pool, ENTITY, &sans_ligne).is_err());

    // tiers inexistant (identifiant volontairement hors plage)
    let mut tiers_inexistant = NewProposal::new(2_000_000_000);
    tiers_inexistant.lines = vec![NewProposalLine::new(
        "Ligne",
        dec("1"),
        dec("10"),
        dec("20"),
    )];
    let e = db::proposal::create(&pool, ENTITY, &tiers_inexistant)
        .expect_err("un tiers inexistant doit etre refuse");
    assert!(
        e.to_string().contains("n'existe pas"),
        "message peu explicite : {e}"
    );

    // quantite nulle (ou negative)
    let mut qty_nulle = NewProposal::new(2_000_000_000);
    qty_nulle.lines = vec![NewProposalLine::new(
        "Ligne",
        dec("0"),
        dec("10"),
        dec("20"),
    )];
    assert!(db::proposal::create(&pool, ENTITY, &qty_nulle).is_err());

    let mut qty_negative = NewProposal::new(2_000_000_000);
    qty_negative.lines = vec![NewProposalLine::new(
        "Ligne",
        dec("-1"),
        dec("10"),
        dec("20"),
    )];
    assert!(db::proposal::create(&pool, ENTITY, &qty_negative).is_err());

    // tiers absent
    let mut sans_tiers = NewProposal::new(0);
    sans_tiers.lines = vec![NewProposalLine::new(
        "Ligne",
        dec("1"),
        dec("10"),
        dec("20"),
    )];
    assert!(db::proposal::create(&pool, ENTITY, &sans_tiers).is_err());

    Ok(())
}

#[test]
fn supprime_le_devis_et_ses_lignes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &[NOM_TIERS_D], &[])?;
    let fk_soc = tiers_de_test(&pool, NOM_TIERS_D)?;

    let mut new = NewProposal::new(fk_soc);
    new.lines = vec![
        NewProposalLine::new("Premiere", dec("1"), dec("10"), dec("20")),
        NewProposalLine::new("Seconde", dec("3"), dec("5.5"), dec("20")),
        NewProposalLine::new("Troisieme", dec("2"), dec("1"), dec("0")),
    ];

    let id = db::proposal::create(&pool, ENTITY, &new)?;
    assert_eq!(db::proposal::lines(&pool, ENTITY, id)?.len(), 3);

    db::proposal::delete(&pool, ENTITY, id)?;
    assert!(db::proposal::get(&pool, ENTITY, id)?.is_none());
    assert!(
        db::proposal::lines(&pool, ENTITY, id)?.is_empty(),
        "les lignes doivent partir avec l'en-tete"
    );

    db::party::delete(&pool, ENTITY, fk_soc)?;
    Ok(())
}
