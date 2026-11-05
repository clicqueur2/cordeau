//! Ressources et liens aux objets métier : création, relecture, attachement,
//! détachement, cloisonnement par la société.
//!
//! Les tests touchent la base : ils ne tournent que si `DOLIBARR_DSN` est posée,
//! et s'ignorent franchement sinon — un test qui échoue faute de base masquerait
//! une vraie régression.
//!
//! Pas de verrou de série ici, contrairement aux projets : rien n'est numéroté,
//! donc rien ne se dispute. Chaque test porte sa propre référence de ressource
//! et son propre identifiant d'élément, et fait le ménage avant **et** après —
//! un test interrompu laisserait sinon des restes qui feraient échouer le
//! suivant.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::ressource::{
    attacher, combien_de_lies, creer, detacher, get, list, modifier, ressources_de, ModifRessource,
    NewRessource, REF_MAX, STATUT_VALIDE, TYPE_RESSOURCE,
};

const ENTITY: i64 = 1;
/// Une autre société, pour vérifier le cloisonnement. `llx_element_resources`
/// n'a pas d'entité : c'est la ressource qui en porte une.
const ENTITY_AUTRE: i64 = 2;

/// Un identifiant d'utilisateur quelconque, mais strictement positif.
///
/// `llx_resource.fk_user_author` ne porte aucune clé étrangère vers `llx_user` :
/// le module ne contrôle que la positivité, et c'est tout ce qu'on teste ici.
const AUTEUR: i64 = 1;

/// Un type de ressource réellement présent et actif dans `llx_c_type_resource`.
const TYPE_ACTIF: &str = "RES_ROOMS";

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

/// Une ressource minimale valide : une référence, un type actif, un auteur.
fn nouvelle(reference: &str) -> NewRessource {
    NewRessource {
        ref_: reference.to_string(),
        fk_code_type_resource: Some(TYPE_ACTIF.to_string()),
        fk_user_author: AUTEUR,
        ..Default::default()
    }
}

/// Supprime les liens d'une ressource puis la ressource elle-même.
///
/// Les liens partent en premier : ils pointent sur `resource_id`, et les laisser
/// derrière soi ferait échouer `combien_de_lies` d'un test suivant sur une
/// ressource recréée avec le même `rowid`.
fn effacer(pool: &Pool, entity: i64, ressource: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_element_resources WHERE resource_id = :ressource",
        params! { "ressource" => ressource },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_resource WHERE entity = :entity AND rowid = :ressource",
        params! { "entity" => entity, "ressource" => ressource },
    )?;
    Ok(())
}

