//! Immobilisations amortissables : plan pur, création, valeur nette, cession.
//!
//! Les tests purs tournent sans base. Les autres s'appuient sur `DOLIBARR_DSN`
//! et sont ignorés (franchement) quand la variable n'est pas posée : un test qui
//! échoue faute de base masquerait une vraie régression.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::immobilisation::{self, NewImmobilisation, PlanAmortissement};

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

// ---------------------------------------------------------------------------
// Calcul pur
// ---------------------------------------------------------------------------

/// Le cas de référence du module : 1 000 € sur 3 ans.
///
/// 1 000 / 3 = 333,333… : trois lignes à 333,33 donneraient 999,99 et
/// laisseraient un centime jamais amorti. La dernière annuité porte donc le
/// reliquat (333,34) et la somme tombe sur 1 000,00 **exactement** — c'est le
/// point qui justifie `plan()`.
#[test]
fn mille_euros_sur_trois_ans_le_reliquat_est_sur_la_derniere_annuite() {
    let annuites = immobilisation::plan(dec("1000"), 3, "2026-01-01");
    assert_eq!(annuites.len(), 3, "trois exercices");

    assert_eq!(annuites[0].dotation, dec("333.33"), "première annuité");
    assert_eq!(annuites[1].dotation, dec("333.33"), "deuxième annuité");
    assert_eq!(
        annuites[2].dotation,
        dec("333.34"),
        "la dernière porte le reliquat d'arrondi"
    );

    // Les deux premières dotations sont identiques : un amortissement linéaire
    // ne varie pas d'un exercice à l'autre, seul le reliquat s'ajoute à la fin.
    assert_eq!(annuites[0].dotation, annuites[1].dotation);

    assert_eq!(annuites[0].cumule, dec("333.33"), "cumul après un exercice");
    assert_eq!(annuites[1].cumule, dec("666.66"), "cumul après deux");
    assert_eq!(
        annuites[2].cumule,
        dec("1000.00"),
        "le cumul final égale la valeur d'acquisition"
    );

    let somme: Decimal = annuites.iter().map(|a| a.dotation).sum();
    assert_eq!(
        somme,
        dec("1000.00"),
        "la somme des dotations doit égaler la valeur au centime"
    );

    // Chaque annuité est numérotée dans l'ordre des exercices.
    for (i, a) in annuites.iter().enumerate() {
        assert_eq!(a.exercice as usize, i + 1);
    }
}

/// 10 000 € sur 4 ans : le quotient tombe juste (2 500), donc aucune annuité ne
/// s'écarte, et le cumul suit une progression régulière.
#[test]
fn dix_mille_euros_sur_quatre_ans_donnent_quatre_annuites_egales() {
    let annuites = immobilisation::plan(dec("10000"), 4, "2026-01-01");
    assert_eq!(annuites.len(), 4);

    for (i, a) in annuites.iter().enumerate() {
        assert_eq!(a.dotation, dec("2500.00"), "annuité {}", a.exercice);
        assert_eq!(
            a.cumule,
            dec("2500.00") * Decimal::from(i as i64 + 1),
            "cumul après {} exercices",
            a.exercice
        );
    }
    assert_eq!(annuites[3].cumule, dec("10000.00"));

    let somme: Decimal = annuites.iter().map(|a| a.dotation).sum();
    assert_eq!(somme, dec("10000.00"));
}

