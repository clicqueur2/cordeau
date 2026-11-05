//! Prêts bancaires : échéancier pur, création, règlement, capital restant dû.
//!
//! Les tests purs tournent sans base. Les autres s'appuient sur `DOLIBARR_DSN`
//! et sont ignorés (franchement) quand la variable n'est pas posée : un test qui
//! échoue faute de base masquerait une vraie régression.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::pret::{echeancier, Echeance, NewPret};

const ENTITY: i64 = 1;

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test ignoré");
            None
        }
    }
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// Somme des parts de capital, arrondie au centime comme le fait la base.
fn somme_capital(echeances: &[Echeance]) -> Decimal {
    echeances.iter().map(|e| e.capital).sum()
}

// ---------------------------------------------------------------------------
// Calcul pur
// ---------------------------------------------------------------------------

/// Le cas de référence du module : 10 000 € à 3 % sur 12 mois.
///
/// La mensualité vaut 846,94 € et les intérêts de la première échéance 25,00 €
/// (10 000 × 3 % / 12). Surtout, la somme des parts de capital tombe sur
/// 10 000,00 **exactement** : c'est la dernière échéance qui absorbe le
/// reliquat de l'arrondi, et c'est le point qui justifie `echeancier()`.
#[test]
fn dix_mille_a_trois_pour_cent_sur_douze_mois_solde_le_capital_au_centime() {
    let echeances = echeancier(dec("10000"), dec("3"), 12, "2026-01-15");
    assert_eq!(echeances.len(), 12, "douze échéances");

    assert_eq!(
        echeances[0].interets,
        dec("25.00"),
        "intérêts du premier mois"
    );
    assert_eq!(
        echeances[0].capital,
        dec("821.94"),
        "part de capital du premier mois"
    );
    assert_eq!(
        echeances[0].mensualite,
        dec("846.94"),
        "mensualité constante"
    );

    // Les onze premières échéances portent la même mensualité arrondie ; seule
    // la dernière s'en écarte, du reliquat de l'arrondi.
    for e in &echeances[..11] {
        assert_eq!(
            e.capital + e.interets,
            dec("846.94"),
            "échéance {}",
            e.numero
        );
    }

    assert_eq!(
        somme_capital(&echeances),
        dec("10000.00"),
        "la somme des parts de capital doit égaler le capital au centime"
    );

    // Chaque échéance est numérotée dans l'ordre des dates.
    for (i, e) in echeances.iter().enumerate() {
        assert_eq!(e.numero as usize, i + 1);
    }
}

/// Un taux nul est licite : la mensualité vaut `capital / n`, sans intérêts.
#[test]
fn un_taux_nul_donne_capital_divise_par_le_nombre_d_echeances() {
    let echeances = echeancier(dec("1200"), Decimal::ZERO, 12, "2026-03-01");
    assert_eq!(echeances.len(), 12);
    for e in &echeances {
        assert_eq!(e.interets, Decimal::ZERO, "aucun intérêt à taux nul");
        assert_eq!(e.capital, dec("100.00"), "1 200 / 12 = 100,00");
        assert_eq!(e.mensualite, dec("100.00"));
    }

    // Quand la division ne tombe pas juste, le reliquat va sur la dernière.
    let echeances = echeancier(dec("1000"), Decimal::ZERO, 12, "2026-03-01");
    assert_eq!(echeances[0].capital, dec("83.33"));
    assert_eq!(echeances[11].capital, dec("83.37"), "1000 − 11 × 83,33");
    assert_eq!(somme_capital(&echeances), dec("1000.00"));
}

/// Un échéancier de zéro échéance est vide : ce n'est pas un calcul, et la
/// fonction pure ne panique pas. Le refus opposable est celui de `create`.
#[test]
fn zero_echeance_rend_un_echeancier_vide() {
    assert!(echeancier(dec("10000"), dec("3"), 0, "2026-01-15").is_empty());
    // Entrées inexploitables : vide aussi, jamais de panique.
    assert!(echeancier(Decimal::ZERO, dec("3"), 12, "2026-01-15").is_empty());
    assert!(echeancier(dec("10000"), dec("-1"), 12, "2026-01-15").is_empty());
    assert!(echeancier(dec("10000"), dec("3"), 12, "pas une date").is_empty());
}

