//! Interventions (`fichinter`) : création, cycle de vie, durées.
//!
//! Tout se passe dans l'entité 1, et chaque test porte son **propre nom de
//! tiers** : ils tournent en parallèle, deux tests qui partageraient le même nom
//! se détruiraient mutuellement au nettoyage. Le ménage se fait avant autant
//! qu'après, pour qu'un test interrompu ne fasse pas échouer le suivant.
//!
//! Les tests sont aussi sérialisés par un verrou : la numérotation lit
//! `MAX(ref) + 1` sur la même table, et cinq créations concurrentes sur
//! `llx_fichinter` se disputeraient la même référence (et les mêmes verrous
//! InnoDB) plutôt que d'écrire des choses différentes.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;

use db::fichinter::{
    NewFichinter, NewFichinterLine, STATUT_BROUILLON, STATUT_CLOTURE, STATUT_VALIDE,
};
use mysql::prelude::Queryable;
use mysql::{params, Pool};
use rust_decimal::Decimal;

const ENTITY: i64 = 1;

/// Les tests partagent la table de numérotation : on les sérialise.
static SERIE: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// Supprime les interventions du tiers de test, puis le tiers.
///
/// Indispensable avant de supprimer le tiers : `llx_fichinter.fk_soc` porte une
/// vraie clé étrangère, et `llx_fichinterdet.fk_fichinter` aussi — les lignes
/// doivent partir avant leur en-tête.
fn nettoyer(pool: &Pool, nom: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let fiches: Vec<i64> = conn.exec(
        "SELECT f.rowid FROM llx_fichinter f \
         INNER JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE f.entity = :entity AND s.nom = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    for fiche in fiches {
        conn.exec_drop(
            "DELETE FROM llx_fichinterdet WHERE fk_fichinter = :fiche",
            params! { "fiche" => fiche },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_fichinter WHERE entity = :entity AND rowid = :fiche",
            params! { "entity" => ENTITY, "fiche" => fiche },
        )?;
    }
    let socs: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    drop(conn);
    for soc in socs {
        db::party::delete(pool, ENTITY, soc)?;
    }
    Ok(())
}

fn tiers(pool: &Pool, nom: &str) -> Result<i64, Box<dyn Error>> {
    db::party::create(pool, ENTITY, &db::party::NewParty::client(nom))
}

/// L'intervention de référence : deux lignes, une durée d'en-tête.
fn intervention(fk_soc: i64) -> NewFichinter {
    let mut new = NewFichinter::new(fk_soc);
    new.datei = "2026-03-02".into();
    new.description = "Reprise du linteau de la grange".into();
    new.duree = Some(dec("45.5"));
    new.lines = vec![
        NewFichinterLine::new("Dépose du linteau fendu", 30),
        NewFichinterLine::new("Pose du linteau neuf", 90),
    ];
    new
}

