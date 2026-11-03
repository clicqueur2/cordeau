//! Ecritures derivees d'une facture : le pont entre documents et partie double.
//!
//! La base de developpement est partagee : chaque test travaille dans **sa
//! propre entite** (`>= 900 000`), y cree son tiers et ses factures, et
//! supprime tout a la fin. Le prefixe `ZZ-ECR-` marque les fiches a nettoyer ;
//! il est aussi pose sur les references de commande pour retrouver une fuite.
//!
//! Aucun test ne compte les lignes de la base : les assertions portent sur les
//! identifiants crees par le test, jamais sur un `COUNT(*)` global.

use std::error::Error;

use comptabilite::GrandLivre;
use db::invoice::{NewInvoice, NewInvoiceLine};
use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

const PREFIXE: &str = "ZZ-ECR-";

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test d'integration ignore");
            None
        }
    }
}

fn dec(valeur: &str) -> Decimal {
    valeur.parse().expect("montant de test illisible")
}

/// Somme `(debit, credit)` d'un compte dans une ecriture.
fn total(ecriture: &comptabilite::Ecriture, compte: &str) -> (Decimal, Decimal) {
    ecriture
        .lignes
        .iter()
        .filter(|l| l.compte == compte)
        .fold((Decimal::ZERO, Decimal::ZERO), |(debit, credit), l| {
            (debit + l.debit, credit + l.credit)
        })
}

fn reference_vente(pool: &Pool, entity: i64, facture: i64) -> Result<String, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let r: Option<String> = conn.exec_first(
        "SELECT ref FROM llx_facture WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => facture },
    )?;
    Ok(r.unwrap_or_default())
}

fn reference_achat(pool: &Pool, entity: i64, facture: i64) -> Result<String, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let r: Option<String> = conn.exec_first(
        "SELECT ref FROM llx_facture_fourn WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => facture },
    )?;
    Ok(r.unwrap_or_default())
}

/// Supprime les factures de vente, les factures d'achat et les tiers du test.
///
/// L'ordre compte : les lignes portent une cle vers leur en-tete, et l'en-tete
/// une cle vers le tiers.
fn nettoyer(pool: &Pool, entity: i64, nom: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let societes: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => entity, "nom" => nom },
    )?;
    for soc in societes {
        let ventes: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_facture WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => entity, "soc" => soc },
        )?;
        for f in ventes {
            conn.exec_drop(
                "DELETE FROM llx_element_element WHERE fk_source = :f OR fk_target = :f",
                params! { "f" => f },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_facturedet WHERE fk_facture = :f",
                params! { "f" => f },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_facture WHERE rowid = :f",
                params! { "f" => f },
            )?;
        }
        let achats: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_facture_fourn WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => entity, "soc" => soc },
        )?;
        for f in achats {
            conn.exec_drop(
                "DELETE FROM llx_facture_fourn_det WHERE fk_facture_fourn = :f",
                params! { "f" => f },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_facture_fourn WHERE rowid = :f",
                params! { "f" => f },
            )?;
        }
        conn.exec_drop(
            "DELETE FROM llx_societe WHERE rowid = :rowid",
            params! { "rowid" => soc },
        )?;
    }
    Ok(())
}

/// Une condition de reglement du dictionnaire.
///
/// `llx_c_payment_term` est rempli par Dolibarr, pas par les tests, et
/// `db::invoice::create` cherche celle de l'entite : une entite neuve n'en a
/// aucune. On lui en donne une explicitement plutot que d'ecrire dans un
/// dictionnaire.
fn condition_reglement(pool: &Pool) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let id: Option<Option<i64>> = conn.exec_first(
        "SELECT MIN(rowid) FROM llx_c_payment_term WHERE active = 1",
        (),
    )?;
    id.flatten()
        .ok_or_else(|| "aucune condition de reglement active : test impossible".into())
}

