//! Tests d'intégration du module Dons.
//!
//! Les tests qui touchent la base ne tournent que si `DOLIBARR_DSN` est posé :
//! sans base, ils s'ignorent en le disant.
//!
//! Tout est écrit en entité 1 et nettoyé derrière : les seules lignes laissées
//! sont celles qui existaient avant le test. Quatre précautions :
//!
//! - l'email porte le nom du test et le PID (`std::process::id()`) : les
//!   fichiers de test tournent en parallèle, et rien n'empêche deux exécutions
//!   de la suite de se croiser sur la même base ;
//! - les tests se **sérialisent** sur un verrou (voir [`VERROU`]) : [`dons::total`]
//!   additionne **tous** les dons de l'entité, et un test qui créerait une ligne
//!   pendant qu'un autre mesure son écart rendrait la mesure fausse ;
//! - le compte bancaire nécessaire à l'encaissement est **jetable** (référence
//!   `TD<pid>`), créé au besoin et supprimé à la fin ; on ne touche jamais aux
//!   comptes réels de l'instance ;
//! - le moyen de paiement est celui du dictionnaire de l'instance ; si le
//!   dictionnaire est vide, le test s'ignore au lieu d'échouer.
//!
//! [`dons::total`]: db::dons::total

/// Une ligne de `llx_payment_donation` relue pour verifier que le reglement
/// recopie bien le don : `(don, montant, banque, moyen, jour)`.
///
/// L'alias n'est pas cosmetique : `clippy` refuse un tuple de plus de six
/// elements, et cette forme se relit mal dans une assertion.
type LignePaiement = (
    Option<i64>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<String>,
);

use std::error::Error;
use std::sync::{Mutex, MutexGuard};

use db::dons::{self, NewDon, STATUT_PROMESSE, STATUT_VALIDE};
use money::{dec, round_money};
use mysql::prelude::*;
use mysql::Pool;
use rust_decimal::Decimal;

const ENTITY: i64 = 1;

/// Auteur des saisies de test. `llx_don.fk_user_author` est NOT NULL mais
/// n'porte aucune clé étrangère vers `llx_user` : un identifiant fixe suffit, et
/// cela évite de dépendre de la présence d'un utilisateur 1 dans la base.
const AUTEUR: i64 = 1;

/// Les tests de ce fichier partagent `llx_don` en entité 1.
///
/// Les tests d'un même fichier tournent en parallèle : sans ce verrou, un test
/// qui crée un don ferait bouger les totaux pendant qu'un autre mesure son
/// écart. Le verrou les sérialise ; ils sont courts, le coût est nul.
static VERROU: Mutex<()> = Mutex::new(());

fn verrou() -> MutexGuard<'static, ()> {
    // Un test déjà paniqué laisse le verrou empoisonné : on le reprend, plutôt
    // que de faire échouer les suivants pour une raison qui ne les concerne pas.
    VERROU.lock().unwrap_or_else(|e| e.into_inner())
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

/// Un email unique par test et par processus.
fn email_test(base: &str) -> String {
    format!("don.{base}.{}@exemple.test", std::process::id())
}

fn nouveau_don(base: &str) -> NewDon {
    NewDon {
        lastname: format!("Dupont {base}"),
        firstname: Some("Marie".into()),
        societe: None,
        email: Some(email_test(base)),
        address: Some("1 rue des Dons".into()),
        zip: Some("69390".into()),
        town: Some("Charly".into()),
        fk_country: 0,
        amount: dec("150.50"),
        datedon: None,
        fk_projet: None,
        note_private: None,
        note_public: None,
        fk_user_author: AUTEUR,
    }
}

/// Efface les dons d'un test (règlements puis dons) : sert aussi à nettoyer le
/// reliquat d'une exécution précédente interrompue.
fn effacer(pool: &Pool, base: &str) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let email = email_test(base);
    let _ = conn.exec_drop(
        "DELETE p FROM llx_payment_donation p \
         INNER JOIN llx_don d ON d.rowid = p.fk_donation \
         WHERE d.entity = ? AND d.email = ?",
        (ENTITY, email.as_str()),
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_don WHERE entity = ? AND email = ?",
        (ENTITY, email.as_str()),
    );
}

/// Efface un don et ses règlements par son `rowid` : indispensable pour un don
/// encaissé, que [`dons::supprimer`] refuse justement de supprimer.
fn effacer_id(pool: &Pool, rowid: i64) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE FROM llx_payment_donation WHERE fk_donation = ?",
        (rowid,),
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_don WHERE entity = ? AND rowid = ?",
        (ENTITY, rowid),
    );
}

