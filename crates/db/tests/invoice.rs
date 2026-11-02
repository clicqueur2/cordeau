use std::error::Error;
use std::str::FromStr;

use db::invoice::{NewInvoice, NewInvoiceLine};
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

/// Supprime les documents puis le tiers d'un test. Chaque test a son propre nom
/// de tiers pour ne pas interferer avec les autres, qui tournent en parallele.
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

fn tiers(pool: &Pool, nom: &str) -> Result<i64, Box<dyn Error>> {
    db::party::create(pool, ENTITY, &db::party::NewParty::client(nom))
}

#[test]
fn cree_une_facture_avec_sections_et_totaux() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Facture Sections";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let mut a = NewInvoiceLine::new("Depose carrelage", dec("10"), dec("30"), dec("10"));
    a.section = Some("Depose".into());
    let mut b = NewInvoiceLine::new("Ragreage", dec("5"), dec("40"), dec("20"));
    b.section = Some("Reprise".into());

    let mut new = NewInvoice::new(fk_soc);
    new.lines = vec![a, b];
    let id = db::invoice::create(&pool, ENTITY, &new)?;

    let f = db::invoice::get(&pool, ENTITY, id)?.expect("facture relue");
    assert_eq!(f.ref_.len(), 11, "FA + AAAMM + - + 4 chiffres: {}", f.ref_);
    assert!(f.ref_.starts_with("FA"), "reference inattendue: {}", f.ref_);
    assert_eq!(f.fk_statut, db::invoice::STATUT_BROUILLON);
    assert!(f.est_brouillon());
    assert_eq!(f.total_ht, Some(dec("500")), "300 + 200, titres exclus");

    let lignes = db::invoice::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 4, "2 titres + 2 prestations");
    assert!(lignes[0].is_section());
    assert!(lignes[2].is_section());

    db::invoice::validate(&pool, ENTITY, id)?;
    assert_eq!(
        db::invoice::get(&pool, ENTITY, id)?.unwrap().fk_statut,
        db::invoice::STATUT_VALIDEE
    );
    assert!(
        db::invoice::validate(&pool, ENTITY, id).is_err(),
        "une facture validee ne se revalide pas"
    );

    db::invoice::delete(&pool, ENTITY, id)?;
    assert!(db::invoice::get(&pool, ENTITY, id)?.is_none());
    assert!(db::invoice::lines(&pool, ENTITY, id)?.is_empty());
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn convertit_un_devis_en_facture_au_meme_montant() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Facture Conversion";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let mut new = NewProposal::new(fk_soc);
    new.lines = vec![
        NewProposalLine::new("Bordure granit", dec("55"), dec("125.50"), dec("20")),
        NewProposalLine::new("Main d'oeuvre", dec("2"), dec("48"), dec("10")),
    ];
    let devis = db::proposal::create(&pool, ENTITY, &new)?;
    let devis_lu = db::proposal::get(&pool, ENTITY, devis)?.expect("devis");

    let facture = db::invoice::create_from_proposal(&pool, ENTITY, devis)?;
    let f = db::invoice::get(&pool, ENTITY, facture)?.expect("facture");
    assert_eq!(f.fk_soc, Some(fk_soc));
    assert_eq!(
        f.total_ht, devis_lu.total_ht,
        "le total HT doit etre recopie a l'identique"
    );
    assert_eq!(f.total_ttc, devis_lu.total_ttc);
    assert_eq!(db::invoice::lines(&pool, ENTITY, facture)?.len(), 2);

    // Le lien est enregistre pour que Dolibarr affiche la facture depuis le devis.
    let mut conn = pool.get_conn()?;
    use mysql::prelude::Queryable;
    let lien: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_element_element \
         WHERE sourcetype = 'propal' AND fk_source = ? \
           AND targettype = 'facture' AND fk_target = ?",
        (devis, facture),
    )?;
    assert!(lien.unwrap_or(0) >= 1, "lien devis -> facture absent");

    db::invoice::delete(&pool, ENTITY, facture)?;
    db::proposal::delete(&pool, ENTITY, devis)?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn refuse_les_factures_invalides() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Facture Refus";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    assert!(
        db::invoice::create(&pool, ENTITY, &NewInvoice::new(fk_soc)).is_err(),
        "une facture sans ligne doit etre refusee"
    );

    let mut sans_tiers = NewInvoice::new(999_999);
    sans_tiers.lines = vec![NewInvoiceLine::new("x", dec("1"), dec("1"), dec("20"))];
    assert!(db::invoice::create(&pool, ENTITY, &sans_tiers).is_err());

    let mut qte_nulle = NewInvoice::new(fk_soc);
    qte_nulle.lines = vec![NewInvoiceLine::new("x", dec("0"), dec("10"), dec("20"))];
    assert!(db::invoice::create(&pool, ENTITY, &qte_nulle).is_err());

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn numerote_les_factures_sans_doublon() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Facture Numero";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let mut new = NewInvoice::new(fk_soc);
    new.lines = vec![NewInvoiceLine::new("x", dec("1"), dec("10"), dec("20"))];

    // Dix creations concurrentes : aucun doublon de reference.
    let mut handles = Vec::new();
    for _ in 0..10 {
        let p = pool.clone();
        let n = new.clone();
        handles.push(std::thread::spawn(move || {
            db::invoice::create(&p, ENTITY, &n).map_err(|e| e.to_string())
        }));
    }

    let mut ids = Vec::new();
    for h in handles {
        ids.push(h.join().expect("thread")?);
    }

    let mut refs = Vec::new();
    for id in &ids {
        refs.push(db::invoice::get(&pool, ENTITY, *id)?.unwrap().ref_);
    }
    let uniques: std::collections::BTreeSet<&String> = refs.iter().collect();
    assert_eq!(uniques.len(), 10, "references dupliquees: {refs:?}");

    for id in &ids {
        db::invoice::delete(&pool, ENTITY, *id)?;
    }
    nettoyer(&pool, nom)?;
    Ok(())
}

