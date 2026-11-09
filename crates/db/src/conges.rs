//! Congés et absences (`llx_holiday`, `llx_c_holiday_types`).
//!
//! Une demande de congé, c'est une période déclarée par un salarié, un type
//! pris dans le dictionnaire `llx_c_holiday_types` (congés payés, RTT, maladie,
//! sans solde…) et un circuit à trois signatures : la demande (statut 1), la
//! validation par le responsable (3), l'approbation RH (5). Un refus (4) ou une
//! annulation sortent la demande du circuit sans jamais l'effacer : une absence
//! est une pièce de paie, elle doit rester lisible des années après.
//!
//! # Les statuts
//!
//! Dolibarr numérote ses statuts dans `Holiday::$statuts` :
//!
//! | valeur | sens |
//! |---|---|
//! | 1 | demandé, en attente de validation |
//! | 2 | validé — **ancien** code, conservé pour les bases reprises |
//! | 3 | validé par le responsable |
//! | 4 | refusé (ou annulé : Dolibarr n'a qu'un seul code « hors circuit ») |
//! | 5 | approuvé par les RH : le congé est acquis |
//!
//! Le statut 2 n'est jamais écrit par ce module : il n'existe que pour relire
//! d'anciennes lignes sans les considérer comme des brouillons.
//!
//! # Le décompte des jours
//!
//! Dolibarr stocke `nb_open_day` (un `double(24,8)`) sans le recalculer à la
//! lecture : c'est cette valeur qui fait foi sur un bulletin de paie. Ce module
//! la calcule donc **une seule fois**, à la création, et l'écrit. Voir
//! [`jours_ouvres`] pour ce qui est compté — et surtout pour ce qui ne l'est
//! pas : les jours fériés ne sont pas déduits, le chiffre rendu est une **borne
//! haute**.
//!
//! # La référence
//!
//! `llx_holiday.ref` est un `varchar(30)` NOT NULL, sans index unique. La
//! numérotation passe par [`divers::suivant_ref`] (`CP0001`, `CP0002`…) : faute
//! d'index unique, deux créations simultanées peuvent tomber sur le même numero
//! et la base ne s'en plaindra pas. C'est un défaut connu de Dolibarr, pas de
//! ce noyau ; mieux vaut le dire que d'ajouter une boucle de reprise qui ne
//! servirait à rien (rien ne refuserait la seconde insertion).

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row};
use rust_decimal::Decimal;

use crate::divers;

/// Préfixe de référence des demandes de congé.
pub const PREFIX: &str = "CP";

/// Demandé : la demande existe, personne ne l'a encore regardée.
pub const STATUT_DEMANDE: i16 = 1;
/// Validé par le responsable — **ancien code**, jamais écrit ici.
pub const STATUT_VALIDE_ANCIEN: i16 = 2;
/// Validé par le responsable : prêt pour l'approbation RH.
pub const STATUT_VALIDE: i16 = 3;
/// Refusé, ou annulé : hors circuit. `date_refuse` et `date_cancel` distinguent
/// les deux cas, Dolibarr n'ayant qu'un code pour l'un comme pour l'autre.
pub const STATUT_REFUSE: i16 = 4;
/// Approuvé par les RH : les jours sont décomptés du solde.
pub const STATUT_APPROUVE: i16 = 5;

/// Taille de `llx_holiday.description`, en octets.
const DESCRIPTION_MAX: usize = 255;
/// Taille de `llx_holiday.detail_refuse`, en octets.
const MOTIF_MAX: usize = 250;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Conge {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    /// Le salarié concerné (`llx_holiday.fk_user`).
    pub fk_user: i64,
    /// Login de ce salarié (`llx_user.login`).
    pub demandeur: Option<String>,
    pub fk_type: i64,
    /// Libellé du type (`llx_c_holiday_types.label`).
    pub type_conge: Option<String>,
    pub date_create: Option<String>,
    pub description: String,
    pub date_debut: String,
    pub date_fin: String,
    /// La demande porte-t-elle sur des demi-journées ?
    pub halfday: bool,
    /// Jours ouvrés décomptés, tels que stockés en base.
    pub nb_open_day: Decimal,
    pub statut: i16,
    /// Le valideur désigné, puis celui qui a réellement validé.
    pub fk_validator: i64,
    pub date_valid: Option<String>,
    pub fk_user_valid: Option<i64>,
    pub date_approval: Option<String>,
    pub fk_user_approve: Option<i64>,
    pub date_refuse: Option<String>,
    pub fk_user_refuse: Option<i64>,
    pub date_cancel: Option<String>,
    pub fk_user_cancel: Option<i64>,
    /// Motif du refus, borné à 250 octets.
    pub detail_refuse: Option<String>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
}

