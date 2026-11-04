//! Contrats : cycle de vie complet sur une entite de test isolee.
//!
//! Toute la matiere du test (tiers, contrats, lignes) vit dans `entity = 900501`
//! et sous le prefixe `ZZ-CTR-`. Un garde `Drop` nettoie meme si le test panique.
//! `assert` de total : les totaux stockes sont la somme des lignes **actives**
//! (statut 4), c'est la regle portee par `db::contrat`.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::contrat::{NewContrat, NewContratLine};

const ENTITY: i64 = 900_501;
const AUTRE_ENTITE: i64 = 900_502;
const PREFIXE: &str = "ZZ-CTR-";

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

/// Garde de nettoyage : supprime tout ce que le test a cree pour son tiers.
///
/// Best-effort volontaire : un `Drop` ne doit jamais masquer l'echec du test en
/// cours, donc aucune erreur de nettoyage n'est propagee.
struct Garde {
    pool: Pool,
    fk_soc: i64,
}

impl Drop for Garde {
    fn drop(&mut self) {
        let Ok(mut conn) = self.pool.get_conn() else {
            return;
        };
        let _ = conn.exec_drop(
            "DELETE d FROM llx_contratdet d \
             INNER JOIN llx_contrat c ON c.rowid = d.fk_contrat \
             WHERE c.entity = :entity AND c.fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => self.fk_soc },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_contrat WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => self.fk_soc },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_societe WHERE entity = :entity AND rowid = :soc",
            params! { "entity" => ENTITY, "soc" => self.fk_soc },
        );
    }
}

/// Cree un tiers de test propre et son garde de nettoyage.
fn nouveau_tiers(pool: &Pool, suffixe: &str) -> Result<(i64, Garde), Box<dyn Error>> {
    let nom = format!("{PREFIXE}{suffixe}");
    let mut conn = pool.get_conn()?;
    // Residus d'un run precedent interrompu avant le `Drop`.
    let anciens: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => nom.as_str() },
    )?;
    for soc in anciens {
        conn.exec_drop(
            "DELETE d FROM llx_contratdet d \
             INNER JOIN llx_contrat c ON c.rowid = d.fk_contrat \
             WHERE c.entity = :entity AND c.fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => soc },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_contrat WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => soc },
        )?;
        db::party::delete(pool, ENTITY, soc)?;
    }
    drop(conn);

    let fk_soc = db::party::create(pool, ENTITY, &db::party::NewParty::client(&nom))?;
    Ok((
        fk_soc,
        Garde {
            pool: pool.clone(),
            fk_soc,
        },
    ))
}

/// Cree un contrat de deux services : 1 x 100 a 20 % et 2 x 50 a 10 %.
fn contrat_deux_lignes(pool: &Pool, fk_soc: i64) -> Result<(i64, i64, i64), Box<dyn Error>> {
    let contrat = db::contrat::create(pool, ENTITY, &NewContrat::new(fk_soc))?;
    let l1 = db::contrat::add_line(
        pool,
        ENTITY,
        contrat,
        &NewContratLine::new("Entretien annuel", dec("1"), dec("100"), dec("20")),
    )?;
    let l2 = db::contrat::add_line(
        pool,
        ENTITY,
        contrat,
        &NewContratLine::new("Deplacement", dec("2"), dec("50"), dec("10")),
    )?;
    Ok((contrat, l1, l2))
}

