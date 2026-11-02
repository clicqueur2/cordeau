//! Achats : fournisseurs (creation, code, relecture) et total des achats.
//!
//! Les tests s'executent en parallele sur la meme base : les assertions portent
//! sur les fiches creees (identifiants connus), jamais sur un `COUNT(*)` global.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

const ENTITY: i64 = 1;

/// Prefixe commun a toutes les fiches de test : le nettoyage s'appuie dessus.
const PREFIXE: &str = "ZZ Fourn Test";

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test d'integration ignore");
            None
        }
    }
}

/// Supprime les restes d'un test precedent interrompu, pour que la suite soit
/// rejouable sans intervention manuelle.
fn nettoyer(pool: &Pool, suffixe: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_societe WHERE entity = :entity AND nom LIKE :motif",
        params! { "entity" => ENTITY, "motif" => format!("{PREFIXE} {suffixe}%") },
    )?;
    Ok(())
}

/// `CU` + `AAMM` + `-` + 5 chiffres, soit 12 caracteres.
fn format_code_valide(code: &str) -> bool {
    code.len() == 12
        && code.starts_with("CU")
        && code.as_bytes()[6] == b'-'
        && code[7..].bytes().all(|b| b.is_ascii_digit())
}

#[test]
fn cree_un_fournisseur_et_le_relit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Alpha")?;

    let nom = format!("{PREFIXE} Alpha");
    let id = db::achat::creer_fournisseur(&pool, ENTITY, &nom, None)?;
    assert!(id > 0);

    let lu = db::achat::fournisseur_par_id(&pool, ENTITY, id)?.expect("fournisseur relu");
    assert_eq!(lu.nom, nom);
    assert_eq!(lu.rowid, id);
    assert_eq!(lu.statut, 1);
    assert!(lu.est_actif());

    let code = lu
        .code_fournisseur
        .clone()
        .expect("code fournisseur genere");
    assert!(
        format_code_valide(&code),
        "format de code fournisseur inattendu: {code}"
    );

    let listes = db::achat::fournisseurs(&pool, ENTITY, 1000, 0)?;
    assert!(
        listes.iter().any(|f| f.rowid == id && f.nom == nom),
        "le fournisseur cree doit apparaitre dans fournisseurs()"
    );

    // La fiche est un fournisseur pur : elle n'est pas un client.
    let tiers = db::party::get(&pool, ENTITY, id)?.expect("la fiche tiers existe");
    assert_eq!(tiers.fournisseur, 1);
    assert_eq!(tiers.client, 0);

    // Le filtrage par entite est bien applique.
    assert!(
        db::achat::fournisseur_par_id(&pool, 2, id)?.is_none(),
        "un fournisseur ne doit pas etre visible depuis une autre entite"
    );

    nettoyer(&pool, "Alpha")?;
    assert!(db::achat::fournisseur_par_id(&pool, ENTITY, id)?.is_none());
    Ok(())
}

#[test]
fn les_codes_fournisseurs_sont_uniques_et_bien_formes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    // Les codes imposes et le doublon porte des noms distincts : on les
    // nettoie aussi, sinon un passage interrompu bloquerait la suite.
    nettoyer(&pool, "Code")?;
    nettoyer(&pool, "Impose")?;
    nettoyer(&pool, "Doublon")?;

    let mut codes = Vec::new();
    let mut ids = Vec::new();
    for i in 0..3 {
        let nom = format!("{PREFIXE} Code {i}");
        let id = db::achat::creer_fournisseur(&pool, ENTITY, &nom, None)?;
        let lu = db::achat::fournisseur_par_id(&pool, ENTITY, id)?.expect("fournisseur relu");
        let code = lu.code_fournisseur.expect("code genere");
        assert!(
            format_code_valide(&code),
            "format de code fournisseur inattendu a l'iteration {i}: {code}"
        );
        codes.push(code);
        ids.push(id);
    }

    let mut uniques = codes.clone();
    uniques.sort();
    uniques.dedup();
    assert_eq!(
        uniques.len(),
        codes.len(),
        "les codes fournisseurs doivent etre distincts: {codes:?}"
    );

    // Un code impose est repris tel quel, et un doublon est refuse.
    let impose = "ZZFOURN-IMPOSE-1";
    let id =
        db::achat::creer_fournisseur(&pool, ENTITY, &format!("{PREFIXE} Impose"), Some(impose))?;
    let lu = db::achat::fournisseur_par_id(&pool, ENTITY, id)?.expect("fournisseur a code impose");
    assert_eq!(lu.code_fournisseur.as_deref(), Some(impose));
    assert!(
        db::achat::creer_fournisseur(&pool, ENTITY, &format!("{PREFIXE} Doublon"), Some(impose))
            .is_err(),
        "deux fournisseurs ne peuvent pas partager le meme code"
    );

    // Un nom vide est refuse avant toute ecriture.
    assert!(db::achat::creer_fournisseur(&pool, ENTITY, "   ", None).is_err());

    // Les codes imposes et le doublon porte des noms distincts : on les
    // nettoie aussi, sinon un passage interrompu bloquerait la suite.
    nettoyer(&pool, "Code")?;
    nettoyer(&pool, "Impose")?;
    nettoyer(&pool, "Doublon")?;
    Ok(())
}

#[test]
fn total_achats_renvoie_un_montant_meme_sans_facture() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    let total = db::achat::total_achats(&pool, ENTITY)?;
    assert!(
        total >= Decimal::ZERO,
        "le total des achats ne peut pas etre negatif: {total}"
    );

    // Un total se lit aussi sur une entite sans aucune facture fournisseur.
    let vide = db::achat::total_achats(&pool, 999_999)?;
    assert_eq!(vide, Decimal::ZERO, "une entite sans achat totalise zero");

    Ok(())
}

#[test]
fn liste_des_factures_fournisseurs_lisible() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    let factures = db::achat::factures_fournisseur(&pool, ENTITY, 50, 0)?;
    for f in &factures {
        assert!(
            !f.ref_.is_empty(),
            "une facture fournisseur a une reference"
        );
        if f.est_brouillon() {
            assert_eq!(f.fk_statut, db::achat::STATUT_BROUILLON);
        }
    }
    Ok(())
}
