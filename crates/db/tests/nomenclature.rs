//! Nomenclature : création, cycle de vie et coût de matière, sur la vraie base.
//!
//! Ces tests tournent **uniquement** si `DOLIBARR_DSN` est posée : sans base, ils
//! s'ignorent en le disant. Ils travaillent en entité 1 et préfixent toutes leurs
//! références (`ZZNOMA-`, `ZZNOMB-`…) pour pouvoir nettoyer derrière eux sans
//! toucher aux données réelles : chaque test a **son** préfixe, sinon deux tests
//! lancés en parallèle se supprimeraient mutuellement le travail.
//!
//! # Pourquoi ce `#[path]`
//!
//! La consigne de cette intervention interdit de modifier un fichier existant :
//! `crates/db/src/lib.rs` n'a donc pas encore de `pub mod nomenclature;`. Le
//! module est inclus ici par chemin, tel quel, sans copie. Le jour où `lib.rs`
//! le déclarera, on remplacera par `use db::nomenclature::{...};` et ce bloc
//! `#[path]` disparaîtra.
//!
//! # Les produits, lus et non inventés
//!
//! Le produit fini est **lu** parmi les produits qui existent déjà dans
//! l'entité 1 : on ne devine pas d'identifiant, et créer un faux produit fini
//! ne prouverait rien. S'il n'y a aucun produit, le test s'ignore avec un
//! message. En revanche, les composants **chiffrés** sont créés par les tests :
//! ils ont besoin d'un `cost_price` / `pmp` connu, et la base de référence n'en
//! porte aucun (mesuré : deux produits, tous deux sans prix, et zéro ligne dans
//! `llx_product_fournisseur_price`).

use db::nomenclature;

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use nomenclature::{NouveauComposant, NouvelleNomenclature};

/// Entité de travail : la base de référence est en entité 1.
const ENTITE: i64 = 1;

/// Préfixe de *tous* les objets créés par ce fichier. Les tests ajoutent leur
/// propre lettre (`ZZNOMA-`, `ZZNOMB-`…) pour ne pas se marcher dessus.
const PREFIXE: &str = "ZZNOM";

fn dec(valeur: &str) -> Decimal {
    Decimal::from_str(valeur).expect("décimal valide")
}

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

/// Efface nomenclatures et produits dont la référence commence par `prefixe`.
///
/// Un test qui échoue au milieu laisse des lignes : on nettoie donc **avant** et
/// **après**. Le produit fini, lui, appartient à la base et n'est jamais touché.
fn nettoyer(pool: &Pool, prefixe: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let motif = format!("{prefixe}%");

    let nomenclatures: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_bom_bom WHERE entity = :entity AND ref LIKE :motif",
        params! { "entity" => ENTITE, "motif" => motif.as_str() },
    )?;
    for id in nomenclatures {
        conn.exec_drop(
            "DELETE FROM llx_bom_bomline WHERE fk_bom = :bom",
            params! { "bom" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_bom_bom WHERE rowid = :bom",
            params! { "bom" => id },
        )?;
    }

    let produits: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_product WHERE entity = :entity AND ref LIKE :motif",
        params! { "entity" => ENTITE, "motif" => motif.as_str() },
    )?;
    for id in produits {
        conn.exec_drop(
            "DELETE FROM llx_product_fournisseur_price WHERE fk_product = :produit",
            params! { "produit" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_product_price WHERE fk_product = :produit",
            params! { "produit" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_product WHERE rowid = :produit",
            params! { "produit" => id },
        )?;
    }
    Ok(())
}

/// Le premier produit réellement présent dans l'entité 1, hors objets de test.
///
/// `None` (avec un message) s'il n'y en a aucun : le test s'ignore alors plutôt
/// que d'inventer un identifiant qui n'existe pas.
fn produit_fini(pool: &Pool) -> Result<Option<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let id: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_product \
         WHERE entity = :entity AND ref NOT LIKE :motif ORDER BY rowid LIMIT 1",
        params! { "entity" => ENTITE, "motif" => format!("{PREFIXE}%") },
    )?;
    if id.is_none() {
        eprintln!(
            "aucun produit dans l'entité 1 : test ignoré \
             (créez un produit pour l'exécuter)"
        );
    }
    Ok(id)
}

