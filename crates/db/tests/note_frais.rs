use std::error::Error;
use std::str::FromStr;

use db::note_frais::{
    self, NewLigneFrais, NewNoteFrais, STATUT_APPROUVEE, STATUT_ATTENTE_VALIDATION,
    STATUT_BROUILLON, STATUT_REFUSEE,
};
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
            eprintln!("connexion impossible ({e}) : test ignore");
            None
        }
    }
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// Un auteur réel de l'entité 1 : `llx_expensereport.fk_user_author` n'a pas de
/// clé étrangère, on ne veut donc pas d'un identifiant inventé.
fn auteur(pool: &Pool) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity ORDER BY rowid LIMIT 1",
        params! { "entity" => ENTITY },
    )
    .ok()
    .flatten()
}

/// Un type de frais actif du dictionnaire national.
fn type_frais(pool: &Pool) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    conn.exec_first(
        "SELECT id FROM llx_c_type_fees WHERE active = 1 ORDER BY id LIMIT 1",
        (),
    )
    .ok()
    .flatten()
}

fn ligne(fk_c_type_fees: i64, qty: &str, subprice: &str, tva_tx: &str) -> NewLigneFrais {
    NewLigneFrais {
        fk_c_type_fees,
        date: "2026-11-02".into(),
        comments: "Déplacement chantier".into(),
        qty: dec(qty),
        subprice: dec(subprice),
        tva_tx: dec(tva_tx),
    }
}

fn note(auteur: i64, type_frais: i64) -> NewNoteFrais {
    NewNoteFrais {
        fk_user_author: auteur,
        date_debut: "2026-11-01".into(),
        date_fin: "2026-11-03".into(),
        note_public: Some("Note de frais de test".into()),
        note_private: None,
        lines: vec![ligne(type_frais, "1", "45.00", "10")],
    }
}

fn effacer(pool: &Pool, rowid: i64) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE FROM llx_expensereport_det WHERE fk_expensereport = ?",
        (rowid,),
    );
    let _ = conn.exec_drop("DELETE FROM llx_expensereport WHERE rowid = ?", (rowid,));
}

/// Le cas de référence, posé à la main : 2 × 45,00 à 10 % font 90,00 HT et
/// 99,00 TTC (90 × 1,10) ; 1 × 120,00 à 20 % font 120,00 HT et 144,00 TTC.
/// L'en-tête porte donc 210,00 / 33,00 / 243,00.
#[test]
fn cree_une_note_et_calcule_les_totaux() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(auteur), Some(type_frais)) = (auteur(&pool), type_frais(&pool)) else {
        eprintln!("ni utilisateur ni type de frais en base : test ignore");
        return Ok(());
    };

    let mut new = note(auteur, type_frais);
    new.lines = vec![
        ligne(type_frais, "2", "45.00", "10"),
        ligne(type_frais, "1", "120.00", "20"),
    ];
    let rowid = note_frais::create(&pool, ENTITY, &new)?;

    let f = note_frais::get(&pool, ENTITY, rowid)?.expect("la note créée doit se relire");
    assert_eq!(f.statut, STATUT_BROUILLON, "une note naît en brouillon");
    assert_eq!(f.fk_user_author, auteur);
    assert_eq!(f.date_debut, "2026-11-01");
    assert_eq!(f.date_fin, "2026-11-03");
    assert_eq!(f.note_public.as_deref(), Some("Note de frais de test"));
    assert_eq!(f.total_ht, dec("210.00"));
    assert_eq!(f.total_tva, dec("33.00"));
    assert_eq!(f.total_ttc, dec("243.00"));
    assert!(f.date_valid.is_none() && f.date_approve.is_none());
    assert!(
        f.ref_.starts_with("NF") && f.ref_[2..].chars().all(|c| c.is_ascii_digit()),
        "référence inattendue : {}",
        f.ref_
    );

    let login: Option<String> = {
        let mut conn = pool.get_conn()?;
        conn.exec_first(
            "SELECT login FROM llx_user WHERE rowid = :rowid",
            params! { "rowid" => auteur },
        )?
    };
    assert_eq!(f.auteur, login, "l'auteur doit sortir avec son login");

    let lignes = note_frais::lines(&pool, ENTITY, rowid)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(lignes[0].rang, 1);
    assert_eq!(lignes[0].qty, dec("2"));
    assert_eq!(lignes[0].total_ht, dec("90.00"));
    assert_eq!(lignes[0].total_tva, dec("9.00"));
    assert_eq!(lignes[0].total_ttc, dec("99.00"));
    assert_eq!(lignes[1].total_ttc, dec("144.00"));
    assert_eq!(lignes[1].rang, 2);

    // Le brouillon se supprime : c'est le seul statut qui l'autorise.
    note_frais::supprimer(&pool, ENTITY, rowid)?;
    assert!(
        note_frais::get(&pool, ENTITY, rowid)?.is_none(),
        "après suppression, la note ne doit plus se lire"
    );
    assert!(note_frais::lines(&pool, ENTITY, rowid)?.is_empty());
    Ok(())
}

