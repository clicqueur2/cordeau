use std::error::Error;
use std::str::FromStr;

use db::projet::NewProjet;
use mysql::prelude::Queryable;
use mysql::{params, Pool};
use rust_decimal::Decimal;

const ENTITY: i64 = 1;

/// Les tests de projet partagent le meme verrou de numerotation et les memes
/// tables : les faire tourner en parallele provoque des interblocages InnoDB
/// (erreur 1213). On les serialise, c'est plus honnete qu'un test qui echoue
/// une fois sur trois.
static SERIE: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

/// Supprime un projet et ses taches par identifiant.
///
/// Indispensable pour les projets sans tiers (chantier interne) : le menage par
/// nom de tiers ne les atteindrait pas et ils resteraient dans la base.
fn effacer(pool: &Pool, projet: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // Les enfants d'abord : `llx_projet_task.fk_projet` porte une vraie cle
    // etrangere vers `llx_projet`.
    conn.exec_drop(
        "DELETE FROM llx_projet_task WHERE fk_projet = :projet",
        params! { "projet" => projet },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_projet WHERE entity = :entity AND rowid = :projet",
        params! { "entity" => ENTITY, "projet" => projet },
    )?;
    Ok(())
}

/// Supprime les taches puis les projets du tiers de test, puis le tiers.
///
/// Chaque test porte son propre nom de tiers : ils tournent en parallele, deux
/// tests qui partageraient le meme nom se detruiraient mutuellement. Le menage
/// se fait **avant** autant qu'apres : un test interrompu laisse sinon des
/// restes qui font echouer le suivant.
fn nettoyer(pool: &Pool, nom: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let projets: Vec<i64> = conn.exec(
        "SELECT p.rowid FROM llx_projet p \
         INNER JOIN llx_societe s ON s.rowid = p.fk_soc \
         WHERE p.entity = :entity AND s.nom = :nom",
        params! { "entity" => ENTITY, "nom" => nom },
    )?;
    for id in projets {
        effacer(pool, id)?;
    }
    for p in db::party::list(pool, ENTITY, 10_000, 0)? {
        if p.nom == nom {
            db::party::delete(pool, ENTITY, p.rowid)?;
        }
    }
    Ok(())
}

fn tiers(pool: &Pool, nom: &str) -> Result<i64, Box<dyn Error>> {
    db::party::create(pool, ENTITY, &db::party::NewParty::client(nom))
}

#[test]
fn cree_un_projet_et_le_relit() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "Tiers Projet Chantier";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let mut new = NewProjet::new("Chantier calade Saint-Cyr");
    new.fk_soc = Some(fk_soc);
    new.description = Some("Reprise du mur de soutenement".into());
    new.date_start = Some("2026-03-02".into());
    new.date_end = Some("2026-05-29".into());
    // Trois decimales : le budget doit ressortir arrondi au centime.
    new.budget = dec("18450.456");
    let id = db::projet::create(&pool, ENTITY, &new)?;

    let p = db::projet::get(&pool, ENTITY, id)?.expect("projet relu");
    assert_eq!(p.rowid, id);
    assert_eq!(p.entity, ENTITY);
    assert_eq!(p.ref_.len(), 12, "PRJ + AAAMM + - + 4 chiffres: {}", p.ref_);
    assert!(
        p.ref_.starts_with("PRJ"),
        "reference inattendue: {}",
        p.ref_
    );
    assert_eq!(p.title, "Chantier calade Saint-Cyr");
    assert_eq!(p.fk_soc, Some(fk_soc));
    assert_eq!(
        p.date_start.as_deref(),
        Some("2026-03-02"),
        "dateo porte le debut du chantier"
    );
    assert_eq!(p.date_end.as_deref(), Some("2026-05-29"));
    assert_eq!(p.fk_statut, db::projet::STATUT_OUVERT);
    assert!(p.est_ouvert());
    assert_eq!(
        p.budget,
        Some(dec("18450.46")),
        "18450,456 arrondi au centime"
    );
    assert_eq!(
        p.description.as_deref(),
        Some("Reprise du mur de soutenement")
    );

    // Un projet sans tiers ni dates est licite : c'est un chantier interne.
    let interne = db::projet::create(&pool, ENTITY, &NewProjet::new("Chantier interne"))?;
    let i = db::projet::get(&pool, ENTITY, interne)?.expect("projet interne");
    assert_eq!(i.fk_soc, None);
    assert_eq!(i.date_start, None, "date absente = NULL, pas 0000-00-00");
    assert_eq!(i.budget, Some(dec("0")), "budget par defaut a zero");

    effacer(&pool, id)?;
    effacer(&pool, interne)?;
    nettoyer(&pool, nom)?;
    assert!(db::projet::get(&pool, ENTITY, id)?.is_none());
    Ok(())
}