/// L'echeance d'une facture se deduit de sa condition de reglement, pas d'un
/// delai code en dur : c'est la date qui figure sur le document.
#[test]
fn l_echeance_se_deduit_de_la_condition_de_reglement() -> Result<(), Box<dyn Error>> {
    let nom = "Tiers Facture Echeance";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    // La condition est retrouvee par son code : l'identifiant depend de la base.
    let cond = db::reglement::conditions(&pool, ENTITY)?
        .into_iter()
        .find(|c| c.code == "30DENDMONTH")
        .expect("condition 30DENDMONTH presente dans llx_c_payment_term");
    assert_eq!(cond.libelle_fr(), "30 jours fin de mois");

    let mut new = NewInvoice::new(fk_soc);
    new.datef = Some("2026-01-15".into());
    new.fk_cond_reglement = Some(cond.rowid);
    new.lines = vec![NewInvoiceLine::new(
        "Prestation",
        dec("1"),
        dec("100"),
        dec("20"),
    )];

    let id = db::invoice::create(&pool, ENTITY, &new)?;
    let f = db::invoice::get(&pool, ENTITY, id)?.expect("facture relue");
    assert_eq!(f.fk_cond_reglement, Some(cond.rowid));
    assert_eq!(
        f.date_lim_reglement.as_deref(),
        Some("2026-02-28"),
        "15/01 + 30 jours = 14/02, ramene a la fin de fevrier"
    );

    // Une date d'echeance explicite n'est jamais ecrasee par le calcul.
    let mut forcee = NewInvoice::new(fk_soc);
    forcee.datef = Some("2026-01-15".into());
    forcee.fk_cond_reglement = Some(cond.rowid);
    forcee.date_lim_reglement = Some("2026-03-31".into());
    forcee.lines = vec![NewInvoiceLine::new(
        "Prestation",
        dec("1"),
        dec("100"),
        dec("20"),
    )];
    let id2 = db::invoice::create(&pool, ENTITY, &forcee)?;
    assert_eq!(
        db::invoice::get(&pool, ENTITY, id2)?
            .unwrap()
            .date_lim_reglement
            .as_deref(),
        Some("2026-03-31")
    );

    db::invoice::delete(&pool, ENTITY, id)?;
    db::invoice::delete(&pool, ENTITY, id2)?;
    nettoyer(&pool, nom)?;
    Ok(())
}
