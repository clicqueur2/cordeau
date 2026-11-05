//! Marges d'affaire : création, relecture, lignes, validation, suppression,
//! totaux. Les tables `llx_marge_*` sont créées par le module lui-même
//! (`db::marge::preparer`) : voir la doc de `crates/db/src/marge.rs` sur ce que
//! Dolibarr 24.0.1 contient réellement en matière de marges.
//!
//! Les tests touchent la base : ils ne tournent que si `DOLIBARR_DSN` est posée,
//! et s'ignorent franchement sinon — un test qui échoue faute de base masquerait
//! une vraie régression.
//!
//! Chaque test porte son propre libellé (préfixé `ZZ `) et fait le ménage avant
//! **et** après : un test interrompu laisserait sinon des restes qui feraient
//! échouer le suivant. Le ménage supprime les lignes d'abord, l'en-tête ensuite,
//! comme le fait `supprimer` — le schéma ne pose pas de clé étrangère, un reste
//! de ligne ne serait jamais signalé.
//!
//! Les calculs purs (marge d'une ligne, taux, taux global pondéré, division par
//! zéro) sont testés dans `crates/db/src/marge.rs`, sans base : ils n'ont pas
//! besoin d'un serveur pour être vrais.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::marge::{
    ajouter_ligne, creer, get, lignes, list, marge_ht, preparer, supprimer, taux_marge, totaux,
    valider, NouvelleLigneMarge, NouvelleMarge, STATUT_BROUILLON, STATUT_VALIDE,
};

const ENTITY: i64 = 1;
/// Une autre société, pour vérifier le cloisonnement. `llx_marge_ligne` n'a pas
/// d'`entity` : c'est l'en-tête qui en porte une, et la jointure qui l'applique.
const ENTITY_AUTRE: i64 = 2;

/// Un identifiant d'utilisateur quelconque, mais strictement positif.
///
/// Aucune clé étrangère ne lie `fk_user_author` à `llx_user` : le module ne
/// contrôle que la positivité, et c'est tout ce qu'on teste ici.
const AUTEUR: i64 = 1;

/// Un identifiant de tiers quelconque, mais strictement positif. Même remarque :
/// le module vérifie la positivité, pas l'existence.
const TIERS: i64 = 1;

/// Ouvre une connexion et s'assure que les tables du module existent.
///
/// `preparer` est idempotent ; s'il échoue (utilisateur sans droit `CREATE`), on
/// ignore le test avec un message, plutôt que d'échouer sur une cause qui n'est
/// pas une régression du code.
fn demarrer() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test ignoré");
            return None;
        }
    };
    match preparer(&pool) {
        Ok(()) => Some(pool),
        Err(e) => {
            eprintln!("création des tables de marge impossible ({e}) : test ignoré");
            None
        }
    }
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// Une marge minimale valide : un libellé, un tiers, un auteur.
fn nouvelle(libelle: &str) -> NouvelleMarge {
    NouvelleMarge {
        libelle: libelle.to_string(),
        fk_soc: Some(TIERS),
        fk_user_author: AUTEUR,
        ..Default::default()
    }
}

/// Une ligne minimale valide : une désignation libre, quantité 1, deux prix.
fn ligne(qte: &str, pv: &str, pa: &str) -> NouvelleLigneMarge {
    NouvelleLigneMarge {
        designation: "Fourniture et pose".to_string(),
        quantite: dec(qte),
        prix_vente_ht: dec(pv),
        prix_achat_ht: dec(pa),
        ..Default::default()
    }
}

