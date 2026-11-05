//! Livraisons : création, totaux au centime, numérotation, validation,
//! livraison, suppression.
//!
//! Chaque test crée son propre tiers, au nom unique, et le supprime à la fin :
//! la base est partagée par tous les tests du dépôt. Les montants sont vérifiés
//! à la main, pas en recopiant ce que le code vient de calculer.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::livraison::{
    NewLigneLivraison, NewLivraison, STATUT_BROUILLON, STATUT_LIVREE, STATUT_VALIDEE,
};

const ENTITY: i64 = 1;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

/// Efface les livraisons du tiers de test, puis le tiers.
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
        let livraisons: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_delivery WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for l in livraisons {
            conn.exec_drop(
                "DELETE FROM llx_element_element \
                 WHERE targettype = 'delivery' AND fk_target = :l",
                params! { "l" => l },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_deliverydet WHERE fk_delivery = :l",
                params! { "l" => l },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_delivery WHERE rowid = :l",
                params! { "l" => l },
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
    let mut livraison = NewLivraison::new(fk_soc);
    livraison.lines = vec![NewLigneLivraison::new(
        "Palette de moellons",
        dec("1"),
        dec("100.00"),
    )];
    db::livraison::create(pool, ENTITY, &livraison)
}

/// Le nombre de livraisons du tiers : sert à prouver qu'un refus n'écrit rien.
fn compte(pool: &Pool, fk_soc: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_delivery WHERE entity = :entity AND fk_soc = :soc",
        params! { "entity" => ENTITY, "soc" => fk_soc },
    )?;
    Ok(n.unwrap_or(0))
}

