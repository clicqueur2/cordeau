//! Transferts de stock : création, numérotation, cycle expédition/réception,
//! suppression, et surtout la preuve que **le stock ne bouge pas**.
//!
//! Chaque test crée ses propres entrepôts et produit sous un préfixe unique, et
//! les supprime à la fin : la base est partagée par tous les tests du dépôt.
//!
//! Les transferts, eux, sont numérotés `TR…` par `divers::suivant_ref` : leur
//! référence ne porte donc pas le préfixe du test. L'ancre du nettoyage, ce sont
//! les entrepôts créés pour lui — un transfert ne s'efface pas sans savoir d'où
//! il partait.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::transfert_stock::{
    NewLigneTransfert, NewTransfert, STATUT_BROUILLON, STATUT_EXPEDIE, STATUT_RECU,
};

const ENTITY: i64 = 1;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

/// Efface tout ce qu'un test a pu laisser : transferts, lignes, liens, puis
/// produit, mouvements et entrepôts.
///
/// Les lignes partent avant l'en-tête et les liens avant la pièce : le schéma
/// Dolibarr ne pose pas de clé étrangère, un reste orphelin ne serait donc
/// jamais signalé.
fn nettoyer(pool: &Pool, prefixe: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let entrepots: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_entrepot WHERE entity = :entity AND ref LIKE :motif",
        params! { "entity" => ENTITY, "motif" => format!("{prefixe}%") },
    )?;
    for &entrepot in &entrepots {
        let transferts: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_stocktransfer_stocktransfer \
             WHERE entity = :entity \
               AND (fk_warehouse_source = :entrepot OR fk_warehouse_destination = :entrepot)",
            params! { "entity" => ENTITY, "entrepot" => entrepot },
        )?;
        for transfert in transferts {
            conn.exec_drop(
                "DELETE FROM llx_element_element \
                 WHERE targettype = 'stocktransfer' AND fk_target = :transfert",
                params! { "transfert" => transfert },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_stocktransfer_stocktransferline \
                 WHERE fk_stocktransfer = :transfert",
                params! { "transfert" => transfert },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_stocktransfer_stocktransfer WHERE rowid = :transfert",
                params! { "transfert" => transfert },
            )?;
        }
    }

    let produits: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_product WHERE entity = :entity AND ref LIKE :motif",
        params! { "entity" => ENTITY, "motif" => format!("{prefixe}%") },
    )?;
    for produit in produits {
        conn.exec_drop(
            "DELETE FROM llx_stock_mouvement WHERE fk_product = :produit",
            params! { "produit" => produit },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_product_stock WHERE fk_product = :produit",
            params! { "produit" => produit },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_product WHERE rowid = :produit",
            params! { "produit" => produit },
        )?;
    }

    for &entrepot in &entrepots {
        conn.exec_drop(
            "DELETE FROM llx_entrepot WHERE rowid = :entrepot",
            params! { "entrepot" => entrepot },
        )?;
    }
    Ok(())
}

/// Un auteur réel : `fk_user_creat` est `NOT NULL` et porte une clé étrangère
/// vers `llx_user`, on ne veut donc pas d'un identifiant inventé. Sans
/// utilisateur en base, le test est ignoré plutôt que rouge.
fn auteur(pool: &Pool) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    conn.query_first::<i64, _>("SELECT rowid FROM llx_user ORDER BY rowid ASC LIMIT 1")
        .ok()
        .flatten()
}

/// Un produit et deux entrepôts neufs, propres au test.
struct Jeu {
    produit: i64,
    source: i64,
    destination: i64,
}

fn jeu(pool: &Pool, prefixe: &str) -> Result<Jeu, Box<dyn Error>> {
    nettoyer(pool, prefixe)?;
    let produit = db::product::create(
        pool,
        ENTITY,
        &db::product::NewProduct::new(&format!("{prefixe}-PROD"), "Moellons de réemploi"),
    )?;
    let source =
        db::stock::creer_entrepot(pool, ENTITY, &format!("{prefixe}-SRC"), Some("Charly"))?;
    let destination = db::stock::creer_entrepot(
        pool,
        ENTITY,
        &format!("{prefixe}-DST"),
        Some("Lyon"),
    )?;
    Ok(Jeu {
        produit,
        source,
        destination,
    })
}

