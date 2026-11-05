//! Tests d'intégration du module Congés.
//!
//! Les tests qui touchent la base ne tournent que si `DOLIBARR_DSN` est posé :
//! sans base, ils s'ignorent en le disant. Les tests de [`conges::jours_ouvres`]
//! sont purs et tournent toujours.
//!
//! Tout est écrit en entité 1 et nettoyé derrière : les seules lignes laissées
//! sont celles qui existaient avant le test.

use std::error::Error;
use std::str::FromStr;

use db::conges::{self, NewConge, STATUT_APPROUVE, STATUT_DEMANDE, STATUT_REFUSE, STATUT_VALIDE};
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
            eprintln!("connexion impossible ({e}) : test ignoré");
            None
        }
    }
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("décimale de test")
}

/// Un utilisateur réel de l'entité 1 : `llx_holiday.fk_user` n'a aucune clé
/// étrangère, on ne veut donc pas d'un identifiant inventé.
fn utilisateur(pool: &Pool) -> Option<i64> {
    let mut conn = pool.get_conn().ok()?;
    conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity ORDER BY rowid LIMIT 1",
        params! { "entity" => ENTITY },
    )
    .ok()
    .flatten()
}

fn login_de(pool: &Pool, rowid: i64) -> Option<String> {
    let mut conn = pool.get_conn().ok()?;
    conn.exec_first(
        "SELECT login FROM llx_user WHERE rowid = :rowid",
        params! { "rowid" => rowid },
    )
    .ok()
    .flatten()
}

/// Un type de congé utilisable par les tests.
struct TypeTest {
    id: i64,
    /// Vrai quand la ligne a été créée par le test : elle est alors supprimée.
    /// On ne touche jamais au dictionnaire réel d'une instance.
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
            "DELETE FROM llx_c_holiday_types WHERE rowid = :rowid",
            params! { "rowid" => self.id },
        );
    }
}

/// Un type de congé actif : celui de l'instance s'il en existe un, sinon un type
/// jetable créé pour le test. `llx_c_holiday_types` porte une clé unique
/// `(entity, code)`, d'où le nettoyage préalable d'un reliquat de test.
fn type_conge(pool: &Pool) -> Result<Option<TypeTest>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existant: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_c_holiday_types WHERE entity = :entity AND active = 1 \
         ORDER BY rowid LIMIT 1",
        params! { "entity" => ENTITY },
    )?;
    if let Some(id) = existant {
        return Ok(Some(TypeTest {
            id,
            temporaire: false,
        }));
    }

    let code = format!("ZZCP{}", std::process::id());
    let _ = conn.exec_drop(
        "DELETE FROM llx_c_holiday_types WHERE entity = :entity AND code = :code",
        params! { "entity" => ENTITY, "code" => code.as_str() },
    );
    conn.exec_drop(
        "INSERT INTO llx_c_holiday_types \
         (entity, code, label, affect, delay, newbymonth, block_if_negative, sortorder, active) \
         VALUES (:entity, :code, 'Congé de test', 1, 1, 0, 0, 0, 1)",
        params! { "entity" => ENTITY, "code" => code.as_str() },
    )?;
    Ok(Some(TypeTest {
        id: conn.last_insert_id() as i64,
        temporaire: true,
    }))
}

fn demande(salarie: i64, type_id: i64, debut: &str, fin: &str) -> NewConge {
    NewConge {
        fk_user: salarie,
        fk_type: type_id,
        description: "Congé de test".into(),
        date_debut: debut.into(),
        date_fin: fin.into(),
        halfday: false,
        // Le valideur désigné est le salarié lui-même : la colonne est NOT NULL
        // et un zéro y serait un mensonge.
        fk_validator: salarie,
        note_private: None,
        note_public: None,
    }
}

fn effacer(pool: &Pool, rowid: i64) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    let _ = conn.exec_drop(
        "DELETE FROM llx_holiday WHERE rowid = :rowid",
        params! { "rowid" => rowid },
    );
}

// ---------------------------------------------------------------------------
// Le décompte des jours : fonction pure, aucun besoin de base
// ---------------------------------------------------------------------------

/// Du lundi 2 au vendredi 6 novembre 2026 : cinq jours pleins.
#[test]
fn jours_ouvres_compte_les_jours_de_semaine() {
    let total = conges::jours_ouvres("2026-11-02", "2026-11-06", false).expect("période valide");
    assert_eq!(total.normalize(), dec("5"));
    // Un seul jour ouvré vaut 1, pas 0 : les bornes comptent toutes les deux.
    assert_eq!(
        conges::jours_ouvres("2026-11-02", "2026-11-02", false)
            .expect("période valide")
            .normalize(),
        dec("1")
    );
}