#[test]
fn une_livraison_se_cree_se_relit_et_calcule_ses_totaux() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Livraison Relecture";
    let fk_soc = tiers(&pool, nom)?;

    let mut livraison = NewLivraison::new(fk_soc);
    livraison.ref_customer = Some("CMD-CLIENT-77".to_string());
    livraison.date_delivery = Some("2026-11-20".to_string());
    livraison.note_public = Some("Livraison sur le chantier du haut".to_string());
    livraison.lines = vec![
        // 3 × 12,3456 = 37,0368 → 37,04
        NewLigneLivraison::new("Moellons calibrés", dec("3"), dec("12.3456")),
        // 2,5 × 19,99 = 49,975 → 49,98 (demi hors de zéro)
        NewLigneLivraison::new("Sable 0/4 en big bag", dec("2.5"), dec("19.99")),
        // 1 × 0,005 = 0,005 → 0,01
        NewLigneLivraison::new("Ciment prompt", dec("1"), dec("0.005")),
    ];
    let id = db::livraison::create(&pool, ENTITY, &livraison)?;

    let lue = db::livraison::get(&pool, ENTITY, id)?.expect("livraison relue");
    assert!(
        lue.ref_.starts_with("BL"),
        "reference inattendue : {}",
        lue.ref_
    );
    assert_eq!(lue.fk_statut, STATUT_BROUILLON);
    assert_eq!(lue.fk_soc, fk_soc);
    assert_eq!(lue.tiers.as_deref(), Some(nom));
    assert_eq!(lue.ref_customer.as_deref(), Some("CMD-CLIENT-77"));
    assert!(lue.date_valid.is_none());
    assert_eq!(
        lue.date_livraison.as_deref(),
        Some("2026-11-20"),
        "la date prévue est posée dès la création"
    );
    assert_eq!(lue.nb_lignes, 3);
    assert_eq!(lue.quantite_totale, dec("6.5"), "3 + 2,5 + 1");
    // 37,04 + 49,98 + 0,01 = 87,03 : fait à la main, pas recopié du code.
    assert_eq!(lue.total_ht, dec("87.03"));

    let lignes = db::livraison::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 3);
    assert_eq!(lignes[0].description.as_deref(), Some("Moellons calibrés"));
    assert_eq!(lignes[0].qty, dec("3"));
    assert_eq!(lignes[0].subprice, dec("12.3456"));
    assert_eq!(lignes[0].total_ht, dec("37.04"));
    assert_eq!(
        lignes[0].subprice_ttc, lignes[0].subprice,
        "sans TVA sur la ligne, le TTC vaut le HT"
    );
    assert_eq!(lignes[0].rang, 0, "le rang suit l'ordre de saisie");
    assert_eq!(lignes[1].rang, 1);
    assert_eq!(lignes[1].total_ht, dec("49.98"));
    assert_eq!(lignes[2].rang, 2);
    assert_eq!(lignes[2].total_ht, dec("0.01"));

    // La somme des lignes redonne l'en-tête : l'arrondi n'a eu lieu qu'une fois.
    let somme: Decimal = lignes.iter().map(|l| l.total_ht).sum();
    assert_eq!(somme, lue.total_ht);

    // La liste rend le plus récent en premier : la pièce créée doit y figurer.
    let liste = db::livraison::list(&pool, ENTITY, 50, 0)?;
    assert!(
        liste.iter().any(|x| x.rowid == id),
        "la livraison {id} manque dans la liste"
    );

    db::livraison::valider(&pool, ENTITY, id)?;
    let validee = db::livraison::get(&pool, ENTITY, id)?.expect("livraison relue");
    assert_eq!(validee.fk_statut, STATUT_VALIDEE);
    assert!(
        validee.date_valid.is_some(),
        "la validation pose date_valid"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn une_livraison_sans_ligne_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Livraison Sans Ligne";
    let fk_soc = tiers(&pool, nom)?;

    let vide = NewLivraison::new(fk_soc);
    let erreur = db::livraison::create(&pool, ENTITY, &vide)
        .expect_err("un bon sans marchandise n'a rien à livrer");
    assert!(
        erreur.to_string().contains("sans ligne"),
        "message inattendu : {erreur}"
    );

    // Aucun numéro n'a été consommé pour rien, aucune pièce n'a été créée.
    assert_eq!(compte(&pool, fk_soc)?, 0, "aucune piece ne doit etre creee");

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn une_ligne_sans_quantite_au_prix_negatif_ou_sans_libelle_est_refusee(
) -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Livraison Ligne Invalide";
    let fk_soc = tiers(&pool, nom)?;

    let mut quantite_nulle = NewLivraison::new(fk_soc);
    quantite_nulle.lines = vec![NewLigneLivraison::new("Palette vide", dec("0"), dec("10"))];
    let erreur = db::livraison::create(&pool, ENTITY, &quantite_nulle)
        .expect_err("une quantité nulle ne livre rien");
    assert!(
        erreur.to_string().contains("strictement positive"),
        "message inattendu : {erreur}"
    );

    let mut prix_negatif = NewLivraison::new(fk_soc);
    prix_negatif.lines = vec![NewLigneLivraison::new("Avoir déguisé", dec("1"), dec("-5"))];
    let erreur = db::livraison::create(&pool, ENTITY, &prix_negatif)
        .expect_err("un prix négatif n'est pas une remise");
    assert!(
        erreur.to_string().contains("négatif"),
        "message inattendu : {erreur}"
    );

    // Une ligne sans texte et sans produit serait illisible sur le bon : le
    // client ne saurait pas ce qu'il a reçu.
    let mut sans_libelle = NewLivraison::new(fk_soc);
    sans_libelle.lines = vec![NewLigneLivraison::new("   ", dec("1"), dec("10"))];
    let erreur = db::livraison::create(&pool, ENTITY, &sans_libelle)
        .expect_err("ni texte ni produit : ligne illisible");
    assert!(
        erreur.to_string().contains("description ou un produit"),
        "message inattendu : {erreur}"
    );

    assert_eq!(compte(&pool, fk_soc)?, 0, "aucune piece ne doit etre creee");

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn un_tiers_inexistant_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    // Un identifiant très au-dessus du maximum : il ne peut pas exister, et il
    // n'en crée pas pour autant.
    let mut conn = pool.get_conn()?;
    let absent: i64 = conn
        .query_first("SELECT COALESCE(MAX(rowid), 0) + 1000 FROM llx_societe")?
        .unwrap_or(1_000);

    let mut livraison = NewLivraison::new(absent);
    livraison.lines = vec![NewLigneLivraison::new(
        "Palette de moellons",
        dec("1"),
        dec("10"),
    )];
    let erreur = db::livraison::create(&pool, ENTITY, &livraison)
        .expect_err("un tiers inexistant doit etre refuse");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message inattendu : {erreur}"
    );
    Ok(())
}