/// Supprime toute ressource portant cette référence dans cette société.
fn nettoyer(pool: &Pool, entity: i64, reference: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_resource WHERE entity = :entity AND ref = :reference",
        params! { "entity" => entity, "reference" => reference },
    )?;
    for id in ids {
        effacer(pool, entity, id)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Création et relecture
// ---------------------------------------------------------------------------

#[test]
fn cree_une_ressource_et_la_relit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Salle réunion R1";
    nettoyer(&pool, ENTITY, reference)?;

    let mut new = nouvelle(reference);
    new.asset_number = Some("INV-2026-014".into());
    new.description = Some("Salle du premier étage, douze places".into());
    new.address = Some("4 rue de la Calade".into());
    new.zip = Some("69390".into());
    new.town = Some("Charly".into());
    new.phone = Some("04 78 00 00 00".into());
    new.email = Some("salle@exemple.fr".into());
    new.url = Some("https://exemple.fr/salle".into());
    new.max_users = Some(12);

    let id = creer(&pool, ENTITY, &new)?;
    assert!(id > 0);

    let r = get(&pool, ENTITY, id)?.expect("ressource relue");
    assert_eq!(r.rowid, id);
    assert_eq!(r.entity, ENTITY);
    assert_eq!(
        r.ref_, reference,
        "la référence est le nom saisi, jamais un compteur"
    );
    assert_eq!(r.asset_number.as_deref(), Some("INV-2026-014"));
    assert_eq!(
        r.description.as_deref(),
        Some("Salle du premier étage, douze places")
    );
    assert_eq!(r.fk_code_type_resource.as_deref(), Some(TYPE_ACTIF));
    assert_eq!(
        r.type_label.as_deref(),
        Some("Rooms"),
        "le libellé du dictionnaire vient de la jointure"
    );
    assert_eq!(r.address.as_deref(), Some("4 rue de la Calade"));
    assert_eq!(r.zip.as_deref(), Some("69390"));
    assert_eq!(r.town.as_deref(), Some("Charly"));
    assert_eq!(r.phone.as_deref(), Some("04 78 00 00 00"));
    assert_eq!(r.email.as_deref(), Some("salle@exemple.fr"));
    assert_eq!(r.url.as_deref(), Some("https://exemple.fr/salle"));
    assert_eq!(r.max_users, Some(12));
    assert_eq!(r.fk_statut, STATUT_VALIDE);
    assert!(r.datec.is_some(), "datec est posée à NOW()");
    assert_eq!(r.fk_user_author, Some(AUTEUR));
    assert_eq!(
        r.duree, None,
        "la durée est une colonne de la jonction, pas de la fiche"
    );
    assert!(!r.busy);

    // La liste de l'entité la contient, et elle est cloisonnée.
    let liste = list(&pool, ENTITY, 500, 0)?;
    assert!(
        liste.iter().any(|x| x.rowid == id),
        "la ressource créée doit apparaître dans la liste de son entité"
    );
    assert!(
        get(&pool, ENTITY_AUTRE, id)?.is_none(),
        "une ressource ne se lit pas depuis une autre société"
    );

    effacer(&pool, ENTITY, id)?;
    nettoyer(&pool, ENTITY, reference)?;
    assert!(get(&pool, ENTITY, id)?.is_none());
    Ok(())
}

// ---------------------------------------------------------------------------
// Refus à la création
// ---------------------------------------------------------------------------

#[test]
fn refuse_une_reference_vide_ou_trop_longue() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };

    let e = creer(&pool, ENTITY, &nouvelle("   ")).unwrap_err();
    assert!(
        e.to_string().contains("obligatoire"),
        "message inattendu : {e}"
    );

    // La borne est en octets : 255 passe, 256 est refusé.
    let limite = "R".repeat(REF_MAX);
    nettoyer(&pool, ENTITY, &limite)?;
    let id = creer(&pool, ENTITY, &nouvelle(&limite))?;
    assert_eq!(get(&pool, ENTITY, id)?.unwrap().ref_.len(), REF_MAX);
    effacer(&pool, ENTITY, id)?;
    nettoyer(&pool, ENTITY, &limite)?;

    let trop_longue = "R".repeat(REF_MAX + 1);
    let e = creer(&pool, ENTITY, &nouvelle(&trop_longue)).unwrap_err();
    assert!(e.to_string().contains("octets"), "message inattendu : {e}");
    Ok(())
}

