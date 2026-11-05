//! Tests d'intégration du module Adhérents.
//!
//! Les tests qui touchent la base ne tournent que si `DOLIBARR_DSN` est posé :
//! sans base, ils s'ignorent en le disant.
//!
//! Tout est écrit en entité 1 et nettoyé derrière : les seules lignes laissées
//! sont celles qui existaient avant le test. Deux précautions particulières :
//!
//! - l'identifiant de connexion et l'email sont **uniques par test et par
//!   processus** (suffixe `std::process::id()`) : les fichiers de test tournent
//!   en parallèle, et rien n'empêche deux exécutions de la suite de se croiser
//!   sur la même base ;
//! - le type d'adhérent est celui de l'instance s'il en existe un (on ne touche
//!   jamais au dictionnaire réel), sinon un type jetable, supprimé à la fin.
//!   `uk_adherent_type_libelle (libelle, entity)` impose un libellé unique,
//!   d'où le même suffixe ;
//! - les tests se **sérialisent** sur un verrou (voir [`VERROU`]) : `a_jour`
//!   compte des lignes globales à l'entité, et un test qui créerait une fiche
//!   pendant qu'un autre mesure son écart rendrait la mesure fausse.

use std::error::Error;
use std::sync::{Mutex, MutexGuard};

use db::adherent::{self, NewAdherent, MORPHY_PHYSIQUE, STATUT_A_JOUR, STATUT_SUSPENDU};
use mysql::params;
use mysql::prelude::Queryable;
use mysql::Pool;

const ENTITY: i64 = 1;

/// Les tests de ce fichier partagent `llx_adherent` en entité 1.
///
/// Les tests d'un même fichier tournent en parallèle : sans ce verrou, un test
/// qui crée une fiche ferait bouger le compteur de [`adherent::a_jour`] pendant
/// qu'un autre mesure son écart, et deux tests pourraient se disputer le type
/// jetable. Le verrou les sérialise ; ils sont courts, le coût est nul.
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

/// Un identifiant de connexion unique par test et par processus.
fn login_test(base: &str) -> String {
    format!("test.adherent.{base}.{}", std::process::id())
}

/// Un email unique par test et par processus.
fn email_test(base: &str) -> String {
    format!("adherent.{base}.{}@exemple.test", std::process::id())
}

/// Supprime un adhérent par son identifiant de connexion : sert aussi à
/// effacer le reliquat d'une exécution précédente interrompue.
fn effacer(pool: &Pool, login: &str) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE FROM llx_adherent WHERE entity = :entity AND login = :login",
        params! { "entity" => ENTITY, "login" => login },
    );
}

/// Supprime un adhérent par son `rowid` : nécessaire quand la fiche n'a pas
/// d'identifiant de connexion, donc rien pour la retrouver autrement.
fn effacer_id(pool: &Pool, rowid: i64) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE FROM llx_adherent WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => ENTITY, "rowid" => rowid },
    );
}

/// Une date calculée par le serveur de base, à `jours` de aujourd'hui.
///
/// Passer par SQL évite d'embarquer une bibliothèque de dates dans les tests et
/// garantit que la date comparée est bien celle que verra `cotiser`.
fn date_relative(pool: &Pool, jours: i64) -> Result<String, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let date: Option<String> = conn.exec_first(
        "SELECT DATE_FORMAT(DATE_ADD(CURDATE(), INTERVAL :jours DAY), '%Y-%m-%d')",
        params! { "jours" => jours },
    )?;
    Ok(date.unwrap_or_default())
}

/// Un type d'adhérent utilisable par un test.
struct TypeTest {
    id: i64,
    libelle: String,
    /// Vrai quand le test a créé la ligne : elle est alors supprimée.
    temporaire: bool,
}

impl TypeTest {
    fn oter(&self, pool: &Pool) {
        if !self.temporaire {
            return;
        }
        let Ok(mut conn) = pool.get_conn() else {
            return;
        };
        let _ = conn.exec_drop(
            "DELETE FROM llx_adherent_type WHERE entity = :entity AND rowid = :rowid",
            params! { "entity" => ENTITY, "rowid" => self.id },
        );
    }
}

