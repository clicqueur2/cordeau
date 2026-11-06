//! Tests d'intégration du module Organisation d'événements (inscriptions).
//!
//! ## Ce qu'il faut pour que ces tests tournent
//!
//! `crates/db/src/lib.rs` doit porter **`pub mod evenement;`** (ordre
//! alphabétique : entre `ecritures_tva` et `events`). Cette ligne n'est pas
//! ajoutée ici : ce fichier de test ne modifie aucun fichier existant, et la
//! déclaration appartient au noyau. Les tests passent ensuite par
//! `db::evenement`, la vraie déclaration, et non par une inclusion de fichier —
//! une erreur de déclaration se voit donc tout de suite.
//!
//! Les tests qui touchent la base ne tournent que si `DOLIBARR_DSN` est posé :
//! sans base, ils s'ignorent **en le disant**. Les tests de séquence et de
//! validation, eux, sont purs et tournent toujours — ils ne demandent ni MariaDB
//! ni horloge.
//!
//! ## Le terrain, mesuré avant d'écrire
//!
//! `llx_eventorganization_conferenceorboothattendee` est **vide** sur l'instance
//! de référence (`SELECT COUNT(*)` → 0) : aucun historique réel n'est en jeu.
//! Il y a en revanche 28 projets et 9 événements d'agenda, qui ne nous
//! appartiennent pas : chaque test crée donc **son** projet et **son** événement,
//! et les efface.
//!
//! Tout est écrit en entité 1, sous une marque `zz<pid>-<n>` propre à chaque
//! test. Le nettoyage porte sur **cette marque**, jamais sur un préfixe global :
//! cargo lance les tests en parallèle, et un `DELETE ... LIKE 'zz%'` effacerait
//! les lignes du test voisin en pleine vérification.
//!
//! La marque n'utilise ni `%` ni `_` : le nettoyage s'en sert comme motif `LIKE`,
//! et un `_` y serait un joker. Elle reste courte parce que `llx_actioncomm.ref`
//! est un `varchar(30)`.
//!
//! ## Pourquoi un projet et un événement par test
//!
//! Parce que `fk_project` est **NOT NULL** sur l'inscription et que le
//! cloisonnement vient de `llx_projet.entity` : sans projet, pas d'inscription
//! possible. L'événement, lui, est un `llx_actioncomm` (voir l'en-tête du
//! module) : c'est le seul moyen d'avoir une date et un lieu.

use std::sync::atomic::{AtomicU32, Ordering};

use mysql::prelude::Queryable;
use mysql::{params, Pool};

use db::evenement::{
    annuler, confirmer, date_valide, get, inscrire, inscrits, libelle_statut, list,
    marquer_present, presents, transition_permise, valider_inscription, NouvelleInscription,
    INSCRIPTION_ANNULEE, INSCRIPTION_BROUILLON, INSCRIPTION_CONFIRMEE, INSCRIPTION_PRESENTE,
};

const ENTITY: i64 = 1;

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(pool) => Some(pool),
        Err(erreur) => {
            eprintln!("connexion impossible ({erreur}) : test ignoré");
            None
        }
    }
}

/// Compteur de marques : deux tests ne peuvent pas tomber sur le même suffixe.
static COMPTEUR: AtomicU32 = AtomicU32::new(0);

/// Une marque unique par test, courte ( `llx_actioncomm.ref` = `varchar(30)`).
fn marque() -> String {
    let numero = COMPTEUR.fetch_add(1, Ordering::Relaxed);
    format!("zz{}-{numero}", std::process::id())
}

/// Un email unique par test, dans le domaine d'exemple.
fn email(marque: &str, sujet: &str) -> String {
    format!("{marque}-{sujet}@exemple.fr")
}

/// Le premier utilisateur de la base : `llx_projet.fk_user_creat` est NOT NULL et
/// n'admet aucune valeur par défaut.
fn utilisateur(pool: &Pool) -> Result<i64, Box<dyn std::error::Error>> {
    let mut conn = pool.get_conn()?;
    let id: Option<i64> = conn.query_first("SELECT rowid FROM llx_user ORDER BY rowid LIMIT 1")?;
    match id {
        Some(id) => Ok(id),
        None => Err("aucun utilisateur en base : impossible de créer un projet".into()),
    }
}