#[test]
fn refuse_un_type_de_ressource_inconnu_ou_desactive() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Ressource type refusé";
    nettoyer(&pool, ENTITY, reference)?;

    // Type absent du dictionnaire national : le message doit citer le code.
    let mut inconnu = nouvelle(reference);
    inconnu.fk_code_type_resource = Some("RES_ASTRONAUTES".into());
    let e = creer(&pool, ENTITY, &inconnu).unwrap_err();
    assert!(
        e.to_string().contains("RES_ASTRONAUTES"),
        "le refus doit nommer le code refusé : {e}"
    );

    // Type présent mais désactivé : refus distinct, avec le code aussi.
    let mut conn = pool.get_conn()?;
    let code_inactif = "RES_ZZ_RESSOURCE_INACTIF";
    conn.exec_drop(
        "DELETE FROM llx_c_type_resource WHERE code = :modele_type",
        params! { "modele_type" => code_inactif },
    )?;
    conn.exec_drop(
        "INSERT INTO llx_c_type_resource (code, label, active) \
         VALUES (:modele_type, 'ZZ Ressource inactive', 0)",
        params! { "modele_type" => code_inactif },
    )?;
    let mut inactif = nouvelle(reference);
    inactif.fk_code_type_resource = Some(code_inactif.to_string());
    let e = creer(&pool, ENTITY, &inactif).unwrap_err();
    assert!(
        e.to_string().contains(code_inactif) && e.to_string().contains("désactivé"),
        "message inattendu : {e}"
    );
    conn.exec_drop(
        "DELETE FROM llx_c_type_resource WHERE code = :modele_type",
        params! { "modele_type" => code_inactif },
    )?;

    // Sans type, la ressource est licite : la colonne est nullable et Dolibarr
    // accepte une ressource non classée.
    let mut sans_type = nouvelle(reference);
    sans_type.fk_code_type_resource = None;
    let id = creer(&pool, ENTITY, &sans_type)?;
    let r = get(&pool, ENTITY, id)?.unwrap();
    assert_eq!(r.fk_code_type_resource, None);
    assert_eq!(r.type_label, None);

    effacer(&pool, ENTITY, id)?;
    nettoyer(&pool, ENTITY, reference)?;
    Ok(())
}

#[test]
fn refuse_un_email_sans_arobase() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Ressource email";
    nettoyer(&pool, ENTITY, reference)?;

    let mut sans_arobase = nouvelle(reference);
    sans_arobase.email = Some("salle.exemple.fr".into());
    let e = creer(&pool, ENTITY, &sans_arobase).unwrap_err();
    assert!(e.to_string().contains("arobase"), "message inattendu : {e}");

    // Avec l'arobase, la même fiche passe : le refus vient bien du contrôle.
    let mut valide = nouvelle(reference);
    valide.email = Some("salle@exemple.fr".into());
    let id = creer(&pool, ENTITY, &valide)?;
    assert_eq!(
        get(&pool, ENTITY, id)?.unwrap().email.as_deref(),
        Some("salle@exemple.fr")
    );

    effacer(&pool, ENTITY, id)?;
    nettoyer(&pool, ENTITY, reference)?;
    Ok(())
}

#[test]
fn refuse_une_url_sans_http() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Ressource url";
    nettoyer(&pool, ENTITY, reference)?;

    let mut sans_http = nouvelle(reference);
    sans_http.url = Some("exemple.fr/salle".into());
    let e = creer(&pool, ENTITY, &sans_http).unwrap_err();
    assert!(e.to_string().contains("http"), "message inattendu : {e}");

    let mut valide = nouvelle(reference);
    valide.url = Some("https://exemple.fr/salle".into());
    let id = creer(&pool, ENTITY, &valide)?;
    assert_eq!(
        get(&pool, ENTITY, id)?.unwrap().url.as_deref(),
        Some("https://exemple.fr/salle")
    );

    effacer(&pool, ENTITY, id)?;
    nettoyer(&pool, ENTITY, reference)?;
    Ok(())
}

#[test]
fn refuse_un_max_users_negatif_et_un_auteur_invalide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Ressource champs invalides";
    nettoyer(&pool, ENTITY, reference)?;

    let mut negatif = nouvelle(reference);
    negatif.max_users = Some(-1);
    let e = creer(&pool, ENTITY, &negatif).unwrap_err();
    assert!(e.to_string().contains("négatif"), "message inattendu : {e}");

    let mut sans_auteur = nouvelle(reference);
    sans_auteur.fk_user_author = 0;
    let e = creer(&pool, ENTITY, &sans_auteur).unwrap_err();
    assert!(e.to_string().contains("auteur"), "message inattendu : {e}");

    assert!(
        creer(&pool, ENTITY, &nouvelle(reference)).is_ok(),
        "les refus ci-dessus viennent de la validation, pas des données"
    );
    nettoyer(&pool, ENTITY, reference)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Modification
// ---------------------------------------------------------------------------

