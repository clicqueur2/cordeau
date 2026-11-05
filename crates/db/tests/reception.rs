//! Réceptions : création, numérotation, validation, réception, suppression,
//! et le stock qui ne bouge pas.
//!
//! Chaque test crée son propre tiers, au nom unique, et le supprime à la fin :
//! la base est partagée par tous les tests du dépôt.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::reception::{
    NewReception, NewReceptionLine, STATUT_BROUILLON, STATUT_RECEPTIONNEE, STATUT_VALIDEE,
};

const ENTITY: i64 = 1;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

/// Efface les réceptions du tiers de test, puis le tiers.
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
        let receptions: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_reception WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for r in receptions {
            conn.exec_drop(
                "DELETE FROM llx_element_element \
                 WHERE targettype = 'reception' AND fk_target = :r",
                params! { "r" => r },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_receptiondet_batch WHERE fk_reception = :r",
                params! { "r" => r },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_reception WHERE rowid = :r",
                params! { "r" => r },
            )?;
        }
        db::party::delete(pool, ENTITY, id)?;
    }
    Ok(())
}

/// Un tiers neuf au nom donné : le nom est l'ancre du nettoyage.
///
/// Il est marqué fournisseur : une réception vient d'un fournisseur, et un
/// tiers « client seulement » ne serait pas proposé par l'écran de saisie.
fn tiers(pool: &Pool, nom: &str) -> Result<i64, Box<dyn Error>> {
    nettoyer(pool, nom)?;
    let mut nouveau = db::party::NewParty::client(nom);
    nouveau.fournisseur = 1;
    db::party::create(pool, ENTITY, &nouveau)
}

/// Un brouillon d'une ligne, pour les tests qui n'examinent que les statuts.
fn brouillon(pool: &Pool, fk_soc: i64) -> Result<i64, Box<dyn Error>> {
    let mut reception = NewReception::new(fk_soc);
    reception.lines = vec![NewReceptionLine::new("Palette de parpaings", dec("1"))];
    db::reception::create(pool, ENTITY, &reception)
}

/// Quantité en stock d'un produit, tous entrepôts confondus, telle que la
/// lirait l'écran de stock.
fn stock_total(pool: &Pool, fk_product: i64) -> Result<String, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let total: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(reel), 0) AS CHAR) FROM llx_product_stock \
         WHERE fk_product = :produit",
        params! { "produit" => fk_product },
    )?;
    Ok(total.unwrap_or_else(|| "0".to_string()))
}

