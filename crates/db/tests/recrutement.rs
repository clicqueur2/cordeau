//! Tests d'intégration du module Recrutement.
//!
//! Les tests qui touchent la base ne tournent que si `DOLIBARR_DSN` est posé :
//! sans base, ils s'ignorent **en le disant**. Les tests de validation et de
//! séquence, eux, sont purs et tournent toujours — ils ne demandent ni MariaDB
//! ni horloge.
//!
//! Tout est écrit en entité 1, sous une marque `ZZTEST-…` propre à chaque test.
//! Le nettoyage porte sur **cette marque**, jamais sur tout le préfixe : cargo
//! lance les tests en parallèle, et un `DELETE … LIKE 'ZZTEST%'` global
//! effacerait les lignes du test voisin en pleine vérification.
//!
//! Les tables `llx_recruitment_*` sont **vides** sur l'instance de référence :
//! aucun historique réel n'est en jeu. Les deux seules lignes partagées sont un
//! utilisateur (lecture seule) et le dictionnaire `llx_c_recruitment_origin`,
//! lui aussi vide, dans lequel chaque test crée puis supprime sa propre origine.
//!
//! ## Le module est déclaré dans `lib.rs`
//!
//! `crates/db/src/lib.rs` porte `pub mod recrutement;` : les tests passent donc
//! par `db::recrutement`, la vraie déclaration, et non par une inclusion de
//! fichier. Le module est ainsi compilé là où il vit, avec son `crate::divers`
//! d'origine : une erreur de déclaration dans `lib.rs` se verrait tout de suite.

use std::sync::atomic::{AtomicU32, Ordering};

use mysql::params;
use mysql::prelude::Queryable;
use mysql::Pool;

use db::recrutement::{
    bilan_postes, candidatures, changer_statut_candidature, clore_offre, compter_candidatures,
    creer_candidature, creer_offre, libelle_statut_candidature, libelle_statut_offre, offres,
    publier_offre, transition_permise, valider_candidature, valider_offre, NouvelleCandidature,
    NouvelleOffre, CANDIDATURE_ANNULEE, CANDIDATURE_CONTRAT_PROPOSE, CANDIDATURE_RECUE,
    CANDIDATURE_REFUSEE, CANDIDATURE_RETENUE, STATUT_BROUILLON, STATUT_CLOTUREE, STATUT_PUBLIEE,
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

/// Une marque unique par test : secondes + compteur, préfixée `ZZTEST`.
fn marque() -> String {
    let numero = COMPTEUR.fetch_add(1, Ordering::Relaxed);
    let secondes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duree| duree.as_secs())
        .unwrap_or(0);
    format!("{secondes}{numero:02}")
}

/// Le code de l'origine de candidature d'un test (`varchar(32)`, donc court).
fn code_origine(suffixe: &str) -> String {
    format!("ZZTEST{suffixe}")
}

/// Supprime ce que **ce test** a écrit, et rien d'autre.
fn nettoyer(pool: &Pool, suffixe: &str) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let motif_reference = format!("ZZTEST-{suffixe}%");
    let motif_nom = format!("ZZTEST-Nom-{suffixe}%");
    let code = code_origine(suffixe);
    let _ = conn.exec_drop(
        "DELETE FROM llx_recruitment_recruitmentcandidature \
         WHERE lastname LIKE :nom \
            OR fk_recruitmentjobposition IN \
               (SELECT rowid FROM llx_recruitment_recruitmentjobposition \
                 WHERE ref LIKE :reference)",
        params! { "nom" => motif_nom.as_str(), "reference" => motif_reference.as_str() },
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_recruitment_recruitmentjobposition WHERE ref LIKE :reference",
        params! { "reference" => motif_reference.as_str() },
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_c_recruitment_origin WHERE code = :code",
        params! { "code" => code.as_str() },
    );
}

/// Un utilisateur réel de l'entité 1 : `fk_user_recruiter` n'a aucune clé
/// étrangère, on ne veut pas d'un identifiant inventé.
fn utilisateur(pool: &Pool) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity ORDER BY rowid LIMIT 1",
        params! { "entity" => ENTITY },
    )
    .ok()
    .flatten()
}