/// Combien de dons portent l'email d'un test : sert à prouver qu'un refus
/// n'écrit rien.
fn compter(pool: &Pool, base: &str) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_don WHERE entity = ? AND email = ?",
        (ENTITY, email_test(base)),
    )?;
    Ok(n.unwrap_or(0))
}

/// Combien de règlements pointent sur un don donné.
fn compter_reglements(pool: &Pool, rowid: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_payment_donation WHERE fk_donation = ?",
        (rowid,),
    )?;
    Ok(n.unwrap_or(0))
}

/// Un compte bancaire jetable, pour encaisser sans toucher aux comptes réels.
///
/// `llx_bank_account` impose `ref`, `label`, `fk_pays` et `currency_code` : on
/// les renseigne, le reste reste aux valeurs du schéma. La référence porte le
/// PID pour ne jamais heurter deux exécutions concurrentes de la suite.
fn compte_bancaire_de_test(pool: &Pool) -> Result<i64, Box<dyn Error>> {
    let reference = format!("TD{}", std::process::id());
    // Le libellé aussi porte le PID : `uk_bank_account_label (label, entity)`
    // interdirait deux comptes de test homonymes dans la même entité.
    let libelle = format!("Compte de test dons {}", std::process::id());
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_bank_account WHERE ref = ?",
        (reference.as_str(),),
    )?;
    conn.exec_drop(
        "INSERT INTO llx_bank_account \
         (datec, ref, label, entity, courant, clos, fk_pays, currency_code, state_id) \
         VALUES (NOW(), ?, ?, ?, 1, 0, 0, 'EUR', 0)",
        (reference.as_str(), libelle.as_str(), ENTITY),
    )?;
    Ok(conn.last_insert_id() as i64)
}

fn oter_compte(pool: &Pool, rowid: i64) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop("DELETE FROM llx_bank_account WHERE rowid = ?", (rowid,));
}

/// Un moyen de paiement du dictionnaire : le premier actif.
fn moyen_paiement(pool: &Pool) -> Result<Option<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let id: Option<i64> = conn.exec_first(
        "SELECT id FROM llx_c_paiement WHERE active = 1 ORDER BY id LIMIT 1",
        (),
    )?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// Création et lecture
// ---------------------------------------------------------------------------

#[test]
fn cree_relit_et_liste_un_don() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "creation";
    effacer(&pool, base);

    let id = dons::create(&pool, ENTITY, &nouveau_don(base))?;
    assert!(id > 0);

    let lu = dons::get(&pool, ENTITY, id)?.expect("don relu");
    assert_eq!(lu.rowid, id);
    assert_eq!(lu.entity, ENTITY);
    assert_eq!(round_money(lu.montant), dec("150.50"));
    assert_eq!(lu.statut, STATUT_PROMESSE);
    assert!(!lu.encaisse, "un don créé n'est pas encaissé");
    assert_eq!(lu.fk_payment, None);
    assert!(!lu.public, "un don n'est pas publié par défaut");
    assert_eq!(lu.nom, Some(format!("Dupont {base}")));
    assert_eq!(lu.prenom.as_deref(), Some("Marie"));
    assert_eq!(lu.donateur(), format!("Marie Dupont {base}"));
    assert_eq!(lu.email, Some(email_test(base)));
    assert_eq!(lu.ville.as_deref(), Some("Charly"));
    assert_eq!(lu.pays, 0, "fk_country vaut 0 quand il n'est pas fourni");
    assert!(lu.date_creation.is_some(), "datec posée");
    assert!(
        lu.date_validation.is_none(),
        "une création ne valide pas le don"
    );
    assert!(!lu.encaissable(), "un don non validé n'est pas encaissable");

    let reference = lu.ref_.clone();
    assert!(
        reference.starts_with("DON") && reference[3..].chars().all(|c| c.is_ascii_digit()),
        "format de référence inattendu : {reference}"
    );

    // `datedon` absent : le don est daté du jour, comme le formulaire Dolibarr.
    let aujourdhui = db::aujourdhui(&pool)?;
    assert_eq!(lu.datedon.as_deref(), Some(aujourdhui.as_str()));

    let listes = dons::list(&pool, ENTITY, 1000, 0)?;
    assert!(listes.iter().any(|d| d.rowid == id));

    assert!(
        dons::get(&pool, ENTITY + 1, id)?.is_none(),
        "un don d'une autre entité ne se lit pas"
    );

    effacer(&pool, base);
    Ok(())
}

