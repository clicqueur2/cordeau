//! Organisation d'événements : les **inscriptions** à une conférence ou à un stand.
//!
//! Le module Dolibarr s'appelle `eventorganization` (« Organisation
//! d'événements »). Malgré ce nom, il ne tient qu'**une** table métier, et cette
//! table porte les inscriptions. Mesuré sur la base de référence
//! (`information_schema.tables`, schéma `dolibarr`) :
//!
//! ```sql
//! SELECT table_name FROM information_schema.tables
//!  WHERE table_schema = 'dolibarr' AND table_name LIKE '%eventorganization%';
//! -- llx_eventorganization_conferenceorboothattendee
//! -- llx_eventorganization_conferenceorboothattendee_extrafields
//! ```
//!
//! **Il n'existe aucune table d'événement propre au module.** L'événement est
//! ailleurs : c'est un **`llx_actioncomm`**, c'est-à-dire une entrée d'agenda.
//!
//! # L'événement est un `llx_actioncomm` : ce que ça change
//!
//! La preuve est dans la source, pas dans une supposition
//! (`htdocs/eventorganization/class/conferenceorbooth.class.php`) :
//!
//! ```php
//! class ConferenceOrBooth extends ActionComm          // ligne 34
//!   public $table_element = 'actioncomm';             // ligne 49
//! ```
//!
//! Le module « habille » donc une ligne d'agenda : la conférence ou le stand
//! **est** un `llx_actioncomm`, et l'inscription le vise par sa colonne
//! `fk_actioncomm` (`int(11)`, nullable en base — voir plus bas pourquoi on la
//! refuse vide).
//!
//! Conséquence directe, et c'est la raison pour laquelle une inscription sans
//! événement n'a aucun sens : **la date et le lieu ne sont pas sur
//! l'inscription**. Ils sont sur l'agenda — `llx_actioncomm.datep`, `datef`,
//! `location` (relevés dans `information_schema.columns`) — et le libellé sur
//! `label`. Une inscription orpheline serait une ligne avec un email et rien
//! d'autre : ni quand, ni où. On la refuse plutôt que de créer un fantôme.
//!
//! Deuxième conséquence : **l'événement a son propre statut**, distinct de celui
//! de l'inscription. `conferenceorbooth.class.php` :
//!
//! ```text
//! const STATUS_DRAFT         = 0;   // brouillon
//! const STATUS_SUGGESTED     = 1;   // suggéré
//! const STATUS_CONFIRMED     = 2;   // confirmé
//! const STATUS_NOT_QUALIFIED = 3;   // non qualifié
//! const STATUS_DONE          = 4;   // fait
//! const STATUS_CANCELED      = 9;   // annulé
//! ```
//!
//! Ce module-ci ne les écrit pas : il écrit les statuts d'**inscription**.
//! Confondre les deux serait une faute de sens (une inscription « confirmée »
//! n'est pas un événement « confirmé »).
//!
//! # Les colonnes réellement présentes
//!
//! Relevées par `information_schema.columns` sur
//! `llx_eventorganization_conferenceorboothattendee` :
//!
//! | colonne | type | null | rôle |
//! |---|---|---|---|
//! | `rowid` | `int(11)` auto_increment | non | identifiant |
//! | `ref` | `varchar(128)` | non | référence, **égale au rowid** (voir plus bas) |
//! | `fk_soc` | `int(11)` | oui | tiers inscrit |
//! | `fk_actioncomm` | `int(11)` | oui | **l'événement** (agenda) |
//! | `fk_project` | `int(11)` | **non** | le projet qui porte l'événement |
//! | `fk_invoice` | `int(11)` | oui | facture liée, faite ailleurs |
//! | `email` | `varchar(128)` | oui | email de l'inscrit |
//! | `email_company` | `varchar(128)` | oui | email de la société |
//! | `firstname` | `varchar(100)` | oui | prénom |
//! | `lastname` | `varchar(100)` | oui | nom |
//! | `date_subscription` | `datetime` | oui | date d'inscription |
//! | `amount` | `double` | oui | montant payé, **déclaratif** |
//! | `note_public` | `text` | oui | note visible |
//! | `note_private` | `text` | oui | note interne |
//! | `date_creation` | `datetime` | non | création de la ligne |
//! | `tms` | `timestamp` | oui | mise à jour automatique |
//! | `fk_user_creat` / `fk_user_modif` | `int(11)` | oui | auteurs |
//! | `last_main_doc` / `model_pdf` | `varchar(255)` | oui | PDF (hors périmètre) |
//! | `ip` / `import_key` | `varchar(250)` / `varchar(14)` | oui | import (hors périmètre) |
//! | `status` | `smallint(6)` | non | statut d'inscription |
//! | `fk_replacement` | `int(11)` | oui | remplacement d'un inscrit |
//!
//! Deux écarts entre la source PHP et la base méritent d'être dits, parce qu'ils
//! changent l'écriture :
//!
//! - `status` est `NOT NULL` **sans valeur par défaut** en base, alors que la
//!   classe déclare `'default' => '0'`. Un `INSERT` qui l'omet échoue : on
//!   l'écrit toujours explicitement (0, brouillon).
//! - `date_subscription` est **nullable en base** alors que la classe la déclare
//!   `'notnull' => 1`. On suit la classe : la date est toujours écrite, `NOW()`
//!   si l'appelant n'en fournit pas.
//!
//! # Les statuts d'inscription, recopiés de la source
//!
//! `conferenceorboothattendee.class.php`, lignes 56-59 :
//!
//! ```text
//! const STATUS_DRAFT     = 0;   // saisie
//! const STATUS_VALIDATED = 1;   // inscrit, confirmé
//! const STATUS_USED      = 5;   // « was present, presence confirmed, no more
//!                               //   entrances can be done using this ticket »
//! const STATUS_CANCELED  = 9;   // annulé
//! ```
//!
//! Le libellé officiel de chaque valeur est donné par le tableau
//! `'arrayofkeyval'` de la classe : « Draft », « Registered », « ShowedUp »,
//! « Canceled ».
//!
//! **Le statut 5 n'a aucun écrivain dans la source.** Il apparaît dans les
//! libellés et les couleurs (`LibStatut`, lignes 944-968), jamais dans un
//! `UPDATE`. C'est le commentaire de la constante qui fait foi : « présence
//! confirmée, plus aucune entrée avec ce billet ». [`marquer_present`] écrit donc
//! 5 directement, sans imiter une méthode qui n'existe pas — et seulement depuis
//! l'état confirmé (1), parce qu'on ne confirme pas la présence de quelqu'un qui
//! n'était pas inscrit.
//!
//! La séquence retenue, et les gardes de la source qui la fondent :
//!
//! ```text
//! 0 (brouillon) --confirmer--> 1 (confirmée)
//! 1 (confirmée) --annuler----> 9 (annulée)     cancel() : « if ($this->status !=
//!                                              self::STATUS_VALIDATED) return 0 »
//! 1 (confirmée) --présent----> 5 (présente)    commentaire de STATUS_USED
//! ```
//!
//! `validate()` refuse de revalider (`if ($this->status == self::STATUS_VALIDATED)
//! return 0;`, ligne 587) : reconfirmer est donc une erreur, pas un no-op
//! silencieux. `cancel()` refuse tout ce qui n'est pas 1 : **on n'annule pas un
//! brouillon** — il n'a jamais été confirmé, il se supprime, et l'annuler
//! inventerait une trace commerciale qui n'existe pas.
//!
//! La source connaît aussi `reopen()` (9 → 1). Ce module ne l'expose pas : il
//! rend les trois actes demandés, et rouvrir une inscription annulée demande une
//! décision explicite (et un nouvel email au client) qui n'a pas sa place dans
//! un portage de socle. La transition 9 → 1 est donc **refusée ici**, et c'est
//! dit.
//!
//! # Le cloisonnement : la table **n'a pas** de colonne `entity`
//!
//! C'est un point mesuré, pas une hypothèse : la colonne `entity` **est absente**
//! de `llx_eventorganization_conferenceorboothattendee` (`information_schema.columns`,
//! liste complète ci-dessus). Le module s'appuie donc sur autre chose, et la
//! source le dit explicitement :
//!
//! ```php
//! $this->ismultientitymanaged = 'fk_project@projet';   // ligne 240
//! ```
//!
//! Traduction : **le cloisonnement vient du projet** (`llx_projet.entity`), par
//! la colonne `fk_project` que l'inscription porte et qui est `NOT NULL`. Toutes
//! les lectures de ce module joignent donc `llx_projet` et filtrent sur
//! `p.entity = :entity` ; aucune ne filtre sur une colonne `entity` qui
//! n'existe pas.
//!
//! D'où une contrainte d'écriture que la signature demandée ne portait pas :
//! `fk_project` est obligatoire et la structure n'a pas de champ pour lui. On le
//! **déduit de l'événement** (`llx_actioncomm.fk_project`), ce qui est la seule
//! valeur juste : le projet qui porte l'événement est celui qui porte ses
//! inscriptions, et c'est lui qui décide de l'entité. Si l'événement n'est
//! rattaché à aucun projet, l'inscription est **refusée** : écrire
//! `fk_project = 0` la rendrait invisible à toutes les lectures cloisonnées —
//! elle existerait sans que personne puisse la voir. Un refus vaut mieux qu'une
//! ligne fantôme.
//!
//! Corollaire assumé : une inscription dont le projet a été supprimé reste en
//! base mais n'est plus lue. Le cloisonnement n'a pas d'autre source ; le dire
//! vaut mieux que de laisser croire à une lecture exhaustive.
//!
//! # Le doublon : même email, même événement
//!
//! `SHOW INDEX` donne un index **unique** qui répond exactement à la question :
//!
//! ```text
//! uk_eventorganization_conferenceorboothattendee : (fk_project, email, fk_actioncomm)
//! ```
//!
//! Les trois colonnes y sont, dans cet ordre. Comme `fk_project` est **déduit**
//! de `fk_actioncomm` (voir ci-dessus), contrôler `(email, fk_actioncomm)` est la
//! même règle, et c'est la bonne : c'est l'événement qui identifie l'inscription,
//! pas le projet, sinon changer l'événement de projet ferait apparaître des
//! doublons.
//!
//! L'email est en `utf8mb4_unicode_ci` (relevé dans `information_schema.columns`) :
//! la comparaison est **insensible à la casse**, et l'index unique aussi. Une
//! recherche `WHERE email = :email` a donc exactement la sémantique de l'index —
//! `jean@exemple.fr` et `JEAN@EXEMPLE.FR` sont le même inscrit. On ne normalise
//! pas l'email en minuscules à l'écriture (Dolibarr conserve la saisie), on
//! s'appuie sur la collation, comme la base.
//!
//! Si deux inscriptions concurrentes passaient entre la vérification et
//! l'insertion, c'est l'index unique qui tranche : l'erreur du pilote remonte
//! telle quelle. Le contrôle applicatif sert à donner un message lisible, pas à
//! remplacer la contrainte.
//!
//! # La référence : `ref` = `rowid`, faute de module de numérotation
//!
//! La source appelle `getNextNumRef()`, qui charge
//! `mod_conferenceorboothattendee_standard` dans `core/modules/eventorganization/`.
//! **Ce répertoire n'existe pas** dans la copie vendorisée (vérifié :
//! `core/modules/` n'a pas de dossier `eventorganization`). Le seul comportement
//! réellement observable est celui de `create()` puis `clone` : `ref` reçoit
//! `"(PROV)"`, puis, juste après l'insertion, `$object->ref = (string) $object->id`
//! et `update()`. La référence **est l'identifiant**, en texte.
//!
//! On reproduit cela, avec une précaution que PHP n'a pas : la référence
//! provisoire est **unique** (`(PROV-<nanosecondes>-<compteur>)`), parce que
//! `ref` porte un index unique et que deux insertions simultanées avec
//! `"(PROV)"` se heurteraient. La ligne est ensuite renommée en `rowid`, dans la
//! même transaction.
//!
//! Effet secondaire agréable : `validate()` ne renumérote pas nos références, son
//! test `preg_match('/^[\(]?PROV/i', $num)` ne les reconnaissant plus.
//!
//! # Ce que ce module ne fait pas, et pourquoi c'est mesuré
//!
//! - **Pas de billetterie.** Aucune table de billet : ni `llx_eventorganization_ticket`,
//!   ni rien d'approchant (`information_schema.tables`, requête ci-dessus). Le mot
//!   « ticket » n'apparaît que dans un commentaire de liste (« copied almost
//!   verbertum from htdocs/ticket/list.php ») et dans le commentaire de
//!   `STATUS_USED`. Le module `ticket` de Dolibarr est le **support client**, pas
//!   la billetterie : on ne le détourne pas.
//! - **Pas de paiement.** Il existe bien une colonne `amount` (`double`,
//!   « AmountPaid », aide « AmountOfRegistrationPaid ») et un lien `fk_invoice`
//!   vers `llx_facture`. Ce sont des **traces** : le montant est saisi, la
//!   facture est faite par le module de facturation. Aucun encaissement, aucun
//!   règlement, aucune TVA n'est écrit ici. `amount` est **lu** en [`Inscription`]
//!   (en `Decimal`, jamais en `f64` : c'est un montant) et **jamais écrit**.
//!   Le statut « Paid » de la source n'existe d'ailleurs pas : c'est un
//!   affichage calculé (`status == 1 && date_subscription && amount`).
//! - **Pas de capacité maximale.** Aucune colonne de jauge : ni sur la table
//!   d'inscription, ni sur `llx_actioncomm` (vérifié : pas de `nb_available`, pas
//!   de `capacity`). Rien à contrôler, donc rien à prétendre. `inscrits()` rend
//!   un compte, l'appelant en fait ce qu'il veut.
//! - **Pas de remplacement.** `fk_replacement` existe (`int(11)`, index unique
//!   `uk_eventorganization_confboothattendee_replacement`) : c'est le mécanisme
//!   par lequel un inscrit remplace un autre. Il touche à la facturation et à la
//!   traçabilité ; hors périmètre, et non écrit.
//! - **Pas d'email, pas de PDF.** `last_main_doc`, `model_pdf` restent à NULL.
//!
//! # Notes techniques
//!
//! - Aucun montant dans [`NouvelleInscription`] : rien n'est facturé à
//!   l'inscription. `amount` est en lecture seule, en `Decimal`.
//! - Aucune clé `type` dans les `params!` : `type` est un mot réservé embêtant et
//!   une source de confusion avec les discriminators Dolibarr ; si une colonne
//!   portait ce nom, elle serait liée sous `modele_type`. Ici, aucune ne le porte.
//! - `fk_country` est dans la structure demandée mais **n'a pas de colonne** : comme
//!   dans `recrutement`, il est refusé s'il est renseigné plutôt qu'ignoré en
//!   silence — l'appelant croirait avoir enregistré un pays. Dans Dolibarr, le pays
//!   d'un inscrit est celui de sa société (`llx_societe.fk_pays`).