#[test]
fn modifie_une_ressource_sans_toucher_a_la_reference() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Ressource à modifier";
    nettoyer(&pool, ENTITY, reference)?;

    let mut new = nouvelle(reference);
    new.description = Some("Description d'origine".into());
    new.max_users = Some(8);
    let id = creer(&pool, ENTITY, &new)?;
    let avant = get(&pool, ENTITY, id)?.expect("ressource créée");

    // Seuls les champs renseignés changent : la structure de modification est
    // complète, un champ laissé à `None` est donc remis à `NULL`.
    let modif = ModifRessource {
        town: Some("Vienne".into()),
        max_users: Some(4),
        ..Default::default()
    };
    modifier(&pool, ENTITY, id, &modif)?;

    let apres = get(&pool, ENTITY, id)?.expect("ressource relue");
    assert_eq!(
        apres.ref_, avant.ref_,
        "la référence est saisie une fois : elle ne change jamais"
    );
    assert_eq!(apres.town.as_deref(), Some("Vienne"));
    assert_eq!(apres.max_users, Some(4));
    assert_eq!(
        apres.description, None,
        "un champ omis dans la modification est remis à NULL"
    );
    assert_eq!(apres.fk_code_type_resource, None);
    assert_eq!(apres.fk_statut, STATUT_VALIDE, "le statut n'est pas touché");

    // Les contrôles de la création s'appliquent aussi ici.
    let email_faux = ModifRessource {
        email: Some("sans.arobase".into()),
        ..Default::default()
    };
    assert!(modifier(&pool, ENTITY, id, &email_faux).is_err());

    let type_faux = ModifRessource {
        fk_code_type_resource: Some("RES_ASTRONAUTES".into()),
        ..Default::default()
    };
    assert!(modifier(&pool, ENTITY, id, &type_faux).is_err());

    assert!(
        modifier(&pool, ENTITY, id + 9_999_999, &modif).is_err(),
        "une ressource inexistante ne se met pas à jour"
    );

    // Aucun des refus n'a laissé de trace.
    assert_eq!(get(&pool, ENTITY, id)?.unwrap().ref_, avant.ref_);
    assert_eq!(
        get(&pool, ENTITY, id)?.unwrap().town.as_deref(),
        Some("Vienne")
    );

    effacer(&pool, ENTITY, id)?;
    nettoyer(&pool, ENTITY, reference)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Attachement et détachement
// ---------------------------------------------------------------------------

#[test]
fn attache_une_ressource_puis_refuse_le_doublon() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Fourgon attachement";
    let reference_bis = "ZZ Fourgon attachement bis";
    nettoyer(&pool, ENTITY, reference)?;
    nettoyer(&pool, ENTITY, reference_bis)?;

    let r = creer(&pool, ENTITY, &nouvelle(reference))?;
    let r2 = creer(&pool, ENTITY, &nouvelle(reference_bis))?;
    let propal = 424_242_i64;

    let lien = attacher(&pool, ENTITY, r, propal, "propal", Some(dec("2.5")))?;
    assert!(lien > 0);

    // Doublon exact : même ressource, même élément, même type d'élément.
    let e = attacher(&pool, ENTITY, r, propal, "propal", None).unwrap_err();
    assert!(
        e.to_string().contains("déjà attachée"),
        "message inattendu : {e}"
    );

    // Même élément, autre ressource : licite.
    assert!(attacher(&pool, ENTITY, r2, propal, "propal", None).is_ok());
    // Même ressource, autre élément : licite.
    assert!(attacher(&pool, ENTITY, r, propal + 1, "propal", None).is_ok());

    effacer(&pool, ENTITY, r)?;
    effacer(&pool, ENTITY, r2)?;
    nettoyer(&pool, ENTITY, reference)?;
    nettoyer(&pool, ENTITY, reference_bis)?;
    Ok(())
}