#[test]
fn accepte_un_don_au_nom_d_une_societe() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "societe";
    effacer(&pool, base);

    // Un don d'entreprise : le nom de la personne est vide, la société porte
    // l'identité. Le refus du don anonyme ne doit pas emporter ce cas.
    let mut new = nouveau_don(base);
    new.lastname = String::new();
    new.firstname = None;
    new.societe = Some(format!("Association {base}"));

    let id = dons::create(&pool, ENTITY, &new)?;
    let lu = dons::get(&pool, ENTITY, id)?.expect("don relu");
    assert_eq!(lu.nom, None);
    assert_eq!(lu.societe, Some(format!("Association {base}")));
    assert_eq!(lu.donateur(), format!("Association {base}"));

    effacer(&pool, base);
    Ok(())
}

#[test]
fn refuse_un_montant_nul() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "montant_nul";
    effacer(&pool, base);

    let mut nul = nouveau_don(base);
    nul.amount = Decimal::ZERO;
    let erreur = dons::create(&pool, ENTITY, &nul).expect_err("montant nul refusé");
    assert!(
        erreur.to_string().contains("strictement positif"),
        "{erreur}"
    );
    assert_eq!(compter(&pool, base)?, 0, "rien ne doit être écrit");

    effacer(&pool, base);
    Ok(())
}

#[test]
fn refuse_un_montant_negatif() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "montant_negatif";
    effacer(&pool, base);

    let mut negatif = nouveau_don(base);
    negatif.amount = dec("-10.00");
    let erreur = dons::create(&pool, ENTITY, &negatif).expect_err("montant négatif refusé");
    assert!(
        erreur.to_string().contains("strictement positif"),
        "{erreur}"
    );
    assert_eq!(compter(&pool, base)?, 0, "rien ne doit être écrit");

    effacer(&pool, base);
    Ok(())
}

#[test]
fn refuse_un_don_sans_nom_ni_societe() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "anonyme";
    effacer(&pool, base);

    let mut anonyme = nouveau_don(base);
    anonyme.lastname = "   ".into();
    anonyme.societe = Some("  ".into());
    let erreur = dons::create(&pool, ENTITY, &anonyme).expect_err("don anonyme refusé");
    assert!(erreur.to_string().contains("société"), "{erreur}");
    assert_eq!(compter(&pool, base)?, 0, "rien ne doit être écrit");

    effacer(&pool, base);
    Ok(())
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[test]
fn valide_puis_refuse_de_revalider() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "validation";
    effacer(&pool, base);

    let id = dons::create(&pool, ENTITY, &nouveau_don(base))?;
    dons::valider(&pool, ENTITY, id)?;

    let lu = dons::get(&pool, ENTITY, id)?.expect("don relu");
    assert_eq!(lu.statut, STATUT_VALIDE);
    assert!(lu.date_validation.is_some(), "date_valid posée");
    assert!(lu.encaissable());

    let erreur = dons::valider(&pool, ENTITY, id).expect_err("double validation refusée");
    assert!(erreur.to_string().contains("déjà validé"), "{erreur}");

    // Un don inconnu ne se valide pas.
    let erreur = dons::valider(&pool, ENTITY, 999_999).expect_err("don inconnu refusé");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");

    effacer(&pool, base);
    Ok(())
}

// ---------------------------------------------------------------------------
// Encaissement
// ---------------------------------------------------------------------------