use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mysql::prelude::*;
use mysql::{params, Pool, Row};
use rust_decimal::Decimal;

// ---------------------------------------------------------------------------
// Statuts — recopiés de conferenceorboothattendee.class.php, pas devinés
// ---------------------------------------------------------------------------

/// `STATUS_DRAFT` : l'inscription est saisie, pas encore confirmée.
pub const INSCRIPTION_BROUILLON: i16 = 0;
/// `STATUS_VALIDATED` : l'inscrit est enregistré.
pub const INSCRIPTION_CONFIRMEE: i16 = 1;
/// `STATUS_USED` : présence constatée (« ShowedUp »).
pub const INSCRIPTION_PRESENTE: i16 = 5;
/// `STATUS_CANCELED` : inscription annulée.
pub const INSCRIPTION_ANNULEE: i16 = 9;

// ---------------------------------------------------------------------------
// Bornes — les largeurs réelles des colonnes, en caractères
// ---------------------------------------------------------------------------
//
// `VARCHAR(N)` compte des **caractères** en `utf8mb4`, pas des octets : on
// mesure donc avec `chars().count()`, sinon un nom accentué serait refusé alors
// que la base l'accepte.

/// `email` : `varchar(128)`.
const EMAIL_MAX: usize = 128;
/// `email_company` : `varchar(128)`.
const SOCIETE_EMAIL_MAX: usize = 128;
/// `firstname` : `varchar(100)`.
const PRENOM_MAX: usize = 100;
/// `lastname` : `varchar(100)`.
const NOM_MAX: usize = 100;
/// `ref` est réécrite en `rowid` : la borne ne sert qu'à la valeur provisoire.
const REF_MAX: usize = 128;