#[test]
fn refuse_un_type_d_element_inconnu() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Ressource type d'élément";
    nettoyer(&pool, ENTITY, reference)?;
    let r = creer(&pool, ENTITY, &nouvelle(reference))?;

    let e = attacher(&pool, ENTITY, r, 1, "sous-marin", None).unwrap_err();
    assert!(
        e.to_string().contains("n'est pas géré"),
        "message inattendu : {e}"
    );
    assert!(
        e.to_string().contains("propal"),
        "le refus doit rappeler les types acceptés : {e}"
    );

    // Le détachement et la lecture refusent la même chose.
    assert!(detacher(&pool, ENTITY, r, 1, "sous-marin").is_err());
    assert!(ressources_de(&pool, ENTITY, 1, "sous-marin").is_err());

    effacer(&pool, ENTITY, r)?;
    nettoyer(&pool, ENTITY, reference)?;
    Ok(())
}

#[test]
fn refuse_un_attachement_invalide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Ressource attachement invalide";
    nettoyer(&pool, ENTITY, reference)?;
    let r = creer(&pool, ENTITY, &nouvelle(reference))?;

    assert!(
        attacher(&pool, ENTITY, r, 151_515, "propal", Some(dec("-1"))).is_err(),
        "une durée négative est refusée"
    );
    assert!(
        attacher(&pool, ENTITY, r, 0, "propal", None).is_err(),
        "un élément d'identifiant nul est refusé"
    );
    assert!(
        attacher(&pool, ENTITY, r + 9_999_999, 151_515, "propal", None).is_err(),
        "une ressource inexistante est refusée"
    );

    // Une durée nulle est licite : elle veut dire « non renseignée ».
    assert!(attacher(&pool, ENTITY, r, 151_515, "propal", Some(dec("0"))).is_ok());

    effacer(&pool, ENTITY, r)?;
    nettoyer(&pool, ENTITY, reference)?;
    Ok(())
}

#[test]
fn detache_puis_refuse_de_detacher_deux_fois() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Ressource détachement";
    nettoyer(&pool, ENTITY, reference)?;
    let r = creer(&pool, ENTITY, &nouvelle(reference))?;
    let facture = 525_252_i64;

    attacher(&pool, ENTITY, r, facture, "facture", None)?;
    assert_eq!(ressources_de(&pool, ENTITY, facture, "facture")?.len(), 1);

    detacher(&pool, ENTITY, r, facture, "facture")?;
    assert!(
        ressources_de(&pool, ENTITY, facture, "facture")?.is_empty(),
        "le lien est bien parti"
    );

    let e = detacher(&pool, ENTITY, r, facture, "facture").unwrap_err();
    assert!(
        e.to_string().contains("aucun lien"),
        "message inattendu : {e}"
    );

    effacer(&pool, ENTITY, r)?;
    nettoyer(&pool, ENTITY, reference)?;
    Ok(())
}

#[test]
fn ressources_de_rend_la_ressource_attachee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Nacelle";
    nettoyer(&pool, ENTITY, reference)?;
    let r = creer(&pool, ENTITY, &nouvelle(reference))?;
    let projet = 636_363_i64;

    attacher(&pool, ENTITY, r, projet, "projet", Some(dec("8.25")))?;

    let liees = ressources_de(&pool, ENTITY, projet, "projet")?;
    assert_eq!(liees.len(), 1);
    assert_eq!(liees[0].rowid, r);
    assert_eq!(liees[0].ref_, reference);
    assert_eq!(
        liees[0].duree,
        Some(dec("8.25")),
        "la durée sort en Decimal, pas en flottant"
    );
    assert!(!liees[0].busy, "attacher écrit busy = 0, comme Dolibarr");
    assert_eq!(liees[0].fk_code_type_resource.as_deref(), Some(TYPE_ACTIF));

    // `projet` et `project` désignent le même élément : la lecture doit rendre
    // la même ligne, sinon on aurait deux liens pour un seul élément.
    assert_eq!(
        ressources_de(&pool, ENTITY, projet, "project")?.len(),
        1,
        "l'orthographe Dolibarr doit retrouver le lien écrit depuis « projet »"
    );
    // Un autre élément n'a rien.
    assert!(ressources_de(&pool, ENTITY, projet + 1, "projet")?.is_empty());

    effacer(&pool, ENTITY, r)?;
    nettoyer(&pool, ENTITY, reference)?;
    Ok(())
}

