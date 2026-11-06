//! Recette des variantes sur la vraie base, quand `DOLIBARR_DSN` est posée.
//!
//! Le module est inclus **par chemin** plutôt que par `db::variante` : `lib.rs`
//! n'est pas modifié par cette livraison, donc `pub mod variante;` n'y figure
//! pas encore. Un test d'intégration n'a pas besoin de cette déclaration pour
//! exercer le code — il compile le fichier dans son propre binaire. Le jour où
//! la ligne sera ajoutée à `lib.rs`, l'inclusion par chemin pourra laisser place
//! à l'appel habituel `db::variante::…`.
//!
//! ## Chaque test a son propre préfixe
//!
//! `cargo test` lance les tests en parallèle. Deux tests qui partageraient la
//! référence `TEST-VAR-TAILLE` se supprimeraient mutuellement : l'un nettoie
//! pendant que l'autre lit. Chaque test travaille donc sous son préfixe
//! (`TEST-VAR-1-`, `TEST-VAR-2-`…) et `menage()` ne balaie que celui-là. On
//! obtient l'isolation sans verrou ni dépendance supplémentaire.
//!
//! La base de travail contient de vraies données (deux produits, aucune
//! variante au 06/11/2026) : aucun test ne doit rien y laisser.

use db::variante;
use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

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

/// Efface ce qu'un test a pu laisser, et seulement sous son préfixe.
///
/// L'ordre suit les dépendances réelles : les lignes de `combination2val`
/// d'abord (elles n'ont pas d'`entity`, on les atteint par leur combinaison),
/// puis les combinaisons, les valeurs, les attributs, et enfin les produits.
fn menage(pool: &Pool, prefixe: &str) -> Result<(), Box<dyn Error>> {
    let motif = format!("{prefixe}%");
    let mut conn = pool.get_conn()?;

    let combinaisons: Vec<i64> = conn.exec(
        "SELECT c.rowid FROM llx_product_attribute_combination c \
         JOIN llx_product p ON p.rowid = c.fk_product_parent \
         WHERE p.ref LIKE :motif",
        params! { "motif" => motif.as_str() },
    )?;
    for id in combinaisons {
        conn.exec_drop(
            "DELETE FROM llx_product_attribute_combination2val \
             WHERE fk_prod_combination = :id",
            params! { "id" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_product_attribute_combination_price_level \
             WHERE fk_product_attribute_combination = :id",
            params! { "id" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_product_attribute_combination WHERE rowid = :id",
            params! { "id" => id },
        )?;
    }

    let valeurs: Vec<i64> = conn.exec(
        "SELECT v.rowid FROM llx_product_attribute_value v \
         JOIN llx_product_attribute a ON a.rowid = v.fk_product_attribute \
         WHERE a.ref LIKE :motif",
        params! { "motif" => motif.as_str() },
    )?;
    for id in valeurs {
        conn.exec_drop(
            "DELETE FROM llx_product_attribute_value WHERE rowid = :id",
            params! { "id" => id },
        )?;
    }
    conn.exec_drop(
        "DELETE FROM llx_product_attribute WHERE ref LIKE :motif",
        params! { "motif" => motif.as_str() },
    )?;

    let produits: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_product WHERE ref LIKE :motif AND entity = :entity",
        params! { "motif" => motif.as_str(), "entity" => ENTITY },
    )?;
    for id in produits {
        db::product::delete(pool, ENTITY, id)?;
    }
    Ok(())
}

fn produit(pool: &Pool, reference: &str, libelle: &str) -> Result<i64, Box<dyn Error>> {
    let nouveau = db::product::NewProduct::new(reference, libelle);
    db::product::create(pool, ENTITY, &nouveau)
}

fn valeur_en_double<'a>(
    valeurs: &'a [variante::Valeur],
    reference: &str,
) -> Option<&'a variante::Valeur> {
    valeurs.iter().find(|v| v.ref_ == reference)
}

