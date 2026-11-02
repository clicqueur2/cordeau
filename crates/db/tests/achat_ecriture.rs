//! Saisie d'une facture fournisseur : creation, relecture, validation.
//!
//! Les tests tournent en parallele sur la meme base : chaque test a son propre
//! fournisseur (nom unique) et ses assertions portent sur les identifiants
//! qu'il a crees, jamais sur un `COUNT(*)` global.

use std::error::Error;
use std::str::FromStr;

use db::achat_ecriture::{LigneAchat, NouvelleFactureAchat};
use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

const ENTITY: i64 = 1;

/// Prefixe commun aux fiches de test : le nettoyage s'appuie dessus.
const PREFIXE: &str = "ZZ Achat Ecriture";

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

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// Supprime les lignes de facture, puis les factures, puis la fiche du tiers.
///
/// L'ordre compte : `llx_facture_fourn` porte une cle etrangere vers
/// `llx_societe` (`fk_facture_fourn_fk_soc`) et `llx_facture_fourn_det` une cle
/// vers son en-tete. Supprimer le tiers d'abord echoue.
fn nettoyer(pool: &Pool, nom: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let societes: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    for soc in societes {
        let factures: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_facture_fourn WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => soc },
        )?;
        for f in factures {
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
            "DELETE FROM llx_societe WHERE rowid = :id",
            params! { "id" => soc },
        )?;
    }
    Ok(())
}

fn fournisseur(pool: &Pool, nom: &str) -> Result<i64, Box<dyn Error>> {
    db::achat::creer_fournisseur(pool, ENTITY, nom, None)
}

/// Reference lue en base (le module d'ecriture n'expose pas de lecteur
/// d'en-tete : `db::achat` s'en charge pour les listes).
fn entete(pool: &Pool, rowid: i64) -> Result<Entete, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let row: Option<mysql::Row> = conn.exec_first(
        "SELECT ref, ref_supplier, DATE_FORMAT(datef, '%Y-%m-%d'), fk_statut, \
                CAST(total_ht AS CHAR), CAST(total_tva AS CHAR), CAST(total_ttc AS CHAR), \
                note_private \
         FROM llx_facture_fourn WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => ENTITY, "rowid" => rowid },
    )?;
    let row = row.expect("facture fournisseur relue");
    let texte = |i: usize| -> Option<String> { row.get_opt::<String, _>(i).and_then(|r| r.ok()) };
    Ok(Entete {
        ref_: texte(0).unwrap_or_default(),
        ref_supplier: texte(1).unwrap_or_default(),
        datef: texte(2).unwrap_or_default(),
        statut: row.get_opt::<i64, _>(3).and_then(|r| r.ok()).unwrap_or(-1),
        total_ht: texte(4).and_then(|s| Decimal::from_str(s.trim()).ok()),
        total_tva: texte(5).and_then(|s| Decimal::from_str(s.trim()).ok()),
        total_ttc: texte(6).and_then(|s| Decimal::from_str(s.trim()).ok()),
        note: texte(7).unwrap_or_default(),
    })
}

struct Entete {
    ref_: String,
    ref_supplier: String,
    datef: String,
    statut: i64,
    total_ht: Option<Decimal>,
    total_tva: Option<Decimal>,
    total_ttc: Option<Decimal>,
    note: String,
}

/// `FF` + AAMM + `-` + 4 chiffres, soit 11 caracteres.
fn reference_valide(reference: &str) -> bool {
    reference.len() == 11
        && reference.starts_with("FF")
        && reference[2..6].bytes().all(|b| b.is_ascii_digit())
        && reference.as_bytes()[6] == b'-'
        && reference[7..].bytes().all(|b| b.is_ascii_digit())
}