#[test]
fn jours_ouvres_d_un_week_end_vaut_zero() {
    // Samedi 7 et dimanche 8 novembre 2026.
    let total = conges::jours_ouvres("2026-11-07", "2026-11-08", false).expect("période valide");
    assert_eq!(total.normalize(), dec("0"));
    // Un seul samedi vaut aussi zéro.
    assert_eq!(
        conges::jours_ouvres("2026-11-07", "2026-11-07", false)
            .expect("période valide")
            .normalize(),
        dec("0")
    );
}

/// Vendredi 30 octobre au lundi 2 novembre 2026 : la période traverse une fin de
/// mois et le week-end. Deux jours ouvrés seulement.
#[test]
fn jours_ouvres_traverse_une_fin_de_mois() {
    let total = conges::jours_ouvres("2026-10-30", "2026-11-02", false).expect("période valide");
    assert_eq!(total.normalize(), dec("2"));
}

#[test]
fn jours_ouvres_en_demi_journees_divise_par_deux() {
    let total = conges::jours_ouvres("2026-11-02", "2026-11-06", true).expect("période valide");
    assert_eq!(total.normalize(), dec("2.5"));
}

#[test]
fn jours_ouvres_refuse_une_date_invalide() {
    let erreur = conges::jours_ouvres("2026-02-30", "2026-03-02", false).expect_err("30 février");
    assert!(erreur.to_string().contains("date invalide"), "{erreur}");
    assert!(conges::jours_ouvres("02/11/2026", "2026-11-06", false).is_err());
    assert!(conges::jours_ouvres("2026-11-02", "2026-11-3", false).is_err());
}

#[test]
fn jours_ouvres_refuse_une_periode_inversee() {
    let erreur = conges::jours_ouvres("2026-11-06", "2026-11-02", false).expect_err("période");
    assert!(erreur.to_string().contains("précéder"), "{erreur}");
    assert_eq!(
        erreur,
        conges::Erreur::PeriodeInversee {
            debut: "2026-11-06".into(),
            fin: "2026-11-02".into(),
        }
    );
}

// ---------------------------------------------------------------------------
// Création
// ---------------------------------------------------------------------------

#[test]
fn cree_une_demande_et_stocke_les_jours_ouvres() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(type_test)) = (utilisateur(&pool), type_conge(&pool)?) else {
        eprintln!("aucun utilisateur en base : test ignoré");
        return Ok(());
    };

    let rowid = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, type_test.id, "2026-11-02", "2026-11-06"),
    )?;

    let c = conges::get(&pool, ENTITY, rowid)?.expect("le congé créé doit se relire");
    assert_eq!(c.entity, ENTITY);
    assert_eq!(
        c.statut, STATUT_DEMANDE,
        "une demande naît au statut demandé"
    );
    assert_eq!(c.fk_user, salarie);
    assert_eq!(c.fk_type, type_test.id);
    assert_eq!(c.description, "Congé de test");
    assert_eq!(c.date_debut, "2026-11-02");
    assert_eq!(c.date_fin, "2026-11-06");
    assert!(!c.halfday);
    // 5 jours ouvrés pour un lundi-vendredi : la valeur est écrite en base.
    assert_eq!(c.nb_open_day.normalize(), dec("5"));
    assert!(c.date_create.is_some(), "la date de saisie est posée");
    assert!(c.date_valid.is_none() && c.date_approval.is_none());
    assert!(
        c.ref_.starts_with("CP") && c.ref_[2..].chars().all(|ch| ch.is_ascii_digit()),
        "référence inattendue : {}",
        c.ref_
    );
    assert_eq!(
        c.demandeur,
        login_de(&pool, salarie),
        "le login suit la demande"
    );
    assert!(c.type_conge.is_some(), "le libellé du type suit la demande");

    // La liste doit la contenir, triée du plus récent au plus ancien.
    let liste = conges::list(&pool, ENTITY, 100_000, 0)?;
    assert!(liste.iter().any(|l| l.rowid == rowid));
    assert!(
        liste
            .windows(2)
            .all(|paire| paire[0].date_debut >= paire[1].date_debut),
        "la liste doit être triée par date de début décroissante"
    );

    effacer(&pool, rowid);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn cree_une_demande_en_demi_journee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(type_test)) = (utilisateur(&pool), type_conge(&pool)?) else {
        return Ok(());
    };

    let mut new = demande(salarie, type_test.id, "2026-11-02", "2026-11-06");
    new.halfday = true;
    let rowid = conges::create(&pool, ENTITY, &new)?;

    let c = conges::get(&pool, ENTITY, rowid)?.expect("congé");
    assert!(c.halfday, "l'indicateur de demi-journée est conservé");
    assert_eq!(c.nb_open_day.normalize(), dec("2.5"));

    effacer(&pool, rowid);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_des_dates_inversees() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(type_test)) = (utilisateur(&pool), type_conge(&pool)?) else {
        return Ok(());
    };

    let erreur = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, type_test.id, "2026-11-06", "2026-11-02"),
    )
    .expect_err("période inversée refusée");
    assert!(erreur.to_string().contains("précéder"), "{erreur}");

    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_une_description_vide() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(type_test)) = (utilisateur(&pool), type_conge(&pool)?) else {
        return Ok(());
    };

    let mut new = demande(salarie, type_test.id, "2026-11-02", "2026-11-06");
    new.description = "   ".into();
    let erreur = conges::create(&pool, ENTITY, &new).expect_err("description vide refusée");
    assert!(erreur.to_string().contains("description"), "{erreur}");

    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_un_type_de_conge_inconnu() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(salarie) = utilisateur(&pool) else {
        return Ok(());
    };

    let erreur = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, 9_999_999, "2026-11-02", "2026-11-06"),
    )
    .expect_err("type inconnu refusé");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");
    Ok(())
}

