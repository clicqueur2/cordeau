//! Tests d'intégration du module Mailings.
//!
//! Comme les autres fichiers de test du crate, ceux qui touchent la base ne
//! tournent que si `DOLIBARR_DSN` est posé : sans base, ils s'ignorent en le
//! disant.
//!
//! Tout est écrit en entité 1 et nettoyé derrière. Deux précautions :
//!
//! - le titre porte `std::process::id()` : `uk_mailing (titre, entity)` rend le
//!   titre unique par entité, et deux exécutions de la suite peuvent se croiser
//!   sur la même base ;
//! - aucune mesure globale n'est faite (pas de « nombre de campagnes avant /
//!   après ») : chaque test ne regarde que le mailing qu'il a créé, donc ils
//!   peuvent tourner en parallèle sans se gêner.

use std::error::Error;

use db::mailing::{
    self, NewMailing, CIBLE_DESINSCRITE, CIBLE_ENVOYEE, CIBLE_ERREUR, CIBLE_NON_ENVOYEE,
    MESSTYPE_EMAIL, STATUT_APPROUVEE, STATUT_BROUILLON, STATUT_ENVOYEE, STATUT_VALIDEE,
};
use mysql::params;
use mysql::prelude::Queryable;
use mysql::Pool;

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

/// Un titre unique par test et par processus.
fn titre_test(base: &str) -> String {
    format!("Campagne test {base} {}", std::process::id())
}

/// Une campagne plausible : le cas courant du module.
fn mailing_test(titre: &str) -> NewMailing {
    let sujet = format!("Objet de {titre}");
    NewMailing::new(
        titre,
        &sujet,
        "<p>Bonjour à tous,</p><p>Voici des nouvelles.</p>",
        "contact@exemple.test",
    )
}

/// Efface une campagne et ses cibles par son titre : sert aussi à enlever le
/// reliquat d'une exécution précédente interrompue.
fn effacer(pool: &Pool, titre: &str) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE mc FROM llx_mailing_cibles mc \
         JOIN llx_mailing m ON m.rowid = mc.fk_mailing \
         WHERE m.entity = :entity AND m.titre = :titre",
        params! { "entity" => ENTITY, "titre" => titre },
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_mailing WHERE entity = :entity AND titre = :titre",
        params! { "entity" => ENTITY, "titre" => titre },
    );
}

/// Le nombre de lignes portant ce titre : permet de prouver qu'un refus n'écrit
/// rien du tout, et pas seulement qu'il renvoie une erreur.
fn lignes(pool: &Pool, titre: &str) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let nombre: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_mailing WHERE entity = :entity AND titre = :titre",
        params! { "entity" => ENTITY, "titre" => titre },
    )?;
    Ok(nombre.unwrap_or(0))
}

/// Pose une cible à la main : le module n'expose volontairement aucune écriture
/// de destinataires (la sélection est le travail de Dolibarr).
fn ajouter_cible(
    pool: &Pool,
    fk_mailing: i64,
    email: &str,
    statut: i16,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // `fk_contact` est NOT NULL : 0 dit « pas issu d'une fiche contact », ce qui
    // est le cas d'une adresse saisie ou importée.
    conn.exec_drop(
        "INSERT INTO llx_mailing_cibles (fk_mailing, fk_contact, email, statut) \
         VALUES (:id, 0, :email, :statut)",
        params! { "id" => fk_mailing, "email" => email, "statut" => statut },
    )?;
    Ok(())
}

/// Inscrit une adresse à la liste de désinscription globale de l'entité.
fn desinscrire(pool: &Pool, email: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_mailing_unsubscribe (entity, email, unsubscribegroup) \
         VALUES (:entity, :email, '')",
        params! { "entity" => ENTITY, "email" => email },
    )?;
    Ok(())
}

fn raye_desinscription(pool: &Pool, email: &str) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE FROM llx_mailing_unsubscribe WHERE entity = :entity AND email = :email",
        params! { "entity" => ENTITY, "email" => email },
    );
}