/// Les clôtures tombent d'année en année au même jour, et le jour d'ancre est
/// conservé quand l'année suivante est bissextile.
///
/// Une mise en service au 29 février 2028 ne peut clôturer au 29 février 2029
/// (le jour n'existe pas) : on ramène au 28. Mais **2032 est bissextile** et la
/// clôture doit revenir au 29 — sans ancre mémorisée, l'actif décrocherait
/// définitivement sur un jour plus court.
#[test]
fn les_dates_de_cloture_traversent_les_annees_bissextiles() {
    let annuites = immobilisation::plan(dec("4000"), 4, "2028-02-29");
    let dates: Vec<&str> = annuites.iter().map(|a| a.date_cloture.as_str()).collect();
    assert_eq!(
        dates,
        vec!["2029-02-28", "2030-02-28", "2031-02-28", "2032-02-29"]
    );

    // Un 28 février reste un 28 même quand l'année suivante est bissextile : on
    // ne décale pas un jour qui existe, sinon le plan glisserait tout seul.
    let annuites = immobilisation::plan(dec("900"), 4, "2028-02-28");
    let dates: Vec<&str> = annuites.iter().map(|a| a.date_cloture.as_str()).collect();
    assert_eq!(
        dates,
        vec!["2029-02-28", "2030-02-28", "2031-02-28", "2032-02-28"]
    );

    // Une mise en service en fin d'année traverse l'année sans se décaler.
    let annuites = immobilisation::plan(dec("1200"), 3, "2026-12-31");
    let dates: Vec<&str> = annuites.iter().map(|a| a.date_cloture.as_str()).collect();
    assert_eq!(dates, vec!["2027-12-31", "2028-12-31", "2029-12-31"]);

    // Un jour de fin de mois qui existe partout : aucune correction à appliquer.
    let annuites = immobilisation::plan(dec("900"), 2, "2027-01-31");
    assert_eq!(annuites[0].date_cloture, "2028-01-31");
    assert_eq!(annuites[1].date_cloture, "2029-01-31");
}

/// Un plan de zéro exercice est vide : ce n'est pas un calcul, et la fonction
/// pure ne panique pas. Le refus opposable est celui de `create`.
#[test]
fn duree_nulle_rend_un_plan_vide() {
    assert!(immobilisation::plan(dec("1000"), 0, "2026-01-01").is_empty());
    // Entrées inexploitables : vide aussi, jamais de panique.
    assert!(immobilisation::plan(Decimal::ZERO, 3, "2026-01-01").is_empty());
    assert!(immobilisation::plan(dec("-1000"), 3, "2026-01-01").is_empty());
    assert!(immobilisation::plan(dec("1000"), 3, "pas une date").is_empty());
    assert!(immobilisation::plan(dec("1000"), 3, "").is_empty());
}

/// La propriété « somme des dotations = valeur » ne doit pas dépendre d'un cas
/// favorable : on la vérifie sur des montants et des durées ingrats.
#[test]
fn la_somme_des_dotations_egale_toujours_la_valeur() {
    for (valeur, duree) in [
        ("1000", 3u32),
        ("10000", 4),
        ("1234.56", 7),
        ("999.99", 11),
        ("45000.55", 2),
        ("0.03", 3),
        ("0.02", 3),
        ("1234567.89", 20),
    ] {
        let annuites = immobilisation::plan(dec(valeur), duree, "2026-02-28");
        assert_eq!(annuites.len(), duree as usize, "{valeur} sur {duree} ans");

        let somme: Decimal = annuites.iter().map(|a| a.dotation).sum();
        assert_eq!(
            somme,
            dec(valeur),
            "somme des dotations pour {valeur} sur {duree} ans"
        );
        assert_eq!(
            annuites[duree as usize - 1].cumule,
            dec(valeur),
            "cumul final pour {valeur} sur {duree} ans"
        );

        // Aucune dotation négative, et le cumul ne recule jamais : un plan
        // d'amortissement ne peut pas rendre d'argent.
        let mut precedent = Decimal::ZERO;
        for a in &annuites {
            assert!(
                a.dotation >= Decimal::ZERO,
                "dotation négative ({valeur} / {duree} ans)"
            );
            assert!(a.cumule >= precedent, "cumul décroissant ({valeur})");
            precedent = a.cumule;
        }
    }
}