#[test]
fn cree_un_attribut_et_ses_valeurs() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool, "TEST-VAR-1-")?;

    let attribut = variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-1-TAILLE", "Taille"),
    )?;
    assert!(attribut > 0);

    // Rejouer la création rend le même attribut : une resynchronisation ne
    // doit pas produire de doublon ni d'erreur au second passage.
    let encore = variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-1-TAILLE", "Taille"),
    )?;
    assert_eq!(encore, attribut);

    let mut taille_s = variante::NouvelleValeur::new(attribut, "S", "Small");
    taille_s.position = 1;
    let mut taille_m = variante::NouvelleValeur::new(attribut, "M", "Medium");
    taille_m.position = 2;
    let s = variante::creer_valeur(&pool, ENTITY, &taille_s)?;
    let m = variante::creer_valeur(&pool, ENTITY, &taille_m)?;
    assert!(s > 0 && m > 0 && s != m);

    let listes = variante::valeurs(&pool, attribut)?;
    assert_eq!(listes.len(), 2);
    // Tri par position : S (1) avant M (2), et non l'ordre alphabétique.
    assert_eq!(listes[0].rowid, s);
    assert_eq!(listes[0].ref_, "S");
    assert_eq!(listes[0].value, "Small");
    assert_eq!(listes[0].entity, ENTITY);
    assert_eq!(listes[1].rowid, m);
    assert_eq!(listes[1].ref_, "M");

    let attributs = variante::attributs(&pool, ENTITY)?;
    let lu = attributs
        .iter()
        .find(|a| a.rowid == attribut)
        .expect("l'attribut doit être listé");
    assert_eq!(lu.label, "Taille");
    assert_eq!(lu.ref_, "TEST-VAR-1-TAILLE");
    assert_eq!(lu.entity, ENTITY);

    // Aucune fuite vers une autre société.
    assert!(variante::attributs(&pool, ENTITY + 1)
        .unwrap()
        .iter()
        .all(|a| a.ref_ != "TEST-VAR-1-TAILLE"));

    menage(&pool, "TEST-VAR-1-")?;
    assert!(variante::attributs(&pool, ENTITY)?
        .iter()
        .all(|a| a.ref_ != "TEST-VAR-1-TAILLE"));
    Ok(())
}

#[test]
fn refuse_une_reference_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool, "TEST-VAR-2-")?;

    // Référence vide ou faite d'espaces : refusée avant tout accès à la base.
    assert!(variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("   ", "Taille")
    )
    .is_err());
    // Libellé vide : refusé aussi, un axe sans nom ne se lit pas à l'écran.
    assert!(variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-2-VIDE", "  ")
    )
    .is_err());

    let attribut = variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-2-TAILLE", "Taille"),
    )?;
    assert!(variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(attribut, "", "Small")
    )
    .is_err());
    assert!(variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(attribut, "S", "   ")
    )
    .is_err());

    // Les refus n'ont rien écrit.
    assert!(variante::valeurs(&pool, attribut)?.is_empty());

    menage(&pool, "TEST-VAR-2-")?;
    Ok(())
}

#[test]
fn refuse_une_valeur_sur_un_attribut_inexistant() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool, "TEST-VAR-3-")?;

    let erreur = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(999_999_999, "S", "Small"),
    )
    .expect_err("un attribut inexistant doit être refusé");
    assert!(
        format!("{erreur}").contains("n'existe pas"),
        "message inattendu : {erreur}"
    );

    // Un attribut réel, mais d'une autre société, est refusé de la même façon :
    // c'est le cloisonnement, pas seulement l'existence, qui est vérifié.
    let attribut = variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-3-TAILLE", "Taille"),
    )?;
    assert!(variante::creer_valeur(
        &pool,
        ENTITY + 1,
        &variante::NouvelleValeur::new(attribut, "S", "Small")
    )
    .is_err());

    menage(&pool, "TEST-VAR-3-")?;
    Ok(())
}

#[test]
fn refuse_une_valeur_en_double_dans_un_attribut() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool, "TEST-VAR-4-")?;

    let attribut = variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-4-TAILLE", "Taille"),
    )?;
    let premiere = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(attribut, "S", "Small"),
    )?;

    // Même référence dans le même attribut : la clé unique de Dolibarr est
    // (fk_product_attribute, ref), elle refuse le doublon.
    let erreur = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(attribut, "S", "Small bis"),
    )
    .expect_err("une référence déjà prise doit être refusée");
    assert!(
        format!("{erreur}").contains("déjà prise"),
        "message inattendu : {erreur}"
    );

    let listes = variante::valeurs(&pool, attribut)?;
    assert_eq!(listes.len(), 1);
    assert_eq!(listes[0].rowid, premiere);
    // Le libellé d'origine n'a pas été écrasé par la tentative.
    assert_eq!(listes[0].value, "Small");

    // La même référence dans un autre attribut est en revanche légitime :
    // « S » pour la taille et « S » pour la grandeur sont deux valeurs.
    let autre = variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-4-GRANDEUR", "Grandeur"),
    )?;
    assert!(variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(autre, "S", "Small")
    )
    .is_ok());

    menage(&pool, "TEST-VAR-4-")?;
    Ok(())
}