#[test]
fn cree_et_relit_un_mailing() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = titre_test("creation");
    effacer(&pool, &titre);

    let new = mailing_test(&titre);
    let id = mailing::create(&pool, ENTITY, &new)?;
    assert!(id > 0);

    let lu = mailing::get(&pool, ENTITY, id)?.expect("mailing relu");
    assert_eq!(lu.titre, titre);
    assert_eq!(lu.sujet, new.sujet);
    assert_eq!(lu.entity, ENTITY);
    assert_eq!(lu.statut, STATUT_BROUILLON, "une création est un brouillon");
    assert_eq!(lu.nbemail, 0, "aucun destinataire au départ");
    assert_eq!(
        lu.messtype, MESSTYPE_EMAIL,
        "le type par défaut est l'e-mail"
    );
    assert_eq!(lu.email_from.as_deref(), Some("contact@exemple.test"));
    assert_eq!(
        lu.body.as_deref(),
        Some("<p>Bonjour à tous,</p><p>Voici des nouvelles.</p>"),
        "get ramène le corps"
    );
    assert!(lu.date_creation.is_some(), "date_creat posée");
    assert!(lu.date_valid.is_none(), "un brouillon n'est pas validé");
    assert!(lu.date_appro.is_none());
    assert!(lu.date_envoi.is_none());

    let listes = mailing::list(&pool, ENTITY, 1000, 0)?;
    let dans_liste = listes
        .iter()
        .find(|m| m.rowid == id)
        .expect("la campagne apparaît dans la liste");
    assert_eq!(dans_liste.titre, titre);
    assert_eq!(dans_liste.sujet, new.sujet);
    assert_eq!(dans_liste.statut, STATUT_BROUILLON);
    assert!(
        dans_liste.body.is_none(),
        "la liste ne charge pas le corps : mediumtext"
    );

    // Un identifiant absent ne remonte pas d'erreur, il ne remonte rien.
    assert!(mailing::get(&pool, ENTITY, 2_000_000_000)?.is_none());
    // Une campagne d'une autre entité n'existe pas pour celle-ci.
    assert!(mailing::get(&pool, ENTITY + 1, id)?.is_none());

    effacer(&pool, &titre);
    Ok(())
}

#[test]
fn refuse_un_titre_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = titre_test("titre_vide");
    effacer(&pool, &titre);

    let mut vide = mailing_test(&titre);
    vide.titre = "   ".into();
    let erreur = mailing::create(&pool, ENTITY, &vide).expect_err("titre vide refusé");
    assert!(erreur.to_string().contains("titre"), "{erreur}");
    assert_eq!(lignes(&pool, &titre)?, 0);
    Ok(())
}

#[test]
fn refuse_un_titre_trop_long() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = "x".repeat(129);
    effacer(&pool, &titre);

    let mut trop_long = mailing_test("titre_long");
    trop_long.titre = titre;
    let erreur = mailing::create(&pool, ENTITY, &trop_long).expect_err("titre trop long refusé");
    assert!(erreur.to_string().contains("128"), "{erreur}");
    Ok(())
}

#[test]
fn refuse_un_corps_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = titre_test("corps_vide");
    effacer(&pool, &titre);

    let mut vide = mailing_test(&titre);
    vide.body = "   \n  ".into();
    let erreur = mailing::create(&pool, ENTITY, &vide).expect_err("corps vide refusé");
    assert!(erreur.to_string().contains("corps"), "{erreur}");
    assert_eq!(lignes(&pool, &titre)?, 0, "rien ne s'écrit sur un refus");
    Ok(())
}

#[test]
fn refuse_une_adresse_d_expedition_sans_arobase() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = titre_test("sans_arobase");
    effacer(&pool, &titre);

    let mut sans_arobase = mailing_test(&titre);
    sans_arobase.email_from = "contact.exemple.test".into();
    let erreur = mailing::create(&pool, ENTITY, &sans_arobase).expect_err("adresse sans @ refusée");
    assert!(erreur.to_string().contains("arobase"), "{erreur}");
    assert_eq!(lignes(&pool, &titre)?, 0);

    // Une adresse absente est refusée aussi, mais pour son propre motif.
    let mut sans_adresse = mailing_test(&titre);
    sans_adresse.email_from = "  ".into();
    let erreur = mailing::create(&pool, ENTITY, &sans_adresse).expect_err("adresse vide refusée");
    assert!(erreur.to_string().contains("expédition"), "{erreur}");
    assert_eq!(lignes(&pool, &titre)?, 0);
    Ok(())
}

#[test]
fn refuse_un_titre_deja_pris() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = titre_test("titre_pris");
    effacer(&pool, &titre);

    let id = mailing::create(&pool, ENTITY, &mailing_test(&titre))?;
    assert!(id > 0);

    // Même titre, autre objet : seule la clé `uk_mailing (titre, entity)` entre
    // en cause.
    let mut doublon = mailing_test(&titre);
    doublon.sujet = "Un autre objet".into();
    let erreur = mailing::create(&pool, ENTITY, &doublon).expect_err("titre déjà pris");
    assert!(erreur.to_string().contains("existe déjà"), "{erreur}");

    effacer(&pool, &titre);
    Ok(())
}