// ---------------------------------------------------------------------------
// Structures
// ---------------------------------------------------------------------------

/// Une inscription à créer.
///
/// `fk_actioncomm` est **l'événement** : c'est une ligne d'agenda
/// (`llx_actioncomm`), pas une table du module (voir l'en-tête). `fk_project`
/// n'est pas ici : il est déduit de l'événement, parce que `NOT NULL` en base et
/// sans autre source juste.
///
/// `company` s'écrit dans **`email_company`** : la colonne porte un email de
/// société (`'type' => 'mail'`, libellé « EmailCompany »), pas une raison
/// sociale. Le **nom** de la société, lui, est dans `llx_societe.nom` et se lit
/// par `fk_soc` : le dupliquer ici créerait deux vérités.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NouvelleInscription {
    /// Tiers inscrit (`llx_societe`), facultatif : un visiteur peut venir à titre
    /// personnel.
    pub fk_soc: Option<i64>,
    /// L'événement visé → `fk_actioncomm` (`llx_actioncomm.id`). Obligatoire en
    /// pratique : sans lui, ni date ni lieu (voir l'en-tête).
    pub fk_actioncomm: i64,
    /// Email de l'inscrit, obligatoire : c'est la clé du doublon.
    pub email: String,
    /// `varchar(100)`, facultatif.
    pub firstname: Option<String>,
    /// `varchar(100)`, facultatif — mais le nom **ou** la société doivent être
    /// renseignés.
    pub lastname: Option<String>,
    /// Email de la société → `email_company`. Ce n'est **pas** un nom.
    pub company: Option<String>,
    /// `date_subscription`. `None` écrit `NOW()` : la colonne est obligatoire
    /// pour la classe Dolibarr.
    pub date_subscription: Option<String>,
    /// Aucune colonne dans la table : renseigné, il est **refusé**.
    pub fk_country: Option<i64>,
    /// Écrit dans **`note_public`** : la note accompagne l'inscription, elle est
    /// visible sur la fiche. `note_private` reste à NULL (note interne, hors
    /// périmètre).
    pub note: Option<String>,
}