#[test]
fn cree_une_combinaison_a_deux_valeurs() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool, "TEST-VAR-5-")?;

    let taille = variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-5-TAILLE", "Taille"),
    )?;
    let couleur = variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-5-COULEUR", "Couleur"),
    )?;
    let s = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(taille, "S", "Small"),
    )?;
    let bleu = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(couleur, "BLEU", "Bleu"),
    )?;

    let pere = produit(&pool, "TEST-VAR-5-CHEMISE", "Chemise")?;
    let fils = produit(&pool, "TEST-VAR-5-CHEMISE-S-BLEU", "Chemise S bleu")?;

    let combinaison = variante::creer_combinaison(&pool, ENTITY, pere, &[s, bleu], Some(fils))?;
    assert!(combinaison > 0);

    let listes = variante::combinaisons(&pool, ENTITY, pere)?;
    assert_eq!(listes.len(), 1);
    assert_eq!(listes[0].rowid, combinaison);
    assert_eq!(listes[0].entity, ENTITY);
    assert_eq!(listes[0].fk_produit_pere, pere);
    assert_eq!(listes[0].fk_produit_fils, fils);
    // Aucun montant n'est demandé à la création : la variation reste nulle,
    // et elle est lue en `Decimal`, jamais en flottant.
    assert_eq!(listes[0].variation_prix, Decimal::ZERO);

    menage(&pool, "TEST-VAR-5-")?;
    Ok(())
}

#[test]
fn refuse_une_combinaison_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool, "TEST-VAR-6-")?;

    let pere = produit(&pool, "TEST-VAR-6-CHEMISE", "Chemise")?;

    // Une combinaison sans valeur ne décline rien : refusée, sans produit fils.
    let erreur = variante::creer_combinaison(&pool, ENTITY, pere, &[], None)
        .expect_err("une liste vide doit être refusée");
    assert!(
        format!("{erreur}").contains("au moins une valeur"),
        "message inattendu : {erreur}"
    );
    assert!(variante::combinaisons(&pool, ENTITY, pere)?.is_empty());

    // Un produit père qui n'existe pas dans cette société est refusé aussi.
    assert!(variante::creer_combinaison(&pool, ENTITY, 999_999_999, &[1], None).is_err());

    menage(&pool, "TEST-VAR-6-")?;
    Ok(())
}

#[test]
fn refuse_deux_valeurs_du_meme_attribut() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool, "TEST-VAR-7-")?;

    let taille = variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-7-TAILLE", "Taille"),
    )?;
    let s = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(taille, "S", "Small"),
    )?;
    let m = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(taille, "M", "Medium"),
    )?;

    let pere = produit(&pool, "TEST-VAR-7-CHEMISE", "Chemise")?;

    // « Taille S + Taille M » n'est pas une combinaison mais une contradiction :
    // le produit ne peut pas être simultanément deux tailles.
    let erreur = variante::creer_combinaison(&pool, ENTITY, pere, &[s, m], None)
        .expect_err("deux valeurs du même attribut doivent être refusées");
    assert!(
        format!("{erreur}").contains("même attribut"),
        "message inattendu : {erreur}"
    );
    assert!(variante::combinaisons(&pool, ENTITY, pere)?.is_empty());

    // La même valeur donnée deux fois est refusée avec son propre message.
    let erreur = variante::creer_combinaison(&pool, ENTITY, pere, &[s, s], None)
        .expect_err("une valeur répétée doit être refusée");
    assert!(
        format!("{erreur}").contains("deux fois"),
        "message inattendu : {erreur}"
    );

    // Rien n'a été écrit : la transaction n'a même pas été ouverte.
    assert!(variante::combinaisons(&pool, ENTITY, pere)?.is_empty());

    menage(&pool, "TEST-VAR-7-")?;
    Ok(())
}