/// Supprime les lignes puis les marges portant ce libellé dans cette société.
fn nettoyer(pool: &Pool, libelle: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE l FROM llx_marge_ligne l \
         INNER JOIN llx_marge_affaire a ON a.rowid = l.fk_marge \
         WHERE a.entity = :entity AND a.libelle = :libelle",
        params! { "entity" => ENTITY, "libelle" => libelle },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_marge_affaire WHERE entity = :entity AND libelle = :libelle",
        params! { "entity" => ENTITY, "libelle" => libelle },
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Création et relecture
// ---------------------------------------------------------------------------

#[test]
fn cree_une_marge_et_la_relit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge chantier Calade";
    nettoyer(&pool, libelle)?;

    let mut new = nouvelle(libelle);
    new.ref_ = Some("AFF-2026-011".to_string());
    new.date_marge = Some("2026-11-05".to_string());

    let id = creer(&pool, ENTITY, &new)?;
    assert!(id > 0);

    let m = get(&pool, ENTITY, id)?.expect("marge relue");
    assert_eq!(m.rowid, id);
    assert_eq!(m.entity, ENTITY);
    assert_eq!(m.libelle, libelle);
    assert_eq!(m.ref_.as_deref(), Some("AFF-2026-011"));
    assert_eq!(m.fk_soc, Some(TIERS));
    assert_eq!(m.fk_projet, None);
    assert_eq!(
        m.date_marge.as_deref(),
        Some("2026-11-05"),
        "la date de l'affaire est celle qui a été saisie"
    );
    assert_eq!(
        m.statut, STATUT_BROUILLON,
        "une marge naît en brouillon, jamais validée"
    );
    assert!(m.datec.is_some(), "la date de création est posée par NOW()");
    assert_eq!(
        m.fk_user_valid, None,
        "créer n'est pas valider : l'auteur de validation reste vide"
    );
    assert_eq!(m.date_valid, None);

    let liste = list(&pool, ENTITY, 50, 0)?;
    assert!(
        liste.iter().any(|x| x.rowid == id),
        "la marge créée doit apparaître dans la liste de son entité"
    );

    nettoyer(&pool, libelle)?;
    assert!(
        get(&pool, ENTITY, id)?.is_none(),
        "après nettoyage, la marge ne doit plus exister"
    );
    Ok(())
}

/// Une marge sans tiers ni projet ne mesure rien : elle est refusée avant
/// écriture.
#[test]
fn refuse_une_marge_sans_tiers_ni_projet() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge orpheline";
    nettoyer(&pool, libelle)?;

    let mut new = nouvelle(libelle);
    new.fk_soc = None;

    let e = creer(&pool, ENTITY, &new).expect_err("une marge sans rattachement doit être refusée");
    assert!(
        e.to_string().contains("tiers"),
        "le message doit nommer le rattachement manquant, reçu : {e}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Lignes : refus des saisies qui n'ont pas de sens
// ---------------------------------------------------------------------------

#[test]
fn refuse_une_ligne_de_quantite_nulle() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge quantite nulle";
    nettoyer(&pool, libelle)?;
    let id = creer(&pool, ENTITY, &nouvelle(libelle))?;

    let e = ajouter_ligne(&pool, ENTITY, id, &ligne("0", "100", "60"))
        .expect_err("une quantité nulle doit être refusée");
    assert!(
        e.to_string().contains("nulle"),
        "le message doit dire que la quantité est nulle, reçu : {e}"
    );
    assert!(
        lignes(&pool, ENTITY, id)?.is_empty(),
        "aucune ligne ne doit avoir été écrite"
    );

    nettoyer(&pool, libelle)?;
    Ok(())
}

#[test]
fn refuse_un_prix_de_vente_negatif() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge prix negatif";
    nettoyer(&pool, libelle)?;
    let id = creer(&pool, ENTITY, &nouvelle(libelle))?;

    let e = ajouter_ligne(&pool, ENTITY, id, &ligne("1", "-10", "60"))
        .expect_err("un prix de vente négatif doit être refusé");
    assert!(
        e.to_string().contains("négatif"),
        "le message doit dire que le prix est négatif, reçu : {e}"
    );
    assert!(lignes(&pool, ENTITY, id)?.is_empty());

    nettoyer(&pool, libelle)?;
    Ok(())
}