/// Une inscription relue, avec l'événement et la société qu'elle désigne.
///
/// Le libellé de l'événement est joint parce que, sans table d'événement, un
/// `rowid` de `llx_actioncomm` seul ne dit rien à personne.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inscription {
    pub rowid: i64,
    /// `fk_actioncomm` : l'événement (agenda).
    pub fk_evenement: Option<i64>,
    /// `llx_actioncomm.ref`, si l'événement existe encore.
    pub evenement_ref: Option<String>,
    /// `llx_actioncomm.label` : le nom de la conférence ou du stand.
    pub evenement_label: Option<String>,
    /// `fk_project` : le projet qui porte l'inscription, donc son entité.
    pub fk_project: i64,
    pub fk_soc: Option<i64>,
    /// `llx_societe.nom`, si le tiers existe encore.
    pub societe: Option<String>,
    pub email: Option<String>,
    /// `email_company` : l'email de la société, pas son nom.
    pub email_company: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub date_subscription: Option<String>,
    /// `amount` : montant **déclaré** payé, en `Decimal`. Ce module ne l'écrit
    /// jamais (voir l'en-tête).
    pub montant_paye: Option<Decimal>,
    pub statut: i16,
    pub note_public: Option<String>,
    pub date_creation: Option<String>,
}

// ---------------------------------------------------------------------------
// Ce que le module refuse sans toucher la base
// ---------------------------------------------------------------------------

/// Le refus d'une transition de statut, avec sa raison.
///
/// Ce type existe pour que la table des transitions reste **pure** : elle ne
/// touche ni la base ni l'horloge, et s'explique dans un test sans MariaDB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurTransition {
    /// Valeur qui n'existe dans aucun statut de Dolibarr.
    Inconnu(i16),
    /// Transition absente de la séquence (état terminal compris).
    HorsSequence { depuis: i16, vers: i16 },
}

impl fmt::Display for ErreurTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErreurTransition::Inconnu(valeur) => write!(
                f,
                "statut d'inscription inconnu : {valeur} (la source n'en connaît que \
                 0, 1, 5 et 9)"
            ),
            ErreurTransition::HorsSequence { depuis, vers } => write!(
                f,
                "transition refusée : de « {} » ({depuis}) vers « {} » ({vers}) n'est \
                 pas dans la séquence des inscriptions",
                libelle_statut(*depuis),
                libelle_statut(*vers)
            ),
        }
    }
}

impl Error for ErreurTransition {}

/// Les transitions permises, et **seulement** celles-là.
///
/// Voir l'en-tête : la table est calquée sur les gardes de la source
/// (`validate()` refuse de revalider, `cancel()` exige le statut 1, `STATUS_USED`
/// n'a pas d'écrivain). `reopen()` (9 → 1) existe en PHP mais n'est pas exposé
/// ici, donc refusé.
pub fn transition_permise(depuis: i16, vers: i16) -> bool {
    matches!(
        (depuis, vers),
        (INSCRIPTION_BROUILLON, INSCRIPTION_CONFIRMEE)
            | (INSCRIPTION_CONFIRMEE, INSCRIPTION_ANNULEE)
            | (INSCRIPTION_CONFIRMEE, INSCRIPTION_PRESENTE)
    )
}

/// Le libellé d'un statut, tel que le tableau `arrayofkeyval` de la source le
/// donne : « Draft », « Registered », « ShowedUp », « Canceled ».
pub fn libelle_statut(statut: i16) -> &'static str {
    match statut {
        INSCRIPTION_BROUILLON => "brouillon",
        INSCRIPTION_CONFIRMEE => "confirmée",
        INSCRIPTION_PRESENTE => "présente",
        INSCRIPTION_ANNULEE => "annulée",
        _ => "statut inconnu",
    }
}