/// Un brouillon d'une ligne, pour les tests qui n'examinent que les statuts.
fn brouillon(pool: &Pool, j: &Jeu, createur: i64) -> Result<i64, Box<dyn Error>> {
    let mut new = NewTransfert::new(j.source, j.destination, createur);
    new.lines = vec![NewLigneTransfert::new(j.produit, dec("1"))];
    db::transfert_stock::create(pool, ENTITY, &new)
}

#[test]
fn un_transfert_se_cree_et_se_relit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZTRANSFERTA";
    let j = jeu(&pool, prefixe)?;
    let Some(createur) = auteur(&pool) else {
        nettoyer(&pool, prefixe)?;
        return Ok(());
    };

    let mut new = NewTransfert::new(j.source, j.destination, createur);
    new.label = Some("Réassort chantier du haut".to_string());
    new.note_public = Some("Deux palettes, camion de 19 t".to_string());
    new.date_prevue_depart = Some("2026-11-10".to_string());
    new.date_prevue_arrivee = Some("2026-11-11".to_string());
    let mut premiere = NewLigneTransfert::new(j.produit, dec("2.5"));
    premiere.batch = Some("LOT-2026-11".to_string());
    premiere.pmp = Some(dec("18.40"));
    new.lines = vec![premiere, NewLigneTransfert::new(j.produit, dec("10"))];

    let id = db::transfert_stock::create(&pool, ENTITY, &new)?;

    let lue = db::transfert_stock::get(&pool, ENTITY, id)?.expect("transfert relu");
    assert!(
        lue.ref_.starts_with("TR"),
        "référence inattendue : {}",
        lue.ref_
    );
    assert_eq!(lue.entity, ENTITY);
    assert_eq!(lue.status, STATUT_BROUILLON);
    assert!(lue.est_brouillon());
    assert_eq!(lue.fk_warehouse_source, Some(j.source));
    assert_eq!(lue.fk_warehouse_destination, Some(j.destination));
    assert_eq!(lue.entrepot_source, Some(format!("{prefixe}-SRC")));
    assert_eq!(lue.entrepot_destination, Some(format!("{prefixe}-DST")));
    assert_eq!(lue.label.as_deref(), Some("Réassort chantier du haut"));
    assert_eq!(
        lue.note_public.as_deref(),
        Some("Deux palettes, camion de 19 t")
    );
    assert_eq!(lue.date_prevue_depart.as_deref(), Some("2026-11-10"));
    assert_eq!(lue.date_prevue_arrivee.as_deref(), Some("2026-11-11"));
    assert!(
        lue.date_reelle_depart.is_none(),
        "un brouillon n'est pas parti"
    );
    assert!(lue.date_reelle_arrivee.is_none());
    assert_eq!(lue.fk_user_creat, createur);
    assert_eq!(lue.nb_lignes, 2);
    assert_eq!(lue.quantite_totale, dec("12.5"), "2,5 + 10");

    let lignes = db::transfert_stock::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(lignes[0].fk_product, j.produit);
    assert_eq!(lignes[0].ref_produit, format!("{prefixe}-PROD"));
    assert_eq!(lignes[0].qty, dec("2.5"));
    assert_eq!(lignes[0].batch.as_deref(), Some("LOT-2026-11"));
    assert_eq!(lignes[0].pmp, Some(dec("18.40")));
    assert_eq!(lignes[0].amount, Some(dec("46.00")), "2,5 x 18,40");
    assert_eq!(lignes[0].rang, 0, "le rang suit l'ordre de saisie");
    // Les lignes recopient les entrepôts de l'en-tête.
    assert_eq!(lignes[0].fk_warehouse_source, j.source);
    assert_eq!(lignes[0].fk_warehouse_destination, j.destination);
    assert_eq!(lignes[1].rang, 1);
    assert!(
        lignes[1].batch.is_none(),
        "pas de lot inventé quand il n'y en a pas"
    );
    assert!(
        lignes[1].amount.is_none(),
        "sans PMP, aucune valorisation n'est inventée"
    );

    assert_eq!(
        db::transfert_stock::quantite_totale(&pool, ENTITY, id)?,
        dec("12.5")
    );

    // La liste rend le plus récent en premier : la pièce créée doit y figurer.
    let liste = db::transfert_stock::list(&pool, ENTITY, 50, 0)?;
    assert!(
        liste.iter().any(|t| t.rowid == id),
        "le transfert {id} manque dans la liste"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn un_transfert_vers_le_meme_entrepot_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZTRANSFERTB";
    let j = jeu(&pool, prefixe)?;
    let Some(createur) = auteur(&pool) else {
        nettoyer(&pool, prefixe)?;
        return Ok(());
    };

    let mut new = NewTransfert::new(j.source, j.source, createur);
    new.lines = vec![NewLigneTransfert::new(j.produit, dec("1"))];
    let erreur = db::transfert_stock::create(&pool, ENTITY, &new)
        .expect_err("transférer vers le même entrepôt n'est pas un transfert");
    assert!(
        erreur.to_string().contains("ce n'est pas un transfert"),
        "message inattendu : {erreur}"
    );

    // Aucune pièce n'a été créée pour rien.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_stocktransfer_stocktransfer \
         WHERE entity = :entity \
           AND (fk_warehouse_source = :source OR fk_warehouse_destination = :arrivee)",
        params! { "entity" => ENTITY, "source" => j.source, "arrivee" => j.source },
    )?;
    assert_eq!(restes, Some(0), "aucun transfert ne doit être enregistré");

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn un_transfert_sans_ligne_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZTRANSFERTC";
    let j = jeu(&pool, prefixe)?;
    let Some(createur) = auteur(&pool) else {
        nettoyer(&pool, prefixe)?;
        return Ok(());
    };

    let vide = NewTransfert::new(j.source, j.destination, createur);
    assert!(
        db::transfert_stock::create(&pool, ENTITY, &vide).is_err(),
        "un transfert sans marchandise n'a rien à déplacer"
    );

    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_stocktransfer_stocktransfer \
         WHERE entity = :entity AND fk_warehouse_source = :source",
        params! { "entity" => ENTITY, "source" => j.source },
    )?;
    assert_eq!(restes, Some(0), "aucune pièce ne doit être créée");

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn une_quantite_nulle_ou_negative_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZTRANSFERTD";
    let j = jeu(&pool, prefixe)?;
    let Some(createur) = auteur(&pool) else {
        nettoyer(&pool, prefixe)?;
        return Ok(());
    };

    let mut nulle = NewTransfert::new(j.source, j.destination, createur);
    nulle.lines = vec![NewLigneTransfert::new(j.produit, Decimal::ZERO)];
    assert!(
        db::transfert_stock::create(&pool, ENTITY, &nulle).is_err(),
        "une quantité nulle ne déplace rien"
    );

    let mut negative = NewTransfert::new(j.source, j.destination, createur);
    negative.lines = vec![NewLigneTransfert::new(j.produit, dec("-3"))];
    assert!(
        db::transfert_stock::create(&pool, ENTITY, &negative).is_err(),
        "un transfert négatif ferait entrer du stock par la sortie"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn un_entrepot_ou_un_produit_inexistant_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZTRANSFERTF";
    let j = jeu(&pool, prefixe)?;
    let Some(createur) = auteur(&pool) else {
        nettoyer(&pool, prefixe)?;
        return Ok(());
    };

    // Des identifiants très au-dessus du maximum : ils ne peuvent pas exister,
    // et ils n'en créent pas pour autant.
    let mut conn = pool.get_conn()?;
    let entrepot_absent: i64 = conn
        .query_first("SELECT COALESCE(MAX(rowid), 0) + 1000 FROM llx_entrepot")?
        .unwrap_or(1_000);
    let produit_absent: i64 = conn
        .query_first("SELECT COALESCE(MAX(rowid), 0) + 1000 FROM llx_product")?
        .unwrap_or(1_000);

    let mut mauvais_entrepot = NewTransfert::new(entrepot_absent, j.destination, createur);
    mauvais_entrepot.lines = vec![NewLigneTransfert::new(j.produit, dec("1"))];
    let erreur = db::transfert_stock::create(&pool, ENTITY, &mauvais_entrepot)
        .expect_err("un entrepôt inconnu doit être refusé");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message inattendu : {erreur}"
    );

    let mut mauvais_produit = NewTransfert::new(j.source, j.destination, createur);
    mauvais_produit.lines = vec![NewLigneTransfert::new(produit_absent, dec("1"))];
    let erreur = db::transfert_stock::create(&pool, ENTITY, &mauvais_produit)
        .expect_err("un produit inconnu doit être refusé");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message inattendu : {erreur}"
    );

    // Un auteur absent est refusé avant même d'ouvrir une connexion.
    let mut sans_auteur = NewTransfert::new(j.source, j.destination, 0);
    sans_auteur.lines = vec![NewLigneTransfert::new(j.produit, dec("1"))];
    let erreur = db::transfert_stock::create(&pool, ENTITY, &sans_auteur)
        .expect_err("fk_user_creat est obligatoire");
    assert!(
        erreur.to_string().contains("obligatoire"),
        "message inattendu : {erreur}"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn expedition_puis_reception_dans_l_ordre() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZTRANSFERTG";
    let j = jeu(&pool, prefixe)?;
    let Some(createur) = auteur(&pool) else {
        nettoyer(&pool, prefixe)?;
        return Ok(());
    };

    let id = brouillon(&pool, &j, createur)?;

    db::transfert_stock::expedier(&pool, ENTITY, id, "2026-11-12")?;
    let partie = db::transfert_stock::get(&pool, ENTITY, id)?.expect("transfert relu");
    assert_eq!(partie.status, STATUT_EXPEDIE);
    assert!(partie.est_expedie());
    assert_eq!(partie.date_reelle_depart.as_deref(), Some("2026-11-12"));
    assert!(
        partie.date_reelle_arrivee.is_none(),
        "parti n'est pas arrivé"
    );

    db::transfert_stock::recevoir(&pool, ENTITY, id, "2026-11-13")?;
    let arrivee = db::transfert_stock::get(&pool, ENTITY, id)?.expect("transfert relu");
    assert_eq!(arrivee.status, STATUT_RECU);
    assert!(arrivee.est_recu());
    assert_eq!(arrivee.date_reelle_arrivee.as_deref(), Some("2026-11-13"));
    assert_eq!(
        arrivee.date_reelle_depart.as_deref(),
        Some("2026-11-12"),
        "la réception ne réécrit pas la date de départ"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn les_transitions_hors_sequence_sont_refusees() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZTRANSFERTH";
    let j = jeu(&pool, prefixe)?;
    let Some(createur) = auteur(&pool) else {
        nettoyer(&pool, prefixe)?;
        return Ok(());
    };

    let id = brouillon(&pool, &j, createur)?;

    // On ne réceptionne pas une pièce jamais partie : la marchandise
    // apparaîtrait à destination sans avoir quitté la source.
    let erreur = db::transfert_stock::recevoir(&pool, ENTITY, id, "2026-11-13")
        .expect_err("réceptionner un brouillon ferait apparaître du stock");
    assert!(
        erreur.to_string().contains("n'est pas expédié"),
        "message inattendu : {erreur}"
    );

    // Une date au format français passerait la liaison SQL mais ferait échouer
    // la requête en mode strict : elle est refusée avant.
    assert!(
        db::transfert_stock::expedier(&pool, ENTITY, id, "12/11/2026").is_err(),
        "date au mauvais format"
    );

    db::transfert_stock::expedier(&pool, ENTITY, id, "2026-11-12")?;
    let erreur = db::transfert_stock::expedier(&pool, ENTITY, id, "2026-11-12")
        .expect_err("une pièce déjà expédiée ne repart pas");
    assert!(
        erreur.to_string().contains("déjà expédié"),
        "message inattendu : {erreur}"
    );

    db::transfert_stock::recevoir(&pool, ENTITY, id, "2026-11-13")?;
    let erreur = db::transfert_stock::recevoir(&pool, ENTITY, id, "2026-11-14")
        .expect_err("une pièce reçue ne se réceptionne pas deux fois");
    assert!(
        erreur.to_string().contains("déjà reçu"),
        "message inattendu : {erreur}"
    );
    let erreur = db::transfert_stock::expedier(&pool, ENTITY, id, "2026-11-14")
        .expect_err("une pièce reçue ne repart pas");
    assert!(
        erreur.to_string().contains("déjà reçu"),
        "message inattendu : {erreur}"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn la_suppression_est_refusee_apres_expedition() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZTRANSFERTI";
    let j = jeu(&pool, prefixe)?;
    let Some(createur) = auteur(&pool) else {
        nettoyer(&pool, prefixe)?;
        return Ok(());
    };

    // Un brouillon part avec ses lignes.
    let jete = brouillon(&pool, &j, createur)?;
    db::transfert_stock::supprimer(&pool, ENTITY, jete)?;
    assert!(
        db::transfert_stock::get(&pool, ENTITY, jete)?.is_none(),
        "le brouillon est parti"
    );
    assert!(
        db::transfert_stock::lines(&pool, ENTITY, jete)?.is_empty(),
        "les lignes sont parties avec l'en-tête"
    );

    // Une pièce expédiée prouve un départ : elle reste.
    let garde = brouillon(&pool, &j, createur)?;
    db::transfert_stock::expedier(&pool, ENTITY, garde, "2026-11-12")?;
    let erreur = db::transfert_stock::supprimer(&pool, ENTITY, garde)
        .expect_err("un transfert expédié ne se supprime pas");
    assert!(
        erreur.to_string().contains("ne se supprime pas"),
        "message inattendu : {erreur}"
    );
    assert!(
        db::transfert_stock::get(&pool, ENTITY, garde)?.is_some(),
        "la pièce refusée est toujours là"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

/// Le test qui compte : une pièce menée jusqu'au bout ne doit **rien** changer
/// au stock.
#[test]
fn la_reception_ne_fait_pas_bouger_le_stock() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZTRANSFERTJ";
    let j = jeu(&pool, prefixe)?;
    let Some(createur) = auteur(&pool) else {
        nettoyer(&pool, prefixe)?;
        return Ok(());
    };

    // Dix unités à la source, rien à destination.
    db::stock::mouvement(
        &pool,
        ENTITY,
        j.produit,
        j.source,
        dec("10"),
        db::stock::TYPE_ENTREE,
        Some("stock initial"),
    )?;

    let mut conn = pool.get_conn()?;
    let mouvements_avant: i64 = conn
        .exec_first(
            "SELECT COUNT(*) FROM llx_stock_mouvement WHERE fk_product = :produit",
            params! { "produit" => j.produit },
        )?
        .unwrap_or(0);
    let niveaux_avant: Vec<(i64, String)> = conn.exec(
        "SELECT fk_entrepot, CAST(reel AS CHAR) FROM llx_product_stock \
         WHERE fk_product = :produit ORDER BY fk_entrepot ASC",
        params! { "produit" => j.produit },
    )?;

    let id = brouillon(&pool, &j, createur)?;
    db::transfert_stock::expedier(&pool, ENTITY, id, "2026-11-12")?;
    db::transfert_stock::recevoir(&pool, ENTITY, id, "2026-11-13")?;

    // La pièce est allée au bout…
    let lue = db::transfert_stock::get(&pool, ENTITY, id)?.expect("transfert relu");
    assert_eq!(lue.status, STATUT_RECU);

    // … et pourtant le stock n'a pas bougé d'un gramme.
    assert_eq!(
        db::stock::quantite(&pool, j.produit, j.source)?,
        dec("10"),
        "la source n'a pas été décrémentée"
    );
    assert_eq!(
        db::stock::quantite(&pool, j.produit, j.destination)?,
        Decimal::ZERO,
        "la destination n'a pas été incrémentée"
    );

    let mouvements_apres: i64 = conn
        .exec_first(
            "SELECT COUNT(*) FROM llx_stock_mouvement WHERE fk_product = :produit",
            params! { "produit" => j.produit },
        )?
        .unwrap_or(0);
    assert_eq!(
        mouvements_apres, mouvements_avant,
        "aucun mouvement de stock n'est écrit par un transfert"
    );

    let niveaux_apres: Vec<(i64, String)> = conn.exec(
        "SELECT fk_entrepot, CAST(reel AS CHAR) FROM llx_product_stock \
         WHERE fk_product = :produit ORDER BY fk_entrepot ASC",
        params! { "produit" => j.produit },
    )?;
    assert_eq!(
        niveaux_apres, niveaux_avant,
        "les niveaux de stock sont identiques, ligne pour ligne"
    );

    nettoyer(&pool, prefixe)?;
    Ok(())
}

#[test]
fn les_transferts_sont_numerotes_sans_doublon() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let prefixe = "ZZTRANSFERTK";
    let j = jeu(&pool, prefixe)?;
    let Some(createur) = auteur(&pool) else {
        nettoyer(&pool, prefixe)?;
        return Ok(());
    };

    let mut references = Vec::new();
    for _ in 0..3 {
        let id = brouillon(&pool, &j, createur)?;
        references.push(
            db::transfert_stock::get(&pool, ENTITY, id)?
                .expect("transfert relu")
                .ref_,
        );
    }
    let uniques: BTreeSet<&String> = references.iter().collect();
    assert_eq!(uniques.len(), 3, "références dupliquées : {references:?}");
    for reference in &references {
        assert!(
            reference.starts_with("TR"),
            "référence inattendue : {reference}"
        );
    }

    nettoyer(&pool, prefixe)?;
    Ok(())
}
