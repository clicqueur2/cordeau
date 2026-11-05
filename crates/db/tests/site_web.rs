//! Sites web et pages : création, unicité des adresses, publication, suppression.
//!
//! Tout se passe dans l'entité 1, et chaque test porte sa **propre marque**
//! (`zz-sw-<pid>-<sujet>`) : les tests tournent en parallèle, et deux tests qui
//! partageraient la même référence de site se détruiraient mutuellement au
//! nettoyage. Le ménage se fait avant autant qu'après, pour qu'un test
//! interrompu ne fasse pas échouer le suivant.
//!
//! Pas besoin de verrou de sérialisation ici, contrairement aux fiches de
//! connaissance : ce module n'attribue aucune référence automatique. Chaque
//! test nomme ses sites, donc deux tests ne peuvent pas se disputer un nom.
//!
//! Les marques n'utilisent ni `%` ni `_` : le nettoyage s'en sert comme motif
//! `LIKE`, et un `_` y serait un joker qui effacerait les sites du voisin.

use std::error::Error;

use db::site_web::{
    creer_page, creer_site, depublier_page, page, pages, publier_page, renommer_page, sites,
    supprimer_page, supprimer_site, NewPage, NewSite, STATUT_EN_LIGNE, STATUT_HORS_LIGNE,
};
use mysql::prelude::Queryable;
use mysql::{params, Pool};

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

/// La marque du test : un préfixe unique, sans `%` ni `_`.
fn marque(sujet: &str) -> String {
    format!("zz-sw-{}-{sujet}", std::process::id())
}