/// Un type désactivé reste en base (les congés passés doivent s'afficher) mais
/// ne doit plus servir à en créer un.
#[test]
fn refuse_un_type_de_conge_inactif() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(salarie) = utilisateur(&pool) else {
        return Ok(());
    };

    let code = format!("ZZCI{}", std::process::id());
    let mut conn = pool.get_conn()?;
    let _ = conn.exec_drop(
        "DELETE FROM llx_c_holiday_types WHERE entity = :entity AND code = :code",
        params! { "entity" => ENTITY, "code" => code.as_str() },
    );
    conn.exec_drop(
        "INSERT INTO llx_c_holiday_types \
         (entity, code, label, affect, delay, newbymonth, block_if_negative, sortorder, active) \
         VALUES (:entity, :code, 'Congé de test désactivé', 1, 1, 0, 0, 0, 0)",
        params! { "entity" => ENTITY, "code" => code.as_str() },
    )?;
    let desactive: i64 = conn.last_insert_id() as i64;

    let resultat = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, desactive, "2026-11-02", "2026-11-06"),
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_c_holiday_types WHERE rowid = :rowid",
        params! { "rowid" => desactive },
    );

    let erreur = resultat.expect_err("un type désactivé doit être refusé");
    assert!(erreur.to_string().contains("désactivé"), "{erreur}");
    Ok(())
}

#[test]
fn refuse_un_utilisateur_inconnu() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(type_test) = type_conge(&pool)? else {
        return Ok(());
    };

    let erreur = conges::create(
        &pool,
        ENTITY,
        &demande(9_999_999, type_test.id, "2026-11-02", "2026-11-06"),
    )
    .expect_err("salarié inconnu refusé");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");

    type_test.oter(&pool);
    Ok(())
}

// ---------------------------------------------------------------------------
// Circuit : validation, approbation, refus, annulation
// ---------------------------------------------------------------------------

#[test]
fn valide_puis_refuse_de_revalider() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(type_test)) = (utilisateur(&pool), type_conge(&pool)?) else {
        return Ok(());
    };
    let rowid = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, type_test.id, "2026-11-02", "2026-11-06"),
    )?;

    conges::valider(&pool, ENTITY, rowid, salarie)?;
    let c = conges::get(&pool, ENTITY, rowid)?.expect("congé");
    assert_eq!(c.statut, STATUT_VALIDE);
    assert!(c.date_valid.is_some(), "la validation date la demande");
    assert_eq!(c.fk_user_valid, Some(salarie));
    assert_eq!(
        c.fk_validator, salarie,
        "le valideur réel remplace le désigné"
    );

    let erreur = conges::valider(&pool, ENTITY, rowid, salarie).expect_err("revalidation refusée");
    assert!(erreur.to_string().contains("déjà"), "{erreur}");

    effacer(&pool, rowid);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn approbation_refusee_si_non_validee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(type_test)) = (utilisateur(&pool), type_conge(&pool)?) else {
        return Ok(());
    };
    let rowid = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, type_test.id, "2026-11-02", "2026-11-06"),
    )?;

    // Une demande encore au statut 1 n'a pas de `date_valid` : l'approuver
    // court-circuiterait la validation.
    let erreur = conges::approuver(&pool, ENTITY, rowid, salarie).expect_err("non validée");
    assert!(erreur.to_string().contains("validée"), "{erreur}");

    conges::valider(&pool, ENTITY, rowid, salarie)?;
    conges::approuver(&pool, ENTITY, rowid, salarie)?;
    let c = conges::get(&pool, ENTITY, rowid)?.expect("congé");
    assert_eq!(c.statut, STATUT_APPROUVE);
    assert!(c.date_approval.is_some());
    assert_eq!(c.fk_user_approve, Some(salarie));

    effacer(&pool, rowid);
    type_test.oter(&pool);
    Ok(())
}