#[test]
fn encaisse_pose_paid_et_le_lien_vers_le_paiement() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "encaissement";
    effacer(&pool, base);

    let id = dons::create(&pool, ENTITY, &nouveau_don(base))?;
    dons::valider(&pool, ENTITY, id)?;

    let compte = compte_bancaire_de_test(&pool)?;
    let Some(moyen) = moyen_paiement(&pool)? else {
        eprintln!("aucun moyen de paiement dans llx_c_paiement : test ignoré");
        oter_compte(&pool, compte);
        effacer(&pool, base);
        return Ok(());
    };
    let date = db::aujourdhui(&pool)?;

    let paiement = dons::encaisser(&pool, ENTITY, id, compte, moyen, &date)?;
    assert!(paiement > 0, "un règlement doit être créé");

    let lu = dons::get(&pool, ENTITY, id)?.expect("don relu");
    assert!(lu.encaisse, "paid passe à 1");
    assert_eq!(
        lu.fk_payment,
        Some(paiement),
        "le don pointe sur la ligne de règlement"
    );
    assert!(!lu.encaissable(), "un don encaissé ne s'encaisse plus");

    // La ligne de `llx_payment_donation` recopie le montant du don.
    let mut conn = pool.get_conn()?;
    let ligne: Option<LignePaiement> = conn.exec_first(
        "SELECT fk_donation, CAST(amount AS CHAR), fk_bank, fk_typepayment, \
                    DATE_FORMAT(datep, '%Y-%m-%d') \
             FROM llx_payment_donation WHERE rowid = ?",
        (paiement,),
    )?;
    drop(conn);
    let (don, montant, banque, moyen_lu, jour) = ligne.expect("règlement relu");
    assert_eq!(don, Some(id));
    let montant = montant.unwrap_or_default();
    assert_eq!(
        round_money(dec(&montant)),
        dec("150.50"),
        "le règlement recopie le montant du don (lu : {montant})"
    );
    assert_eq!(banque, Some(compte));
    assert_eq!(moyen_lu, Some(moyen));
    assert_eq!(jour.as_deref(), Some(date.as_str()));

    effacer_id(&pool, id);
    oter_compte(&pool, compte);
    Ok(())
}

#[test]
fn refuse_d_encaisser_un_don_non_valide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "encaissement_non_valide";
    effacer(&pool, base);

    let id = dons::create(&pool, ENTITY, &nouveau_don(base))?;
    let compte = compte_bancaire_de_test(&pool)?;
    let Some(moyen) = moyen_paiement(&pool)? else {
        eprintln!("aucun moyen de paiement dans llx_c_paiement : test ignoré");
        oter_compte(&pool, compte);
        effacer(&pool, base);
        return Ok(());
    };
    let date = db::aujourdhui(&pool)?;

    let erreur = dons::encaisser(&pool, ENTITY, id, compte, moyen, &date)
        .expect_err("don non validé refusé");
    assert!(erreur.to_string().contains("validé"), "{erreur}");
    assert_eq!(
        compter_reglements(&pool, id)?,
        0,
        "aucun règlement ne doit être écrit"
    );
    assert!(!dons::get(&pool, ENTITY, id)?.unwrap().encaisse);

    effacer_id(&pool, id);
    oter_compte(&pool, compte);
    Ok(())
}

#[test]
fn refuse_de_encaisser_deux_fois() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "double_encaissement";
    effacer(&pool, base);

    let id = dons::create(&pool, ENTITY, &nouveau_don(base))?;
    dons::valider(&pool, ENTITY, id)?;
    let compte = compte_bancaire_de_test(&pool)?;
    let Some(moyen) = moyen_paiement(&pool)? else {
        eprintln!("aucun moyen de paiement dans llx_c_paiement : test ignoré");
        oter_compte(&pool, compte);
        effacer(&pool, base);
        return Ok(());
    };
    let date = db::aujourdhui(&pool)?;

    let premier = dons::encaisser(&pool, ENTITY, id, compte, moyen, &date)?;
    let erreur = dons::encaisser(&pool, ENTITY, id, compte, moyen, &date)
        .expect_err("second encaissement refusé");
    assert!(erreur.to_string().contains("déjà encaissé"), "{erreur}");
    assert_eq!(
        compter_reglements(&pool, id)?,
        1,
        "un seul règlement doit exister"
    );
    assert_eq!(
        dons::get(&pool, ENTITY, id)?.unwrap().fk_payment,
        Some(premier)
    );

    effacer_id(&pool, id);
    oter_compte(&pool, compte);
    Ok(())
}

#[test]
fn refuse_un_compte_bancaire_inexistant() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "compte_inconnu";
    effacer(&pool, base);

    let id = dons::create(&pool, ENTITY, &nouveau_don(base))?;
    dons::valider(&pool, ENTITY, id)?;
    let Some(moyen) = moyen_paiement(&pool)? else {
        eprintln!("aucun moyen de paiement dans llx_c_paiement : test ignoré");
        effacer(&pool, base);
        return Ok(());
    };
    let date = db::aujourdhui(&pool)?;

    let erreur = dons::encaisser(&pool, ENTITY, id, 999_999, moyen, &date)
        .expect_err("compte inconnu refusé");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");
    assert_eq!(
        compter_reglements(&pool, id)?,
        0,
        "aucun règlement ne doit être écrit"
    );
    assert!(
        !dons::get(&pool, ENTITY, id)?.unwrap().encaisse,
        "le don reste non encaissé"
    );

    effacer(&pool, base);
    Ok(())
}

// ---------------------------------------------------------------------------
// Suppression
// ---------------------------------------------------------------------------