/// Les dates tombent de mois en mois, à partir du mois qui suit le départ, et
/// traversent l'année sans se décaler.
#[test]
fn les_dates_sont_mensuelles_et_traversent_l_annee() {
    let echeances = echeancier(dec("3000"), dec("2"), 4, "2026-11-15");
    let dates: Vec<&str> = echeances.iter().map(|e| e.date.as_str()).collect();
    assert_eq!(
        dates,
        vec!["2026-12-15", "2027-01-15", "2027-02-15", "2027-03-15"]
    );
}

/// Le jour est ancré sur celui du départ : un prêt signé un 31 janvier échoit
/// au 28 (ou 29) février, puis **de nouveau au 31** en mars. Sans ancre, le
/// prêt décrocherait définitivement sur un jour plus court.
#[test]
fn le_jour_du_31_revient_apres_un_mois_court() {
    let echeances = echeancier(dec("5000"), dec("1.5"), 4, "2027-01-31");
    let dates: Vec<&str> = echeances.iter().map(|e| e.date.as_str()).collect();
    assert_eq!(
        dates,
        vec!["2027-02-28", "2027-03-31", "2027-04-30", "2027-05-31"]
    );

    // Année bissextile : le 29 février existe.
    let echeances = echeancier(dec("5000"), dec("1.5"), 2, "2028-01-31");
    let dates: Vec<&str> = echeances.iter().map(|e| e.date.as_str()).collect();
    assert_eq!(dates, vec!["2028-02-29", "2028-03-31"]);
}

/// La propriété « somme des capitaux = capital » ne doit pas dépendre d'un cas
/// favorable : on la vérifie sur des montants et des durées ingrats.
#[test]
fn la_somme_des_capitaux_egale_toujours_le_capital() {
    for (capital, taux, nb) in [
        ("1234.56", "4.75", 37u32),
        ("999.99", "0.1", 7),
        ("250000", "1.27", 240),
        ("0.03", "19.9", 3),
        ("45000.55", "3.333", 61),
    ] {
        let echeances = echeancier(dec(capital), dec(taux), nb, "2026-02-28");
        assert_eq!(echeances.len(), nb as usize);
        assert_eq!(
            somme_capital(&echeances),
            dec(capital),
            "capital {capital} à {taux} % sur {nb} échéances"
        );
        assert!(
            echeances.iter().all(|e| e.capital > Decimal::ZERO),
            "aucune part de capital négative ni nulle ({capital} / {taux} % / {nb})"
        );
    }
}

// ---------------------------------------------------------------------------
// Base de données
// ---------------------------------------------------------------------------

fn pret_de_test(label: &str, capital: &str, taux: &str, nbterm: u32) -> NewPret {
    NewPret {
        label: label.to_string(),
        fk_bank: None,
        capital: dec(capital),
        insurance_amount: Decimal::ZERO,
        datestart: "2026-01-15".to_string(),
        dateend: None,
        nbterm,
        rate: dec(taux),
        fk_projet: None,
        note_private: None,
        note_public: None,
    }
}

fn nettoyer(pool: &Pool, fk_loan: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_payment_loan WHERE fk_loan = :loan",
        params! { "loan" => fk_loan },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_loan_schedule WHERE fk_loan = :loan",
        params! { "loan" => fk_loan },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_loan WHERE rowid = :loan",
        params! { "loan" => fk_loan },
    )?;
    Ok(())
}

