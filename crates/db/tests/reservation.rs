//! BookCal : découpage pur des créneaux, puis vie réelle contre la base.
//!
//! Deux familles de tests, volontairement séparées :
//!
//! * **Les tests purs** ne demandent ni serveur ni base. Ils vérifient la règle
//!   de découpage, qui est le cœur du module : une plage 9 h-12 h découpée en
//!   créneaux de 60 ou de 90 minutes.
//! * **Les tests d'intégration** écrivent vraiment dans `llx_bookcal_calendar`,
//!   `llx_bookcal_availabilities` et `llx_actioncomm`. Ils sont ignorés si la
//!   variable `DOLIBARR_DSN` n'est pas posée.
//!
//! Les tests d'intégration tournent en parallèle sur la même base : chaque test
//! porte un suffixe de libellé distinct et ne nettoie **que** ses propres
//! fiches. Aucune assertion ne porte sur un `COUNT(*)` global — sinon deux tests
//! lancés ensemble se compteraient mutuellement.
//!
//! L'entité de travail est la 1.

use std::error::Error;

use db::reservation::{
    creer_calendrier, creer_plage, creneaux_libres, decouper_plages, heure_en_minutes,
    minutes_en_heure, plages, reservations, reserver, se_recouvrent, Calendrier, NouveauCalendrier,
    NouvellePlage, PlageHoraire,
};
use mysql::prelude::*;
use mysql::{params, Pool};

const ENTITY: i64 = 1;

/// Préfixe commun : le nettoyage s'appuie dessus.
const PREFIXE: &str = "ZZ BookCal Test";

/// Jour de travail des tests. Une date fixe, loin devant, pour qu'aucun
/// calendrier réel ne la croise.
const JOUR: &str = "2031-03-10";

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

/// Supprime les restes d'un test précédent interrompu, pour que la suite soit
/// rejouable sans intervention manuelle.
///
/// L'ordre compte : `llx_actioncomm` et `llx_bookcal_availabilities` désignent
/// le calendrier, on les efface avant lui. Comme ces deux tables n'ont pas de
/// colonne `entity`, le cloisonnement passe par la sous-requête sur le
/// calendrier — c'est exactement la contrainte relevée à la mesure.
fn nettoyer(pool: &Pool, suffixe: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let motif = format!("{PREFIXE} {suffixe}%");

    conn.exec_drop(
        "DELETE FROM llx_actioncomm WHERE fk_bookcal_calendar IN ( \
           SELECT rowid FROM llx_bookcal_calendar WHERE entity = :entity AND label LIKE :motif)",
        params! { "entity" => ENTITY, "motif" => motif.as_str() },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_bookcal_availabilities WHERE fk_bookcal_calendar IN ( \
           SELECT rowid FROM llx_bookcal_calendar WHERE entity = :entity AND label LIKE :motif)",
        params! { "entity" => ENTITY, "motif" => motif.as_str() },
    )?;
    conn.exec_drop(
        "DELETE FROM llx_bookcal_calendar WHERE entity = :entity AND label LIKE :motif",
        params! { "entity" => ENTITY, "motif" => motif.as_str() },
    )?;
    Ok(())
}

/// Premier compte de l'entité, pour satisfaire `fk_user_creat` qui est
/// `NOT NULL` sans valeur par défaut.
fn utilisateur(pool: &Pool) -> Result<Option<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rowid: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity ORDER BY rowid LIMIT 1",
        params! { "entity" => ENTITY },
    )?;
    Ok(rowid)
}

/// Prépare un calendrier propre avec une plage 9 h-12 h sur [`JOUR`].
fn preparer(pool: &Pool, suffixe: &str) -> Result<Option<(i64, i64)>, Box<dyn Error>> {
    nettoyer(pool, suffixe)?;
    let Some(user) = utilisateur(pool)? else {
        return Ok(None);
    };

    let libelle = format!("{PREFIXE} {suffixe}");
    let calendrier = creer_calendrier(
        pool,
        ENTITY,
        &NouveauCalendrier {
            label: &libelle,
            description: Some("Calendrier de test"),
            fk_soc: None,
            fk_user_creat: user,
            note_public: None,
        },
    )?;

    creer_plage(
        pool,
        ENTITY,
        &NouvellePlage {
            fk_bookcal_calendar: calendrier,
            jour_debut: JOUR,
            jour_fin: JOUR,
            heure_debut: "09:00",
            heure_fin: "12:00",
            fk_user_creat: user,
        },
    )?;

    Ok(Some((calendrier, user)))
}

// ---------------------------------------------------------------------------
// Tests purs : la règle de découpage, sans base.
// ---------------------------------------------------------------------------