/// Un type d'adhérent jetable, au statut demandé (1 actif, 0 désactivé).
///
/// Le libellé porte le PID : sans cela, deux exécutions de la suite se
/// heurteraient à `uk_adherent_type_libelle`. Le reliquat d'une exécution
/// interrompue est effacé avant l'insertion, pour que le test soit rejouable
/// sans intervention à la main.
fn type_jetable(pool: &Pool, marque: &str, statut: i16) -> Result<TypeTest, Box<dyn Error>> {
    let libelle = format!("Adhérent test {marque} {}", std::process::id());
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_adherent_type WHERE entity = :entity AND libelle = :libelle",
        params! { "entity" => ENTITY, "libelle" => libelle.as_str() },
    )?;
    conn.exec_drop(
        "INSERT INTO llx_adherent_type \
         (entity, statut, libelle, morphy, duration, subscription, amount, \
          caneditamount, minimumamount, vote) \
         VALUES (:entity, :statut, :libelle, 'phy', '1y', '1', 0, 0, 0, '1')",
        params! { "entity" => ENTITY, "statut" => statut, "libelle" => libelle.as_str() },
    )?;
    Ok(TypeTest {
        id: conn.last_insert_id() as i64,
        libelle,
        temporaire: true,
    })
}

/// Un type actif : celui de l'instance s'il en existe un, sinon un type jetable.
fn type_adherent(pool: &Pool) -> Result<TypeTest, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existant: Option<(i64, String)> = conn.exec_first(
        "SELECT rowid, libelle FROM llx_adherent_type \
         WHERE entity = :entity AND statut = 1 ORDER BY rowid LIMIT 1",
        params! { "entity" => ENTITY },
    )?;
    // La connexion est rendue avant d'en reprendre une : garder les deux
    // ouvertes épuise le pool quand plusieurs tests tournent de front.
    drop(conn);
    match existant {
        Some((id, libelle)) => Ok(TypeTest {
            id,
            libelle,
            temporaire: false,
        }),
        None => type_jetable(pool, "commun", 1),
    }
}

fn nouvel_adherent(base: &str, fk_adherent_type: i64) -> NewAdherent {
    NewAdherent {
        lastname: format!("Dupont {base}"),
        firstname: Some("Marie".into()),
        email: Some(email_test(base)),
        login: Some(login_test(base)),
        fk_adherent_type,
        societe: None,
        address: Some("1 rue des Tests".into()),
        zip: Some("69390".into()),
        town: Some("Charly".into()),
        phone: Some("0400000000".into()),
        phone_mobile: Some("0600000000".into()),
        note_private: None,
        note_public: None,
    }
}