/// Un produit de test, avec son prix d'achat : `cost_price` ou, s'il est NULL,
/// `pmp` — les deux branches de la cascade de `BOM::calculateCosts()`.
fn creer_produit(
    pool: &Pool,
    reference: &str,
    cost_price: Option<&str>,
    pmp: &str,
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_product \
         (ref, label, entity, cost_price, pmp, fk_product_type, tosell, tobuy, datec) \
         VALUES (:ref, :label, :entity, :cout, :pmp, 0, 1, 1, NOW())",
        params! {
            "ref" => reference,
            "label" => format!("Composant de test {reference}"),
            "entity" => ENTITE,
            "cout" => cost_price,
            "pmp" => pmp,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Identifiant d'une unité du dictionnaire (`M2`, `M3`…), sans coder l'id en dur.
fn unite(pool: &Pool, code: &str) -> Result<Option<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let id: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_c_units WHERE code = ? AND active = 1 LIMIT 1",
        (code,),
    )?;
    Ok(id)
}

/// Un auteur réel : `llx_bom_bom.fk_user_creat` est `NOT NULL` **et** clé
/// étrangère vers `llx_user.rowid` — un identifiant inventé est refusé par la
/// base (erreur 1452). On préfère un utilisateur de l'entité 1, sinon n'importe
/// quel utilisateur de l'instance.
fn auteur(pool: &Pool) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let id: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_user ORDER BY (entity = :entity) DESC, rowid LIMIT 1",
        params! { "entity" => ENTITE },
    )?;
    // Une instance Dolibarr sans aucun utilisateur ne peut pas être ouverte :
    // ce repli ne sert qu'à rendre le message d'erreur lisible si cela arrive.
    Ok(id.unwrap_or(1))
}

/// L'en-tête d'un test : un produit fini réel, quantité 1.
fn entete(pool: &Pool, reference: &str, fini: i64) -> Result<NouvelleNomenclature, Box<dyn Error>> {
    let mut e = NouvelleNomenclature::new(
        reference,
        "Mur en pierre, 1 m²",
        fini,
        dec("1"),
        auteur(pool)?,
    );
    e.description = Some("Nomenclature de recette".to_string());
    Ok(e)
}

// ---------------------------------------------------------------------------
// Création et relecture
// ---------------------------------------------------------------------------

