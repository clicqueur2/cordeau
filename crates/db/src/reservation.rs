//! BookCal : calendriers de rendez-vous, plages d'ouverture et réservations.
//!
//! # Ce que la mesure a montré (et pourquoi le modèle est celui-ci)
//!
//! Le module BookCal de Dolibarr 24.0.1 tient dans **deux** tables, plus une
//! troisième pour les réservations :
//!
//! * `llx_bookcal_calendar` — le calendrier : `entity`, `ref`, `label`,
//!   `fk_soc`, `fk_project`, `status`, `type`, `visibility`, `fk_user_creat`.
//! * `llx_bookcal_availabilities` — les plages : `start` (date), `end` (date),
//!   `startHour` (entier), `endHour` (entier), `duration` (entier, défaut 30),
//!   `status`, `fk_bookcal_calendar`. **Pas de colonne `entity`.**
//! * `llx_actioncomm` — les réservations, avec `code = 'AC_RDV'` et
//!   `fk_bookcal_calendar`.
//!
//! ## Pourquoi une plage est une *période de dates* et pas un jour de semaine
//!
//! La spécification demandait `NouvellePlage { day_of_week, … }` avec un refus
//! d'un jour hors 1..7. **Cette colonne n'existe pas.** Une recherche sur
//! `information_schema` pour les tables `%bookcal%` et les colonnes `%day%` /
//! `%week%` ne rend **aucune ligne** ; les extrafields ne sont pas concernés.
//!
//! La validation « jour 1..7 » est donc remplacée par la validation des deux
//! dates : `AAAA-MM-JJ` réellement existantes (le 31 avril est refusé), et une
//! fin qui ne précède pas le début. C'est la seule borne que la base sache
//! vérifier ; un numéro de jour n'aurait où s'écrire.
//!
//! BookCal exprime la récurrence autrement : une ligne de disponibilité couvre
//! la période `start` → `end`, et chaque jour de cette période est ouvert de
//! `startHour` à `endHour`. « Tous les lundis de 9 h à 12 h » se décrit donc par
//! une ligne qui court sur l'année, pas par un numéro de jour. « Le lundi
//! 10 mars seulement » se décrit par `start = end = 2026-03-10`.
//!
//! J'ai donc retenu `jour_debut` / `jour_fin` plutôt que `day_of_week` : c'est
//! la seule forme qui s'écrive dans une colonne réelle. Inventer une colonne
//! `day_of_week`, ou la parquer dans un extrafield, aurait produit une donnée
//! que Dolibarr n'affiche jamais et que personne ne relirait.
//!
//! ## Fuseau horaire : les heures sont **locales**, jamais UTC
//!
//! C'est le piège de ce module, et un bugs qui ne se voit qu'en production.
//! La preuve est dans la source, pas dans une intuition :
//!
//! * `htdocs/core/db/DoliDB.class.php`, l. 153 : « Convert (by PHP) a GM
//!   Timestamp date into a string date **with PHP server TZ** to insert into a
//!   date field. » — c'est le contrat de `idate()`, dont le défaut est
//!   `$gm = 'tzserver'`.
//! * même fichier, l. 392 : « 19700101020000 -> 3600 with server TZ = +1 and
//!   `$gm='tzserver'` » — `jdate()`, le chemin inverse.
//! * Les deux fonctions portent en commentaire `// TODO $param should be gmt, so
//!   we should have default $gm to 'gmt' instead of default 'tzserver'` : les
//!   auteurs de Dolibarr eux-mêmes signalent que le défaut est l'heure locale.
//!
//! Conséquence : **on écrit `AAAA-MM-JJ HH:MM:00` tel quel, sans convertir.**
//! Une réservation de 14 h doit être stockée à 14 h. La convertir en UTC
//! donnerait 12 h en été, et le rendez-vous s'afficherait deux heures trop tôt.
//! Aucune fonction de ce module ne touche au fuseau : elle recopie l'heure
//! locale qu'on lui donne.
//!
//! ## Où va une réservation, et pourquoi
//!
//! Dans `llx_actioncomm`, pas dans `llx_bookcal_availabilities`. Preuve :
//! `htdocs/bookcal/booking_list.php`, l. 200-203, construit la liste des
//! réservations d'un calendrier par
//! `FROM llx_actioncomm as ac JOIN llx_actioncomm_resources as acr … WHERE
//! ac.fk_bookcal_calendar = <id> AND ac.code = 'AC_RDV'`.
//!
//! Une disponibilité est une **règle** d'ouverture ; une réservation est un
//! **événement** daté. Écrire une réservation dans la table des disponibilités
//! la transformerait en règle récurrente — exactement l'inverse du but.
//!
//! ## Ce que ce module ne fait pas
//!
//! * **Aucun paiement, aucun acompte.** Un rendez-vous BookCal n'est pas une
//!   facture ; il n'y a pas de montant ici, et le module ne touche ni
//!   `llx_facture` ni `llx_paiement`.
//! * **Aucun rappel automatique.** Pas d'e-mail, pas de SMS, pas de tâche
//!   planifiée : réserver ne notifie personne.
//! * **Aucun lien visio.** Pas d'URL de réunion, pas de champ `location`
//!   renseigné : le rendez-vous se tient là où les gens se trouvent.
//! * **Aucun lien avec les ressources humaines** (`llx_actioncomm_resources`) :
//!   Dolibarr sait y attacher des participants, ce module se contente de la
//!   table des événements.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Calendrier au brouillon. Constante `STATUS_DRAFT` de
/// `bookcal/class/calendar.class.php`, l. 62.
pub const CALENDRIER_BROUILLON: i32 = 0;
/// Calendrier ouvert aux réservations. `STATUS_VALIDATED`, l. 63.
pub const CALENDRIER_VALIDE: i32 = 1;
/// Calendrier fermé. `STATUS_CANCELED`, l. 64.
pub const CALENDRIER_FERME: i32 = 9;

