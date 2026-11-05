//! Base de connaissances : création, relecture, recherche, publication.
//!
//! Tout se passe dans l'entité 1, et chaque test porte sa **propre marque**
//! (`ZZ-KB-<pid>-<sujet>`) : les tests tournent en parallèle, deux tests qui
//! partageraient le même texte se détruiraient mutuellement au nettoyage. Le
//! ménage se fait avant autant qu'après, pour qu'un test interrompu ne fasse pas
//! échouer le suivant.
//!
//! Les tests sont aussi sérialisés par un verrou : la numérotation lit
//! `MAX(ref) + 1` sur la même table, et deux créations simultanées se
//! disputeraient la même référence plutôt que d'écrire des choses différentes.

use std::error::Error;

use db::connaissances::{
    create, depublier, depuis_ticket, get, list, modifier, publier, rechercher, supprimer,
    NewFiche, QUESTION_MAX, STATUT_BROUILLON, STATUT_PUBLIEE,
};
use mysql::prelude::Queryable;
use mysql::{params, Pool};

const ENTITY: i64 = 1;

/// La table de numérotation est commune : on sérialise les tests.
static SERIE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Prend le verrou de sérialisation.
///
/// Un test qui panique en le tenant empoisonne le verrou ; sans le
/// `unwrap_or_else`, tous les tests suivants échoueraient sur le poison au lieu
/// de tester ce qu'ils ont à tester.
fn serie() -> std::sync::MutexGuard<'static, ()> {
    SERIE
        .lock()
        .unwrap_or_else(|empoisonne| empoisonne.into_inner())
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

/// La marque du test : un préfixe unique, sans `%` ni `_` (le nettoyage s'en
/// sert comme motif `LIKE`).
fn marque(sujet: &str) -> String {
    format!("ZZ-KB-{}-{sujet}", std::process::id())
}

/// L'auteur des fiches : un utilisateur réel de l'entité 1.
///
/// `llx_knowledgemanagement_knowledgerecord.fk_user_creat` est `NOT NULL` et
/// porte une clé étrangère : sans aucun utilisateur en base, ces tests n'ont
/// rien à vérifier et s'ignorent.
fn auteur(pool: &Pool) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity ORDER BY rowid ASC LIMIT 1",
        params! { "entity" => ENTITY },
    )
    .ok()?
}

/// Efface tout ce que les tests de ce fichier ont pu laisser.
///
/// Le ménage se fait par marque et non par identifiant : un test interrompu
/// laisse des lignes dont on ne connaît plus le `rowid`, et elles feraient
/// échouer le décompte du test suivant.
fn nettoyer(pool: &Pool, marque: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_knowledgemanagement_knowledgerecord \
         WHERE entity = :entity AND question LIKE :motif",
        params! { "entity" => ENTITY, "motif" => format!("{marque}%") },
    )?;
    for id in ids {
        // Les étiquettes portent une clé étrangère vers la fiche : elles partent
        // d'abord, sinon la suppression de la fiche échouerait.
        conn.exec_drop(
            "DELETE FROM llx_categorie_knowledgemanagement WHERE fk_knowledgemanagement = :id",
            params! { "id" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_knowledgemanagement_knowledgerecord_extrafields WHERE fk_object = :id",
            params! { "id" => id },
        )?;
        conn.exec_drop(
            "DELETE FROM llx_knowledgemanagement_knowledgerecord WHERE rowid = :id",
            params! { "id" => id },
        )?;
    }
    conn.exec_drop(
        "DELETE FROM llx_ticket WHERE entity = :entity AND subject LIKE :motif",
        params! { "entity" => ENTITY, "motif" => format!("{marque}%") },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_c_ticket_category WHERE label = :label",
        params! { "label" => marque },
    )?;
    // Les catégories créées par le test des étiquettes (`llx_categorie`). Leurs
    // étiquettes partent d'abord : `fk_categorie_knowledgemanagement_categorie_rowid`
    // refuserait de supprimer une catégorie encore utilisée — cas d'un test
    // interrompu avant sa suppression de fiche.
    conn.exec_drop(
        "DELETE FROM llx_categorie_knowledgemanagement WHERE fk_categorie IN \
           (SELECT rowid FROM llx_categorie WHERE entity = :entity AND label = :label)",
        params! { "entity" => ENTITY, "label" => marque },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_categorie WHERE entity = :entity AND label = :label",
        params! { "entity" => ENTITY, "label" => marque },
    )?;
    Ok(())
}

