//! Postes de charge : création, cycle de vie, coût horaire, machines et groupes,
//! sur la vraie base.
//!
//! Ces tests tournent **uniquement** si `DOLIBARR_DSN` est posée : sans base, ils
//! s'ignorent en le disant. Un test qui échoue faute de base masquerait une vraie
//! régression ; un test qui passe sans rien tester serait pire encore.
//!
//! # Pourquoi ce `#[path]`
//!
//! La consigne de cette intervention interdit de modifier un fichier existant :
//! `crates/db/src/lib.rs` n'a donc pas encore de `pub mod poste_charge;`. Le
//! module est inclus ici par chemin, tel quel, sans copie, sous un
//! `#[allow(dead_code)]` (le fichier de test n'appelle pas forcément les treize
//! symboles publics). Le jour où `lib.rs` le déclarera, on remplacera par
//! `use db::poste_charge::{…}` et ce bloc disparaîtra.
//!
//! # Ce que la base ne fournit pas
//!
//! Mesuré avant d'écrire : `llx_workstation_workstation` est vide (0 ligne) et
//! `llx_resource` aussi **dans l'entité 1**. Les tests créent donc leur machine et
//! leur groupe d'utilisateurs, les attachent, puis les effacent. Tout ce qu'ils
//! créent porte le préfixe `ZZPOSTE`, et ils nettoient **avant** et **après** :
//! un test interrompu au milieu laisserait sinon des restes qui feraient échouer
//! le suivant.
//!
//! L'auteur, lui, n'est jamais inventé : `fk_user_creat` porte une vraie clé
//! étrangère vers `llx_user`, on lit donc le plus petit identifiant réel de
//! l'entité et on s'ignore poliment s'il n'y a aucun utilisateur.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::poste_charge::{
    attacher_groupe, attacher_ressource, cout_horaire, creer, desactiver, detacher_groupe,
    detacher_ressource, get, groupes_de, list, modifier, ressources_de, valider, ModifPoste,
    NouveauPoste, Poste, LABEL_MAX, REF_MAX, STATUT_ACTIF, STATUT_DESACTIVE,
};

/// Entité de travail : la base de référence est en entité 1.
const ENTITE: i64 = 1;

/// Préfixe de *tous* les objets créés par ce fichier — postes, ressources,
/// groupes. Les tests ajoutent leur propre suffixe (`A-`, `COUT-`…) pour ne pas
/// se marcher dessus.
const PREFIXE: &str = "ZZPOSTE";

/// Un identifiant que la base ne peut pas contenir : `int(11)` plafonne à
/// 2 147 483 647, et la table des postes est vide au départ.
const ID_INEXISTANT: i64 = 999_999_999;

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

fn dec(valeur: &str) -> Decimal {
    Decimal::from_str(valeur).expect("décimal valide")
}

/// Le plus petit identifiant d'utilisateur réel de l'entité, ou `None` si la base
/// n'en a aucun. `fk_user_creat` est une clé étrangère : on ne peut pas inventer.
fn auteur(pool: &Pool) -> Result<Option<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let id: Option<i64> = conn.exec_first(
        "SELECT MIN(rowid) FROM llx_user WHERE entity = :entity",
        params! { "entity" => ENTITE },
    )?;
    Ok(id.filter(|i| *i > 0))
}

/// Efface tout ce que ce fichier a pu laisser, dans l'ordre des dépendances :
/// les liens d'abord (ils pointent sur le poste, la ressource et le groupe), puis
/// les objets eux-mêmes.
fn nettoyer(pool: &Pool) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let motif = format!("{PREFIXE}%");

    let postes: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_workstation_workstation WHERE ref LIKE :motif",
        params! { "motif" => motif.as_str() },
    )?;
    for poste in postes {
        conn.exec_drop(
            "DELETE FROM llx_workstation_workstation_resource WHERE fk_workstation = :poste",
            params! { "poste" => poste },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_workstation_workstation_usergroup WHERE fk_workstation = :poste",
            params! { "poste" => poste },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_workstation_workstation WHERE rowid = :poste",
            params! { "poste" => poste },
        )?;
    }

    conn.exec_drop(
        "DELETE FROM llx_resource WHERE ref LIKE :motif",
        params! { "motif" => motif.as_str() },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_usergroup WHERE nom LIKE :motif",
        params! { "motif" => motif.as_str() },
    )?;
    Ok(())
}