/// Plage au brouillon. `STATUS_DRAFT` de
/// `bookcal/class/availabilities.class.php`, l. 62.
pub const PLAGE_BROUILLON: i32 = 0;
/// Plage active. `STATUS_VALIDATED`, l. 63.
pub const PLAGE_VALIDEE: i32 = 1;
/// Plage annulée. `STATUS_CANCELED`, l. 64.
pub const PLAGE_ANNULEE: i32 = 9;

/// Code d'événement d'un rendez-vous BookCal. Relevé dans
/// `htdocs/bookcal/booking_list.php`, l. 203.
pub const CODE_RENDEZ_VOUS: &str = "AC_RDV";

/// Nombre de minutes dans une journée. Sert à découper les plages et à
/// rogner les événements qui débordent du jour demandé.
const MINUTES_PAR_JOUR: i64 = 1440;

/// Un calendrier tel qu'il est stocké.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calendrier {
    pub rowid: i64,
    pub reference: String,
    pub libelle: String,
    pub description: Option<String>,
    pub fk_soc: Option<i64>,
    pub statut: i32,
}

/// Une plage d'ouverture telle qu'elle est stockée.
///
/// `jour_debut` / `jour_fin` sont des dates `AAAA-MM-JJ` : la plage est ouverte
/// de `heure_debut` à `heure_fin` pour **chaque** jour de cet intervalle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plage {
    pub rowid: i64,
    pub jour_debut: String,
    pub jour_fin: String,
    pub heure_debut: String,
    pub heure_fin: String,
}

/// Une réservation confirmée, lue dans l'agenda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    pub rowid: i64,
    pub debut: String,
    pub fin: Option<String>,
    pub libelle: String,
    pub fk_soc: Option<i64>,
    pub note: Option<String>,
}

/// Ce qu'il faut pour ouvrir un calendrier.
#[derive(Debug, Clone)]
pub struct NouveauCalendrier<'a> {
    pub label: &'a str,
    pub description: Option<&'a str>,
    pub fk_soc: Option<i64>,
    pub fk_user_creat: i64,
    pub note_public: Option<&'a str>,
}

/// Ce qu'il faut pour déclarer une plage d'ouverture.
///
/// `fk_user_creat` n'était pas dans la spécification, mais la colonne
/// `llx_bookcal_availabilities.fk_user_creat` est `NOT NULL` **sans valeur par
/// défaut** : sans elle, l'INSERT est refusé en mode strict. On la demande donc
/// plutôt que de laisser la base échouer.
#[derive(Debug, Clone)]
pub struct NouvellePlage<'a> {
    pub fk_bookcal_calendar: i64,
    pub jour_debut: &'a str,
    pub jour_fin: &'a str,
    pub heure_debut: &'a str,
    pub heure_fin: &'a str,
    pub fk_user_creat: i64,
}

/// Une plage ramenée en minutes depuis minuit, pour le calcul pur.
///
/// C'est la seule forme sur laquelle raisonne le découpage : plus de dates, plus
/// de chaînes, des entiers. Un test peut alors vérifier la mécanique sans base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlageHoraire {
    /// Première minute ouverte (inclus).
    pub debut: i64,
    /// Minute de fermeture (exclue) : une plage 9 h → 12 h vaut `540 → 720`.
    pub fin: i64,
}

impl PlageHoraire {
    pub fn nouvelle(debut: i64, fin: i64) -> Self {
        Self { debut, fin }
    }
}

// ---------------------------------------------------------------------------
// Outils purs : validation, conversion, découpage. Aucune base ici.
// ---------------------------------------------------------------------------

/// Construit une erreur métier lisible, du même type que le reste du crate.
fn refus(message: impl Into<String>) -> Box<dyn Error> {
    let texte: String = message.into();
    Box::<dyn Error>::from(texte)
}

/// Vrai si l'année est bissextile (règle grégorienne).
fn bissextile(annee: i64) -> bool {
    (annee % 4 == 0 && annee % 100 != 0) || annee % 400 == 0
}

/// Nombre de jours d'un mois, ou `None` si le mois n'existe pas.
fn jours_du_mois(annee: i64, mois: i64) -> Option<i64> {
    let jours = match mois {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if bissextile(annee) {
                29
            } else {
                28
            }
        }
        _ => return None,
    };
    Some(jours)
}

