//! Tests d'intégration du module Partenariats.
//!
//! Les tests qui touchent la base ne tournent que si `DOLIBARR_DSN` est posé :
//! sans base, ils s'ignorent en le disant.
//!
//! Tout est écrit en entité 1 et nettoyé derrière. Trois précautions :
//!
//! - **les références portent le PID du processus** (`PA-TEST-<cas>-<pid>`) : les
//!   fichiers de test tournent en parallèle et rien n'empêche deux exécutions de
//!   la suite de se croiser sur la même base. Chaque `creer` est précédé d'un
//!   `DELETE` de la même référence, ce qui rend un test rejouable après une
//!   interruption, même si le nettoyage de la fin n'a pas eu lieu ;
//! - **le type de partenariat est jetable**, jamais celui de l'instance : le
//!   dictionnaire `llx_c_partnership_type` est **vide par défaut** dans Dolibarr
//!   (relevé sur la base de référence), donc on ne peut pas compter sur un type
//!   existant. Chaque test insère le sien (code unique, `uk_c_partnership_type
//!   (entity, code)`) et le supprime à la fin ;
//! - **les tests se sérialisent sur un verrou** (voir [`VERROU`]) : `en_cours`
//!   compte des lignes globales à l'entité, et un test qui créerait un
//!   partenariat pendant qu'un autre mesure son écart rendrait la mesure fausse.
//!
//! Le tiers n'est **pas** créé : `fk_soc` reçoit un identifiant fictif
//! (`TIERS_FICTIF + n`). Dolibarr ne pose aucune contrainte de clé étrangère sur
//! `llx_partnership.fk_soc`, et ce module ne prétend pas vérifier l'existence du
//! tiers — l'inventer dans `llx_societe` salirait la table des tiers pour rien.
//! Conséquence à connaître : `tiers_nom` reste `None` dans ces tests, la jointure
//! ne trouvant personne.

use std::error::Error;
use std::sync::{Mutex, MutexGuard};

use db::partenariat::{
    self, NouveauPartenariat, MOTIF_MAX, STATUT_ACCEPTE, STATUT_ANNULE, STATUT_BROUILLON,
    STATUT_REFUSE, STATUT_VALIDE,
};
use mysql::params;
use mysql::prelude::Queryable;
use mysql::Pool;

const ENTITY: i64 = 1;

/// Premier identifiant de tiers fictif. Au-delà de n'importe quelle société
/// réelle, et distinct d'un test à l'autre pour ne pas heurter
/// `uk_fk_type_fk_soc (fk_type, fk_soc, date_partnership_start)`.
const TIERS_FICTIF: i64 = 990_000_000;

/// Les tests de ce fichier partagent `llx_partnership` en entité 1.
///
/// Les tests d'un même fichier tournent en parallèle : sans ce verrou, un test
/// qui crée un partenariat ferait bouger le compteur de [`partenariat::en_cours`]
/// pendant qu'un autre mesure son écart. Le verrou les sérialise ; ils sont
/// courts, le coût est nul.
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

/// Un auteur de création, lu dans `llx_user` en entité 1.
///
/// À défaut d'utilisateur, on retombe sur `1` : `fk_user_creat` n'a pas de clé
/// étrangère, et sauter dix tests sur une base fraîche parce qu'aucun
/// utilisateur n'a encore été créé serait une perte sèche. La ligne de test est
/// supprimée juste après, elle ne laisse aucune trace d'audit fausse.
fn auteur(pool: &Pool) -> i64 {
    let Ok(mut conn) = pool.get_conn() else {
        return 1;
    };
    let trouve: Option<i64> = conn
        .exec_first(
            "SELECT rowid FROM llx_user WHERE entity = :entity ORDER BY rowid LIMIT 1",
            params! { "entity" => ENTITY },
        )
        .ok()
        .flatten();
    trouve.unwrap_or(1)
}

/// Une référence unique par cas et par processus.
fn ref_test(base: &str) -> String {
    format!("PA-TEST-{base}-{}", std::process::id())
}

/// Supprime un partenariat par sa référence : sert aussi à effacer le reliquat
/// d'une exécution précédente interrompue.
fn effacer(pool: &Pool, reference: &str) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE FROM llx_partnership WHERE entity = :entity AND ref = :ref",
        params! { "entity" => ENTITY, "ref" => reference },
    );
}