/// Crée une machine de test dans `llx_resource` et rend son `rowid`.
///
/// Écriture directe, volontairement : passer par `db::ressource::creer` lierait
/// les règles de deux modules, et ce test ne veut prouver qu'une chose — que
/// l'attachement refuse une cible absente de la table.
fn creer_ressource(pool: &Pool, reference: &str, auteur: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_resource (entity, ref, description, datec, fk_user_author, fk_statut) \
         VALUES (:entity, :reference, 'Machine de test ZZPOSTE', NOW(), :auteur, 1)",
        params! { "entity" => ENTITE, "reference" => reference, "auteur" => auteur },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Crée un groupe d'utilisateurs de test dans `llx_usergroup` et rend son `rowid`.
///
/// On ne réutilise pas le groupe réel de la base (« Commerciaux ») : un test ne
/// touche pas aux données qui ne sont pas les siennes.
fn creer_groupe(pool: &Pool, nom: &str) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_usergroup (nom, entity, datec) VALUES (:nom, :entity, NOW())",
        params! { "nom" => nom, "entity" => ENTITE },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Combien de postes portent cette référence, toutes entités confondues (le
/// préfixe est propre au test, il n'y a rien à cloisonner ici).
fn combien(pool: &Pool, reference: &str) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let total: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_workstation_workstation WHERE ref = :reference",
        params! { "reference" => reference },
    )?;
    Ok(total.unwrap_or(0))
}

/// Un poste minimal valide : une référence, un libellé, un coût, un auteur.
fn nouveau(reference: &str, auteur: i64) -> NouveauPoste {
    NouveauPoste {
        ref_: reference.to_string(),
        label: "Table de découpe".to_string(),
        description: None,
        nb_operators: 1,
        hourly_cost: dec("38.50"),
        fk_user_creat: auteur,
    }
}

/// Crée un poste de test et le rend : factorise les trois lignes de mise en place
/// des tests qui n'ont pas la création pour sujet.
fn poste_de_test(pool: &Pool, reference: &str, auteur: i64) -> Result<Poste, Box<dyn Error>> {
    let id = creer(pool, ENTITE, &nouveau(reference, auteur))?;
    get(pool, ENTITE, id)?.ok_or_else(|| format!("le poste {id} devrait exister").into())
}

// ---------------------------------------------------------------------------
// Création et relecture
// ---------------------------------------------------------------------------

#[test]
fn cree_un_poste_et_le_relit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        eprintln!("aucun utilisateur dans l'entité {ENTITE} : test ignoré");
        return Ok(());
    };

    let reference = "ZZPOSTE-A-TABLE";
    let mut new = nouveau(reference, auteur);
    new.description = Some("Table de découpe du rez-de-chaussée".into());
    new.nb_operators = 2;

    let id = creer(&pool, ENTITE, &new)?;
    assert!(id > 0, "la création doit rendre un identifiant positif");

    let poste = get(&pool, ENTITE, id)?.expect("le poste créé doit se relire");
    assert_eq!(poste.ref_, reference);
    assert_eq!(poste.label, "Table de découpe");
    assert_eq!(
        poste.description.as_deref(),
        Some("Table de découpe du rez-de-chaussée"),
        "la description doit atterrir dans note_public"
    );
    assert_eq!(poste.nb_operators, 2);
    assert_eq!(
        poste.hourly_cost,
        dec("38.50"),
        "le coût doit aller dans thm_operator_estimated et revenir au centime"
    );
    assert_eq!(
        poste.hourly_cost_machine, None,
        "thm_machine_estimated doit rester NULL : ce module ne l'écrit pas"
    );
    assert_eq!(
        poste.statut, STATUT_DESACTIVE,
        "un poste créé n'est pas en service : STATUS_DISABLED = 0"
    );
    assert_eq!(poste.entity, ENTITE);
    assert_eq!(poste.fk_user_creat, Some(auteur));
    assert_eq!(poste.fk_user_modif, None);
    assert!(
        poste.datec.is_some(),
        "date_creation est NOT NULL : elle doit être écrite"
    );
    assert_eq!(poste.type_, None, "type n'est pas écrit par ce module");

    let tous = list(&pool, ENTITE, 500, 0)?;
    assert!(
        tous.iter().any(|p| p.rowid == id),
        "le poste créé doit apparaître dans la liste de son entité"
    );

    assert!(
        get(&pool, ENTITE, ID_INEXISTANT)?.is_none(),
        "un identifiant inconnu doit rendre None, pas une erreur ni un poste vide"
    );

    nettoyer(&pool)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Refus à la création