#[test]
fn un_contrat_valide_totalise_ses_lignes_actives() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (fk_soc, _garde) = nouveau_tiers(&pool, "validation")?;

    let (contrat, l1, l2) = contrat_deux_lignes(&pool, fk_soc)?;

    let c = db::contrat::get(&pool, ENTITY, contrat)?.expect("contrat relu");
    assert_eq!(c.fk_soc, fk_soc);
    assert_eq!(c.entity, ENTITY);
    assert_eq!(c.statut, db::contrat::STATUT_BROUILLON);
    assert_eq!(
        c.ref_,
        format!("(PROV{contrat})"),
        "un brouillon porte la reference provisoire Dolibarr"
    );
    assert_eq!(c.total_ht, Some(dec("0")), "totaux a zero a la creation");
    assert!(c.est_brouillon());

    let lignes = db::contrat::lines(&pool, ENTITY, contrat)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(
        lignes[0].statut,
        db::contrat::LIGNE_ATTENTE,
        "une ligne nait en attente (0), comme `addline` dans Dolibarr"
    );
    assert_eq!(lignes[0].total_ttc, Some(dec("120")), "1 x 100 a 20 %");
    assert_eq!(lignes[1].total_ttc, Some(dec("110")), "2 x 50 a 10 %");

    db::contrat::valider(&pool, ENTITY, contrat)?;
    let c = db::contrat::get(&pool, ENTITY, contrat)?.expect("contrat valide");
    assert_eq!(c.statut, db::contrat::STATUT_VALIDE);
    assert_eq!(c.ref_.len(), 11, "CT + AAAMM + - + 4 chiffres: {}", c.ref_);
    assert!(c.ref_.starts_with("CT"), "reference definitive: {}", c.ref_);
    assert_eq!(
        c.total_ttc,
        Some(dec("0")),
        "validation sans service actif : la somme des lignes actives est nulle"
    );

    db::contrat::activer_ligne(&pool, ENTITY, l1)?;
    db::contrat::activer_ligne(&pool, ENTITY, l2)?;

    let c = db::contrat::get(&pool, ENTITY, contrat)?.expect("contrat actif");
    assert_eq!(c.total_ht, Some(dec("200")), "100 + 100");
    assert_eq!(c.total_tva, Some(dec("30")), "20 + 10");
    assert_eq!(c.total_ttc, Some(dec("230")), "120 + 110");

    let lignes = db::contrat::lines(&pool, ENTITY, contrat)?;
    assert!(lignes.iter().all(|l| l.est_active()));
    assert!(
        lignes[0].date_ouverture.is_some(),
        "l'activation date l'ouverture reelle"
    );

    assert!(
        db::contrat::valider(&pool, ENTITY, contrat).is_err(),
        "un contrat ne se valide qu'une fois"
    );
    Ok(())
}

#[test]
fn desactiver_une_ligne_baisse_le_total() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (fk_soc, _garde) = nouveau_tiers(&pool, "desactivation")?;

    let (contrat, l1, l2) = contrat_deux_lignes(&pool, fk_soc)?;
    db::contrat::valider(&pool, ENTITY, contrat)?;
    db::contrat::activer_ligne(&pool, ENTITY, l1)?;
    db::contrat::activer_ligne(&pool, ENTITY, l2)?;
    assert_eq!(
        db::contrat::get(&pool, ENTITY, contrat)?
            .expect("contrat")
            .total_ttc,
        Some(dec("230"))
    );

    db::contrat::desactiver_ligne(&pool, ENTITY, l1)?;
    let c = db::contrat::get(&pool, ENTITY, contrat)?.expect("contrat");
    assert_eq!(c.total_ht, Some(dec("100")), "il ne reste que la ligne 2");
    assert_eq!(c.total_tva, Some(dec("10")));
    assert_eq!(c.total_ttc, Some(dec("110")), "230 - 120");

    let ligne = db::contrat::lines(&pool, ENTITY, contrat)?
        .into_iter()
        .find(|l| l.rowid == l1)
        .expect("ligne 1 relue");
    assert_eq!(ligne.statut, db::contrat::LIGNE_CLOSE);
    assert!(!ligne.est_active());

    assert!(
        db::contrat::desactiver_ligne(&pool, ENTITY, l1).is_err(),
        "une ligne deja close ne se ferme pas deux fois"
    );
    Ok(())
}