/// Supprime ce que **ce test** a écrit, et rien d'autre.
///
/// L'ordre compte : l'inscription pointe l'événement, l'événement pointe le
/// projet. On efface donc du plus dépendant au moins dépendant.
///
/// Les inscriptions sont retrouvées par leur **email** (leur `ref` vaut le
/// `rowid`, donc elle ne porte pas la marque) ou par leur événement.
fn nettoyer(pool: &Pool, marque: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = pool.get_conn()?;
    let motif = format!("{marque}%");
    // Deux noms pour la même valeur : le pilote refuse qu'un paramètre nommé soit
    // utilisé deux fois dans une même requête.
    conn.exec_drop(
        "DELETE FROM llx_eventorganization_conferenceorboothattendee \
         WHERE email LIKE :motif_email \
            OR fk_actioncomm IN (SELECT id FROM llx_actioncomm WHERE ref LIKE :motif_evenement)",
        params! { "motif_email" => motif.as_str(), "motif_evenement" => motif.as_str() },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_actioncomm WHERE ref LIKE :motif",
        params! { "motif" => motif.as_str() },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_projet WHERE ref LIKE :motif",
        params! { "motif" => motif.as_str() },
    )?;
    Ok(())
}

/// Crée le projet et l'événement d'un test, et rend `(projet, événement)`.
///
/// `usage_organize_event = 1` parce que c'est ce que Dolibarr exige d'un projet
/// qui porte un événement (le sélecteur de la source filtre là-dessus).
fn evenement(
    pool: &Pool,
    marque: &str,
    suffixe: &str,
) -> Result<(i64, i64), Box<dyn std::error::Error>> {
    let mut conn = pool.get_conn()?;
    let auteur = utilisateur(pool)?;
    conn.exec_drop(
        "INSERT INTO llx_projet (ref, title, entity, fk_user_creat, usage_organize_event) \
         VALUES (:ref, :titre, :entity, :auteur, 1)",
        params! {
            "ref" => format!("{marque}-P{suffixe}"),
            "titre" => format!("Projet de test {marque}{suffixe}"),
            "entity" => ENTITY,
            "auteur" => auteur,
        },
    )?;
    let projet = conn.last_insert_id() as i64;
    let action = ajouter_evenement(pool, marque, suffixe, Some(projet))?;
    Ok((projet, action))
}

/// Ajoute un événement d'agenda sur un projet existant.
///
/// `fk_project` est facultatif côté `llx_actioncomm` : c'est ce qui permet de
/// tester le refus d'un événement sans projet (voir plus bas).
fn ajouter_evenement(
    pool: &Pool,
    marque: &str,
    suffixe: &str,
    projet: Option<i64>,
) -> Result<i64, Box<dyn std::error::Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_actioncomm (ref, label, entity, datep, fk_project, status) \
         VALUES (:ref, :label, :entity, '2026-11-06 09:00:00', :projet, 2)",
        params! {
            "ref" => format!("{marque}-E{suffixe}"),
            "label" => format!("Conférence de test {marque}{suffixe}"),
            "entity" => ENTITY,
            "projet" => projet,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Une inscription minimale valide, à ajuster par chaque test.
fn nouvelle(fk_actioncomm: i64, email: &str) -> NouvelleInscription {
    NouvelleInscription {
        fk_soc: None,
        fk_actioncomm,
        email: email.to_string(),
        firstname: Some("Camille".to_string()),
        lastname: Some("Test".to_string()),
        company: None,
        date_subscription: None,
        fk_country: None,
        note: None,
    }
}

/// Le nombre de lignes qui portent la marque : sert à prouver qu'un refus n'a
/// rien écrit.
fn lignes(pool: &Pool, marque: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let mut conn = pool.get_conn()?;
    let total: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_eventorganization_conferenceorboothattendee \
         WHERE email LIKE :motif",
        params! { "motif" => format!("{marque}%") },
    )?;
    Ok(total.unwrap_or(0))
}

// ---------------------------------------------------------------------------
// Tests purs : ni base, ni horloge
// ---------------------------------------------------------------------------