/// Une date ou un horodatage acceptable pour `date_subscription`.
///
/// Accepte `AAAA-MM-JJ` et `AAAA-MM-JJ HH:MM:SS` (ce que rend la base, ce qu'un
/// formulaire envoie). Refuse tout le reste, y compris les dates qui n'existent
/// pas (`2026-02-30`) : MariaDB les refuserait de toute façon en mode strict,
/// mais le message serait un charabia SQL au lieu de la vraie raison.
///
/// L'année est bornée à 1000-9999 : c'est le domaine de `DATETIME`, et une
/// année 0000 n'est pas une date, c'est une chaîne.
pub fn date_valide(valeur: &str) -> bool {
    let valeur = valeur.trim();
    let (jour, heure) = match valeur.split_once(' ') {
        Some((jour, heure)) => (jour, Some(heure)),
        None => (valeur, None),
    };

    // Largeurs fixes : `AAAA-MM-JJ` (10) et `HH:MM:SS` (8). Sans cela,
    // `2026-1-6` passerait pour le 6 janvier, alors que ce n'est pas la forme
    // que la base écrit ni celle qu'on accepte de relire.
    if jour.len() != 10 {
        return false;
    }
    let Some((annee, mois, jour)) = trois_nombres(jour, '-') else {
        return false;
    };
    if !(1000..=9999).contains(&annee) || !(1..=12).contains(&mois) {
        return false;
    }
    let jours_du_mois = match mois {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        // Février : bissextile si divisible par 4, sauf les siècles non
        // divisibles par 400 (2000 oui, 1900 non). Même écriture que
        // `adherent.rs` : `is_multiple_of`, que clippy préfère à `% n == 0`.
        2 if annee.is_multiple_of(400)
            || (annee.is_multiple_of(4) && !annee.is_multiple_of(100)) =>
        {
            29
        }
        2 => 28,
        _ => return false,
    };
    if !(1..=jours_du_mois).contains(&jour) {
        return false;
    }

    match heure {
        None => true,
        Some(heure) if heure.len() == 8 => match trois_nombres(heure, ':') {
            Some((h, m, s)) => h <= 23 && m <= 59 && s <= 59,
            None => false,
        },
        Some(_) => false,
    }
}

/// Trois nombres séparés par `separateur`, chacun en chiffres ASCII.
fn trois_nombres(valeur: &str, separateur: char) -> Option<(u32, u32, u32)> {
    let morceaux: Vec<&str> = valeur.split(separateur).collect();
    if morceaux.len() != 3 {
        return None;
    }
    let a = nombre(morceaux[0])?;
    let b = nombre(morceaux[1])?;
    let c = nombre(morceaux[2])?;
    Some((a, b, c))
}

/// Un champ numérique : chiffres ASCII uniquement (pas de `+`, pas d'espace).
fn nombre(champ: &str) -> Option<u32> {
    if champ.is_empty() || !champ.bytes().all(|o| o.is_ascii_digit()) {
        return None;
    }
    champ.parse().ok()
}

/// Ce qu'une inscription doit avoir pour être écrite. **Pur** : ni base, ni
/// horloge, donc testable sans MariaDB.
///
/// # Erreurs
///
/// Email vide, sans arobase (ou avec plusieurs), trop long ; ni nom ni société ;
/// date invalide ; `fk_country` renseigné ; événement absent ; champs trop longs.
pub fn valider_inscription(new: &NouvelleInscription) -> Result<(), Box<dyn Error>> {
    let email = new.email.trim();
    if email.is_empty() {
        return Err(
            "l'email de l'inscrit est obligatoire (`email`) : c'est la clé qui \
                    empêche d'inscrire deux fois la même personne"
                .into(),
        );
    }
    if email.chars().count() > EMAIL_MAX {
        return Err(format!(
            "l'email fait {} caractères, la colonne `email` en accepte {EMAIL_MAX}",
            email.chars().count()
        )
        .into());
    }
    if email.contains(char::is_whitespace) {
        return Err(format!("« {email} » contient un espace : ce n'est pas un email").into());
    }
    // Exactement une arobase, avec du texte des deux côtés. On n'exige pas de
    // point dans le domaine : `contact@intranet` est une adresse légitime, et
    // Dolibarr ne l'interdit pas non plus.
    let morceaux: Vec<&str> = email.split('@').collect();
    if morceaux.len() != 2 || morceaux[0].is_empty() || morceaux[1].is_empty() {
        return Err(format!(
            "« {email} » n'est pas une adresse email : il faut exactement une arobase, \
             avec du texte avant et après"
        )
        .into());
    }

    if new.fk_actioncomm <= 0 {
        return Err(
            "une inscription vise un événement (`fk_actioncomm`), et un événement \
                    sans date ni lieu ne donne rien à inscrire"
                .into(),
        );
    }

    // « Un nom » = un nom de famille ou un prénom ; « une société » = un tiers
    // rattaché ou l'email de la société. Les deux vides : on ne sait pas qui est
    // inscrit, et la ligne serait inexploitable.
    let nom = new
        .lastname
        .as_deref()
        .is_some_and(|valeur| !valeur.trim().is_empty())
        || new
            .firstname
            .as_deref()
            .is_some_and(|valeur| !valeur.trim().is_empty());
    let societe = new.fk_soc.is_some_and(|id| id > 0)
        || new
            .company
            .as_deref()
            .is_some_and(|valeur| !valeur.trim().is_empty());
    if !nom && !societe {
        return Err(
            "une inscription sans nom (lastname/firstname) et sans société \
                    (fk_soc/company) ne dit pas qui vient : elle est refusée"
                .into(),
        );
    }

    if let Some(date) = new.date_subscription.as_deref() {
        if !date_valide(date) {
            return Err(format!(
                "la date d'inscription « {date} » n'est pas valide : attendu AAAA-MM-JJ \
                 ou AAAA-MM-JJ HH:MM:SS, et une date qui existe"
            )
            .into());
        }
    }

    if new.fk_country.is_some() {
        return Err("fk_country n'a pas de colonne dans \
                    llx_eventorganization_conferenceorboothattendee : le pays d'un inscrit \
                    est celui de sa société (llx_societe.fk_pays). Le passer ici ferait \
                    croire qu'il est enregistré"
            .into());
    }
    if new.fk_soc.is_some_and(|id| id <= 0) {
        return Err("fk_soc vaut 0 ou moins : soit un tiers réel, soit None".into());
    }

    verifier_longueur(new.lastname.as_deref(), NOM_MAX, "le nom (`lastname`)")?;
    verifier_longueur(
        new.firstname.as_deref(),
        PRENOM_MAX,
        "le prénom (`firstname`)",
    )?;
    verifier_longueur(
        new.company.as_deref(),
        SOCIETE_EMAIL_MAX,
        "l'email de la société (`company` → `email_company`)",
    )?;

    Ok(())
}