/// Une société réelle de l'entité 1, pour lier une offre à un client.
fn societe(pool: &Pool) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE entity = :entity ORDER BY rowid LIMIT 1",
        params! { "entity" => ENTITY },
    )
    .ok()
    .flatten()
}

/// Une origine de candidature jetable.
///
/// Le dictionnaire est **vide** sur l'instance de référence (relevé :
/// `SELECT * FROM llx_c_recruitment_origin LIMIT 10` ne rend aucune ligne) :
/// impossible d'en réutiliser une. Elle est propre au test (son code porte la
/// marque) et supprimée par [`nettoyer`].
fn origine_de_test(pool: &Pool, suffixe: &str) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    let code = code_origine(suffixe);
    conn.exec_drop(
        "INSERT INTO llx_c_recruitment_origin (code, label, active) \
         VALUES (:code, 'Origine de test', 1)",
        params! { "code" => code.as_str() },
    )
    .ok()?;
    Some(conn.last_insert_id() as i64)
}

fn offre_de_test(pool: &Pool, suffixe: &str, recruteur: i64) -> NouvelleOffre {
    NouvelleOffre {
        ref_: format!("ZZTEST-{suffixe}"),
        label: format!("Offre de test {suffixe}"),
        description: "Maçon qualifié, pierre de taille.".into(),
        fk_user_recruiter: recruteur,
        date_planned: Some("2026-12-01".into()),
        // `nb_recruitment_planned` s'écrit dans la colonne `qty`.
        nb_recruitment_planned: 2,
        fk_country: None,
        fk_soc: societe(pool),
    }
}

fn candidature_de_test(fk_offre: i64, suffixe: &str, origine: Option<i64>) -> NouvelleCandidature {
    NouvelleCandidature {
        fk_offre,
        lastname: format!("ZZTEST-Nom-{suffixe}"),
        firstname: Some("Camille".into()),
        email: Some(format!("zztest.{suffixe}@example.test")),
        phone: Some("+33 6 00 00 00 00".into()),
        fk_origin: origine,
        date_birth: Some("1990-04-12".into()),
        note: Some("Candidature de test".into()),
    }
}

/// Crée une offre et la publie : le cas courant d'un poste ouvert.
fn offre_publiee(pool: &Pool, suffixe: &str) -> i64 {
    let recruteur = utilisateur(pool).expect("un utilisateur dans l'entité 1");
    let new = offre_de_test(pool, suffixe, recruteur);
    let id = creer_offre(pool, ENTITY, &new).expect("offre créée");
    publier_offre(pool, ENTITY, id).expect("offre publiée");
    id
}

/// Relit une offre par son `rowid`, en passant par la liste publique.
fn offre_relue(pool: &Pool, id: i64) -> db::recrutement::Offre {
    offres(pool, ENTITY, 200)
        .expect("liste des offres")
        .into_iter()
        .find(|offre| offre.rowid == id)
        .expect("offre dans la liste")
}

/// Une offre minimale valide, pour les tests purs.
fn offre_minimale() -> NouvelleOffre {
    NouvelleOffre {
        ref_: "ZZTEST-PURE".into(),
        label: "Maçon".into(),
        description: String::new(),
        fk_user_recruiter: 1,
        date_planned: None,
        nb_recruitment_planned: 1,
        fk_country: None,
        fk_soc: None,
    }
}

/// Une candidature minimale valide, pour les tests purs.
fn candidature_minimale(fk_offre: i64) -> NouvelleCandidature {
    NouvelleCandidature {
        fk_offre,
        lastname: "ZZTEST-Nom-PUR".into(),
        firstname: None,
        email: None,
        phone: None,
        fk_origin: None,
        date_birth: None,
        note: None,
    }
}

// ---------------------------------------------------------------------------
// Validation pure : aucun besoin de base
// ---------------------------------------------------------------------------

/// Le témoin. Sans lui, les refus qui suivent pourraient passer pour de bonnes
/// raisons alors qu'ils refusent tout.
#[test]
fn une_offre_correcte_passe_la_validation() {
    assert!(valider_offre(&offre_minimale()).is_ok());
}