#[test]
fn refuse_une_note_sans_ligne() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(auteur), Some(type_frais)) = (auteur(&pool), type_frais(&pool)) else {
        return Ok(());
    };
    let mut new = note(auteur, type_frais);
    new.lines.clear();
    let erreur = note_frais::create(&pool, ENTITY, &new).expect_err("zéro ligne doit être refusé");
    assert!(
        erreur.to_string().contains("au moins une ligne"),
        "{erreur}"
    );
    Ok(())
}

#[test]
fn refuse_des_dates_inversees() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(auteur), Some(type_frais)) = (auteur(&pool), type_frais(&pool)) else {
        return Ok(());
    };
    let mut new = note(auteur, type_frais);
    new.date_debut = "2026-11-05".into();
    new.date_fin = "2026-11-01".into();
    let erreur = note_frais::create(&pool, ENTITY, &new).expect_err("période inversée refusée");
    assert!(erreur.to_string().contains("précéder"), "{erreur}");
    Ok(())
}

#[test]
fn refuse_un_type_de_frais_inconnu() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(auteur) = auteur(&pool) else {
        return Ok(());
    };
    let new = note(auteur, 9_999_999);
    let erreur = note_frais::create(&pool, ENTITY, &new).expect_err("type inconnu refusé");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");
    Ok(())
}

/// Pose un type de frais désactivé, le temps du test, dans le dictionnaire
/// national (la table n'a pas d'entité).
fn poser_type_desactive(pool: &Pool, code: &str) -> Result<Option<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let _ = conn.exec_drop("DELETE FROM llx_c_type_fees WHERE code = ?", (code,));
    conn.exec_drop(
        "INSERT INTO llx_c_type_fees (code, label, type, active, position) \
         VALUES (?, 'Type de test désactivé', 0, 0, 0)",
        (code,),
    )?;
    let id: Option<i64> =
        conn.exec_first("SELECT id FROM llx_c_type_fees WHERE code = ?", (code,))?;
    Ok(id)
}

fn oter_type_desactive(pool: &Pool, code: &str) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop("DELETE FROM llx_c_type_fees WHERE code = ?", (code,));
}

#[test]
fn refuse_un_type_de_frais_desactive() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(auteur) = auteur(&pool) else {
        return Ok(());
    };

    let code = format!("ZZNF{}", std::process::id());
    let Some(desactive) = poser_type_desactive(&pool, &code)? else {
        // Le dictionnaire n'a pas gardé la ligne : rien à tester.
        oter_type_desactive(&pool, &code);
        return Ok(());
    };

    let erreur = note_frais::create(&pool, ENTITY, &note(auteur, desactive))
        .expect_err("un type désactivé doit être refusé");
    oter_type_desactive(&pool, &code);
    assert!(erreur.to_string().contains("désactivé"), "{erreur}");
    Ok(())
}

#[test]
fn soumission_puis_resoumission_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(auteur), Some(type_frais)) = (auteur(&pool), type_frais(&pool)) else {
        return Ok(());
    };
    let rowid = note_frais::create(&pool, ENTITY, &note(auteur, type_frais))?;

    note_frais::soumettre(&pool, ENTITY, rowid)?;
    let f = note_frais::get(&pool, ENTITY, rowid)?.expect("note");
    assert_eq!(f.statut, STATUT_ATTENTE_VALIDATION);
    assert!(f.date_valid.is_some(), "la soumission date la validation");

    let erreur = note_frais::soumettre(&pool, ENTITY, rowid).expect_err("resoumission refusée");
    assert!(erreur.to_string().contains("déjà"), "{erreur}");

    effacer(&pool, rowid);
    Ok(())
}