/// Refuse un champ plus long que sa colonne, en **caractères**.
fn verifier_longueur(
    valeur: Option<&str>,
    maximum: usize,
    quoi: &str,
) -> Result<(), Box<dyn Error>> {
    match valeur {
        Some(valeur) if valeur.chars().count() > maximum => Err(format!(
            "{quoi} fait {} caractères, la colonne en accepte {maximum}",
            valeur.chars().count()
        )
        .into()),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Lecture
// ---------------------------------------------------------------------------

/// Colonnes lues, dans l'ordre de [`map_inscription`].
const SELECT_INSCRIPTION: &str = "t.rowid, t.fk_actioncomm, a.ref, a.label, \
     t.fk_project, t.fk_soc, s.nom, t.email, t.email_company, t.firstname, \
     t.lastname, DATE_FORMAT(t.date_subscription, '%Y-%m-%d %H:%i:%s'), \
     CAST(t.amount AS CHAR), t.status, t.note_public, \
     DATE_FORMAT(t.date_creation, '%Y-%m-%d %H:%i:%s')";

/// La jointure qui porte le cloisonnement : `llx_projet` fournit `entity`.
///
/// `JOIN` et non `LEFT JOIN` : une inscription sans projet n'aurait pas d'entité,
/// donc pas de propriétaire ; elle n'est pas lisible ici, et c'est le prix du
/// cloisonnement sans colonne `entity`.
const DEPUIS_INSCRIPTION: &str = "FROM llx_eventorganization_conferenceorboothattendee t \
     JOIN llx_projet p ON p.rowid = t.fk_project \
     LEFT JOIN llx_actioncomm a ON a.id = t.fk_actioncomm \
     LEFT JOIN llx_societe s ON s.rowid = t.fk_soc";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

/// Un montant se lit en texte puis en `Decimal` : `amount` est un `double`, et
/// passer par `f64` pour un montant est exactement ce qu'il ne faut pas faire.
fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_inscription(row: &Row) -> Inscription {
    Inscription {
        rowid: opt_i(row, 0).unwrap_or(0),
        fk_evenement: opt_i(row, 1),
        evenement_ref: opt_s(row, 2),
        evenement_label: opt_s(row, 3),
        fk_project: opt_i(row, 4).unwrap_or(0),
        fk_soc: opt_i(row, 5),
        societe: opt_s(row, 6),
        email: opt_s(row, 7),
        email_company: opt_s(row, 8),
        firstname: opt_s(row, 9),
        lastname: opt_s(row, 10),
        date_subscription: opt_s(row, 11),
        montant_paye: opt_dec(row, 12),
        statut: opt_i(row, 13).unwrap_or(0) as i16,
        note_public: opt_s(row, 14),
        date_creation: opt_s(row, 15),
    }
}

/// Les inscriptions d'une entité, de la plus récente à la plus ancienne.
///
/// Le filtre porte sur `llx_projet.entity` : la table n'a pas de colonne
/// `entity` (voir l'en-tête). Rend un `Result` comme toutes les listes du
/// noyau : une panne MySQL doit remonter, pas devenir une liste vide qu'on
/// croirait complète.
pub fn list(pool: &Pool, entity: i64, limite: u32) -> Result<Vec<Inscription>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_INSCRIPTION} {DEPUIS_INSCRIPTION} \
         WHERE p.entity = :entity \
         ORDER BY t.date_subscription DESC, t.rowid DESC LIMIT :limite"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "limite" => limite })?;
    Ok(rows.iter().map(map_inscription).collect())
}

/// Une inscription, lue dans son entité.
///
/// `Ok(None)` et non une erreur : une lecture qui ne trouve rien n'est pas une
/// panne. Une inscription d'une autre entité est indiscernable d'une inscription
/// absente, et c'est voulu — le cloisonnement ne doit pas servir d'oracle.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Inscription>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_INSCRIPTION} {DEPUIS_INSCRIPTION} \
         WHERE p.entity = :entity AND t.rowid = :rowid"
    );
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.as_ref().map(map_inscription))
}

// ---------------------------------------------------------------------------
// Comptage
// ---------------------------------------------------------------------------

/// Combien de personnes sont inscrites à un événement.
///
/// **Les annulées ne comptent pas.** Une inscription annulée reste en base (elle
/// garde sa trace, et `reopen()` existe côté PHP), mais elle ne représente plus
/// personne : la compter ferait croire à une salle plus pleine qu'elle n'est.
/// Les présentes, elles, comptent : elles étaient inscrites, et le sont restées.
///
/// Un événement inconnu rend **0** et non une erreur : c'est un compte, et le
/// compte de rien vaut zéro. Le contrôle d'existence appartient à [`inscrire`],
/// qui en a besoin pour écrire.
pub fn inscrits(pool: &Pool, entity: i64, fk_evenement: i64) -> Result<usize, Box<dyn Error>> {
    compter(
        pool,
        entity,
        fk_evenement,
        "t.status <> :valeur",
        INSCRIPTION_ANNULEE,
    )
}

/// Combien de présences confirmées (`STATUS_USED`, 5) pour un événement.
///
/// C'est un sous-ensemble de [`inscrits`] : on ne peut être présent qu'en étant
/// inscrit, et `marquer_present` n'accepte que depuis l'état confirmé.
pub fn presents(pool: &Pool, entity: i64, fk_evenement: i64) -> Result<usize, Box<dyn Error>> {
    compter(
        pool,
        entity,
        fk_evenement,
        "t.status = :valeur",
        INSCRIPTION_PRESENTE,
    )
}