#[test]
fn une_reference_vide_est_refusee() {
    let mut offre = offre_minimale();
    offre.ref_ = "   ".into();
    let erreur = valider_offre(&offre).expect_err("référence blanche");
    assert!(erreur.to_string().contains("référence"), "{erreur}");

    offre.ref_ = String::new();
    let erreur = valider_offre(&offre).expect_err("référence vide");
    assert!(erreur.to_string().contains("obligatoire"), "{erreur}");
}

#[test]
fn un_nombre_de_postes_negatif_est_refuse() {
    let mut offre = offre_minimale();
    offre.nb_recruitment_planned = -1;
    let erreur = valider_offre(&offre).expect_err("postes négatifs");
    assert!(erreur.to_string().contains("négatif"), "{erreur}");

    // Zéro n'est pas refusé : la demande ne portait que sur le négatif, et une
    // offre en cours de cadrage peut n'annoncer aucun poste.
    offre.nb_recruitment_planned = 0;
    assert!(valider_offre(&offre).is_ok());
}

#[test]
fn une_date_planifiee_invalide_est_refusee() {
    let mut offre = offre_minimale();
    offre.date_planned = Some("2026-02-30".into());
    let erreur = valider_offre(&offre).expect_err("30 février");
    assert!(erreur.to_string().contains("date prévue"), "{erreur}");

    offre.date_planned = Some("01/12/2026".into());
    assert!(valider_offre(&offre).is_err());

    // Un champ vide n'est pas une date invalide : `date_planned` est nullable.
    offre.date_planned = Some("   ".into());
    assert!(valider_offre(&offre).is_ok());
}

/// `fk_country` est dans la structure demandée, mais la table n'a aucune colonne
/// de pays : il est refusé plutôt que perdu en silence.
#[test]
fn le_pays_est_refuse_parce_que_la_colonne_n_existe_pas() {
    let mut offre = offre_minimale();
    offre.fk_country = Some(74);
    let erreur = valider_offre(&offre).expect_err("fk_country");
    let message = erreur.to_string();
    assert!(message.contains("fk_country"), "{message}");
    assert!(
        message.contains("societe"),
        "le message dit où vit le pays : {message}"
    );
}

#[test]
fn un_nom_de_candidat_vide_est_refuse() {
    let mut candidature = candidature_minimale(1);
    assert!(valider_candidature(&candidature).is_ok());

    candidature.lastname = "  ".into();
    let erreur = valider_candidature(&candidature).expect_err("nom blanc");
    assert!(erreur.to_string().contains("nom"), "{erreur}");
}

#[test]
fn un_email_sans_arobase_est_refuse() {
    let mut candidature = candidature_minimale(1);
    candidature.email = Some("camille.example.test".into());
    let erreur = valider_candidature(&candidature).expect_err("sans arobase");
    assert!(erreur.to_string().contains("arobase"), "{erreur}");

    // Une adresse plausible passe, et un champ vide n'est pas une adresse.
    candidature.email = Some("camille@example.test".into());
    assert!(valider_candidature(&candidature).is_ok());
    candidature.email = Some("   ".into());
    assert!(valider_candidature(&candidature).is_ok());

    candidature.email = Some("@example.test".into());
    assert!(
        valider_candidature(&candidature).is_err(),
        "rien avant l'arobase"
    );
}

#[test]
fn une_date_de_naissance_invalide_est_refusee() {
    let mut candidature = candidature_minimale(1);
    candidature.date_birth = Some("1990-04-31".into());
    let erreur = valider_candidature(&candidature).expect_err("31 avril");
    assert!(erreur.to_string().contains("date de naissance"), "{erreur}");

    candidature.date_birth = Some("1990-04-12".into());
    assert!(valider_candidature(&candidature).is_ok());
}