/// Les dates acceptées et refusées. `date_subscription` est un `datetime` : on
/// accepte donc la date seule **et** l'horodatage complet, mais pas une date qui
/// n'existe pas.
#[test]
fn dates_valides_et_invalides() {
    assert!(date_valide("2026-11-06"));
    assert!(date_valide("2026-11-06 09:30:00"));
    assert!(date_valide(" 2024-02-29 "), "le 29 février 2024 existe");
    assert!(
        date_valide("2000-02-29"),
        "2000 est bissextile (divisible par 400)"
    );

    assert!(
        !date_valide("1900-02-29"),
        "1900 n'est pas bissextile (siècle)"
    );
    assert!(!date_valide("2026-02-30"), "février n'a jamais 30 jours");
    assert!(!date_valide("2026-13-01"), "il n'y a pas de mois 13");
    assert!(!date_valide("2026-00-10"), "il n'y a pas de mois 0");
    assert!(!date_valide("2026-04-31"), "avril n'a que 30 jours");
    assert!(!date_valide("2026-11-06 24:00:00"), "24 h n'existe pas");
    assert!(
        !date_valide("2026-11-06 09:60:00"),
        "60 minutes n'existent pas"
    );
    assert!(
        !date_valide("06/11/2026"),
        "le format français n'est pas celui de la base"
    );
    assert!(
        !date_valide("2026-11"),
        "une date tronquée n'est pas une date"
    );
    assert!(!date_valide(""), "le vide n'est pas une date");
    assert!(
        !date_valide("2026-1-6"),
        "les champs doivent être sur deux chiffres"
    );
}

/// La séquence des statuts, calquée sur les gardes de la source.
#[test]
fn transitions_de_statut() {
    assert!(transition_permise(
        INSCRIPTION_BROUILLON,
        INSCRIPTION_CONFIRMEE
    ));
    assert!(transition_permise(
        INSCRIPTION_CONFIRMEE,
        INSCRIPTION_ANNULEE
    ));
    assert!(transition_permise(
        INSCRIPTION_CONFIRMEE,
        INSCRIPTION_PRESENTE
    ));

    // `validate()` refuse de revalider.
    assert!(!transition_permise(
        INSCRIPTION_CONFIRMEE,
        INSCRIPTION_CONFIRMEE
    ));
    // `cancel()` exige le statut 1 : un brouillon ne s'annule pas, un présent non plus.
    assert!(!transition_permise(
        INSCRIPTION_BROUILLON,
        INSCRIPTION_ANNULEE
    ));
    assert!(!transition_permise(
        INSCRIPTION_PRESENTE,
        INSCRIPTION_ANNULEE
    ));
    // On ne marque pas présent quelqu'un qui n'était pas inscrit.
    assert!(!transition_permise(
        INSCRIPTION_BROUILLON,
        INSCRIPTION_PRESENTE
    ));
    assert!(!transition_permise(
        INSCRIPTION_ANNULEE,
        INSCRIPTION_PRESENTE
    ));
    // `reopen()` (9 → 1) existe en PHP mais n'est pas exposé ici : refusé.
    assert!(!transition_permise(
        INSCRIPTION_ANNULEE,
        INSCRIPTION_CONFIRMEE
    ));
    // Aucun état ne bouge tout seul.
    assert!(!transition_permise(
        INSCRIPTION_ANNULEE,
        INSCRIPTION_ANNULEE
    ));

    assert_eq!(libelle_statut(INSCRIPTION_BROUILLON), "brouillon");
    assert_eq!(libelle_statut(INSCRIPTION_CONFIRMEE), "confirmée");
    assert_eq!(libelle_statut(INSCRIPTION_PRESENTE), "présente");
    assert_eq!(libelle_statut(INSCRIPTION_ANNULEE), "annulée");
    assert_eq!(libelle_statut(3), "statut inconnu");
}