#[test]
fn cree_une_nomenclature_et_relit_ses_composants() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "ZZNOMA-")?;
    let Some(fini) = produit_fini(&pool)? else {
        return Ok(());
    };

    // Deux composants : un article en m² (la pierre) et une heure (la pose).
    let pierre = creer_produit(&pool, "ZZNOMA-PIERRE", Some("45.00"), "0")?;
    let pose = creer_produit(&pool, "ZZNOMA-POSE", None, "38.00")?;

    let mut pierres = NouveauComposant::new(pierre, dec("1.2"));
    pierres.fk_unit = unite(&pool, "M2")?;
    pierres.commentaire = Some("pierres de taille, avec pertes".to_string());
    let heures = NouveauComposant::new(pose, dec("2.5"));

    let e = entete(&pool, "ZZNOMA-MUR", fini)?;
    let id = nomenclature::creer(&pool, ENTITE, &e, &[pierres, heures])?;
    assert!(id > 0, "la nomenclature doit porter un identifiant");

    let lue = nomenclature::get(&pool, ENTITE, id)?.expect("nomenclature relue");
    assert_eq!(lue.ref_, "ZZNOMA-MUR");
    assert_eq!(lue.label, "Mur en pierre, 1 m²");
    assert_eq!(lue.qty, dec("1"));
    assert_eq!(lue.type_, nomenclature::TYPE_FABRICATION);
    assert_eq!(lue.fk_product, Some(fini));
    assert!(lue.est_brouillon(), "une création est un brouillon");
    assert_eq!(lue.description.as_deref(), Some("Nomenclature de recette"));
    assert!(lue.date_creation.is_some());
    assert!(
        lue.date_valid.is_none(),
        "un brouillon n'a pas de date de validation"
    );

    let composes = nomenclature::composants(&pool, id)?;
    assert_eq!(composes.len(), 2, "{composes:?}");
    assert_eq!(composes[0].fk_product, pierre);
    assert_eq!(composes[0].qty, dec("1.2"));
    assert_eq!(composes[0].position, 1, "les rangs commencent à 1");
    assert_eq!(composes[0].fk_unit, unite(&pool, "M2")?);
    assert_eq!(
        composes[0].commentaire.as_deref(),
        Some("pierres de taille, avec pertes")
    );
    assert_eq!(composes[1].fk_product, pose);
    assert_eq!(composes[1].qty, dec("2.5"));
    assert_eq!(composes[1].position, 2);
    assert_eq!(composes[1].fk_unit, None);
    assert_eq!(composes[1].commentaire, None);

    let liste = nomenclature::list(&pool, ENTITE, 500, 0)?;
    assert!(
        liste
            .iter()
            .any(|n| n.rowid == id && n.ref_ == "ZZNOMA-MUR"),
        "la nomenclature doit apparaître dans la liste"
    );

    nettoyer(&pool, "ZZNOMA-")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Refus d'entrée : rien d'écrit, message clair
// ---------------------------------------------------------------------------

#[test]
fn refuse_une_nomenclature_sans_composant() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "ZZNOMB-")?;
    let Some(fini) = produit_fini(&pool)? else {
        return Ok(());
    };

    let e = entete(&pool, "ZZNOMB-VIDE", fini)?;
    let erreur = nomenclature::creer(&pool, ENTITE, &e, &[]).expect_err("un vide doit être refusé");
    let message = erreur.to_string();
    assert!(
        message.contains("sans composant"),
        "le message doit dire pourquoi : {message}"
    );

    // Preuve qu'aucun en-tête orphelin n'a été écrit : une référence consommée
    // pour rien serait un numéro perdu dans la nomenclature du client.
    let liste = nomenclature::list(&pool, ENTITE, 500, 0)?;
    assert!(
        !liste.iter().any(|n| n.ref_ == "ZZNOMB-VIDE"),
        "un refus ne doit rien écrire"
    );

    nettoyer(&pool, "ZZNOMB-")?;
    Ok(())
}

#[test]
fn refuse_une_reference_ou_un_libelle_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    // Aucun produit n'est nécessaire : la référence et le libellé sont
    // contrôlés avant toute lecture en base. L'auteur, lui, est un vrai
    // utilisateur : sa colonne est une clé étrangère.
    let vide = NouvelleNomenclature::new("   ", "Sans référence", 1, dec("1"), auteur(&pool)?);
    let erreur = nomenclature::creer(&pool, ENTITE, &vide, &[NouveauComposant::new(2, dec("1"))])
        .expect_err("une référence vide doit être refusée");
    assert!(
        erreur.to_string().contains("référence"),
        "le message doit nommer la référence : {erreur}"
    );

    let sans_libelle = NouvelleNomenclature::new("ZZNOMF-LIB", "  ", 1, dec("1"), auteur(&pool)?);
    let erreur = nomenclature::creer(
        &pool,
        ENTITE,
        &sans_libelle,
        &[NouveauComposant::new(2, dec("1"))],
    )
    .expect_err("un libellé vide doit être refusé");
    assert!(
        erreur.to_string().contains("libellé"),
        "le message doit nommer le libellé : {erreur}"
    );
    Ok(())
}