// ---------------------------------------------------------------------------

#[test]
fn refuse_une_reference_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let mut new = nouveau("ZZPOSTE-VIDE", auteur);
    new.ref_ = "   ".into();

    let err = creer(&pool, ENTITE, &new).expect_err("une référence vide doit être refusée");
    let message = err.to_string();
    assert!(
        message.contains("référence"),
        "le message doit nommer la référence, reçu : {message}"
    );

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn refuse_un_libelle_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let mut new = nouveau("ZZPOSTE-SANS-NOM", auteur);
    new.label = "  ".into();

    let err = creer(&pool, ENTITE, &new).expect_err("un libellé vide doit être refusé");
    let message = err.to_string();
    assert!(
        message.contains("libellé"),
        "le message doit nommer le libellé, reçu : {message}"
    );
    assert_eq!(
        combien(&pool, "ZZPOSTE-SANS-NOM")?,
        0,
        "un refus ne doit rien écrire"
    );

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn refuse_une_reference_trop_longue() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    // `llx_workstation_workstation.ref` est un varchar(128) : une référence d'un
    // octet de plus ne doit pas partir en troncature silencieuse.
    let mut new = nouveau("ZZPOSTE-LONGUE", auteur);
    new.ref_ = "Z".repeat(REF_MAX + 1);

    let err = creer(&pool, ENTITE, &new).expect_err("une référence trop longue doit être refusée");
    let message = err.to_string();
    let limite = REF_MAX.to_string();
    assert!(
        message.contains(limite.as_str()),
        "le message doit citer la limite ({REF_MAX}), reçu : {message}"
    );

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn refuse_un_cout_horaire_negatif() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let mut new = nouveau("ZZPOSTE-COUT-NEGATIF", auteur);
    new.hourly_cost = dec("-0.01");

    let err = creer(&pool, ENTITE, &new).expect_err("un coût négatif doit être refusé");
    let message = err.to_string();
    assert!(
        message.contains("négatif"),
        "le message doit dire pourquoi, reçu : {message}"
    );
    assert_eq!(combien(&pool, "ZZPOSTE-COUT-NEGATIF")?, 0);

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn refuse_un_nombre_d_operateurs_negatif() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let mut new = nouveau("ZZPOSTE-OPERATEURS", auteur);
    new.nb_operators = -1;

    let err = creer(&pool, ENTITE, &new).expect_err("un effectif négatif doit être refusé");
    let message = err.to_string();
    assert!(
        message.contains("négatif"),
        "le message doit dire pourquoi, reçu : {message}"
    );

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn refuse_un_libelle_trop_long() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let mut new = nouveau("ZZPOSTE-LIBELLE-LONG", auteur);
    new.label = "L".repeat(LABEL_MAX + 1);

    let err = creer(&pool, ENTITE, &new).expect_err("un libellé trop long doit être refusé");
    let message = err.to_string();
    let limite = LABEL_MAX.to_string();
    assert!(
        message.contains(limite.as_str()),
        "le message doit citer la limite ({LABEL_MAX}), reçu : {message}"
    );

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn refuse_un_auteur_inexistant() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;

    // `fk_user_creat` porte une clé étrangère réelle vers `llx_user` : un
    // identifiant positif mais inconnu n'est pas attrapé par nos contrôles, c'est
    // la base qui le refuse — et le module doit traduire l'erreur 1452.
    let new = nouveau("ZZPOSTE-AUTEUR-FANTOME", ID_INEXISTANT);

    let err = creer(&pool, ENTITE, &new).expect_err("un auteur inexistant doit être refusé");
    let message = err.to_string();
    assert!(
        message.contains("n'existe pas"),
        "le message doit dire que l'auteur n'existe pas, reçu : {message}"
    );
    assert_eq!(combien(&pool, "ZZPOSTE-AUTEUR-FANTOME")?, 0);

    // Et un auteur nul ou négatif est refusé avant même d'atteindre la base.
    let new = nouveau("ZZPOSTE-AUTEUR-ZERO", 0);
    let err = creer(&pool, ENTITE, &new).expect_err("un auteur nul doit être refusé");
    assert!(err.to_string().contains("invalide"));

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn refuse_une_reference_deja_prise() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let reference = "ZZPOSTE-DOUBLON";
    creer(&pool, ENTITE, &nouveau(reference, auteur))?;

    // L'index `idx_workstation_workstation_ref` n'est pas unique : c'est donc au
    // code de refuser, et le test le prouve.
    let err = creer(&pool, ENTITE, &nouveau(reference, auteur))
        .expect_err("deux postes de même référence doivent être refusés");
    assert!(err.to_string().contains("déjà"));
    assert_eq!(
        combien(&pool, reference)?,
        1,
        "le refus ne doit pas ajouter une deuxième ligne"
    );

    nettoyer(&pool)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cycle de vie