/// Le plan linéaire est bien celui que Dolibarr nomme `linear`, et le porteur de
/// plan (durée + méthode) donne exactement le résultat de la fonction pure.
#[test]
fn le_plan_lineaire_annonce_le_mode_comptable_attendu() {
    use db::immobilisation::Methode;

    assert_eq!(Methode::Lineaire.mode_comptable(), "linear");

    let porteur = PlanAmortissement::lineaire(3);
    assert_eq!(porteur.duree_annees, 3);
    assert_eq!(porteur.methode, Methode::Lineaire);
    assert_eq!(
        porteur.annuites(dec("1000"), "2026-01-01"),
        immobilisation::plan(dec("1000"), 3, "2026-01-01")
    );

    // Durée nulle : le porteur suit la fonction pure, plan vide.
    assert!(PlanAmortissement::lineaire(0)
        .annuites(dec("1000"), "2026-01-01")
        .is_empty());
}

// ---------------------------------------------------------------------------
// Base de données
// ---------------------------------------------------------------------------

/// Un auteur réel de l'entité 1 : `llx_asset.fk_user_creat` porte une clé
/// étrangère vers `llx_user`, on ne veut donc pas d'un identifiant inventé.
fn auteur(pool: &Pool) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity ORDER BY rowid LIMIT 1",
        params! { "entity" => ENTITY },
    )
    .ok()
    .flatten()
}

fn immobilisation_de_test(
    label: &str,
    valeur: &str,
    duree_annees: u32,
    fk_user_creat: i64,
) -> NewImmobilisation {
    NewImmobilisation {
        label: label.to_string(),
        valeur: dec(valeur),
        recovered_vat: None,
        date_acquisition: String::new(),
        date_mise_en_service: "2026-01-01".to_string(),
        duree_annees,
        fk_user_creat,
        note_public: None,
        note_private: None,
    }
}

fn nettoyer(pool: &Pool, fk_asset: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_asset_depreciation WHERE fk_asset = :asset",
        params! { "asset" => fk_asset },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_asset WHERE rowid = :asset",
        params! { "asset" => fk_asset },
    )?;
    Ok(())
}

/// Création, persistance du plan, lecture de l'actif et de sa valeur nette.
#[test]
fn cree_une_immobilisation_et_persiste_son_plan() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(auteur) = auteur(&pool) else {
        eprintln!("aucun utilisateur dans l'entité {ENTITY} : test ignoré");
        return Ok(());
    };

    let label = format!("ZZ-IMMO-CREATION-{}", std::process::id());
    let new = immobilisation_de_test(&label, "10000", 4, auteur);
    let fk_asset = immobilisation::create(&pool, ENTITY, &new)?;

    // Autant de lignes en base que d'exercices annoncés, et la somme tombe pile.
    let plan = immobilisation::plan_en_base(&pool, ENTITY, fk_asset)?;
    assert_eq!(plan.len(), 4, "quatre annuités persistées");
    let total: Decimal = plan.iter().map(|a| a.dotation).sum();
    assert_eq!(total, dec("10000.00"), "le plan amortit toute la valeur");
    assert_eq!(plan[0].dotation, dec("2500.00"));
    assert_eq!(
        plan[0].date, "2027-01-01",
        "clôture un an après la mise en service"
    );
    assert_eq!(plan[0].cumule, dec("2500.00"));
    assert_eq!(plan[3].cumule, dec("10000.00"));
    assert!(plan.iter().all(|a| a.rowid > 0));

    // La référence suit la numérotation IM de l'entité.
    let actif = immobilisation::get(&pool, ENTITY, fk_asset)?.expect("l'actif créé est relu");
    assert_eq!(actif.label, label);
    assert!(
        actif.reference.starts_with("IM") && actif.reference.len() == 6,
        "référence IMxxxx, trouvée {}",
        actif.reference
    );
    assert_eq!(actif.valeur, dec("10000.00"));
    assert_eq!(actif.duree_annees, 4, "durée déduite du nombre d'annuités");
    assert_eq!(actif.date_mise_en_service, "2026-01-01");
    assert_eq!(
        actif.valeur_nette,
        dec("10000.00"),
        "un bien neuf n'a rien amorti : le plan est une projection"
    );
    assert!(actif.date_cession.is_none());

    // La date d'acquisition vide a été reprise de la mise en service.
    let mut conn = pool.get_conn()?;
    let acquisition: Option<String> = conn.exec_first(
        "SELECT DATE_FORMAT(date_acquisition, '%Y-%m-%d') FROM llx_asset WHERE rowid = :asset",
        params! { "asset" => fk_asset },
    )?;
    assert_eq!(acquisition.as_deref(), Some("2026-01-01"));
    let statut: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_asset WHERE rowid = :asset",
        params! { "asset" => fk_asset },
    )?;
    assert_eq!(statut, Some(1), "l'actif est créé en service");

    // Il figure au registre de l'entité.
    let liste = immobilisation::list(&pool, ENTITY, 10_000, 0)?;
    assert!(
        liste.iter().any(|i| i.rowid == fk_asset),
        "l'immobilisation créée apparaît dans la liste"
    );

    // Il n'existe pas dans une autre entité : le plan suit l'actif.
    assert!(
        immobilisation::get(&pool, 999, fk_asset)?.is_none(),
        "une immobilisation ne se lit pas hors de son entité"
    );
    assert!(immobilisation::plan_en_base(&pool, 999, fk_asset)?.is_empty());

    nettoyer(&pool, fk_asset)?;
    Ok(())
}