/// Une catégorie de tickets propre au test.
///
/// `llx_c_ticket_category` n'est pas livrée remplie avec la base : sans cela, la
/// colonne `fk_c_ticket_category` ne serait jamais exercée. La ligne est effacée
/// par [`nettoyer`], qui reconnaît la catégorie à son libellé.
fn categorie_de_test(pool: &Pool, marque: &str) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let code = format!("ZZKB{}", std::process::id());
    // `uk_code (code, entity)` est un index unique : un essai interrompu aurait
    // laissé la ligne, et l'insertion suivante échouerait sur un doublon.
    conn.exec_drop(
        "DELETE FROM llx_c_ticket_category WHERE entity = :entity AND code = :code",
        params! { "entity" => ENTITY, "code" => code.as_str() },
    )?;
    conn.exec_drop(
        "INSERT INTO llx_c_ticket_category (entity, code, label) \
         VALUES (:entity, :code, :label)",
        params! { "entity" => ENTITY, "code" => code.as_str(), "label" => marque },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Une fiche de test, réduite à une question et un auteur.
fn fiche(question: String, createur: i64) -> NewFiche {
    NewFiche {
        question,
        fk_user_creat: createur,
        ..Default::default()
    }
}

/// Créer, relire : les champs écrits reviennent, la référence est une `KB`.
#[test]
fn une_fiche_se_cree_et_se_relit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _serie = serie();
    let Some(createur) = auteur(&pool) else {
        eprintln!("aucun utilisateur dans l'entité 1 : test ignoré");
        return Ok(());
    };
    let marque = marque("lecture");
    nettoyer(&pool, &marque)?;

    let new = NewFiche {
        question: format!("{marque} : comment enlever une tache de rouille ?"),
        answer: Some("Acide phosphorique, puis rinçage à l'eau claire.".to_string()),
        url: Some("https://maconnerie-architecture.fr/faq/rouille".to_string()),
        lang: Some("fr_FR".to_string()),
        fk_ticket: None,
        fk_c_ticket_category: None,
        fk_user_creat: createur,
    };
    let id = create(&pool, ENTITY, &new)?;

    let lue = get(&pool, ENTITY, id)?.ok_or("la fiche créée n'est pas relue")?;
    assert!(
        lue.ref_.starts_with("KB"),
        "référence inattendue : {}",
        lue.ref_
    );
    assert_eq!(lue.rowid, id);
    assert_eq!(lue.question, new.question);
    assert_eq!(
        lue.reponse.as_deref(),
        Some("Acide phosphorique, puis rinçage à l'eau claire.")
    );
    assert_eq!(
        lue.url.as_deref(),
        Some("https://maconnerie-architecture.fr/faq/rouille")
    );
    assert_eq!(lue.langue.as_deref(), Some("fr_FR"));
    assert_eq!(lue.statut, STATUT_PUBLIEE, "une fiche naît publiée");
    assert!(lue.est_publiee());
    assert!(lue.date.is_some(), "la date de création doit être relue");
    assert_eq!(lue.fk_ticket, None);
    assert_eq!(lue.ticket_ref, None);
    assert_eq!(lue.fk_user_creat, Some(createur));

    // La seconde fiche doit passer devant la première : même seconde, `rowid`
    // tranche.
    let id2 = create(&pool, ENTITY, &fiche(format!("{marque} : suite"), createur))?;
    let liste = list(&pool, ENTITY, 500, 0)?;
    let les_notres: Vec<i64> = liste
        .iter()
        .filter(|f| f.question.starts_with(&marque))
        .map(|f| f.rowid)
        .collect();
    assert_eq!(
        les_notres,
        vec![id2, id],
        "la liste doit rendre la plus récente d'abord"
    );

    nettoyer(&pool, &marque)?;
    assert!(get(&pool, ENTITY, id)?.is_none(), "la fiche devait partir");
    Ok(())
}

