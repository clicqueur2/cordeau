//! Tests d'intégration du module salaires, sur la base Dolibarr réelle.
//!
//! Comme partout dans cette crate, les tests s'ignorent poliment quand
//! `DOLIBARR_DSN` n'est pas posé : ils vérifient un comportement contre MariaDB,
//! pas contre un faux. Chacun nettoie derrière lui — y compris les versements,
//! qu'aucune clé étrangère ne retient.
//!
//! Deux précautions valent d'être dites :
//!
//! - **le salarié est lu dans `llx_user`**, jamais inventé : `llx_salary.fk_user`
//!   n'a pas de clé étrangère, et un identifiant fictif ferait passer les tests
//!   tout en produisant une fiche rattachée à personne ;
//! - **l'exercice 1990 sert de bac à sable** pour `total_par_annee` : aucune
//!   autre ligne ne peut s'y trouver, le total est donc déterministe même sur
//!   une base déjà peuplée.

use std::error::Error;
use std::str::FromStr;

use db::salaire::{self, NouveauSalaire};
use mysql::params;
use mysql::prelude::Queryable;
use mysql::Pool;
use rust_decimal::Decimal;

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

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// Un salarié réel de l'entité 1.
fn salarie(pool: &Pool) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity ORDER BY rowid LIMIT 1",
        params! { "entity" => ENTITY },
    )
    .ok()
    .flatten()
}

/// Un compte bancaire jetable.
///
/// `llx_bank_account` impose `ref`, `label`, `fk_pays` et `currency_code`. Le
/// PID entre dans les deux premières colonnes : deux exécutions concurrentes de
/// la suite ne se heurtent ni sur `ref` ni sur `uk_bank_account_label`.
fn compte_bancaire(pool: &Pool) -> Result<i64, Box<dyn Error>> {
    let pid = std::process::id();
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_bank_account WHERE ref = ?",
        (format!("TS{pid}"),),
    )?;
    conn.exec_drop(
        "INSERT INTO llx_bank_account \
         (datec, ref, label, entity, courant, clos, fk_pays, currency_code, state_id) \
         VALUES (NOW(), ?, ?, ?, 1, 0, 0, 'EUR', 0)",
        (
            format!("TS{pid}"),
            format!("Compte de test salaires {pid}"),
            ENTITY,
        ),
    )?;
    Ok(conn.last_insert_id() as i64)
}

fn oter_compte(pool: &Pool, rowid: i64) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop("DELETE FROM llx_bank_account WHERE rowid = ?", (rowid,));
}

/// Le premier moyen de paiement actif du dictionnaire (2 = VIR sur une base
/// neuve). On ne code pas l'identifiant en dur : le dictionnaire appartient à
/// l'utilisateur, pas au test.
fn moyen_paiement(pool: &Pool) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    conn.exec_first(
        "SELECT id FROM llx_c_paiement WHERE active = 1 ORDER BY id LIMIT 1",
        (),
    )
    .ok()
    .flatten()
}

/// Un salaire de référence : 1 500,00 €, payé le 30 novembre 2026.
fn bulletin(fk_user: i64, montant: &str, datep: &str, datev: &str) -> NouveauSalaire {
    NouveauSalaire {
        fk_user,
        datep: datep.into(),
        datev: datev.into(),
        amount: dec(montant),
        label: "Salaire de novembre 2026".into(),
        fk_project: None,
        paye: false,
    }
}

/// Efface la fiche **et** ses versements : `supprimer` refuse un salaire payé,
/// ce qui est précisément son rôle, donc le nettoyage passe par SQL.
fn effacer(pool: &Pool, rowid: i64) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE FROM llx_payment_salary WHERE fk_salary = ?",
        (rowid,),
    );
    let _ = conn.exec_drop("DELETE FROM llx_salary WHERE rowid = ?", (rowid,));
}

// ---------------------------------------------------------------------------
// Création et relecture
// ---------------------------------------------------------------------------