#[test]
fn cree_relit_et_liste_un_adherent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_adherent(&pool)?;
    let base = "creation";
    effacer(&pool, &login_test(base));

    let new = nouvel_adherent(base, type_test.id);
    let id = adherent::create(&pool, ENTITY, &new)?;
    assert!(id > 0);

    let lu = adherent::get(&pool, ENTITY, id)?.expect("adhérent relu");
    assert_eq!(lu.nom, format!("Dupont {base}"));
    assert_eq!(lu.prenom.as_deref(), Some("Marie"));
    assert_eq!(lu.nom_complet(), format!("Marie Dupont {base}"));
    assert_eq!(lu.entity, ENTITY);
    assert_eq!(lu.email, Some(email_test(base)));
    assert_eq!(lu.login, Some(login_test(base)));
    assert_eq!(lu.ville.as_deref(), Some("Charly"));
    assert_eq!(lu.statut, STATUT_A_JOUR);
    assert!(!lu.public, "une fiche n'est pas publiée par défaut");
    assert_eq!(lu.morphy, MORPHY_PHYSIQUE);
    assert_eq!(lu.fk_adherent_type, type_test.id);
    assert_eq!(
        lu.type_libelle.as_deref(),
        Some(type_test.libelle.as_str()),
        "le libellé du type vient de la jointure"
    );
    assert!(lu.date_creation.is_some(), "datec posée");
    assert!(lu.date_validation.is_some(), "datevalid posée");
    assert!(
        lu.date_fin.is_none(),
        "une création n'enregistre aucune cotisation"
    );

    let reference = lu.ref_.clone();
    assert!(
        reference.starts_with("AD") && reference[2..].chars().all(|c| c.is_ascii_digit()),
        "format de référence inattendu : {reference}"
    );

    let listes = adherent::list(&pool, ENTITY, 1000, 0)?;
    assert!(listes.iter().any(|a| a.rowid == id));

    let par_login = adherent::par_login(&pool, ENTITY, &login_test(base))?.expect("par login");
    assert_eq!(par_login.rowid, id);
    assert!(
        adherent::par_login(&pool, ENTITY, &login_test("inconnu"))?.is_none(),
        "un login inconnu ne ramène rien"
    );

    effacer(&pool, &login_test(base));
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_un_nom_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_adherent(&pool)?;
    let mut vide = nouvel_adherent("nom_vide", type_test.id);
    vide.lastname = "   ".into();
    let erreur = adherent::create(&pool, ENTITY, &vide).expect_err("nom vide refusé");
    assert!(erreur.to_string().contains("nom"), "{erreur}");
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_un_email_sans_arobase() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_adherent(&pool)?;
    let base = "email_sans_arobase";
    effacer(&pool, &login_test(base));

    let mut sans_arobase = nouvel_adherent(base, type_test.id);
    sans_arobase.email = Some("adherent.exemple.test".into());
    let erreur = adherent::create(&pool, ENTITY, &sans_arobase).expect_err("email refusé");
    assert!(erreur.to_string().contains("arobase"), "{erreur}");
    assert!(
        adherent::par_login(&pool, ENTITY, &login_test(base))?.is_none(),
        "rien ne doit être écrit quand la saisie est refusée"
    );

    // Un email absent reste légitime : on n'exige pas d'adresse pour adhérer.
    let mut sans_email = nouvel_adherent(base, type_test.id);
    sans_email.email = Some("   ".into());
    let id = adherent::create(&pool, ENTITY, &sans_email)?;
    assert!(adherent::get(&pool, ENTITY, id)?.unwrap().email.is_none());

    effacer(&pool, &login_test(base));
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_un_type_inconnu_ou_desactive() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let base = "type_inconnu";
    effacer(&pool, &login_test(base));

    let inconnu = nouvel_adherent(base, 999_999);
    let erreur = adherent::create(&pool, ENTITY, &inconnu).expect_err("type inconnu refusé");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");

    // Un type désactivé garde ses adhésions passées mais ne sert plus à en créer.
    let desactive = type_jetable(&pool, "desactive", 0)?;
    let avec_desactive = nouvel_adherent(base, desactive.id);
    let erreur =
        adherent::create(&pool, ENTITY, &avec_desactive).expect_err("type désactivé refusé");
    assert!(erreur.to_string().contains("désactivé"), "{erreur}");

    assert!(
        adherent::par_login(&pool, ENTITY, &login_test(base))?.is_none(),
        "aucune fiche ne doit être créée"
    );

    desactive.oter(&pool);
    Ok(())
}