#[test]
fn supprime_un_don_non_encaisse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "suppression";
    effacer(&pool, base);

    let id = dons::create(&pool, ENTITY, &nouveau_don(base))?;
    dons::supprimer(&pool, ENTITY, id)?;
    assert!(dons::get(&pool, ENTITY, id)?.is_none(), "le don est parti");

    let erreur = dons::supprimer(&pool, ENTITY, id).expect_err("don déjà supprimé");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");

    effacer(&pool, base);
    Ok(())
}

#[test]
fn refuse_de_supprimer_un_don_encaisse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "suppression_encaisse";
    effacer(&pool, base);

    let id = dons::create(&pool, ENTITY, &nouveau_don(base))?;
    dons::valider(&pool, ENTITY, id)?;
    let compte = compte_bancaire_de_test(&pool)?;
    let Some(moyen) = moyen_paiement(&pool)? else {
        eprintln!("aucun moyen de paiement dans llx_c_paiement : test ignoré");
        oter_compte(&pool, compte);
        effacer(&pool, base);
        return Ok(());
    };
    let date = db::aujourdhui(&pool)?;
    dons::encaisser(&pool, ENTITY, id, compte, moyen, &date)?;

    let erreur = dons::supprimer(&pool, ENTITY, id).expect_err("don encaissé non supprimable");
    assert!(erreur.to_string().contains("encaissé"), "{erreur}");
    assert!(
        dons::get(&pool, ENTITY, id)?.is_some(),
        "le don est toujours là"
    );
    assert_eq!(compter_reglements(&pool, id)?, 1);

    effacer_id(&pool, id);
    oter_compte(&pool, compte);
    Ok(())
}

// ---------------------------------------------------------------------------
// Totaux
// ---------------------------------------------------------------------------

#[test]
fn total_distingue_promis_et_encaisse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base_a = "total_a";
    let base_b = "total_b";
    effacer(&pool, base_a);
    effacer(&pool, base_b);

    // La base réelle peut déjà contenir des dons : on mesure des écarts, jamais
    // des valeurs absolues.
    let (avant_promis, avant_encaisse) = dons::total(&pool, ENTITY)?;

    let mut don_a = nouveau_don(base_a);
    don_a.amount = dec("100.00");
    let a = dons::create(&pool, ENTITY, &don_a)?;
    let mut don_b = nouveau_don(base_b);
    don_b.amount = dec("50.25");
    let b = dons::create(&pool, ENTITY, &don_b)?;

    dons::valider(&pool, ENTITY, a)?;
    dons::valider(&pool, ENTITY, b)?;

    let compte = compte_bancaire_de_test(&pool)?;
    let Some(moyen) = moyen_paiement(&pool)? else {
        eprintln!("aucun moyen de paiement dans llx_c_paiement : test ignoré");
        oter_compte(&pool, compte);
        effacer(&pool, base_a);
        effacer(&pool, base_b);
        return Ok(());
    };
    let date = db::aujourdhui(&pool)?;
    dons::encaisser(&pool, ENTITY, a, compte, moyen, &date)?;

    let (promis, encaisse) = dons::total(&pool, ENTITY)?;
    assert_eq!(
        promis - avant_promis,
        dec("150.25"),
        "les deux dons comptent dans le promis"
    );
    assert_eq!(
        encaisse - avant_encaisse,
        dec("100.00"),
        "seul le don encaissé compte dans l'encaissé"
    );
    assert_eq!(
        (promis - encaisse) - (avant_promis - avant_encaisse),
        dec("50.25"),
        "le reste à encaisser est bien la promesse non encore payée"
    );

    // Un don non encaissé ne doit pas s'ajouter à l'encaissé.
    assert_ne!(
        promis - avant_promis,
        encaisse - avant_encaisse,
        "promis et encaissé ne se confondent pas"
    );

    effacer_id(&pool, a);
    effacer_id(&pool, b);
    oter_compte(&pool, compte);
    Ok(())
}

#[test]
fn refuse_un_email_sans_arobase() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "email_invalide";
    effacer(&pool, base);

    let mut invalide = nouveau_don(base);
    invalide.email = Some("don.exemple.test".into());
    let erreur = dons::create(&pool, ENTITY, &invalide).expect_err("email sans arobase refusé");
    assert!(erreur.to_string().contains("arobase"), "{erreur}");
    assert_eq!(compter(&pool, base)?, 0, "rien ne doit être écrit");

    effacer(&pool, base);
    Ok(())
}