/// Ce qu'il faut pour créer une demande.
///
/// `fk_user` est le salarié qui prend le congé, `fk_validator` le responsable
/// désigné. Les deux sont distincts : dans Dolibarr le demandeur choisit son
/// valideur à la saisie, et le circuit échoue si la colonne `fk_validator`
/// (NOT NULL) est laissée à zéro.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewConge {
    pub fk_user: i64,
    pub fk_type: i64,
    pub description: String,
    pub date_debut: String,
    pub date_fin: String,
    pub halfday: bool,
    pub fk_validator: i64,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
}

/// Ce qui empêche de compter les jours ouvrés d'une période.
///
/// Ce type existe pour que [`jours_ouvres`] reste **pure** : elle ne touche ni
/// la base ni l'horloge, et rend une erreur qui s'explique toute seule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Erreur {
    /// Le texte reçu n'est pas une date `AAAA-MM-JJ` qui existe au calendrier.
    DateInvalide(String),
    /// La période se contredit.
    PeriodeInversee { debut: String, fin: String },
}

impl fmt::Display for Erreur {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erreur::DateInvalide(texte) => write!(
                f,
                "date invalide : {texte} — une date de congé s'écrit AAAA-MM-JJ"
            ),
            Erreur::PeriodeInversee { debut, fin } => write!(
                f,
                "la date de fin ({fin}) ne peut pas précéder la date de début ({debut})"
            ),
        }
    }
}

impl Error for Erreur {}

/// Une date vérifiée : son rang (jours écoulés depuis le 1er janvier 1970) et
/// son année.
///
/// Deux composantes seulement : le rang mesure les écarts et porte le jour de
/// la semaine, l'année sert à ranger un congé dans l'exercice du solde. Le mois
/// et le jour ont déjà fait leur travail au moment de la vérification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Date {
    rang: i64,
    annee: i64,
}

/// Décalage de l'époque Unix dans le calendrier de Howard Hinnant.
const EPOQUE: i64 = 719_468;

/// Jours écoulés depuis le 1er janvier 1970 (`days_from_civil` de Hinnant).
///
/// La crate `recurrence` garde la même conversion, mais en privé : elle rend des
/// échéances, pas des numéros de jour, et son calendrier n'est pas exposé. Ce
/// module a besoin du jour de la semaine pour distinguer un lundi d'un samedi ;
/// il refait donc la seule conversion, dix lignes sans table ni cas particulier,
/// plutôt que d'ouvrir la crate pour cela.
fn jours_depuis_epoque(annee: i64, mois: u32, jour: u32) -> i64 {
    // L'année est alignée sur mars : le 29 février tombe alors en fin d'année et
    // ne perturbe plus les divisions.
    let annee = if mois <= 2 { annee - 1 } else { annee };
    let ere = if annee >= 0 { annee } else { annee - 399 } / 400;
    let annee_ere = annee - ere * 400;
    let mois_mars = (i64::from(mois) + 9) % 12;
    let jour_annee = (153 * mois_mars + 2) / 5 + i64::from(jour) - 1;
    let jour_ere = annee_ere * 365 + annee_ere / 4 - annee_ere / 100 + jour_annee;
    ere * 146_097 + jour_ere - EPOQUE
}

/// Nombre de jours d'un mois, par différence avec le premier jour du mois
/// suivant : aucune table, donc aucun oubli d'année bissextile.
fn duree_du_mois(annee: i64, mois: u32) -> u32 {
    let (annee_suivante, mois_suivant) = if mois == 12 {
        (annee + 1, 1)
    } else {
        (annee, mois + 1)
    };
    let debut = jours_depuis_epoque(annee, mois, 1);
    let suite = jours_depuis_epoque(annee_suivante, mois_suivant, 1);
    (suite - debut) as u32
}