/// Vérifie qu'une chaîne est une date `AAAA-MM-JJ` réellement existante.
///
/// On refuse le 31 février : une date impossible passerait le format mais
/// ferait écrire dans `llx_actioncomm` une valeur que MariaDB ramènerait à
/// zéro, et le rendez-vous disparaîtrait de l'agenda sans message.
pub fn date_valide(jour: &str) -> Result<(), Box<dyn Error>> {
    let octets = jour.as_bytes();
    if octets.len() != 10 || octets[4] != b'-' || octets[7] != b'-' {
        return Err(refus(format!(
            "date « {jour} » invalide : format attendu AAAA-MM-JJ"
        )));
    }
    if !octets
        .iter()
        .enumerate()
        .all(|(i, o)| i == 4 || i == 7 || o.is_ascii_digit())
    {
        return Err(refus(format!(
            "date « {jour} » invalide : chiffres attendus hors séparateurs"
        )));
    }
    let annee: i64 = jour[0..4].parse()?;
    let mois: i64 = jour[5..7].parse()?;
    let quantieme: i64 = jour[8..10].parse()?;

    if !(1900..=2999).contains(&annee) {
        return Err(refus(format!(
            "date « {jour} » invalide : année hors bornes"
        )));
    }
    let Some(max) = jours_du_mois(annee, mois) else {
        return Err(refus(format!("date « {jour} » invalide : mois {mois}")));
    };
    if !(1..=max).contains(&quantieme) {
        return Err(refus(format!(
            "date « {jour} » invalide : le mois {mois} de {annee} compte {max} jours"
        )));
    }
    Ok(())
}

/// Convertit `HH:MM` en minutes depuis minuit.
///
/// Refuse tout ce qui n'est pas exactement deux chiffres, deux-points, deux
/// chiffres. `9:00`, `09:0`, `09h00` et `25:00` sont rejetés : une heure
/// approximative finirait par être stockée de travers.
pub fn heure_en_minutes(heure: &str) -> Result<i64, Box<dyn Error>> {
    let octets = heure.as_bytes();
    if octets.len() != 5 || octets[2] != b':' {
        return Err(refus(format!(
            "heure « {heure} » invalide : format attendu HH:MM"
        )));
    }
    if !octets
        .iter()
        .enumerate()
        .all(|(i, o)| i == 2 || o.is_ascii_digit())
    {
        return Err(refus(format!(
            "heure « {heure} » invalide : chiffres attendus hors le séparateur"
        )));
    }
    let h: i64 = heure[0..2].parse()?;
    let m: i64 = heure[3..5].parse()?;

    if !(0..=23).contains(&h) {
        return Err(refus(format!(
            "heure « {heure} » invalide : heures 00 à 23"
        )));
    }
    if !(0..=59).contains(&m) {
        return Err(refus(format!(
            "heure « {heure} » invalide : minutes 00 à 59"
        )));
    }
    Ok(h * 60 + m)
}

/// Rend une heure `HH:MM` à partir de minutes depuis minuit.
pub fn minutes_en_heure(minutes: i64) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// Vrai si deux intervalles `[debut, fin)` se recouvrent.
///
/// C'est **la** question du module : deux réservations peuvent se chevaucher
/// sans commencer à la même heure (9 h-10 h 30 et 10 h-11 h). On compare donc
/// des intervalles semi-ouverts : ils se recouvrent si chacun commence avant
/// que l'autre ne finisse. Deux créneaux jointifs (9 h-10 h puis 10 h-11 h) ne
/// se recouvrent pas, ce qui est le comportement voulu d'un planning.
pub fn se_recouvrent(a_debut: i64, a_fin: i64, b_debut: i64, b_fin: i64) -> bool {
    a_debut < b_fin && b_debut < a_fin
}

/// Vrai si le créneau `[debut, fin)` ne touche aucune occupation.
pub fn creneau_libre(debut: i64, fin: i64, occupees: &[PlageHoraire]) -> bool {
    !occupees
        .iter()
        .any(|o| se_recouvrent(debut, fin, o.debut, o.fin))
}

/// Découpe des plages d'ouverture en heures de début possibles.
///
/// Fonction **pure** : c'est elle qui porte la règle, et un test peut la
/// vérifier sans base de données ni serveur.
///
/// Le découpage part de `plage.debut` et avance de `duree_minutes` tant que le
/// créneau tient **entièrement** dans la plage. Un créneau qui déborderait de
/// la fermeture est abandonné : mieux vaut un créneau de moins qu'un
/// rendez-vous qui finit après la fermeture.
///
/// Les résultats de plusieurs plages sont fusionnés, triés et dédoublonnés,
/// parce que deux plages qui se touchent (9 h-12 h et 12 h-14 h) produisent la
/// même grille qu'une plage unique.
pub fn decouper_plages(
    plages: &[PlageHoraire],
    duree_minutes: i64,
) -> Result<Vec<String>, Box<dyn Error>> {
    if duree_minutes <= 0 {
        return Err(refus(format!(
            "durée {duree_minutes} min refusée : elle doit être strictement positive"
        )));
    }

    let mut debuts: Vec<i64> = Vec::new();
    for plage in plages {
        if plage.fin <= plage.debut {
            // Plage vide ou inversée : aucune heure de début possible.
            continue;
        }
        let mut debut = plage.debut;
        while debut + duree_minutes <= plage.fin {
            debuts.push(debut);
            debut += duree_minutes;
        }
    }

    debuts.sort_unstable();
    debuts.dedup();
    Ok(debuts.into_iter().map(minutes_en_heure).collect())
}