/// Le comptage commun aux deux fonctions ci-dessus.
///
/// `condition` est un fragment **fixe**, choisi par le code ci-dessus, jamais par
/// un appelant : aucune valeur utilisateur n'entre dans le SQL.
fn compter(
    pool: &Pool,
    entity: i64,
    fk_evenement: i64,
    condition: &str,
    valeur: i16,
) -> Result<usize, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT COUNT(*) FROM llx_eventorganization_conferenceorboothattendee t \
         JOIN llx_projet p ON p.rowid = t.fk_project \
         WHERE p.entity = :entity AND t.fk_actioncomm = :evenement AND {condition}"
    );
    let total: Option<i64> = conn.exec_first(
        sql,
        params! { "entity" => entity, "evenement" => fk_evenement, "valeur" => valeur },
    )?;
    // `COUNT(*)` ne renvoie jamais NULL ; le `unwrap_or` couvre le pilote, pas la
    // logique. Le compte ne peut pas être négatif : la conversion est sûre.
    Ok(total.unwrap_or(0).max(0) as usize)
}

// ---------------------------------------------------------------------------
// Écriture
// ---------------------------------------------------------------------------

/// Référence provisoire unique, avant d'être remplacée par le `rowid`.
///
/// Dolibarr écrit `"(PROV)"` : deux créations simultanées se heurteraient à
/// l'index unique `uk_eventorganization_confboothattendee`. On garde la forme
/// reconnaissable (`(PROV…`) et on y met de quoi être unique.
fn reference_provisoire() -> String {
    static COMPTEUR: AtomicU64 = AtomicU64::new(0);
    let numero = COMPTEUR.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duree| duree.as_nanos() as u64)
        .unwrap_or(0);
    let reference = format!("(PROV-{nanos}-{numero})");
    reference.chars().take(REF_MAX).collect()
}

/// Inscrit une personne à un événement et rend le `rowid` de l'inscription.
///
/// L'inscription naît en **brouillon** (0), comme `create()` dans la source :
/// s'inscrire et confirmer sont deux actes, et c'est [`confirmer`] qui promeut.
///
/// `fk_project` n'est pas dans la structure : il est **déduit de l'événement**
/// (`llx_actioncomm.fk_project`), seule valeur qui décide aussi de l'entité (voir
/// l'en-tête). Un événement sans projet est refusé.
///
/// # Erreurs
///
/// Tout ce que refuse [`valider_inscription`], plus : événement inconnu dans
/// l'entité, événement sans projet, projet d'une autre entité, société inconnue,
/// **doublon** (même email, même événement), et toute erreur MySQL remontée telle
/// quelle — y compris la violation de l'index unique, si deux inscriptions
/// concurrentes passaient entre la vérification et l'écriture.
pub fn inscrire(
    pool: &Pool,
    entity: i64,
    new: &NouvelleInscription,
) -> Result<i64, Box<dyn Error>> {
    valider_inscription(new)?;

    let email = new.email.trim();
    let mut conn = pool.get_conn()?;

    // 1. L'événement doit exister **dans cette entité** et porter un projet de la
    //    même entité. Un seul aller-retour : `LEFT JOIN` pour distinguer « pas
    //    d'événement » de « événement sans projet » — deux refus différents, donc
    //    deux messages différents.
    let evenement: Option<(Option<i64>, Option<i64>)> = conn.exec_first(
        "SELECT a.fk_project, p.entity \
         FROM llx_actioncomm a LEFT JOIN llx_projet p ON p.rowid = a.fk_project \
         WHERE a.id = :evenement AND a.entity = :entity",
        params! { "evenement" => new.fk_actioncomm, "entity" => entity },
    )?;
    let (projet, entite_projet) = match evenement {
        None => {
            return Err(format!(
                "événement {} introuvable dans l'entité {entity} : une inscription sans \
                 événement n'aurait ni date ni lieu",
                new.fk_actioncomm
            )
            .into())
        }
        Some((None, _)) => {
            return Err(format!(
                "l'événement {} n'est rattaché à aucun projet : `fk_project` est NOT NULL \
                 et c'est lui qui porte l'entité, l'inscription serait invisible",
                new.fk_actioncomm
            )
            .into())
        }
        Some((Some(projet), None)) => {
            return Err(format!(
                "le projet {projet} de l'événement {} n'existe plus : impossible de \
                 rattacher l'inscription",
                new.fk_actioncomm
            )
            .into())
        }
        Some((Some(projet), Some(entite_projet))) => (projet, entite_projet),
    };
    if entite_projet != entity {
        return Err(format!(
            "le projet {projet} de l'événement {} appartient à l'entité {entite_projet}, \
             pas à {entity} : l'inscription y serait invisible",
            new.fk_actioncomm
        )
        .into());
    }

    // 2. La société visée, si elle est nommée, doit être de la même entité. Sans
    //    clé étrangère en base, c'est la seule garde.
    if let Some(fk_soc) = new.fk_soc {
        verifier_societe(pool, entity, fk_soc)?;
    }

    // 3. Le doublon : même email, même événement. La comparaison suit la
    //    collation `utf8mb4_unicode_ci` de la colonne, donc la casse est ignorée —
    //    exactement la sémantique de l'index unique.
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_eventorganization_conferenceorboothattendee \
         WHERE email = :email AND fk_actioncomm = :evenement LIMIT 1",
        params! { "email" => email, "evenement" => new.fk_actioncomm },
    )?;
    if let Some(rowid) = deja {
        return Err(format!(
            "« {email} » est déjà inscrit à l'événement {} (inscription {rowid}) : un même \
             email ne s'inscrit qu'une fois par événement, et reste libre sur un autre \
             événement",
            new.fk_actioncomm
        )
        .into());
    }

    // 4. Écriture : insertion avec une référence provisoire, puis renommage en
    //    `rowid` — le comportement de `create()`, dans une transaction pour que
    //    l'état intermédiaire ne survive à aucune panne.
    let mut tx = conn.start_transaction(mysql::TxOpts::default())?;
    tx.exec_drop(
        "INSERT INTO llx_eventorganization_conferenceorboothattendee \
           (ref, fk_project, fk_actioncomm, fk_soc, email, email_company, firstname, \
            lastname, date_subscription, note_public, status, date_creation) \
         VALUES (:ref, :projet, :evenement, :soc, :email, :email_societe, :prenom, \
            :nom, COALESCE(:date_souscription, NOW()), :note, :statut, NOW())",
        params! {
            "ref" => reference_provisoire(),
            "projet" => projet,
            "evenement" => new.fk_actioncomm,
            "soc" => new.fk_soc,
            "email" => email,
            "email_societe" => new.company.as_deref().map(str::trim),
            "prenom" => new.firstname.as_deref().map(str::trim),
            "nom" => new.lastname.as_deref().map(str::trim),
            "date_souscription" => new.date_subscription.as_deref(),
            "note" => new.note.as_deref(),
            "statut" => INSCRIPTION_BROUILLON,
        },
    )?;
    // Sur une transaction, `last_insert_id` rend un `Option` : zero
    // signalerait une insertion perdue, et une ligne a 0 ne se relit pas.
    let rowid = tx
        .last_insert_id()
        .ok_or("inscription inseree sans identifiant")? as i64;
    tx.exec_drop(
        "UPDATE llx_eventorganization_conferenceorboothattendee \
         SET ref = CAST(rowid AS CHAR) WHERE rowid = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.commit()?;

    Ok(rowid)
}