/// Une marge validée est figée : elle n'accepte plus de ligne, même juste.
#[test]
fn refuse_d_ajouter_une_ligne_a_une_marge_validee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge figee";
    nettoyer(&pool, libelle)?;
    let id = creer(&pool, ENTITY, &nouvelle(libelle))?;
    ajouter_ligne(&pool, ENTITY, id, &ligne("1", "100", "60"))?;
    valider(&pool, ENTITY, id)?;

    let e = ajouter_ligne(&pool, ENTITY, id, &ligne("1", "50", "20"))
        .expect_err("une marge validée ne reçoit plus de ligne");
    assert!(
        e.to_string().contains("figée"),
        "le message doit dire que la marge est figée, reçu : {e}"
    );
    assert_eq!(
        lignes(&pool, ENTITY, id)?.len(),
        1,
        "la ligne refusée ne doit pas avoir été écrite"
    );

    nettoyer(&pool, libelle)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cycle de vie : validation et suppression
// ---------------------------------------------------------------------------

#[test]
fn valide_puis_refuse_de_revalider() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge a valider";
    nettoyer(&pool, libelle)?;
    let id = creer(&pool, ENTITY, &nouvelle(libelle))?;
    ajouter_ligne(&pool, ENTITY, id, &ligne("1", "100", "60"))?;

    valider(&pool, ENTITY, id)?;
    let m = get(&pool, ENTITY, id)?.expect("marge validée relue");
    assert_eq!(m.statut, STATUT_VALIDE);
    assert!(
        m.date_valid.is_some(),
        "valider date la validation : c'est cette date qui fait foi six mois plus tard"
    );
    assert_eq!(
        m.fk_user_valid, None,
        "le validateur n'est pas inventé : l'auteur de la saisie n'est pas forcément \
         celui qui valide, et écrire l'un pour l'autre serait une fausse attribution"
    );

    let e = valider(&pool, ENTITY, id).expect_err("une marge validée ne se revalide pas");
    assert!(
        e.to_string().contains("déjà"),
        "le message doit dire qu'elle est déjà validée, reçu : {e}"
    );
    assert_eq!(
        get(&pool, ENTITY, id)?.map(|x| x.statut),
        Some(STATUT_VALIDE),
        "le refus ne doit pas avoir changé le statut"
    );

    nettoyer(&pool, libelle)?;
    Ok(())
}

#[test]
fn refuse_de_valider_une_marge_sans_ligne() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge vide";
    nettoyer(&pool, libelle)?;
    let id = creer(&pool, ENTITY, &nouvelle(libelle))?;

    let e = valider(&pool, ENTITY, id).expect_err("une marge sans ligne ne se valide pas");
    assert!(
        e.to_string().contains("sans ligne"),
        "le message doit dire qu'il n'y a pas de ligne, reçu : {e}"
    );
    assert_eq!(
        get(&pool, ENTITY, id)?.map(|x| x.statut),
        Some(STATUT_BROUILLON)
    );

    nettoyer(&pool, libelle)?;
    Ok(())
}

#[test]
fn refuse_de_supprimer_une_marge_validee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge non supprimable";
    nettoyer(&pool, libelle)?;
    let id = creer(&pool, ENTITY, &nouvelle(libelle))?;
    ajouter_ligne(&pool, ENTITY, id, &ligne("1", "100", "60"))?;
    valider(&pool, ENTITY, id)?;

    let e = supprimer(&pool, ENTITY, id).expect_err("une marge validée ne se supprime pas");
    assert!(
        e.to_string().contains("validée"),
        "le message doit dire qu'elle est validée, reçu : {e}"
    );
    assert!(
        get(&pool, ENTITY, id)?.is_some(),
        "le refus ne doit rien avoir effacé, ni l'en-tête ni les lignes"
    );
    assert_eq!(lignes(&pool, ENTITY, id)?.len(), 1);

    nettoyer(&pool, libelle)?;
    Ok(())
}