/// Une plage de trois heures en créneaux d'une heure donne trois départs.
#[test]
fn decoupe_une_plage_de_trois_heures_en_creneaux_d_une_heure() {
    let plages = [PlageHoraire::nouvelle(9 * 60, 12 * 60)];
    let creneaux = decouper_plages(&plages, 60).expect("découpage valide");
    assert_eq!(creneaux, ["09:00", "10:00", "11:00"]);
}

/// En créneaux de 90 minutes il n'en reste que deux : le troisième
/// dépasserait 12 h, on l'abandonne plutôt que de déborder.
#[test]
fn decoupe_une_plage_de_trois_heures_en_creneaux_de_quatre_vingt_dix_minutes() {
    let plages = [PlageHoraire::nouvelle(9 * 60, 12 * 60)];
    let creneaux = decouper_plages(&plages, 90).expect("découpage valide");
    assert_eq!(creneaux, ["09:00", "10:30"]);
}

/// Une plage plus courte que la durée demandée ne donne aucun créneau.
#[test]
fn une_plage_plus_courte_que_la_duree_ne_donne_aucun_creneau() {
    let plages = [PlageHoraire::nouvelle(9 * 60, 10 * 60)];
    let creneaux = decouper_plages(&plages, 90).expect("découpage valide");
    assert!(creneaux.is_empty(), "aucun créneau ne tient en une heure");
}

/// Plusieurs plages le même jour : les départs sont fusionnés, triés et
/// dédoublonnés. Deux plages jointives doivent donner la même grille qu'une
/// seule plage continue.
#[test]
fn plusieurs_plages_le_meme_jour_sont_fusionnees_et_triees() {
    // Données volontairement dans le désordre, et en double.
    let plages = [
        PlageHoraire::nouvelle(11 * 60, 12 * 60),
        PlageHoraire::nouvelle(9 * 60, 11 * 60),
        PlageHoraire::nouvelle(9 * 60, 11 * 60),
    ];
    let creneaux = decouper_plages(&plages, 60).expect("découpage valide");
    assert_eq!(creneaux, ["09:00", "10:00", "11:00"]);
}

/// Une durée nulle est refusée : elle produirait une infinité de créneaux au
/// même endroit, ou une boucle sans fin.
#[test]
fn une_duree_nulle_est_refusee() {
    let plages = [PlageHoraire::nouvelle(9 * 60, 12 * 60)];
    assert!(decouper_plages(&plages, 0).is_err());
    assert!(decouper_plages(&plages, -30).is_err());
}

/// Deux créneaux jointifs ne se recouvrent pas ; deux créneaux croisés oui.
#[test]
fn le_recouvrement_ne_confond_pas_jointif_et_croise() {
    assert!(!se_recouvrent(9 * 60, 10 * 60, 10 * 60, 11 * 60));
    assert!(se_recouvrent(9 * 60, 10 * 60 + 30, 10 * 60, 11 * 60));
}

/// Les conversions d'heure sont l'inverse l'une de l'autre.
#[test]
fn les_conversions_d_heure_sont_reversibles() {
    for minutes in [0, 60, 570, 720, 1439] {
        let texte = minutes_en_heure(minutes);
        assert_eq!(heure_en_minutes(&texte).expect("heure valide"), minutes);
    }
}

// ---------------------------------------------------------------------------
// Tests d'intégration : la base réelle.
// ---------------------------------------------------------------------------

/// Un calendrier et sa plage s'écrivent et se relisent.
#[test]
fn cree_un_calendrier_et_ses_plages() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some((calendrier, _user)) = preparer(&pool, "creation")? else {
        return Ok(());
    };

    let liste = plages(&pool, ENTITY, calendrier)?;
    assert_eq!(liste.len(), 1, "une plage doit avoir été créée");
    assert_eq!(liste[0].jour_debut, JOUR);
    assert_eq!(liste[0].jour_fin, JOUR);
    assert_eq!(liste[0].heure_debut, "09:00");
    assert_eq!(liste[0].heure_fin, "12:00");

    let tous: Vec<Calendrier> = db::reservation::calendriers(&pool, ENTITY, 500)?;
    let trouve = tous
        .iter()
        .find(|c| c.rowid == calendrier)
        .expect("le calendrier créé apparaît dans la liste");
    assert_eq!(trouve.libelle, format!("{PREFIXE} creation"));
    // La référence est fabriquée par le module : Dolibarr ne sait pas la
    // générer (`getNextNumRef()` renvoie '').
    assert!(
        trouve.reference.starts_with("BCAL-"),
        "référence inattendue : {}",
        trouve.reference
    );

    nettoyer(&pool, "creation")?;
    Ok(())
}