/// Efface tout ce que les tests de ce fichier ont pu laisser.
///
/// Le ménage se fait par marque et non par identifiant : un test interrompu
/// laisse des lignes dont on ne connaît plus le `rowid`, et elles feraient
/// échouer le décompte du test suivant. La marque porte le pid du processus, il
/// n'y a donc aucun risque d'emporter les lignes d'un autre test — y compris
/// celles qu'un test a créées **dans une autre entité** pour vérifier que la
/// référence n'est unique que par entité.
///
/// L'ordre compte : les étiquettes de page portent une clé étrangère vers la
/// page, et la page vers le site.
fn nettoyer(pool: &Pool, marque: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_website WHERE ref LIKE :motif",
        params! { "motif" => format!("{marque}%") },
    )?;
    for id in ids {
        conn.exec_drop(
            "DELETE FROM llx_categorie_website_page WHERE fk_website_page IN \
             (SELECT rowid FROM llx_website_page WHERE fk_website = :id)",
            params! { "id" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_website_page WHERE fk_website = :id",
            params! { "id" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_website_extrafields WHERE fk_object = :id",
            params! { "id" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_website WHERE rowid = :id",
            params! { "id" => id },
        )?;
    }
    Ok(())
}

/// Un site de test, complet comme un vrai : description, langue, pas de
/// domaine propre.
fn site_de_test(pool: &Pool, reference: &str) -> Result<i64, Box<dyn Error>> {
    creer_site(
        pool,
        ENTITY,
        &NewSite {
            ref_: reference.to_string(),
            description: Some("Site de test, effacé par le nettoyage".to_string()),
            lang: Some("fr_FR".to_string()),
            virtualhost: None,
        },
    )
}

/// Une page de test, en ligne, avec un contenu reconnaissable.
fn page_de_test(pool: &Pool, site: i64, adresse: &str) -> Result<i64, Box<dyn Error>> {
    creer_page(
        pool,
        ENTITY,
        &NewPage {
            fk_website: site,
            pageurl: adresse.to_string(),
            title: Some(format!("Page {adresse}")),
            description: Some("Page de test".to_string()),
            keywords: Some("test, site".to_string()),
            lang: Some("fr_FR".to_string()),
            content: "<p>Bonjour</p>".to_string(),
            status: STATUT_EN_LIGNE,
        },
    )
}

/// Le site de test, relu dans la liste de l'entité.
fn site_relu(pool: &Pool, reference: &str) -> Result<db::site_web::Site, Box<dyn Error>> {
    let tous = sites(pool, ENTITY, 500)?;
    tous.into_iter()
        .find(|s| s.ref_ == reference)
        .ok_or_else(|| format!("le site « {reference} » n'est pas relu dans la liste").into())
}

/// Créer un site et une page, puis les relire : les champs écrits reviennent, et
/// le contenu n'est chargé que quand on le demande.
#[test]
fn un_site_et_une_page_se_creent_et_se_relisent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("lecture");
    nettoyer(&pool, &marque)?;

    let reference = format!("{marque}-site");
    let id_site = site_de_test(&pool, &reference)?;

    let adresse = format!("{marque}-contact");
    // Un contenu de plusieurs kilo-octets : c'est le cas normal d'une page, et
    // c'est ce que la liste ne doit pas charger.
    let contenu = format!("<p>{}</p>", "Bloc de texte. ".repeat(2000));
    let id_page = creer_page(
        &pool,
        ENTITY,
        &NewPage {
            fk_website: id_site,
            pageurl: adresse.clone(),
            title: Some("Nous contacter".to_string()),
            description: Some("Téléphone, adresse et formulaire".to_string()),
            keywords: Some("contact, devis, maçonnerie".to_string()),
            lang: Some("fr_FR".to_string()),
            content: contenu.clone(),
            status: STATUT_EN_LIGNE,
        },
    )?;

    let site = site_relu(&pool, &reference)?;
    assert_eq!(site.rowid, id_site);
    assert_eq!(
        site.description.as_deref(),
        Some("Site de test, effacé par le nettoyage")
    );
    assert_eq!(site.langue.as_deref(), Some("fr_FR"));
    assert_eq!(site.statut, STATUT_EN_LIGNE, "un site naît en ligne");
    assert!(site.est_en_ligne());
    assert!(site.date.is_some(), "la date de création doit être relue");
    assert_eq!(site.accueil, None, "aucune page d'accueil n'est désignée");
    assert_eq!(site.pages, 1, "le nombre de pages est compté par jointure");
    assert_eq!(site.vues, 0);

    // La liste des pages : tout sauf le contenu.
    let listes = pages(&pool, ENTITY, id_site)?;
    assert_eq!(listes.len(), 1);
    let liste = &listes[0];
    assert_eq!(liste.rowid, id_page);
    assert_eq!(liste.fk_website, id_site);
    assert_eq!(liste.adresse, adresse);
    assert_eq!(liste.titre.as_deref(), Some("Nous contacter"));
    assert_eq!(
        liste.description.as_deref(),
        Some("Téléphone, adresse et formulaire")
    );
    assert_eq!(
        liste.mots_cles.as_deref(),
        Some("contact, devis, maçonnerie")
    );
    assert_eq!(liste.langue.as_deref(), Some("fr_FR"));
    assert_eq!(liste.statut, STATUT_EN_LIGNE);
    assert_eq!(liste.reference_site.as_deref(), Some(reference.as_str()));
    assert_eq!(
        liste.contenu, None,
        "la liste ne doit pas charger le contenu : il peut peser des mégaoctets"
    );

    // La page seule : avec son contenu, à l'octet.
    let complete = page(&pool, ENTITY, id_page)?.ok_or("la page créée n'est pas relue")?;
    assert_eq!(complete.adresse, adresse);
    assert_eq!(complete.contenu.as_deref(), Some(contenu.as_str()));

    // Le type de conteneur est celui qu'on a écrit, et il tient dans un
    // varchar(16).
    let mut conn = pool.get_conn()?;
    let conteneurs: Vec<(String, String)> = conn.exec(
        "SELECT 'site', type_container FROM llx_website WHERE rowid = :site \
         UNION ALL SELECT 'page', type_container FROM llx_website_page WHERE rowid = :page",
        params! { "site" => id_site, "page" => id_page },
    )?;
    for (quoi, conteneur) in conteneurs {
        let attendu = if quoi == "site" { "website" } else { "page" };
        assert_eq!(conteneur, attendu, "type_container du {quoi}");
    }
    // Le pool rend ses connexions : on les rend avant d'en redemander une.
    drop(conn);

    nettoyer(&pool, &marque)?;
    assert!(
        sites(&pool, ENTITY, 500)?
            .iter()
            .all(|s| s.ref_ != reference),
        "le site devait partir"
    );
    Ok(())
}

/// Une référence vide, blanche ou démesurée ne s'écrit pas — et rien n'est créé.
#[test]
fn une_reference_de_site_vide_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("reference");
    nettoyer(&pool, &marque)?;

    for reference in ["", "   ", "\n\t"] {
        let erreur = creer_site(
            &pool,
            ENTITY,
            &NewSite {
                ref_: reference.to_string(),
                ..Default::default()
            },
        )
        .expect_err("une référence vide doit être refusée");
        assert!(
            erreur.to_string().contains("référence"),
            "message : {erreur}"
        );
    }

    // Au-delà de 128 octets, on refuse avant la base : la colonne est un
    // varchar(128), et le serveur couperait sans le dire.
    let erreur = creer_site(
        &pool,
        ENTITY,
        &NewSite {
            ref_: "a".repeat(db::site_web::REF_MAX + 1),
            ..Default::default()
        },
    )
    .expect_err("une référence de plus de 128 octets doit être refusée");
    assert!(erreur.to_string().contains("octets"), "message : {erreur}");

    // La langue d'un site est un varchar(8) : « français » en fait neuf.
    let erreur = creer_site(
        &pool,
        ENTITY,
        &NewSite {
            ref_: format!("{marque}-langue"),
            lang: Some("français".to_string()),
            ..Default::default()
        },
    )
    .expect_err("une langue de plus de 8 octets doit être refusée");
    assert!(erreur.to_string().contains("langue"), "message : {erreur}");

    // Aucun site de ce test ne doit exister : les refus ont eu lieu avant
    // l'écriture, pas après.
    assert!(
        sites(&pool, ENTITY, 500)?
            .iter()
            .all(|s| !s.ref_.starts_with(marque.as_str())),
        "un refus ne doit rien écrire"
    );
    Ok(())
}