#[test]
fn cree_une_facture_fournisseur_et_relit_les_totaux() -> Result<(), Box<dyn Error>> {
    let nom = format!("{PREFIXE} Totaux");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &nom)?;
    let fk_soc = fournisseur(&pool, &nom)?;

    let mut new = NouvelleFactureAchat::new(fk_soc);
    new.ref_supplier = Some("FA-2026-001".into());
    new.datef = Some("2026-03-10".into());
    new.note = Some("Livraison directe chantier".into());
    new.lines = vec![
        // 12 x 4,55 = 54,60 HT ; TTC 65,52 ; TVA 10,92
        LigneAchat::new("Tasseaux 45x70", dec("12"), dec("4.55"), dec("20")),
        // 2,5 x 48 = 120,00 HT ; TTC 132,00 ; TVA 12,00
        LigneAchat::new("Main d'oeuvre pose", dec("2.5"), dec("48"), dec("10")),
    ];

    let id = db::achat_ecriture::creer(&pool, ENTITY, &new)?;
    assert!(id > 0);

    let e = entete(&pool, id)?;
    assert!(
        reference_valide(&e.ref_),
        "reference inattendue: {}",
        e.ref_
    );
    assert!(
        e.ref_.starts_with("FF2603-"),
        "le AAMM vient de la date de facture: {}",
        e.ref_
    );
    assert_eq!(e.ref_supplier, "FA-2026-001");
    assert_eq!(e.datef, "2026-03-10");
    assert_eq!(e.note, "Livraison directe chantier");
    assert_eq!(
        e.statut,
        i64::from(db::achat::STATUT_BROUILLON),
        "une facture saisie est un brouillon"
    );

    // Totaux d'en-tete : 54,60 + 120,00 / 10,92 + 12,00 / 65,52 + 132,00.
    let (ht, tva, ttc) = db::achat_ecriture::total(&pool, ENTITY, id)?;
    assert_eq!(ht, dec("174.60"));
    assert_eq!(tva, dec("22.92"));
    assert_eq!(ttc, dec("197.52"));
    assert_eq!(ht + tva, ttc, "HT + TVA doit valoir TTC");
    assert_eq!(e.total_ht.map(money::round_money), Some(ht));
    assert_eq!(e.total_tva.map(money::round_money), Some(tva));
    assert_eq!(e.total_ttc.map(money::round_money), Some(ttc));

    // Relecture des lignes, dans l'ordre de saisie.
    let lignes = db::achat_ecriture::lignes(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(lignes[0].label, "Tasseaux 45x70");
    assert_eq!(lignes[0].qty, dec("12"));
    assert_eq!(lignes[0].unit_price_ht, dec("4.55"));
    assert_eq!(lignes[0].tva_tx, dec("20"));
    assert_eq!(lignes[1].label, "Main d'oeuvre pose");
    assert_eq!(lignes[1].qty, dec("2.5"));
    assert_eq!(lignes[1].unit_price_ht, dec("48"));
    assert_eq!(lignes[1].tva_tx, dec("10"));

    // La facture apparait dans la liste des achats, qui est la lecture
    // existante (`db::achat`) : c'est ce que l'ecran affichera.
    let facture = db::achat::factures_fournisseur(&pool, ENTITY, 200, 0)?
        .into_iter()
        .find(|f| f.rowid == id)
        .expect("la facture saisie doit apparaitre dans la liste des achats");
    assert_eq!(facture.ref_, e.ref_);
    assert_eq!(facture.fk_soc, fk_soc);
    assert_eq!(facture.total_ttc.map(money::round_money), Some(ttc));
    assert!(facture.est_brouillon());

    // Une autre entite ne voit ni la facture ni ses lignes.
    assert!(db::achat_ecriture::lignes(&pool, 999_999, id)?.is_empty());
    assert!(db::achat_ecriture::total(&pool, 999_999, id).is_err());

    nettoyer(&pool, &nom)?;
    assert!(
        db::achat_ecriture::total(&pool, ENTITY, id).is_err(),
        "la facture a bien ete supprimee"
    );
    Ok(())
}