/// Cree une facture de vente en brouillon.
fn vente_brouillon(
    pool: &Pool,
    entity: i64,
    fk_soc: i64,
    date: &str,
    lignes: Vec<NewInvoiceLine>,
) -> Result<i64, Box<dyn Error>> {
    let mut facture = NewInvoice::new(fk_soc);
    facture.datef = Some(date.to_string());
    facture.ref_client = Some(format!("{PREFIXE}{date}"));
    facture.fk_cond_reglement = Some(condition_reglement(pool)?);
    facture.lines = lignes;
    db::invoice::create(pool, entity, &facture)
}

/// Cree une facture de vente, la valide, et rend son identifiant.
fn vente_validee(
    pool: &Pool,
    entity: i64,
    fk_soc: i64,
    date: &str,
    lignes: Vec<NewInvoiceLine>,
) -> Result<i64, Box<dyn Error>> {
    let id = vente_brouillon(pool, entity, fk_soc, date, lignes)?;
    db::invoice::validate(pool, entity, id)?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// 1. Equilibre
// ---------------------------------------------------------------------------

#[test]
fn une_facture_de_vente_de_1200_est_equilibree() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_001;
    let nom = format!("{PREFIXE}Vente 1200");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, entity, &nom)?;
    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;

    // 1 000,00 HT a 20 % : 200,00 de TVA, 1 200,00 TTC.
    let id = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-04-10",
        vec![NewInvoiceLine::new(
            "Reprise de maconnerie",
            dec("1"),
            dec("1000"),
            dec("20"),
        )],
    )?;

    let ecriture = db::ecritures::facture_vente(&pool, entity, id)?;

    // Numero stable et date du document.
    let reference = reference_vente(&pool, entity, id)?;
    assert_eq!(ecriture.numero, format!("VE-{reference}"));
    assert_eq!(ecriture.date, "2026-04-10");
    assert!(
        ecriture.libelle.contains(&nom),
        "le libelle porte le tiers : {}",
        ecriture.libelle
    );

    // Le moteur accepte l'ecriture.
    comptabilite::verifier(&ecriture).expect("l'ecriture doit tenir");

    // 1 200 au debit, 1 000 + 200 au credit.
    let (debit, credit) = total(&ecriture, db::ecritures::COMPTE_CLIENTS);
    assert_eq!(debit, dec("1200.00"), "la creance vaut le TTC");
    assert_eq!(credit, Decimal::ZERO);
    let (debit, credit) = total(&ecriture, db::ecritures::COMPTE_VENTES);
    assert_eq!(debit, Decimal::ZERO);
    assert_eq!(credit, dec("1000.00"), "le produit vaut le HT");
    let (debit, credit) = total(&ecriture, db::ecritures::COMPTE_TVA_COLLECTEE);
    assert_eq!(debit, Decimal::ZERO);
    assert_eq!(credit, dec("200.00"), "la TVA collectee vaut 20 % du HT");

    let somme_debit: Decimal = ecriture.lignes.iter().map(|l| l.debit).sum();
    let somme_credit: Decimal = ecriture.lignes.iter().map(|l| l.credit).sum();
    assert_eq!(somme_debit, dec("1200.00"));
    assert_eq!(somme_credit, dec("1200.00"));
    assert_eq!(
        somme_debit, somme_credit,
        "le TTC egale la somme des debits"
    );

    // Dans le grand livre, la balance boucle.
    let mut livre = GrandLivre::nouveau();
    livre.ajouter(&ecriture)?;
    assert_eq!(livre.solde(db::ecritures::COMPTE_CLIENTS), dec("1200.00"));
    assert_eq!(livre.solde(db::ecritures::COMPTE_VENTES), dec("-1000.00"));
    assert_eq!(livre.total_debit(), livre.total_credit());

    nettoyer(&pool, entity, &nom)?;
    assert!(
        db::ecritures::facture_vente(&pool, entity, id).is_err(),
        "la facture a bien ete supprimee"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Numero stable
// ---------------------------------------------------------------------------

#[test]
fn deux_appels_donnent_le_meme_numero() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_002;
    let nom = format!("{PREFIXE}Numero Stable");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, entity, &nom)?;
    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;
    let id = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-05-04",
        vec![NewInvoiceLine::new(
            "Taille de pierre",
            dec("2"),
            dec("150"),
            dec("10"),
        )],
    )?;

    let premier = db::ecritures::facture_vente(&pool, entity, id)?;
    let second = db::ecritures::facture_vente(&pool, entity, id)?;

    assert_eq!(premier.numero, second.numero, "le numero ne bouge pas");
    assert_eq!(premier, second, "l'ecriture entiere est reproductible");
    assert!(premier.numero.starts_with("VE-"));
    // Deux passages du meme numero ne peuvent pas entrer deux fois.
    let mut livre = GrandLivre::nouveau();
    livre.ajouter(&premier)?;
    assert!(livre.ajouter(&second).is_err(), "pas de double saisie");

    nettoyer(&pool, entity, &nom)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. Ventilation de la TVA