#[test]
fn cree_un_salaire_et_le_relit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(salarie) = salarie(&pool) else {
        eprintln!("aucun utilisateur dans l'entité 1 : test ignoré");
        return Ok(());
    };

    let rowid = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1500.00", "2026-11-30", "2026-11-30"),
    )?;

    let fiche = salaire::get(&pool, ENTITY, rowid)?.expect("le salaire créé doit se relire");
    assert_eq!(fiche.fk_user, salarie);
    assert_eq!(fiche.amount, dec("1500.00"), "le montant doit revenir exact");
    assert_eq!(fiche.label.as_deref(), Some("Salaire de novembre 2026"));
    assert_eq!(fiche.datep.as_deref(), Some("2026-11-30"));
    assert_eq!(fiche.datev.as_deref(), Some("2026-11-30"));
    assert!(!fiche.paye, "un salaire naît non payé");
    assert_eq!(
        fiche.fk_typepayment, 0,
        "aucun moyen de paiement n'est choisi à l'ouverture"
    );
    assert!(
        fiche.fk_account.is_none() && fiche.fk_bank.is_none(),
        "aucun compte ni ligne bancaire avant le versement"
    );
    assert!(
        fiche.periode_debut.is_none() && fiche.periode_fin.is_none(),
        "la période de paie n'est pas devinée à partir de la date de paiement"
    );

    // La référence est le rowid, écrit dans la colonne pour que la liste ne
    // montre pas du vide (les fiches du noyau, elles, gardent `ref` à NULL).
    assert_eq!(fiche.ref_, rowid.to_string());
    let colonne: Option<String> = {
        let mut conn = pool.get_conn()?;
        conn.exec_first("SELECT ref FROM llx_salary WHERE rowid = ?", (rowid,))?
    };
    assert_eq!(colonne.as_deref(), Some(fiche.ref_.as_str()));

    // Le login accompagne le salaire : sans lui, l'écran n'affiche qu'un numéro.
    let login: Option<String> = {
        let mut conn = pool.get_conn()?;
        conn.exec_first("SELECT login FROM llx_user WHERE rowid = ?", (salarie,))?
    };
    assert_eq!(fiche.login, login);

    // Elle apparaît dans la liste de son entité.
    let liste = salaire::list(&pool, ENTITY, 200, 0)?;
    assert!(
        liste.iter().any(|s| s.rowid == rowid),
        "le salaire créé doit apparaître dans la liste"
    );

    effacer(&pool, rowid);
    Ok(())
}

// ---------------------------------------------------------------------------
// Refus de saisie
// ---------------------------------------------------------------------------

#[test]
fn refuse_un_montant_negatif() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(salarie) = salarie(&pool) else {
        return Ok(());
    };

    let mut new = bulletin(salarie, "-1500.00", "2026-11-30", "2026-11-30");
    new.amount = dec("-1500.00");
    let err = salaire::creer(&pool, ENTITY, &new).unwrap_err();
    assert!(
        err.to_string().contains("négatif"),
        "message inattendu : {err}"
    );

    // Rien ne doit être écrit : pas de fiche à moitié créée.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_salary WHERE entity = ? AND fk_user = ? AND amount < 0",
        (ENTITY, salarie),
    )?;
    assert_eq!(restes.unwrap_or(0), 0);
    Ok(())
}

#[test]
fn refuse_des_dates_inversees() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(salarie) = salarie(&pool) else {
        return Ok(());
    };

    // Valeur au 15, paiement au 30 : impossible.
    let err = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1500.00", "2026-11-30", "2026-11-15"),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("précède"),
        "message inattendu : {err}"
    );

    // Une date qui n'existe pas est refusée avant même la comparaison.
    let err = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1500.00", "2026-02-30", "2026-02-30"),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("n'existe pas"),
        "message inattendu : {err}"
    );
    Ok(())
}

#[test]
fn refuse_un_utilisateur_inexistant() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    // Identifiant volontairement hors d'atteinte : `fk_user` n'a pas de clé
    // étrangère, c'est le module qui doit refuser.
    let err = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(999_999_999, "1500.00", "2026-11-30", "2026-11-30"),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("n'existe pas"),
        "message inattendu : {err}"
    );
    Ok(())
}

#[test]
fn refuse_un_libelle_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(salarie) = salarie(&pool) else {
        return Ok(());
    };

    let mut new = bulletin(salarie, "1500.00", "2026-11-30", "2026-11-30");
    new.label = "   ".into();
    let err = salaire::creer(&pool, ENTITY, &new).unwrap_err();
    assert!(
        err.to_string().contains("libellé"),
        "message inattendu : {err}"
    );
    Ok(())
}