/// Le cycle de la candidature, sans base : reçue → contrat proposé → retenue,
/// et les deux sorties que la source prévoit.
#[test]
fn les_transitions_de_candidature_suivent_la_sequence() {
    // Le cycle demandé.
    assert!(transition_permise(
        CANDIDATURE_RECUE,
        CANDIDATURE_CONTRAT_PROPOSE
    ));
    assert!(transition_permise(
        CANDIDATURE_CONTRAT_PROPOSE,
        CANDIDATURE_RETENUE
    ));
    // Les sorties.
    assert!(transition_permise(CANDIDATURE_RECUE, CANDIDATURE_REFUSEE));
    assert!(transition_permise(CANDIDATURE_RECUE, CANDIDATURE_ANNULEE));
    // La réouverture, telle que `reopen()` la définit (8, 9 et 6 → 1).
    assert!(transition_permise(CANDIDATURE_REFUSEE, CANDIDATURE_RECUE));
    assert!(transition_permise(CANDIDATURE_ANNULEE, CANDIDATURE_RECUE));
    // Hors séquence : on ne saute pas d'étape…
    assert!(!transition_permise(CANDIDATURE_RECUE, CANDIDATURE_RETENUE));
    // …et « retenue » est terminal.
    assert!(!transition_permise(
        CANDIDATURE_RETENUE,
        CANDIDATURE_REFUSEE
    ));
    assert!(!transition_permise(CANDIDATURE_RETENUE, CANDIDATURE_RECUE));
    // Le statut 2 n'existe pas : commenté dans la source.
    assert!(!transition_permise(CANDIDATURE_RECUE, 2));
}

#[test]
fn les_libelles_de_statut_sont_ceux_de_la_source() {
    assert_eq!(libelle_statut_offre(STATUT_BROUILLON), "brouillon");
    assert_eq!(libelle_statut_offre(STATUT_PUBLIEE), "publiée");
    assert_eq!(libelle_statut_offre(STATUT_CLOTUREE), "clôturée");
    assert_eq!(libelle_statut_offre(42), "statut inconnu");

    assert_eq!(
        libelle_statut_candidature(CANDIDATURE_RECUE),
        "reçue (entretien à faire)"
    );
    assert_eq!(
        libelle_statut_candidature(CANDIDATURE_CONTRAT_PROPOSE),
        "contrat proposé"
    );
    assert_eq!(libelle_statut_candidature(CANDIDATURE_RETENUE), "retenue");
    assert_eq!(libelle_statut_candidature(CANDIDATURE_REFUSEE), "refusée");
    assert_eq!(libelle_statut_candidature(42), "statut inconnu");
}

// ---------------------------------------------------------------------------
// Écriture et lecture en base
// ---------------------------------------------------------------------------

#[test]
fn creation_offre_et_candidature() {
    let Some(pool) = pool_or_skip() else { return };
    let Some(recruteur) = utilisateur(&pool) else {
        eprintln!("aucun utilisateur en entité {ENTITY} : test ignoré");
        return;
    };
    let suffixe = marque();
    let new = offre_de_test(&pool, &suffixe, recruteur);

    let id = creer_offre(&pool, ENTITY, &new).expect("offre créée");
    assert!(id > 0);

    // Une offre naît en brouillon : publier est un acte distinct.
    let offre = offre_relue(&pool, id);
    assert_eq!(offre.entity, ENTITY);
    assert_eq!(offre.ref_, new.ref_);
    assert_eq!(offre.label, new.label);
    assert_eq!(offre.nb_postes, 2, "qty porte le nombre de postes prévus");
    assert_eq!(offre.fk_soc, new.fk_soc);
    assert_eq!(offre.fk_user_recruiter, Some(recruteur));
    assert!(offre.recruteur.is_some(), "le login du recruteur est joint");
    if new.fk_soc.is_some() {
        assert!(offre.societe.is_some(), "le nom de la société est joint");
    }
    assert_eq!(offre.date_planned.as_deref(), Some("2026-12-01"));
    // Le libellé porte des accents : on vérifie une sous-chaîne **sans accent**
    // pour que l'assertion ne dépende pas du jeu de caractères de la connexion.
    let description = offre.description.as_deref().unwrap_or("");
    assert!(
        description.contains("pierre de taille"),
        "la description est écrite telle quelle : {description}"
    );
    assert_eq!(offre.statut, STATUT_BROUILLON);
    assert!(offre.date_creation.is_some());

    publier_offre(&pool, ENTITY, id).expect("publication");

    let origine = origine_de_test(&pool, &suffixe);
    let candidature = candidature_de_test(id, &suffixe, origine);
    let creee = creer_candidature(&pool, ENTITY, &candidature).expect("candidature créée");
    assert!(creee.rowid > 0);
    assert!(
        !creee.offre_hors_publication,
        "l'offre était publiée : rien à signaler"
    );

    let liste = candidatures(&pool, ENTITY, id).expect("candidatures de l'offre");
    let une = liste
        .iter()
        .find(|element| element.rowid == creee.rowid)
        .expect("candidature dans la liste");
    assert_eq!(une.entity, ENTITY);
    assert!(!une.ref_.is_empty(), "la référence est numérotée");
    assert_eq!(une.fk_offre, Some(id));
    assert_eq!(une.offre_ref.as_deref(), Some(new.ref_.as_str()));
    assert_eq!(une.lastname.as_deref(), Some(candidature.lastname.as_str()));
    assert_eq!(une.firstname.as_deref(), Some("Camille"));
    assert_eq!(une.email.as_deref(), candidature.email.as_deref());
    assert_eq!(une.phone.as_deref(), candidature.phone.as_deref());
    assert_eq!(une.date_birth.as_deref(), Some("1990-04-12"));
    assert_eq!(une.statut, CANDIDATURE_RECUE);
    assert_eq!(une.fk_origin, origine);
    assert!(une.origine.is_some(), "le libellé de l'origine est joint");
    // L'offre était publiée : la note du recruteur reste seule, sans mention.
    assert_eq!(une.note_private.as_deref(), Some("Candidature de test"));
    assert!(une.date_creation.is_some());

    nettoyer(&pool, &suffixe);
}

