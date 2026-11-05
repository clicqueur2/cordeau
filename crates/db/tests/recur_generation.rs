//! La generation d'une facture a partir d'une serie recurrente : une seule
//! facture par occurrence, en brouillon, avec les bonnes lignes, et l'echeance
//! marquee une fois le document ecrit.
//!
//! Tout ce qui est ecrit ici vit dans les entites de test `900201` et suivantes,
//! et porte le prefixe `ZZ-RGEN-`. Chaque test a **sa** propre entite : les tests
//! tournent en parallele, et un `generer_tout` ne doit pas ramasser la serie du
//! voisin. Le `Drop` du garde efface factures, lignes, serie, trace et tiers, y
//! compris quand une assertion echoue.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Premiere entite de test. Aucune n'existe chez Dolibarr : aucun document reel
/// ne peut y etre touche.
const ENTITY_BASE: i64 = 900_201;

/// Date de reference, fixe : le test ne depend pas de l'horloge de la machine.
const JOUR: &str = "2026-11-03";

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test ignore");
            None
        }
    }
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("montant de test")
}

/// Une ligne de modele a poser : libelle, quantite, prix unitaire, TVA, unite.
type Ligne = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Option<i64>,
);

/// Efface tout ce qu'un test a pu laisser, meme apres une panique.
///
/// L'ancrage est le tiers : chaque test cree le sien, et vit dans sa propre
/// entite.
struct Garde {
    pool: Pool,
    entity: i64,
    fk_soc: i64,
    id_serie: i64,
}

impl Drop for Garde {
    fn drop(&mut self) {
        let Ok(mut conn) = self.pool.get_conn() else {
            return;
        };
        let _ = conn.exec_drop(
            "DELETE d FROM llx_facturedet d \
               INNER JOIN llx_facture f ON f.rowid = d.fk_facture \
              WHERE f.entity = :entity AND f.fk_soc = :soc",
            params! { "entity" => self.entity, "soc" => self.fk_soc },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_facture WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => self.entity, "soc" => self.fk_soc },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_facturedet_rec WHERE fk_facture = :serie",
            params! { "serie" => self.id_serie },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_facture_rec WHERE rowid = :serie AND entity = :entity",
            params! { "serie" => self.id_serie, "entity" => self.entity },
        );
        // La trace d'emission de `db::recurrence`, cachee dans les constantes :
        // une seule ligne, nommee exactement, pour ne pas effacer celle d'un autre.
        let _ = conn.exec_drop(
            "DELETE FROM llx_const WHERE name = :nom AND entity = :entity",
            params! {
                "nom" => format!("DOLIBARR_RS_RECUR_facture_rec_{}", self.id_serie),
                "entity" => self.entity,
            },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_societe WHERE rowid = :soc AND entity = :entity",
            params! { "soc" => self.fk_soc, "entity" => self.entity },
        );
    }
}