/// Une question vide, blanche ou démesurée ne s'écrit pas.
#[test]
fn une_question_vide_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _serie = serie();
    let Some(createur) = auteur(&pool) else {
        return Ok(());
    };

    for question in ["", "   ", "\n\t "] {
        let erreur = create(&pool, ENTITY, &fiche(question.to_string(), createur))
            .expect_err("une question vide doit être refusée");
        assert!(
            erreur.to_string().contains("question"),
            "message : {erreur}"
        );
    }

    // Au-delà de 65 000 octets, on refuse avant la base : la colonne est un
    // `text`, et le serveur tronquerait sans le dire.
    let trop_longue = "a".repeat(QUESTION_MAX + 1);
    let erreur = create(&pool, ENTITY, &fiche(trop_longue, createur))
        .expect_err("une question de plus de 65 000 octets doit être refusée");
    assert!(erreur.to_string().contains("octets"), "message : {erreur}");
    Ok(())
}

/// Le champ `url` est un lien cliquable, pas un chemin de fichier.
#[test]
fn une_url_sans_http_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _serie = serie();
    let Some(createur) = auteur(&pool) else {
        return Ok(());
    };
    let marque = marque("url");
    nettoyer(&pool, &marque)?;

    let sans_schema = NewFiche {
        question: format!("{marque} : adresse sans schéma"),
        url: Some("maconnerie-architecture.fr/faq".to_string()),
        fk_user_creat: createur,
        ..Default::default()
    };
    let erreur =
        create(&pool, ENTITY, &sans_schema).expect_err("un chemin sans schéma doit être refusé");
    assert!(erreur.to_string().contains("http"), "message : {erreur}");

    let en_ftp = NewFiche {
        question: format!("{marque} : adresse ftp"),
        url: Some("ftp://exemple.test/notice.pdf".to_string()),
        fk_user_creat: createur,
        ..Default::default()
    };
    assert!(
        create(&pool, ENTITY, &en_ftp).is_err(),
        "ftp n'est pas un lien cliquable"
    );

    // Une adresse blanche vaut absence : la fiche se crée sans lien.
    let sans_lien = NewFiche {
        question: format!("{marque} : adresse blanche"),
        url: Some("   ".to_string()),
        fk_user_creat: createur,
        ..Default::default()
    };
    let id = create(&pool, ENTITY, &sans_lien)?;
    let lue = get(&pool, ENTITY, id)?.ok_or("fiche non relue")?;
    assert_eq!(lue.url, None);

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Un auteur absent (0) ou inconnu est refusé : la colonne porte une clé
/// étrangère, et 0 ferait échouer la base avec un message incompréhensible.
#[test]
fn un_auteur_inexistant_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _serie = serie();
    let marque = marque("auteur");

    let erreur = create(&pool, ENTITY, &fiche(format!("{marque} : sans auteur"), 0))
        .expect_err("un auteur à 0 doit être refusé");
    assert!(erreur.to_string().contains("auteur"), "message : {erreur}");

    let erreur = create(
        &pool,
        ENTITY,
        &fiche(format!("{marque} : auteur fantôme"), 999_999_999),
    )
    .expect_err("un auteur inexistant doit être refusé");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message : {erreur}"
    );
    Ok(())
}