#[test]
fn cycle_validation_approbation_envoi() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = titre_test("cycle");
    effacer(&pool, &titre);
    let id = mailing::create(&pool, ENTITY, &mailing_test(&titre))?;

    // Hors séquence, rien ne passe : on ne saute pas la relecture, et on
    // n'annonce pas un envoi qui n'a pas été autorisé.
    let erreur = mailing::approuver(&pool, ENTITY, id).expect_err("approbation d'un brouillon");
    assert!(erreur.to_string().contains("pas validé"), "{erreur}");
    let erreur = mailing::marquer_envoye(&pool, ENTITY, id).expect_err("envoi d'un brouillon");
    assert!(erreur.to_string().contains("pas approuvé"), "{erreur}");
    assert_eq!(
        mailing::get(&pool, ENTITY, id)?.unwrap().statut,
        STATUT_BROUILLON,
        "les refus n'ont rien changé"
    );

    // 0 → 1.
    mailing::valider(&pool, ENTITY, id)?;
    let valide = mailing::get(&pool, ENTITY, id)?.expect("mailing relu");
    assert_eq!(valide.statut, STATUT_VALIDEE);
    assert!(valide.date_valid.is_some(), "date_valid posée");
    assert!(valide.est_validee());

    let erreur = mailing::valider(&pool, ENTITY, id).expect_err("double validation");
    assert!(erreur.to_string().contains("déjà validé"), "{erreur}");

    // 1 → 2.
    mailing::approuver(&pool, ENTITY, id)?;
    let approuve = mailing::get(&pool, ENTITY, id)?.expect("mailing relu");
    assert_eq!(approuve.statut, STATUT_APPROUVEE);
    assert!(approuve.date_appro.is_some(), "date_appro posée");
    assert!(approuve.date_envoi.is_none(), "approuver n'envoie pas");
    assert!(approuve.est_approuvee());

    let erreur = mailing::approuver(&pool, ENTITY, id).expect_err("double approbation");
    assert!(erreur.to_string().contains("déjà approuvé"), "{erreur}");

    // 2 → 3.
    mailing::marquer_envoye(&pool, ENTITY, id)?;
    let envoye = mailing::get(&pool, ENTITY, id)?.expect("mailing relu");
    assert_eq!(envoye.statut, STATUT_ENVOYEE);
    assert!(envoye.date_envoi.is_some(), "date_envoi posée");
    assert!(envoye.est_envoyee());

    let erreur = mailing::marquer_envoye(&pool, ENTITY, id).expect_err("double envoi");
    assert!(erreur.to_string().contains("déjà marqué"), "{erreur}");
    let erreur = mailing::valider(&pool, ENTITY, id).expect_err("validation après envoi");
    assert!(erreur.to_string().contains("déjà envoyé"), "{erreur}");

    effacer(&pool, &titre);
    Ok(())
}

#[test]
fn refuse_les_transitions_sur_une_campagne_inconnue() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let inconnu = 2_000_000_000;
    for resultat in [
        mailing::valider(&pool, ENTITY, inconnu),
        mailing::approuver(&pool, ENTITY, inconnu),
        mailing::marquer_envoye(&pool, ENTITY, inconnu),
        mailing::supprimer(&pool, ENTITY, inconnu),
    ] {
        let erreur = resultat.expect_err("identifiant inconnu refusé");
        assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");
    }
    Ok(())
}

#[test]
fn refuse_de_supprimer_une_campagne_validee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = titre_test("suppression_refusee");
    effacer(&pool, &titre);
    let id = mailing::create(&pool, ENTITY, &mailing_test(&titre))?;
    mailing::valider(&pool, ENTITY, id)?;

    let erreur = mailing::supprimer(&pool, ENTITY, id).expect_err("suppression refusée");
    assert!(erreur.to_string().contains("supprime"), "{erreur}");
    assert_eq!(
        mailing::get(&pool, ENTITY, id)?.map(|m| m.statut),
        Some(STATUT_VALIDEE),
        "le refus laisse la campagne intacte"
    );

    effacer(&pool, &titre);
    Ok(())
}

#[test]
fn supprime_un_brouillon_et_ses_destinataires() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = titre_test("suppression");
    effacer(&pool, &titre);
    let id = mailing::create(&pool, ENTITY, &mailing_test(&titre))?;
    ajouter_cible(
        &pool,
        id,
        &format!("cible.{}@exemple.test", std::process::id()),
        CIBLE_NON_ENVOYEE,
    )?;
    assert_eq!(mailing::compter_destinataires(&pool, ENTITY, id)?, 1);

    mailing::supprimer(&pool, ENTITY, id)?;
    assert!(
        mailing::get(&pool, ENTITY, id)?.is_none(),
        "l'en-tête est parti"
    );
    assert!(
        mailing::destinataires(&pool, ENTITY, id)?.is_empty(),
        "les cibles sont parties avec l'en-tête"
    );

    // Reliquat éventuel : la suppression aurait dû tout enlever.
    effacer(&pool, &titre);
    Ok(())
}