#[test]
fn un_recruteur_inexistant_est_refuse() {
    let Some(pool) = pool_or_skip() else { return };
    let suffixe = marque();
    let new = offre_de_test(&pool, &suffixe, 987_654_321);

    // La validation pure passe : l'identifiant est positif, c'est la base qui
    // doit refuser le fantôme.
    assert!(valider_offre(&new).is_ok());
    let erreur = creer_offre(&pool, ENTITY, &new).expect_err("recruteur fantôme");
    assert!(erreur.to_string().contains("recruteur"), "{erreur}");

    nettoyer(&pool, &suffixe);
}

#[test]
fn une_candidature_sur_une_offre_inexistante_est_refusee() {
    let Some(pool) = pool_or_skip() else { return };
    let suffixe = marque();
    let candidature = candidature_de_test(987_654_321, &suffixe, None);

    let erreur = creer_candidature(&pool, ENTITY, &candidature).expect_err("offre fantôme");
    assert!(erreur.to_string().contains("introuvable"), "{erreur}");

    nettoyer(&pool, &suffixe);
}

#[test]
fn une_origine_inexistante_est_refusee() {
    let Some(pool) = pool_or_skip() else { return };
    let suffixe = marque();
    let id = offre_publiee(&pool, &suffixe);
    let candidature = candidature_de_test(id, &suffixe, Some(987_654_321));

    let erreur = creer_candidature(&pool, ENTITY, &candidature).expect_err("origine fantôme");
    assert!(erreur.to_string().contains("origine"), "{erreur}");

    nettoyer(&pool, &suffixe);
}

#[test]
fn publication_puis_refus_de_republier() {
    let Some(pool) = pool_or_skip() else { return };
    let Some(recruteur) = utilisateur(&pool) else {
        eprintln!("aucun utilisateur en entité {ENTITY} : test ignoré");
        return;
    };
    let suffixe = marque();
    let new = offre_de_test(&pool, &suffixe, recruteur);
    let id = creer_offre(&pool, ENTITY, &new).expect("offre créée");

    publier_offre(&pool, ENTITY, id).expect("première publication");
    assert_eq!(offre_relue(&pool, id).statut, STATUT_PUBLIEE);

    let erreur = publier_offre(&pool, ENTITY, id).expect_err("déjà publiée");
    assert!(erreur.to_string().contains("déjà publiée"), "{erreur}");

    // Une offre qui n'existe pas est refusée aussi, dans l'entité demandée.
    let erreur = publier_offre(&pool, ENTITY, 987_654_321).expect_err("offre fantôme");
    assert!(erreur.to_string().contains("introuvable"), "{erreur}");

    nettoyer(&pool, &suffixe);
}