/// Les refus qui ne demandent pas la base : email, identité, pays.
#[test]
fn validations_pures_de_l_inscription() {
    let valide = nouvelle(42, "camille@exemple.fr");
    assert!(valider_inscription(&valide).is_ok());

    // Sans arobase : refusé, et le message dit pourquoi.
    let mut sans_arobase = valide.clone();
    sans_arobase.email = "pas-un-email".to_string();
    let erreur = valider_inscription(&sans_arobase).unwrap_err().to_string();
    assert!(erreur.contains("arobase"), "{erreur}");

    let mut deux_arobases = valide.clone();
    deux_arobases.email = "a@@b.fr".to_string();
    assert!(valider_inscription(&deux_arobases).is_err());

    let mut arobase_seule = valide.clone();
    arobase_seule.email = "@exemple.fr".to_string();
    assert!(valider_inscription(&arobase_seule).is_err());

    // Ni nom ni société : on ne sait pas qui vient.
    let mut anonyme = valide.clone();
    anonyme.firstname = None;
    anonyme.lastname = None;
    let erreur = valider_inscription(&anonyme).unwrap_err().to_string();
    assert!(erreur.contains("sans nom"), "{erreur}");

    // Un prénom seul suffit : c'est un nom, au sens de la règle.
    let mut prenom_seul = anonyme.clone();
    prenom_seul.firstname = Some("Camille".to_string());
    assert!(valider_inscription(&prenom_seul).is_ok());

    // Une société seule suffit aussi.
    let mut societe_seule = anonyme.clone();
    societe_seule.fk_soc = Some(1);
    assert!(valider_inscription(&societe_seule).is_ok());

    // `fk_country` n'a pas de colonne : refusé plutôt qu'ignoré en silence.
    let mut avec_pays = valide.clone();
    avec_pays.fk_country = Some(76);
    let erreur = valider_inscription(&avec_pays).unwrap_err().to_string();
    assert!(erreur.contains("fk_country"), "{erreur}");

    // Événement absent : une inscription sans date ni lieu.
    let mut sans_evenement = valide.clone();
    sans_evenement.fk_actioncomm = 0;
    assert!(valider_inscription(&sans_evenement).is_err());

    // Une date qui n'existe pas est refusée avant d'atteindre MariaDB.
    let mut mauvaise_date = valide.clone();
    mauvaise_date.date_subscription = Some("2026-02-30".to_string());
    let erreur = valider_inscription(&mauvaise_date).unwrap_err().to_string();
    assert!(erreur.contains("date"), "{erreur}");

    // Un nom plus long que `varchar(100)` est refusé avec le compte exact.
    let mut trop_long = valide;
    trop_long.lastname = Some("x".repeat(101));
    let erreur = valider_inscription(&trop_long).unwrap_err().to_string();
    assert!(erreur.contains("101"), "{erreur}");
}

// ---------------------------------------------------------------------------
// Tests sur base
// ---------------------------------------------------------------------------

/// Inscrire, puis relire : le `rowid` rendu, la référence, et le projet **déduit
/// de l'événement**.
#[test]
fn inscription_puis_relecture() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    let marque = marque();
    nettoyer(&pool, &marque).expect("nettoyage initial");

    let (projet, evenement) = evenement(&pool, &marque, "A").expect("fixture");
    let adresse = email(&marque, "camille");
    let mut new = nouvelle(evenement, &adresse);
    new.note = Some("Vient avec un collègue".to_string());

    let rowid = inscrire(&pool, ENTITY, &new).expect("inscription acceptée");
    assert!(rowid > 0);

    let relue = get(&pool, ENTITY, rowid)
        .expect("lecture")
        .expect("l'inscription doit se relire");
    assert_eq!(relue.rowid, rowid);
    assert_eq!(relue.email.as_deref(), Some(adresse.as_str()));
    assert_eq!(relue.firstname.as_deref(), Some("Camille"));
    assert_eq!(relue.lastname.as_deref(), Some("Test"));
    assert_eq!(relue.fk_evenement, Some(evenement));
    assert_eq!(
        relue.fk_project, projet,
        "le projet vient de l'événement, pas de l'appelant"
    );
    assert_eq!(
        relue.statut, INSCRIPTION_BROUILLON,
        "create() écrit un brouillon"
    );
    let libelle_attendu = format!("Conférence de test {marque}A");
    assert_eq!(
        relue.evenement_label.as_deref(),
        Some(libelle_attendu.as_str())
    );
    assert_eq!(relue.note_public.as_deref(), Some("Vient avec un collègue"));
    assert!(
        relue.date_subscription.is_some(),
        "NOW() si l'appelant n'en donne pas"
    );
    assert_eq!(relue.montant_paye, None, "aucun montant n'est écrit ici");

    // La référence est le rowid, comme `create()` + `update()` dans la source.
    let mut conn = pool.get_conn().expect("connexion");
    let reference: Option<String> = conn
        .exec_first(
            "SELECT ref FROM llx_eventorganization_conferenceorboothattendee WHERE rowid = :rowid",
            params! { "rowid" => rowid },
        )
        .expect("lecture de la référence");
    assert_eq!(reference.as_deref(), Some(rowid.to_string().as_str()));

    let liste = list(&pool, ENTITY, 100).expect("liste");
    assert!(liste.iter().any(|i| i.rowid == rowid));

    nettoyer(&pool, &marque).expect("nettoyage final");
}