/// Un type de partenariat jetable, actif ou non.
struct TypeTest {
    id: i64,
    label: String,
    code: String,
}

impl TypeTest {
    fn oter(&self, pool: &Pool) {
        let Ok(mut conn) = pool.get_conn() else {
            return;
        };
        let _ = conn.exec_drop(
            "DELETE FROM llx_c_partnership_type WHERE entity = :entity AND code = :code",
            params! { "entity" => ENTITY, "code" => self.code.as_str() },
        );
    }
}

/// Un type de partenariat jetable, au statut demandé (`1` actif, `0` désactivé).
///
/// Le code porte le PID : sans cela, deux exécutions de la suite se heurteraient
/// à `uk_c_partnership_type (entity, code)`. Il est borné à 32 octets, la
/// largeur de la colonne. Le reliquat d'une exécution interrompue est effacé
/// avant l'insertion, pour que le test soit rejouable sans intervention.
fn type_jetable(pool: &Pool, marque: &str, actif: i16) -> Result<TypeTest, Box<dyn Error>> {
    let brut = format!("pt{}{marque}", std::process::id());
    let code: String = brut.chars().take(32).collect();
    let label = format!("Type de test {marque} {}", std::process::id());
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_c_partnership_type WHERE entity = :entity AND code = :code",
        params! { "entity" => ENTITY, "code" => code.as_str() },
    )?;
    conn.exec_drop(
        "INSERT INTO llx_c_partnership_type (entity, code, label, active) \
         VALUES (:entity, :code, :label, :actif)",
        params! {
            "entity" => ENTITY,
            "code" => code.as_str(),
            "label" => label.as_str(),
            "actif" => actif,
        },
    )?;
    Ok(TypeTest {
        id: conn.last_insert_id() as i64,
        label,
        code,
    })
}

/// Une date calculée par le serveur de base, à `jours` de aujourd'hui.
///
/// Passer par SQL évite d'embarquer une bibliothèque de dates dans les tests et
/// garantit que la date comparée est bien celle que verra `en_cours`.
fn date_relative(pool: &Pool, jours: i64) -> Result<String, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let date: Option<String> = conn.exec_first(
        "SELECT DATE_FORMAT(DATE_ADD(CURDATE(), INTERVAL :jours DAY), '%Y-%m-%d')",
        params! { "jours" => jours },
    )?;
    Ok(date.unwrap_or_default())
}

/// Un partenariat prêt à créer, avec un tiers fictif propre à `soc`.
fn nouveau(reference: &str, fk_type: i64, soc: i64, auteur: i64) -> NouveauPartenariat {
    NouveauPartenariat {
        ref_: reference.to_string(),
        fk_type,
        fk_soc: Some(soc),
        fk_member: None,
        email_partnership: Some("partenariat@exemple.test".into()),
        date_partnership_start: "2026-01-15".into(),
        date_partnership_end: Some("2027-01-14".into()),
        url_to_check: Some("https://exemple.test/partenaires".into()),
        fk_user_creat: auteur,
    }
}