/// Un salaire marqué payé sans ligne de versement serait un trou dans la
/// trésorerie : le module refuse, comme l'écran Dolibarr qui force `paye = 0`.
#[test]
fn refuse_un_salaire_cree_deja_paye() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(salarie) = salarie(&pool) else {
        return Ok(());
    };

    let mut new = bulletin(salarie, "1500.00", "2026-11-30", "2026-11-30");
    new.paye = true;
    let err = salaire::creer(&pool, ENTITY, &new).unwrap_err();
    assert!(
        err.to_string().contains("déjà payé"),
        "message inattendu : {err}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Versement
// ---------------------------------------------------------------------------

#[test]
fn le_versement_pose_le_drapeau_et_le_lien() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(moyen)) = (salarie(&pool), moyen_paiement(&pool)) else {
        eprintln!("ni utilisateur ni moyen de paiement : test ignoré");
        return Ok(());
    };
    let compte = compte_bancaire(&pool)?;

    let rowid = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1500.00", "2026-11-30", "2026-11-30"),
    )?;
    let paiement = salaire::payer(
        &pool,
        ENTITY,
        rowid,
        compte,
        moyen,
        "2026-12-02",
        Some("VIR-77"),
    )?;
    assert!(paiement > 0, "le versement doit avoir un rowid");

    // 1. La trace du versement, montant recopié du salaire.
    let trace = {
        let mut conn = pool.get_conn()?;
        conn.exec_first::<(String, i64, i64, Option<String>, i64, String), _, _>(
            "SELECT CAST(amount AS CHAR), fk_salary, fk_typepayment, num_payment, \
                    fk_bank, DATE_FORMAT(datep, '%Y-%m-%d') \
             FROM llx_payment_salary WHERE rowid = ?",
            (paiement,),
        )?
    };
    let Some((montant, fk_salary, fk_typepayment, num, banque, jour)) = trace else {
        panic!("le versement {paiement} doit exister");
    };
    assert_eq!(dec(montant.trim()), dec("1500.00"), "montant recopié");
    assert_eq!(fk_salary, rowid, "le versement désigne le salaire");
    assert_eq!(fk_typepayment, moyen);
    assert_eq!(num.as_deref(), Some("VIR-77"));
    assert_eq!(
        banque, compte,
        "fk_bank du versement porte le compte bancaire"
    );
    assert_eq!(jour, "2026-12-02", "date réelle du versement");

    // 2. La fiche : drapeau, moyen, compte et numéro, pour l'écran et les totaux.
    let fiche = salaire::get(&pool, ENTITY, rowid)?.expect("la fiche doit se relire");
    assert!(fiche.paye, "le salaire doit être marqué payé");
    assert_eq!(fiche.fk_typepayment, moyen);
    assert_eq!(fiche.type_payment_code.is_some(), true, "code du dictionnaire");
    assert_eq!(fiche.fk_account, Some(compte));
    assert_eq!(fiche.num_payment.as_deref(), Some("VIR-77"));
    assert!(
        fiche.fk_bank.is_none(),
        "aucune ligne llx_bank n'est créée : fk_bank reste vide"
    );
    assert_eq!(
        fiche.datep.as_deref(),
        Some("2026-11-30"),
        "la date déclarée sur la fiche n'est pas écrasée par la date réelle du \
         versement, qui vit dans llx_payment_salary"
    );

    effacer(&pool, rowid);
    oter_compte(&pool, compte);
    Ok(())
}

#[test]
fn refuse_de_repayer_un_salaire() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(moyen)) = (salarie(&pool), moyen_paiement(&pool)) else {
        return Ok(());
    };
    let compte = compte_bancaire(&pool)?;

    let rowid = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1500.00", "2026-11-30", "2026-11-30"),
    )?;
    salaire::payer(&pool, ENTITY, rowid, compte, moyen, "2026-12-02", None)?;

    let err = salaire::payer(&pool, ENTITY, rowid, compte, moyen, "2026-12-03", None).unwrap_err();
    assert!(
        err.to_string().contains("déjà payé"),
        "message inattendu : {err}"
    );

    // Un seul versement, pas deux.
    let nombre: Option<i64> = {
        let mut conn = pool.get_conn()?;
        conn.exec_first(
            "SELECT COUNT(*) FROM llx_payment_salary WHERE fk_salary = ?",
            (rowid,),
        )?
    };
    assert_eq!(nombre.unwrap_or(0), 1, "aucun second versement ne doit rester");

    effacer(&pool, rowid);
    oter_compte(&pool, compte);
    Ok(())
}

#[test]
fn refuse_un_compte_bancaire_inexistant() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(moyen)) = (salarie(&pool), moyen_paiement(&pool)) else {
        return Ok(());
    };

    let rowid = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1500.00", "2026-11-30", "2026-11-30"),
    )?;
    let err =
        salaire::payer(&pool, ENTITY, rowid, 999_999_999, moyen, "2026-12-02", None).unwrap_err();
    assert!(
        err.to_string().contains("compte bancaire"),
        "message inattendu : {err}"
    );

    // Le refus ne doit rien avoir laissé derrière lui : ni versement, ni drapeau.
    let fiche = salaire::get(&pool, ENTITY, rowid)?.expect("la fiche doit survivre");
    assert!(!fiche.paye, "un refus ne marque pas le salaire payé");
    let nombre: Option<i64> = {
        let mut conn = pool.get_conn()?;
        conn.exec_first(
            "SELECT COUNT(*) FROM llx_payment_salary WHERE fk_salary = ?",
            (rowid,),
        )?
    };
    assert_eq!(nombre.unwrap_or(0), 0, "aucun versement ne doit être écrit");

    effacer(&pool, rowid);
    Ok(())
}