/// Lit une date `AAAA-MM-JJ` et vérifie qu'elle existe réellement.
///
/// `2026-02-30` et `2026-1-1` sont refusés : une date qui n'existe pas ne doit
/// pas entrer dans un décompte de paie. Le format est exigé tel quel, c'est
/// celui que Dolibarr stocke (ordre alphabétique = ordre chronologique).
fn analyser_date(valeur: &str) -> Result<Date, Erreur> {
    let texte = valeur.trim();
    let octets = texte.as_bytes();
    let forme = octets.len() == 10
        && octets[4] == b'-'
        && octets[7] == b'-'
        && octets[..4].iter().all(u8::is_ascii_digit)
        && octets[5..7].iter().all(u8::is_ascii_digit)
        && octets[8..].iter().all(u8::is_ascii_digit);
    if !forme {
        return Err(Erreur::DateInvalide(valeur.to_string()));
    }
    let annee = i64::from(nombre(&octets[..4]));
    let mois = nombre(&octets[5..7]);
    let jour = nombre(&octets[8..]);
    if !(1..=9999).contains(&annee) || !(1..=12).contains(&mois) {
        return Err(Erreur::DateInvalide(valeur.to_string()));
    }
    if jour == 0 || jour > duree_du_mois(annee, mois) {
        return Err(Erreur::DateInvalide(valeur.to_string()));
    }
    Ok(Date {
        rang: jours_depuis_epoque(annee, mois, jour),
        annee,
    })
}

/// Lit un nombre sur des octets déjà vérifiés chiffres.
fn nombre(octets: &[u8]) -> u32 {
    octets
        .iter()
        .fold(0u32, |valeur, &octet| valeur * 10 + u32::from(octet - b'0'))
}

/// Jours ouvrés entre deux dates, bornes comprises. **Fonction pure** : ni base
/// de données, ni horloge, donc vérifiable sans rien monter.
///
/// Sont comptés les lundis, mardis, mercredis, jeudis et vendredis ; les samedis
/// et dimanches sont exclus. Les deux bornes comptent quand elles tombent un
/// jour ouvré, une période d'un seul jour valant 1.
///
/// `demi` à `true` divise le total par deux : la demande est saisie en
/// demi-journées, et le module retient la moitié du décompte (2,5 pour une
/// semaine pleine de cinq jours). C'est une convention **du module**, pas un
/// calcul de paie : Dolibarr, lui, retire une demi-journée à chaque extrémité
/// de la période quand `halfday` vaut 1, ce qui donnerait 4 sur la même semaine.
/// Le chiffre écrit ici est celui que le module assume, et il est stocké tel
/// quel dans `nb_open_day`.
///
/// # Ce qui n'est **pas** fait
///
/// Les **jours fériés ne sont pas déduits**. Dolibarr 24 en garde pourtant un
/// dictionnaire, `llx_c_hrm_public_holiday` (`code`, `dayrule`, `day`, `month`,
/// `year`, `fk_country`, `fk_departement`) : il n'est **pas lu ici**. Deux
/// raisons, assumées :
///
/// 1. cette fonction est **pure** — ni base, ni horloge —, c'est ce qui la rend
///    vérifiable sans rien monter ; la lire dans une table la rendrait
///    dépendante d'un jeu de données local ;
/// 2. ce dictionnaire est indexé par pays **et par département** : le 15 août
///    n'est pas férié partout de la même façon, et un décompte faux-vrai serait
///    plus dangereux qu'un décompte majoré qui se voit.
///
/// Le chiffre rendu est donc une **borne haute** : un congé du 24 décembre au
/// 1er janvier annonce plus de jours ouvrés qu'il n'en consomme réellement.
/// C'est un choix, pas un oubli : brancher ce dictionnaire (en soustrayant les
/// fériés qui tombent un jour ouvré) est un travail séparé, à faire sur une
/// version non pure de la fonction.
///
/// # Erreurs
///
/// [`Erreur::DateInvalide`] si une borne n'est pas une date `AAAA-MM-JJ`
/// existante, [`Erreur::PeriodeInversee`] si la fin précède le début.
pub fn jours_ouvres(debut: &str, fin: &str, demi: bool) -> Result<Decimal, Erreur> {
    let debut_lu = analyser_date(debut)?;
    let fin_lu = analyser_date(fin)?;
    if fin_lu.rang < debut_lu.rang {
        return Err(Erreur::PeriodeInversee {
            debut: debut.trim().to_string(),
            fin: fin.trim().to_string(),
        });
    }

    // Le 1er janvier 1970 était un jeudi : avec lundi = 0, le jeudi vaut 3.
    let mut jour_semaine = (debut_lu.rang + 3).rem_euclid(7);
    let mut jours: i64 = 0;
    let mut courant = debut_lu.rang;
    while courant <= fin_lu.rang {
        if jour_semaine < 5 {
            jours += 1;
        }
        courant += 1;
        jour_semaine = (jour_semaine + 1) % 7;
    }

    let total = Decimal::from(jours);
    Ok(if demi {
        total / Decimal::from(2)
    } else {
        total
    })
}