// ---------------------------------------------------------------------------

#[test]
fn deux_taux_de_tva_font_deux_lignes() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_003;
    let nom = format!("{PREFIXE}Deux Taux");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, entity, &nom)?;
    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;

    // 500 a 20 % (100 de TVA) + 500 a 10 % (50 de TVA) = 1 000 HT, 1 150 TTC.
    let id = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-06-01",
        vec![
            NewInvoiceLine::new("Gros oeuvre", dec("1"), dec("500"), dec("20")),
            NewInvoiceLine::new("Enrobes", dec("1"), dec("500"), dec("10")),
        ],
    )?;

    let ecriture = db::ecritures::facture_vente(&pool, entity, id)?;
    comptabilite::verifier(&ecriture)?;

    // Une ligne de TVA par taux, sur le meme compte, dans l'ordre des taux
    // croissants (la ventilation est une table triee par taux).
    let lignes_tva: Vec<&comptabilite::Ligne> = ecriture
        .lignes
        .iter()
        .filter(|l| l.compte == db::ecritures::COMPTE_TVA_COLLECTEE)
        .collect();
    assert_eq!(lignes_tva.len(), 2, "un compte, deux taux, deux lignes");
    assert_eq!(lignes_tva[0].credit, dec("50.00"), "TVA a 10 % d'abord");
    assert!(
        lignes_tva[0].libelle.contains("TVA 10 %"),
        "le taux est dans le libelle : {}",
        lignes_tva[0].libelle
    );
    assert_eq!(lignes_tva[1].credit, dec("100.00"), "TVA a 20 % ensuite");
    assert!(
        lignes_tva[1].libelle.contains("TVA 20 %"),
        "le taux est dans le libelle : {}",
        lignes_tva[1].libelle
    );

    let (debit, _) = total(&ecriture, db::ecritures::COMPTE_CLIENTS);
    assert_eq!(debit, dec("1150.00"), "le TTC porte les deux taux");
    let (_, credit) = total(&ecriture, db::ecritures::COMPTE_VENTES);
    assert_eq!(credit, dec("1000.00"), "le HT est agrege, la TVA ventilee");

    nettoyer(&pool, entity, &nom)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Ce qui ne s'ecrit pas
// ---------------------------------------------------------------------------

#[test]
fn une_autre_entite_ne_voit_pas_la_facture() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_004;
    let nom = format!("{PREFIXE}Cloisonnement");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, entity, &nom)?;
    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;
    let id = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-07-01",
        vec![NewInvoiceLine::new("Pose", dec("1"), dec("100"), dec("20"))],
    )?;

    // Elle existe dans son entite.
    db::ecritures::facture_vente(&pool, entity, id)?;

    // Pas dans une entite qui n'existe pas (999 999 n'est utilise par aucun
    // autre test : `entity + 1` pourrait tomber sur une entite voisine vivante).
    let erreur = db::ecritures::facture_vente(&pool, 999_999, id)
        .expect_err("une facture d'une autre entite est introuvable");
    assert!(
        erreur.to_string().contains("introuvable"),
        "message inattendu : {erreur}"
    );
    let journal = db::ecritures::ecritures_du_journal(&pool, 999_999, "VE", 100)?;
    assert!(
        journal.is_empty(),
        "le journal d'une autre entite ne contient rien : {journal:?}"
    );

    nettoyer(&pool, entity, &nom)?;
    Ok(())
}

