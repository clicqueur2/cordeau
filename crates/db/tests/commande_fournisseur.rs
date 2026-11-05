//! Commandes fournisseurs : totaux, cycle de statuts, numérotation,
//! suppression et quantité commandée.
//!
//! Les tests s'exécutent en parallèle sur la même base : chaque test crée son
//! propre fournisseur (nom unique) et n'affirme rien sur un `COUNT(*)` global.
//! Les montants sont vérifiés à la main, ligne par ligne, pour que l'arithmétique
//! monétaire soit prouvée et pas seulement « cohérente avec elle-même ».

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::commande_fournisseur::{
    NewCommandeFournisseur, NewLigneAchat, STATUT_APPROUVEE, STATUT_BROUILLON, STATUT_COMMANDEE,
    STATUT_RECEPTIONNEE, STATUT_VALIDEE,
};

const ENTITY: i64 = 1;

/// Préfixe commun aux fiches de test : le nettoyage s'appuie dessus.
const PREFIXE: &str = "ZZ CF Test";

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("décimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test d'intégration ignoré");
            None
        }
    }
}

/// Supprime les restes d'un test précédent interrompu, pour que la suite soit
/// rejouable sans intervention manuelle.
///
/// L'ordre compte : `llx_commande_fournisseur.fk_soc` est une clé étrangère vers
/// `llx_societe`, donc les commandes partent avant le fournisseur.
fn nettoyer(pool: &Pool, suffixe: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom LIKE :motif",
        params! { "entity" => ENTITY, "motif" => format!("{PREFIXE} {suffixe}%") },
    )?;
    for id in ids {
        let commandes: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_commande_fournisseur WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for c in commandes {
            conn.exec_drop(
                "DELETE FROM llx_commande_fournisseurdet WHERE fk_commande = :c",
                params! { "c" => c },
            )?;
            // Dolibarr nomme `order_supplier` le type d'une commande fournisseur
            // dans `llx_element_element`. Ce module n'en pose aucun, mais une
            // fiche touchée par l'interface Dolibarr en aurait.
            conn.exec_drop(
                "DELETE FROM llx_element_element \
                 WHERE (targettype = 'order_supplier' AND fk_target = :c) \
                    OR (sourcetype = 'order_supplier' AND fk_source = :c)",
                params! { "c" => c },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_commande_fournisseur WHERE rowid = :c",
                params! { "c" => c },
            )?;
        }
        db::party::delete(pool, ENTITY, id)?;
    }
    Ok(())
}

/// Un fournisseur neuf, au nom unique : deux tests en parallèle ne peuvent pas
/// se marcher dessus.
fn fournisseur(pool: &Pool, suffixe: &str) -> Result<i64, Box<dyn Error>> {
    let nom = format!("{PREFIXE} {suffixe}");
    db::achat::creer_fournisseur(pool, ENTITY, &nom, None)
}

/// Une commande d'une ligne, prête à être validée.
fn commande_simple(fk_soc: i64, libelle: &str, qty: &str, prix: &str) -> NewCommandeFournisseur {
    let mut new = NewCommandeFournisseur::new(fk_soc);
    new.lines = vec![NewLigneAchat::new(libelle, dec(qty), dec(prix), dec("20"))];
    new
}