#[test]
fn refuse_un_moyen_de_paiement_inexistant() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(salarie) = salarie(&pool) else {
        return Ok(());
    };
    let compte = compte_bancaire(&pool)?;

    let rowid = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1500.00", "2026-11-30", "2026-11-30"),
    )?;
    let err = salaire::payer(&pool, ENTITY, rowid, compte, 999_999, "2026-12-02", None)
        .unwrap_err();
    assert!(
        err.to_string().contains("moyen de paiement"),
        "message inattendu : {err}"
    );

    let fiche = salaire::get(&pool, ENTITY, rowid)?.expect("la fiche doit survivre");
    assert!(!fiche.paye);

    effacer(&pool, rowid);
    oter_compte(&pool, compte);
    Ok(())
}

// ---------------------------------------------------------------------------
// Suppression
// ---------------------------------------------------------------------------

#[test]
fn refuse_de_supprimer_un_salaire_paye() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(moyen)) = (salarie(&pool), moyen_paiement(&pool)) else {
        return Ok(());
    };
    let compte = compte_bancaire(&pool)?;

    let rowid = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1500.00", "2026-11-30", "2026-11-30"),
    )?;
    salaire::payer(&pool, ENTITY, rowid, compte, moyen, "2026-12-02", Some("VIR-78"))?;

    let err = salaire::supprimer(&pool, ENTITY, rowid).unwrap_err();
    assert!(
        err.to_string().contains("payé"),
        "message inattendu : {err}"
    );
    assert!(
        salaire::get(&pool, ENTITY, rowid)?.is_some(),
        "la fiche doit toujours exister après le refus"
    );

    effacer(&pool, rowid);
    oter_compte(&pool, compte);
    Ok(())
}

// ---------------------------------------------------------------------------
// Totaux et références
// ---------------------------------------------------------------------------

/// L'exercice 1990 ne peut contenir aucune autre ligne : le total y est
/// déterministe, même sur une base déjà remplie de vrais bulletins.
#[test]
fn total_par_annee_isole_les_exercices() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(salarie) = salarie(&pool) else {
        return Ok(());
    };

    let avant = salaire::total_par_annee(&pool, ENTITY, 1990)?;
    assert_eq!(avant, dec("0"), "l'exercice de test doit être vide");

    let a = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1500.55", "1990-03-15", "1990-03-15"),
    )?;
    let b = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "2449.45", "1990-04-15", "1990-04-15"),
    )?;

    assert_eq!(
        salaire::total_par_annee(&pool, ENTITY, 1990)?,
        dec("3950.00"),
        "le total somme les montants enregistrés de l'année"
    );
    // Les montants en centimes ne doivent pas dériver : 1500,55 + 2449,45 fait
    // exactement 3950,00, ce qu'un flottant ne garantit pas.
    assert_eq!(
        salaire::total_par_annee(&pool, ENTITY, 1991)?,
        dec("0"),
        "un autre exercice reste à zéro"
    );

    effacer(&pool, a);
    effacer(&pool, b);
    Ok(())
}

/// Deux salaires créés à la suite ne peuvent pas se confondre : la référence
/// est le `rowid`, et l'index unique `uk_salary_ref (ref, entity)` la tient.
#[test]
fn la_reference_ne_se_repete_pas() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(salarie) = salarie(&pool) else {
        return Ok(());
    };

    let premier = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1000.00", "2026-11-30", "2026-11-30"),
    )?;
    let second = salaire::creer(
        &pool,
        ENTITY,
        &bulletin(salarie, "1000.00", "2026-11-30", "2026-11-30"),
    )?;

    let a = salaire::get(&pool, ENTITY, premier)?.expect("premier salaire");
    let b = salaire::get(&pool, ENTITY, second)?.expect("second salaire");
    assert_ne!(a.ref_, b.ref_, "deux salaires, deux références");
    assert_eq!(a.ref_, premier.to_string());
    assert_eq!(b.ref_, second.to_string());

    let nombre: Option<i64> = {
        let mut conn = pool.get_conn()?;
        conn.exec_first(
            "SELECT COUNT(DISTINCT ref) FROM llx_salary WHERE entity = ? AND rowid IN (?, ?)",
            (ENTITY, premier, second),
        )?
    };
    assert_eq!(nombre.unwrap_or(0), 2, "aucune référence partagée en base");

    effacer(&pool, premier);
    effacer(&pool, second);
    Ok(())
}