#[test]
fn refuse_un_login_deja_pris() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_adherent(&pool)?;
    let base = "login_pris";
    effacer(&pool, &login_test(base));

    let premier = adherent::create(&pool, ENTITY, &nouvel_adherent(base, type_test.id))?;

    // Même login, autre nom et autre email : seul l'identifiant entre en cause.
    let mut doublon = nouvel_adherent("login_pris_bis", type_test.id);
    doublon.login = Some(login_test(base));
    let erreur = adherent::create(&pool, ENTITY, &doublon).expect_err("login refusé");
    assert!(erreur.to_string().contains("déjà pris"), "{erreur}");

    effacer(&pool, &login_test(base));
    effacer_id(&pool, premier);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn cotiser_pose_la_date_de_fin() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_adherent(&pool)?;
    let base = "cotisation";
    effacer(&pool, &login_test(base));

    let id = adherent::create(&pool, ENTITY, &nouvel_adherent(base, type_test.id))?;
    assert!(adherent::get(&pool, ENTITY, id)?
        .unwrap()
        .date_fin
        .is_none());

    let echeance = date_relative(&pool, 365)?;
    adherent::cotiser(&pool, ENTITY, id, &echeance)?;

    let lu = adherent::get(&pool, ENTITY, id)?.expect("adhérent relu");
    assert_eq!(lu.date_fin.as_deref(), Some(echeance.as_str()));
    assert_eq!(lu.statut, STATUT_A_JOUR);

    // Une adhésion en cours ne se raccourcit pas : ce serait retirer des mois
    // déjà payés.
    let plus_court = date_relative(&pool, 30)?;
    let erreur = adherent::cotiser(&pool, ENTITY, id, &plus_court).expect_err("raccourci refusé");
    assert!(erreur.to_string().contains("raccourcirait"), "{erreur}");
    assert_eq!(
        adherent::get(&pool, ENTITY, id)?
            .unwrap()
            .date_fin
            .as_deref(),
        Some(echeance.as_str()),
        "la date de fin n'a pas bougé"
    );

    // Une date au bon format mais absente du calendrier est refusée avant
    // d'atteindre MySQL.
    let erreur = adherent::cotiser(&pool, ENTITY, id, "2027-02-31").expect_err("date refusée");
    assert!(erreur.to_string().contains("invalide"), "{erreur}");

    effacer(&pool, &login_test(base));
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_une_cotisation_dans_le_passe() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_adherent(&pool)?;
    let base = "cotisation_passee";
    effacer(&pool, &login_test(base));

    let id = adherent::create(&pool, ENTITY, &nouvel_adherent(base, type_test.id))?;
    let passe = date_relative(&pool, -1)?;
    let erreur = adherent::cotiser(&pool, ENTITY, id, &passe).expect_err("date passée refusée");
    assert!(erreur.to_string().contains("passé"), "{erreur}");
    assert!(
        adherent::get(&pool, ENTITY, id)?
            .unwrap()
            .date_fin
            .is_none(),
        "une cotisation refusée n'écrit rien"
    );

    effacer(&pool, &login_test(base));
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn suspend_puis_refuse_de_resuspendre() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_adherent(&pool)?;
    let base = "suspension";
    effacer(&pool, &login_test(base));

    let id = adherent::create(&pool, ENTITY, &nouvel_adherent(base, type_test.id))?;
    adherent::suspendre(&pool, ENTITY, id)?;
    assert_eq!(
        adherent::get(&pool, ENTITY, id)?.unwrap().statut,
        STATUT_SUSPENDU
    );

    let erreur = adherent::suspendre(&pool, ENTITY, id).expect_err("double suspension refusée");
    assert!(erreur.to_string().contains("déjà suspendu"), "{erreur}");

    effacer(&pool, &login_test(base));
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn reactive_un_adherent_suspendu() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_adherent(&pool)?;
    let base = "reactivation";
    effacer(&pool, &login_test(base));

    let id = adherent::create(&pool, ENTITY, &nouvel_adherent(base, type_test.id))?;
    adherent::suspendre(&pool, ENTITY, id)?;
    adherent::reactiver(&pool, ENTITY, id)?;
    assert_eq!(
        adherent::get(&pool, ENTITY, id)?.unwrap().statut,
        STATUT_A_JOUR
    );

    let erreur = adherent::reactiver(&pool, ENTITY, id).expect_err("réactivation en double");
    assert!(erreur.to_string().contains("déjà à jour"), "{erreur}");

    effacer(&pool, &login_test(base));
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn a_jour_compte_les_cotisations_en_cours() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_adherent(&pool)?;
    let base = "a_jour";
    effacer(&pool, &login_test(base));

    // La base réelle peut déjà contenir des adhérents : on mesure des écarts,
    // jamais des valeurs absolues.
    let (avant_a_jour, avant_total) = adherent::a_jour(&pool, ENTITY)?;

    let id = adherent::create(&pool, ENTITY, &nouvel_adherent(base, type_test.id))?;
    let (apres_a_jour, apres_total) = adherent::a_jour(&pool, ENTITY)?;
    assert_eq!(apres_a_jour, avant_a_jour + 1);
    assert_eq!(apres_total, avant_total + 1);

    adherent::suspendre(&pool, ENTITY, id)?;
    let (suspendu_a_jour, suspendu_total) = adherent::a_jour(&pool, ENTITY)?;
    assert_eq!(suspendu_a_jour, avant_a_jour);
    assert_eq!(suspendu_total, avant_total + 1);

    // Statut remis à jour par une cotisation : `cotiser` vaut réintégration,
    // inutile d'appeler `reactiver` derrière.
    let echeance = date_relative(&pool, 365)?;
    adherent::cotiser(&pool, ENTITY, id, &echeance)?;
    let (cotise_a_jour, cotise_total) = adherent::a_jour(&pool, ENTITY)?;
    assert_eq!(
        cotise_a_jour,
        avant_a_jour + 1,
        "une cotisation en cours remet l'adhérent à jour"
    );
    assert_eq!(cotise_total, avant_total + 1);

    // Statut à jour mais cotisation expirée : hors décompte. On force la date
    // par SQL, aucun chemin de ce module ne permettant d'écrire dans le passé.
    let passe = date_relative(&pool, -30)?;
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_adherent SET statut = :statut, datefin = :datefin \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_A_JOUR, "datefin" => passe.as_str(), "entity" => ENTITY, "rowid" => id },
    )?;
    drop(conn);
    let (expire_a_jour, expire_total) = adherent::a_jour(&pool, ENTITY)?;
    assert_eq!(
        expire_a_jour, avant_a_jour,
        "une adhésion expirée n'est plus à jour"
    );
    assert_eq!(expire_total, avant_total + 1);

    effacer(&pool, &login_test(base));
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn types_liste_les_types_actifs() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let actif = type_jetable(&pool, "menu", 1)?;
    let inactif = type_jetable(&pool, "menu_off", 0)?;

    let liste = adherent::types(&pool, ENTITY)?;
    assert!(!liste.is_empty(), "un type actif doit apparaître");
    assert!(
        liste
            .iter()
            .any(|(id, libelle)| *id == actif.id && *libelle == actif.libelle),
        "le type actif créé pour le test doit être proposé"
    );
    assert!(
        !liste.iter().any(|(id, _)| *id == inactif.id),
        "un type désactivé ne doit pas être proposé"
    );

    actif.oter(&pool);
    inactif.oter(&pool);
    Ok(())
}