/// Insere tiers, serie et lignes d'un modele, et rend le garde de nettoyage.
///
/// `total_ht` / `total_tva` / `total_ttc` restent a zero : la facture doit
/// recalculer ses totaux, pas recopier ceux du modele.
fn serie(
    pool: &Pool,
    entity: i64,
    etiquette: &str,
    date_when: &str,
    lignes: &[Ligne],
) -> Result<Garde, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_societe (nom, entity, datec) VALUES (:nom, :entity, NOW())",
        params! { "nom" => etiquette, "entity" => entity },
    )?;
    let fk_soc: i64 = conn
        .query_first("SELECT LAST_INSERT_ID()")?
        .ok_or("pas d'identifiant de tiers rendu")?;

    conn.exec_drop(
        "INSERT INTO llx_facture_rec \
           (titre, entity, fk_soc, datec, date_when, frequency, unit_frequency, \
            suspended, amount, fk_cond_reglement, nb_gen_done, nb_gen_max) \
         VALUES (:titre, :entity, :fk_soc, NOW(), :date_when, 1, 'm', 0, 0, 1, 0, NULL)",
        params! {
            "titre" => etiquette,
            "entity" => entity,
            "fk_soc" => fk_soc,
            "date_when" => date_when,
        },
    )?;
    let id_serie: i64 = conn
        .query_first("SELECT LAST_INSERT_ID()")?
        .ok_or("pas d'identifiant de serie rendu")?;

    for (rang, (label, qty, prix, tva, unite)) in lignes.iter().enumerate() {
        conn.exec_drop(
            "INSERT INTO llx_facturedet_rec \
               (fk_facture, fk_product, product_type, label, description, tva_tx, qty, \
                remise_percent, subprice, total_ht, total_tva, total_ttc, fk_unit, \
                special_code, rang) \
             VALUES (:serie, NULL, 0, :label, NULL, :tva, :qty, 0, :prix, 0, 0, 0, \
                     :unite, 0, :rang)",
            params! {
                "serie" => id_serie,
                "label" => label,
                "tva" => tva,
                "qty" => qty,
                "prix" => prix,
                "unite" => unite,
                "rang" => (rang + 1) as i32,
            },
        )?;
    }

    Ok(Garde {
        pool: pool.clone(),
        entity,
        fk_soc,
        id_serie,
    })
}

/// Nombre de factures du tiers : un compteur par test, insensible au reste.
fn factures_du_tiers(pool: &Pool, entity: i64, fk_soc: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_facture WHERE entity = :entity AND fk_soc = :soc",
        params! { "entity" => entity, "soc" => fk_soc },
    )?;
    Ok(n.unwrap_or(0))
}

/// La reference externe, telle qu'elle est ecrite en base.
fn ref_ext(pool: &Pool, entity: i64, facture: i64) -> Result<Option<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let valeur: Option<Option<String>> = conn.exec_first(
        "SELECT ref_ext FROM llx_facture WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => facture },
    )?;
    Ok(valeur.flatten())
}

/// Une echeance tombee produit une facture en brouillon, datee de l'echeance,
/// avec les lignes et les totaux du modele.
#[test]
fn une_serie_due_cree_une_facture_en_brouillon() -> Result<(), Box<dyn Error>> {
    let entity = ENTITY_BASE;
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = serie(
        &pool,
        entity,
        "ZZ-RGEN brouillon",
        "2026-10-05",
        &[
            ("Pose de pave", "2", "120.50", "20", Some(10)),
            ("Main d'oeuvre", "3", "48", "10", Some(5)),
        ],
    )?;

    let facture =
        db::recur_generation::generer(&pool, entity, "facture_rec", garde.id_serie, JOUR)?
            .expect("l'echeance du 05/10 est tombee, une facture doit naitre");
    assert_eq!(
        factures_du_tiers(&pool, entity, garde.fk_soc)?,
        1,
        "une seule facture"
    );

    let f = db::invoice::get(&pool, entity, facture)?.expect("facture relue");
    assert_eq!(f.entity, entity, "la facture reste dans son entite");
    assert_eq!(f.fk_soc, Some(garde.fk_soc));
    assert_eq!(
        f.fk_statut,
        db::invoice::STATUT_BROUILLON,
        "le document genere se relit : jamais valide tout seul"
    );
    assert!(f.est_brouillon());
    assert_eq!(
        f.datef.as_deref(),
        Some("2026-10-05"),
        "la facture porte la date de l'echeance, pas celle du traitement"
    );

    // Totaux recalcules depuis quantite x prix, pas recopies du modele (a zero).
    assert_eq!(f.total_ht, Some(dec("385")), "2 x 120,50 + 3 x 48");
    assert_eq!(f.total_ttc, Some(dec("447.60")));

    // La garde : la reference externe porte la source, la serie et l'echeance.
    let attendue = format!("PA-RECUR-facture_rec-{}-2026-10-05", garde.id_serie);
    assert_eq!(
        ref_ext(&pool, entity, facture)?.as_deref(),
        Some(attendue.as_str())
    );
    assert!(
        attendue.starts_with("PA-RECUR-"),
        "prefixe de garde inattendu : {attendue}"
    );

    let lignes = db::invoice::lines(&pool, entity, facture)?;
    assert_eq!(lignes.len(), 2, "deux prestations, aucun titre de lot");
    assert_eq!(lignes[0].label.as_deref(), Some("Pose de pave"));
    assert_eq!(lignes[0].qty, Some(dec("2")));
    assert_eq!(lignes[0].subprice, Some(dec("120.50")));
    assert_eq!(lignes[0].tva_tx, Some(dec("20")));
    assert_eq!(
        lignes[0].fk_unit,
        Some(10),
        "l'unite du modele est recopiee"
    );
    assert_eq!(lignes[1].label.as_deref(), Some("Main d'oeuvre"));
    assert_eq!(lignes[1].fk_unit, Some(5));
    Ok(())
}