/// Refus des entrées qui rendraient le plan faux, sans rien laisser en base.
#[test]
fn refuse_une_valeur_nulle_un_libelle_vide_et_une_duree_nulle() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(auteur) = auteur(&pool) else {
        eprintln!("aucun utilisateur dans l'entité {ENTITY} : test ignoré");
        return Ok(());
    };

    let mut nul = immobilisation_de_test("ZZ-IMMO-REFUS-NUL", "0", 3, auteur);
    let erreur = immobilisation::create(&pool, ENTITY, &nul).expect_err("valeur nulle");
    assert!(erreur.to_string().contains("valeur"), "message : {erreur}");

    nul.valeur = dec("-500");
    let erreur = immobilisation::create(&pool, ENTITY, &nul).expect_err("valeur négative");
    assert!(erreur.to_string().contains("valeur"), "message : {erreur}");

    let duree_nulle = immobilisation_de_test("ZZ-IMMO-REFUS-DUREE", "1000", 0, auteur);
    let erreur = immobilisation::create(&pool, ENTITY, &duree_nulle).expect_err("durée nulle");
    assert!(erreur.to_string().contains("durée"), "message : {erreur}");

    let libelle_vide = immobilisation_de_test("   ", "1000", 3, auteur);
    let erreur = immobilisation::create(&pool, ENTITY, &libelle_vide).expect_err("libellé vide");
    assert!(erreur.to_string().contains("libellé"), "message : {erreur}");

    // 256 octets : un de trop pour la colonne varchar(255).
    let trop_long = immobilisation_de_test(&"Z".repeat(256), "1000", 3, auteur);
    let erreur = immobilisation::create(&pool, ENTITY, &trop_long).expect_err("libellé trop long");
    assert!(erreur.to_string().contains("libellé"), "message : {erreur}");

    let mut date_fausse = immobilisation_de_test("ZZ-IMMO-REFUS-DATE", "1000", 3, auteur);
    date_fausse.date_mise_en_service = "01/01/2026".to_string();
    let erreur = immobilisation::create(&pool, ENTITY, &date_fausse).expect_err("date illisible");
    assert!(erreur.to_string().contains("date"), "message : {erreur}");

    // Aucun refus n'a écrit quoi que ce soit.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_asset WHERE entity = :entity AND label LIKE 'ZZ-IMMO-REFUS%'",
        params! { "entity" => ENTITY },
    )?;
    assert_eq!(restes, Some(0), "aucune immobilisation orpheline");
    Ok(())
}