/// Un email sans arobase est refusé **et rien n'est écrit**.
#[test]
fn email_sans_arobase_refuse_sans_ecrire() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    let marque = marque();
    nettoyer(&pool, &marque).expect("nettoyage initial");
    let (_projet, evenement) = evenement(&pool, &marque, "A").expect("fixture");

    let mut new = nouvelle(evenement, "pas-un-email");
    // La marque est portée par l'email, même invalide : c'est notre repère de
    // nettoyage.
    new.email = format!("{marque}-pas-un-email");
    let erreur = inscrire(&pool, ENTITY, &new)
        .expect_err("un email sans arobase doit être refusé")
        .to_string();
    assert!(erreur.contains("arobase"), "{erreur}");
    assert_eq!(
        lignes(&pool, &marque).expect("compte"),
        0,
        "aucune ligne écrite"
    );

    nettoyer(&pool, &marque).expect("nettoyage final");
}

/// Sans nom **et** sans société, l'inscription est refusée ; avec l'un des deux,
/// elle passe.
#[test]
fn nom_et_societe_vides_refuses() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    let marque = marque();
    nettoyer(&pool, &marque).expect("nettoyage initial");
    let (_projet, evenement) = evenement(&pool, &marque, "A").expect("fixture");

    let mut anonyme = nouvelle(evenement, &email(&marque, "anonyme"));
    anonyme.firstname = None;
    anonyme.lastname = None;
    let erreur = inscrire(&pool, ENTITY, &anonyme)
        .expect_err("ni nom ni société : refusé")
        .to_string();
    assert!(erreur.contains("sans nom"), "{erreur}");
    assert_eq!(lignes(&pool, &marque).expect("compte"), 0);

    // Avec une société, et toujours pas de nom : accepté.
    anonyme.fk_soc = Some(societe_de_test(&pool));
    let rowid = inscrire(&pool, ENTITY, &anonyme).expect("une société suffit");
    assert!(rowid > 0);
    assert_eq!(lignes(&pool, &marque).expect("compte"), 1);

    nettoyer(&pool, &marque).expect("nettoyage final");
}

/// Le doublon : même email, même événement. La casse ne sauve pas, parce que la
/// collation de `email` est `utf8mb4_unicode_ci` et que l'index unique l'est
/// aussi.
#[test]
fn doublon_meme_email_meme_evenement_refuse() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    let marque = marque();
    nettoyer(&pool, &marque).expect("nettoyage initial");
    let (_projet, evenement) = evenement(&pool, &marque, "A").expect("fixture");

    let adresse = email(&marque, "camille");
    let premiere = nouvelle(evenement, &adresse);
    let rowid = inscrire(&pool, ENTITY, &premiere).expect("première inscription");

    let erreur = inscrire(&pool, ENTITY, &premiere)
        .expect_err("le doublon doit être refusé")
        .to_string();
    assert!(erreur.contains("déjà inscrit"), "{erreur}");
    assert!(
        erreur.contains(&rowid.to_string()),
        "le message donne l'inscription existante : {erreur}"
    );

    // Même adresse en majuscules : c'est la même personne pour la base.
    let mut majuscules = nouvelle(evenement, &adresse.to_uppercase());
    majuscules.lastname = Some("Autre".to_string());
    let erreur = inscrire(&pool, ENTITY, &majuscules)
        .expect_err("la casse ne crée pas une seconde inscription")
        .to_string();
    assert!(erreur.contains("déjà inscrit"), "{erreur}");

    assert_eq!(
        lignes(&pool, &marque).expect("compte"),
        1,
        "une seule ligne"
    );

    nettoyer(&pool, &marque).expect("nettoyage final");
}