/// La référence d'un site est unique **par entité** : la même dans deux entités
/// est légitime, deux fois dans la même ne l'est pas.
#[test]
fn une_reference_de_site_deja_prise_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("unicite");
    nettoyer(&pool, &marque)?;

    let reference = format!("{marque}-site");
    let id = site_de_test(&pool, &reference)?;

    let erreur = creer_site(
        &pool,
        ENTITY,
        &NewSite {
            ref_: reference.clone(),
            ..Default::default()
        },
    )
    .expect_err("deux sites de même référence dans la même entité doivent être refusés");
    assert!(erreur.to_string().contains("déjà"), "message : {erreur}");

    // Le même nom dans une autre entité : accepté, l'index est composite
    // (`ref`, `entity`).
    let ailleurs = creer_site(
        &pool,
        ENTITY + 1,
        &NewSite {
            ref_: reference.clone(),
            ..Default::default()
        },
    )?;
    assert!(ailleurs > 0);

    // Toujours un seul site de ce nom dans l'entité 1.
    let combien = sites(&pool, ENTITY, 500)?
        .iter()
        .filter(|s| s.ref_ == reference)
        .count();
    assert_eq!(combien, 1);

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_website WHERE rowid = :id",
        params! { "id" => ailleurs },
    )?;
    drop(conn);

    assert_eq!(site_relu(&pool, &reference)?.rowid, id);
    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Une page sur un site qui n'existe pas est refusée : l'entité d'une page vient