/// Colonnes lues. Le nom de la date de creation depend de la version de
/// Dolibarr (`date_create` en 24, `date_creation` en 25) : il est demande a la
/// base plutot qu'ecrit en dur.
fn select_cols(date_create: &str) -> String {
    format!(
        "h.rowid, h.entity, h.ref, h.fk_user, u.login, h.fk_type, t.label, \
     DATE_FORMAT(h.{date_create}, '%Y-%m-%d %H:%i:%s'), h.description, \
     DATE_FORMAT(h.date_debut, '%Y-%m-%d'), DATE_FORMAT(h.date_fin, '%Y-%m-%d'), \
     h.halfday, CAST(h.nb_open_day AS CHAR), h.statut, h.fk_validator, \
     DATE_FORMAT(h.date_valid, '%Y-%m-%d %H:%i:%s'), h.fk_user_valid, \
     DATE_FORMAT(h.date_approval, '%Y-%m-%d %H:%i:%s'), h.fk_user_approve, \
     DATE_FORMAT(h.date_refuse, '%Y-%m-%d %H:%i:%s'), h.fk_user_refuse, \
     DATE_FORMAT(h.date_cancel, '%Y-%m-%d %H:%i:%s'), h.fk_user_cancel, \
     h.detail_refuse, h.note_private, h.note_public"
    )
}

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

/// Une colonne `double(24,8)` ne se relit pas en entier : elle est ramenée en
/// texte par `CAST`, puis lue en `Decimal`. Jamais de `f64` : un solde se
/// compare au centième près, et 2,5 jours de congé ne valent pas 2,4999.
fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: &Row) -> Conge {
    Conge {
        rowid: opt_i(row, 0).unwrap_or(0),
        entity: opt_i(row, 1).unwrap_or(1),
        ref_: opt_s(row, 2).unwrap_or_default(),
        fk_user: opt_i(row, 3).unwrap_or(0),
        demandeur: opt_s(row, 4),
        fk_type: opt_i(row, 5).unwrap_or(0),
        type_conge: opt_s(row, 6),
        date_create: opt_s(row, 7),
        description: opt_s(row, 8).unwrap_or_default(),
        date_debut: opt_s(row, 9).unwrap_or_default(),
        date_fin: opt_s(row, 10).unwrap_or_default(),
        halfday: opt_i(row, 11).unwrap_or(0) != 0,
        nb_open_day: opt_dec(row, 12).unwrap_or_default(),
        statut: opt_i(row, 13).unwrap_or(0) as i16,
        fk_validator: opt_i(row, 14).unwrap_or(0),
        date_valid: opt_s(row, 15),
        fk_user_valid: opt_i(row, 16),
        date_approval: opt_s(row, 17),
        fk_user_approve: opt_i(row, 18),
        date_refuse: opt_s(row, 19),
        fk_user_refuse: opt_i(row, 20),
        date_cancel: opt_s(row, 21),
        fk_user_cancel: opt_i(row, 22),
        detail_refuse: opt_s(row, 23),
        note_private: opt_s(row, 24),
        note_public: opt_s(row, 25),
    }
}

/// Demandes de congé d'une entité, de la plus récente à la plus ancienne.
///
/// Le tri porte sur `date_debut` et non sur `date_create` : c'est le congé
/// lui-même qu'on cherche dans la liste (celui de la semaine prochaine), pas le
/// moment où la saisie a été faite.
///
/// Rend un `Result` comme toutes les listes du noyau, bien que la signature
/// attendue fût `Vec<Conge>` : une panne MySQL doit remonter, pas se traduire
/// par une liste vide qu'on croirait complète.
/// Les types de conge proposes a la saisie.
///
/// Seuls les types actifs sortent : un type desactive reste dans le
/// dictionnaire pour ne pas casser les demandes deja posees, mais il ne doit
/// plus apparaitre dans un formulaire. Les lignes d'entite 0 sont les
/// dictionnaires partages, comme partout chez Dolibarr.
pub fn types(pool: &Pool, entity: i64) -> Result<Vec<(i64, String)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(i64, String)> = conn.exec(
        "SELECT rowid, label FROM llx_c_holiday_types \
         WHERE entity IN (0, :entity) AND active = 1 \
         ORDER BY sortorder ASC, rowid ASC",
        params! { "entity" => entity },
    )?;
    Ok(rows)
}

pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Conge>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {} FROM llx_holiday h \
         LEFT JOIN llx_user u ON u.rowid = h.fk_user \
         LEFT JOIN llx_c_holiday_types t ON t.rowid = h.fk_type \
         WHERE h.entity = :entity \
         ORDER BY h.date_debut DESC, h.rowid DESC LIMIT :limite OFFSET :offset",
        select_cols(&crate::colonnes::date_creation(pool, "llx_holiday")?)
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limite" => limite, "offset" => offset },
    )?;
    Ok(rows.iter().map(map_row).collect())
}

/// Une demande précise, ou `None` si elle n'existe pas dans l'entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Conge>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {} FROM llx_holiday h \
         LEFT JOIN llx_user u ON u.rowid = h.fk_user \
         LEFT JOIN llx_c_holiday_types t ON t.rowid = h.fk_type \
         WHERE h.entity = :entity AND h.rowid = :rowid",
        select_cols(&crate::colonnes::date_creation(pool, "llx_holiday")?)
    );
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.as_ref().map(map_row))
}

/// Les références existent-elles vraiment, dans cette entité ?
///
/// Ouvert sur sa propre connexion, refermée aussitôt : la création a besoin
/// d'une seconde connexion pour la numérotation, et garder les deux ouvertes
/// épuise le pool quand plusieurs demandes se créent de front.
fn verifier_references(pool: &Pool, entity: i64, new: &NewConge) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;

    // `llx_holiday` n'a aucune clé étrangère : un salarié fantôme passerait sans
    // bruit et le congé n'apparaîtrait dans l'écran de personne.
    for (id, role) in [(new.fk_user, "salarié"), (new.fk_validator, "valideur")] {
        let existe: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_user WHERE entity = :entity AND rowid = :rowid",
            params! { "entity" => entity, "rowid" => id },
        )?;
        if existe.is_none() {
            return Err(
                format!("l'utilisateur {id} ({role}) n'existe pas dans l'entité {entity}").into(),
            );
        }
    }

    // Un type désactivé reste en base (les congés passés doivent continuer de
    // s'afficher) mais ne doit plus servir à en créer un : Dolibarr ne saurait
    // ni le libeller ni l'imputer sur un compteur.
    //
    // `entity IN (0, :entity)` : comme les autres dictionnaires Dolibarr, une
    // ligne d'entité 0 est partagée par toutes les entités. Refuser un type
    // partagé ferait échouer une saisie parfaitement légitime.
    let actif: Option<Option<i64>> = conn.exec_first(
        "SELECT active FROM llx_c_holiday_types WHERE rowid = :id AND entity IN (0, :entity)",
        params! { "id" => new.fk_type, "entity" => entity },
    )?;
    let Some(actif) = actif else {
        return Err(format!(
            "le type de congé {} n'existe pas dans l'entité {entity}",
            new.fk_type
        )
        .into());
    };
    // `active` est nullable avec 1 pour défaut : un NULL vaut actif.
    if actif.unwrap_or(1) == 0 {
        return Err(format!("le type de congé {} est désactivé", new.fk_type).into());
    }
    Ok(())
}