/// Le même email sur **deux événements différents** est accepté : c'est la règle
/// du doublon, lue à l'envers.
#[test]
fn meme_email_sur_deux_evenements_accepte() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    let marque = marque();
    nettoyer(&pool, &marque).expect("nettoyage initial");
    let (projet, premier) = evenement(&pool, &marque, "A").expect("fixture");
    let second = ajouter_evenement(&pool, &marque, "B", Some(projet)).expect("second événement");

    let adresse = email(&marque, "camille");
    let a = inscrire(&pool, ENTITY, &nouvelle(premier, &adresse)).expect("premier événement");
    let b = inscrire(&pool, ENTITY, &nouvelle(second, &adresse)).expect("second événement");
    assert_ne!(a, b);

    assert_eq!(inscrits(&pool, ENTITY, premier).expect("compte"), 1);
    assert_eq!(inscrits(&pool, ENTITY, second).expect("compte"), 1);
    assert_eq!(lignes(&pool, &marque).expect("compte"), 2);

    nettoyer(&pool, &marque).expect("nettoyage final");
}

/// Confirmer, puis refuser de reconfirmer (`validate()` ne revalide pas).
#[test]
fn confirmation_puis_reconfirmation_refusee() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    let marque = marque();
    nettoyer(&pool, &marque).expect("nettoyage initial");
    let (_projet, evenement) = evenement(&pool, &marque, "A").expect("fixture");
    let rowid = inscrire(
        &pool,
        ENTITY,
        &nouvelle(evenement, &email(&marque, "camille")),
    )
    .expect("inscription");

    confirmer(&pool, ENTITY, rowid).expect("confirmation");
    let relue = get(&pool, ENTITY, rowid)
        .expect("lecture")
        .expect("présente");
    assert_eq!(relue.statut, INSCRIPTION_CONFIRMEE);

    let erreur = confirmer(&pool, ENTITY, rowid)
        .expect_err("reconfirmer doit être refusé")
        .to_string();
    assert!(erreur.contains("transition refusée"), "{erreur}");
    assert!(erreur.contains("confirmée"), "{erreur}");

    // L'inscription n'a pas bougé.
    let relue = get(&pool, ENTITY, rowid)
        .expect("lecture")
        .expect("présente");
    assert_eq!(relue.statut, INSCRIPTION_CONFIRMEE);

    nettoyer(&pool, &marque).expect("nettoyage final");
}

/// L'annulation suit la garde de `cancel()` : seulement depuis « confirmée ».
#[test]
fn annulation_et_garde_de_la_source() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    let marque = marque();
    nettoyer(&pool, &marque).expect("nettoyage initial");
    let (_projet, evenement) = evenement(&pool, &marque, "A").expect("fixture");
    let rowid = inscrire(
        &pool,
        ENTITY,
        &nouvelle(evenement, &email(&marque, "camille")),
    )
    .expect("inscription");

    // Un brouillon ne s'annule pas : il n'a jamais été confirmé.
    let erreur = annuler(&pool, ENTITY, rowid)
        .expect_err("un brouillon ne s'annule pas")
        .to_string();
    assert!(erreur.contains("transition refusée"), "{erreur}");

    confirmer(&pool, ENTITY, rowid).expect("confirmation");
    annuler(&pool, ENTITY, rowid).expect("annulation depuis confirmée");
    let relue = get(&pool, ENTITY, rowid)
        .expect("lecture")
        .expect("présente");
    assert_eq!(relue.statut, INSCRIPTION_ANNULEE);

    // Annuler deux fois : hors séquence, et l'état terminal ne se quitte pas.
    let erreur = annuler(&pool, ENTITY, rowid)
        .expect_err("une annulée ne se réannule pas")
        .to_string();
    assert!(erreur.contains("transition refusée"), "{erreur}");

    nettoyer(&pool, &marque).expect("nettoyage final");
}