/// Un lien vers un ticket ou une catégorie qui n'existe pas est refusé ; 0 vaut
/// « aucun lien ».
#[test]
fn des_liens_inconnus_sont_refuses() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _serie = serie();
    let Some(createur) = auteur(&pool) else {
        return Ok(());
    };
    let marque = marque("liens");
    nettoyer(&pool, &marque)?;

    let ticket_inconnu = NewFiche {
        question: format!("{marque} : ticket mort"),
        fk_ticket: Some(999_999_999),
        fk_user_creat: createur,
        ..Default::default()
    };
    assert!(
        create(&pool, ENTITY, &ticket_inconnu).is_err(),
        "un ticket inexistant doit être refusé"
    );

    let categorie_inconnue = NewFiche {
        question: format!("{marque} : catégorie morte"),
        fk_c_ticket_category: Some(999_999_999),
        fk_user_creat: createur,
        ..Default::default()
    };
    assert!(
        create(&pool, ENTITY, &categorie_inconnue).is_err(),
        "une catégorie inexistante doit être refusée"
    );

    // 0 est ce que renvoient les formulaires quand on ne choisit rien.
    let aucun_lien = NewFiche {
        question: format!("{marque} : aucun lien"),
        fk_ticket: Some(0),
        fk_c_ticket_category: Some(0),
        fk_user_creat: createur,
        ..Default::default()
    };
    let id = create(&pool, ENTITY, &aucun_lien)?;
    let lue = get(&pool, ENTITY, id)?.ok_or("fiche non relue")?;
    assert_eq!(lue.fk_ticket, None);
    assert_eq!(lue.fk_c_ticket_category, None);

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Modifier change le texte, jamais la référence ni la date de création.
#[test]
fn modifier_ne_change_pas_la_reference() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _serie = serie();
    let Some(createur) = auteur(&pool) else {
        return Ok(());
    };
    let marque = marque("modifier");
    nettoyer(&pool, &marque)?;

    let id = create(
        &pool,
        ENTITY,
        &fiche(format!("{marque} : question"), createur),
    )?;
    let avant = get(&pool, ENTITY, id)?.ok_or("fiche non relue")?;
    let categorie = categorie_de_test(&pool, &marque)?;

    modifier(
        &pool,
        ENTITY,
        id,
        &NewFiche {
            question: format!("{marque} : question reformulée"),
            answer: Some("Réponse corrigée.".to_string()),
            url: Some("https://exemple.test/fiche".to_string()),
            lang: Some("en_US".to_string()),
            // `fk_user_creat` est ignoré par la modification : on le laisse à 0
            // exprès, pour prouver que ce champ ne bloque rien ici.
            fk_ticket: None,
            fk_c_ticket_category: Some(categorie),
            fk_user_creat: 0,
        },
    )?;

    let apres = get(&pool, ENTITY, id)?.ok_or("fiche disparue après modification")?;
    assert_eq!(apres.ref_, avant.ref_, "la référence ne se réécrit pas");
    assert_eq!(
        apres.date, avant.date,
        "la date de création ne se réécrit pas"
    );
    assert_eq!(apres.question, format!("{marque} : question reformulée"));
    assert_eq!(apres.reponse.as_deref(), Some("Réponse corrigée."));
    assert_eq!(apres.url.as_deref(), Some("https://exemple.test/fiche"));
    assert_eq!(apres.langue.as_deref(), Some("en_US"));
    assert_eq!(apres.fk_c_ticket_category, Some(categorie));
    assert_eq!(
        apres.fk_user_creat, avant.fk_user_creat,
        "l'auteur d'origine reste"
    );

    // Une adresse invalide est refusée aussi en modification, et rien ne bouge.
    let invalide = NewFiche {
        question: format!("{marque} : brouillon de correction"),
        url: Some("pas-une-url".to_string()),
        fk_user_creat: createur,
        ..Default::default()
    };
    assert!(modifier(&pool, ENTITY, id, &invalide).is_err());
    let intacte = get(&pool, ENTITY, id)?.ok_or("fiche disparue")?;
    assert_eq!(intacte.question, format!("{marque} : question reformulée"));

    let fantome = fiche("fantôme".to_string(), createur);
    assert!(
        modifier(&pool, ENTITY, 999_999_999, &fantome).is_err(),
        "modifier une fiche absente doit être signalé"
    );

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
    let _serie = serie();
    let Some(createur) = auteur(&pool) else {
        return Ok(());
    };
    let marque = marque("statut");
    nettoyer(&pool, &marque)?;

    let id = create(
        &pool,
        ENTITY,
        &fiche(format!("{marque} : statut"), createur),
    )?;
    assert_eq!(
        get(&pool, ENTITY, id)?.ok_or("fiche non relue")?.statut,
        STATUT_PUBLIEE
    );

    // Dépublier : elle quitte la base de connaissances sans être supprimée.
    depublier(&pool, ENTITY, id)?;
    assert_eq!(
        get(&pool, ENTITY, id)?.ok_or("fiche non relue")?.statut,
        STATUT_BROUILLON
    );

    let erreur = depublier(&pool, ENTITY, id).expect_err("dépublier deux fois doit être refusé");
    assert!(
        erreur.to_string().contains("brouillon"),
        "message : {erreur}"
    );

    publier(&pool, ENTITY, id)?;
    assert_eq!(
        get(&pool, ENTITY, id)?.ok_or("fiche non relue")?.statut,
        STATUT_PUBLIEE
    );

    let erreur = publier(&pool, ENTITY, id).expect_err("publier deux fois doit être refusé");
    assert!(erreur.to_string().contains("publiée"), "message : {erreur}");

    assert!(
        publier(&pool, ENTITY, 999_999_999).is_err(),
        "publier une fiche absente doit être signalé"
    );

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// La recherche voit la question **et** la réponse, sans tenir compte de la
/// casse.
#[test]
fn rechercher_trouve_par_la_question_et_par_la_reponse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _serie = serie();
    let Some(createur) = auteur(&pool) else {
        return Ok(());
    };
    let marque = marque("recherche");
    nettoyer(&pool, &marque)?;

    let par_question = create(
        &pool,
        ENTITY,
        &NewFiche {
            question: format!("{marque}-question : quel dosage pour un mortier bâtard ?"),
            answer: Some("Un volume de chaux pour deux volumes de sable.".to_string()),
            fk_user_creat: createur,
            ..Default::default()
        },
    )?;
    let par_reponse = create(
        &pool,
        ENTITY,
        &NewFiche {
            question: format!("{marque}-reponse : d'où viennent les taches blanches ?"),
            // La marque est glissée dans la réponse : elle n'apparaît nulle part
            // ailleurs, ce qui rend la recherche par la réponse certaine.
            answer: Some(format!(
                "C'est de la Kalicinite, un sulfate qui migre avec l'humidité. \
                 Voir {marque}-fiche-blanche."
            )),
            fk_user_creat: createur,
            ..Default::default()
        },
    )?;

    // Par la question.
    let trouves = rechercher(&pool, ENTITY, &format!("{marque}-question"), 50)?;
    assert_eq!(trouves.len(), 1, "une seule fiche porte ce texte");
    assert_eq!(trouves[0].rowid, par_question);

    // La casse ne compte pas, dans un sens comme dans l'autre.
    let majuscules = rechercher(&pool, ENTITY, &format!("{marque}-QUESTION"), 50)?;
    assert_eq!(majuscules.len(), 1);
    assert_eq!(majuscules[0].rowid, par_question);

    // Par la réponse seule : le texte cherché n'existe que dans l'`answer`, et
    // il est tapé en majuscules alors que la fiche l'écrit en minuscules.
    let par_reponse_seule = rechercher(&pool, ENTITY, &format!("{}-FICHE-BLANCHE", marque), 50)?;
    assert_eq!(
        par_reponse_seule.len(),
        1,
        "la recherche doit voir la réponse, sans tenir compte de la casse"
    );
    assert_eq!(par_reponse_seule[0].rowid, par_reponse);

    // Un mot de la réponse se trouve aussi, sans être confondu avec la question.
    let mot = rechercher(&pool, ENTITY, "kalicinite", 50)?;
    assert!(mot.iter().any(|f| f.rowid == par_reponse));
    assert!(mot.iter().all(|f| f.rowid != par_question));

    // Un terme vide n'est pas une demande de tout afficher.
    assert!(
        rechercher(&pool, ENTITY, "   ", 50)?.is_empty(),
        "une recherche blanche ne doit rien rendre"
    );

    // Un terme qui ne correspond à rien ne rend rien.
    assert!(rechercher(&pool, ENTITY, &format!("{marque}-introuvable"), 50)?.is_empty());

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Les jokers de `LIKE` sont échappés : un `%` tapé par l'utilisateur reste un
/// pour cent.
///
/// C'est le test qui protège la fonction : sans échappement, la recherche sur
/// `%` ramènerait **toute** la base et personne ne le verrait.
#[test]
fn rechercher_echappe_les_jokers() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _serie = serie();
    let Some(createur) = auteur(&pool) else {
        return Ok(());
    };
    let marque = marque("jokers");
    nettoyer(&pool, &marque)?;

    // Aucun joker dans ce texte : il ne doit jamais sortir sur une recherche de
    // joker seul.
    let sans_joker = create(
        &pool,
        ENTITY,
        &fiche(format!("{marque}-sans-joker : quel dosage ?"), createur),
    )?;
    // Un pour cent et un souligné littéraux dans le texte.
    let avec_joker = create(
        &pool,
        ENTITY,
        &NewFiche {
            question: format!("{marque}-avec-joker : remise de 20 % sur la fourniture"),
            answer: Some(format!("{marque} : le prix_au_m2 est ferme.")),
            fk_user_creat: createur,
            ..Default::default()
        },
    )?;

    // Un pour cent dans le terme cherché trouve la fiche qui en contient un.
    let trouves = rechercher(
        &pool,
        ENTITY,
        &format!("{marque}-avec-joker : remise de 20 %"),
        50,
    )?;
    assert_eq!(trouves.len(), 1, "le % littéral doit être trouvé");
    assert_eq!(trouves[0].rowid, avec_joker);

    // Un pour cent seul ne ramène pas la fiche qui n'en contient pas.
    let tous = rechercher(&pool, ENTITY, "%", 500)?;
    assert!(
        tous.iter().all(|f| f.rowid != sans_joker),
        "un % seul ramène la base entière : le terme n'est pas échappé"
    );

    // Le souligné est un caractère, pas « n'importe quoi » : `sans-joker` ne
    // doit pas être trouvé par `san_`.
    let souligne = rechercher(&pool, ENTITY, &format!("{marque}-san_"), 50)?;
    assert!(
        souligne.is_empty(),
        "un _ s'est comporté comme un joker : le terme n'est pas échappé"
    );

    // Un souligné littéral dans le texte se trouve quand on le tape : l'avoir
    // échappé ne doit pas rendre les recherches légitimes impossibles.
    let trouves = rechercher(&pool, ENTITY, &format!("{marque} : le prix_au_m2"), 50)?;
    assert_eq!(trouves.len(), 1, "un souligné littéral doit se trouver");
    assert_eq!(trouves[0].rowid, avec_joker);

    // Un antislash ne casse pas la requête : c'est un caractère comme un autre.
    let _ = rechercher(&pool, ENTITY, "\\", 50)?;

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Une fiche née d'un ticket porte le lien, et le ticket n'est pas touché.
#[test]
fn depuis_ticket_pose_le_lien() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _serie = serie();
    let Some(_createur) = auteur(&pool) else {
        return Ok(());
    };
    let marque = marque("ticket");
    nettoyer(&pool, &marque)?;

    let ticket = db::ticket::creer(
        &pool,
        ENTITY,
        None,
        &format!("{marque} : fissure sur un linteau"),
        Some("Une fissure est apparue au-dessus de la baie."),
        None,
    )?;
    let avant = db::ticket::get(&pool, ENTITY, ticket)?.ok_or("ticket non relu")?;

    let id = depuis_ticket(
        &pool,
        ENTITY,
        ticket,
        &format!("{marque} : que faire d'une fissure sur un linteau ?"),
        "On étaye, on reprend en sous-œuvre avec un IPE, puis on rebouche.",
    )?;

    let fiche = get(&pool, ENTITY, id)?.ok_or("fiche non relue")?;
    assert_eq!(fiche.fk_ticket, Some(ticket), "le lien au ticket est posé");
    assert_eq!(
        fiche.ticket_ref.as_deref(),
        Some(avant.ref_.as_str()),
        "la référence du ticket est relue"
    );
    assert_eq!(fiche.statut, STATUT_PUBLIEE);
    assert!(
        fiche.fk_user_creat.is_some(),
        "une fiche a toujours un auteur"
    );
    assert!(fiche.reponse.is_some(), "la réponse du ticket est reprise");

    // Le ticket, lui, n'a pas bougé : une fiche de connaissance n'est pas une
    // réponse au client.
    let apres = db::ticket::get(&pool, ENTITY, ticket)?.ok_or("ticket disparu")?;
    assert_eq!(apres.statut, db::ticket::STATUT_NOUVEAU);
    assert_eq!(apres.sujet, avant.sujet);
    assert_eq!(apres.message, avant.message);
    assert_eq!(apres.date, avant.date);

    // Sans réponse, la fiche ne garderait rien : c'est refusé.
    assert!(
        depuis_ticket(&pool, ENTITY, ticket, "question", "   ").is_err(),
        "une réponse vide doit être refusée"
    );
    // Un ticket inexistant est signalé, pas enregistré en silence.
    assert!(
        depuis_ticket(&pool, ENTITY, 999_999_999, "question", "réponse").is_err(),
        "un ticket inexistant doit être refusé"
    );

    nettoyer(&pool, &marque)?;
    Ok(())
}