/// Ramène un horodatage `AAAA-MM-JJ HH:MM:SS` à des minutes dans `jour`.
///
/// Un événement qui commence la veille vaut 0 (il occupe depuis le début de la
/// journée), un événement qui finit le lendemain vaut 1440. Sans ce rognage, un
/// rendez-vous à cheval sur minuit ferait déborder la comparaison.
fn minutes_du_jour(quand: &str, jour: &str) -> i64 {
    // `get` plutôt qu'un découpage direct : une valeur inattendue rend 0 au lieu
    // de faire paniquer le serveur sur une frontière de caractère.
    let (Some(date), Some(reste)) = (quand.get(0..10), quand.get(10..)) else {
        return 0;
    };
    // Les dates sont en `AAAA-MM-JJ` : l'ordre lexicographique est l'ordre
    // chronologique, la comparaison de chaînes suffit.
    if date < jour {
        return 0;
    }
    if date > jour {
        return MINUTES_PAR_JOUR;
    }
    // La colonne porte `HH:MM:SS` ; `heure_en_minutes` attend `HH:MM`.
    let heure = reste.trim_start().get(0..5).unwrap_or("");
    heure_en_minutes(heure).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Calendriers et plages : lecture et écriture.
// ---------------------------------------------------------------------------

/// Crée un calendrier et rend son identifiant.
///
/// Trois choses méritent d'être dites, parce qu'elles ne se devinent pas :
///
/// * **La référence est fabriquée ici.** `llx_bookcal_calendar.ref` est
///   `NOT NULL` sans valeur par défaut, et
///   `Calendar::getNextNumRef()` (l. 949 de `calendar.class.php`) ne contient
///   que `// Not used` puis `return '';`. Dolibarr ne sait donc pas numéroter un
///   calendrier. On insère un jeton temporaire, puis on le remplace par
///   `BCAL-<rowid>` : la référence devient stable et unique par construction.
/// * **`type` vaut 0 (client) ou 3 (autre).** Le vocabulaire relevé dans
///   `calendar.class.php`, l. 116 : `0 => Customer`, `1 => Supplier`,
///   `3 => Other`. Un calendrier rattaché à un tiers prend 0, sinon 3.
/// * **`visibility` est le propriétaire, pas un niveau de visibilité.** Le champ
///   est déclaré `integer:User:user/class/user.class.php` avec le libellé
///   « Owner » (l. 115). On y met le créateur : c'est la personne qui gère
///   l'agenda.
pub fn creer_calendrier(
    pool: &Pool,
    entity: i64,
    nouveau: &NouveauCalendrier<'_>,
) -> Result<i64, Box<dyn Error>> {
    let libelle = nouveau.label.trim();
    if libelle.is_empty() {
        return Err(refus("libellé vide : un calendrier doit être nommé"));
    }
    if nouveau.fk_user_creat <= 0 {
        return Err(refus(
            "créateur absent : fk_user_creat est NOT NULL sans défaut",
        ));
    }

    // `type` : 0 = client (un tiers est rattaché), 3 = autre.
    let typecal: i32 = if nouveau.fk_soc.is_some() { 0 } else { 3 };

    let mut conn = pool.get_conn()?;
    // La clé « type » est interdite dans nos `params!` : le nom du placeholder
    // est libre, la colonne ne l'est pas. On écrit donc `:typecal` pour la
    // colonne `type`.
    conn.exec_drop(
        "INSERT INTO llx_bookcal_calendar \
           (entity, ref, label, description, note_public, date_creation, \
            fk_user_creat, status, type, visibility) \
         VALUES \
           (:entity, :ref, :label, :description, :note_public, NOW(), \
            :fk_user_creat, :statut, :typecal, :proprietaire)",
        params! {
            "entity" => entity,
            "ref" => "BCAL-PROV",
            "label" => libelle,
            "description" => nouveau.description,
            "note_public" => nouveau.note_public,
            "fk_user_creat" => nouveau.fk_user_creat,
            "statut" => CALENDRIER_VALIDE,
            "typecal" => typecal,
            "proprietaire" => nouveau.fk_user_creat,
        },
    )?;
    let rowid = conn.last_insert_id() as i64;

    conn.exec_drop(
        "UPDATE llx_bookcal_calendar SET ref = CONCAT('BCAL-', rowid) WHERE rowid = :rowid",
        params! { "rowid" => rowid },
    )?;

    Ok(rowid)
}

/// Crée une plage d'ouverture et rend son identifiant.
///
/// Refuse : un calendrier inexistant ou fermé, une date mal formée ou inversée,
/// une heure hors `HH:MM`, des minutes non nulles, et une fin qui ne suit pas
/// le début.
///
/// **Pourquoi refuser les minutes non nulles ?** `startHour` et `endHour` sont
/// des `int` : `availabilities_card.php`, l. 153, appelle
/// `dol_mktime($starthour, 0, 0, …)`, les minutes sont forcées à zéro. Une plage
/// 9 h 30 serait donc silencieusement ramenée à 9 h. On refuse plutôt que de
/// stocker autre chose que ce qui a été demandé.
///
/// `duration` n'est pas écrite : la colonne a un défaut de 30 et notre API
/// reçoit la durée au moment de calculer les créneaux, pas à la déclaration.
pub fn creer_plage(
    pool: &Pool,
    entity: i64,
    plage: &NouvellePlage<'_>,
) -> Result<i64, Box<dyn Error>> {
    date_valide(plage.jour_debut)?;
    date_valide(plage.jour_fin)?;
    if plage.jour_fin < plage.jour_debut {
        return Err(refus(format!(
            "plage inversée : la fin ({}) précède le début ({})",
            plage.jour_fin, plage.jour_debut
        )));
    }

    let debut = heure_en_minutes(plage.heure_debut)?;
    let fin = heure_en_minutes(plage.heure_fin)?;
    if debut % 60 != 0 || fin % 60 != 0 {
        return Err(refus(
            "minutes non nulles refusées : startHour et endHour sont des heures entières",
        ));
    }
    if fin <= debut {
        return Err(refus(format!(
            "plage vide : {} ne suit pas {}",
            plage.heure_fin, plage.heure_debut
        )));
    }
    if plage.fk_user_creat <= 0 {
        return Err(refus(
            "créateur absent : fk_user_creat est NOT NULL sans défaut",
        ));
    }

    let mut conn = pool.get_conn()?;
    let statut: Option<i32> = conn.exec_first(
        "SELECT status FROM llx_bookcal_calendar WHERE rowid = :rowid AND entity = :entity",
        params! { "rowid" => plage.fk_bookcal_calendar, "entity" => entity },
    )?;
    match statut {
        None => {
            return Err(refus(format!(
                "calendrier {} inexistant dans l'entité {entity}",
                plage.fk_bookcal_calendar
            )))
        }
        Some(CALENDRIER_FERME) => {
            return Err(refus(format!(
                "calendrier {} fermé : on n'y ajoute pas de plage",
                plage.fk_bookcal_calendar
            )))
        }
        Some(_) => {}
    }

    let libelle = format!(
        "{} → {} · {}-{}",
        plage.jour_debut, plage.jour_fin, plage.heure_debut, plage.heure_fin
    );

    conn.exec_drop(
        "INSERT INTO llx_bookcal_availabilities \
           (fk_bookcal_calendar, label, start, end, startHour, endHour, \
            status, date_creation, fk_user_creat) \
         VALUES \
           (:fk_calendrier, :label, :debut, :fin, :heure_debut, :heure_fin, \
            :statut, NOW(), :fk_user_creat)",
        params! {
            "fk_calendrier" => plage.fk_bookcal_calendar,
            "label" => libelle,
            "debut" => plage.jour_debut,
            "fin" => plage.jour_fin,
            "heure_debut" => debut / 60,
            "heure_fin" => fin / 60,
            "statut" => PLAGE_VALIDEE,
            "fk_user_creat" => plage.fk_user_creat,
        },
    )?;

    Ok(conn.last_insert_id() as i64)
}

/// Les calendriers d'une entité, du plus récent au plus ancien.
pub fn calendriers(
    pool: &Pool,
    entity: i64,
    limite: u32,
) -> Result<Vec<Calendrier>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT rowid, ref, label, description, fk_soc, status \
         FROM llx_bookcal_calendar WHERE entity = :entity \
         ORDER BY rowid DESC LIMIT :limite",
        params! { "entity" => entity, "limite" => limite.clamp(1, 500) },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| Calendrier {
            rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
            reference: r
                .get_opt::<String, _>(1)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            libelle: r
                .get_opt::<String, _>(2)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            description: r.get_opt::<String, _>(3).and_then(|v| v.ok()),
            fk_soc: r.get_opt::<i64, _>(4).and_then(|v| v.ok()),
            statut: r.get_opt::<i32, _>(5).and_then(|v| v.ok()).unwrap_or(0),
        })
        .collect())
}