// ---------------------------------------------------------------------------

#[test]
fn valide_un_poste_puis_refuse_de_le_revalider() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let poste = poste_de_test(&pool, "ZZPOSTE-VALIDE", auteur)?;
    assert_eq!(poste.statut, STATUT_DESACTIVE);

    valider(&pool, ENTITE, poste.rowid)?;
    let apres = get(&pool, ENTITE, poste.rowid)?.expect("poste relu");
    assert_eq!(
        apres.statut, STATUT_ACTIF,
        "valider doit mener à STATUS_ENABLED = 1"
    );

    let err = valider(&pool, ENTITE, poste.rowid)
        .expect_err("revalider un poste en service doit échouer");
    assert!(
        err.to_string().contains("déjà validé"),
        "le refus doit dire que c'est déjà fait, reçu : {err}"
    );

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn desactive_un_poste_puis_refuse_de_le_desactiver() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let poste = poste_de_test(&pool, "ZZPOSTE-DESACTIVE", auteur)?;

    // Un poste créé est déjà hors service : le désactiver n'a aucun sens.
    let err = desactiver(&pool, ENTITE, poste.rowid)
        .expect_err("désactiver un poste déjà hors service doit échouer");
    assert!(err.to_string().contains("déjà hors service"));

    valider(&pool, ENTITE, poste.rowid)?;
    desactiver(&pool, ENTITE, poste.rowid)?;
    assert_eq!(
        get(&pool, ENTITE, poste.rowid)?.expect("poste relu").statut,
        STATUT_DESACTIVE,
        "désactiver doit ramener à STATUS_DISABLED = 0"
    );

    let err =
        desactiver(&pool, ENTITE, poste.rowid).expect_err("un second désactivement doit échouer");
    assert!(err.to_string().contains("déjà hors service"));

    // Une transition sur un poste qui n'existe pas ne se contente pas d'un refus
    // silencieux : elle nomme le poste absent.
    let err = valider(&pool, ENTITE, ID_INEXISTANT).expect_err("un poste absent ne se valide pas");
    assert!(err.to_string().contains("n'existe pas"));

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn modifie_sans_jamais_toucher_la_reference() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let reference = "ZZPOSTE-MODIF";
    let poste = poste_de_test(&pool, reference, auteur)?;

    modifier(
        &pool,
        ENTITE,
        poste.rowid,
        &ModifPoste {
            label: "Four 1".into(),
            description: Some("Four à sole, 1 200 °C".into()),
            nb_operators: 3,
            hourly_cost: dec("55.75"),
        },
    )?;

    let apres = get(&pool, ENTITE, poste.rowid)?.expect("poste relu");
    assert_eq!(
        apres.ref_, reference,
        "la référence ne se modifie jamais : elle n'est même pas dans ModifPoste"
    );
    assert_eq!(apres.label, "Four 1");
    assert_eq!(apres.description.as_deref(), Some("Four à sole, 1 200 °C"));
    assert_eq!(apres.nb_operators, 3);
    assert_eq!(apres.hourly_cost, dec("55.75"));

    // La modification est totale : une description absente efface la note, et
    // c'est le libellé obligatoire qui empêche de vider la fiche par accident.
    modifier(
        &pool,
        ENTITE,
        poste.rowid,
        &ModifPoste {
            label: "Four 1".into(),
            ..Default::default()
        },
    )?;
    let apres = get(&pool, ENTITE, poste.rowid)?.expect("poste relu");
    assert_eq!(apres.description, None);
    assert_eq!(apres.hourly_cost, Decimal::ZERO);

    let err = modifier(
        &pool,
        ENTITE,
        poste.rowid,
        &ModifPoste {
            label: "   ".into(),
            ..Default::default()
        },
    )
    .expect_err("modifier avec un libellé vide doit échouer");
    assert!(err.to_string().contains("libellé"));

    let err = modifier(
        &pool,
        ENTITE,
        poste.rowid,
        &ModifPoste {
            label: "Four 1".into(),
            hourly_cost: dec("-5"),
            ..Default::default()
        },
    )
    .expect_err("modifier avec un coût négatif doit échouer");
    assert!(err.to_string().contains("négatif"));

    nettoyer(&pool)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Machines attachées