#[test]
fn une_commande_fournisseur_calcule_ses_totaux_au_centime() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Totaux")?;
    let fk_soc = fournisseur(&pool, "Totaux")?;

    let mut new = NewCommandeFournisseur::new(fk_soc);
    new.ref_supplier = Some("BC-FOURN-2026-11".to_string());
    new.date_livraison = Some("2026-11-20".to_string());
    new.note_private = Some("chantier rue des Carrières".to_string());
    new.lines = vec![
        // 2,5 x 40,00 = 100,00 HT ; TVA 20 % = 20,00 ; TTC 120,00.
        NewLigneAchat::new("Sable 0/4", dec("2.5"), dec("40"), dec("20")),
        // 3 x 33,33 = 99,99 ; remise 5 % -> 94,9905 -> 94,99 HT ;
        // TTC = 94,9905 x 1,10 = 104,48955 -> 104,49 ; TVA = 104,49 - 94,99 = 9,50.
        {
            let mut l = NewLigneAchat::new("Gravier 6/10", dec("3"), dec("33.33"), dec("10"));
            l.remise_percent = dec("5");
            l
        },
    ];

    let commande = db::commande_fournisseur::create(&pool, ENTITY, &new)?;
    let c = db::commande_fournisseur::get(&pool, ENTITY, commande)?.expect("commande relue");

    assert_eq!(c.fk_statut, STATUT_BROUILLON);
    assert_eq!(c.fk_soc, fk_soc);
    assert_eq!(c.ref_supplier.as_deref(), Some("BC-FOURN-2026-11"));
    assert_eq!(c.date_livraison.as_deref(), Some("2026-11-20"));
    assert!(c.date_creation.is_some(), "la date de création est posée");
    assert_eq!(c.fournisseur.as_deref(), Some("ZZ CF Test Totaux"));
    assert!(c.fk_mode_reglement.is_none());
    assert!(!c.billed);

    assert!(
        c.ref_.starts_with("CF"),
        "référence inattendue : {}",
        c.ref_
    );

    // 100,00 + 94,99 ; 20,00 + 9,50 ; 120,00 + 104,49.
    assert_eq!(c.total_ht, Some(dec("194.99")));
    assert_eq!(c.total_tva, Some(dec("29.50")));
    assert_eq!(c.total_ttc, Some(dec("224.49")));
    assert_eq!(
        c.total_ht.unwrap() + c.total_tva.unwrap(),
        c.total_ttc.unwrap(),
        "ht + tva doit valoir ttc au centime près"
    );
    assert_eq!(
        c.amount_ht, c.total_ht,
        "sans remise d'en-tête, amount_ht et total_ht coïncident"
    );

    let lignes = db::commande_fournisseur::lines(&pool, ENTITY, commande)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(lignes[0].label.as_deref(), Some("Sable 0/4"));
    assert_eq!(lignes[0].total_ht, Some(dec("100.00")));
    assert_eq!(lignes[0].total_tva, Some(dec("20.00")));
    assert_eq!(lignes[0].total_ttc, Some(dec("120.00")));
    assert_eq!(lignes[1].total_ht, Some(dec("94.99")));
    assert_eq!(lignes[1].total_tva, Some(dec("9.50")));
    assert_eq!(lignes[1].total_ttc, Some(dec("104.49")));
    assert_eq!(lignes[1].remise_percent, Some(dec("5")));
    assert_eq!(lignes[0].rang, 1, "le rang suit l'ordre de saisie");
    assert_eq!(lignes[1].rang, 2);
    assert!(!lignes[0].is_section());

    // La liste porte le nom du fournisseur, sans seconde requête par ligne.
    let liste = db::commande_fournisseur::list(&pool, ENTITY, 5000, 0)?;
    let vue = liste
        .iter()
        .find(|x| x.rowid == commande)
        .expect("la commande créée apparaît dans list()");
    assert_eq!(vue.fournisseur.as_deref(), Some("ZZ CF Test Totaux"));
    assert_eq!(vue.total_ttc, Some(dec("224.49")));

    // Une autre entité ne la voit pas.
    assert!(db::commande_fournisseur::get(&pool, 2, commande)?.is_none());

    nettoyer(&pool, "Totaux")?;
    assert!(db::commande_fournisseur::get(&pool, ENTITY, commande)?.is_none());
    Ok(())
}

#[test]
fn refuse_un_tiers_qui_n_est_pas_fournisseur() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Client")?;
    let nom = format!("{PREFIXE} Client");
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(&nom))?;

    let new = commande_simple(fk_soc, "Prestation", "1", "100");
    let erreur = db::commande_fournisseur::create(&pool, ENTITY, &new)
        .expect_err("un client n'est pas un fournisseur");
    let message = erreur.to_string();
    assert!(
        message.contains("n'est pas un fournisseur"),
        "message peu clair : {message}"
    );
    assert!(
        message.contains("fournisseur = 0"),
        "le message doit nommer la colonne à corriger : {message}"
    );

    // Rien n'a été écrit : le refus est prononcé avant la transaction.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_commande_fournisseur WHERE entity = :entity AND fk_soc = :soc",
        params! { "entity" => ENTITY, "soc" => fk_soc },
    )?;
    assert_eq!(restes.unwrap_or(0), 0, "un refus ne crée aucune commande");

    // Un identifiant inexistant est refusé lui aussi, avec un autre message.
    let fantome = commande_simple(999_999_999, "Prestation", "1", "100");
    assert!(db::commande_fournisseur::create(&pool, ENTITY, &fantome).is_err());

    nettoyer(&pool, "Client")?;
    Ok(())
}