/// La valeur nette suit les dotations : elle vaut la valeur d'acquisition tant
/// qu'aucune dotation n'existe, puis diminue exactement du montant de chaque
/// dotation jusqu'à zéro.
///
/// Le plan complet est écrit d'un coup à la création ; pour rejouer le passage
/// des exercices, on retire les annuités **par la fin**, comme le temps les
/// ajoute. Aucun calcul n'est refait à la main : `valeur_nette` additionne ce
/// qui reste en base.
#[test]
fn la_valeur_nette_decroit_a_mesure_que_les_dotations_s_ajoutent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(auteur) = auteur(&pool) else {
        eprintln!("aucun utilisateur dans l'entité {ENTITY} : test ignoré");
        return Ok(());
    };

    // 1 000 sur 3 ans : 333,33 + 333,33 + 333,34, le cas du reliquat.
    let label = format!("ZZ-IMMO-VNC-{}", std::process::id());
    let new = immobilisation_de_test(&label, "1000", 3, auteur);
    let fk_asset = immobilisation::create(&pool, ENTITY, &new)?;

    let plan = immobilisation::plan_en_base(&pool, ENTITY, fk_asset)?;
    assert_eq!(plan.len(), 3);

    // Le plan est une **projection** : tant que la date d'une dotation n'est
    // pas echue, elle n'est pas comptabilisee. Un bien mis en service
    // aujourd'hui vaut donc sa valeur d'acquisition, pas zero.
    assert_eq!(
        immobilisation::valeur_nette(&pool, ENTITY, fk_asset)?,
        dec("1000.00"),
        "aucune dotation n'est encore echue"
    );

    // On fait passer les exercices en antidatant les annuites, une par une.
    let mut conn = pool.get_conn()?;
    echeance_passee(&mut conn, plan[0].rowid, "2020-01-01")?;
    assert_eq!(
        immobilisation::valeur_nette(&pool, ENTITY, fk_asset)?,
        dec("666.67"),
        "valeur nette = valeur - dotation de l'exercice 1"
    );

    echeance_passee(&mut conn, plan[1].rowid, "2021-01-01")?;
    assert_eq!(
        immobilisation::valeur_nette(&pool, ENTITY, fk_asset)?,
        dec("333.34"),
        "deux exercices amortis : il reste le reliquat"
    );

    echeance_passee(&mut conn, plan[2].rowid, "2022-01-01")?;
    assert_eq!(
        immobilisation::valeur_nette(&pool, ENTITY, fk_asset)?,
        Decimal::ZERO,
        "trois exercices amortis : plus rien"
    );

    // Un actif inconnu se refuse au lieu de rendre zéro en silence.
    let erreur =
        immobilisation::valeur_nette(&pool, ENTITY, 999_999_999).expect_err("actif inexistant");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message : {erreur}"
    );

    nettoyer(&pool, fk_asset)?;
    Ok(())
}

/// Antidate une annuite : c'est ainsi qu'on fait passer un exercice sans
/// attendre trois ans. La valeur nette ne bouge pas parce qu'on supprime une
/// ligne, mais parce que sa date devient passee.
fn echeance_passee(
    conn: &mut mysql::PooledConn,
    rowid: i64,
    date: &str,
) -> Result<(), Box<dyn Error>> {
    use mysql::prelude::Queryable;
    conn.exec_drop(
        "UPDATE llx_asset_depreciation SET depreciation_date = :date WHERE rowid = :rowid",
        params! { "date" => date, "rowid" => rowid },
    )?;
    Ok(())
}