/// Un brouillon, lui, part avec ses lignes.
#[test]
fn supprime_un_brouillon_et_ses_lignes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge jetable";
    nettoyer(&pool, libelle)?;
    let id = creer(&pool, ENTITY, &nouvelle(libelle))?;
    ajouter_ligne(&pool, ENTITY, id, &ligne("2", "100", "60"))?;

    supprimer(&pool, ENTITY, id)?;
    assert!(get(&pool, ENTITY, id)?.is_none());
    assert!(
        lignes(&pool, ENTITY, id).is_err(),
        "les lignes d'une marge supprimée ne doivent pas survivre : la marge n'existe plus"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Totaux
// ---------------------------------------------------------------------------

#[test]
fn calcule_les_totaux_en_base() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge totaux";
    nettoyer(&pool, libelle)?;
    let id = creer(&pool, ENTITY, &nouvelle(libelle))?;

    // Deux poids très différents, exprès : 10 € de vente à 20 % de marque, puis
    // 10 000 € à 30 %. La moyenne des taux vaut 25 %, le taux réel 29,99 %.
    ajouter_ligne(&pool, ENTITY, id, &ligne("1", "10", "8"))?;
    ajouter_ligne(&pool, ENTITY, id, &ligne("1", "10000", "7000"))?;

    let lues = lignes(&pool, ENTITY, id)?;
    assert_eq!(lues.len(), 2);
    assert_eq!(lues[0].rang, 1, "la première saisie porte le rang 1");
    assert_eq!(lues[1].rang, 2, "la seconde suit, dans l'ordre de saisie");
    assert_eq!(marge_ht(&lues[0]), dec("2.00"));
    assert_eq!(marge_ht(&lues[1]), dec("3000.00"));

    let (vente, achat, taux) = totaux(&pool, ENTITY, id)?;
    assert_eq!(vente, dec("10010.00"));
    assert_eq!(achat, dec("7008.00"));
    assert_eq!(
        taux,
        Some(dec("29.99")),
        "le taux global se calcule sur les totaux : 3 002 de marge sur 10 010 de ventes"
    );
    assert_eq!(vente - achat, dec("3002.00"));

    // La preuve que ce n'est pas une moyenne : les deux taux de ligne valent
    // 20 % et 30 %, dont la moyenne est 25 %.
    let moyenne = (taux_marge(&lues[0]).unwrap() + taux_marge(&lues[1]).unwrap()) / dec("2");
    assert_eq!(moyenne, dec("25.00"));
    assert_ne!(
        taux,
        Some(moyenne),
        "moyenner les taux donnerait un chiffre qu'aucune des deux affaires ne produit"
    );

    nettoyer(&pool, libelle)?;
    Ok(())
}

#[test]
fn totaux_d_une_marge_inconnue_est_une_erreur() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    // Une marge absente ne rend pas des zéros : un total nul se confondrait avec
    // une affaire dont tout a été offert.
    assert!(totaux(&pool, ENTITY, 999_999_999).is_err());
    Ok(())
}

// ---------------------------------------------------------------------------
// Cloisonnement
// ---------------------------------------------------------------------------

#[test]
fn cloisonne_les_marges_par_entite() -> Result<(), Box<dyn Error>> {
    let Some(pool) = demarrer() else {
        return Ok(());
    };
    let libelle = "ZZ marge cloisonnee";
    nettoyer(&pool, libelle)?;
    let id = creer(&pool, ENTITY, &nouvelle(libelle))?;
    ajouter_ligne(&pool, ENTITY, id, &ligne("1", "100", "60"))?;

    assert!(
        get(&pool, ENTITY_AUTRE, id)?.is_none(),
        "une marge de la société 1 ne se lit pas depuis la société 2"
    );
    assert!(
        lignes(&pool, ENTITY_AUTRE, id).is_err(),
        "les lignes suivent la même règle : `llx_marge_ligne` n'a pas d'entity, \
         c'est la jointure sur l'en-tête qui cloisonne"
    );

    nettoyer(&pool, libelle)?;
    Ok(())
}