#[test]
fn une_reception_se_cree_se_relit_et_se_valide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Reception Relecture";
    let fk_soc = tiers(&pool, nom)?;

    let mut reception = NewReception::new(fk_soc);
    reception.ref_supplier = Some("BL-FOURNISSEUR-42".to_string());
    reception.note_public = Some("Déchargement par le portail du haut".to_string());

    let mut palette = NewReceptionLine::new("Palette de parpaings", dec("2.5"));
    palette.fk_unit = Some(1);
    palette.batch = Some("LOT-A".to_string());
    palette.eatby = Some("2026-12-31".to_string());
    palette.sellby = Some("2027-06-30".to_string());
    palette.cost_price = Some(dec("1.5"));

    reception.lines = vec![
        palette,
        NewReceptionLine::new("Sac de ciment 35 kg", dec("10")),
    ];
    let id = db::reception::create(&pool, ENTITY, &reception)?;

    let lue = db::reception::get(&pool, ENTITY, id)?.expect("réception relue");
    assert!(
        lue.ref_.starts_with("RE"),
        "référence inattendue : {}",
        lue.ref_
    );
    assert_eq!(lue.fk_statut, STATUT_BROUILLON);
    assert_eq!(lue.fk_soc, fk_soc);
    assert_eq!(lue.tiers.as_deref(), Some(nom));
    assert_eq!(
        lue.ref_supplier.as_deref(),
        Some("BL-FOURNISSEUR-42"),
        "la référence du fournisseur est reprise telle quelle"
    );
    assert!(!lue.facturee, "une réception neuve n'est pas facturée");
    assert!(lue.date_valid.is_none());
    assert!(lue.date_reception.is_none());
    assert!(lue.date_creation.is_some(), "la création date la pièce");
    assert_eq!(lue.nb_lignes, 2);
    assert_eq!(lue.quantite_totale, dec("12.5"), "2,5 + 10");

    let lignes = db::reception::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(
        lignes[0].description.as_deref(),
        Some("Palette de parpaings")
    );
    assert_eq!(lignes[0].qty, dec("2.5"));
    assert_eq!(lignes[0].rang, 0, "le rang suit l'ordre de saisie");
    assert_eq!(lignes[0].fk_unit, Some(1), "l'unité est conservée");
    assert_eq!(lignes[0].batch.as_deref(), Some("LOT-A"));
    assert_eq!(lignes[0].eatby.as_deref(), Some("2026-12-31"));
    assert_eq!(lignes[0].sellby.as_deref(), Some("2027-06-30"));
    assert_eq!(lignes[0].cost_price, Some(dec("1.5")));
    assert_eq!(lignes[1].rang, 1);
    assert_eq!(lignes[1].qty, dec("10"));
    assert!(lignes[1].batch.is_none(), "pas de lot sur cette ligne");

    // La liste rend le plus récent en premier : la pièce créée doit y figurer.
    let liste = db::reception::list(&pool, ENTITY, 50, 0)?;
    assert!(
        liste.iter().any(|x| x.rowid == id),
        "la réception {id} manque dans la liste"
    );

    db::reception::valider(&pool, ENTITY, id)?;
    let validee = db::reception::get(&pool, ENTITY, id)?.expect("réception relue");
    assert_eq!(validee.fk_statut, STATUT_VALIDEE);
    assert!(
        validee.date_valid.is_some(),
        "la validation pose date_valid"
    );
    assert!(
        validee.date_reception.is_none(),
        "valider n'est pas réceptionner"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn une_reception_sans_ligne_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Reception Sans Ligne";
    let fk_soc = tiers(&pool, nom)?;

    let vide = NewReception::new(fk_soc);
    assert!(
        db::reception::create(&pool, ENTITY, &vide).is_err(),
        "un bon de réception sans marchandise n'est pas une pièce"
    );

    // Aucun numéro n'a été consommé pour rien.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_reception WHERE entity = :entity AND fk_soc = :soc",
        params! { "entity" => ENTITY, "soc" => fk_soc },
    )?;
    assert_eq!(restes, Some(0), "aucune pièce ne doit être créée");

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn une_ligne_sans_quantite_ou_sans_libelle_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Reception Ligne Invalide";
    let fk_soc = tiers(&pool, nom)?;

    let mut quantite_nulle = NewReception::new(fk_soc);
    quantite_nulle.lines = vec![NewReceptionLine::new("Palette vide", dec("0"))];
    assert!(
        db::reception::create(&pool, ENTITY, &quantite_nulle).is_err(),
        "une quantité nulle n'entre pas en stock"
    );

    let mut quantite_negative = NewReception::new(fk_soc);
    quantite_negative.lines = vec![NewReceptionLine::new("Palette à l'envers", dec("-2"))];
    assert!(
        db::reception::create(&pool, ENTITY, &quantite_negative).is_err(),
        "une quantité négative n'est pas une réception"
    );

    let mut sans_libelle = NewReception::new(fk_soc);
    sans_libelle.lines = vec![NewReceptionLine::new("   ", dec("1"))];
    assert!(
        db::reception::create(&pool, ENTITY, &sans_libelle).is_err(),
        "une ligne sans texte ni produit est illisible sur le bon"
    );

    // Une date limite mal formée est refusée avant tout aller-retour SQL : elle
    // ferait échouer l'insertion en mode strict, sans dire pourquoi.
    let mut date_fausse = NewReception::new(fk_soc);
    let mut ligne = NewReceptionLine::new("Palette de parpaings", dec("1"));
    ligne.eatby = Some("31/12/2026".to_string());
    date_fausse.lines = vec![ligne];
    assert!(
        db::reception::create(&pool, ENTITY, &date_fausse).is_err(),
        "date au mauvais format"
    );

    let mut date_impossible = NewReception::new(fk_soc);
    let mut ligne = NewReceptionLine::new("Palette de parpaings", dec("1"));
    ligne.sellby = Some("2026-02-31".to_string());
    date_impossible.lines = vec![ligne];
    assert!(
        db::reception::create(&pool, ENTITY, &date_impossible).is_err(),
        "le 31 février existe au format mais pas au calendrier"
    );

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

    let mut reception = NewReception::new(absent);
    reception.lines = vec![NewReceptionLine::new("Palette de parpaings", dec("1"))];
    let erreur = db::reception::create(&pool, ENTITY, &reception)
        .expect_err("un tiers inexistant doit être refusé");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message inattendu : {erreur}"
    );
    Ok(())
}