/// La société existe-t-elle dans cette entité ?
fn verifier_societe(pool: &Pool, entity: i64, id: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE rowid = :rowid AND entity = :entity",
        params! { "entity" => entity, "rowid" => id },
    )?;
    match existe {
        Some(_) => Ok(()),
        None => Err(format!("la société {id} n'existe pas dans l'entité {entity}").into()),
    }
}

/// Le changement de statut, avec sa garde.
///
/// Une seule implémentation pour les trois actes : la séquence est vérifiée une
/// fois, au même endroit, et les trois fonctions publiques ne sont que des noms
/// justes sur une cible juste. C'est ce qui garantit qu'aucun chemin ne
/// contourne [`transition_permise`].
fn changer_statut(pool: &Pool, entity: i64, rowid: i64, vers: i16) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // Le statut est lu sous la même jointure que les lectures : on ne change pas
    // le statut d'une inscription qu'on ne pourrait pas lire.
    let depuis: Option<i64> = conn.exec_first(
        "SELECT t.status FROM llx_eventorganization_conferenceorboothattendee t \
         JOIN llx_projet p ON p.rowid = t.fk_project \
         WHERE p.entity = :entity AND t.rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(depuis) = depuis else {
        return Err(format!("inscription {rowid} introuvable dans l'entité {entity}").into());
    };
    let depuis = depuis as i16;

    if !matches!(
        depuis,
        INSCRIPTION_BROUILLON | INSCRIPTION_CONFIRMEE | INSCRIPTION_PRESENTE | INSCRIPTION_ANNULEE
    ) {
        return Err(ErreurTransition::Inconnu(depuis).into());
    }
    if !transition_permise(depuis, vers) {
        return Err(ErreurTransition::HorsSequence { depuis, vers }.into());
    }

    conn.exec_drop(
        "UPDATE llx_eventorganization_conferenceorboothattendee \
         SET status = :statut WHERE rowid = :rowid",
        params! { "statut" => vers, "rowid" => rowid },
    )?;
    Ok(())
}

/// Confirme une inscription : **brouillon (0) → confirmée (1)**.
///
/// Reprend la garde de `validate()` : une inscription déjà confirmée est refusée
/// (`return 0` dans la source), pas silencieusement réécrite. La référence n'est
/// pas renumérotée : elle vaut déjà le `rowid`, donc le test
/// `preg_match('/^[\(]?PROV/i')` de la source ne la reconnaît pas.
///
/// # Erreurs
///
/// Inscription introuvable dans l'entité, statut inconnu, ou transition hors
/// séquence (déjà confirmée, présente, annulée).
pub fn confirmer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    changer_statut(pool, entity, rowid, INSCRIPTION_CONFIRMEE)
}

/// Annule une inscription : **confirmée (1) → annulée (9)**.
///
/// Reprend la garde de `cancel()` : `if ($this->status != self::STATUS_VALIDATED)
/// return 0;`. Un **brouillon ne s'annule donc pas** : il n'a jamais été
/// confirmé, il se supprime. Inventer une annulation laisserait une trace
/// commerciale qui n'a pas eu lieu.
///
/// # Erreurs
///
/// Inscription introuvable, statut inconnu, ou transition hors séquence
/// (brouillon, déjà annulée, présente).
pub fn annuler(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    changer_statut(pool, entity, rowid, INSCRIPTION_ANNULEE)
}

/// Marque la présence : **confirmée (1) → présente (5)**.
///
/// `STATUS_USED` n'a aucun écrivain dans la source ; son commentaire fait foi
/// (« présence confirmée, plus aucune entrée avec ce billet »). On l'écrit donc
/// directement, et seulement depuis l'état confirmé : marquer présent quelqu'un
/// qui n'était pas inscrit n'a pas de sens, et un brouillon n'est pas une
/// inscription.
///
/// # Erreurs
///
/// Inscription introuvable, statut inconnu, ou transition hors séquence
/// (brouillon, déjà présente, annulée).
pub fn marquer_present(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    changer_statut(pool, entity, rowid, INSCRIPTION_PRESENTE)
}