#[test]
fn refuse_une_quantite_nulle() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "ZZNOMC-")?;
    let Some(fini) = produit_fini(&pool)? else {
        return Ok(());
    };
    let composant = creer_produit(&pool, "ZZNOMC-SABLE", Some("4.00"), "0")?;

    // Quantité produite nulle : la nomenclature ne décrit aucun ouvrage.
    let mut e = entete(&pool, "ZZNOMC-ZERO", fini)?;
    e.qty = Decimal::ZERO;
    let erreur = nomenclature::creer(
        &pool,
        ENTITE,
        &e,
        &[NouveauComposant::new(composant, dec("1"))],
    )
    .expect_err("une quantité produite nulle doit être refusée");
    assert!(
        erreur.to_string().contains("strictement positive"),
        "le message doit dire ce qui est attendu : {erreur}"
    );

    // Quantité de composant nulle : la ligne ne coûte rien et ne sert rien.
    let e = entete(&pool, "ZZNOMC-LIGNE", fini)?;
    let erreur = nomenclature::creer(
        &pool,
        ENTITE,
        &e,
        &[NouveauComposant::new(composant, Decimal::ZERO)],
    )
    .expect_err("une quantité de composant nulle doit être refusée");
    assert!(
        erreur.to_string().contains("non strictement positive"),
        "le message doit viser la ligne : {erreur}"
    );

    // Négative non plus : on ne retire pas de matière d'un ouvrage.
    let erreur = nomenclature::creer(
        &pool,
        ENTITE,
        &e,
        &[NouveauComposant::new(composant, dec("-1"))],
    )
    .expect_err("une quantité négative doit être refusée");
    assert!(erreur.to_string().contains("non strictement positive"));

    nettoyer(&pool, "ZZNOMC-")?;
    Ok(())
}

#[test]
fn refuse_un_composant_egal_au_produit_fini() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "ZZNOMD-")?;
    let Some(fini) = produit_fini(&pool)? else {
        return Ok(());
    };

    let e = entete(&pool, "ZZNOMD-BOUCLE", fini)?;
    let erreur = nomenclature::creer(&pool, ENTITE, &e, &[NouveauComposant::new(fini, dec("1"))])
        .expect_err("une pièce ne peut pas être son propre composant");
    let message = erreur.to_string();
    assert!(
        message.contains("une pièce ne se contient pas"),
        "le message doit être celui-là, en français : {message}"
    );
    assert!(
        message.contains(&format!("{fini}")),
        "il doit aussi nommer le produit fautif : {message}"
    );

    nettoyer(&pool, "ZZNOMD-")?;
    Ok(())
}

#[test]
fn refuse_un_produit_fini_inexistant() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    // Identifiant volontairement hors d'atteinte : la base de référence tient
    // dans quelques dizaines de produits.
    let inexistant = 9_000_000_001_i64;
    let e = NouvelleNomenclature::new(
        "ZZNOME-FANTOME",
        "Produit qui n'existe pas",
        inexistant,
        dec("1"),
        1,
    );
    let erreur = nomenclature::creer(
        &pool,
        ENTITE,
        &e,
        &[NouveauComposant::new(9_000_000_002, dec("1"))],
    )
    .expect_err("un produit fini inexistant doit être refusé");
    let message = erreur.to_string();
    assert!(
        message.contains("n'existe pas dans l'entité"),
        "le message doit dire que le produit manque : {message}"
    );
    assert!(message.contains(&inexistant.to_string()));
    Ok(())
}

// ---------------------------------------------------------------------------
// Cycle de vie
// ---------------------------------------------------------------------------