#[test]
fn une_reception_ne_se_valide_qu_une_fois() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Reception Double Validation";
    let fk_soc = tiers(&pool, nom)?;

    let id = brouillon(&pool, fk_soc)?;
    db::reception::valider(&pool, ENTITY, id)?;
    let erreur = db::reception::valider(&pool, ENTITY, id)
        .expect_err("une validation deux fois est un bug d'appelant");
    assert!(
        erreur.to_string().contains("déjà validée"),
        "message inattendu : {erreur}"
    );

    // La pièce reste validée, rien n'a basculé.
    let lue = db::reception::get(&pool, ENTITY, id)?.expect("réception relue");
    assert_eq!(lue.fk_statut, STATUT_VALIDEE);

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn on_ne_receptionne_qu_une_piece_validee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Reception Entree";
    let fk_soc = tiers(&pool, nom)?;

    let id = brouillon(&pool, fk_soc)?;
    assert!(
        db::reception::receptionner(&pool, ENTITY, id, "2026-11-05").is_err(),
        "un brouillon n'entre pas en stock"
    );

    db::reception::valider(&pool, ENTITY, id)?;
    // Une date au format français passerait la liaison SQL mais ferait échouer
    // la requête en mode strict : elle est refusée avant.
    assert!(
        db::reception::receptionner(&pool, ENTITY, id, "05/11/2026").is_err(),
        "date au mauvais format"
    );

    db::reception::receptionner(&pool, ENTITY, id, "2026-11-05")?;
    let entree = db::reception::get(&pool, ENTITY, id)?.expect("réception relue");
    assert_eq!(entree.fk_statut, STATUT_RECEPTIONNEE);
    assert_eq!(entree.date_reception.as_deref(), Some("2026-11-05"));

    assert!(
        db::reception::receptionner(&pool, ENTITY, id, "2026-11-06").is_err(),
        "une pièce déjà réceptionnée n'entre pas deux fois"
    );
    assert!(
        db::reception::valider(&pool, ENTITY, id).is_err(),
        "une pièce réceptionnée n'a plus à être validée"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn la_suppression_n_accepte_que_les_brouillons() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Reception Suppression";
    let fk_soc = tiers(&pool, nom)?;

    let jete = brouillon(&pool, fk_soc)?;
    db::reception::supprimer(&pool, ENTITY, jete)?;
    assert!(
        db::reception::get(&pool, ENTITY, jete)?.is_none(),
        "le brouillon est parti"
    );
    assert!(
        db::reception::lines(&pool, ENTITY, jete)?.is_empty(),
        "les lignes sont parties avec l'en-tête"
    );

    let garde = brouillon(&pool, fk_soc)?;
    db::reception::valider(&pool, ENTITY, garde)?;
    let erreur = db::reception::supprimer(&pool, ENTITY, garde)
        .expect_err("une pièce validée prouve une entrée de marchandise");
    assert!(
        erreur.to_string().contains("ne se supprime pas"),
        "message inattendu : {erreur}"
    );
    assert!(
        db::reception::get(&pool, ENTITY, garde)?.is_some(),
        "la pièce refusée est toujours là"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn les_receptions_sont_numerotees_sans_doublon() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = "Tiers Reception Numeros";
    let fk_soc = tiers(&pool, nom)?;

    let mut refs = Vec::new();
    for _ in 0..3 {
        let id = brouillon(&pool, fk_soc)?;
        refs.push(
            db::reception::get(&pool, ENTITY, id)?
                .expect("réception relue")
                .ref_,
        );
    }
    let uniques: BTreeSet<&String> = refs.iter().collect();
    assert_eq!(uniques.len(), 3, "références dupliquées : {refs:?}");
    for reference in &refs {
        assert!(
            reference.starts_with("RE"),
            "référence inattendue : {reference}"
        );
    }

    nettoyer(&pool, nom)?;
    Ok(())
}

/// Le manque assumé du module : la réception est tracée, le stock ne bouge pas.
///
/// Le contrôle porte sur **tous** les entrepôts du produit : si un mouvement
/// était écrit, il créerait une ligne de `llx_product_stock` et la somme
/// changerait, même quand aucun niveau n'existait avant.
#[test]
fn une_reception_ne_touche_pas_au_stock() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    // Un produit et, si possible, un entrepôt de l'entité : sans produit, il
    // n'y a rien à mesurer et le test n'a pas d'objet.
    let (fk_product, fk_entrepot) = {
        let mut conn = pool.get_conn()?;
        let produit: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_product WHERE entity = :entity ORDER BY rowid ASC LIMIT 1",
            params! { "entity" => ENTITY },
        )?;
        let Some(produit) = produit else {
            return Ok(());
        };
        let entrepot: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_entrepot WHERE entity = :entity ORDER BY rowid ASC LIMIT 1",
            params! { "entity" => ENTITY },
        )?;
        (produit, entrepot)
    };

    let avant = stock_total(&pool, fk_product)?;

    let nom = "Tiers Reception Stock Intact";
    let fk_soc = tiers(&pool, nom)?;
    let mut reception = NewReception::new(fk_soc);
    // Une ligne réduite à son produit : elle est valide, et c'est bien ce
    // produit qu'un mouvement de stock créditerait.
    let mut ligne = NewReceptionLine::new("", dec("7"));
    ligne.fk_product = Some(fk_product);
    ligne.fk_entrepot = fk_entrepot;
    reception.lines = vec![ligne];
    let id = db::reception::create(&pool, ENTITY, &reception)?;

    // Même menée jusqu'au bout du cycle, la pièce ne crédite rien.
    db::reception::valider(&pool, ENTITY, id)?;
    db::reception::receptionner(&pool, ENTITY, id, "2026-11-05")?;

    let apres = stock_total(&pool, fk_product)?;
    assert_eq!(
        avant, apres,
        "le stock du produit {fk_product} a bougé ({avant} → {apres}) : ce module \
         ne doit pas écrire de mouvement tant que le module de stock dédié n'existe pas"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}