#[test]
fn numerote_les_projets_sans_doublon() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "Tiers Projet Numero";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let mut ids = Vec::new();
    for i in 0..5 {
        let mut new = NewProjet::new(&format!("Chantier numero {i}"));
        new.fk_soc = Some(fk_soc);
        ids.push(db::projet::create(&pool, ENTITY, &new)?);
    }

    let mut refs = Vec::new();
    for id in &ids {
        refs.push(db::projet::get(&pool, ENTITY, *id)?.unwrap().ref_);
    }
    let uniques: std::collections::BTreeSet<&String> = refs.iter().collect();
    assert_eq!(uniques.len(), 5, "references dupliquees: {refs:?}");
    for r in &refs {
        assert_eq!(r.len(), 12, "format PRJ AAAMM - NNNN: {r}");
    }

    for id in &ids {
        effacer(&pool, *id)?;
    }
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn update_change_le_titre_sans_toucher_a_la_reference() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "Tiers Projet Update";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let mut new = NewProjet::new("Titre avant");
    new.fk_soc = Some(fk_soc);
    new.budget = dec("1000");
    let id = db::projet::create(&pool, ENTITY, &new)?;
    let avant = db::projet::get(&pool, ENTITY, id)?.expect("projet cree");

    let mut modif = NewProjet::new("Titre apres");
    modif.description = Some("Delai decale d'un mois".into());
    modif.date_start = Some("2026-04-06".into());
    modif.budget = dec("1250.505");
    db::projet::update(&pool, ENTITY, id, &modif)?;

    let apres = db::projet::get(&pool, ENTITY, id)?.expect("projet relu");
    assert_eq!(apres.title, "Titre apres");
    assert_eq!(
        apres.ref_, avant.ref_,
        "la reference est imprimee : elle ne change jamais"
    );
    assert_eq!(apres.budget, Some(dec("1250.51")));
    assert_eq!(apres.date_start.as_deref(), Some("2026-04-06"));
    assert_eq!(apres.date_end, None);
    assert_eq!(apres.description.as_deref(), Some("Delai decale d'un mois"));

    assert!(
        db::projet::update(&pool, ENTITY, id + 9_999_999, &modif).is_err(),
        "un projet inexistant ne se met pas a jour"
    );

    effacer(&pool, id)?;
    nettoyer(&pool, nom)?;
    Ok(())
}

#[test]
fn cree_une_tache_et_la_relit() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "Tiers Projet Tache";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    let mut new = NewProjet::new("Chantier a taches");
    new.fk_soc = Some(fk_soc);
    let projet = db::projet::create(&pool, ENTITY, &new)?;

    let t1 = db::projet::creer_tache(
        &pool,
        ENTITY,
        projet,
        "Terrassement",
        Some("2026-03-02 08:30:00"),
        Some("2026-03-06 17:00:00"),
    )?;
    let t2 = db::projet::creer_tache(&pool, ENTITY, projet, "Fondations", None, None)?;
    assert_ne!(t1, t2, "la reference de tache doit rester unique");

    let taches = db::projet::taches(&pool, ENTITY, projet)?;
    assert_eq!(taches.len(), 2, "deux taches creees");
    assert_eq!(taches[0].rowid, t1);
    assert_eq!(taches[0].fk_projet, projet);
    assert_eq!(taches[0].label, "Terrassement");
    assert_eq!(
        taches[0].ref_.as_deref(),
        Some(format!("T{t1}").as_str()),
        "reference de tache = T + rowid"
    );
    assert_eq!(taches[0].date_start.as_deref(), Some("2026-03-02 08:30:00"));
    assert_eq!(taches[0].date_end.as_deref(), Some("2026-03-06 17:00:00"));
    assert_eq!(taches[0].progress, 0);
    assert_eq!(taches[1].rowid, t2);
    assert_eq!(taches[1].label, "Fondations");
    assert_eq!(taches[1].ref_.as_deref(), Some(format!("T{t2}").as_str()));
    assert_eq!(taches[1].date_start, None);

    // Les taches d'un projet ne se melangent pas a celles d'un autre.
    let autre = db::projet::create(&pool, ENTITY, &NewProjet::new("Autre chantier"))?;
    assert!(db::projet::taches(&pool, ENTITY, autre)?.is_empty());

    assert!(
        db::projet::creer_tache(&pool, ENTITY, projet + 9_999_999, "Fantome", None, None).is_err(),
        "on n'ajoute pas de tache a un projet inexistant"
    );
    assert!(
        db::projet::creer_tache(&pool, ENTITY, projet, "   ", None, None).is_err(),
        "le libelle est obligatoire"
    );

    effacer(&pool, projet)?;
    effacer(&pool, autre)?;
    nettoyer(&pool, nom)?;
    assert!(db::projet::taches(&pool, ENTITY, projet)?.is_empty());
    Ok(())
}

#[test]
fn refuse_les_projets_invalides() -> Result<(), Box<dyn Error>> {
    let _garde = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let nom = "Tiers Projet Refus";
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, nom)?;
    let fk_soc = tiers(&pool, nom)?;

    assert!(
        db::projet::create(&pool, ENTITY, &NewProjet::new("   ")).is_err(),
        "un titre vide est refuse"
    );

    let mut tiers_inconnu = NewProjet::new("Chantier orphelin");
    tiers_inconnu.fk_soc = Some(999_999);
    assert!(
        db::projet::create(&pool, ENTITY, &tiers_inconnu).is_err(),
        "un tiers inexistant est refuse"
    );

    let mut negatif = NewProjet::new("Chantier negatif");
    negatif.fk_soc = Some(-1);
    assert!(db::projet::create(&pool, ENTITY, &negatif).is_err());

    // Le meme tiers, correctement rattache, passe : les refus ci-dessus viennent
    // bien de la validation, pas d'un probleme de donnees.
    let mut valide = NewProjet::new("Chantier valide");
    valide.fk_soc = Some(fk_soc);
    let id = db::projet::create(&pool, ENTITY, &valide)?;
    assert_eq!(
        db::projet::get(&pool, ENTITY, id)?.unwrap().fk_soc,
        Some(fk_soc)
    );

    effacer(&pool, id)?;
    nettoyer(&pool, nom)?;
    Ok(())
}