/// `inscrits` et `presents` : deux comptes différents, et les annulées ne
/// comptent plus comme inscrites.
#[test]
fn inscrits_et_presents() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    let marque = marque();
    nettoyer(&pool, &marque).expect("nettoyage initial");
    let (projet, evenement) = evenement(&pool, &marque, "A").expect("fixture");
    let autre = ajouter_evenement(&pool, &marque, "B", Some(projet)).expect("autre événement");

    let premiere = inscrire(&pool, ENTITY, &nouvelle(evenement, &email(&marque, "un")))
        .expect("inscription 1");
    let seconde = inscrire(&pool, ENTITY, &nouvelle(evenement, &email(&marque, "deux")))
        .expect("inscription 2");

    // Deux inscrites, aucune présence, et rien sur l'autre événement.
    assert_eq!(inscrits(&pool, ENTITY, evenement).expect("compte"), 2);
    assert_eq!(presents(&pool, ENTITY, evenement).expect("compte"), 0);
    assert_eq!(
        inscrits(&pool, ENTITY, autre).expect("compte"),
        0,
        "les comptes sont par événement"
    );

    confirmer(&pool, ENTITY, premiere).expect("confirmation 1");
    confirmer(&pool, ENTITY, seconde).expect("confirmation 2");
    marquer_present(&pool, ENTITY, premiere).expect("présence 1");
    assert_eq!(inscrits(&pool, ENTITY, evenement).expect("compte"), 2);
    assert_eq!(
        presents(&pool, ENTITY, evenement).expect("compte"),
        1,
        "une seule présence constatée"
    );

    // Annuler la seconde : elle ne compte plus comme inscrite, mais la présence
    // constatée reste.
    annuler(&pool, ENTITY, seconde).expect("annulation");
    assert_eq!(inscrits(&pool, ENTITY, evenement).expect("compte"), 1);
    assert_eq!(presents(&pool, ENTITY, evenement).expect("compte"), 1);

    // On ne marque pas présent deux fois.
    let erreur = marquer_present(&pool, ENTITY, premiere)
        .expect_err("déjà présente")
        .to_string();
    assert!(erreur.contains("transition refusée"), "{erreur}");

    // Un événement inconnu rend 0, pas une erreur : c'est un compte.
    assert_eq!(inscrits(&pool, ENTITY, 999_999_999).expect("compte"), 0);

    nettoyer(&pool, &marque).expect("nettoyage final");
}

/// Un événement sans projet est refusé : `fk_project` est NOT NULL et porte le
/// cloisonnement. Écrire 0 la rendrait invisible à toutes les lectures.
#[test]
fn evenement_sans_projet_refuse() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    let marque = marque();
    nettoyer(&pool, &marque).expect("nettoyage initial");
    let orphelin = ajouter_evenement(&pool, &marque, "S", None).expect("événement sans projet");

    let erreur = inscrire(
        &pool,
        ENTITY,
        &nouvelle(orphelin, &email(&marque, "camille")),
    )
    .expect_err("un événement sans projet ne reçoit pas d'inscription")
    .to_string();
    assert!(erreur.contains("aucun projet"), "{erreur}");
    assert_eq!(lignes(&pool, &marque).expect("compte"), 0);

    nettoyer(&pool, &marque).expect("nettoyage final");
}

/// Le cloisonnement vient du projet : une inscription de l'entité 1 est invisible
/// depuis l'entité 2, et on n'y inscrit pas sur un événement de l'entité 1.
#[test]
fn cloisonnement_par_le_projet() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    let marque = marque();
    nettoyer(&pool, &marque).expect("nettoyage initial");
    let (_projet, evenement) = evenement(&pool, &marque, "A").expect("fixture");
    let adresse = email(&marque, "camille");
    let rowid = inscrire(&pool, ENTITY, &nouvelle(evenement, &adresse)).expect("inscription");

    // La table n'a pas de colonne `entity` : c'est `llx_projet.entity` qui filtre.
    assert!(
        get(&pool, 2, rowid).expect("lecture").is_none(),
        "une inscription de l'entité 1 ne se lit pas depuis l'entité 2"
    );
    let liste = list(&pool, 2, 100).expect("liste");
    assert!(
        !liste
            .iter()
            .any(|i| i.email.as_deref() == Some(adresse.as_str())),
        "et elle n'apparaît pas dans la liste de l'entité 2"
    );

    // Inscrire depuis l'entité 2 sur un événement de l'entité 1 : refusé, parce que
    // le projet de l'événement appartient à l'entité 1.
    let erreur = inscrire(&pool, 2, &nouvelle(evenement, &email(&marque, "autre")))
        .expect_err("l'événement n'existe pas dans l'entité 2")
        .to_string();
    assert!(erreur.contains("introuvable"), "{erreur}");

    nettoyer(&pool, &marque).expect("nettoyage final");
}

/// Une société réelle de l'entité 1, pour les tests qui en ont besoin.
fn societe_de_test(pool: &Pool) -> i64 {
    let mut conn = pool.get_conn().expect("connexion");
    let id: Option<i64> = conn
        .query_first("SELECT rowid FROM llx_societe WHERE entity = 1 ORDER BY rowid LIMIT 1")
        .expect("lecture d'une société");
    id.expect("l'instance de référence a des sociétés")
}