#[test]
fn refuse_les_lignes_impossibles() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Lignes")?;
    let fk_soc = fournisseur(&pool, "Lignes")?;

    // Aucune ligne : une enveloppe vide, pas une commande.
    let vide = NewCommandeFournisseur::new(fk_soc);
    let erreur = db::commande_fournisseur::create(&pool, ENTITY, &vide)
        .expect_err("une commande sans ligne ne se crée pas");
    assert!(erreur.to_string().contains("sans ligne"), "{erreur}");

    // Quantité nulle.
    let mut zero = commande_simple(fk_soc, "Sable", "0", "40");
    let erreur = db::commande_fournisseur::create(&pool, ENTITY, &zero)
        .expect_err("zéro unité ne se commande pas");
    assert!(erreur.to_string().contains("quantité"), "{erreur}");

    // Quantité négative : le refus doit porter sur la quantité, pas ailleurs.
    zero.lines[0].qty = dec("-1");
    let erreur =
        db::commande_fournisseur::create(&pool, ENTITY, &zero).expect_err("quantité négative");
    assert!(erreur.to_string().contains("quantité"), "{erreur}");

    // Taux de TVA hors 0..=100.
    let mut taux = commande_simple(fk_soc, "Sable", "1", "40");
    taux.lines[0].tva_tx = dec("101");
    let erreur = db::commande_fournisseur::create(&pool, ENTITY, &taux).expect_err("TVA à 101 %");
    assert!(erreur.to_string().contains("TVA"), "{erreur}");

    taux.lines[0].tva_tx = dec("-1");
    assert!(db::commande_fournisseur::create(&pool, ENTITY, &taux).is_err());

    // Remise hors bornes : 150 % rendrait un montant négatif.
    let mut remise = commande_simple(fk_soc, "Sable", "1", "40");
    remise.lines[0].remise_percent = dec("150");
    assert!(db::commande_fournisseur::create(&pool, ENTITY, &remise).is_err());

    // Aucun de ces refus n'a consommé de numéro : ni en-tête ni ligne.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_commande_fournisseur WHERE entity = :entity AND fk_soc = :soc",
        params! { "entity" => ENTITY, "soc" => fk_soc },
    )?;
    assert_eq!(restes.unwrap_or(0), 0, "un refus ne crée aucune commande");

    nettoyer(&pool, "Lignes")?;
    Ok(())
}