#[test]
fn refuse_une_demande_avec_motif_borne() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(type_test)) = (utilisateur(&pool), type_conge(&pool)?) else {
        return Ok(());
    };
    let rowid = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, type_test.id, "2026-11-02", "2026-11-06"),
    )?;

    // Un motif de 400 octets (200 « é ») doit tenir dans le varchar(250) sans
    // qu'un caractère soit coupé en deux : 125 « é » font 250 octets.
    let motif = "é".repeat(200);
    conges::refuser(&pool, ENTITY, rowid, salarie, &motif)?;
    let c = conges::get(&pool, ENTITY, rowid)?.expect("congé");
    assert_eq!(c.statut, STATUT_REFUSE);
    assert!(c.date_refuse.is_some());
    assert_eq!(c.fk_user_refuse, Some(salarie));
    let garde = c.detail_refuse.expect("motif conservé");
    assert!(garde.len() <= 250, "{} octets écrits", garde.len());
    assert_eq!(garde.chars().count(), 125);
    assert!(motif.starts_with(&garde));

    // Un second refus n'a plus de sens : la demande est déjà hors circuit.
    let erreur =
        conges::refuser(&pool, ENTITY, rowid, salarie, "encore").expect_err("déjà refusée");
    assert!(erreur.to_string().contains("déjà"), "{erreur}");

    effacer(&pool, rowid);
    type_test.oter(&pool);
    Ok(())
}

/// Une absence annulée reste une trace : la ligne ne doit pas disparaître.
#[test]
fn annule_sans_supprimer_la_trace() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(type_test)) = (utilisateur(&pool), type_conge(&pool)?) else {
        return Ok(());
    };
    let rowid = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, type_test.id, "2026-11-02", "2026-11-06"),
    )?;

    conges::annuler(&pool, ENTITY, rowid)?;
    let c = conges::get(&pool, ENTITY, rowid)?.expect("la ligne annulée reste en base");
    // Statut 4 : « hors circuit ». C'est `date_cancel` — et non `date_refuse` —
    // qui distingue une annulation d'un refus, Dolibarr n'ayant qu'un seul code.
    assert_eq!(c.statut, STATUT_REFUSE);
    assert!(c.date_cancel.is_some(), "l'annulation est datée");
    assert!(c.date_refuse.is_none(), "aucun refus n'a eu lieu");
    assert_eq!(c.description, "Congé de test", "la demande reste lisible");

    let erreur = conges::annuler(&pool, ENTITY, rowid).expect_err("déjà annulée");
    assert!(erreur.to_string().contains("déjà"), "{erreur}");

    effacer(&pool, rowid);
    type_test.oter(&pool);
    Ok(())
}

// ---------------------------------------------------------------------------
// Solde
// ---------------------------------------------------------------------------

/// Seuls les congés approuvés (statut 5) entrent dans le solde. Le test mesure
/// des écarts et non des totaux absolus : une base de production peut déjà
/// contenir des congés approuvés pour ce salarié.
#[test]
fn solde_ne_compte_que_les_approuves() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (Some(salarie), Some(type_test)) = (utilisateur(&pool), type_conge(&pool)?) else {
        return Ok(());
    };

    let avant = conges::solde(&pool, ENTITY, salarie, "2026-06-30")?;
    let avant_autre_annee = conges::solde(&pool, ENTITY, salarie, "2032-06-30")?;

    // Approuvé : 5 jours ouvrés du lundi 2 au vendredi 6 novembre 2026.
    let approuve = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, type_test.id, "2026-11-02", "2026-11-06"),
    )?;
    conges::valider(&pool, ENTITY, approuve, salarie)?;
    conges::approuver(&pool, ENTITY, approuve, salarie)?;

    // Validé seulement : rien n'est acquis, le solde ne bouge pas.
    let valide_seul = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, type_test.id, "2026-11-09", "2026-11-13"),
    )?;
    conges::valider(&pool, ENTITY, valide_seul, salarie)?;

    // Refusé : rien n'est consommé.
    let refuse = conges::create(
        &pool,
        ENTITY,
        &demande(salarie, type_test.id, "2026-11-16", "2026-11-20"),
    )?;
    conges::refuser(&pool, ENTITY, refuse, salarie, "effectif insuffisant")?;

    let apres = conges::solde(&pool, ENTITY, salarie, "2026-06-30")?;
    assert_eq!(
        (apres - avant).normalize(),
        dec("5"),
        "seul le congé approuvé entre dans le solde"
    );
    // L'exercice 2032 n'a pas bougé : le filtre porte bien sur l'année.
    assert_eq!(
        conges::solde(&pool, ENTITY, salarie, "2032-06-30")?,
        avant_autre_annee
    );

    for rowid in [approuve, valide_seul, refuse] {
        effacer(&pool, rowid);
    }
    type_test.oter(&pool);
    Ok(())
}