#[test]
fn definit_un_mot_de_passe_seulement_hache() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_adherent(&pool)?;
    let base = "mot_de_passe";
    effacer(&pool, &login_test(base));

    let id = adherent::create(&pool, ENTITY, &nouvel_adherent(base, type_test.id))?;

    // Un mot de passe en clair est refusé : c'est tout l'objet de la fonction.
    let erreur = adherent::definir_mot_de_passe(&pool, ENTITY, id, "MotDePasse!2026")
        .expect_err("mot de passe en clair refusé");
    assert!(erreur.to_string().contains("empreinte"), "{erreur}");

    let empreinte = auth::hash("MotDePasse!2026");
    adherent::definir_mot_de_passe(&pool, ENTITY, id, &empreinte)?;

    let mut conn = pool.get_conn()?;
    let stocke: Option<(Option<String>, Option<String>)> = conn.exec_first(
        "SELECT pass_crypted, pass FROM llx_adherent WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => ENTITY, "rowid" => id },
    )?;
    drop(conn);
    let (pass_crypted, pass) = stocke.expect("adhérent relu");
    let stockee = pass_crypted.expect("empreinte stockée");
    assert_eq!(stockee, empreinte);
    assert!(
        pass.is_none(),
        "aucun mot de passe en clair ne reste en base"
    );
    assert!(
        auth::verify("MotDePasse!2026", &stockee, Some("bcrypt")),
        "l'empreinte relue en base doit être vérifiable par le crate auth"
    );

    // Un adhérent sans login ne peut pas se servir d'un mot de passe.
    let mut sans_login = nouvel_adherent("mot_de_passe_sans_login", type_test.id);
    sans_login.login = None;
    let sans_login_id = adherent::create(&pool, ENTITY, &sans_login)?;
    let erreur = adherent::definir_mot_de_passe(&pool, ENTITY, sans_login_id, &empreinte)
        .expect_err("adhérent sans login refusé");
    assert!(erreur.to_string().contains("identifiant"), "{erreur}");

    effacer(&pool, &login_test(base));
    effacer_id(&pool, sans_login_id);
    type_test.oter(&pool);
    Ok(())
}