#[test]
fn cree_relit_et_liste_un_partenariat() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_jetable(&pool, "creation", 1)?;
    let auteur = auteur(&pool);
    let reference = ref_test("creation");
    effacer(&pool, &reference);

    let new = nouveau(&reference, type_test.id, TIERS_FICTIF + 1, auteur);
    let id = partenariat::creer(&pool, ENTITY, &new)?;
    assert!(id > 0);

    let lu = partenariat::get(&pool, ENTITY, id)?.expect("partenariat relu");
    assert_eq!(lu.entity, ENTITY);
    assert_eq!(lu.ref_, reference);
    assert_eq!(lu.statut, STATUT_BROUILLON, "une création est un brouillon");
    assert!(lu.est_brouillon());
    assert_eq!(lu.fk_type, type_test.id);
    assert_eq!(
        lu.type_libelle.as_deref(),
        Some(type_test.label.as_str()),
        "le libellé du type vient de la jointure"
    );
    assert_eq!(lu.fk_soc, Some(TIERS_FICTIF + 1));
    assert_eq!(lu.email.as_deref(), Some("partenariat@exemple.test"));
    assert_eq!(lu.date_debut.as_deref(), Some("2026-01-15"));
    assert_eq!(lu.date_fin.as_deref(), Some("2027-01-14"));
    assert_eq!(
        lu.url_to_check.as_deref(),
        Some("https://exemple.test/partenaires")
    );
    assert_eq!(lu.fk_user_creat, Some(auteur));
    assert!(
        lu.date_creation.is_some(),
        "date_creation est posée par NOW()"
    );
    assert_eq!(lu.count_last_url_check_error, 0);
    assert!(
        lu.motif.is_none(),
        "un brouillon neuf n'a ni refus ni annulation"
    );
    assert!(
        lu.tiers_nom.is_none(),
        "le tiers est fictif : la jointure ne trouve personne"
    );

    let listes = partenariat::list(&pool, ENTITY, 1000, 0)?;
    assert!(listes.iter().any(|p| p.rowid == id));

    assert!(
        partenariat::get(&pool, ENTITY, id + 100_000_000)?.is_none(),
        "un rowid inconnu ne ramène rien"
    );

    effacer(&pool, &reference);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_une_reference_vide_ou_trop_longue() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_jetable(&pool, "reference", 1)?;
    let auteur = auteur(&pool);

    let mut vide = nouveau("sans-objet", type_test.id, TIERS_FICTIF + 2, auteur);
    vide.ref_ = "   ".into();
    let erreur = partenariat::creer(&pool, ENTITY, &vide).expect_err("référence vide refusée");
    assert!(erreur.to_string().contains("obligatoire"), "{erreur}");

    let mut longue = nouveau("sans-objet", type_test.id, TIERS_FICTIF + 2, auteur);
    longue.ref_ = "R".repeat(129);
    let erreur = partenariat::creer(&pool, ENTITY, &longue).expect_err("129 octets refusés");
    assert!(erreur.to_string().contains("128"), "{erreur}");

    // 128 octets passent : la borne est la largeur de la colonne, pas moins.
    let base = ref_test("longue");
    let reference: String = format!("{base}{}", "R".repeat(128 - base.len()));
    assert_eq!(reference.len(), 128, "la référence de test fait 128 octets");
    effacer(&pool, &reference);
    let mut juste = nouveau("sans-objet", type_test.id, TIERS_FICTIF + 2, auteur);
    juste.ref_ = reference.clone();
    let id = partenariat::creer(&pool, ENTITY, &juste)?;
    assert!(id > 0);

    effacer(&pool, &reference);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_un_type_inconnu_ou_desactive() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let inactif = type_jetable(&pool, "inactif", 0)?;
    let auteur = auteur(&pool);

    let inconnu = nouveau("type_inconnu", 900_000_001, TIERS_FICTIF + 3, auteur);
    let erreur = partenariat::creer(&pool, ENTITY, &inconnu).expect_err("type inconnu refusé");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");

    let desactive = nouveau("type_desactive", inactif.id, TIERS_FICTIF + 3, auteur);
    let erreur = partenariat::creer(&pool, ENTITY, &desactive).expect_err("type désactivé refusé");
    assert!(erreur.to_string().contains("désactivé"), "{erreur}");

    inactif.oter(&pool);
    Ok(())
}