/// de son site, pas d'elle-même.
#[test]
fn une_page_sur_un_site_inexistant_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("orpheline");
    nettoyer(&pool, &marque)?;

    for site in [0i64, 999_999_999] {
        let erreur = creer_page(
            &pool,
            ENTITY,
            &NewPage {
                fk_website: site,
                pageurl: format!("{marque}-contact"),
                content: "<p>Bonjour</p>".to_string(),
                status: STATUT_EN_LIGNE,
                ..Default::default()
            },
        )
        .expect_err("une page sans site ne se crée pas");
        assert!(
            erreur.to_string().contains("n'existe pas"),
            "message : {erreur}"
        );
    }

    // Le site existe, mais dans une autre entité : refusé aussi.
    let ailleurs = site_de_test(&pool, &format!("{marque}-ailleurs"))?;
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_website SET entity = :autre WHERE rowid = :id",
        params! { "autre" => ENTITY + 1, "id" => ailleurs },
    )?;
    drop(conn);
    let erreur = creer_page(
        &pool,
        ENTITY,
        &NewPage {
            fk_website: ailleurs,
            pageurl: format!("{marque}-contact"),
            content: "<p>Bonjour</p>".to_string(),
            status: STATUT_EN_LIGNE,
            ..Default::default()
        },
    )
    .expect_err("une page ne se rattache pas au site d'une autre entité");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message : {erreur}"
    );

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Une adresse vide ou démesurée, un contenu vide : refusés.
#[test]
fn une_url_de_page_vide_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("adresse");
    nettoyer(&pool, &marque)?;

    let id_site = site_de_test(&pool, &format!("{marque}-site"))?;

    for adresse in ["", "   ", "\n\t"] {
        let erreur = creer_page(
            &pool,
            ENTITY,
            &NewPage {
                fk_website: id_site,
                pageurl: adresse.to_string(),
                content: "<p>Bonjour</p>".to_string(),
                status: STATUT_EN_LIGNE,
                ..Default::default()
            },
        )
        .expect_err("une adresse vide doit être refusée");
        assert!(erreur.to_string().contains("adresse"), "message : {erreur}");
    }

    let erreur = creer_page(
        &pool,
        ENTITY,
        &NewPage {
            fk_website: id_site,
            pageurl: "a".repeat(db::site_web::ADRESSE_MAX + 1),
            content: "<p>Bonjour</p>".to_string(),
            status: STATUT_EN_LIGNE,
            ..Default::default()
        },
    )
    .expect_err("une adresse de plus de 255 octets doit être refusée");
    assert!(erreur.to_string().contains("octets"), "message : {erreur}");

    let erreur = creer_page(
        &pool,
        ENTITY,
        &NewPage {
            fk_website: id_site,
            pageurl: format!("{marque}-vide"),
            content: "   ".to_string(),
            status: STATUT_EN_LIGNE,
            ..Default::default()
        },
    )
    .expect_err("un contenu vide doit être refusé");
    assert!(erreur.to_string().contains("contenu"), "message : {erreur}");

    assert!(
        pages(&pool, ENTITY, id_site)?.is_empty(),
        "un refus ne doit rien écrire"
    );
    nettoyer(&pool, &marque)?;
    Ok(())
}

