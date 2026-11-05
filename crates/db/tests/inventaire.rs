//! Inventaires : création, écart constaté, validation, suppression, et le
//! stock qui ne bouge pas.
//!
//! Chaque test crée ses propres produits et entrepôts, sous un préfixe de
//! référence unique, et les supprime à la fin : la base est partagée par tous
//! les tests du dépôt, et un entrepôt laissé derrière ferait échouer le
//! prochain run (« un entrepôt porte déjà la référence »).

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::inventaire::{
    NewInventaire, NewLigneInventaire, STATUT_BROUILLON, STATUT_VALIDE, TITRE_MAX_OCTETS,
};
use db::product::NewProduct;

const ENTITY: i64 = 1;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

/// Efface tout ce qu'un test a pu laisser sous son préfixe.
///
/// L'ordre compte : les inventaires d'abord (ils pointent produits et
/// entrepôts), puis les mouvements de stock des produits, puis les produits,
/// puis les entrepôts. Le schéma Dolibarr ne pose pas de clé étrangère : un
/// reste ne serait jamais signalé, il pourrirait seulement les runs suivants.
fn nettoyer(pool: &Pool, prefixe: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let motif = format!("{prefixe}%");

    // Un inventaire se rattache au test par son produit d'en-tête ou par celui
    // d'une de ses lignes : les deux chemins sont couverts, sinon un inventaire
    // créé sans produit d'en-tête survivrait au nettoyage.
    let inventaires: Vec<i64> = conn.exec(
        "SELECT DISTINCT i.rowid FROM llx_inventory i \
         WHERE i.entity = :entity AND ( \
           i.fk_product IN (SELECT rowid FROM llx_product \
                             WHERE entity = :entity AND ref LIKE :motif) \
           OR EXISTS (SELECT 1 FROM llx_inventorydet d \
                      JOIN llx_product p ON p.rowid = d.fk_product \
                      WHERE d.fk_inventory = i.rowid AND p.entity = :entity \
                        AND p.ref LIKE :motif))",
        params! { "entity" => ENTITY, "motif" => motif.as_str() },
    )?;
    for id in inventaires {
        conn.exec_drop(
            "DELETE FROM llx_inventorydet WHERE fk_inventory = :id",
            params! { "id" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_inventory WHERE rowid = :id",
            params! { "id" => id },
        )?;
    }

    let produits: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_product WHERE entity = :entity AND ref LIKE :motif",
        params! { "entity" => ENTITY, "motif" => motif.as_str() },
    )?;
    for p in produits {
        conn.exec_drop(
            "DELETE FROM llx_stock_mouvement WHERE fk_product = :p",
            params! { "p" => p },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_product_stock WHERE fk_product = :p",
            params! { "p" => p },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_product WHERE rowid = :p",
            params! { "p" => p },
        )?;
    }

    let entrepots: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_entrepot WHERE entity = :entity AND ref LIKE :motif",
        params! { "entity" => ENTITY, "motif" => motif.as_str() },
    )?;
    for e in entrepots {
        conn.exec_drop(
            "DELETE FROM llx_entrepot WHERE rowid = :e",
            params! { "e" => e },
        )?;
    }
    Ok(())
}

/// Un produit neuf sous la référence donnée (le nettoyage a déjà eu lieu).
fn produit(pool: &Pool, reference: &str) -> Result<i64, Box<dyn Error>> {
    db::product::create(pool, ENTITY, &NewProduct::new(reference, "Bordure granit"))
}

/// Un entrepôt neuf sous la référence donnée.
fn entrepot(pool: &Pool, reference: &str) -> Result<i64, Box<dyn Error>> {
    db::stock::creer_entrepot(pool, ENTITY, reference, Some("Charly"))
}

/// Une ligne d'inventaire : le système dit `stock`, le magasin a compté `vue`.
fn ligne(entrepot: i64, produit: i64, stock: &str, vue: &str) -> NewLigneInventaire {
    NewLigneInventaire::new(entrepot, produit, dec(stock), dec(vue))
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

/// Nombre de mouvements de stock du produit : l'historique qui justifie le
/// niveau affiché.
fn mouvements_total(pool: &Pool, fk_product: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let total: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_stock_mouvement WHERE fk_product = :produit",
        params! { "produit" => fk_product },
    )?;
    Ok(total.unwrap_or(0))
}