#[test]
fn un_brouillon_ne_s_ecrit_pas() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_005;
    let nom = format!("{PREFIXE}Brouillon");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, entity, &nom)?;
    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;

    let mut facture = NewInvoice::new(fk_soc);
    facture.datef = Some("2026-08-01".to_string());
    facture.fk_cond_reglement = Some(condition_reglement(&pool)?);
    facture.lines = vec![NewInvoiceLine::new(
        "Devis non signe",
        dec("1"),
        dec("100"),
        dec("20"),
    )];
    let brouillon = db::invoice::create(&pool, entity, &facture)?;

    let erreur = db::ecritures::facture_vente(&pool, entity, brouillon)
        .expect_err("un brouillon n'est pas un fait comptable");
    assert!(
        erreur.to_string().contains("brouillon"),
        "message inattendu : {erreur}"
    );

    // Le journal ne le ramasse pas non plus.
    db::invoice::validate(&pool, entity, brouillon)?;
    assert_eq!(
        db::ecritures::ecritures_du_journal(&pool, entity, "VE", 100)?.len(),
        1,
        "une fois validee, la facture entre dans le journal"
    );

    nettoyer(&pool, entity, &nom)?;
    Ok(())
}

#[test]
fn un_ttc_qui_ne_vaut_pas_ses_lignes_est_refuse() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_006;
    let nom = format!("{PREFIXE}Ttc Faux");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, entity, &nom)?;
    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;
    let id = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-09-01",
        vec![NewInvoiceLine::new(
            "Facture a corriger",
            dec("1"),
            dec("100"),
            dec("20"),
        )],
    )?;

    // Un centime de trop dans l'en-tete : la creance ne vaut plus ses lignes.
    {
        let mut conn = pool.get_conn()?;
        conn.exec_drop(
            "UPDATE llx_facture SET total_ttc = total_ttc + 0.01 \
             WHERE entity = :entity AND rowid = :rowid",
            params! { "entity" => entity, "rowid" => id },
        )?;
    }

    let erreur = db::ecritures::facture_vente(&pool, entity, id)
        .expect_err("un TTC faux ne doit pas produire d'ecriture");
    let message = erreur.to_string();
    assert!(
        message.contains("ne vaut pas ses lignes"),
        "message : {message}"
    );
    assert!(message.contains("0.01"), "l'ecart est chiffre : {message}");

    nettoyer(&pool, entity, &nom)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Le journal
// ---------------------------------------------------------------------------

#[test]
fn le_journal_des_ventes_suit_les_dates() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_007;
    let nom = format!("{PREFIXE}Journal");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, entity, &nom)?;
    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;

    // Saisies dans le desordre : fevrier, puis janvier.
    let fevrier = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-02-01",
        vec![NewInvoiceLine::new(
            "Fevrier",
            dec("1"),
            dec("200"),
            dec("20"),
        )],
    )?;
    let janvier = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-01-15",
        vec![NewInvoiceLine::new(
            "Janvier",
            dec("1"),
            dec("300"),
            dec("20"),
        )],
    )?;
    // Un quatrieme document, dans une autre entite : il ne doit pas apparaitre.
    let autre_entity: i64 = 900_017;
    let autre_nom = format!("{PREFIXE}Journal Autre");
    nettoyer(&pool, autre_entity, &autre_nom)?;
    let autre_soc = db::party::create(
        &pool,
        autre_entity,
        &db::party::NewParty::client(&autre_nom),
    )?;
    vente_validee(
        &pool,
        autre_entity,
        autre_soc,
        "2025-12-01",
        vec![NewInvoiceLine::new(
            "Ailleurs",
            dec("1"),
            dec("50"),
            dec("0"),
        )],
    )?;

    let journal = db::ecritures::ecritures_du_journal(&pool, entity, "VE", 100)?;
    assert_eq!(journal.len(), 2);
    assert_eq!(journal[0].date, "2026-01-15", "la plus ancienne d'abord");
    assert_eq!(journal[1].date, "2026-02-01");
    assert_eq!(
        journal[0].numero,
        format!("VE-{}", reference_vente(&pool, entity, janvier)?)
    );
    assert_eq!(
        journal[1].numero,
        format!("VE-{}", reference_vente(&pool, entity, fevrier)?)
    );

    // La casse et les espaces du code journal sont sans effet.
    assert_eq!(
        db::ecritures::ecritures_du_journal(&pool, entity, " ve ", 100)?.len(),
        2
    );

    // `limite` borne le nombre d'ecritures, et 0 n'en rend aucune.
    let une = db::ecritures::ecritures_du_journal(&pool, entity, "VE", 1)?;
    assert_eq!(une.len(), 1);
    assert_eq!(
        une[0].date, "2026-01-15",
        "la limite coupe apres la plus ancienne"
    );
    assert!(db::ecritures::ecritures_du_journal(&pool, entity, "VE", 0)?.is_empty());

    // Un journal que le pont ne produit pas rend une liste vide, pas une erreur.
    assert!(db::ecritures::ecritures_du_journal(&pool, entity, "BQ", 100)?.is_empty());
    // Un code vide, lui, est une faute.
    assert!(db::ecritures::ecritures_du_journal(&pool, entity, "   ", 100).is_err());

    nettoyer(&pool, entity, &nom)?;
    nettoyer(&pool, autre_entity, &autre_nom)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. Les achats