/// Les plages d'un calendrier, dans l'ordre chronologique.
///
/// `llx_bookcal_availabilities` **n'a pas de colonne `entity`** : le
/// cloisonnement passe obligatoirement par la jointure sur le calendrier. On
/// joint donc explicitement, sans quoi une plage d'une autre entité
/// apparaîtrait.
pub fn plages(pool: &Pool, entity: i64, fk_calendrier: i64) -> Result<Vec<Plage>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT a.rowid, DATE_FORMAT(a.start, '%Y-%m-%d'), \
                DATE_FORMAT(a.end, '%Y-%m-%d'), a.startHour, a.endHour \
         FROM llx_bookcal_availabilities a \
         JOIN llx_bookcal_calendar c ON c.rowid = a.fk_bookcal_calendar \
         WHERE a.fk_bookcal_calendar = :fk_calendrier AND c.entity = :entity \
         ORDER BY a.start, a.startHour",
        params! { "fk_calendrier" => fk_calendrier, "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let debut: i64 = r.get_opt::<i64, _>(3).and_then(|v| v.ok()).unwrap_or(0);
            let fin: i64 = r.get_opt::<i64, _>(4).and_then(|v| v.ok()).unwrap_or(0);
            Plage {
                rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
                jour_debut: r
                    .get_opt::<String, _>(1)
                    .and_then(|v| v.ok())
                    .unwrap_or_default(),
                jour_fin: r
                    .get_opt::<String, _>(2)
                    .and_then(|v| v.ok())
                    .unwrap_or_default(),
                heure_debut: minutes_en_heure(debut * 60),
                heure_fin: minutes_en_heure(fin * 60),
            }
        })
        .collect())
}