/// Un brouillon d'une ligne, pour les tests qui n'examinent que les statuts.
fn brouillon(pool: &Pool, entrepot: i64, produit: i64) -> Result<i64, Box<dyn Error>> {
    let mut new = NewInventaire::new("Comptage de contrôle");
    new.lines = vec![ligne(entrepot, produit, "8", "10")];
    db::inventaire::create(pool, ENTITY, &new)
}

#[test]
fn un_inventaire_se_cree_se_relit_et_se_valide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZINVA";
    nettoyer(&pool, prefixe)?;
    let p = produit(&pool, "ZZINVA-P1")?;
    let e = entrepot(&pool, "ZZINVA-ENT")?;

    let mut new = NewInventaire::new("  Inventaire de fin d'année  ");
    // L'en-tête couvre l'entrepôt : les lignes le répètent, comme Dolibarr les
    // écrit, mais un inventaire multi-entrepôts laisserait ce champ vide.
    new.fk_warehouse = Some(e);
    new.fk_product = Some(p);
    new.date_inventory = Some("2026-10-31".to_string());
    new.lines = vec![
        // 7,5 annoncés, 10 comptés : il en manquait 2,5 au système.
        ligne(e, p, "7.5", "10").avec_lot("LOT-A"),
        // 4 annoncés, 1 compté : trois unités ont disparu.
        ligne(e, p, "4", "1"),
    ];
    let id = db::inventaire::create(&pool, ENTITY, &new)?;

    let lu = db::inventaire::get(&pool, ENTITY, id)?.expect("inventaire relu");
    assert!(
        lu.ref_.starts_with("IN"),
        "référence inattendue : {}",
        lu.ref_
    );
    assert_eq!(lu.status, STATUT_BROUILLON);
    assert_eq!(
        lu.title, "Inventaire de fin d'année",
        "le titre est rangé émondé : les espaces de saisie ne sont pas un titre"
    );
    assert_eq!(lu.fk_warehouse, Some(e));
    assert_eq!(lu.fk_product, Some(p));
    assert_eq!(lu.date_inventory.as_deref(), Some("2026-10-31"));
    assert!(lu.date_creation.is_some(), "la création date la pièce");
    assert!(
        lu.date_validation.is_none(),
        "un brouillon n'a pas de date de validation"
    );
    assert_eq!(lu.nb_lignes, 2);
    assert_eq!(
        lu.ecart_total,
        dec("-0.5"),
        "2,5 trouvés en plus, 3 en moins : l'écart total est leur somme"
    );

    let lignes = db::inventaire::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(lignes[0].fk_warehouse, e);
    assert_eq!(lignes[0].fk_product, p);
    assert_eq!(lignes[0].batch.as_deref(), Some("LOT-A"));
    assert_eq!(lignes[0].qty_stock, dec("7.5"));
    assert_eq!(lignes[0].qty_viewed, dec("10"));
    assert_eq!(
        lignes[0].qty_regulated,
        dec("2.5"),
        "l'écart est calculé à la création : 10 comptés - 7,5 annoncés"
    );
    assert!(
        lignes[0].pmp_real.is_none(),
        "aucune valorisation : le PMP suppose les mouvements de stock"
    );
    assert!(lignes[1].batch.is_none(), "pas de lot sur cette ligne");
    assert_eq!(
        lignes[1].qty_regulated,
        dec("-3"),
        "l'écart négatif est conservé tel quel : c'est un manque, pas une erreur"
    );

    // La liste rend le plus récent en premier : la pièce créée doit y figurer.
    let liste = db::inventaire::list(&pool, ENTITY, 50, 0)?;
    assert!(
        liste.iter().any(|x| x.rowid == id),
        "l'inventaire {id} manque dans la liste"
    );

    db::inventaire::valider(&pool, ENTITY, id)?;
    let valide = db::inventaire::get(&pool, ENTITY, id)?.expect("inventaire relu");
    assert_eq!(valide.status, STATUT_VALIDE);
    assert!(
        valide.date_validation.is_some(),
        "la validation pose date_validation"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn un_inventaire_sans_ligne_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZINVB";
    nettoyer(&pool, prefixe)?;
    let p = produit(&pool, "ZZINVB-P1")?;

    let mut vide = NewInventaire::new("Inventaire sans comptage");
    vide.fk_product = Some(p);
    assert!(
        db::inventaire::create(&pool, ENTITY, &vide).is_err(),
        "un inventaire sans ligne ne constate aucun écart"
    );

    // Aucun numéro n'a été consommé pour rien.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_inventory WHERE entity = :entity AND fk_product = :p",
        params! { "entity" => ENTITY, "p" => p },
    )?;
    assert_eq!(restes, Some(0), "aucune pièce ne doit être créée");

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn un_titre_vide_ou_trop_long_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZINVC";
    nettoyer(&pool, prefixe)?;
    let p = produit(&pool, "ZZINVC-P1")?;
    let e = entrepot(&pool, "ZZINVC-ENT")?;

    let mut sans_titre = NewInventaire::new("   ");
    sans_titre.lines = vec![ligne(e, p, "1", "1")];
    assert!(
        db::inventaire::create(&pool, ENTITY, &sans_titre).is_err(),
        "un titre fait de blancs n'est pas un titre"
    );

    // Un octet de trop : en mode non strict, MySQL tronquerait sans le dire et
    // la pièce ne se retrouverait plus par son nom.
    let trop_long = "a".repeat(TITRE_MAX_OCTETS + 1);
    let mut long = NewInventaire::new(&trop_long);
    long.lines = vec![ligne(e, p, "1", "1")];
    let erreur = db::inventaire::create(&pool, ENTITY, &long)
        .expect_err("un titre de 256 octets doit être refusé");
    assert!(
        erreur.to_string().contains("trop long"),
        "message inattendu : {erreur}"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn une_quantite_negative_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZINVD";
    nettoyer(&pool, prefixe)?;
    let p = produit(&pool, "ZZINVD-P1")?;
    let e = entrepot(&pool, "ZZINVD-ENT")?;

    let mut stock_negatif = NewInventaire::new("Stock négatif");
    stock_negatif.lines = vec![ligne(e, p, "-1", "5")];
    assert!(
        db::inventaire::create(&pool, ENTITY, &stock_negatif).is_err(),
        "un stock négatif au départ est une faute de frappe, pas un comptage"
    );

    let mut compte_negatif = NewInventaire::new("Comptage négatif");
    compte_negatif.lines = vec![ligne(e, p, "5", "-2")];
    let erreur = db::inventaire::create(&pool, ENTITY, &compte_negatif)
        .expect_err("on ne compte pas -2 unités");
    assert!(
        erreur.to_string().contains("négative"),
        "message inattendu : {erreur}"
    );

    // Un écart négatif, lui, est parfaitement normal : il n'est pas saisi, il
    // se déduit. Le refus ne doit donc pas le viser.
    let mut manque = NewInventaire::new("Manque constaté");
    manque.lines = vec![ligne(e, p, "5", "2")];
    let id = db::inventaire::create(&pool, ENTITY, &manque)?;
    assert_eq!(db::inventaire::ecarts(&pool, ENTITY, id)?, dec("-3"));

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn un_produit_ou_un_entrepot_inexistant_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZINVE";
    nettoyer(&pool, prefixe)?;
    let p = produit(&pool, "ZZINVE-P1")?;
    let e = entrepot(&pool, "ZZINVE-ENT")?;

    // Des identifiants très au-dessus du maximum : ils ne peuvent pas exister,
    // et ils n'en créent pas pour autant.
    let mut conn = pool.get_conn()?;
    let absent_produit: i64 = conn
        .query_first("SELECT COALESCE(MAX(rowid), 0) + 1000 FROM llx_product")?
        .unwrap_or(1_000);
    let absent_entrepot: i64 = conn
        .query_first("SELECT COALESCE(MAX(rowid), 0) + 1000 FROM llx_entrepot")?
        .unwrap_or(1_000);

    let mut mauvais_produit = NewInventaire::new("Produit inconnu");
    mauvais_produit.fk_product = Some(absent_produit);
    mauvais_produit.lines = vec![ligne(e, p, "1", "1")];
    let erreur = db::inventaire::create(&pool, ENTITY, &mauvais_produit)
        .expect_err("un produit inexistant doit être refusé");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message inattendu : {erreur}"
    );

    // L'entrepôt de la ligne compte autant que celui de l'en-tête : c'est lui
    // que le futur mouvement de régularisation devra créditer.
    let mut mauvais_entrepot = NewInventaire::new("Entrepôt inconnu");
    mauvais_entrepot.lines = vec![ligne(absent_entrepot, p, "1", "1")];
    let erreur = db::inventaire::create(&pool, ENTITY, &mauvais_entrepot)
        .expect_err("un entrepôt inexistant doit être refusé");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message inattendu : {erreur}"
    );

    // Deux lignes identiques : l'index unique `uk_inventorydet` les refuse, et
    // le message doit dire laquelle plutôt que de laisser remonter un 1062.
    let mut doublon = NewInventaire::new("Doublon");
    doublon.lines = vec![ligne(e, p, "1", "2"), ligne(e, p, "1", "3")];
    let erreur = db::inventaire::create(&pool, ENTITY, &doublon)
        .expect_err("un produit ne se compte qu'une fois par entrepôt et par lot");
    assert!(
        erreur.to_string().contains("ligne 2"),
        "message inattendu : {erreur}"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn un_inventaire_ne_se_valide_qu_une_fois() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZINVF";
    nettoyer(&pool, prefixe)?;
    let p = produit(&pool, "ZZINVF-P1")?;
    let e = entrepot(&pool, "ZZINVF-ENT")?;

    let id = brouillon(&pool, e, p)?;
    // Sans date de comptage, la pièce prend celle du serveur : c'est lui qui
    // date, pas le poste dont l'horloge peut être fausse.
    let avant = db::inventaire::get(&pool, ENTITY, id)?.expect("inventaire relu");
    assert!(
        avant.date_inventory.is_some(),
        "à défaut de date saisie, la base date le comptage"
    );

    db::inventaire::valider(&pool, ENTITY, id)?;
    let erreur = db::inventaire::valider(&pool, ENTITY, id)
        .expect_err("une validation deux fois est un bug d'appelant");
    assert!(
        erreur.to_string().contains("déjà validé"),
        "message inattendu : {erreur}"
    );

    // La pièce reste validée, rien n'a basculé.
    let lue = db::inventaire::get(&pool, ENTITY, id)?.expect("inventaire relu");
    assert_eq!(lue.status, STATUT_VALIDE);

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn la_suppression_n_accepte_que_les_brouillons() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZINVG";
    nettoyer(&pool, prefixe)?;
    let p = produit(&pool, "ZZINVG-P1")?;
    let e = entrepot(&pool, "ZZINVG-ENT")?;

    let jete = brouillon(&pool, e, p)?;
    db::inventaire::supprimer(&pool, ENTITY, jete)?;
    assert!(
        db::inventaire::get(&pool, ENTITY, jete)?.is_none(),
        "le brouillon est parti"
    );
    assert!(
        db::inventaire::lines(&pool, ENTITY, jete)?.is_empty(),
        "les lignes sont parties avec l'en-tête"
    );

    let garde = brouillon(&pool, e, p)?;
    db::inventaire::valider(&pool, ENTITY, garde)?;
    let erreur = db::inventaire::supprimer(&pool, ENTITY, garde)
        .expect_err("un inventaire validé atteste d'un comptage");
    assert!(
        erreur.to_string().contains("ne se supprime pas"),
        "message inattendu : {erreur}"
    );
    assert!(
        db::inventaire::get(&pool, ENTITY, garde)?.is_some(),
        "la pièce refusée est toujours là"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

/// L'écart, c'est le constat, pas le stock : 10 comptés pour 8 annoncés valent
/// +2, jamais 10 ni 8.
#[test]
fn l_ecart_constate_s_ajoute_et_ne_se_confond_pas_avec_le_stock() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZINVH";
    nettoyer(&pool, prefixe)?;
    let p = produit(&pool, "ZZINVH-P1")?;
    let e = entrepot(&pool, "ZZINVH-ENT")?;

    let mut new = NewInventaire::new("Comptage du dépôt");
    new.lines = vec![
        ligne(e, p, "8", "10"),
        // Une ligne juste ne pèse rien : elle ne doit pas gonfler la somme.
        ligne(e, p, "4", "4").avec_lot("LOT-B"),
    ];
    let id = db::inventaire::create(&pool, ENTITY, &new)?;

    let ecart = db::inventaire::ecarts(&pool, ENTITY, id)?;
    assert_eq!(ecart, dec("2"), "10 comptés pour 8 en stock : +2");
    assert_ne!(ecart, dec("10"), "l'écart n'est pas la quantité comptée");
    assert_ne!(ecart, dec("12"), "ni la quantité annoncée par le système");

    // La liste et `ecarts` doivent dire la même chose : deux vérités pour un
    // même chiffre finiraient par diverger.
    let lu = db::inventaire::get(&pool, ENTITY, id)?.expect("inventaire relu");
    assert_eq!(lu.ecart_total, ecart);

    // Un identifiant inconnu est une erreur, pas un écart nul : sinon un
    // appelant qui se trompe de numéro lirait « tout est juste ».
    assert!(
        db::inventaire::ecarts(&pool, ENTITY, id + 100_000).is_err(),
        "un inventaire inexistant ne rend pas 0"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

/// Le manque assumé du module : l'inventaire est constaté, le stock ne bouge
/// pas.
///
/// Le contrôle porte sur **tous** les entrepôts du produit (la somme de
/// `llx_product_stock`) et sur l'historique (`llx_stock_mouvement`) : si une
/// régularisation était écrite, elle créerait un mouvement **et** un niveau,
/// même quand aucun niveau n'existait avant.
#[test]
fn valider_un_inventaire_ne_touche_pas_au_stock() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZINVI";
    nettoyer(&pool, prefixe)?;
    let p = produit(&pool, "ZZINVI-P1")?;
    let e = entrepot(&pool, "ZZINVI-ENT")?;

    let avant_stock = stock_total(&pool, p)?;
    let avant_mouvements = mouvements_total(&pool, p)?;
    assert_eq!(avant_mouvements, 0, "un produit neuf n'a pas d'historique");

    // Un écart franc : le produit n'a jamais été en stock, le magasin en compte
    // 12. C'est exactement le cas où la tentation d'écrire le stock est la plus
    // forte — et le cas où une quantité fantôme serait la plus invisible.
    let mut new = NewInventaire::new("Inventaire d'ouverture");
    new.lines = vec![ligne(e, p, "0", "12")];
    let id = db::inventaire::create(&pool, ENTITY, &new)?;
    db::inventaire::valider(&pool, ENTITY, id)?;

    let apres_stock = stock_total(&pool, p)?;
    let apres_mouvements = mouvements_total(&pool, p)?;
    assert_eq!(
        avant_stock, apres_stock,
        "le stock du produit {p} a bougé ({avant_stock} → {apres_stock}) : ce module \
         ne doit pas écrire de mouvement tant que le module de stock dédié n'existe pas"
    );
    assert_eq!(
        apres_mouvements, 0,
        "un mouvement de régularisation a été écrit : la quantité n'aurait plus \
         d'historique explicable"
    );

    // Le constat, lui, est bien là et vaut +12 : c'est tout ce que la pièce
    // promet.
    assert_eq!(db::inventaire::ecarts(&pool, ENTITY, id)?, dec("12"));

    nettoyer(&pool, prefixe)?;
    Ok(())
}