#[test]
fn cloture_refusee_si_l_offre_n_est_pas_publiee() {
    let Some(pool) = pool_or_skip() else { return };
    let Some(recruteur) = utilisateur(&pool) else {
        eprintln!("aucun utilisateur en entité {ENTITY} : test ignoré");
        return;
    };
    let suffixe = marque();
    let new = offre_de_test(&pool, &suffixe, recruteur);
    let id = creer_offre(&pool, ENTITY, &new).expect("offre créée");

    // `cancel()` de Dolibarr exige `STATUS_VALIDATED` : un brouillon n'a jamais
    // été ouvert, il n'a rien à clore.
    let erreur = clore_offre(&pool, ENTITY, id).expect_err("brouillon");
    assert!(erreur.to_string().contains("publiée"), "{erreur}");

    publier_offre(&pool, ENTITY, id).expect("publication");
    clore_offre(&pool, ENTITY, id).expect("clôture");
    assert_eq!(offre_relue(&pool, id).statut, STATUT_CLOTUREE);

    nettoyer(&pool, &suffixe);
}

#[test]
fn changer_statut_dans_l_ordre_et_refus_hors_sequence() {
    let Some(pool) = pool_or_skip() else { return };
    let suffixe = marque();
    let id = offre_publiee(&pool, &suffixe);

    let premiere = creer_candidature(
        &pool,
        ENTITY,
        &candidature_de_test(id, &format!("{suffixe}-a"), None),
    )
    .expect("candidature créée")
    .rowid;
    changer_statut_candidature(&pool, ENTITY, premiere, CANDIDATURE_CONTRAT_PROPOSE)
        .expect("reçue → contrat proposé");
    changer_statut_candidature(&pool, ENTITY, premiere, CANDIDATURE_RETENUE)
        .expect("contrat proposé → retenue");

    // « retenue » est terminal : on ne refuse pas une candidature acceptée, on
    // la rouvre.
    let erreur = changer_statut_candidature(&pool, ENTITY, premiere, CANDIDATURE_REFUSEE)
        .expect_err("retenue → refusée");
    assert!(
        erreur.to_string().contains("transition refusée"),
        "{erreur}"
    );

    // Le statut 2 n'existe pas dans Dolibarr 24 : commenté dans la source.
    let erreur = changer_statut_candidature(&pool, ENTITY, premiere, 2).expect_err("statut 2");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");

    let seconde = creer_candidature(
        &pool,
        ENTITY,
        &candidature_de_test(id, &format!("{suffixe}-b"), None),
    )
    .expect("candidature créée")
    .rowid;

    // On ne saute pas d'étape : reçue → retenue est refusé.
    let erreur = changer_statut_candidature(&pool, ENTITY, seconde, CANDIDATURE_RETENUE)
        .expect_err("reçue → retenue");
    assert!(
        erreur.to_string().contains("transition refusée"),
        "{erreur}"
    );

    changer_statut_candidature(&pool, ENTITY, seconde, CANDIDATURE_REFUSEE).expect("refus");
    changer_statut_candidature(&pool, ENTITY, seconde, CANDIDATURE_RECUE)
        .expect("réouverture, comme reopen()");
    let erreur = changer_statut_candidature(&pool, ENTITY, seconde, CANDIDATURE_RECUE)
        .expect_err("déjà reçue");
    assert!(erreur.to_string().contains("déjà au statut"), "{erreur}");

    // Ce que la base porte vraiment.
    let liste = candidatures(&pool, ENTITY, id).expect("candidatures");
    let une = liste
        .iter()
        .find(|element| element.rowid == premiere)
        .expect("première candidature");
    assert_eq!(une.statut, CANDIDATURE_RETENUE);
    let autre = liste
        .iter()
        .find(|element| element.rowid == seconde)
        .expect("seconde candidature");
    assert_eq!(autre.statut, CANDIDATURE_RECUE);

    nettoyer(&pool, &suffixe);
}