/// Crée une demande de congé au statut « demandé » et renvoie son `rowid`.
///
/// Le nombre de jours ouvrés est calculé ici, une fois, et écrit dans
/// `nb_open_day` : c'est cette valeur que la paie relira. Une période sans jour
/// ouvré (un samedi et un dimanche) est acceptée et vaut zéro — Dolibarr le
/// permet, et refuser la saisie ferait perdre la trace d'une demande déposée.
///
/// `fk_user_create` reçoit le salarié lui-même : la structure ne porte pas
/// d'auteur distinct, et dans l'immense majorité des cas le salarié saisit sa
/// propre demande. Un congé posé par un tiers pourra être distingué plus tard en
/// ajoutant ce champ à [`NewConge`], pas en devinant ici.
///
/// # Erreurs
///
/// Utilisateur ou valideur inexistant, type inconnu ou désactivé, description
/// vide ou trop longue, date mal formée ou période inversée, et toute erreur
/// MySQL remontée telle quelle.
pub fn create(pool: &Pool, entity: i64, new: &NewConge) -> Result<i64, Box<dyn Error>> {
    if new.fk_user <= 0 {
        return Err("le salarié (fk_user) est obligatoire".into());
    }
    if new.fk_type <= 0 {
        return Err("le type de congé (fk_type) est obligatoire".into());
    }
    // `fk_validator` est NOT NULL sans défaut : un zéro y écrirait un mensonge,
    // et le circuit n'aurait personne à qui présenter la demande.
    if new.fk_validator <= 0 {
        return Err("le valideur (fk_validator) est obligatoire".into());
    }
    let description = new.description.trim();
    if description.is_empty() {
        return Err("la description du congé est obligatoire".into());
    }
    // Refus plutôt que troncature : une description amputée en silence
    // laisserait croire que le salarié n'a rien écrit de plus.
    if description.len() > DESCRIPTION_MAX {
        return Err(format!(
            "la description du congé dépasse {DESCRIPTION_MAX} octets ({})",
            description.len()
        )
        .into());
    }

    let nb_open_day = jours_ouvres(&new.date_debut, &new.date_fin, new.halfday)?;
    verifier_references(pool, entity, new)?;

    let reference = divers::suivant_ref(pool, "llx_holiday", "ref", PREFIX, entity)?;
    let demi: i32 = if new.halfday { 1 } else { 0 };

    let mut conn = pool.get_conn()?;
    let date_create = crate::colonnes::date_creation(pool, "llx_holiday")?;
    conn.exec_drop(
        format!(
            "INSERT INTO llx_holiday \
         (entity, ref, fk_user, fk_user_create, fk_type, {date_create}, description, \
          date_debut, date_fin, halfday, nb_open_day, statut, fk_validator, \
          note_private, note_public) \
         VALUES (:entity, :ref, :utilisateur, :createur, :type_conge, NOW(), \
          :description, :debut, :fin, :demi, :jours, :statut, :valideur, \
          :note_private, :note_public)"
        ),
        params! {
            "entity" => entity,
            "ref" => reference.as_str(),
            "utilisateur" => new.fk_user,
            "createur" => new.fk_user,
            "type_conge" => new.fk_type,
            "description" => description,
            "debut" => new.date_debut.trim(),
            "fin" => new.date_fin.trim(),
            "demi" => demi,
            // `nb_open_day` est un `double(24,8)` : on passe le texte du
            // décimal, jamais un flottant Rust, pour que 2,5 reste 2,5.
            "jours" => nb_open_day.to_string(),
            "statut" => STATUT_DEMANDE,
            "valideur" => new.fk_validator,
            "note_private" => new.note_private.as_deref(),
            "note_public" => new.note_public.as_deref(),
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Statut courant d'une demande, vue par l'entité qui la demande.
fn statut_actuel(
    conn: &mut mysql::PooledConn,
    entity: i64,
    rowid: i64,
) -> Result<i16, Box<dyn Error>> {
    let statut: Option<i64> = conn.exec_first(
        "SELECT statut FROM llx_holiday WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        Some(s) => Ok(s as i16),
        None => Err(format!("demande de congé {rowid} introuvable dans l'entité {entity}").into()),
    }
}

/// Valide une demande : demandé (1) → validé (3).
///
/// `fk_validator` est réécrit avec le valideur réel : la colonne porte d'abord
/// le responsable *désigné* à la saisie, puis celui qui a *effectivement*
/// signé. C'est la même colonne chez Dolibarr, et c'est elle qui fait foi en cas
/// de contestation du circuit.
pub fn valider(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    fk_validator: i64,
) -> Result<(), Box<dyn Error>> {
    if fk_validator <= 0 {
        return Err("le valideur (fk_validator) est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    match statut {
        STATUT_VALIDE | STATUT_VALIDE_ANCIEN => {
            return Err("cette demande de congé est déjà validée".into())
        }
        STATUT_APPROUVE => {
            return Err("cette demande est déjà approuvée : plus rien à valider".into())
        }
        STATUT_REFUSE => {
            return Err(
                "cette demande est refusée ou annulée : elle ne peut plus être validée".into(),
            )
        }
        STATUT_DEMANDE => {}
        autre => return Err(format!("statut inattendu pour une validation : {autre}").into()),
    }
    conn.exec_drop(
        "UPDATE llx_holiday SET statut = :nouveau, date_valid = NOW(), \
         fk_user_valid = :valideur, fk_validator = :valideur \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "nouveau" => STATUT_VALIDE,
            "valideur" => fk_validator,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Approuve une demande validée : validé (3) → approuvé (5).
///
/// L'approbation est la seule transition qui engage l'employeur : c'est elle qui
/// fait entrer les jours dans le solde ([`solde`]). Une demande encore au statut
/// 1 n'a pas de `date_valid` ; l'approuver court-circuiterait la validation et
/// le refus est donc explicite.
pub fn approuver(pool: &Pool, entity: i64, rowid: i64, fk_user: i64) -> Result<(), Box<dyn Error>> {
    if fk_user <= 0 {
        return Err("l'approbateur (fk_user) est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    match statut {
        STATUT_APPROUVE => return Err("cette demande de congé est déjà approuvée".into()),
        STATUT_DEMANDE => {
            return Err(format!(
                "seule une demande validée peut être approuvée (statut actuel : {statut})"
            )
            .into())
        }
        // Ancien code de validation : Dolibarr ne le produit plus, mais une base
        // reprise en contient. On ne l'approuve pas d'office — le circuit actuel
        // veut une validation fraîche, avec sa date.
        STATUT_VALIDE_ANCIEN => {
            return Err(
                "cette demande porte l'ancien code de validation (2) : elle doit être revalidée"
                    .into(),
            )
        }
        STATUT_REFUSE => {
            return Err("une demande refusée ou annulée ne peut pas être approuvée".into())
        }
        STATUT_VALIDE => {}
        autre => return Err(format!("statut inattendu pour une approbation : {autre}").into()),
    }
    conn.exec_drop(
        "UPDATE llx_holiday SET statut = :nouveau, date_approval = NOW(), \
         fk_user_approve = :approbateur \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "nouveau" => STATUT_APPROUVE,
            "approbateur" => fk_user,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Refuse une demande : demandé (1) ou validé (3) → refusé (4), avec son motif.
///
/// Un motif vide est écrit `NULL` plutôt qu'en chaîne vide : Dolibarr teste
/// `empty()`, mais un export ne lit pas de la même façon « pas de motif » et
/// « motif vide ».
pub fn refuser(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    fk_user: i64,
    motif: &str,
) -> Result<(), Box<dyn Error>> {
    if fk_user <= 0 {
        return Err("l'auteur du refus (fk_user) est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    match statut {
        STATUT_REFUSE => return Err("cette demande de congé est déjà refusée ou annulée".into()),
        STATUT_APPROUVE => {
            return Err("une demande approuvée ne peut être refusée : il faut l'annuler".into())
        }
        STATUT_DEMANDE | STATUT_VALIDE | STATUT_VALIDE_ANCIEN => {}
        autre => return Err(format!("statut inattendu pour un refus : {autre}").into()),
    }
    let motif = motif.trim();
    let motif = if motif.is_empty() {
        None
    } else {
        Some(tronquer(motif, MOTIF_MAX))
    };
    conn.exec_drop(
        "UPDATE llx_holiday SET statut = :nouveau, date_refuse = NOW(), \
         fk_user_refuse = :auteur, detail_refuse = :motif \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "nouveau" => STATUT_REFUSE,
            "auteur" => fk_user,
            "motif" => motif,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Annule une demande : elle sort du circuit **sans être supprimée**.
///
/// Une absence annulée reste une pièce : elle explique pourquoi une période a
/// été bloquée, et c'est la seule trace qu'un contrôle de paie pourra relire.
/// On pose donc `date_cancel` et on passe au statut **4**, le seul code « hors
/// circuit » que Dolibarr connaisse — il sert au refus comme à l'annulation, et
/// c'est `date_cancel` (opposée à `date_refuse`) qui distingue les deux cas.
/// Créer un statut 6 serait un mensonge vis-à-vis des écrans Dolibarr, qui ne le
/// connaissent pas et l'afficheraient vide.
///
/// L'annulation est permise depuis 1, 3 et 5 : un congé déjà approuvé doit
/// pouvoir être annulé (départ reporté, chantier décalé), et le passer en 4 le
/// retire du solde, ce qui est le but. `fk_user_cancel` reste vide : la
/// signature ne se pose pas faute de savoir qui annule — mieux vaut une colonne
/// honnêtement vide qu'un nom faux dans une pièce de paie.
pub fn annuler(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    match statut {
        STATUT_REFUSE => return Err("cette demande de congé est déjà refusée ou annulée".into()),
        STATUT_DEMANDE | STATUT_VALIDE | STATUT_VALIDE_ANCIEN | STATUT_APPROUVE => {}
        autre => return Err(format!("statut inattendu pour une annulation : {autre}").into()),
    }
    conn.exec_drop(
        "UPDATE llx_holiday SET statut = :nouveau, date_cancel = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "nouveau" => STATUT_REFUSE,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Jours de congé approuvés d'un salarié pour l'année de `date_reference`.
///
/// Seul le statut 5 compte : une demande déposée ou validée n'a pas encore
/// consommé de solde, et une demande refusée ou annulée n'en consomme plus.
///
/// `date_reference` (`AAAA-MM-JJ`) existe pour rendre la fonction testable et
/// pour permettre de consulter un exercice passé ; c'est son **année** qui
/// filtre, jamais le jour. Le rattachement se fait sur `date_debut` : un congé
/// du 28 décembre au 3 janvier appartient à l'année où il commence, comme dans
/// l'écran Dolibarr.
///
/// Le total est la somme des `nb_open_day` **stockés**, pas un recalcul : c'est
/// le chiffre que la paie a lu, et le recalculer ici ferait diverger le solde
/// d'un bulletin déjà édité si la règle de décompte changeait un jour.
///
/// # Erreurs
///
/// Date de référence mal formée, et toute erreur MySQL remontée telle quelle.
pub fn solde(
    pool: &Pool,
    entity: i64,
    fk_user: i64,
    date_reference: &str,
) -> Result<Decimal, Box<dyn Error>> {
    let reference = analyser_date(date_reference)?;
    let mut conn = pool.get_conn()?;
    let total: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(h.nb_open_day), 0) AS CHAR) FROM llx_holiday h \
         WHERE h.entity = :entity AND h.fk_user = :utilisateur AND h.statut = :statut \
           AND YEAR(h.date_debut) = :annee",
        params! {
            "entity" => entity,
            "utilisateur" => fk_user,
            "statut" => STATUT_APPROUVE,
            "annee" => reference.annee,
        },
    )?;
    Ok(total
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .unwrap_or_default())
}

/// Borne un texte à `max` octets sans couper un caractère.
///
/// `detail_refuse` est un `varchar(250)` : un accent saisi en UTF-8 occupe deux
/// octets, et une coupe au milieu laisserait un octet invalide que MySQL
/// refuserait d'écrire.
fn tronquer(texte: &str, max: usize) -> &str {
    if texte.len() <= max {
        return texte;
    }
    let mut fin = max;
    while fin > 0 && !texte.is_char_boundary(fin) {
        fin -= 1;
    }
    &texte[..fin]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2 novembre 2026 est un lundi : la semaine de référence des tests.
    #[test]
    fn le_calendrier_place_les_jours_au_bon_endroit() {
        // Lundi = 0. Le 1er janvier 1970 était un jeudi, donc 3.
        assert_eq!(jours_depuis_epoque(1970, 1, 1), 0);
        assert_eq!((jours_depuis_epoque(1970, 1, 1) + 3).rem_euclid(7), 3);
        assert_eq!((jours_depuis_epoque(2026, 11, 2) + 3).rem_euclid(7), 0);
        // 1er mars 2026 suit le 28 février : un jour d'écart, pas trois.
        assert_eq!(
            jours_depuis_epoque(2026, 3, 1) - jours_depuis_epoque(2026, 2, 28),
            1
        );
        assert_eq!(duree_du_mois(2024, 2), 29);
        assert_eq!(duree_du_mois(2025, 2), 28);
        assert_eq!(duree_du_mois(2026, 4), 30);
        assert_eq!(duree_du_mois(2026, 12), 31);
    }

    #[test]
    fn une_date_qui_n_existe_pas_est_refusee() {
        assert!(analyser_date("2026-11-02").is_ok());
        assert_eq!(
            analyser_date("2026-02-30"),
            Err(Erreur::DateInvalide("2026-02-30".to_string()))
        );
        assert!(analyser_date("2026-13-01").is_err());
        assert!(analyser_date("2026-11-3").is_err());
        assert!(analyser_date("02/11/2026").is_err());
        assert!(analyser_date("").is_err());
        // Le 29 février n'existe qu'une année sur quatre.
        assert!(analyser_date("2024-02-29").is_ok());
        assert!(analyser_date("2025-02-29").is_err());
    }

    #[test]
    fn une_periode_inversee_s_explique_en_francais() {
        let erreur = jours_ouvres("2026-11-06", "2026-11-02", false).expect_err("période inversée");
        assert_eq!(
            erreur.to_string(),
            "la date de fin (2026-11-02) ne peut pas précéder la date de début (2026-11-06)"
        );
        assert_eq!(
            Erreur::DateInvalide("2026-11-3".to_string()).to_string(),
            "date invalide : 2026-11-3 — une date de congé s'écrit AAAA-MM-JJ"
        );
    }

    #[test]
    fn le_motif_est_borne_sans_couper_un_caractere() {
        // 400 octets (200 « é ») doivent tenir dans 250 octets : 125 « é ».
        let motif = "é".repeat(200);
        let garde = tronquer(&motif, MOTIF_MAX);
        assert_eq!(garde.len(), 250);
        assert_eq!(garde.chars().count(), 125);
        assert!(motif.starts_with(garde));
        // Un texte déjà court est rendu tel quel.
        assert_eq!(tronquer("hors sujet", MOTIF_MAX), "hors sujet");
    }
}