/// Les plages ouvertes qui couvrent `jour`, en minutes.
///
/// Ne retient que les plages `STATUS_VALIDATED` : une plage au brouillon n'est
/// pas encore ouverte, une plage annulée ne l'est plus. C'est la traduction
/// exacte des libellés relevés dans la classe (`Draft` / `Enabled` / `Disabled`).
fn plages_ouvertes_du_jour<Q: mysql::prelude::Queryable>(
    conn: &mut Q,
    entity: i64,
    fk_calendrier: i64,
    jour: &str,
) -> Result<Vec<PlageHoraire>, Box<dyn Error>> {
    let rows: Vec<(i64, i64)> = conn.exec(
        "SELECT a.startHour, a.endHour \
         FROM llx_bookcal_availabilities a \
         JOIN llx_bookcal_calendar c ON c.rowid = a.fk_bookcal_calendar \
         WHERE a.fk_bookcal_calendar = :fk_calendrier AND c.entity = :entity \
           AND a.status = :statut AND a.start <= :jour AND a.end >= :jour2 \
         ORDER BY a.startHour",
        params! {
            "fk_calendrier" => fk_calendrier,
            "entity" => entity,
            "statut" => PLAGE_VALIDEE,
            "jour" => jour,
            "jour2" => jour,
        },
    )?;

    Ok(rows
        .into_iter()
        .map(|(h_debut, h_fin)| PlageHoraire::nouvelle(h_debut * 60, h_fin * 60))
        .collect())
}

/// Les occupations du jour, ramenées en minutes.
fn occupations_du_jour<Q: mysql::prelude::Queryable>(
    conn: &mut Q,
    entity: i64,
    fk_calendrier: i64,
    jour: &str,
) -> Result<Vec<PlageHoraire>, Box<dyn Error>> {
    // On prend tout ce qui peut mordre sur la journée, y compris un événement
    // commencé la veille : le rognage se fait ensuite dans `minutes_du_jour`.
    let debut_jour = format!("{jour} 00:00:00");
    let fin_jour = format!("{jour} 23:59:59");
    let rows: Vec<(String, Option<String>)> = conn.exec(
        "SELECT DATE_FORMAT(datep, '%Y-%m-%d %H:%i:%s'), \
                DATE_FORMAT(datep2, '%Y-%m-%d %H:%i:%s') FROM llx_actioncomm \
         WHERE fk_bookcal_calendar = :fk_calendrier AND entity = :entity \
           AND code = :code AND datep < :fin_jour \
           AND COALESCE(datep2, datep) > :debut_jour \
         ORDER BY datep",
        params! {
            "fk_calendrier" => fk_calendrier,
            "entity" => entity,
            "code" => CODE_RENDEZ_VOUS,
            "fin_jour" => fin_jour,
            "debut_jour" => debut_jour,
        },
    )?;

    Ok(rows
        .into_iter()
        .map(|(debut, fin)| {
            let d = minutes_du_jour(&debut, jour);
            // Un événement sans fin est traité comme ponctuel : il ne bloque
            // rien, faute de savoir jusqu'à quand il dure.
            let f = fin
                .as_deref()
                .map(|f| minutes_du_jour(f, jour))
                .unwrap_or(d);
            PlageHoraire::nouvelle(d, f)
        })
        .collect())
}

/// Les heures de début possibles pour un jour donné.
///
/// C'est la lecture des plages, puis l'appel à [`decouper_plages`] : la règle de
/// découpage reste pure et testable, la base ne fournit que les bornes.
///
/// Une journée sans plage ouverte rend une liste vide, pas une erreur : un
/// calendrier fermé le dimanche n'est pas une panne.
pub fn creneaux_libres(
    pool: &Pool,
    entity: i64,
    fk_calendrier: i64,
    jour: &str,
    duree_minutes: i64,
) -> Result<Vec<String>, Box<dyn Error>> {
    date_valide(jour)?;
    if duree_minutes <= 0 {
        return Err(refus(format!(
            "durée {duree_minutes} min refusée : elle doit être strictement positive"
        )));
    }

    let mut conn = pool.get_conn()?;
    let statut: Option<i32> = conn.exec_first(
        "SELECT status FROM llx_bookcal_calendar WHERE rowid = :rowid AND entity = :entity",
        params! { "rowid" => fk_calendrier, "entity" => entity },
    )?;
    match statut {
        None => {
            return Err(refus(format!(
                "calendrier {fk_calendrier} inexistant dans l'entité {entity}"
            )))
        }
        // Un calendrier fermé n'ouvre aucun créneau, mais ce n'est pas une
        // erreur : on répond simplement qu'il n'y a rien.
        Some(CALENDRIER_FERME) => return Ok(Vec::new()),
        Some(_) => {}
    }

    let plages = plages_ouvertes_du_jour(&mut conn, entity, fk_calendrier, jour)?;
    let debuts = decouper_plages(&plages, duree_minutes)?;
    if debuts.is_empty() {
        return Ok(Vec::new());
    }

    let occupees = occupations_du_jour(&mut conn, entity, fk_calendrier, jour)?;

    let mut libres = Vec::with_capacity(debuts.len());
    for debut in debuts {
        // `decouper_plages` ne rend que des `HH:MM` valides : l'échec est
        // impossible, mais on propage plutôt que de masquer.
        let minutes = heure_en_minutes(&debut)?;
        if creneau_libre(minutes, minutes + duree_minutes, &occupees) {
            libres.push(debut);
        }
    }
    Ok(libres)
}