#[test]
fn destinataires_deduplique_et_ecarte_les_vides() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = titre_test("dedup");
    effacer(&pool, &titre);
    let id = mailing::create(&pool, ENTITY, &mailing_test(&titre))?;

    let pid = std::process::id();
    let premiere = format!("a.{pid}@exemple.test");
    let seconde = format!("b.{pid}@exemple.test");

    // Quatre lignes pour un résultat de deux, et non trois : avec (a, doublon,
    // vacante) il ne resterait qu'une adresse, la vacante ne comptant pas. On
    // ajoute donc une seconde adresse réelle pour que le compte à vérifier soit
    // bien deux.
    ajouter_cible(&pool, id, &premiere, CIBLE_NON_ENVOYEE)?;
    // Espace de tête : la clé `uk_mailing_cibles (fk_mailing, email)` ne la voit
    // pas (MariaDB n'ignore que les espaces **de fin**), et c'est exactement le
    // doublon qu'un copier-coller produit. Après `TRIM`, c'est la même boîte.
    ajouter_cible(&pool, id, &format!(" {premiere}"), CIBLE_NON_ENVOYEE)?;
    ajouter_cible(&pool, id, &seconde, CIBLE_ENVOYEE)?;
    // Adresse vacante : un import approximatif en laisse toujours passer une.
    ajouter_cible(&pool, id, "", CIBLE_NON_ENVOYEE)?;

    let adresses = mailing::destinataires(&pool, ENTITY, id)?;
    assert_eq!(
        adresses,
        vec![premiere, seconde],
        "dédupliquées et nettoyées"
    );
    assert_eq!(
        mailing::compter_destinataires(&pool, ENTITY, id)?,
        2,
        "le compteur dit la même chose que la liste"
    );

    // Une autre entité ne voit pas ces adresses.
    assert!(mailing::destinataires(&pool, ENTITY + 1, id)?.is_empty());

    effacer(&pool, &titre);
    Ok(())
}

#[test]
fn destinataires_ecarte_les_cibles_en_erreur_ou_desinscrites() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let titre = titre_test("exclusions");
    effacer(&pool, &titre);
    let id = mailing::create(&pool, ENTITY, &mailing_test(&titre))?;

    let pid = std::process::id();
    let joignable = format!("ok.{pid}@exemple.test");
    let en_erreur = format!("erreur.{pid}@exemple.test");
    let desinscrit = format!("parti.{pid}@exemple.test");
    let global = format!("global.{pid}@exemple.test");
    ajouter_cible(&pool, id, &joignable, CIBLE_NON_ENVOYEE)?;
    // `-1` : l'envoi précédent a échoué sur cette adresse.
    ajouter_cible(&pool, id, &en_erreur, CIBLE_ERREUR)?;
    // `3` : désinscription demandée depuis le message reçu.
    ajouter_cible(&pool, id, &desinscrit, CIBLE_DESINSCRITE)?;
    // Désinscription globale, tous mailings confondus.
    ajouter_cible(&pool, id, &global, CIBLE_ENVOYEE)?;
    desinscrire(&pool, &global)?;

    assert_eq!(
        mailing::destinataires(&pool, ENTITY, id)?,
        vec![joignable.clone()],
        "ni l'erreur, ni la désinscription, ni le refus global ne repartent"
    );
    assert_eq!(mailing::compter_destinataires(&pool, ENTITY, id)?, 1);

    // `evenunsubscribe = 1` est le drapeau explicite de Dolibarr : la campagne
    // assume d'écrire à une adresse désinscrite, et le filtre la laisse passer.
    // Le module ne pose jamais ce drapeau (une création reste à 0) ; on le force
    // ici pour prouver que la lecture le respecte.
    {
        let mut conn = pool.get_conn()?;
        conn.exec_drop(
            "UPDATE llx_mailing SET evenunsubscribe = 1 WHERE entity = :entity AND rowid = :id",
            params! { "entity" => ENTITY, "id" => id },
        )?;
    }
    let avec_drapeau = mailing::destinataires(&pool, ENTITY, id)?;
    assert_eq!(avec_drapeau, vec![joignable, global.clone()]);

    raye_desinscription(&pool, &global);
    effacer(&pool, &titre);
    Ok(())
}