#[test]
fn numerote_les_factures_fournisseurs_sans_doublon() -> Result<(), Box<dyn Error>> {
    let nom = format!("{PREFIXE} Numeros");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &nom)?;
    let fk_soc = fournisseur(&pool, &nom)?;

    // Cinq saisies concurrentes : le verrou consultatif doit garantir cinq
    // numeros distincts. Chaque facture a sa reference fournisseur, sans quoi
    // `uk_facture_fourn_ref_supplier` refuserait la deuxieme.
    let mut handles = Vec::new();
    for i in 0..5 {
        let mut new = NouvelleFactureAchat::new(fk_soc);
        new.ref_supplier = Some(format!("NUM-{i}"));
        new.lines = vec![LigneAchat::new(
            "Fourniture",
            dec("1"),
            dec("10"),
            dec("20"),
        )];
        let p = pool.clone();
        handles.push(std::thread::spawn(move || {
            db::achat_ecriture::creer(&p, ENTITY, &new).map_err(|e| e.to_string())
        }));
    }

    let mut ids = Vec::new();
    for h in handles {
        ids.push(h.join().expect("thread")?);
    }
    assert_eq!(ids.len(), 5);

    let mut references = Vec::new();
    for id in &ids {
        let e = entete(&pool, *id)?;
        assert!(
            reference_valide(&e.ref_),
            "reference inattendue: {}",
            e.ref_
        );
        references.push(e.ref_);
    }
    let distinctes: std::collections::BTreeSet<&String> = references.iter().collect();
    assert_eq!(distinctes.len(), 5, "references dupliquees: {references:?}");

    nettoyer(&pool, &nom)?;
    Ok(())
}

#[test]
fn valide_une_facture_fournisseur_une_seule_fois() -> Result<(), Box<dyn Error>> {
    let nom = format!("{PREFIXE} Validation");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &nom)?;
    let fk_soc = fournisseur(&pool, &nom)?;

    let mut new = NouvelleFactureAchat::new(fk_soc);
    new.ref_supplier = Some("VAL-1".into());
    new.lines = vec![LigneAchat::new("Sable 0/4", dec("3"), dec("25"), dec("20"))];
    let id = db::achat_ecriture::creer(&pool, ENTITY, &new)?;

    db::achat_ecriture::valider(&pool, ENTITY, id)?;
    assert_eq!(
        entete(&pool, id)?.statut,
        i64::from(db::achat::STATUT_VALIDEE)
    );
    // La validation ne touche pas aux montants.
    assert_eq!(db::achat_ecriture::total(&pool, ENTITY, id)?.2, dec("90"));

    assert!(
        db::achat_ecriture::valider(&pool, ENTITY, id).is_err(),
        "une facture deja validee ne se revalide pas"
    );

    // Une facture vide ne peut pas etre validee : on force le cas en supprimant
    // les lignes d'un brouillon.
    let mut vide = NouvelleFactureAchat::new(fk_soc);
    vide.ref_supplier = Some("VAL-2".into());
    vide.lines = vec![LigneAchat::new("Ciment", dec("1"), dec("10"), dec("20"))];
    let id_vide = db::achat_ecriture::creer(&pool, ENTITY, &vide)?;
    {
        let mut conn = pool.get_conn()?;
        conn.exec_drop(
            "DELETE FROM llx_facture_fourn_det WHERE fk_facture_fourn = :id",
            params! { "id" => id_vide },
        )?;
    }
    assert!(
        db::achat_ecriture::valider(&pool, ENTITY, id_vide).is_err(),
        "une facture sans ligne ne peut pas etre validee"
    );
    assert_eq!(entete(&pool, id_vide)?.statut, 0);

    // Une facture d'une autre entite est introuvable.
    assert!(db::achat_ecriture::valider(&pool, 999_999, id).is_err());

    nettoyer(&pool, &nom)?;
    Ok(())
}