/// Réserve un créneau et rend l'identifiant de l'événement créé.
///
/// La réservation va dans `llx_actioncomm` (voir l'en-tête du module) avec
/// `code = 'AC_RDV'` et `fk_bookcal_calendar`.
///
/// # Le chevauchement
///
/// Deux réservations se recouvrent dès que leurs intervalles se croisent, même
/// si elles ne commencent pas à la même heure. La comparaison se fait sur des
/// intervalles semi-ouverts (`debut < autre_fin && autre_debut < fin`) : un
/// créneau 9 h-10 h 30 et un créneau 10 h-11 h se chevauchent, un créneau
/// 9 h-10 h et un créneau 10 h-11 h ne se chevauchent pas.
///
/// La lecture des occupations et l'écriture se font dans **une seule
/// transaction**, avec `SELECT … FOR UPDATE`. Sans cela, deux appels simultanés
/// liraient tous les deux « libre » puis écriraient tous les deux : le verrou
/// de plage d'InnoDB ferme cette fenêtre. Ce n'est pas une garantie absolue si
/// la base tourne en `READ COMMITTED`, mais cela supprime le cas courant.
#[allow(clippy::too_many_arguments)]
pub fn reserver(
    pool: &Pool,
    entity: i64,
    fk_calendrier: i64,
    jour: &str,
    heure: &str,
    duree_minutes: i64,
    fk_soc: Option<i64>,
    note: &str,
) -> Result<i64, Box<dyn Error>> {
    // La signature à huit paramètres est imposée par le contrat du module ;
    // l'emballer dans une structure ne ferait que déplacer la verbosité.
    date_valide(jour)?;
    if duree_minutes <= 0 {
        return Err(refus(format!(
            "durée {duree_minutes} min refusée : elle doit être strictement positive"
        )));
    }
    let debut = heure_en_minutes(heure)?;
    let fin = debut + duree_minutes;
    if fin > MINUTES_PAR_JOUR {
        return Err(refus(format!(
            "créneau {heure} + {duree_minutes} min déborde de la journée"
        )));
    }

    let mut conn = pool.get_conn()?;
    let statut: Option<i32> = conn.exec_first(
        "SELECT status FROM llx_bookcal_calendar WHERE rowid = :rowid AND entity = :entity",
        params! { "rowid" => fk_calendrier, "entity" => entity },
    )?;
    match statut {
        None => {
            return Err(refus(format!(
                "calendrier {fk_calendrier} inexistant dans l'entité {entity}"
            )))
        }
        Some(CALENDRIER_FERME) => {
            return Err(refus(format!(
                "calendrier {fk_calendrier} fermé : aucune réservation possible"
            )))
        }
        Some(_) => {}
    }

    // Le créneau doit tenir entièrement dans une plage ouverte.
    let plages = plages_ouvertes_du_jour(&mut conn, entity, fk_calendrier, jour)?;
    let dedans = plages.iter().any(|p| debut >= p.debut && fin <= p.fin);
    if !dedans {
        return Err(refus(format!(
            "créneau {heure} + {duree_minutes} min hors des plages ouvertes du {jour}"
        )));
    }

    let datep = format!("{jour} {heure}:00");
    let datep2 = format!("{jour} {}:00", minutes_en_heure(fin));
    let libelle = format!("Rendez-vous {jour} {heure}");
    let note_sql: Option<&str> = if note.trim().is_empty() {
        None
    } else {
        Some(note)
    };
    // `durationp` est marqué `deprecated` dans `actioncomm.class.php` mais il est
    // renseigné par Dolibarr avec `datef - datep`, soit des **secondes**.
    let durationp = (duree_minutes * 60) as f64;

    conn.query_drop("START TRANSACTION")?;

    let ecriture = (|| -> Result<i64, Box<dyn Error>> {
        let occupees: Vec<(String, Option<String>)> = conn.exec(
            "SELECT DATE_FORMAT(datep, '%Y-%m-%d %H:%i:%s'), \
                DATE_FORMAT(datep2, '%Y-%m-%d %H:%i:%s') FROM llx_actioncomm \
             WHERE fk_bookcal_calendar = :fk_calendrier AND entity = :entity \
               AND code = :code AND COALESCE(datep2, datep) > :debut \
               AND datep < :fin \
             FOR UPDATE",
            params! {
                "fk_calendrier" => fk_calendrier,
                "entity" => entity,
                "code" => CODE_RENDEZ_VOUS,
                "debut" => datep.clone(),
                "fin" => datep2.clone(),
            },
        )?;
        if !occupees.is_empty() {
            return Err(refus(format!(
                "créneau {heure} déjà pris le {jour} : {} réservation(s) se recouvrent",
                occupees.len()
            )));
        }

        conn.exec_drop(
            "INSERT INTO llx_actioncomm \
               (entity, ref, code, datep, datep2, fk_soc, label, note, datec, \
                durationp, fk_bookcal_calendar, status) \
             VALUES \
               (:entity, '(PROV)', :code, :datep, :datep2, :fk_soc, :label, :note, \
                NOW(), :durationp, :fk_calendrier, 0)",
            params! {
                "entity" => entity,
                "code" => CODE_RENDEZ_VOUS,
                "datep" => datep,
                "datep2" => datep2,
                "fk_soc" => fk_soc,
                "label" => libelle,
                "note" => note_sql,
                "durationp" => durationp,
                "fk_calendrier" => fk_calendrier,
            },
        )?;
        let rowid = conn.last_insert_id() as i64;

        // Convention Dolibarr : la référence définitive est `(AC<id>)`, posée
        // après coup puisqu'elle dépend de l'identifiant.
        conn.exec_drop(
            "UPDATE llx_actioncomm SET ref = CONCAT('(AC', id, ')') WHERE id = :id",
            params! { "id" => rowid },
        )?;

        Ok(rowid)
    })();

    match ecriture {
        Ok(rowid) => {
            conn.query_drop("COMMIT")?;
            Ok(rowid)
        }
        Err(e) => {
            // On ferme la transaction nous-mêmes : laisser une transaction
            // ouverte dans le pool empoisonnerait la connexion suivante.
            let _ = conn.query_drop("ROLLBACK");
            Err(e)
        }
    }
}