/// Un jour ouvert rend trois créneaux d'une heure.
#[test]
fn creneaux_libres_sur_un_jour_ouvert() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some((calendrier, _user)) = preparer(&pool, "creneaux")? else {
        return Ok(());
    };

    let creneaux = creneaux_libres(&pool, ENTITY, calendrier, JOUR, 60)?;
    assert_eq!(creneaux, ["09:00", "10:00", "11:00"]);

    // Un jour hors de la plage n'ouvre rien, et ce n'est pas une erreur.
    let ferme = creneaux_libres(&pool, ENTITY, calendrier, "2031-03-11", 60)?;
    assert!(ferme.is_empty());

    nettoyer(&pool, "creneaux")?;
    Ok(())
}

/// Réserver écrit un vrai rendez-vous dans l'agenda et retire le créneau.
#[test]
fn une_reservation_occupe_son_creneau() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some((calendrier, _user)) = preparer(&pool, "occupation")? else {
        return Ok(());
    };

    let id = reserver(
        &pool,
        ENTITY,
        calendrier,
        JOUR,
        "10:00",
        60,
        None,
        "Premier rendez-vous",
    )?;
    assert!(id > 0);

    // La réservation est bien dans `llx_actioncomm`, pas dans les plages.
    let mut conn = pool.get_conn()?;
    let (code, debut, fin, ref_): (String, String, Option<String>, String) = conn
        .exec_first(
            "SELECT code, DATE_FORMAT(datep, '%Y-%m-%d %H:%i:%s'), \
                DATE_FORMAT(datep2, '%Y-%m-%d %H:%i:%s'), ref \
         FROM llx_actioncomm WHERE id = :id",
            params! { "id" => id },
        )?
        .expect("le rendez-vous existe");
    assert_eq!(code, "AC_RDV");
    assert_eq!(debut, format!("{JOUR} 10:00:00"));
    let fin_attendue = format!("{JOUR} 11:00:00");
    assert_eq!(fin.as_deref(), Some(fin_attendue.as_str()));
    assert_eq!(ref_, format!("(AC{id})"));

    // Le créneau a disparu de la grille, les deux autres restent.
    let creneaux = creneaux_libres(&pool, ENTITY, calendrier, JOUR, 60)?;
    assert_eq!(creneaux, ["09:00", "11:00"]);

    let prises = reservations(&pool, ENTITY, calendrier, JOUR)?;
    assert_eq!(prises.len(), 1);
    assert_eq!(prises[0].rowid, id);
    assert_eq!(prises[0].note.as_deref(), Some("Premier rendez-vous"));

    nettoyer(&pool, "occupation")?;
    Ok(())
}

/// Deux réservations qui se croisent sans commencer à la même heure sont
/// refusées : c'est le cas 9 h 30-10 h 30 contre 10 h-11 h.
#[test]
fn une_reservation_qui_se_recouvre_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some((calendrier, _user)) = preparer(&pool, "doublon")? else {
        return Ok(());
    };

    reserver(&pool, ENTITY, calendrier, JOUR, "10:00", 60, None, "")?;

    // Même heure, même durée.
    let identique = reserver(&pool, ENTITY, calendrier, JOUR, "10:00", 60, None, "");
    assert!(identique.is_err(), "un doublon exact doit être refusé");

    // Heure différente, créneaux croisés : 9 h 30-10 h 30 mord sur 10 h-11 h.
    let croise = reserver(&pool, ENTITY, calendrier, JOUR, "09:30", 60, None, "");
    assert!(
        croise.is_err(),
        "un créneau qui se recouvre sans commencer à la même heure doit être refusé"
    );

    // Créneau jointif accepté : 11 h-12 h commence quand l'autre finit.
    let jointif = reserver(&pool, ENTITY, calendrier, JOUR, "11:00", 60, None, "")?;
    assert!(jointif > 0);

    let prises = reservations(&pool, ENTITY, calendrier, JOUR)?;
    assert_eq!(
        prises.len(),
        2,
        "seules deux réservations ont été acceptées"
    );

    nettoyer(&pool, "doublon")?;
    Ok(())
}

/// Un créneau en dehors des plages ouvertes est refusé.
#[test]
fn une_reservation_hors_plage_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some((calendrier, _user)) = preparer(&pool, "horsplage")? else {
        return Ok(());
    };

    // 13 h est après la fermeture (12 h).
    assert!(reserver(&pool, ENTITY, calendrier, JOUR, "13:00", 60, None, "").is_err());
    // 08 h est avant l'ouverture (9 h).
    assert!(reserver(&pool, ENTITY, calendrier, JOUR, "08:00", 60, None, "").is_err());
    // 11 h 30 + 60 min déborde de la fermeture.
    assert!(reserver(&pool, ENTITY, calendrier, JOUR, "11:30", 60, None, "").is_err());

    assert!(reservations(&pool, ENTITY, calendrier, JOUR)?.is_empty());

    nettoyer(&pool, "horsplage")?;
    Ok(())
}