#[test]
fn refuse_les_factures_fournisseurs_invalides() -> Result<(), Box<dyn Error>> {
    let nom = format!("{PREFIXE} Refus");
    let nom_client = format!("{PREFIXE} Client");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, &nom)?;
    nettoyer(&pool, &nom_client)?;
    let fk_soc = fournisseur(&pool, &nom)?;

    // Sans ligne.
    assert!(
        db::achat_ecriture::creer(&pool, ENTITY, &NouvelleFactureAchat::new(fk_soc)).is_err(),
        "une facture sans ligne doit etre refusee"
    );

    // Quantite nulle ou negative.
    for qty in ["0", "-1"] {
        let mut new = NouvelleFactureAchat::new(fk_soc);
        new.lines = vec![LigneAchat::new("x", dec(qty), dec("10"), dec("20"))];
        assert!(
            db::achat_ecriture::creer(&pool, ENTITY, &new).is_err(),
            "une quantite de {qty} doit etre refusee"
        );
    }

    // Libelle vide.
    let mut sans_libelle = NouvelleFactureAchat::new(fk_soc);
    sans_libelle.lines = vec![LigneAchat::new("   ", dec("1"), dec("10"), dec("20"))];
    assert!(db::achat_ecriture::creer(&pool, ENTITY, &sans_libelle).is_err());

    // Tiers inexistant.
    let mut inconnu = NouvelleFactureAchat::new(999_999);
    inconnu.lines = vec![LigneAchat::new("x", dec("1"), dec("10"), dec("20"))];
    assert!(db::achat_ecriture::creer(&pool, ENTITY, &inconnu).is_err());

    // Tiers qui existe mais n'est pas fournisseur : la case fournisseur de la
    // fiche n'est pas cochee.
    let fk_client = db::party::create(&pool, ENTITY, &db::party::NewParty::client(&nom_client))?;
    let mut mauvais = NouvelleFactureAchat::new(fk_client);
    mauvais.ref_supplier = Some("CLI-1".into());
    mauvais.lines = vec![LigneAchat::new("x", dec("1"), dec("10"), dec("20"))];
    let erreur = db::achat_ecriture::creer(&pool, ENTITY, &mauvais)
        .expect_err("un client n'est pas un fournisseur");
    assert!(
        erreur.to_string().contains("n'est pas un fournisseur"),
        "message inattendu: {erreur}"
    );

    // Date de facture illisible.
    let mut mauvaise_date = NouvelleFactureAchat::new(fk_soc);
    mauvaise_date.datef = Some("10/03/2026".into());
    mauvaise_date.lines = vec![LigneAchat::new("x", dec("1"), dec("10"), dec("20"))];
    assert!(db::achat_ecriture::creer(&pool, ENTITY, &mauvaise_date).is_err());

    // Deux factures du meme fournisseur avec la meme reference fournisseur :
    // `uk_facture_fourn_ref_supplier` l'interdit, et le module le dit.
    let mut premiere = NouvelleFactureAchat::new(fk_soc);
    premiere.ref_supplier = Some("DUP-1".into());
    premiere.lines = vec![LigneAchat::new("x", dec("1"), dec("10"), dec("20"))];
    db::achat_ecriture::creer(&pool, ENTITY, &premiere)?;
    let doublon = db::achat_ecriture::creer(&pool, ENTITY, &premiere)
        .expect_err("une reference fournisseur ne peut pas se repeter");
    assert!(
        doublon.to_string().contains("DUP-1"),
        "message inattendu: {doublon}"
    );

    // Deux factures sans reference fournisseur : la chaine vide est une valeur,
    // donc la deuxieme est refusee elle aussi (piege du schema Dolibarr).
    let mut sans_ref = NouvelleFactureAchat::new(fk_soc);
    sans_ref.ref_supplier = None;
    sans_ref.lines = vec![LigneAchat::new("x", dec("1"), dec("10"), dec("20"))];
    db::achat_ecriture::creer(&pool, ENTITY, &sans_ref)?;
    assert!(
        db::achat_ecriture::creer(&pool, ENTITY, &sans_ref).is_err(),
        "deux factures sans reference fournisseur se heurtent a l'index unique"
    );

    // Aucune de ces tentatives ne doit avoir laisse de facture en trop : une
    // seule fiche a une reference fournisseur, une seule sans.
    let mut conn = pool.get_conn()?;
    let nb: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_facture_fourn WHERE entity = :entity AND fk_soc = :soc",
        params! { "entity" => ENTITY, "soc" => fk_soc },
    )?;
    assert_eq!(nb.unwrap_or(0), 2);

    nettoyer(&pool, &nom)?;
    nettoyer(&pool, &nom_client)?;
    Ok(())
}