#[test]
fn refuse_des_dates_inversees() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_jetable(&pool, "dates", 1)?;
    let auteur = auteur(&pool);

    let mut inverse = nouveau("dates_inversees", type_test.id, TIERS_FICTIF + 4, auteur);
    inverse.date_partnership_start = "2027-01-14".into();
    inverse.date_partnership_end = Some("2026-01-15".into());
    let erreur = partenariat::creer(&pool, ENTITY, &inverse).expect_err("dates inversées refusées");
    assert!(erreur.to_string().contains("précède"), "{erreur}");

    // Une date de début vide, puis une date qui n'existe pas au calendrier.
    let mut sans_debut = nouveau("sans_debut", type_test.id, TIERS_FICTIF + 4, auteur);
    sans_debut.date_partnership_start = "  ".into();
    let erreur = partenariat::creer(&pool, ENTITY, &sans_debut).expect_err("début vide refusé");
    assert!(erreur.to_string().contains("obligatoire"), "{erreur}");

    let mut inexistante = nouveau("date_inexistante", type_test.id, TIERS_FICTIF + 4, auteur);
    inexistante.date_partnership_start = "2027-02-31".into();
    inexistante.date_partnership_end = None;
    let erreur = partenariat::creer(&pool, ENTITY, &inexistante).expect_err("31 février refusé");
    assert!(erreur.to_string().contains("calendrier"), "{erreur}");

    // Un 29 février d'une année bissextile, lui, existe.
    let reference = ref_test("date_bissextile");
    effacer(&pool, &reference);
    let mut bissextile = nouveau(&reference, type_test.id, TIERS_FICTIF + 4, auteur);
    bissextile.date_partnership_start = "2028-02-29".into();
    bissextile.date_partnership_end = None;
    let id = partenariat::creer(&pool, ENTITY, &bissextile)?;
    assert_eq!(
        partenariat::get(&pool, ENTITY, id)?.unwrap().date_fin,
        None,
        "sans terme, la date de fin reste NULL"
    );

    effacer(&pool, &reference);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_les_saisies_incompletes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_jetable(&pool, "incomplet", 1)?;
    let auteur = auteur(&pool);

    let mut sans_auteur = nouveau("sans_auteur", type_test.id, TIERS_FICTIF + 5, auteur);
    sans_auteur.fk_user_creat = 0;
    let erreur = partenariat::creer(&pool, ENTITY, &sans_auteur).expect_err("auteur nul refusé");
    assert!(erreur.to_string().contains("fk_user_creat"), "{erreur}");

    let mut sans_arobase = nouveau("sans_arobase", type_test.id, TIERS_FICTIF + 5, auteur);
    sans_arobase.email_partnership = Some("partenariat.exemple.test".into());
    let erreur = partenariat::creer(&pool, ENTITY, &sans_arobase).expect_err("email refusé");
    assert!(erreur.to_string().contains("arobase"), "{erreur}");

    // Ni tiers ni adhérent : Dolibarr refuse aussi (Partnership::create).
    let mut orphelin = nouveau("orphelin", type_test.id, TIERS_FICTIF + 5, auteur);
    orphelin.fk_soc = None;
    orphelin.fk_member = None;
    let erreur = partenariat::creer(&pool, ENTITY, &orphelin).expect_err("sans partenaire refusé");
    assert!(erreur.to_string().contains("tiers"), "{erreur}");

    // Un email vide et un tiers à 0 valent absence, pas faute : la fiche passe
    // dès lors qu'un adhérent est rattaché.
    let reference = ref_test("saisie_minimale");
    effacer(&pool, &reference);
    let mut minimal = nouveau(&reference, type_test.id, 0, auteur);
    minimal.fk_member = Some(TIERS_FICTIF + 14);
    minimal.email_partnership = Some("   ".into());
    minimal.url_to_check = None;
    minimal.date_partnership_end = None;
    let id = partenariat::creer(&pool, ENTITY, &minimal)?;
    let lu = partenariat::get(&pool, ENTITY, id)?.unwrap();
    assert_eq!(lu.email, None);
    assert_eq!(lu.fk_soc, None, "un fk_soc à 0 vaut absence");
    assert_eq!(lu.fk_member, Some(TIERS_FICTIF + 14));
    assert_eq!(lu.url_to_check, None);

    effacer(&pool, &reference);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn cycle_complet_dans_l_ordre() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_jetable(&pool, "cycle", 1)?;
    let auteur = auteur(&pool);
    let reference = ref_test("cycle");
    effacer(&pool, &reference);

    let id = partenariat::creer(
        &pool,
        ENTITY,
        &nouveau(&reference, type_test.id, TIERS_FICTIF + 6, auteur),
    )?;
    assert_eq!(
        partenariat::get(&pool, ENTITY, id)?.unwrap().statut,
        STATUT_BROUILLON
    );

    partenariat::soumettre(&pool, ENTITY, id)?;
    assert_eq!(
        partenariat::get(&pool, ENTITY, id)?.unwrap().statut,
        STATUT_VALIDE
    );

    partenariat::accepter(&pool, ENTITY, id)?;
    let accepte = partenariat::get(&pool, ENTITY, id)?.unwrap();
    assert_eq!(accepte.statut, STATUT_ACCEPTE);
    assert!(accepte.est_accepte());

    // Le partenariat va du 15 janvier 2026 au 14 janvier 2027 : il court encore
    // au 1er juin 2026, plus au 1er janvier 2028.
    assert!(accepte.court_encore("2026-06-01"));
    assert!(!accepte.court_encore("2028-01-01"));

    partenariat::annuler(&pool, ENTITY, id, "Partenariat repris par le successeur")?;
    let annule = partenariat::get(&pool, ENTITY, id)?.unwrap();
    assert_eq!(
        annule.statut, STATUT_ANNULE,
        "l'annulation prend le statut 9 de Dolibarr (STATUS_CANCELED), pas 4"
    );
    assert_eq!(annule.statut, 9);
    assert!(annule.est_annule());
    assert_eq!(
        annule.motif.as_deref(),
        Some("Partenariat repris par le successeur")
    );
    assert!(
        !annule.court_encore("2026-06-01"),
        "un annulé ne court plus"
    );

    effacer(&pool, &reference);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_les_transitions_hors_sequence() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_jetable(&pool, "sequence", 1)?;
    let auteur = auteur(&pool);

    // --- A : rien ne part d'un brouillon sauf la soumission -------------------
    let reference_a = ref_test("sequence_a");
    effacer(&pool, &reference_a);
    let a = partenariat::creer(
        &pool,
        ENTITY,
        &nouveau(&reference_a, type_test.id, TIERS_FICTIF + 7, auteur),
    )?;

    let erreur = partenariat::accepter(&pool, ENTITY, a).expect_err("accepter un brouillon");
    assert!(erreur.to_string().contains("brouillon"), "{erreur}");
    assert!(
        erreur.to_string().contains("statut 0"),
        "le message doit dire l'état courant : {erreur}"
    );
    let erreur =
        partenariat::refuser(&pool, ENTITY, a, "trop tôt").expect_err("refuser un brouillon");
    assert!(erreur.to_string().contains("validé"), "{erreur}");
    let erreur =
        partenariat::annuler(&pool, ENTITY, a, "trop tôt").expect_err("annuler un brouillon");
    assert!(erreur.to_string().contains("brouillon"), "{erreur}");

    partenariat::soumettre(&pool, ENTITY, a)?;
    let erreur = partenariat::soumettre(&pool, ENTITY, a).expect_err("soumettre deux fois");
    assert!(erreur.to_string().contains("validé"), "{erreur}");

    // --- B : un accepté ne se rouvre pas ------------------------------------
    partenariat::accepter(&pool, ENTITY, a)?;
    let erreur = partenariat::soumettre(&pool, ENTITY, a).expect_err("soumettre un accepté");
    assert!(erreur.to_string().contains("accepté"), "{erreur}");
    let erreur = partenariat::accepter(&pool, ENTITY, a).expect_err("accepter deux fois");
    assert!(erreur.to_string().contains("accepté"), "{erreur}");
    let erreur =
        partenariat::refuser(&pool, ENTITY, a, "trop tard").expect_err("refuser un accepté");
    assert!(erreur.to_string().contains("accepté"), "{erreur}");

    // Un accepté s'annule, en revanche : c'est la sortie normale.
    partenariat::annuler(&pool, ENTITY, a, "fin de convention")?;
    let erreur =
        partenariat::annuler(&pool, ENTITY, a, "déjà fait").expect_err("annuler deux fois");
    assert!(erreur.to_string().contains("annulé"), "{erreur}");
    let erreur = partenariat::accepter(&pool, ENTITY, a).expect_err("accepter un annulé");
    assert!(erreur.to_string().contains("annulé"), "{erreur}");
    let erreur = partenariat::refuser(&pool, ENTITY, a, "non").expect_err("refuser un annulé");
    assert!(erreur.to_string().contains("annulé"), "{erreur}");

    // --- C : un refusé reste refusé -----------------------------------------
    let reference_c = ref_test("sequence_c");
    effacer(&pool, &reference_c);
    let c = partenariat::creer(
        &pool,
        ENTITY,
        &nouveau(&reference_c, type_test.id, TIERS_FICTIF + 8, auteur),
    )?;
    partenariat::soumettre(&pool, ENTITY, c)?;
    partenariat::refuser(&pool, ENTITY, c, "dossier incomplet")?;
    assert_eq!(
        partenariat::get(&pool, ENTITY, c)?.unwrap().statut,
        STATUT_REFUSE
    );

    let erreur = partenariat::accepter(&pool, ENTITY, c).expect_err("accepter un refusé");
    assert!(erreur.to_string().contains("refusé"), "{erreur}");
    let erreur = partenariat::soumettre(&pool, ENTITY, c).expect_err("soumettre un refusé");
    assert!(erreur.to_string().contains("refusé"), "{erreur}");
    let erreur = partenariat::refuser(&pool, ENTITY, c, "encore").expect_err("refuser deux fois");
    assert!(erreur.to_string().contains("refusé"), "{erreur}");
    let erreur = partenariat::annuler(&pool, ENTITY, c, "non").expect_err("annuler un refusé");
    assert!(erreur.to_string().contains("refusé"), "{erreur}");

    // Une transition refusée ne doit rien écrire : le statut est resté le même.
    assert_eq!(
        partenariat::get(&pool, ENTITY, c)?.unwrap().statut,
        STATUT_REFUSE
    );

    // --- D : une fiche d'une autre entité est invisible ---------------------
    let erreur = partenariat::soumettre(&pool, 999, a).expect_err("autre entité");
    assert!(erreur.to_string().contains("introuvable"), "{erreur}");

    effacer(&pool, &reference_a);
    effacer(&pool, &reference_c);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refus_motive_et_motif_borne() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_jetable(&pool, "motif", 1)?;
    let auteur = auteur(&pool);

    // Un refus avec son motif : le motif est relu tel qu'il a été saisi.
    let reference_motivee = ref_test("motif_court");
    effacer(&pool, &reference_motivee);
    let courte = partenariat::creer(
        &pool,
        ENTITY,
        &nouveau(&reference_motivee, type_test.id, TIERS_FICTIF + 9, auteur),
    )?;
    partenariat::soumettre(&pool, ENTITY, courte)?;
    let motif = "Devis trop élevé, voir la note du 3 novembre";
    partenariat::refuser(&pool, ENTITY, courte, motif)?;
    let lu = partenariat::get(&pool, ENTITY, courte)?.unwrap();
    assert_eq!(lu.statut, STATUT_REFUSE);
    assert_eq!(lu.motif.as_deref(), Some(motif));

    // Un refus sans motif laisse la colonne à NULL, il ne l'écrit pas vide.
    let reference_muette = ref_test("motif_vide");
    effacer(&pool, &reference_muette);
    let muette = partenariat::creer(
        &pool,
        ENTITY,
        &nouveau(&reference_muette, type_test.id, TIERS_FICTIF + 10, auteur),
    )?;
    partenariat::soumettre(&pool, ENTITY, muette)?;
    partenariat::refuser(&pool, ENTITY, muette, "   ")?;
    assert_eq!(
        partenariat::get(&pool, ENTITY, muette)?.unwrap().motif,
        None
    );

    // Un motif démesuré est borné, sans casser un caractère : 70 000 « é » font
    // 140 000 octets, on n'en garde que 65 000 — un compte pair, donc aucun
    // accent coupé en deux.
    let reference_longue = ref_test("motif_long");
    effacer(&pool, &reference_longue);
    let longue = partenariat::creer(
        &pool,
        ENTITY,
        &nouveau(&reference_longue, type_test.id, TIERS_FICTIF + 11, auteur),
    )?;
    partenariat::soumettre(&pool, ENTITY, longue)?;
    let enorme = "é".repeat(70_000);
    assert_eq!(enorme.len(), 140_000);
    partenariat::refuser(&pool, ENTITY, longue, &enorme)?;
    let garde = partenariat::get(&pool, ENTITY, longue)?
        .unwrap()
        .motif
        .expect("motif borné, mais écrit");
    assert_eq!(garde.len(), MOTIF_MAX);
    assert_eq!(
        garde.chars().count(),
        MOTIF_MAX / 2,
        "65 000 octets, ce sont 32 500 « é » de deux octets : la coupe n'a pas \
         entamé un caractère"
    );
    assert!(garde.starts_with('é'));

    effacer(&pool, &reference_motivee);
    effacer(&pool, &reference_muette);
    effacer(&pool, &reference_longue);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn compte_les_partenariats_en_cours() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let type_test = type_jetable(&pool, "encours", 1)?;
    let auteur = auteur(&pool);

    let avant = partenariat::en_cours(&pool, ENTITY)?;

    // Trois partenariats acceptés : sans terme, à terme futur, et expiré. Seuls
    // les deux premiers courent encore. Les dates de début diffèrent toutes, sans
    // quoi `uk_fk_type_fk_soc` refuserait le deuxième.
    let sans_terme = ref_test("encours_sans_terme");
    let a_terme = ref_test("encours_a_terme");
    let expire = ref_test("encours_expire");
    for r in [&sans_terme, &a_terme, &expire] {
        effacer(&pool, r);
    }

    let mut premier = nouveau(&sans_terme, type_test.id, TIERS_FICTIF + 12, auteur);
    premier.date_partnership_start = date_relative(&pool, -10)?;
    premier.date_partnership_end = None;

    let mut second = nouveau(&a_terme, type_test.id, TIERS_FICTIF + 12, auteur);
    second.date_partnership_start = date_relative(&pool, -5)?;
    second.date_partnership_end = Some(date_relative(&pool, 10)?);

    let mut troisieme = nouveau(&expire, type_test.id, TIERS_FICTIF + 12, auteur);
    troisieme.date_partnership_start = date_relative(&pool, -30)?;
    troisieme.date_partnership_end = Some(date_relative(&pool, -1)?);

    let mut ids = Vec::new();
    for new in [&premier, &second, &troisieme] {
        let id = partenariat::creer(&pool, ENTITY, new)?;
        partenariat::soumettre(&pool, ENTITY, id)?;
        partenariat::accepter(&pool, ENTITY, id)?;
        ids.push(id);
    }

    let apres = partenariat::en_cours(&pool, ENTITY)?;
    assert_eq!(
        apres,
        avant + 2,
        "seuls le sans-terme et le à-terme futur courent ; l'expiré est exclu"
    );

    // Un brouillon ne compte pas non plus : seul un accepté court.
    let reference_brouillon = ref_test("encours_brouillon");
    effacer(&pool, &reference_brouillon);
    let brouillon = partenariat::creer(
        &pool,
        ENTITY,
        &nouveau(
            &reference_brouillon,
            type_test.id,
            TIERS_FICTIF + 13,
            auteur,
        ),
    )?;
    assert!(brouillon > 0);
    assert_eq!(
        partenariat::en_cours(&pool, ENTITY)?,
        apres,
        "un brouillon non soumis ne compte pas"
    );

    // Nettoyage, puis retour exact au compte de départ : c'est l'écart mesuré.
    for id in ids {
        let lu = partenariat::get(&pool, ENTITY, id)?.expect("relu avant suppression");
        effacer(&pool, &lu.ref_);
    }
    effacer(&pool, &reference_brouillon);
    assert_eq!(
        partenariat::en_cours(&pool, ENTITY)?,
        avant,
        "après nettoyage, le compteur revient à sa valeur de départ"
    );

    type_test.oter(&pool);
    Ok(())
}