#[test]
fn approbation_refusee_si_non_soumise() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(auteur), Some(type_frais)) = (auteur(&pool), type_frais(&pool)) else {
        return Ok(());
    };
    let rowid = note_frais::create(&pool, ENTITY, &note(auteur, type_frais))?;

    let erreur =
        note_frais::approuver(&pool, ENTITY, rowid).expect_err("brouillon non approuvable");
    assert!(erreur.to_string().contains("soumise"), "{erreur}");

    note_frais::soumettre(&pool, ENTITY, rowid)?;
    note_frais::approuver(&pool, ENTITY, rowid)?;
    let f = note_frais::get(&pool, ENTITY, rowid)?.expect("note");
    assert_eq!(f.statut, STATUT_APPROUVEE);
    assert!(f.date_approve.is_some());

    effacer(&pool, rowid);
    Ok(())
}

#[test]
fn refuse_une_note_avec_motif() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(auteur), Some(type_frais)) = (auteur(&pool), type_frais(&pool)) else {
        return Ok(());
    };
    let rowid = note_frais::create(&pool, ENTITY, &note(auteur, type_frais))?;

    // Un refus sur un brouillon n'aurait pas de sens : rien n'a été soumis.
    assert!(note_frais::refuser(&pool, ENTITY, rowid, "hors sujet").is_err());

    note_frais::soumettre(&pool, ENTITY, rowid)?;
    note_frais::refuser(&pool, ENTITY, rowid, "  Justificatifs manquants  ")?;
    let f = note_frais::get(&pool, ENTITY, rowid)?.expect("note");
    assert_eq!(f.statut, STATUT_REFUSEE);
    assert!(f.date_refuse.is_some());
    assert_eq!(
        f.detail_refuse.as_deref(),
        Some("Justificatifs manquants"),
        "le motif est conservé, débarrassé de ses espaces"
    );

    effacer(&pool, rowid);
    Ok(())
}

/// Un motif de 400 octets (200 « é ») doit tenir dans le `varchar(255)` sans
/// qu'un caractère soit coupé en deux : 127 « é » font 254 octets.
#[test]
fn borne_le_motif_a_255_octets() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(auteur), Some(type_frais)) = (auteur(&pool), type_frais(&pool)) else {
        return Ok(());
    };
    let rowid = note_frais::create(&pool, ENTITY, &note(auteur, type_frais))?;
    note_frais::soumettre(&pool, ENTITY, rowid)?;

    let motif = "é".repeat(200);
    note_frais::refuser(&pool, ENTITY, rowid, &motif)?;
    let f = note_frais::get(&pool, ENTITY, rowid)?.expect("note");
    let garde = f.detail_refuse.expect("motif conservé");
    assert!(garde.len() <= 255, "{} octets écrits", garde.len());
    assert_eq!(garde.chars().count(), 127);
    assert!(motif.starts_with(&garde));

    effacer(&pool, rowid);
    Ok(())
}

#[test]
fn suppression_refusee_si_soumise() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(auteur), Some(type_frais)) = (auteur(&pool), type_frais(&pool)) else {
        return Ok(());
    };
    let rowid = note_frais::create(&pool, ENTITY, &note(auteur, type_frais))?;
    note_frais::soumettre(&pool, ENTITY, rowid)?;

    let erreur = note_frais::supprimer(&pool, ENTITY, rowid).expect_err("soumise non supprimable");
    assert!(erreur.to_string().contains("brouillon"), "{erreur}");
    assert!(
        note_frais::get(&pool, ENTITY, rowid)?.is_some(),
        "la note doit être intacte"
    );

    effacer(&pool, rowid);
    Ok(())
}

#[test]
fn numerote_sans_doublon() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(auteur), Some(type_frais)) = (auteur(&pool), type_frais(&pool)) else {
        return Ok(());
    };
    let premier = note_frais::create(&pool, ENTITY, &note(auteur, type_frais))?;
    let second = note_frais::create(&pool, ENTITY, &note(auteur, type_frais))?;

    let a = note_frais::get(&pool, ENTITY, premier)?.expect("première note");
    let b = note_frais::get(&pool, ENTITY, second)?.expect("seconde note");
    assert_ne!(a.ref_, b.ref_, "deux notes ne partagent pas une référence");
    for r in [&a.ref_, &b.ref_] {
        assert!(
            r.starts_with("NF") && r[2..].chars().all(|c| c.is_ascii_digit()),
            "référence hors format : {r}"
        );
    }

    let liste = note_frais::list(&pool, ENTITY, 100_000, 0)?;
    assert!(liste.iter().any(|n| n.rowid == premier));
    assert!(liste.iter().any(|n| n.rowid == second));

    effacer(&pool, premier);
    effacer(&pool, second);
    Ok(())
}