#[test]
fn validation_puis_refus_de_revalider() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "ZZNOMG-")?;
    let Some(fini) = produit_fini(&pool)? else {
        return Ok(());
    };
    let ciment = creer_produit(&pool, "ZZNOMG-CIMENT", Some("0.35"), "0")?;

    let e = entete(&pool, "ZZNOMG-VALIDE", fini)?;
    let id = nomenclature::creer(
        &pool,
        ENTITE,
        &e,
        &[NouveauComposant::new(ciment, dec("0.5"))],
    )?;

    nomenclature::valider(&pool, ENTITE, id)?;
    let validee = nomenclature::get(&pool, ENTITE, id)?.expect("nomenclature relue");
    assert_eq!(validee.statut, nomenclature::STATUT_VALIDEE);
    assert!(validee.est_validee());
    assert!(
        validee.date_valid.is_some(),
        "valider pose la date de validation, comme Dolibarr"
    );

    // Revalider masquerait un double traitement : on le refuse.
    let erreur = nomenclature::valider(&pool, ENTITE, id).expect_err("revalider doit échouer");
    assert!(erreur.to_string().contains("déjà validée"), "{erreur}");

    // Une pièce inconnue ne se valide pas non plus.
    let erreur = nomenclature::valider(&pool, ENTITE, 9_000_000_003)
        .expect_err("une pièce inconnue doit échouer");
    assert!(erreur.to_string().contains("n'existe pas"));

    nettoyer(&pool, "ZZNOMG-")?;
    Ok(())
}

#[test]
fn desactive_une_validee_et_refuse_de_la_supprimer() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "ZZNOMH-")?;
    let Some(fini) = produit_fini(&pool)? else {
        return Ok(());
    };
    let sable = creer_produit(&pool, "ZZNOMH-SABLE", Some("6.00"), "0")?;

    let e = entete(&pool, "ZZNOMH-SERVICE", fini)?;
    let id = nomenclature::creer(&pool, ENTITE, &e, &[NouveauComposant::new(sable, dec("3"))])?;

    // Un brouillon ne se désactive pas : il n'a jamais servi, il se supprime.
    let erreur = nomenclature::desactiver(&pool, ENTITE, id)
        .expect_err("un brouillon ne doit pas se désactiver");
    assert!(erreur.to_string().contains("brouillon"), "{erreur}");

    nomenclature::valider(&pool, ENTITE, id)?;

    // Une validée ne se supprime pas : elle justifie un chiffrage diffusé.
    let erreur = nomenclature::supprimer(&pool, ENTITE, id)
        .expect_err("une validée ne doit pas se supprimer");
    assert!(
        erreur.to_string().contains("ne se supprime pas"),
        "{erreur}"
    );

    nomenclature::desactiver(&pool, ENTITE, id)?;
    let desactivee = nomenclature::get(&pool, ENTITE, id)?.expect("nomenclature relue");
    assert_eq!(desactivee.statut, nomenclature::STATUT_DESACTIVEE);
    assert!(desactivee.est_desactivee());
    assert_eq!(
        nomenclature::composants(&pool, id)?.len(),
        1,
        "désactiver ne touche pas aux composants"
    );

    // Une désactivée n'est ni revalidable ni supprimable : c'est une trace.
    assert!(nomenclature::valider(&pool, ENTITE, id).is_err());
    let erreur = nomenclature::supprimer(&pool, ENTITE, id)
        .expect_err("une désactivée ne doit pas se supprimer");
    assert!(erreur.to_string().contains("trace"), "{erreur}");
    let erreur =
        nomenclature::desactiver(&pool, ENTITE, id).expect_err("désactiver deux fois doit échouer");
    assert!(erreur.to_string().contains("déjà désactivée"), "{erreur}");

    // Un brouillon, lui, disparaît vraiment, composants compris.
    let brouillon = entete(&pool, "ZZNOMH-BROUILLON", fini)?;
    let autre = nomenclature::creer(
        &pool,
        ENTITE,
        &brouillon,
        &[NouveauComposant::new(sable, dec("1"))],
    )?;
    nomenclature::supprimer(&pool, ENTITE, autre)?;
    assert!(nomenclature::get(&pool, ENTITE, autre)?.is_none());
    assert!(
        nomenclature::composants(&pool, autre)?.is_empty(),
        "les composants partent avec l'en-tête : aucune ligne orpheline"
    );

    nettoyer(&pool, "ZZNOMH-")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Coût de matière
