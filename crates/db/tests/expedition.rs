//! Expéditions : création, numérotation, validation, expédition, suppression.
//!
//! Chaque test crée son propre tiers, au nom unique, et le supprime à la fin :
//! la base est partagée par tous les tests du dépôt.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::expedition::{
    NewExpedition, NewExpeditionLine, STATUT_BROUILLON, STATUT_EXPEDIEE, STATUT_VALIDEE,
};

const ENTITY: i64 = 1;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

/// Efface les expéditions du tiers de test, puis le tiers.
///
/// Les lignes partent avant l'en-tête : le schéma Dolibarr ne pose pas de clé
/// étrangère, un reste de ligne ne serait donc jamais signalé.
fn nettoyer(pool: &Pool, nom: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    for id in ids {
        let expeditions: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_expedition WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for e in expeditions {
            conn.exec_drop(
                "DELETE FROM llx_element_element WHERE targettype = 'expedition' AND fk_target = :e",
                params! { "e" => e },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_expeditiondet WHERE fk_expedition = :e",
                params! { "e" => e },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_expedition WHERE rowid = :e",
                params! { "e" => e },
            )?;
        }
        db::party::delete(pool, ENTITY, id)?;
    }
    Ok(())
}

/// Un tiers neuf au nom donné : le nom est l'ancre du nettoyage.
fn tiers(pool: &Pool, nom: &str) -> Result<i64, Box<dyn Error>> {
    nettoyer(pool, nom)?;
    db::party::create(pool, ENTITY, &db::party::NewParty::client(nom))
}

/// Un brouillon d'une ligne, pour les tests qui n'examinent que les statuts.
fn brouillon(pool: &Pool, fk_soc: i64) -> Result<i64, Box<dyn Error>> {
    let mut expedition = NewExpedition::new(fk_soc);
    expedition.lines = vec![NewExpeditionLine::new("Palette de moellons", dec("1"))];
    db::expedition::create(pool, ENTITY, &expedition)
}

#[test]
fn une_expedition_se_cree_se_relit_et_se_valide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Expedition Relecture";
    let fk_soc = tiers(&pool, nom)?;

    let mut expedition = NewExpedition::new(fk_soc);
    expedition.ref_customer = Some("CMD-CLIENT-42".to_string());
    expedition.note_public = Some("Livraison sur le chantier du haut".to_string());
    expedition.lines = vec![
        NewExpeditionLine::new("Palette de moellons", dec("2.5")),
        NewExpeditionLine::new("Sac de ciment 35 kg", dec("10")),
    ];
    let id = db::expedition::create(&pool, ENTITY, &expedition)?;

    let lue = db::expedition::get(&pool, ENTITY, id)?.expect("expedition relue");
    assert!(
        lue.ref_.starts_with("EX"),
        "reference inattendue : {}",
        lue.ref_
    );
    assert_eq!(lue.fk_statut, STATUT_BROUILLON);
    assert_eq!(lue.fk_soc, fk_soc);
    assert_eq!(lue.tiers.as_deref(), Some(nom));
    assert_eq!(lue.ref_customer.as_deref(), Some("CMD-CLIENT-42"));
    assert!(!lue.facturee, "une expedition neuve n'est pas facturee");
    assert!(lue.date_valid.is_none());
    assert_eq!(lue.nb_lignes, 2);
    assert_eq!(lue.quantite_totale, dec("12.5"), "2,5 + 10");

    let lignes = db::expedition::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(
        lignes[0].description.as_deref(),
        Some("Palette de moellons")
    );
    assert_eq!(lignes[0].qty, dec("2.5"));
    assert_eq!(lignes[0].rang, 0, "le rang suit l'ordre de saisie");
    assert_eq!(lignes[1].rang, 1);
    assert_eq!(lignes[1].qty, dec("10"));

    // La liste rend le plus recent en premier : la piece creee doit y figurer.
    let liste = db::expedition::list(&pool, ENTITY, 50, 0)?;
    assert!(
        liste.iter().any(|x| x.rowid == id),
        "l'expedition {id} manque dans la liste"
    );

    db::expedition::valider(&pool, ENTITY, id)?;
    let validee = db::expedition::get(&pool, ENTITY, id)?.expect("expedition relue");
    assert_eq!(validee.fk_statut, STATUT_VALIDEE);
    assert!(
        validee.date_valid.is_some(),
        "la validation pose date_valid"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn une_expedition_sans_ligne_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Expedition Sans Ligne";
    let fk_soc = tiers(&pool, nom)?;

    let vide = NewExpedition::new(fk_soc);
    assert!(
        db::expedition::create(&pool, ENTITY, &vide).is_err(),
        "un bon de livraison sans marchandise n'est pas une piece"
    );

    // Aucun numero n'a ete consomme pour rien.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_expedition WHERE entity = :entity AND fk_soc = :soc",
        params! { "entity" => ENTITY, "soc" => fk_soc },
    )?;
    assert_eq!(restes, Some(0), "aucune piece ne doit etre creee");

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn une_ligne_sans_quantite_ou_sans_libelle_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Expedition Ligne Invalide";
    let fk_soc = tiers(&pool, nom)?;

    let mut quantite_nulle = NewExpedition::new(fk_soc);
    quantite_nulle.lines = vec![NewExpeditionLine::new("Palette vide", dec("0"))];
    assert!(
        db::expedition::create(&pool, ENTITY, &quantite_nulle).is_err(),
        "une quantite nulle ne sort pas de stock"
    );

    let mut sans_libelle = NewExpedition::new(fk_soc);
    sans_libelle.lines = vec![NewExpeditionLine::new("   ", dec("1"))];
    assert!(
        db::expedition::create(&pool, ENTITY, &sans_libelle).is_err(),
        "une ligne sans texte ni produit est illisible sur le bon"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn un_tiers_inexistant_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    // Un identifiant tres au-dessus du maximum : il ne peut pas exister, et il
    // n'en cree pas pour autant.
    let mut conn = pool.get_conn()?;
    let absent: i64 = conn
        .query_first("SELECT COALESCE(MAX(rowid), 0) + 1000 FROM llx_societe")?
        .unwrap_or(1_000);

    let mut expedition = NewExpedition::new(absent);
    expedition.lines = vec![NewExpeditionLine::new("Palette de moellons", dec("1"))];
    let erreur = db::expedition::create(&pool, ENTITY, &expedition)
        .expect_err("un tiers inexistant doit etre refuse");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message inattendu : {erreur}"
    );
    Ok(())
}