/// On ne cède pas un bien avant de l'avoir mis en service.
#[test]
fn refuse_une_cession_anterieure_a_la_mise_en_service() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(auteur) = auteur(&pool) else {
        eprintln!("aucun utilisateur dans l'entité {ENTITY} : test ignoré");
        return Ok(());
    };

    let label = format!("ZZ-IMMO-CESSION-REFUS-{}", std::process::id());
    let mut new = immobilisation_de_test(&label, "1000", 3, auteur);
    new.date_mise_en_service = "2026-03-01".to_string();
    let fk_asset = immobilisation::create(&pool, ENTITY, &new)?;

    let erreur = immobilisation::ceder(&pool, ENTITY, fk_asset, "2026-02-28", dec("500"))
        .expect_err("cession avant mise en service");
    assert!(erreur.to_string().contains("précède"), "message : {erreur}");

    // Le refus n'a rien écrit.
    let actif = immobilisation::get(&pool, ENTITY, fk_asset)?.expect("actif relu");
    assert!(actif.date_cession.is_none(), "aucune date de cession posée");

    // Une date de cession illisible est refusée aussi.
    let erreur = immobilisation::ceder(&pool, ENTITY, fk_asset, "15/06/2026", dec("500"))
        .expect_err("date illisible");
    assert!(erreur.to_string().contains("date"), "message : {erreur}");

    // Un prix négatif n'a pas de sens.
    let erreur = immobilisation::ceder(&pool, ENTITY, fk_asset, "2026-06-15", dec("-1"))
        .expect_err("prix négatif");
    assert!(erreur.to_string().contains("prix"), "message : {erreur}");

    nettoyer(&pool, fk_asset)?;
    Ok(())
}

/// Céder ne supprime rien : l'actif reste au registre, son plan aussi, et une
/// seconde cession est refusée plutôt que d'écraser la première pièce.
#[test]
fn une_immobilisation_cedee_reste_au_registre() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(auteur) = auteur(&pool) else {
        eprintln!("aucun utilisateur dans l'entité {ENTITY} : test ignoré");
        return Ok(());
    };

    let label = format!("ZZ-IMMO-CESSION-{}", std::process::id());
    let new = immobilisation_de_test(&label, "10000", 4, auteur);
    let fk_asset = immobilisation::create(&pool, ENTITY, &new)?;

    immobilisation::ceder(&pool, ENTITY, fk_asset, "2026-09-15", dec("1500"))?;

    let actif = immobilisation::get(&pool, ENTITY, fk_asset)?.expect("actif cédé relu");
    assert_eq!(actif.date_cession.as_deref(), Some("2026-09-15"));
    assert_eq!(actif.valeur, dec("10000.00"), "la valeur ne bouge pas");
    assert_eq!(
        actif.valeur_nette,
        dec("10000.00"),
        "le plan reste en place, mais aucune dotation n'est encore echue"
    );

    // Le plan est intact : quatre annuités, somme inchangée.
    let plan = immobilisation::plan_en_base(&pool, ENTITY, fk_asset)?;
    assert_eq!(plan.len(), 4, "les dotations passées sont des pièces");
    let total: Decimal = plan.iter().map(|a| a.dotation).sum();
    assert_eq!(total, dec("10000.00"));

    // Toujours au registre de l'entité.
    let liste = immobilisation::list(&pool, ENTITY, 10_000, 0)?;
    assert!(
        liste.iter().any(|i| i.rowid == fk_asset),
        "un actif cédé reste listé"
    );

    // Le prix de cession est bien posé en base.
    let mut conn = pool.get_conn()?;
    let prix: Option<String> = conn.exec_first(
        "SELECT CAST(disposal_amount_ht AS CHAR) FROM llx_asset WHERE rowid = :asset",
        params! { "asset" => fk_asset },
    )?;
    // `double(24,8)` casté en texte : on relit par `Decimal`, jamais par un f64,
    // et la forme exacte de la chaîne MySQL n'est pas un contrat.
    let prix = prix
        .and_then(|p| Decimal::from_str(p.trim()).ok())
        .unwrap_or_default();
    assert_eq!(prix, dec("1500.00"));

    // Une seconde cession écraserait la première : refusée.
    let erreur = immobilisation::ceder(&pool, ENTITY, fk_asset, "2026-10-01", dec("1200"))
        .expect_err("cession déjà faite");
    assert!(erreur.to_string().contains("déjà"), "message : {erreur}");

    let apres = immobilisation::get(&pool, ENTITY, fk_asset)?.expect("actif relu");
    assert_eq!(
        apres.date_cession.as_deref(),
        Some("2026-09-15"),
        "le refus laisse la première cession"
    );

    nettoyer(&pool, fk_asset)?;
    Ok(())
}