// ---------------------------------------------------------------------------

#[test]
fn attache_une_ressource_puis_refuse_le_doublon() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let poste = poste_de_test(&pool, "ZZPOSTE-RESSOURCE", auteur)?;
    let machine = creer_ressource(&pool, "ZZPOSTE-RESS-MACHINE", auteur)?;

    let lien = attacher_ressource(&pool, ENTITE, poste.rowid, machine)?;
    assert!(lien > 0);
    assert_eq!(
        ressources_de(&pool, ENTITE, poste.rowid)?,
        vec![machine],
        "la machine attachée doit se relire sur le poste"
    );

    let err = attacher_ressource(&pool, ENTITE, poste.rowid, machine)
        .expect_err("attacher deux fois la même machine doit échouer");
    let message = err.to_string();
    assert!(
        message.contains("déjà attachée"),
        "le refus doit dire que c'est déjà fait, reçu : {message}"
    );
    assert_eq!(
        ressources_de(&pool, ENTITE, poste.rowid)?.len(),
        1,
        "le refus ne doit pas créer un second lien"
    );

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn refuse_d_attacher_une_ressource_inexistante() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let poste = poste_de_test(&pool, "ZZPOSTE-RESS-FANTOME", auteur)?;

    let err = attacher_ressource(&pool, ENTITE, poste.rowid, ID_INEXISTANT)
        .expect_err("une machine inexistante ne s'attache pas");
    assert!(err.to_string().contains("n'existe pas"));
    assert!(ressources_de(&pool, ENTITE, poste.rowid)?.is_empty());

    // Et un poste inexistant ne reçoit rien non plus.
    let machine = creer_ressource(&pool, "ZZPOSTE-RESS-ORPHELINE", auteur)?;
    let err = attacher_ressource(&pool, ENTITE, ID_INEXISTANT, machine)
        .expect_err("un poste inexistant ne reçoit pas de machine");
    assert!(err.to_string().contains("n'existe pas"));

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn detache_une_ressource_puis_refuse_de_la_detacher_deux_fois() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let poste = poste_de_test(&pool, "ZZPOSTE-DETACHE-RESS", auteur)?;
    let machine = creer_ressource(&pool, "ZZPOSTE-RESS-DETACHEE", auteur)?;
    attacher_ressource(&pool, ENTITE, poste.rowid, machine)?;

    detacher_ressource(&pool, ENTITE, poste.rowid, machine)?;
    assert!(
        ressources_de(&pool, ENTITE, poste.rowid)?.is_empty(),
        "après détachement, plus aucun lien ne doit rester"
    );

    let err = detacher_ressource(&pool, ENTITE, poste.rowid, machine)
        .expect_err("détacher deux fois doit échouer");
    assert!(
        err.to_string().contains("aucun lien"),
        "le refus doit dire qu'il n'y a pas de lien, reçu : {err}"
    );

    nettoyer(&pool)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Groupes d'utilisateurs attachés
// ---------------------------------------------------------------------------