/// Création, persistance de l'échéancier, lecture du prêt et du capital dû.
#[test]
fn cree_un_pret_et_persiste_son_echeancier() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let label = format!("ZZ-PRET-CREATION-{}", std::process::id());
    let new = pret_de_test(&label, "10000", "3", 12);
    let fk_loan = db::pret::create(&pool, ENTITY, &new)?;

    // Autant de lignes en base que d'échéances annoncées.
    let echeances = db::pret::echeances(&pool, ENTITY, fk_loan)?;
    assert_eq!(echeances.len(), 12, "douze échéances persistées");
    let total: Decimal = echeances.iter().map(|e| e.capital).sum();
    assert_eq!(total, dec("10000.00"), "l'échéancier soldé le capital");
    assert_eq!(
        echeances[0].datep, "2026-02-15",
        "première mensualité un mois après"
    );
    assert_eq!(echeances[0].capital, dec("821.94"));
    assert!(!echeances[0].regle, "aucune échéance réglée à la création");
    assert!(echeances[0].fk_payment_loan.is_none());

    // Rien n'est dû au départ : le capital restant est le capital entier.
    assert_eq!(
        db::pret::capital_restant(&pool, ENTITY, fk_loan)?,
        dec("10000.00")
    );

    let pret = db::pret::get(&pool, ENTITY, fk_loan)?.expect("le prêt créé est relu");
    assert_eq!(pret.label, label);
    assert_eq!(pret.capital, dec("10000.00"));
    assert_eq!(pret.taux, dec("3"));
    assert_eq!(pret.nbterm, 12);
    assert_eq!(pret.capital_restant_du, dec("10000.00"));
    assert_eq!(pret.prochaine_echeance.as_deref(), Some("2026-02-15"));
    assert!(pret.active);

    // Il figure dans la liste de l'entité.
    let liste = db::pret::list(&pool, ENTITY, 10_000, 0)?;
    assert!(
        liste.iter().any(|p| p.rowid == fk_loan),
        "le prêt créé apparaît dans la liste"
    );

    // Il n'existe pas dans une autre entité : l'échéancier suit le prêt.
    assert!(
        db::pret::get(&pool, 999, fk_loan)?.is_none(),
        "un prêt ne se lit pas hors de son entité"
    );
    assert!(db::pret::echeances(&pool, 999, fk_loan)?.is_empty());

    nettoyer(&pool, fk_loan)?;
    Ok(())
}

/// Un compte bancaire inconnu est refusé avant toute écriture.
#[test]
fn refuse_un_compte_bancaire_inconnu() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let mut new = pret_de_test("ZZ-PRET-BANQUE", "1000", "2", 3);
    new.fk_bank = Some(999_999_999);
    let erreur = db::pret::create(&pool, ENTITY, &new).expect_err("compte inexistant");
    assert!(
        erreur.to_string().contains("compte bancaire"),
        "message : {erreur}"
    );

    // Refus avant écriture : rien n'a été laissé derrière.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_loan WHERE entity = :entity AND label = :label",
        params! { "entity" => ENTITY, "label" => "ZZ-PRET-BANQUE" },
    )?;
    assert_eq!(restes, Some(0), "aucun prêt orphelin");
    Ok(())
}

/// Refus des entrées qui rendraient l'échéancier faux.
#[test]
fn refuse_les_entrees_invalides_a_la_creation() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    let mut nul = pret_de_test("ZZ-PRET-NUL", "0", "3", 12);
    let erreur = db::pret::create(&pool, ENTITY, &nul).expect_err("capital nul");
    assert!(erreur.to_string().contains("capital"), "message : {erreur}");

    nul.capital = dec("-500");
    let erreur = db::pret::create(&pool, ENTITY, &nul).expect_err("capital négatif");
    assert!(erreur.to_string().contains("capital"), "message : {erreur}");

    let sans_echeance = pret_de_test("ZZ-PRET-SANS-ECH", "1000", "3", 0);
    let erreur =
        db::pret::create(&pool, ENTITY, &sans_echeance).expect_err("zéro échéance refusée");
    assert!(
        erreur.to_string().contains("échéance"),
        "message : {erreur}"
    );

    let taux_negatif = pret_de_test("ZZ-PRET-TAUX", "1000", "-1", 12);
    let erreur = db::pret::create(&pool, ENTITY, &taux_negatif).expect_err("taux négatif");
    assert!(erreur.to_string().contains("taux"), "message : {erreur}");

    let mut date_fausse = pret_de_test("ZZ-PRET-DATE", "1000", "3", 12);
    date_fausse.datestart = "15/01/2026".to_string();
    let erreur = db::pret::create(&pool, ENTITY, &date_fausse).expect_err("date illisible");
    assert!(erreur.to_string().contains("date"), "message : {erreur}");

    let libelle_vide = pret_de_test("   ", "1000", "3", 12);
    let erreur = db::pret::create(&pool, ENTITY, &libelle_vide).expect_err("libellé vide");
    assert!(erreur.to_string().contains("libellé"), "message : {erreur}");
    Ok(())
}