#[test]
fn cree_une_intervention_et_la_relit() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "ZZ-FI-Creation";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let mut new = intervention(fk_soc);
    new.ref_client = Some("BC-2026-14".into());
    new.note_private = Some("Prévoir l'étaiement du mur mitoyen".into());
    new.note_public = Some("Travaux réalisés sans interruption de la circulation".into());
    let id = db::fichinter::create(&pool, ENTITY, &new)?;

    let f = db::fichinter::get(&pool, ENTITY, id)?.expect("intervention relue");
    assert_eq!(f.rowid, id);
    assert_eq!(f.entity, ENTITY);
    assert_eq!(f.fk_soc, fk_soc);
    assert_eq!(
        f.tiers.as_deref(),
        Some(nom),
        "le nom du tiers vient de la jointure"
    );
    assert!(
        f.ref_.starts_with("FI"),
        "référence Dolibarr attendue, trouvée: {}",
        f.ref_
    );
    assert_eq!(f.statut, STATUT_BROUILLON);
    assert!(f.est_brouillon());
    assert!(!f.est_validee() && !f.est_cloturee());
    assert_eq!(f.datei.as_deref(), Some("2026-03-02"));
    assert!(f.datec.is_some(), "la saisie est datée");
    assert_eq!(f.date_valid, None, "un brouillon n'est pas validé");
    assert_eq!(f.datee, None, "un brouillon n'a pas de date de fin");
    assert_eq!(f.duree, Some(dec("45.5")), "durée d'en-tête en minutes");
    assert_eq!(
        f.description.as_deref(),
        Some("Reprise du linteau de la grange")
    );
    assert_eq!(
        f.note_private.as_deref(),
        Some("Prévoir l'étaiement du mur mitoyen")
    );
    assert_eq!(
        f.note_public.as_deref(),
        Some("Travaux réalisés sans interruption de la circulation")
    );
    assert_eq!(f.ref_client.as_deref(), Some("BC-2026-14"));
    assert_eq!(
        f.fk_projet, None,
        "aucun projet : 0 en base, None à la lecture"
    );

    let lignes = db::fichinter::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(lignes[0].fk_fichinter, id);
    assert_eq!(
        lignes[0].description.as_deref(),
        Some("Dépose du linteau fendu")
    );
    assert_eq!(lignes[0].duree, 30);
    assert_eq!(lignes[0].rang, 1, "première ligne du bon");
    assert_eq!(
        lignes[0].date.as_deref(),
        Some("2026-03-02 00:00"),
        "une ligne sans date tombe le jour de l'intervention"
    );
    assert_eq!(lignes[1].duree, 90);
    assert_eq!(lignes[1].rang, 2);
    assert_eq!(lignes[1].date.as_deref(), Some("2026-03-02 00:00"));

    assert_eq!(
        db::fichinter::duree_totale(&pool, ENTITY, id)?,
        dec("165.5"),
        "45,5 minutes d'en-tête + 30 + 90"
    );

    let liste = db::fichinter::list(&pool, ENTITY, 50, 0)?;
    assert!(
        liste.iter().any(|x| x.rowid == id),
        "l'intervention créée apparaît dans la liste"
    );

    // Une ligne peut porter sa propre date et son propre horaire.
    let mut datee = NewFichinter::new(fk_soc);
    datee.datei = "2026-03-02".into();
    datee.lines = vec![NewFichinterLine {
        date: Some("2026-03-03 08:30:00".into()),
        ..NewFichinterLine::new("Reprise du enduit", 60)
    }];
    let id_datee = db::fichinter::create(&pool, ENTITY, &datee)?;
    let ligne = db::fichinter::lines(&pool, ENTITY, id_datee)?
        .into_iter()
        .next()
        .expect("ligne datée");
    assert_eq!(ligne.date.as_deref(), Some("2026-03-03 08:30"));

    nettoyer(&pool, nom)?;
    assert!(
        db::fichinter::get(&pool, ENTITY, id)?.is_none(),
        "le ménage a bien tout emporté"
    );
    Ok(())
}