/// L'adresse est l'identifiant public de la page : la même deux fois dans un
/// site, c'est deux pages sous un seul nom.
#[test]
fn une_url_en_double_dans_le_meme_site_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("double");
    nettoyer(&pool, &marque)?;

    let id_site = site_de_test(&pool, &format!("{marque}-site"))?;
    let adresse = format!("{marque}-contact");
    let id_page = page_de_test(&pool, id_site, &adresse)?;

    let erreur = page_de_test(&pool, id_site, &adresse)
        .expect_err("la même adresse deux fois dans un site doit être refusée");
    let message = erreur.to_string();
    assert!(message.contains("déjà prise"), "message : {message}");
    assert!(
        message.contains("identifiant public"),
        "le message doit dire pourquoi l'adresse compte : {message}"
    );

    let listes = pages(&pool, ENTITY, id_site)?;
    assert_eq!(listes.len(), 1, "le refus ne doit pas créer de page");
    assert_eq!(listes[0].rowid, id_page);

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// La même adresse dans deux sites différents est légitime : c'est l'index
/// `(fk_website, pageurl)` qui le dit, pas une règle inventée.
#[test]
fn la_meme_url_est_acceptee_dans_deux_sites_differents() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("deux-sites");
    nettoyer(&pool, &marque)?;

    let premier = site_de_test(&pool, &format!("{marque}-a"))?;
    let second = site_de_test(&pool, &format!("{marque}-b"))?;

    let adresse = format!("{marque}-contact");
    let page_a = page_de_test(&pool, premier, &adresse)?;
    let page_b = page_de_test(&pool, second, &adresse)?;
    assert_ne!(page_a, page_b);

    for (site, attendu) in [(premier, page_a), (second, page_b)] {
        let listes = pages(&pool, ENTITY, site)?;
        assert_eq!(listes.len(), 1, "chaque site a sa page");
        assert_eq!(listes[0].rowid, attendu);
        assert_eq!(listes[0].adresse, adresse);
    }
    assert_eq!(site_relu(&pool, &format!("{marque}-a"))?.pages, 1);
    assert_eq!(site_relu(&pool, &format!("{marque}-b"))?.pages, 1);

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Publier puis republier est refusé : rien à faire, et le dire vaut mieux que
/// de laisser croire à une republication.
#[test]
fn publier_puis_republier_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("statut");
    nettoyer(&pool, &marque)?;

    let id_site = site_de_test(&pool, &format!("{marque}-site"))?;
    let id_page = creer_page(
        &pool,
        ENTITY,
        &NewPage {
            fk_website: id_site,
            pageurl: format!("{marque}-brouillon"),
            title: Some("Page en préparation".to_string()),
            content: "<p>À relire</p>".to_string(),
            status: STATUT_HORS_LIGNE,
            ..Default::default()
        },
    )?;

    let brouillon = page(&pool, ENTITY, id_page)?.ok_or("page non relue")?;
    assert_eq!(brouillon.statut, STATUT_HORS_LIGNE);
    assert!(!brouillon.est_en_ligne());

    publier_page(&pool, ENTITY, id_page)?;
    assert_eq!(
        page(&pool, ENTITY, id_page)?
            .ok_or("page non relue")?
            .statut,
        STATUT_EN_LIGNE
    );

    let erreur = publier_page(&pool, ENTITY, id_page).expect_err("publier deux fois est refusé");
    assert!(
        erreur.to_string().contains("déjà en ligne"),
        "message : {erreur}"
    );

    // Dépublier : la page quitte le site sans être perdue.
    depublier_page(&pool, ENTITY, id_page)?;
    assert_eq!(
        page(&pool, ENTITY, id_page)?
            .ok_or("page non relue")?
            .statut,
        STATUT_HORS_LIGNE
    );

    let erreur =
        depublier_page(&pool, ENTITY, id_page).expect_err("dépublier deux fois est refusé");
    assert!(
        erreur.to_string().contains("déjà hors ligne"),
        "message : {erreur}"
    );

    assert!(
        publier_page(&pool, ENTITY, 999_999_999).is_err(),
        "publier une page absente doit être signalé"
    );

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Renommer vers une adresse déjà prise dans le site est refusé ; vers une
/// adresse libre, accepté.
#[test]
fn renommer_vers_une_url_prise_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("renommer");
    nettoyer(&pool, &marque)?;

    let id_site = site_de_test(&pool, &format!("{marque}-site"))?;
    let occupante = format!("{marque}-contact");
    page_de_test(&pool, id_site, &occupante)?;
    let mobile = format!("{marque}-mentions");
    let id_mobile = page_de_test(&pool, id_site, &mobile)?;

    let erreur = renommer_page(&pool, ENTITY, id_mobile, &occupante)
        .expect_err("renommer vers une adresse prise doit être refusé");
    let message = erreur.to_string();
    assert!(message.contains("déjà prise"), "message : {message}");
    assert!(
        message.contains("identifiant public"),
        "le message doit dire pourquoi l'adresse compte : {message}"
    );
    // Rien n'a bougé.
    assert_eq!(
        page(&pool, ENTITY, id_mobile)?
            .ok_or("page non relue")?
            .adresse,
        mobile
    );

    // Vers une adresse libre : accepté, et relu.
    let libre = format!("{marque}-a-propos");
    renommer_page(&pool, ENTITY, id_mobile, &libre)?;
    assert_eq!(
        page(&pool, ENTITY, id_mobile)?
            .ok_or("page non relue")?
            .adresse,
        libre
    );

    // Vers sa propre adresse : rien à faire, et on le dit.
    let erreur = renommer_page(&pool, ENTITY, id_mobile, &libre)
        .expect_err("renommer vers la même adresse doit être signalé");
    assert!(erreur.to_string().contains("déjà"), "message : {erreur}");

    // Une adresse vide est refusée, comme à la création.
    let erreur = renommer_page(&pool, ENTITY, id_mobile, "   ")
        .expect_err("une adresse vide doit être refusée");
    assert!(
        erreur.to_string().contains("obligatoire"),
        "message : {erreur}"
    );

    assert!(
        renommer_page(&pool, ENTITY, 999_999_999, &libre).is_err(),
        "renommer une page absente doit être signalé"
    );

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Un site qui a des pages ne se supprime pas ; une page d'abord, le site
/// ensuite.
#[test]
fn supprimer_un_site_non_vide_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("supprimer");
    nettoyer(&pool, &marque)?;

    let reference = format!("{marque}-site");
    let id_site = site_de_test(&pool, &reference)?;
    let premiere = page_de_test(&pool, id_site, &format!("{marque}-a"))?;
    let seconde = page_de_test(&pool, id_site, &format!("{marque}-b"))?;

    let erreur = supprimer_site(&pool, ENTITY, id_site)
        .expect_err("un site avec des pages ne se supprime pas");
    let message = erreur.to_string();
    assert!(message.contains("page"), "message : {message}");
    assert!(
        message.contains("accident"),
        "le message doit dire pourquoi on refuse : {message}"
    );
    assert_eq!(
        site_relu(&pool, &reference)?.pages,
        2,
        "le site et ses deux pages sont toujours là"
    );

    // Une page à la fois. La première part, la seconde reste.
    supprimer_page(&pool, ENTITY, premiere)?;
    assert!(
        page(&pool, ENTITY, premiere)?.is_none(),
        "la page devait partir"
    );
    assert_eq!(site_relu(&pool, &reference)?.pages, 1);

    let erreur = supprimer_page(&pool, ENTITY, premiere)
        .expect_err("supprimer deux fois n'est pas un succès silencieux");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message : {erreur}"
    );

    supprimer_page(&pool, ENTITY, seconde)?;
    assert!(pages(&pool, ENTITY, id_site)?.is_empty());

    // Le site, maintenant vide, part.
    supprimer_site(&pool, ENTITY, id_site)?;
    assert!(
        sites(&pool, ENTITY, 500)?
            .iter()
            .all(|s| s.rowid != id_site),
        "le site devait partir"
    );

    let erreur = supprimer_site(&pool, ENTITY, id_site)
        .expect_err("supprimer un site absent doit être signalé");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message : {erreur}"
    );

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Supprimer la page d'accueil ne laisse pas le site pointer dans le vide : le
/// cœur de Dolibarr, lui, n'y touche pas.
#[test]
fn detacher_la_page_d_accueil_du_site() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let marque = marque("accueil");
    nettoyer(&pool, &marque)?;

    let reference = format!("{marque}-site");
    let id_site = site_de_test(&pool, &reference)?;
    let accueil = page_de_test(&pool, id_site, &format!("{marque}-accueil"))?;

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_website SET fk_default_home = :page WHERE rowid = :site",
        params! { "page" => accueil, "site" => id_site },
    )?;
    drop(conn);
    assert_eq!(site_relu(&pool, &reference)?.accueil, Some(accueil));

    supprimer_page(&pool, ENTITY, accueil)?;
    assert_eq!(
        site_relu(&pool, &reference)?.accueil,
        None,
        "un site ne doit pas garder une page d'accueil disparue"
    );

    nettoyer(&pool, &marque)?;
    Ok(())
}