#[test]
fn types_actifs_pour_le_menu() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = verrou();
    let actif = type_jetable(&pool, "menu_actif", 1)?;
    // Un second type actif dont le libellé vient après le premier dans **tous**
    // les interclassements : même préfixe, puis « menu_actif » / « menu_zactif ».
    // Comparer deux libellés voisins évite de dépendre de la collation du
    // serveur (insensible à la casse et aux accents), que le tri de Rust ne
    // reproduit pas.
    let suivant = type_jetable(&pool, "menu_zactif", 1)?;
    let inactif = type_jetable(&pool, "menu_inactif", 0)?;

    let liste = partenariat::types(&pool)?;
    assert!(
        !liste.is_empty(),
        "deux types actifs viennent d'être créés : le menu ne peut pas être vide"
    );
    assert!(
        liste
            .iter()
            .any(|(id, label)| *id == actif.id && label == &actif.label),
        "le type actif doit sortir avec son libellé : {liste:?}"
    );
    assert!(
        liste.iter().any(|(id, _)| *id == suivant.id),
        "le second type actif doit sortir aussi : {liste:?}"
    );
    assert!(
        liste.iter().all(|(id, _)| *id != inactif.id),
        "un type désactivé ne doit pas être proposé à la saisie"
    );

    // Les types sortent par libellé croissant : le « a » avant le « z ».
    let position = |cherche: i64| liste.iter().position(|(id, _)| *id == cherche);
    let (premier, second) = match (position(actif.id), position(suivant.id)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err("les deux types actifs devraient être dans la liste".into()),
    };
    assert!(
        premier < second,
        "les types sortent par libellé croissant : {liste:?}"
    );

    actif.oter(&pool);
    suivant.oter(&pool);
    inactif.oter(&pool);
    Ok(())
}