#[test]
fn cree_une_intervention_sans_ligne() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "ZZ-FI-SansLigne";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    // L'artisan est passé, il constate : une phrase suffit, sans ligne de détail.
    let mut new = NewFichinter::new(fk_soc);
    new.datei = "2026-04-11".into();
    new.description = "Constat de fuite sur la descente d'eaux pluviales".into();
    new.duree = Some(dec("20"));
    let id = db::fichinter::create(&pool, ENTITY, &new)?;

    assert!(
        db::fichinter::lines(&pool, ENTITY, id)?.is_empty(),
        "une intervention sans ligne est licite"
    );
    let f = db::fichinter::get(&pool, ENTITY, id)?.expect("intervention simple");
    assert_eq!(f.statut, STATUT_BROUILLON);
    assert_eq!(
        f.description.as_deref(),
        Some("Constat de fuite sur la descente d'eaux pluviales")
    );
    assert_eq!(
        db::fichinter::duree_totale(&pool, ENTITY, id)?,
        dec("20"),
        "sans ligne, seule la durée d'en-tête compte"
    );

    // Sans aucune durée non plus : zéro minute, et ce n'est pas une erreur.
    let mut muette = NewFichinter::new(fk_soc);
    muette.datei = "2026-04-11".into();
    let id_muette = db::fichinter::create(&pool, ENTITY, &muette)?;
    assert_eq!(
        db::fichinter::get(&pool, ENTITY, id_muette)?.unwrap().duree,
        None
    );
    assert_eq!(
        db::fichinter::duree_totale(&pool, ENTITY, id_muette)?,
        dec("0")
    );
    assert_eq!(
        db::fichinter::get(&pool, ENTITY, id_muette)?
            .unwrap()
            .description,
        None,
        "une description vide s'écrit NULL, pas une chaîne vide"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn refuse_un_tiers_inexistant() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "ZZ-FI-TiersInconnu";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let mut orpheline = NewFichinter::new(999_999_999);
    orpheline.datei = "2026-03-02".into();
    let err = db::fichinter::create(&pool, ENTITY, &orpheline)
        .expect_err("un tiers inexistant doit être refusé");
    assert!(
        err.to_string().contains("n'existe pas"),
        "message inattendu: {err}"
    );

    assert!(
        db::fichinter::create(&pool, ENTITY, &NewFichinter::new(0)).is_err(),
        "un tiers absent (fk_soc = 0) est refusé"
    );

    // Une intervention sans date ne se classe nulle part : elle est refusée.
    let mut sans_date = NewFichinter::new(fk_soc);
    sans_date.description = "Passage sans date".into();
    let err = db::fichinter::create(&pool, ENTITY, &sans_date)
        .expect_err("une date d'intervention vide doit être refusée");
    assert!(
        err.to_string().contains("date de l'intervention"),
        "message inattendu: {err}"
    );

    // Une durée négative n'a pas de sens.
    let mut negatif = NewFichinter::new(fk_soc);
    negatif.datei = "2026-03-02".into();
    negatif.duree = Some(dec("-5"));
    assert!(
        db::fichinter::create(&pool, ENTITY, &negatif).is_err(),
        "une durée d'en-tête négative est refusée"
    );
    let mut ligne_negative = NewFichinter::new(fk_soc);
    ligne_negative.datei = "2026-03-02".into();
    ligne_negative.lines = vec![NewFichinterLine::new("Tâche impossible", -10)];
    assert!(
        db::fichinter::create(&pool, ENTITY, &ligne_negative).is_err(),
        "une durée de ligne négative est refusée"
    );

    // Le même tiers, correctement renseigné, passe : les refus viennent bien de
    // la validation et pas d'un problème de données.
    let id = db::fichinter::create(&pool, ENTITY, &intervention(fk_soc))?;
    assert_eq!(
        db::fichinter::get(&pool, ENTITY, id)?.unwrap().fk_soc,
        fk_soc
    );
    assert!(
        db::fichinter::get(&pool, ENTITY, 999_999_999)?.is_none(),
        "aucune intervention orpheline n'a été écrite"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn valide_puis_refuse_de_revalider() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "ZZ-FI-Validation";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;
    let id = db::fichinter::create(&pool, ENTITY, &intervention(fk_soc))?;

    assert_eq!(
        db::fichinter::get(&pool, ENTITY, id)?.unwrap().date_valid,
        None
    );

    db::fichinter::valider(&pool, ENTITY, id)?;
    let f = db::fichinter::get(&pool, ENTITY, id)?.expect("intervention validée");
    assert_eq!(f.statut, STATUT_VALIDE);
    assert!(f.est_validee());
    assert!(f.date_valid.is_some(), "la validation pose sa date");
    let date_valid = f.date_valid.clone();

    let err = db::fichinter::valider(&pool, ENTITY, id)
        .expect_err("une intervention ne se valide qu'une fois");
    assert!(err.to_string().contains("déjà"), "message inattendu: {err}");
    let apres = db::fichinter::get(&pool, ENTITY, id)?.expect("intervention");
    assert_eq!(apres.statut, STATUT_VALIDE);
    assert_eq!(
        apres.date_valid, date_valid,
        "la revalidation refusée n'a pas écrasé la date d'origine"
    );

    assert!(
        db::fichinter::valider(&pool, ENTITY, id + 9_999_999).is_err(),
        "une intervention inconnue ne se valide pas"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn cloture_refusee_si_non_validee() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "ZZ-FI-Cloture";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;
    let id = db::fichinter::create(&pool, ENTITY, &intervention(fk_soc))?;

    let err = db::fichinter::cloturer(&pool, ENTITY, id, "2026-03-10")
        .expect_err("un brouillon ne se clôture pas");
    assert!(
        err.to_string().contains("validée"),
        "message inattendu: {err}"
    );
    assert_eq!(
        db::fichinter::get(&pool, ENTITY, id)?.unwrap().statut,
        STATUT_BROUILLON,
        "la clôture refusée n'a rien changé"
    );
    assert!(
        db::fichinter::cloturer(&pool, ENTITY, id, "   ").is_err(),
        "une date de fin vide est refusée"
    );

    db::fichinter::valider(&pool, ENTITY, id)?;
    db::fichinter::cloturer(&pool, ENTITY, id, "2026-03-10")?;
    let f = db::fichinter::get(&pool, ENTITY, id)?.expect("intervention clôturée");
    assert_eq!(f.statut, STATUT_CLOTURE);
    assert!(f.est_cloturee());
    assert_eq!(f.datee.as_deref(), Some("2026-03-10"));
    assert!(
        db::fichinter::valider(&pool, ENTITY, id).is_err(),
        "une intervention clôturée ne se revalide pas"
    );
    assert!(
        db::fichinter::cloturer(&pool, ENTITY, id, "2026-03-11").is_err(),
        "une intervention clôturée ne se reclôture pas"
    );
    assert_eq!(
        db::fichinter::get(&pool, ENTITY, id)?
            .unwrap()
            .datee
            .as_deref(),
        Some("2026-03-10"),
        "la date de fin d'origine est conservée"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn suppression_refusee_si_validee() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "ZZ-FI-Suppression";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let brouillon = db::fichinter::create(&pool, ENTITY, &intervention(fk_soc))?;
    let validee = db::fichinter::create(&pool, ENTITY, &intervention(fk_soc))?;
    db::fichinter::valider(&pool, ENTITY, validee)?;

    assert_eq!(db::fichinter::lines(&pool, ENTITY, brouillon)?.len(), 2);
    db::fichinter::supprimer(&pool, ENTITY, brouillon)?;
    assert!(
        db::fichinter::get(&pool, ENTITY, brouillon)?.is_none(),
        "le brouillon est supprimé"
    );
    assert!(
        db::fichinter::lines(&pool, ENTITY, brouillon)?.is_empty(),
        "ses lignes partent avec l'en-tête"
    );

    let err = db::fichinter::supprimer(&pool, ENTITY, validee)
        .expect_err("une intervention validée ne se supprime pas");
    assert!(
        err.to_string().contains("clôture"),
        "message inattendu: {err}"
    );
    assert!(
        db::fichinter::get(&pool, ENTITY, validee)?.is_some(),
        "la refusée est toujours là"
    );
    assert_eq!(db::fichinter::lines(&pool, ENTITY, validee)?.len(), 2);

    assert!(
        db::fichinter::supprimer(&pool, ENTITY, validee + 9_999_999).is_err(),
        "une intervention inconnue ne se supprime pas"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn duree_totale_additionne_entete_et_lignes() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "ZZ-FI-Duree";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    // En-tête et lignes se cumulent : 45,5 + 120 + 15 = 180,5 minutes.
    let mut new = intervention(fk_soc);
    new.lines = vec![
        NewFichinterLine::new("Taille de pierre", 120),
        NewFichinterLine::new("Nettoyage du chantier", 15),
    ];
    let id = db::fichinter::create(&pool, ENTITY, &new)?;
    assert_eq!(
        db::fichinter::duree_totale(&pool, ENTITY, id)?,
        dec("180.5")
    );

    // En-tête absent : la somme des lignes seule.
    let mut sans_entete = NewFichinter::new(fk_soc);
    sans_entete.datei = "2026-05-05".into();
    sans_entete.lines = vec![
        NewFichinterLine::new("Dépose", 30),
        NewFichinterLine::new("Repose", 30),
    ];
    let id_sans_entete = db::fichinter::create(&pool, ENTITY, &sans_entete)?;
    assert_eq!(
        db::fichinter::duree_totale(&pool, ENTITY, id_sans_entete)?,
        dec("60")
    );

    // En-tête seul : c'est le déplacement, sans ligne de détail.
    let mut entete_seul = NewFichinter::new(fk_soc);
    entete_seul.datei = "2026-05-06".into();
    entete_seul.duree = Some(dec("90"));
    let id_entete = db::fichinter::create(&pool, ENTITY, &entete_seul)?;
    assert_eq!(
        db::fichinter::duree_totale(&pool, ENTITY, id_entete)?,
        dec("90")
    );

    // Rien du tout : zéro minute, sans erreur.
    let mut rien = NewFichinter::new(fk_soc);
    rien.datei = "2026-05-07".into();
    let id_rien = db::fichinter::create(&pool, ENTITY, &rien)?;
    assert_eq!(
        db::fichinter::duree_totale(&pool, ENTITY, id_rien)?,
        dec("0")
    );

    assert!(
        db::fichinter::duree_totale(&pool, ENTITY, id + 9_999_999).is_err(),
        "une intervention inconnue n'a pas de durée"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn numerote_les_interventions_sans_doublon() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "ZZ-FI-Numerotation";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let mut ids = Vec::new();
    for passage in 0..5 {
        let mut new = NewFichinter::new(fk_soc);
        new.datei = "2026-06-01".into();
        new.description = format!("Passage {passage}");
        ids.push(db::fichinter::create(&pool, ENTITY, &new)?);
    }

    let mut refs = Vec::new();
    for id in &ids {
        refs.push(db::fichinter::get(&pool, ENTITY, *id)?.unwrap().ref_);
    }
    let uniques: BTreeSet<&String> = refs.iter().collect();
    assert_eq!(uniques.len(), 5, "références dupliquées: {refs:?}");
    for reference in &refs {
        assert!(
            reference.starts_with("FI"),
            "référence Dolibarr attendue, trouvée: {reference}"
        );
    }

    // La liste rend du plus récent au plus ancien : nos cinq interventions y
    // apparaissent donc dans l'ordre inverse de leur création.
    let liste = db::fichinter::list(&pool, ENTITY, 200, 0)?;
    let ordre: Vec<i64> = liste
        .iter()
        .map(|f| f.rowid)
        .filter(|id| ids.contains(id))
        .collect();
    let mut attendu = ids.clone();
    attendu.reverse();
    assert_eq!(ordre, attendu, "ordre de liste inattendu: {ordre:?}");

    // La pagination ne réordonne pas : la deuxième page continue la première.
    let premiere = db::fichinter::list(&pool, ENTITY, 1, 0)?;
    let suivante = db::fichinter::list(&pool, ENTITY, 1, 1)?;
    assert_ne!(
        premiere.first().map(|f| f.rowid),
        suivante.first().map(|f| f.rowid),
        "décaler d'un rang change bien la tête de liste"
    );

    nettoyer(&pool, nom)?;
    Ok(())
}