/// Les réservations d'un calendrier pour un jour donné.
pub fn reservations(
    pool: &Pool,
    entity: i64,
    fk_calendrier: i64,
    jour: &str,
) -> Result<Vec<Reservation>, Box<dyn Error>> {
    date_valide(jour)?;

    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT id, DATE_FORMAT(datep, '%Y-%m-%d %H:%i:%s'), \
                DATE_FORMAT(datep2, '%Y-%m-%d %H:%i:%s'), label, fk_soc, note \
         FROM llx_actioncomm \
         WHERE fk_bookcal_calendar = :fk_calendrier AND entity = :entity \
           AND code = :code AND datep >= :debut AND datep <= :fin \
         ORDER BY datep, id",
        params! {
            "fk_calendrier" => fk_calendrier,
            "entity" => entity,
            "code" => CODE_RENDEZ_VOUS,
            "debut" => format!("{jour} 00:00:00"),
            "fin" => format!("{jour} 23:59:59"),
        },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| Reservation {
            rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
            debut: r
                .get_opt::<String, _>(1)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            fin: r.get_opt::<String, _>(2).and_then(|v| v.ok()),
            libelle: r
                .get_opt::<String, _>(3)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            fk_soc: r.get_opt::<i64, _>(4).and_then(|v| v.ok()),
            note: r.get_opt::<String, _>(5).and_then(|v| v.ok()),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_heure_valide_donne_ses_minutes() {
        assert_eq!(heure_en_minutes("00:00").unwrap(), 0);
        assert_eq!(heure_en_minutes("09:30").unwrap(), 570);
        assert_eq!(heure_en_minutes("23:59").unwrap(), 1439);
        assert_eq!(minutes_en_heure(570), "09:30");
    }

    #[test]
    fn une_heure_mal_formee_est_refusee() {
        for mauvais in ["9:00", "09:0", "09h00", "24:00", "09:60", "", "09:00:00"] {
            assert!(
                heure_en_minutes(mauvais).is_err(),
                "« {mauvais} » aurait dû être refusé"
            );
        }
    }

    #[test]
    fn une_date_impossible_est_refusee() {
        assert!(date_valide("2031-03-10").is_ok());
        assert!(date_valide("2032-02-29").is_ok(), "2032 est bissextile");
        assert!(date_valide("2031-02-29").is_err(), "2031 ne l'est pas");
        assert!(date_valide("2031-13-01").is_err());
        assert!(date_valide("2031-04-31").is_err());
        assert!(date_valide("10/03/2031").is_err());
    }

    #[test]
    fn deux_creneaux_jointifs_ne_se_recouvrent_pas() {
        assert!(!se_recouvrent(540, 600, 600, 660));
        assert!(!se_recouvrent(600, 660, 540, 600));
    }

    #[test]
    fn deux_creneaux_qui_se_croisent_se_recouvrent() {
        // 9 h-10 h 30 et 10 h-11 h : le second commence avant la fin du premier.
        assert!(se_recouvrent(540, 630, 600, 660));
        assert!(se_recouvrent(600, 660, 540, 630));
        // Un créneau inclus dans un autre.
        assert!(se_recouvrent(540, 720, 600, 660));
    }
}