/// Deux passages le meme jour pour la meme echeance : la seconde ne cree rien.
#[test]
fn un_second_appel_le_meme_jour_ne_cree_rien() -> Result<(), Box<dyn Error>> {
    let entity = ENTITY_BASE + 1;
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = serie(
        &pool,
        entity,
        "ZZ-RGEN doublon",
        "2026-10-05",
        &[("Reprise de seuil", "1", "300", "20", Some(2))],
    )?;

    let premiere =
        db::recur_generation::generer(&pool, entity, "facture_rec", garde.id_serie, JOUR)?
            .expect("premiere emission");
    let apres = factures_du_tiers(&pool, entity, garde.fk_soc)?;
    assert_eq!(apres, 1);

    // Sans marquage de l'echeance (c'est `generer_tout` qui le fait), la serie est
    // toujours due : c'est la reference externe, et elle seule, qui retient.
    let seconde =
        db::recur_generation::generer(&pool, entity, "facture_rec", garde.id_serie, JOUR)?;
    assert_eq!(seconde, None, "l'occurrence est deja emise");
    assert_eq!(
        factures_du_tiers(&pool, entity, garde.fk_soc)?,
        apres,
        "le compteur de factures ne bouge pas"
    );

    // Une troisieme fois, puis le lendemain : meme cle, meme reponse.
    assert_eq!(
        db::recur_generation::generer(&pool, entity, "facture_rec", garde.id_serie, JOUR)?,
        None
    );
    // La cle porte l'echeance : le lendemain, tant que le curseur n'a pas avance,
    // l'occurrence est la meme et reste bloquee.
    assert_eq!(
        db::recur_generation::generer(&pool, entity, "facture_rec", garde.id_serie, "2026-11-04")?,
        None,
        "un passage le lendemain ne recree pas l'occurrence du 05/10"
    );
    assert_eq!(factures_du_tiers(&pool, entity, garde.fk_soc)?, apres);

    let f = db::invoice::get(&pool, entity, premiere)?.expect("facture toujours la");
    assert!(f.est_brouillon());
    Ok(())
}