#[test]
fn une_livraison_ne_se_valide_qu_une_fois() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Livraison Double Validation";
    let fk_soc = tiers(&pool, nom)?;

    let id = brouillon(&pool, fk_soc)?;
    db::livraison::valider(&pool, ENTITY, id)?;
    let erreur = db::livraison::valider(&pool, ENTITY, id)
        .expect_err("une validation deux fois est un bug d'appelant");
    assert!(
        erreur
            .to_string()
            .contains("seule une livraison en brouillon"),
        "message inattendu : {erreur}"
    );

    // La pièce reste validée, rien n'a basculé.
    let lue = db::livraison::get(&pool, ENTITY, id)?.expect("livraison relue");
    assert_eq!(lue.fk_statut, STATUT_VALIDEE);

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn on_ne_livre_qu_une_piece_validee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Livraison Envoi";
    let fk_soc = tiers(&pool, nom)?;

    let id = brouillon(&pool, fk_soc)?;
    let erreur = db::livraison::livrer(&pool, ENTITY, id, "2026-11-05")
        .expect_err("un brouillon ne se livre pas");
    assert!(
        erreur.to_string().contains("n'est pas validée"),
        "message inattendu : {erreur}"
    );

    db::livraison::valider(&pool, ENTITY, id)?;
    // Une date au format français passerait la liaison SQL mais ferait échouer
    // la requête en mode strict : elle est refusée avant.
    assert!(
        db::livraison::livrer(&pool, ENTITY, id, "05/11/2026").is_err(),
        "date au mauvais format"
    );

    db::livraison::livrer(&pool, ENTITY, id, "2026-11-05")?;
    let livree = db::livraison::get(&pool, ENTITY, id)?.expect("livraison relue");
    assert_eq!(livree.fk_statut, STATUT_LIVREE);
    assert_eq!(livree.date_livraison.as_deref(), Some("2026-11-05"));

    assert!(
        db::livraison::livrer(&pool, ENTITY, id, "2026-11-06").is_err(),
        "une pièce déjà livrée ne se relivre pas"
    );
    assert!(
        db::livraison::valider(&pool, ENTITY, id).is_err(),
        "une pièce livrée n'a plus à être validée"
    );

    // Décision assumée : la date réelle écrase la date prévue, comme
    // `Delivery::setDelivered()` dans Dolibarr. La colonne ne garde que la
    // dernière vérité, celle du jour où le camion est passé.
    let mut prevue = NewLivraison::new(fk_soc);
    prevue.date_delivery = Some("2026-11-20".to_string());
    prevue.lines = vec![NewLigneLivraison::new(
        "Palette de moellons",
        dec("1"),
        dec("10"),
    )];
    let autre = db::livraison::create(&pool, ENTITY, &prevue)?;
    db::livraison::valider(&pool, ENTITY, autre)?;
    db::livraison::livrer(&pool, ENTITY, autre, "2026-11-28")?;
    let relue = db::livraison::get(&pool, ENTITY, autre)?.expect("livraison relue");
    assert_eq!(relue.date_livraison.as_deref(), Some("2026-11-28"));

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn la_suppression_n_accepte_que_les_brouillons() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Livraison Suppression";
    let fk_soc = tiers(&pool, nom)?;

    let jete = brouillon(&pool, fk_soc)?;
    db::livraison::supprimer(&pool, ENTITY, jete)?;
    assert!(
        db::livraison::get(&pool, ENTITY, jete)?.is_none(),
        "le brouillon est parti"
    );
    assert!(
        db::livraison::lines(&pool, ENTITY, jete)?.is_empty(),
        "les lignes sont parties avec l'en-tête"
    );

    let garde = brouillon(&pool, fk_soc)?;
    db::livraison::valider(&pool, ENTITY, garde)?;
    let erreur = db::livraison::supprimer(&pool, ENTITY, garde)
        .expect_err("une pièce validée prouve ce que le client a reçu");
    assert!(
        erreur.to_string().contains("ne se supprime pas"),
        "message inattendu : {erreur}"
    );
    assert!(
        db::livraison::get(&pool, ENTITY, garde)?.is_some(),
        "la pièce refusée est toujours là"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn les_livraisons_sont_numerotees_sans_doublon() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Livraison Numeros";
    let fk_soc = tiers(&pool, nom)?;

    let mut refs = Vec::new();
    for _ in 0..3 {
        let id = brouillon(&pool, fk_soc)?;
        refs.push(
            db::livraison::get(&pool, ENTITY, id)?
                .expect("livraison relue")
                .ref_,
        );
    }
    let uniques: BTreeSet<&String> = refs.iter().collect();
    assert_eq!(uniques.len(), 3, "references dupliquees : {refs:?}");
    for reference in &refs {
        assert!(
            reference.starts_with("BL"),
            "reference inattendue : {reference}"
        );
    }

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn le_total_egale_la_somme_des_lignes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Livraison Total";
    let fk_soc = tiers(&pool, nom)?;

    let mut livraison = NewLivraison::new(fk_soc);
    livraison.lines = vec![
        NewLigneLivraison::new("Chevron 63 × 75", dec("12"), dec("4.83")),
        NewLigneLivraison::new("Pointes inox 50 mm", dec("3"), dec("9.99")),
    ];
    let id = db::livraison::create(&pool, ENTITY, &livraison)?;

    // 12 × 4,83 = 57,96 ; 3 × 9,99 = 29,97 ; total 87,93 (au centime, à la main).
    assert_eq!(db::livraison::total(&pool, ENTITY, id)?, dec("87.93"));

    let lignes = db::livraison::lines(&pool, ENTITY, id)?;
    let somme: Decimal = lignes.iter().map(|l| l.total_ht).sum();
    assert_eq!(
        db::livraison::total(&pool, ENTITY, id)?,
        somme,
        "l'en-tete ne doit pas deriver de ses lignes"
    );

    // Un identifiant absent est une erreur, pas un total de zéro : zéro se
    // confondrait avec une livraison vide.
    assert!(
        db::livraison::total(&pool, ENTITY, id + 100_000).is_err(),
        "une livraison inconnue n'a pas de total"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}