#[test]
fn combien_de_lies_compte_les_liens() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Ressource comptage";
    nettoyer(&pool, ENTITY, reference)?;
    let r = creer(&pool, ENTITY, &nouvelle(reference))?;

    assert_eq!(
        combien_de_lies(&pool, ENTITY, r)?,
        0,
        "aucun lien au départ"
    );

    attacher(&pool, ENTITY, r, 747_474, "commande", None)?;
    attacher(&pool, ENTITY, r, 747_475, "ticket", None)?;
    assert_eq!(combien_de_lies(&pool, ENTITY, r)?, 2);

    detacher(&pool, ENTITY, r, 747_474, "commande")?;
    assert_eq!(combien_de_lies(&pool, ENTITY, r)?, 1);

    assert!(
        combien_de_lies(&pool, ENTITY, r + 9_999_999).is_err(),
        "compter les liens d'une ressource inexistante est une erreur, pas zéro"
    );

    effacer(&pool, ENTITY, r)?;
    nettoyer(&pool, ENTITY, reference)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cloisonnement : le point que la doc du module annonce
// ---------------------------------------------------------------------------

/// `llx_element_resources` n'a pas de colonne `entity` : c'est `llx_resource` qui
/// cloisonne. Ce test le vérifie de bout en bout — écriture, lecture,
/// détachement et comptage depuis la mauvaise société.
#[test]
fn le_cloisonnement_vient_de_la_ressource() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let reference = "ZZ Ressource société 1";
    let reference_autre = "ZZ Ressource autre société";
    nettoyer(&pool, ENTITY, reference)?;
    nettoyer(&pool, ENTITY_AUTRE, reference_autre)?;

    let ici = creer(&pool, ENTITY, &nouvelle(reference))?;
    let ailleurs = creer(&pool, ENTITY_AUTRE, &nouvelle(reference_autre))?;
    let commande = 858_585_i64;

    // On n'attache pas depuis la société 1 une ressource de la société 2.
    assert!(
        attacher(&pool, ENTITY, ailleurs, commande, "commande", None).is_err(),
        "une ressource d'une autre société ne s'attache pas depuis ici"
    );

    attacher(&pool, ENTITY_AUTRE, ailleurs, commande, "commande", None)?;

    // La lecture depuis la société 1 ne doit rien voir, alors même que le lien
    // existe et porte le bon `element_id`.
    assert!(
        ressources_de(&pool, ENTITY, commande, "commande")?.is_empty(),
        "sans la jointure sur llx_resource, la ressource d'une autre société sortirait ici"
    );
    assert_eq!(
        ressources_de(&pool, ENTITY_AUTRE, commande, "commande")?.len(),
        1
    );

    // Détacher depuis la mauvaise société échoue et ne détruit rien.
    assert!(detacher(&pool, ENTITY, ailleurs, commande, "commande").is_err());
    assert_eq!(
        ressources_de(&pool, ENTITY_AUTRE, commande, "commande")?.len(),
        1,
        "un détachement refusé ne doit pas emporter le lien"
    );

    assert_eq!(combien_de_lies(&pool, ENTITY_AUTRE, ailleurs)?, 1);
    assert!(
        combien_de_lies(&pool, ENTITY, ailleurs).is_err(),
        "compter depuis la mauvaise société est une erreur"
    );

    // La ressource de la société 1 n'a jamais été attachée.
    assert_eq!(combien_de_lies(&pool, ENTITY, ici)?, 0);
    assert_eq!(TYPE_RESSOURCE, "dolresource");

    effacer(&pool, ENTITY, ici)?;
    effacer(&pool, ENTITY_AUTRE, ailleurs)?;
    nettoyer(&pool, ENTITY, reference)?;
    nettoyer(&pool, ENTITY_AUTRE, reference_autre)?;
    Ok(())
}