#[test]
fn une_expedition_ne_se_valide_qu_une_fois() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Expedition Double Validation";
    let fk_soc = tiers(&pool, nom)?;

    let id = brouillon(&pool, fk_soc)?;
    db::expedition::valider(&pool, ENTITY, id)?;
    let erreur = db::expedition::valider(&pool, ENTITY, id)
        .expect_err("une validation deux fois est un bug d'appelant");
    assert!(
        erreur.to_string().contains("déjà validée"),
        "message inattendu : {erreur}"
    );

    // La piece reste validee, rien n'a bascule.
    let lue = db::expedition::get(&pool, ENTITY, id)?.expect("expedition relue");
    assert_eq!(lue.fk_statut, STATUT_VALIDEE);

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn on_n_expedie_qu_une_piece_validee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Expedition Envoi";
    let fk_soc = tiers(&pool, nom)?;

    let id = brouillon(&pool, fk_soc)?;
    assert!(
        db::expedition::expedier(&pool, ENTITY, id, "2026-11-05", Some("TRK-0001")).is_err(),
        "un brouillon ne part pas"
    );

    db::expedition::valider(&pool, ENTITY, id)?;
    // Une date au format francais passerait la liaison SQL mais ferait echouer
    // la requete en mode strict : elle est refusee avant.
    assert!(
        db::expedition::expedier(&pool, ENTITY, id, "05/11/2026", None).is_err(),
        "date au mauvais format"
    );

    db::expedition::expedier(&pool, ENTITY, id, "2026-11-05", Some("TRK-0001"))?;
    let expediee = db::expedition::get(&pool, ENTITY, id)?.expect("expedition relue");
    assert_eq!(expediee.fk_statut, STATUT_EXPEDIEE);
    assert_eq!(expediee.date_expedition.as_deref(), Some("2026-11-05"));
    assert_eq!(expediee.tracking_number.as_deref(), Some("TRK-0001"));

    assert!(
        db::expedition::expedier(&pool, ENTITY, id, "2026-11-06", None).is_err(),
        "une piece deja expediee ne repart pas"
    );
    assert!(
        db::expedition::valider(&pool, ENTITY, id).is_err(),
        "une piece expediee n'a plus a etre validee"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn la_suppression_n_accepte_que_les_brouillons() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Expedition Suppression";
    let fk_soc = tiers(&pool, nom)?;

    let jete = brouillon(&pool, fk_soc)?;
    db::expedition::supprimer(&pool, ENTITY, jete)?;
    assert!(
        db::expedition::get(&pool, ENTITY, jete)?.is_none(),
        "le brouillon est parti"
    );
    assert!(
        db::expedition::lines(&pool, ENTITY, jete)?.is_empty(),
        "les lignes sont parties avec l'en-tete"
    );

    let garde = brouillon(&pool, fk_soc)?;
    db::expedition::valider(&pool, ENTITY, garde)?;
    let erreur = db::expedition::supprimer(&pool, ENTITY, garde)
        .expect_err("une piece validee prouve une sortie de stock");
    assert!(
        erreur.to_string().contains("ne se supprime pas"),
        "message inattendu : {erreur}"
    );
    assert!(
        db::expedition::get(&pool, ENTITY, garde)?.is_some(),
        "la piece refusee est toujours la"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn les_expeditions_sont_numerotees_sans_doublon() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Expedition Numeros";
    let fk_soc = tiers(&pool, nom)?;

    let mut refs = Vec::new();
    for _ in 0..3 {
        let id = brouillon(&pool, fk_soc)?;
        refs.push(
            db::expedition::get(&pool, ENTITY, id)?
                .expect("expedition relue")
                .ref_,
        );
    }
    let uniques: BTreeSet<&String> = refs.iter().collect();
    assert_eq!(uniques.len(), 3, "references dupliquees : {refs:?}");
    for reference in &refs {
        assert!(
            reference.starts_with("EX"),
            "reference inattendue : {reference}"
        );
    }

    nettoyer(&pool, nom)?;
    Ok(())
}