#[test]
fn combinaisons_rend_les_bonnes_valeurs() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool, "TEST-VAR-8-")?;

    // Les positions des attributs décident de l'ordre de lecture :
    // la taille avant la couleur.
    let mut attr_taille = variante::NouvelAttribut::new("TEST-VAR-8-TAILLE", "Taille");
    attr_taille.position = 1;
    let mut attr_couleur = variante::NouvelAttribut::new("TEST-VAR-8-COULEUR", "Couleur");
    attr_couleur.position = 2;
    let taille = variante::creer_attribut(&pool, ENTITY, &attr_taille)?;
    let couleur = variante::creer_attribut(&pool, ENTITY, &attr_couleur)?;

    let s = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(taille, "S", "Small"),
    )?;
    let m = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(taille, "M", "Medium"),
    )?;
    let bleu = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(couleur, "BLEU", "Bleu"),
    )?;
    let rouge = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(couleur, "ROUGE", "Rouge"),
    )?;

    let pere = produit(&pool, "TEST-VAR-8-CHEMISE", "Chemise")?;
    let c1 = variante::creer_combinaison(&pool, ENTITY, pere, &[s, bleu], None)?;
    let c2 = variante::creer_combinaison(&pool, ENTITY, pere, &[m, rouge], None)?;

    let listes = variante::combinaisons(&pool, ENTITY, pere)?;
    assert_eq!(listes.len(), 2);

    let une = listes.iter().find(|c| c.rowid == c1).expect("première");
    let refs: Vec<&str> = une.valeurs.iter().map(|v| v.ref_.as_str()).collect();
    // Taille (position 1) avant Couleur (position 2) : S puis BLEU.
    assert_eq!(refs, vec!["S", "BLEU"]);
    assert_eq!(valeur_en_double(&une.valeurs, "S").unwrap().rowid, s);
    assert_eq!(valeur_en_double(&une.valeurs, "BLEU").unwrap().rowid, bleu);
    // Aucune contamination croisée entre les deux combinaisons.
    assert!(valeur_en_double(&une.valeurs, "M").is_none());
    assert!(valeur_en_double(&une.valeurs, "ROUGE").is_none());
    // Pas de produit enfant : la colonne est NOT NULL, c'est 0 qui le dit.
    assert_eq!(une.fk_produit_fils, 0);

    let autre = listes.iter().find(|c| c.rowid == c2).expect("seconde");
    let refs: Vec<&str> = autre.valeurs.iter().map(|v| v.ref_.as_str()).collect();
    assert_eq!(refs, vec!["M", "ROUGE"]);
    assert_eq!(
        valeur_en_double(&autre.valeurs, "ROUGE")
            .unwrap()
            .fk_attribut,
        couleur
    );

    // Toutes les valeurs rendues sont bien de cette société.
    assert!(une.valeurs.iter().all(|v| v.entity == ENTITY));

    // Une autre société ne voit aucune de ces combinaisons.
    assert!(variante::combinaisons(&pool, ENTITY + 1, pere)?.is_empty());

    menage(&pool, "TEST-VAR-8-")?;
    Ok(())
}

#[test]
fn supprime_une_combinaison() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    menage(&pool, "TEST-VAR-9-")?;

    let taille = variante::creer_attribut(
        &pool,
        ENTITY,
        &variante::NouvelAttribut::new("TEST-VAR-9-TAILLE", "Taille"),
    )?;
    let s = variante::creer_valeur(
        &pool,
        ENTITY,
        &variante::NouvelleValeur::new(taille, "S", "Small"),
    )?;
    let pere = produit(&pool, "TEST-VAR-9-CHEMISE", "Chemise")?;
    let combinaison = variante::creer_combinaison(&pool, ENTITY, pere, &[s], None)?;

    // Une société qui n'est pas la sienne ne supprime rien : les lignes de
    // `combination2val` n'ayant pas d'`entity`, c'est ce garde-fou qui empêche
    // un identifiant deviné d'effacer les valeurs d'une autre société.
    assert_eq!(
        variante::supprimer_combinaison(&pool, ENTITY + 1, combinaison)?,
        0
    );
    assert_eq!(variante::combinaisons(&pool, ENTITY, pere)?.len(), 1);

    // La bonne société supprime la combinaison et ses lignes.
    assert_eq!(
        variante::supprimer_combinaison(&pool, ENTITY, combinaison)?,
        1
    );
    assert!(variante::combinaisons(&pool, ENTITY, pere)?.is_empty());

    // Les lignes de liaison sont bien parties : sans cela, elles resteraient
    // orphelines, sans `entity` pour les rattacher un jour à quoi que ce soit.
    let mut conn = pool.get_conn()?;
    let restantes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_product_attribute_combination2val \
         WHERE fk_prod_combination = :id",
        params! { "id" => combinaison },
    )?;
    assert_eq!(restantes, Some(0));

    // Supprimer deux fois ne fait rien et ne se plaint pas.
    assert_eq!(
        variante::supprimer_combinaison(&pool, ENTITY, combinaison)?,
        0
    );

    menage(&pool, "TEST-VAR-9-")?;
    Ok(())
}