#[test]
fn compter_les_candidatures_d_une_offre() {
    let Some(pool) = pool_or_skip() else { return };
    let suffixe = marque();
    let id = offre_publiee(&pool, &suffixe);
    let autre = offre_publiee(&pool, &format!("{suffixe}-bis"));

    for indice in 0..2 {
        creer_candidature(
            &pool,
            ENTITY,
            &candidature_de_test(id, &format!("{suffixe}-{indice}"), None),
        )
        .expect("candidature créée");
    }

    assert_eq!(compter_candidatures(&pool, ENTITY, id).expect("compte"), 2);
    // Le compte est celui de l'offre, pas celui de l'entité.
    assert_eq!(
        compter_candidatures(&pool, ENTITY, autre).expect("compte"),
        0
    );
    // Une offre inexistante rend 0 : c'est un compte, pas une existence.
    assert_eq!(
        compter_candidatures(&pool, ENTITY, 987_654_321).expect("compte"),
        0
    );

    // Le bilan sépare ce qui est prévu de ce qui est déclaré pourvu.
    let avant = bilan_postes(&pool, ENTITY, id).expect("bilan");
    assert_eq!(avant.prevus, 2, "qty porte les postes prévus");
    assert_eq!(avant.pourvus, 0);

    let premiere = candidatures(&pool, ENTITY, id).expect("candidatures")[0].rowid;
    changer_statut_candidature(&pool, ENTITY, premiere, CANDIDATURE_CONTRAT_PROPOSE)
        .expect("contrat proposé");
    changer_statut_candidature(&pool, ENTITY, premiere, CANDIDATURE_RETENUE).expect("retenue");

    let apres = bilan_postes(&pool, ENTITY, id).expect("bilan");
    assert_eq!(apres.prevus, 2);
    assert_eq!(apres.pourvus, 1, "une candidature déclarée retenue");
    // Compter les candidatures n'est pas compter les retenus : une retenue
    // reste une candidature reçue.
    assert_eq!(compter_candidatures(&pool, ENTITY, id).expect("compte"), 2);

    nettoyer(&pool, &suffixe);
}

/// La décision du module : une offre close reçoit quand même la candidature,
/// et le dit.
#[test]
fn une_offre_close_accepte_encore_une_candidature_en_le_signalant() {
    let Some(pool) = pool_or_skip() else { return };
    let suffixe = marque();
    let id = offre_publiee(&pool, &suffixe);
    clore_offre(&pool, ENTITY, id).expect("clôture");
    assert_eq!(offre_relue(&pool, id).statut, STATUT_CLOTUREE);

    let candidature = candidature_de_test(id, &suffixe, None);
    let creee = creer_candidature(&pool, ENTITY, &candidature).expect("acceptée malgré la clôture");
    assert!(
        creee.offre_hors_publication,
        "l'offre était clôturée : la création doit le signaler"
    );

    let liste = candidatures(&pool, ENTITY, id).expect("candidatures");
    let une = liste
        .iter()
        .find(|element| element.rowid == creee.rowid)
        .expect("candidature dans la liste");
    let note = une.note_private.as_deref().unwrap_or("");
    assert!(
        note.contains("Candidature de test"),
        "la note du recruteur est conservée : {note}"
    );
    // La mention est écrite ; on la reconnaît sur des fragments **sans accent**,
    // pour ne pas dépendre du jeu de caractères de la connexion.
    assert!(
        note.contains("offre") && note.contains("statut 9"),
        "la mention donne l'offre et son statut : {note}"
    );
    assert_eq!(
        une.statut, CANDIDATURE_RECUE,
        "elle est reçue, pas mise de côté"
    );
    assert_eq!(compter_candidatures(&pool, ENTITY, id).expect("compte"), 1);

    nettoyer(&pool, &suffixe);
}

/// Les deux tables portent leur propre colonne `entity` : une liste d'entité 2
/// ne voit rien de l'entité 1, même en visant le bon `rowid`.
#[test]
fn les_listes_sont_cloisonnees_par_entite() {
    let Some(pool) = pool_or_skip() else { return };
    let suffixe = marque();
    let id = offre_publiee(&pool, &suffixe);
    creer_candidature(&pool, ENTITY, &candidature_de_test(id, &suffixe, None))
        .expect("candidature créée");

    let ici = offres(&pool, ENTITY, 200).expect("liste");
    assert!(ici.iter().any(|offre| offre.rowid == id));
    let ailleurs = offres(&pool, ENTITY + 1, 200).expect("liste");
    assert!(!ailleurs.iter().any(|offre| offre.rowid == id));
    assert!(
        candidatures(&pool, ENTITY + 1, id)
            .expect("liste")
            .is_empty(),
        "la candidature est cloisonnée par sa propre colonne entity"
    );

    nettoyer(&pool, &suffixe);
}