/// Supprimer une fiche, y compris étiquetée.
#[test]
fn supprimer_une_fiche() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _serie = serie();
    let Some(createur) = auteur(&pool) else {
        return Ok(());
    };
    let marque = marque("supprimer");
    nettoyer(&pool, &marque)?;

    let id = create(
        &pool,
        ENTITY,
        &fiche(format!("{marque} : à jeter"), createur),
    )?;

    // Une fiche étiquetée : `llx_categorie_knowledgemanagement` porte une clé
    // étrangère vers la fiche, la suppression doit retirer l'étiquette d'abord.
    // La base de test ne contient aucune catégorie : on en crée une, que
    // `nettoyer` efface. Sans cela, ce chemin ne serait jamais exercé.
    let etiquette = db::categorie::creer(
        &pool,
        ENTITY,
        db::categorie::Famille::Client,
        &marque,
        "",
        None,
        0,
    )?;
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_categorie_knowledgemanagement (fk_categorie, fk_knowledgemanagement) \
         VALUES (:categorie, :fiche)",
        params! { "categorie" => etiquette, "fiche" => id },
    )?;

    supprimer(&pool, ENTITY, id)?;
    assert!(get(&pool, ENTITY, id)?.is_none(), "la fiche devait partir");
    let restantes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_categorie_knowledgemanagement WHERE fk_knowledgemanagement = :id",
        params! { "id" => id },
    )?;
    assert_eq!(
        restantes,
        Some(0),
        "l'étiquette devait partir avec la fiche"
    );

    // Supprimer deux fois n'est pas un succès silencieux.
    let erreur = supprimer(&pool, ENTITY, id).expect_err("une fiche absente doit être signalée");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message : {erreur}"
    );

    nettoyer(&pool, &marque)?;
    Ok(())
}