/// `generer_tout` facture ce qui est du et marque l'echeance honoree : le
/// passage suivant ne rend plus la serie.
#[test]
fn generer_tout_marque_l_echeance_honoree() -> Result<(), Box<dyn Error>> {
    let entity = ENTITY_BASE + 2;
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = serie(
        &pool,
        entity,
        "ZZ-RGEN tout",
        "2026-09-15",
        &[("Entretien annuel", "1", "450", "20", None)],
    )?;

    let bilan = db::recur_generation::generer_tout(&pool, entity, JOUR, 500)?;
    let trouvee = bilan
        .creees
        .iter()
        .find(|(source, id, _)| source == "facture_rec" && *id == garde.id_serie)
        .ok_or("la serie due n'a pas ete traitee")?;
    assert_eq!(trouvee.0, "facture_rec");
    assert_eq!(trouvee.1, garde.id_serie);
    let facture = trouvee.2;

    let f = db::invoice::get(&pool, entity, facture)?.expect("facture relue");
    assert!(f.est_brouillon(), "genere ne veut pas dire valide");
    assert_eq!(f.total_ttc, Some(dec("540")), "450 HT a 20 %");

    // L'echeance est honoree : le curseur est ecrit dans les constantes.
    let mut conn = pool.get_conn()?;
    let trace: Option<String> = conn.exec_first(
        "SELECT value FROM llx_const WHERE name = :nom AND entity = :entity",
        params! {
            "nom" => format!("DOLIBARR_RS_RECUR_facture_rec_{}", garde.id_serie),
            "entity" => entity,
        },
    )?;
    assert_eq!(trace.as_deref(), Some(JOUR), "l'echeance n'est pas marquee");

    // Second passage : la serie n'est plus due, rien de neuf.
    let avant = factures_du_tiers(&pool, entity, garde.fk_soc)?;
    let encore = db::recur_generation::generer_tout(&pool, entity, JOUR, 500)?;
    assert!(
        !encore
            .creees
            .iter()
            .any(|(source, id, _)| source == "facture_rec" && *id == garde.id_serie),
        "la serie est encore rendue apres marquage"
    );
    assert_eq!(
        factures_du_tiers(&pool, entity, garde.fk_soc)?,
        avant,
        "aucune facture supplementaire"
    );
    Ok(())
}

/// Une serie d'une entite ne se facture pas dans une autre.
#[test]
fn generer_respecte_l_entite() -> Result<(), Box<dyn Error>> {
    let entity = ENTITY_BASE + 3;
    let ailleurs = entity + 1;
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = serie(
        &pool,
        entity,
        "ZZ-RGEN entite",
        "2026-10-05",
        &[("Taille de pierre", "1", "800", "20", Some(9))],
    )?;

    // Vu d'une autre entite, la serie n'a pas d'echeance due : on refuse.
    assert!(
        db::recur_generation::generer(&pool, ailleurs, "facture_rec", garde.id_serie, JOUR)
            .is_err(),
        "une serie ne se facture pas depuis une autre entite"
    );

    let traitees = db::recur_generation::generer_tout(&pool, ailleurs, JOUR, 500)?;
    assert!(
        !traitees
            .creees
            .iter()
            .any(|(_, id, _)| *id == garde.id_serie),
        "la serie fuit hors de son entite"
    );
    assert_eq!(
        factures_du_tiers(&pool, entity, garde.fk_soc)?,
        0,
        "aucune facture ecrite pour cette serie"
    );

    // Dans son entite, elle se facture normalement.
    let facture =
        db::recur_generation::generer(&pool, entity, "facture_rec", garde.id_serie, JOUR)?
            .expect("facture creee dans l'entite de la serie");
    assert_eq!(
        db::invoice::get(&pool, entity, facture)?
            .expect("facture")
            .entity,
        entity
    );
    assert_eq!(factures_du_tiers(&pool, entity, garde.fk_soc)?, 1);
    Ok(())
}

/// Les garde-fous : source non facturable ici, identifiant bidon, echeance qui
/// n'est pas due. Rien ne doit etre ecrit.
#[test]
fn refuse_ce_qui_n_est_pas_une_facture_client_due() -> Result<(), Box<dyn Error>> {
    let entity = ENTITY_BASE + 4;
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = serie(
        &pool,
        entity,
        "ZZ-RGEN refus",
        "2026-12-15",
        &[("Travaux a venir", "1", "100", "20", None)],
    )?;

    assert!(
        db::recur_generation::generer(&pool, entity, "facture_fourn_rec", 1, JOUR).is_err(),
        "une facture fournisseur ne se genere pas ici"
    );
    assert!(
        db::recur_generation::generer(&pool, entity, "facture_rec", 0, JOUR).is_err(),
        "identifiant nul accepte"
    );
    assert!(
        db::recur_generation::generer(&pool, entity, "facture_rec", garde.id_serie, JOUR).is_err(),
        "une echeance future ne se facture pas en avance"
    );
    assert_eq!(
        factures_du_tiers(&pool, entity, garde.fk_soc)?,
        0,
        "aucune facture ne doit exister apres des refus"
    );
    Ok(())
}