/// Régler une échéance fait baisser le capital restant dû ; la régler deux fois
/// est refusé et laisse le capital inchangé.
#[test]
fn regle_une_echeance_puis_refuse_de_la_regler_deux_fois() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let label = format!("ZZ-PRET-REGLEMENT-{}", std::process::id());
    let new = pret_de_test(&label, "10000", "3", 12);
    let fk_loan = db::pret::create(&pool, ENTITY, &new)?;

    let echeances = db::pret::echeances(&pool, ENTITY, fk_loan)?;
    let premiere = &echeances[0];
    let part = premiere.capital;
    assert_eq!(part, dec("821.94"));

    let paiement = db::pret::regler(&pool, ENTITY, fk_loan, premiere.rowid, 0)?;
    assert!(paiement > 0, "le paiement porte un identifiant");

    // Le capital restant a baissé exactement de la part de capital réglée.
    assert_eq!(
        db::pret::capital_restant(&pool, ENTITY, fk_loan)?,
        dec("10000.00") - part
    );
    let apres = db::pret::echeances(&pool, ENTITY, fk_loan)?;
    assert!(apres[0].regle, "l'échéance est marquée réglée");
    assert_eq!(apres[0].fk_payment_loan, Some(paiement));
    assert_eq!(
        apres[1].datep, "2026-03-15",
        "l'échéance suivante prend la main"
    );
    let pret = db::pret::get(&pool, ENTITY, fk_loan)?.expect("prêt relu");
    assert_eq!(pret.prochaine_echeance.as_deref(), Some("2026-03-15"));
    assert_eq!(pret.capital_restant_du, dec("10000.00") - part);

    // Deuxième règlement de la même échéance : refusé, et sans effet.
    let erreur = db::pret::regler(&pool, ENTITY, fk_loan, premiere.rowid, 0)
        .expect_err("une échéance déjà réglée ne se règle pas deux fois");
    assert!(erreur.to_string().contains("déjà"), "message : {erreur}");
    assert_eq!(
        db::pret::capital_restant(&pool, ENTITY, fk_loan)?,
        dec("10000.00") - part,
        "le refus n'a rien écrit"
    );

    // Une échéance d'un autre prêt est refusée.
    let autre = db::pret::create(
        &pool,
        ENTITY,
        &pret_de_test("ZZ-PRET-AUTRE", "1000", "1", 2),
    )?;
    let erreur = db::pret::regler(&pool, ENTITY, autre, premiere.rowid, 0)
        .expect_err("une échéance ne se règle pas depuis un autre prêt");
    assert!(
        erreur.to_string().contains("n'appartient pas"),
        "message : {erreur}"
    );

    nettoyer(&pool, fk_loan)?;
    nettoyer(&pool, autre)?;
    Ok(())
}

/// Un prêt réglé en entier ne doit plus rien, et son capital restant tombe à
/// zéro — pas à un centime résiduel.
#[test]
fn un_pret_entierement_regle_ne_doit_plus_rien() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let new = pret_de_test("ZZ-PRET-SOLDE", "1000", "4.5", 3);
    let fk_loan = db::pret::create(&pool, ENTITY, &new)?;

    for echeance in db::pret::echeances(&pool, ENTITY, fk_loan)? {
        db::pret::regler(&pool, ENTITY, fk_loan, echeance.rowid, 0)?;
    }

    assert_eq!(
        db::pret::capital_restant(&pool, ENTITY, fk_loan)?,
        Decimal::ZERO
    );
    let pret = db::pret::get(&pool, ENTITY, fk_loan)?.expect("prêt soldé relu");
    assert_eq!(pret.capital_restant_du, Decimal::ZERO);
    assert!(
        pret.prochaine_echeance.is_none(),
        "plus aucune échéance à venir"
    );

    nettoyer(&pool, fk_loan)?;
    Ok(())
}