#[test]
fn attache_un_groupe_puis_refuse_le_doublon() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let poste = poste_de_test(&pool, "ZZPOSTE-GROUPE", auteur)?;
    let groupe = creer_groupe(&pool, "ZZPOSTE-EQUIPE-ATELIER")?;

    let lien = attacher_groupe(&pool, ENTITE, poste.rowid, groupe)?;
    assert!(lien > 0);
    assert_eq!(groupes_de(&pool, ENTITE, poste.rowid)?, vec![groupe]);

    let err = attacher_groupe(&pool, ENTITE, poste.rowid, groupe)
        .expect_err("attacher deux fois le même groupe doit échouer");
    assert!(err.to_string().contains("déjà"));
    assert_eq!(groupes_de(&pool, ENTITE, poste.rowid)?.len(), 1);

    // Un groupe absent de l'entité ne s'attache pas davantage.
    let err = attacher_groupe(&pool, ENTITE, poste.rowid, ID_INEXISTANT)
        .expect_err("un groupe inexistant ne s'attache pas");
    assert!(err.to_string().contains("n'existe pas"));

    nettoyer(&pool)?;
    Ok(())
}

#[test]
fn detache_un_groupe_puis_refuse_de_le_detacher_deux_fois() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let poste = poste_de_test(&pool, "ZZPOSTE-DETACHE-GROUPE", auteur)?;
    let groupe = creer_groupe(&pool, "ZZPOSTE-EQUIPE-FOUR")?;
    attacher_groupe(&pool, ENTITE, poste.rowid, groupe)?;

    detacher_groupe(&pool, ENTITE, poste.rowid, groupe)?;
    assert!(groupes_de(&pool, ENTITE, poste.rowid)?.is_empty());

    let err = detacher_groupe(&pool, ENTITE, poste.rowid, groupe)
        .expect_err("détacher deux fois doit échouer");
    assert!(err.to_string().contains("aucun lien"));

    nettoyer(&pool)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Coût horaire
// ---------------------------------------------------------------------------

#[test]
fn cout_horaire_additionne_operateur_et_machine() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool)?;
    let Some(auteur) = auteur(&pool)? else {
        return Ok(());
    };

    let poste = poste_de_test(&pool, "ZZPOSTE-COUT", auteur)?;
    assert_eq!(
        cout_horaire(&pool, ENTITE, poste.rowid)?,
        dec("38.50"),
        "sans coût machine, le coût horaire est celui de l'opérateur"
    );

    // On ajoute un coût machine comme le ferait le formulaire de Dolibarr (ce
    // module ne l'écrit pas) : le coût total doit suivre, et être arrondi.
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_workstation_workstation SET thm_machine_estimated = '7.256' \
         WHERE rowid = :poste",
        params! { "poste" => poste.rowid },
    )?;
    assert_eq!(
        cout_horaire(&pool, ENTITE, poste.rowid)?,
        dec("45.76"),
        "38.50 + 7.256 doit donner 45.76 (arrondi au centime), et inclure la machine"
    );

    // Le nombre d'opérateurs ne multiplie rien : c'est un effectif, pas un tarif.
    // Le calcul de Dolibarr (bom.class.php, ligne 1479) ne le lit même pas.
    conn.exec_drop(
        "UPDATE llx_workstation_workstation SET nb_operators_required = 4 \
         WHERE rowid = :poste",
        params! { "poste" => poste.rowid },
    )?;
    assert_eq!(
        cout_horaire(&pool, ENTITE, poste.rowid)?,
        dec("45.76"),
        "quatre opérateurs ne quadruplent pas le coût horaire du poste"
    );

    // Un poste hors service garde son coût : c'est lui qui justifie les prix déjà
    // calculés.
    valider(&pool, ENTITE, poste.rowid)?;
    desactiver(&pool, ENTITE, poste.rowid)?;
    assert_eq!(cout_horaire(&pool, ENTITE, poste.rowid)?, dec("45.76"));

    let err =
        cout_horaire(&pool, ENTITE, ID_INEXISTANT).expect_err("un poste absent n'a pas de coût");
    assert!(err.to_string().contains("n'existe pas"));

    nettoyer(&pool)?;
    Ok(())
}