/// Une durée non strictement positive est refusée.
#[test]
fn une_reservation_de_duree_nulle_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some((calendrier, _user)) = preparer(&pool, "duree")? else {
        return Ok(());
    };

    assert!(reserver(&pool, ENTITY, calendrier, JOUR, "10:00", 0, None, "").is_err());
    assert!(reserver(&pool, ENTITY, calendrier, JOUR, "10:00", -15, None, "").is_err());
    assert!(creneaux_libres(&pool, ENTITY, calendrier, JOUR, 0).is_err());

    nettoyer(&pool, "duree")?;
    Ok(())
}

/// Un libellé vide ne crée rien.
#[test]
fn un_libelle_vide_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(user) = utilisateur(&pool)? else {
        return Ok(());
    };

    // Pas de nettoyage préalable nécessaire : rien ne doit être écrit.
    assert!(creer_calendrier(
        &pool,
        ENTITY,
        &NouveauCalendrier {
            label: "   ",
            description: None,
            fk_soc: None,
            fk_user_creat: user,
            note_public: None,
        },
    )
    .is_err());

    Ok(())
}

/// Un calendrier inexistant est refusé, pour une plage comme pour une
/// réservation ou un calcul de créneaux.
#[test]
fn un_calendrier_inexistant_est_refuse() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(user) = utilisateur(&pool)? else {
        return Ok(());
    };

    let inconnu = 99_999_999_i64;

    assert!(creer_plage(
        &pool,
        ENTITY,
        &NouvellePlage {
            fk_bookcal_calendar: inconnu,
            jour_debut: JOUR,
            jour_fin: JOUR,
            heure_debut: "09:00",
            heure_fin: "12:00",
            fk_user_creat: user,
        },
    )
    .is_err());

    assert!(creneaux_libres(&pool, ENTITY, inconnu, JOUR, 60).is_err());
    assert!(reserver(&pool, ENTITY, inconnu, JOUR, "10:00", 60, None, "").is_err());

    Ok(())
}

/// Les heures et les dates mal formées sont refusées avant tout accès à la
/// base : `9:00` n'est pas `09:00`, et le 31 avril n'existe pas.
#[test]
fn une_heure_ou_une_date_mal_formee_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let Some(user) = utilisateur(&pool)? else {
        return Ok(());
    };
    let Some((calendrier, _)) = preparer(&pool, "formats")? else {
        return Ok(());
    };

    for mauvaise in ["9:00", "09h00", "24:00", "09:60", ""] {
        assert!(
            creer_plage(
                &pool,
                ENTITY,
                &NouvellePlage {
                    fk_bookcal_calendar: calendrier,
                    jour_debut: JOUR,
                    jour_fin: JOUR,
                    heure_debut: mauvaise,
                    heure_fin: "12:00",
                    fk_user_creat: user,
                },
            )
            .is_err(),
            "l'heure « {mauvaise} » aurait dû être refusée"
        );
    }

    // Fin avant début.
    assert!(creer_plage(
        &pool,
        ENTITY,
        &NouvellePlage {
            fk_bookcal_calendar: calendrier,
            jour_debut: JOUR,
            jour_fin: JOUR,
            heure_debut: "12:00",
            heure_fin: "09:00",
            fk_user_creat: user,
        },
    )
    .is_err());

    // Fin égale au début.
    assert!(creer_plage(
        &pool,
        ENTITY,
        &NouvellePlage {
            fk_bookcal_calendar: calendrier,
            jour_debut: JOUR,
            jour_fin: JOUR,
            heure_debut: "09:00",
            heure_fin: "09:00",
            fk_user_creat: user,
        },
    )
    .is_err());

    // Date impossible et date inversée.
    assert!(creneaux_libres(&pool, ENTITY, calendrier, "2031-04-31", 60).is_err());
    assert!(creer_plage(
        &pool,
        ENTITY,
        &NouvellePlage {
            fk_bookcal_calendar: calendrier,
            jour_debut: "2031-03-12",
            jour_fin: "2031-03-10",
            heure_debut: "09:00",
            heure_fin: "12:00",
            fk_user_creat: user,
        },
    )
    .is_err());

    nettoyer(&pool, "formats")?;
    Ok(())
}