// ---------------------------------------------------------------------------

#[test]
fn cout_matiere_echoue_en_nommant_le_produit_sans_prix() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "ZZNOMI-")?;
    let Some(fini) = produit_fini(&pool)? else {
        return Ok(());
    };

    // Aucun prix : `cost_price` NULL et `pmp` à zéro, et aucun prix fournisseur
    // (la base de référence n'en contient aucun). C'est exactement le cas où
    // Dolibarr écrit `unit_cost = 0`.
    let orphelin = creer_produit(&pool, "ZZNOMI-SANS-PRIX", None, "0")?;
    let e = entete(&pool, "ZZNOMI-INCOMPLET", fini)?;
    let id = nomenclature::creer(
        &pool,
        ENTITE,
        &e,
        &[NouveauComposant::new(orphelin, dec("2"))],
    )?;

    let erreur = nomenclature::cout_matiere(&pool, ENTITE, id)
        .expect_err("un composant sans prix doit faire échouer le coût");
    let message = erreur.to_string();
    assert!(
        message.contains("ZZNOMI-SANS-PRIX"),
        "le message doit nommer le produit manquant, sinon on ne sait pas lequel \
         corriger : {message}"
    );
    assert!(
        message.contains("aucun prix d'achat"),
        "et dire ce qui manque : {message}"
    );

    nettoyer(&pool, "ZZNOMI-")?;
    Ok(())
}

#[test]
fn cout_matiere_additionne_les_prix_connus() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "ZZNOMJ-")?;
    let Some(fini) = produit_fini(&pool)? else {
        return Ok(());
    };

    // Les trois branches de la lecture du prix :
    // - pierre   : cost_price = 12,50, quantité 2   → 25,00
    // - sable    : cost_price NULL, pmp = 3,00 (Dolibarr prend le PMP)
    // - ciment   : cost_price = 0,80, quantité 5    →  4,00
    let pierre = creer_produit(&pool, "ZZNOMJ-PIERRE", Some("12.50"), "9.99")?;
    let sable = creer_produit(&pool, "ZZNOMJ-SABLE", None, "3.00")?;
    let ciment = creer_produit(&pool, "ZZNOMJ-CIMENT", Some("0.80"), "0")?;

    let e = entete(&pool, "ZZNOMJ-MUR", fini)?;
    let id = nomenclature::creer(
        &pool,
        ENTITE,
        &e,
        &[
            NouveauComposant::new(pierre, dec("2")),
            NouveauComposant::new(sable, dec("1")),
            NouveauComposant::new(ciment, dec("5")),
        ],
    )?;

    assert_eq!(
        nomenclature::cout_matiere(&pool, ENTITE, id)?,
        dec("32.00"),
        "2 × 12,50 + 1 × 3,00 + 5 × 0,80 = 32,00 (le PMP sert quand cost_price est NULL)"
    );
    assert_eq!(nomenclature::quantite_totale(&pool, ENTITE, id)?, dec("8"));

    // Le coût est un total pour la quantité produite de l'en-tête, pas un coût
    // unitaire : avec 4 m², le même total vaut 32,00 mais l'unité 8,00.
    let mut quatre = entete(&pool, "ZZNOMJ-QUATRE", fini)?;
    quatre.qty = dec("4");
    let id4 = nomenclature::creer(
        &pool,
        ENTITE,
        &quatre,
        &[
            NouveauComposant::new(pierre, dec("2")),
            NouveauComposant::new(sable, dec("1")),
            NouveauComposant::new(ciment, dec("5")),
        ],
    )?;
    assert_eq!(
        nomenclature::cout_matiere(&pool, ENTITE, id4)?,
        dec("32.00")
    );

    nettoyer(&pool, "ZZNOMJ-")?;
    Ok(())
}