#[test]
fn clore_le_contrat_le_passe_en_statut_clos() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (fk_soc, _garde) = nouveau_tiers(&pool, "cloture")?;

    let (contrat, l1, _l2) = contrat_deux_lignes(&pool, fk_soc)?;
    assert!(
        db::contrat::clore(&pool, ENTITY, contrat).is_err(),
        "un brouillon ne se clot pas"
    );
    db::contrat::valider(&pool, ENTITY, contrat)?;
    db::contrat::activer_ligne(&pool, ENTITY, l1)?;

    db::contrat::clore(&pool, ENTITY, contrat)?;
    let c = db::contrat::get(&pool, ENTITY, contrat)?.expect("contrat clos");
    assert_eq!(c.statut, db::contrat::STATUT_CLOS);
    assert!(c.est_clos());
    assert_eq!(
        c.total_ttc,
        Some(dec("120")),
        "clore ne ferme pas les services : le total reste la valeur du contrat"
    );
    assert!(
        db::contrat::clore(&pool, ENTITY, contrat).is_err(),
        "un contrat clos ne se reclot pas"
    );
    Ok(())
}

#[test]
fn valider_un_contrat_sans_ligne_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (fk_soc, _garde) = nouveau_tiers(&pool, "sans-ligne")?;

    let contrat = db::contrat::create(&pool, ENTITY, &NewContrat::new(fk_soc))?;
    let err =
        db::contrat::valider(&pool, ENTITY, contrat).expect_err("validation attendue en echec");
    assert!(
        err.to_string().contains("sans ligne"),
        "message inattendu: {err}"
    );
    assert_eq!(
        db::contrat::get(&pool, ENTITY, contrat)?
            .expect("contrat")
            .statut,
        db::contrat::STATUT_BROUILLON,
        "le contrat refuse reste en brouillon"
    );

    // Le tiers doit exister dans l'entite du contrat.
    assert!(
        db::contrat::create(&pool, ENTITY, &NewContrat::new(0)).is_err(),
        "un contrat sans tiers ne se cree pas"
    );
    Ok(())
}

#[test]
fn un_contrat_ne_sort_pas_de_son_entite() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (fk_soc, _garde) = nouveau_tiers(&pool, "cloisonnement")?;

    let avant = db::contrat::compte(&pool, ENTITY)?;
    let avant_autre = db::contrat::compte(&pool, AUTRE_ENTITE)?;

    let (contrat, l1, _l2) = contrat_deux_lignes(&pool, fk_soc)?;
    db::contrat::valider(&pool, ENTITY, contrat)?;

    assert_eq!(
        db::contrat::compte(&pool, ENTITY)?,
        avant + 1,
        "le contrat compte dans son entite"
    );
    assert_eq!(
        db::contrat::compte(&pool, AUTRE_ENTITE)?,
        avant_autre,
        "il ne compte pas ailleurs"
    );

    assert!(
        db::contrat::get(&pool, AUTRE_ENTITE, contrat)?.is_none(),
        "lecture croisee impossible"
    );
    assert!(
        db::contrat::lines(&pool, AUTRE_ENTITE, contrat)?.is_empty(),
        "les lignes suivent l'entite du contrat"
    );
    assert!(
        db::contrat::valider(&pool, AUTRE_ENTITE, contrat).is_err(),
        "on ne valide pas le contrat d'une autre entite"
    );
    assert!(
        db::contrat::activer_ligne(&pool, AUTRE_ENTITE, l1).is_err(),
        "on n'active pas la ligne d'une autre entite"
    );
    assert!(
        db::contrat::clore(&pool, AUTRE_ENTITE, contrat).is_err(),
        "on ne clot pas le contrat d'une autre entite"
    );

    let liste = db::contrat::list(&pool, AUTRE_ENTITE, 50, 0)?;
    assert!(
        !liste.iter().any(|c| c.rowid == contrat),
        "liste cloisonnee"
    );
    Ok(())
}