#[test]
fn le_cycle_de_statuts_se_suit_et_se_refuse_hors_sequence() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Cycle")?;
    let fk_soc = fournisseur(&pool, "Cycle")?;

    let commande = db::commande_fournisseur::create(
        &pool,
        ENTITY,
        &commande_simple(fk_soc, "Parpaings", "10", "1.20"),
    )?;

    // Brouillon : tout ce qui suit est refusé, et dans l'ordre.
    assert!(
        db::commande_fournisseur::approuver(&pool, ENTITY, commande).is_err(),
        "un brouillon ne s'approuve pas"
    );
    assert!(
        db::commande_fournisseur::commander(&pool, ENTITY, commande, "2026-11-20").is_err(),
        "un brouillon ne se commande pas"
    );
    assert!(
        db::commande_fournisseur::receptionner(&pool, ENTITY, commande, "2026-11-25").is_err(),
        "un brouillon ne se réceptionne pas"
    );
    assert_eq!(
        db::commande_fournisseur::get(&pool, ENTITY, commande)?
            .unwrap()
            .fk_statut,
        STATUT_BROUILLON,
        "un refus ne déplace pas le statut"
    );

    // 0 -> 1.
    db::commande_fournisseur::valider(&pool, ENTITY, commande)?;
    let c = db::commande_fournisseur::get(&pool, ENTITY, commande)?.unwrap();
    assert_eq!(c.fk_statut, STATUT_VALIDEE);
    assert!(c.date_valid.is_some(), "la date de validation est posée");
    assert!(c.est_validee());

    assert!(
        db::commande_fournisseur::valider(&pool, ENTITY, commande).is_err(),
        "une commande ne se valide qu'une fois"
    );
    assert!(
        db::commande_fournisseur::receptionner(&pool, ENTITY, commande, "2026-11-25").is_err(),
        "une commande validée n'est pas encore commandée"
    );
    assert!(
        db::commande_fournisseur::commander(&pool, ENTITY, commande, "2026-11-20").is_err(),
        "une commande validée n'est pas encore approuvée"
    );

    // 1 -> 2.
    db::commande_fournisseur::approuver(&pool, ENTITY, commande)?;
    let c = db::commande_fournisseur::get(&pool, ENTITY, commande)?.unwrap();
    assert_eq!(c.fk_statut, STATUT_APPROUVEE);
    assert!(c.date_approve.is_some(), "la date d'approbation est posée");
    assert!(c.est_approuvee());

    assert!(
        db::commande_fournisseur::approuver(&pool, ENTITY, commande).is_err(),
        "une commande ne s'approuve qu'une fois"
    );
    assert!(
        db::commande_fournisseur::valider(&pool, ENTITY, commande).is_err(),
        "une commande approuvée ne redevient pas brouillon"
    );
    assert!(
        db::commande_fournisseur::receptionner(&pool, ENTITY, commande, "2026-11-25").is_err(),
        "une commande approuvée n'est pas encore commandée"
    );

    // Une date impossible est refusée **avant** toute écriture : le 31 février
    // existe au format AAAA-MM-JJ mais pas au calendrier.
    assert!(
        db::commande_fournisseur::commander(&pool, ENTITY, commande, "2026-02-31").is_err(),
        "le 31 février n'existe pas"
    );
    assert_eq!(
        db::commande_fournisseur::get(&pool, ENTITY, commande)?
            .unwrap()
            .fk_statut,
        STATUT_APPROUVEE,
        "une date refusée ne déplace pas le statut"
    );

    // 2 -> 3.
    db::commande_fournisseur::commander(&pool, ENTITY, commande, "2026-11-20")?;
    let c = db::commande_fournisseur::get(&pool, ENTITY, commande)?.unwrap();
    assert_eq!(c.fk_statut, STATUT_COMMANDEE);
    assert_eq!(c.date_commande.as_deref(), Some("2026-11-20"));
    assert!(c.est_commandee());

    assert!(
        db::commande_fournisseur::commander(&pool, ENTITY, commande, "2026-11-21").is_err(),
        "une commande ne part qu'une fois"
    );
    assert!(db::commande_fournisseur::approuver(&pool, ENTITY, commande).is_err());

    // 3 -> 4.
    db::commande_fournisseur::receptionner(&pool, ENTITY, commande, "2026-11-25")?;
    let c = db::commande_fournisseur::get(&pool, ENTITY, commande)?.unwrap();
    assert_eq!(c.fk_statut, STATUT_RECEPTIONNEE);
    assert_eq!(c.date_reception.as_deref(), Some("2026-11-25"));
    assert!(c.est_receptionnee());

    assert!(
        db::commande_fournisseur::receptionner(&pool, ENTITY, commande, "2026-11-26").is_err(),
        "une commande ne se réceptionne qu'une fois"
    );
    assert!(db::commande_fournisseur::commander(&pool, ENTITY, commande, "2026-11-26").is_err());

    // Une commande qui a servi ne se supprime pas.
    assert!(
        db::commande_fournisseur::supprimer(&pool, ENTITY, commande).is_err(),
        "une commande réceptionnée ne se supprime pas"
    );

    // Une commande inconnue est signalée, pas silencieusement ignorée.
    let erreur = db::commande_fournisseur::valider(&pool, ENTITY, 999_999_999)
        .expect_err("une commande inexistante est une erreur");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");

    nettoyer(&pool, "Cycle")?;
    Ok(())
}

#[test]
fn un_brouillon_se_supprime_mais_pas_une_commande_validee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Suppression")?;
    let fk_soc = fournisseur(&pool, "Suppression")?;

    let brouillon = db::commande_fournisseur::create(
        &pool,
        ENTITY,
        &commande_simple(fk_soc, "Ciment", "5", "8"),
    )?;
    db::commande_fournisseur::supprimer(&pool, ENTITY, brouillon)?;
    assert!(
        db::commande_fournisseur::get(&pool, ENTITY, brouillon)?.is_none(),
        "l'en-tête a disparu"
    );
    assert!(
        db::commande_fournisseur::lines(&pool, ENTITY, brouillon)?.is_empty(),
        "les lignes sont parties avec l'en-tête"
    );

    let validee = db::commande_fournisseur::create(
        &pool,
        ENTITY,
        &commande_simple(fk_soc, "Ciment", "5", "8"),
    )?;
    db::commande_fournisseur::valider(&pool, ENTITY, validee)?;
    let erreur = db::commande_fournisseur::supprimer(&pool, ENTITY, validee)
        .expect_err("une commande validée ne se supprime pas");
    assert!(
        erreur.to_string().contains("ne se supprime pas"),
        "{erreur}"
    );
    let c = db::commande_fournisseur::get(&pool, ENTITY, validee)?.expect("toujours là");
    assert_eq!(c.fk_statut, STATUT_VALIDEE);
    assert_eq!(
        db::commande_fournisseur::lines(&pool, ENTITY, validee)?.len(),
        1,
        "un refus de suppression ne touche pas les lignes"
    );

    nettoyer(&pool, "Suppression")?;
    Ok(())
}