// ---------------------------------------------------------------------------

#[test]
fn une_facture_fournisseur_s_ecrit_en_achat() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_008;
    let nom = format!("{PREFIXE}Fournisseur");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, entity, &nom)?;
    let fk_soc = db::achat::creer_fournisseur(&pool, entity, &nom, None)?;

    // 54,60 a 20 % (10,92) + 120,00 a 10 % (12,00) = 174,60 HT, 197,52 TTC.
    let mut achat = db::achat_ecriture::NouvelleFactureAchat::new(fk_soc);
    achat.ref_supplier = Some(format!("{PREFIXE}FA-1"));
    achat.datef = Some("2026-03-10".to_string());
    achat.lines = vec![
        db::achat_ecriture::LigneAchat::new("Tasseaux", dec("12"), dec("4.55"), dec("20")),
        db::achat_ecriture::LigneAchat::new("Main d'oeuvre", dec("2.5"), dec("48"), dec("10")),
    ];
    let id = db::achat_ecriture::creer(&pool, entity, &achat)?;
    db::achat_ecriture::valider(&pool, entity, id)?;

    let ecriture = db::ecritures::facture_achat(&pool, entity, id)?;
    comptabilite::verifier(&ecriture)?;

    let reference = reference_achat(&pool, entity, id)?;
    assert_eq!(ecriture.numero, format!("AC-{reference}"));
    assert_eq!(ecriture.date, "2026-03-10");
    assert!(ecriture.lignes.iter().all(|l| l.journal == "AC"));

    // Charge et TVA au debit, dette au credit.
    let (debit, credit) = total(&ecriture, db::ecritures::COMPTE_ACHATS);
    assert_eq!(debit, dec("174.60"));
    assert_eq!(credit, Decimal::ZERO);
    let (debit, credit) = total(&ecriture, db::ecritures::COMPTE_TVA_DEDUCTIBLE);
    assert_eq!(debit, dec("22.92"), "les deux taux s'ajoutent");
    assert_eq!(credit, Decimal::ZERO);
    let (debit, credit) = total(&ecriture, db::ecritures::COMPTE_FOURNISSEURS);
    assert_eq!(debit, Decimal::ZERO);
    assert_eq!(credit, dec("197.52"));

    let somme_debit: Decimal = ecriture.lignes.iter().map(|l| l.debit).sum();
    let somme_credit: Decimal = ecriture.lignes.iter().map(|l| l.credit).sum();
    assert_eq!(somme_debit, somme_credit);
    assert_eq!(somme_credit, dec("197.52"), "le TTC egale la dette");

    // Le journal des achats la reprend, la vente n'en sait rien.
    let achats = db::ecritures::ecritures_du_journal(&pool, entity, "AC", 100)?;
    assert_eq!(achats.len(), 1);
    assert_eq!(achats[0].numero, ecriture.numero);
    assert!(db::ecritures::ecritures_du_journal(&pool, entity, "VE", 100)?.is_empty());

    nettoyer(&pool, entity, &nom)?;
    Ok(())
}