#[test]
fn les_references_de_commande_fournisseur_sont_uniques() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Numeros")?;
    let fk_soc = fournisseur(&pool, "Numeros")?;

    let mut refs = Vec::new();
    for i in 0..3 {
        let id = db::commande_fournisseur::create(
            &pool,
            ENTITY,
            &commande_simple(fk_soc, &format!("Ligne {i}"), "1", "10"),
        )?;
        let c = db::commande_fournisseur::get(&pool, ENTITY, id)?.expect("commande relue");
        assert!(
            c.ref_.starts_with("CF"),
            "préfixe inattendu à l'itération {i} : {}",
            c.ref_
        );
        assert!(
            c.ref_.len() > 2,
            "une référence ne se réduit pas au préfixe : {}",
            c.ref_
        );
        refs.push(c.ref_);
    }

    let uniques: std::collections::BTreeSet<&String> = refs.iter().collect();
    assert_eq!(uniques.len(), 3, "références dupliquées : {refs:?}");

    // Les trois commandes du test existent bien, chacune avec sa ligne.
    for c in db::commande_fournisseur::list(&pool, ENTITY, 5000, 0)? {
        if c.fk_soc == fk_soc {
            assert_eq!(
                db::commande_fournisseur::lines(&pool, ENTITY, c.rowid)?.len(),
                1,
                "chaque en-tête a sa ligne"
            );
        }
    }

    nettoyer(&pool, "Numeros")?;
    Ok(())
}

#[test]
fn la_quantite_commandee_ignore_les_lignes_de_structure() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Quantite")?;
    let fk_soc = fournisseur(&pool, "Quantite")?;

    let mut new = NewCommandeFournisseur::new(fk_soc);
    new.lines = vec![
        NewLigneAchat::new("Sable 0/4", dec("4"), dec("40"), dec("20")),
        NewLigneAchat::new("Gravier 6/10", dec("6.5"), dec("33.33"), dec("10")),
    ];
    let commande = db::commande_fournisseur::create(&pool, ENTITY, &new)?;
    assert_eq!(
        db::commande_fournisseur::quantite_commandee(&pool, ENTITY, commande)?,
        dec("10.5"),
        "4 + 6,5"
    );

    // Une ligne de sous-total du module Subtotal porte `qty = 99` pour forcer
    // l'affichage du montant : l'additionner ferait commander 99 unités de rien.
    // Une ligne de structure (`product_type = 9`) n'en porte aucune.
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_commande_fournisseurdet \
         (fk_commande, label, tva_tx, qty, remise_percent, subprice, total_ht, \
          total_tva, total_ttc, product_type, special_code, rang) \
         VALUES (:c, 'Sous-total', '0', '99', '0', '0', '0', '0', '0', 9, 104777, 99)",
        params! { "c" => commande },
    )?;
    assert_eq!(
        db::commande_fournisseur::quantite_commandee(&pool, ENTITY, commande)?,
        dec("10.5"),
        "ni le sous-total à 99 ni la ligne de structure ne comptent"
    );

    let lignes = db::commande_fournisseur::lines(&pool, ENTITY, commande)?;
    assert_eq!(lignes.len(), 3, "la ligne de structure reste lisible");
    assert!(lignes[2].is_section());
    assert!(lignes[2].is_sous_total());

    // Une commande inexistante est une erreur, pas un zéro silencieux : un zéro
    // se lirait comme « rien à recevoir ».
    assert!(db::commande_fournisseur::quantite_commandee(&pool, ENTITY, 999_999_999).is_err());

    nettoyer(&pool, "Quantite")?;
    Ok(())
}
