//! Recrutement : offres d'emploi et candidatures.
//!
//! Deux tables du module `recruitment` de Dolibarr 24.0.1 :
//!
//! | table | ce qu'elle porte |
//! |---|---|
//! | `llx_recruitment_recruitmentjobposition` | l'offre : une référence, un intitulé, un nombre de postes (`qty`), un recruteur, une date prévue, une société, un statut |
//! | `llx_recruitment_recruitmentcandidature` | la candidature : un nom, un prénom, un email, un téléphone, une date de naissance, une origine (dictionnaire `llx_c_recruitment_origin`), un statut |
//!
//! Les deux sont vides sur l'instance de référence : ce module n'a donc aucun
//! historique à ménager, seulement un schéma à respecter.
//!
//! # Les statuts, tels que la source les nomme
//!
//! `recruitmentjobposition.class.php` :
//!
//! ```text
//! const STATUS_DRAFT     = 0;   // brouillon, jamais publiée
//! const STATUS_VALIDATED = 1;   // publiée
//! const STATUS_RECRUITED = 3;   // pourvue : le recrutement a abouti
//! const STATUS_CANCELED  = 9;   // clôturée / annulée
//! ```
//!
//! `recruitmentcandidature.class.php` :
//!
//! ```text
//! const STATUS_DRAFT            = 0;   // saisie en cours
//! const STATUS_VALIDATED        = 1;   // « Received (InterviewToDo) », fr : « Contacts à suivre »
//! //const STATUS_INTERVIEW_SCHEDULED = 2;   <-- COMMENTÉ dans la source
//! const STATUS_CONTRACT_PROPOSED = 3;  // contrat proposé
//! const STATUS_CONTRACT_SIGNED   = 5;  // contrat signé
//! const STATUS_CONTRACT_REFUSED  = 6;  // contrat refusé
//! const STATUS_REFUSED           = 8;  // candidature refusée
//! const STATUS_CANCELED          = 9;  // annulée
//! ```
//!
//! # Le cycle demandé, traduit en codes réels
//!
//! Offre : **brouillon (0) → publiée (1) → close (9)**.
//!
//! La publication reprend la garde de `validate()` : « already validated »
//! renvoie 0, donc republier est refusé. La clôture reprend celle de `cancel()`
//! (`if ($this->status != self::STATUS_VALIDATED) return 0;`), qui écrit
//! **9** et déclenche `RECRUITMENTJOBPOSITION_CLOSE` : dans Dolibarr, « clore
//! l'offre » **est** l'annulation du statut. Le statut 3 (pourvue) existe, mais
//! il n'est écrit que par `cloture($user, $status, …)`, qui n'a **aucune garde**
//! et prend le statut en paramètre. Ce module ne l'écrit pas : voir « les postes
//! pourvus » plus bas, il n'a aucun moyen de savoir qu'un poste est réellement
//! pourvu autrement que par un statut de candidature déclaré à la main.
//!
//! Candidature : **reçue (1) → contrat proposé (3) → retenue (5)**, avec deux
//! sorties, **refusée (8)** et **annulée (9)**, et la réouverture prévue par
//! `reopen()` (8 ou 9 → 1).
//!
//! L'étape « en cours d'entretien » n'a **aucun code** : le statut 2
//! (`INTERVIEW_SCHEDULED`) est **commenté** dans la source. On ne l'invente
//! pas. Une candidature « en cours d'entretien » reste donc au statut 1, dont
//! le libellé officiel est « Contacts à suivre » — c'est-à-dire précisément
//! « reçue, entretien à faire ». Le passage à 3 (« contrat proposé ») marque la
//! fin de l'entretien, pas son début. Toute écriture de la valeur 2 est refusée
//! avec un message qui dit pourquoi.
//!
//! # Le cloisonnement par entité : mesuré, pas supposé
//!
//! **Les deux tables portent une colonne `entity`** (`int(11) NOT NULL DEFAULT 1`
//! sur chacune, relevé dans `information_schema.columns`). Le cloisonnement ne
//! vient donc pas seulement de l'offre : chaque candidature est cloisonnée par
//! sa propre colonne, et une candidature pourrait pointer une offre d'une autre
//! entité sans que la base s'en plaigne — il n'y a **aucune clé étrangère**.
//!
//! Ce module écrit `candidature.entity = entity` et vérifie que l'offre visée
//! existe **dans cette même entité** : la cohérence est obtenue par la
//! vérification, pas par le schéma. Une lecture fautive (offer d'entité 1 lue
//! depuis l'entité 2) est donc refusée à la création.
//!
//! Aucun index ne porte sur `entity` dans les deux tables, et
//! `llx_recruitment_recruitmentcandidature` **n'a aucun index sur
//! `fk_recruitmentjobposition`** (`SHOW INDEX` : PRIMARY, unique sur
//! `email_msgid`, puis `rowid`, `ref`, `fk_user_creat`, `status`). Compter les
//! candidatures d'une offre balaie donc la table. Sur une instance qui reçoit
//! beaucoup de candidatures, c'est un index à ajouter côté base — pas une
//! migration que ce noyau s'autorise.
//!
//! # Une offre close qui reçoit encore une candidature
//!
//! **Décision : on l'accepte, on l'écrit, et on le signale.** Pas de refus.
//!
//! Trois raisons :
//!
//! 1. une candidature close n'est pas une candidature fausse. Les gens
//!    postulent après la clôture, et Dolibarr les collecte par email
//!    (`email_msgid`, `ip` : la table est faite pour l'import automatique) ;
//!    refuser ferait perdre une personne réelle et l'erreur serait invisible ;
//! 2. un refus ferait échouer un collecteur en boucle sur la même candidature,
//!    ou la ferait disparaître sans trace — les deux pires issues ;
//! 3. le statut 9 de l'offre ne dit pas « pourvue » mais « annulée » : la
//!    candidature tardive reste parfaitement exploitable si l'offre rouvre
//!    (`reopen()` existe).
//!
//! Le signalement est matériel, pas décoratif : la mention
//! « Candidature reçue sur une offre non publiée (statut N : …) » est ajoutée à
//! `note_private` (visible dans la fiche Dolibarr), et
//! [`CandidatureCreee::offre_hors_publication`] vaut `true` pour que l'appelant
//! puisse alerter. La même règle couvre le brouillon (statut 0) et l'offre
//! pourvue (3) : tout ce qui n'est pas publié est signalé.
//!
//! # Les postes pourvus : ce qui existe, ce qui manque
//!
//! Le champ demandé `nb_recruitment_planned` **n'existe pas sous ce nom** : la
//! colonne s'appelle **`qty`** (`int(11) NOT NULL DEFAULT 1`). C'est elle qui
//! porte le nombre de postes prévus, et [`NouvelleOffre::nb_recruitment_planned`]
//! y écrit sans détour.
//!
//! Compter les **retenus** est possible, mais seulement au sens déclaratif :
//! `candidature.status = 5` (`STATUS_CONTRACT_SIGNED`). [`bilan_postes`] rend
//! les deux chiffres côte à côte pour qu'on ne les confonde pas.
//!
//! Ce qui **manque** pour compter pour de vrai :
//!
//! - aucune colonne `fk_contrat` ni `date_recruited` : rien ne relie une
//!   candidature retenue à un contrat de travail, et `llx_contrat` de Dolibarr
//!   est un contrat **commercial**, pas un contrat d'embauche ;
//! - `llx_recruitment_recruitmentjobposition.qty` n'est jamais décrémenté par
//!   Dolibarr, et rien ne garantit que `status = 5` soit posé ;
//! - aucun index ni contrainte ne borne le nombre de retenus au nombre de
//!   postes : trois retenus sur `qty = 2` passent sans bruit.
//!
//! Autrement dit : `prevus` est une donnée saisie, `pourvus` une donnée
//! déclarée. Ce module ne prétend pas que l'un découle de l'autre, et n'écrit
//! jamais `status = 3` (pourvue) sur une offre pour faire joli.
//!
//! # Une colonne demandée qui n'existe pas : `fk_country`
//!
//! `NouvelleOffre` porte `fk_country` parce que l'interface demandée le porte,
//! mais `llx_recruitment_recruitmentjobposition` **n'a aucune colonne de pays**
//! (vérifié colonne par colonne dans `information_schema.columns`). Plutôt que
//! de l'ignorer en silence — l'appelant croirait avoir enregistré un pays — il
//! est **refusé** dès qu'il est renseigné, avec un message qui dit où le pays
//! vit réellement dans Dolibarr (la société : `llx_societe.fk_pays`).
//!
//! # Ce que ce module n'écrit pas
//!
//! Ni rémunération, ni PDF, ni note publique, ni email. Les deux colonnes de
//! rémunération de la candidature (`remuneration_requested`,
//! `remuneration_proposed`) sont des **`int(11)`**, pas des décimales : même si
//! on les exposait, `Decimal` serait un contresens. Elles restent hors du
//! périmètre demandé.
//!
//! Une candidature est écrite directement au statut **1** (reçue) : Dolibarr la
//! crée en 0 puis appelle `validate()`, mais la réception *est* l'acte de
//! création ici, et un brouillon de candidature n'aurait aucun sens (personne
//! ne saisit une candidature « pour plus tard »).

use std::error::Error;
use std::fmt;

use mysql::prelude::*;
use mysql::{params, Pool, Row};

use crate::divers;

// ---------------------------------------------------------------------------
// Statuts — recopiés de la source PHP, pas devinés
// ---------------------------------------------------------------------------

/// `STATUS_DRAFT` : l'offre existe, elle n'est pas publiée.
pub const STATUT_BROUILLON: i16 = 0;
/// `STATUS_VALIDATED` : l'offre est publiée, elle reçoit des candidatures.
pub const STATUT_PUBLIEE: i16 = 1;
/// `STATUS_RECRUITED` : le recrutement a abouti. Jamais écrit ici (voir l'entête).
pub const STATUT_POURVUE: i16 = 3;
/// `STATUS_CANCELED`, écrit par `cancel()` : c'est la clôture de l'offre.
pub const STATUT_CLOTUREE: i16 = 9;

/// `STATUS_DRAFT` d'une candidature : saisie en cours, jamais écrite ici.
pub const CANDIDATURE_BROUILLON: i16 = 0;
/// `STATUS_VALIDATED` : reçue, « Contacts à suivre » — entretien à faire.
pub const CANDIDATURE_RECUE: i16 = 1;
/// Le statut 2 (`INTERVIEW_SCHEDULED`) est **commenté** dans la source.
pub const CANDIDATURE_ENTRETIEN_PLANIFIE_RETIRE: i16 = 2;
/// `STATUS_CONTRACT_PROPOSED` : l'entretien a eu lieu, un contrat est proposé.
pub const CANDIDATURE_CONTRAT_PROPOSE: i16 = 3;
/// `STATUS_CONTRACT_SIGNED` : retenue.
pub const CANDIDATURE_RETENUE: i16 = 5;
/// `STATUS_CONTRACT_REFUSED` : le candidat a refusé le contrat.
pub const CANDIDATURE_CONTRAT_REFUSE: i16 = 6;
/// `STATUS_REFUSED` : refusée par l'employeur.
pub const CANDIDATURE_REFUSEE: i16 = 8;
/// `STATUS_CANCELED` : annulée (retrait, doublon…).
pub const CANDIDATURE_ANNULEE: i16 = 9;

/// Préfixe de référence des candidatures créées par ce noyau.
///
/// Le module de numérotation de Dolibarr (`mod_recruitmentcandidature_standard`)
/// n'est **pas** dans la copie de référence vendorisée : son format exact ne
/// peut donc pas être recopié. `CA` + quatre chiffres, par entité, tient lieu de
/// référence lisible et unique. Dolibarr conserve une référence qui ne commence
/// pas par `(PROV`, la nôtre n'est donc pas renumérotée à la validation.
pub const PREFIXE_CANDIDATURE: &str = "CA";

/// Taille de `llx_recruitment_recruitmentjobposition.ref`, en octets.
const REF_MAX: usize = 128;
/// Taille de `llx_recruitment_recruitmentjobposition.label`, en octets.
const LABEL_MAX: usize = 255;
/// Taille de `llx_recruitment_recruitmentcandidature.lastname`.
const NOM_MAX: usize = 128;
/// Taille de `llx_recruitment_recruitmentcandidature.firstname`.
const PRENOM_MAX: usize = 128;
/// Taille de `llx_recruitment_recruitmentcandidature.email`.
const EMAIL_MAX: usize = 255;
/// Taille de `llx_recruitment_recruitmentcandidature.phone`.
const PHONE_MAX: usize = 64;

// ---------------------------------------------------------------------------
// Structures
// ---------------------------------------------------------------------------

/// Une offre à créer.
///
/// `nb_recruitment_planned` s'écrit dans la colonne **`qty`** : la colonne
/// demandée n'existe pas sous ce nom (voir l'entête du module).
///
/// `fk_country` est refusé s'il est renseigné : la table n'a pas de colonne de
/// pays.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NouvelleOffre {
    /// Référence imposée par l'appelant (`llx_…jobposition.ref`, NOT NULL).
    /// `ref` est un mot réservé de Rust : d'où le suffixe, comme dans `conges`.
    pub ref_: String,
    pub label: String,
    pub description: String,
    pub fk_user_recruiter: i64,
    /// `date_planned` est nullable en base : `None` écrit NULL.
    pub date_planned: Option<String>,
    /// Nombre de postes prévus → colonne `qty`.
    pub nb_recruitment_planned: i32,
    /// Aucune colonne dans la table : renseigné, il est refusé.
    pub fk_country: Option<i64>,
    /// Société cliente de l'offre (`llx_societe`), facultative.
    pub fk_soc: Option<i64>,
}

/// Une candidature à créer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NouvelleCandidature {
    /// L'offre visée → colonne `fk_recruitmentjobposition`.
    pub fk_offre: i64,
    pub lastname: String,
    pub firstname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// Origine déclarée → `fk_recruitment_origin` (dictionnaire sans `entity`).
    pub fk_origin: Option<i64>,
    /// `date` nullable, format `AAAA-MM-JJ`.
    pub date_birth: Option<String>,
    /// Écrit dans `note_private` : c'est la note interne du recruteur.
    pub note: Option<String>,
}

/// Une offre relue, avec le libellé de sa société et le login de son recruteur.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Offre {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub label: String,
    /// Colonne `qty`, en `i32` comme elle.
    pub nb_postes: i32,
    pub fk_soc: Option<i64>,
    /// `llx_societe.nom`, si la société existe encore.
    pub societe: Option<String>,
    pub fk_user_recruiter: Option<i64>,
    /// `llx_user.login`, si le compte existe encore.
    pub recruteur: Option<String>,
    pub date_planned: Option<String>,
    pub description: Option<String>,
    pub statut: i16,
    pub date_creation: Option<String>,
}

/// Une candidature relue, avec la référence de son offre et le libellé de son
/// origine.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Candidature {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub fk_offre: Option<i64>,
    /// `llx_…jobposition.ref` de l'offre visée.
    pub offre_ref: Option<String>,
    pub lastname: Option<String>,
    pub firstname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub fk_origin: Option<i64>,
    /// `llx_c_recruitment_origin.label`.
    pub origine: Option<String>,
    pub date_birth: Option<String>,
    pub statut: i16,
    pub note_private: Option<String>,
    pub date_creation: Option<String>,
}

/// Ce qu'une création de candidature a réellement fait.
///
/// Rendue au lieu d'un simple `i64` pour que l'appelant **sache** que la
/// candidature est arrivée sur une offre qui n'était pas publiée : c'est le
/// signalement de la décision prise en tête de module. La même information est
/// écrite dans `note_private`, mais un appelant ne lit pas les notes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CandidatureCreee {
    pub rowid: i64,
    /// `true` si l'offre n'était pas au statut publié (1) au moment de la
    /// création : brouillon, pourvue ou clôturée.
    pub offre_hors_publication: bool,
}

/// Postes prévus et postes déclarés pourvus, sur une offre.
///
/// `prevus` vient de `qty` (`int(11)`), `pourvus` d'un `COUNT(*)` : d'où les
/// deux types. Les deux chiffres ne se déduisent pas l'un de l'autre.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BilanPostes {
    pub prevus: i32,
    pub pourvus: i64,
}

// ---------------------------------------------------------------------------
// Ce que le module refuse sans toucher la base
// ---------------------------------------------------------------------------

/// Le refus d'un statut de candidature, avec sa raison.
///
/// Ce type existe pour que la table des transitions reste **pure** : elle ne
/// touche ni la base ni l'horloge, et s'explique toute seule dans un test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurStatut {
    /// Valeur qui n'existe dans aucun statut de Dolibarr.
    Inconnu(i16),
    /// Le statut 2, commenté dans la source : jamais écrit.
    RetireParDolibarr(i16),
    /// Transition absente de la séquence (état terminal compris).
    HorsSequence { depuis: i16, vers: i16 },
}

impl fmt::Display for ErreurStatut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErreurStatut::Inconnu(valeur) => {
                write!(f, "statut de candidature inconnu : {valeur}")
            }
            ErreurStatut::RetireParDolibarr(valeur) => write!(
                f,
                "le statut {valeur} n'existe pas : Dolibarr l'a commenté dans \
                 recruitmentcandidature.class.php, on ne l'invente pas"
            ),
            ErreurStatut::HorsSequence { depuis, vers } => write!(
                f,
                "transition refusée : « {} » ({depuis}) → « {} » ({vers}) n'est pas \
                 dans la séquence du module",
                libelle_statut_candidature(*depuis),
                libelle_statut_candidature(*vers)
            ),
        }
    }
}

impl Error for ErreurStatut {}

/// La transition de statut de candidature est-elle permise ? **Fonction pure**,
/// donc vérifiable sans base.
///
/// La table vient de la source, transition par transition :
///
/// | depuis | vers | d'où ça sort |
/// |---|---|---|
/// | 0 brouillon | 1 reçue | `validate()`, qui refuse seulement si déjà 1 |
/// | 1 reçue | 3 contrat proposé | l'entretien a eu lieu |
/// | 1 reçue | 8 refusée | `setStatusCommon` : aucun refus direct n'est interdit |
/// | 1 reçue | 9 annulée | idem |
/// | 3 contrat proposé | 5 retenue | le contrat est signé |
/// | 3 contrat proposé | 8, 9 | le candidat ou l'employeur se retire |
/// | 8, 9, 6 | 1 reçue | `reopen()` : ces trois états seulement sont rouverts |
///
/// On ne saute pas d'étape : 1 → 5 est refusé, parce qu'une embauche sans
/// proposition de contrat n'existe pas, et 5 est un état terminal — une
/// candidature retenue ne « se refuse » pas, elle se rouvre.
pub fn transition_permise(depuis: i16, vers: i16) -> bool {
    matches!(
        (depuis, vers),
        (CANDIDATURE_BROUILLON, CANDIDATURE_RECUE)
            | (CANDIDATURE_RECUE, CANDIDATURE_CONTRAT_PROPOSE)
            | (CANDIDATURE_RECUE, CANDIDATURE_REFUSEE)
            | (CANDIDATURE_RECUE, CANDIDATURE_ANNULEE)
            | (CANDIDATURE_CONTRAT_PROPOSE, CANDIDATURE_RETENUE)
            | (CANDIDATURE_CONTRAT_PROPOSE, CANDIDATURE_REFUSEE)
            | (CANDIDATURE_CONTRAT_PROPOSE, CANDIDATURE_ANNULEE)
            | (CANDIDATURE_REFUSEE, CANDIDATURE_RECUE)
            | (CANDIDATURE_ANNULEE, CANDIDATURE_RECUE)
            | (CANDIDATURE_CONTRAT_REFUSE, CANDIDATURE_RECUE)
    )
}

/// Le libellé français d'un statut d'offre, pour les messages.
pub fn libelle_statut_offre(statut: i16) -> &'static str {
    match statut {
        STATUT_BROUILLON => "brouillon",
        STATUT_PUBLIEE => "publiée",
        STATUT_POURVUE => "pourvue",
        STATUT_CLOTUREE => "clôturée",
        _ => "statut inconnu",
    }
}

/// Le libellé français d'un statut de candidature, pour les messages.
pub fn libelle_statut_candidature(statut: i16) -> &'static str {
    match statut {
        CANDIDATURE_BROUILLON => "brouillon",
        CANDIDATURE_RECUE => "reçue (entretien à faire)",
        CANDIDATURE_ENTRETIEN_PLANIFIE_RETIRE => "entretien planifié (statut retiré)",
        CANDIDATURE_CONTRAT_PROPOSE => "contrat proposé",
        CANDIDATURE_RETENUE => "retenue",
        CANDIDATURE_CONTRAT_REFUSE => "contrat refusé",
        CANDIDATURE_REFUSEE => "refusée",
        CANDIDATURE_ANNULEE => "annulée",
        _ => "statut inconnu",
    }
}

// ---------------------------------------------------------------------------
// Dates : validation pure, sans base ni horloge
// ---------------------------------------------------------------------------

/// Décalage de l'époque Unix dans le calendrier de Howard Hinnant.
const EPOQUE: i64 = 719_468;

/// Jours écoulés depuis le 1er janvier 1970 (`days_from_civil` de Hinnant).
///
/// Même conversion que `conges::jours_ouvres`, privée là-bas : elle seule sait
/// combien de jours compte un mois sans table ni oubli d'année bissextile. Ce
/// module n'a besoin que de la longueur du mois, pas du jour de la semaine.
fn jours_depuis_epoque(annee: i64, mois: i64, jour: i64) -> i64 {
    // L'année est alignée sur mars : le 29 février tombe alors en fin d'année et
    // ne perturbe plus les divisions.
    let annee = if mois <= 2 { annee - 1 } else { annee };
    let ere = if annee >= 0 { annee } else { annee - 399 } / 400;
    let annee_ere = annee - ere * 400;
    let mois_mars = (mois + 9) % 12;
    let jour_annee = (153 * mois_mars + 2) / 5 + jour - 1;
    let jour_ere = annee_ere * 365 + annee_ere / 4 - annee_ere / 100 + jour_annee;
    ere * 146_097 + jour_ere - EPOQUE
}

/// Nombre de jours d'un mois, par différence avec le premier jour du mois
/// suivant : aucune table, donc aucun oubli d'année bissextile.
fn duree_du_mois(annee: i64, mois: i64) -> i64 {
    let (annee_suivante, mois_suivant) = if mois == 12 {
        (annee + 1, 1)
    } else {
        (annee, mois + 1)
    };
    jours_depuis_epoque(annee_suivante, mois_suivant, 1) - jours_depuis_epoque(annee, mois, 1)
}

/// Lit un nombre sur des octets déjà vérifiés chiffres.
fn nombre(octets: &[u8]) -> u32 {
    octets
        .iter()
        .fold(0u32, |valeur, &octet| valeur * 10 + u32::from(octet - b'0'))
}

/// Une date `AAAA-MM-JJ` qui existe réellement au calendrier.
///
/// `2026-02-30`, `2026-1-1` et `02/11/2026` sont refusés : Dolibarr stocke ses
/// `date` dans ce format exact, et une date qui n'existe pas n'a rien à faire
/// dans une offre ni dans une date de naissance.
fn date_valide(valeur: &str) -> bool {
    let texte = valeur.trim();
    let octets = texte.as_bytes();
    let forme = octets.len() == 10
        && octets[4] == b'-'
        && octets[7] == b'-'
        && octets[..4].iter().all(u8::is_ascii_digit)
        && octets[5..7].iter().all(u8::is_ascii_digit)
        && octets[8..].iter().all(u8::is_ascii_digit);
    if !forme {
        return false;
    }
    let annee = i64::from(nombre(&octets[..4]));
    let mois = i64::from(nombre(&octets[5..7]));
    let jour = i64::from(nombre(&octets[8..]));
    if !(1..=9999).contains(&annee) || !(1..=12).contains(&mois) {
        return false;
    }
    jour >= 1 && jour <= duree_du_mois(annee, mois)
}

/// Une adresse email plausible : de quoi refuser ce qui n'en est manifestement
/// pas une.
///
/// On ne valide pas une adresse (la RFC 5322 est un piège), on refuse les trois
/// fautes qui font perdre un contact : pas d'arobase, rien avant, rien après,
/// ou un blanc au milieu. Une adresse refusée ici serait une adresse qu'aucun
/// envoi n'atteindrait.
fn email_plausible(valeur: &str) -> bool {
    let texte = valeur.trim();
    if texte.is_empty() || texte.len() > EMAIL_MAX || texte.contains(char::is_whitespace) {
        return false;
    }
    match texte.split_once('@') {
        Some((avant, apres)) => !avant.is_empty() && !apres.is_empty() && !apres.contains('@'),
        None => false,
    }
}

/// Contrôles d'une offre qui ne demandent **pas** la base.
///
/// Séparés de [`creer_offre`] pour deux raisons : l'écran de saisie peut
/// valider sans connexion, et un test peut vérifier les refus sans MariaDB.
///
/// Sont refusés : référence vide ou trop longue, intitulé vide ou trop long,
/// recruteur absent, nombre de postes négatif, date prévue mal formée, et
/// `fk_country` **renseigné** (la colonne n'existe pas).
pub fn valider_offre(new: &NouvelleOffre) -> Result<(), Box<dyn Error>> {
    let reference = new.ref_.trim();
    if reference.is_empty() {
        return Err(
            "la référence de l'offre est obligatoire : sans elle, l'offre \
                    n'est identifiable ni dans la liste ni dans un lien"
                .into(),
        );
    }
    if reference.len() > REF_MAX {
        return Err(format!(
            "la référence de l'offre dépasse {REF_MAX} octets ({})",
            reference.len()
        )
        .into());
    }

    let libelle = new.label.trim();
    if libelle.is_empty() {
        return Err("l'intitulé de l'offre est obligatoire : `label` est NOT NULL".into());
    }
    if libelle.len() > LABEL_MAX {
        return Err(format!(
            "l'intitulé de l'offre dépasse {LABEL_MAX} octets ({})",
            libelle.len()
        )
        .into());
    }

    // `fk_user_recruiter` est nullable en base, mais une offre sans recruteur
    // n'a personne à qui présenter les candidatures : le champ est exigé ici.
    if new.fk_user_recruiter <= 0 {
        return Err("le recruteur (fk_user_recruiter) est obligatoire".into());
    }

    if new.nb_recruitment_planned < 0 {
        return Err(format!(
            "le nombre de postes ne peut pas être négatif ({})",
            new.nb_recruitment_planned
        )
        .into());
    }

    // Un champ vide n'est pas une date invalide : `date_planned` est nullable,
    // et seule une valeur renseignée doit être une date.
    let date_prevue = date_renseignee(new.date_planned.as_deref()).unwrap_or("");
    if !date_prevue.is_empty() && !date_valide(date_prevue) {
        return Err(format!(
            "date prévue invalide : {date_prevue} — une date s'écrit AAAA-MM-JJ et doit \
             exister au calendrier"
        )
        .into());
    }

    // La colonne n'existe pas. Refuser vaut mieux que perdre la valeur en
    // silence : l'appelant croirait avoir enregistré un pays.
    if new.fk_country.is_some() {
        return Err("fk_country n'a pas de colonne : \
                    `llx_recruitment_recruitmentjobposition` ne porte aucun pays. \
                    Le pays d'une offre vit sur la société (`llx_societe.fk_pays`), \
                    pas sur l'offre"
            .into());
    }
    Ok(())
}

/// Contrôles d'une candidature qui ne demandent **pas** la base.
///
/// L'existence de l'offre et celle de l'origine demandent une connexion : elles
/// sont faites dans [`creer_candidature`]. Une date de naissance dans le futur
/// est refusée là-bas aussi, la « date du jour » étant celle du serveur de base
/// et non celle du poste qui exécute le code.
pub fn valider_candidature(new: &NouvelleCandidature) -> Result<(), Box<dyn Error>> {
    if new.fk_offre <= 0 {
        return Err("une candidature doit viser une offre (fk_offre)".into());
    }

    let nom = new.lastname.trim();
    if nom.is_empty() {
        return Err("le nom du candidat est obligatoire".into());
    }
    if nom.len() > NOM_MAX {
        return Err(format!(
            "le nom du candidat dépasse {NOM_MAX} octets ({})",
            nom.len()
        )
        .into());
    }

    // Les champs facultatifs sont ramenés à la chaîne vide quand ils sont
    // absents : une chaîne vide passe tous les plafonds, donc les contrôles
    // ci-dessous ne s'appliquent de fait qu'aux valeurs réellement fournies.
    let prenom = texte_renseigne(new.firstname.as_deref()).unwrap_or("");
    if prenom.len() > PRENOM_MAX {
        return Err(format!(
            "le prénom du candidat dépasse {PRENOM_MAX} octets ({})",
            prenom.len()
        )
        .into());
    }

    let email = texte_renseigne(new.email.as_deref()).unwrap_or("");
    if !email.is_empty() && !email_plausible(email) {
        return Err(format!(
            "l'adresse email « {email} » n'en est pas une : il faut une arobase, \
             quelque chose avant, quelque chose après, pas de blanc, et au plus \
             {EMAIL_MAX} octets"
        )
        .into());
    }

    let telephone = texte_renseigne(new.phone.as_deref()).unwrap_or("");
    if telephone.len() > PHONE_MAX {
        return Err(format!(
            "le téléphone dépasse {PHONE_MAX} octets ({})",
            telephone.len()
        )
        .into());
    }

    let naissance = texte_renseigne(new.date_birth.as_deref()).unwrap_or("");
    if !naissance.is_empty() && !date_valide(naissance) {
        return Err(format!(
            "date de naissance invalide : {naissance} — une date s'écrit AAAA-MM-JJ \
             et doit exister au calendrier"
        )
        .into());
    }

    // `Some(0)` n'est pas « pas d'origine », c'est un identifiant faux : Dolibarr
    // écrirait une ligne de dictionnaire qui n'existe pas.
    verifier_valeur_origine(new.fk_origin)
}

/// `Some(0)` (ou négatif) n'est pas « pas d'origine » : c'est un identifiant
/// faux, que `fk_recruitment_origin` accepterait sans broncher.
fn verifier_valeur_origine(valeur: Option<i64>) -> Result<(), Box<dyn Error>> {
    if let Some(id) = valeur.filter(|identifiant| *identifiant <= 0) {
        return Err(format!("fk_origin vaut {id} : soit une origine réelle, soit None").into());
    }
    Ok(())
}

/// Le texte s'il est renseigné, `None` s'il est vide ou blanc.
fn texte_renseigne(valeur: Option<&str>) -> Option<&str> {
    valeur.map(str::trim).filter(|texte| !texte.is_empty())
}

/// La date si elle est renseignée, `None` si le champ est vide ou blanc.
///
/// Un champ vide n'est pas une date invalide : c'est un champ laissé vide, et
/// `date_planned` est nullable.
fn date_renseignee(valeur: Option<&str>) -> Option<&str> {
    texte_renseigne(valeur)
}

// ---------------------------------------------------------------------------
// Vérifications qui demandent la base
// ---------------------------------------------------------------------------

/// Le recruteur existe-t-il dans l'entité ?
///
/// `llx_…jobposition` n'a **aucune clé étrangère** : un identifiant inventé
/// passerait sans bruit, et l'offre n'apparaîtrait dans l'écran de personne.
/// On ne vérifie pas que le compte est actif : un recruteur désactivé garde ses
/// offres en cours, et Dolibarr l'accepte.
fn verifier_recruteur(pool: &Pool, entity: i64, id: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => id },
    )?;
    if existe.is_none() {
        return Err(format!("le recruteur {id} n'existe pas dans l'entité {entity}").into());
    }
    Ok(())
}

/// La société existe-t-elle dans l'entité ?
fn verifier_societe(pool: &Pool, entity: i64, id: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => id },
    )?;
    if existe.is_none() {
        return Err(format!("la société {id} n'existe pas dans l'entité {entity}").into());
    }
    Ok(())
}

/// L'origine de candidature existe-t-elle, et est-elle active ?
///
/// `llx_c_recruitment_origin` **n'a pas de colonne `entity`** (mesuré : `rowid`,
/// `code`, `label`, `active`, et rien d'autre) : c'est un dictionnaire partagé,
/// comme la plupart de ceux de Dolibarr. Il est vide sur l'instance de
/// référence, ce qui rend le contrôle utile plutôt que théorique.
fn verifier_origine(pool: &Pool, id: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let actif: Option<Option<i64>> = conn.exec_first(
        "SELECT active FROM llx_c_recruitment_origin WHERE rowid = :rowid",
        params! { "rowid" => id },
    )?;
    let Some(actif) = actif else {
        return Err(format!(
            "l'origine de candidature {id} n'existe pas dans llx_c_recruitment_origin"
        )
        .into());
    };
    // `active` est NOT NULL avec 1 pour défaut : un NULL vaudrait actif.
    if actif.unwrap_or(1) == 0 {
        return Err(format!("l'origine de candidature {id} est désactivée").into());
    }
    Ok(())
}

/// Le statut d'une offre, vue par l'entité qui la demande.
fn statut_offre(pool: &Pool, entity: i64, rowid: i64) -> Result<i16, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_recruitment_recruitmentjobposition \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        Some(valeur) => Ok(valeur as i16),
        None => Err(format!("offre {rowid} introuvable dans l'entité {entity}").into()),
    }
}

/// La date du jour telle que la voit le serveur de base.
///
/// On ne prend pas celle du poste : un décalage de fuseau ferait refuser une
/// date de naissance parfaitement valide (ou accepter un futur).
fn aujourdhui_base(pool: &Pool) -> Result<String, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // `query_first` et non `exec_first` : la requête ne prend aucun paramètre,
    // et le noyau lit déjà la date du serveur de cette façon (`lib::aujourdhui`).
    let jour: Option<String> = conn.query_first("SELECT DATE_FORMAT(CURDATE(), '%Y-%m-%d')")?;
    Ok(jour.unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Lecture
// ---------------------------------------------------------------------------

const SELECT_OFFRE: &str = "j.rowid, j.entity, j.ref, j.label, j.qty, j.fk_soc, s.nom, \
     j.fk_user_recruiter, u.login, DATE_FORMAT(j.date_planned, '%Y-%m-%d'), j.description, \
     j.status, DATE_FORMAT(j.date_creation, '%Y-%m-%d %H:%i:%s')";

const SELECT_CANDIDATURE: &str = "c.rowid, c.entity, c.ref, c.fk_recruitmentjobposition, \
     j.ref, c.lastname, c.firstname, c.email, c.phone, c.fk_recruitment_origin, o.label, \
     DATE_FORMAT(c.date_birth, '%Y-%m-%d'), c.status, c.note_private, \
     DATE_FORMAT(c.date_creation, '%Y-%m-%d %H:%i:%s')";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|valeur| valeur.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|valeur| valeur.ok())
}

fn map_offre(row: &Row) -> Offre {
    Offre {
        rowid: opt_i(row, 0).unwrap_or(0),
        entity: opt_i(row, 1).unwrap_or(1),
        ref_: opt_s(row, 2).unwrap_or_default(),
        label: opt_s(row, 3).unwrap_or_default(),
        // `qty` est un `int(11)` : on le relit en i64 (types entiers confondus
        // par le pilote) puis on le ramène à sa largeur réelle.
        nb_postes: opt_i(row, 4).unwrap_or(0) as i32,
        fk_soc: opt_i(row, 5),
        societe: opt_s(row, 6),
        fk_user_recruiter: opt_i(row, 7),
        recruteur: opt_s(row, 8),
        date_planned: opt_s(row, 9),
        description: opt_s(row, 10),
        statut: opt_i(row, 11).unwrap_or(0) as i16,
        date_creation: opt_s(row, 12),
    }
}

fn map_candidature(row: &Row) -> Candidature {
    Candidature {
        rowid: opt_i(row, 0).unwrap_or(0),
        entity: opt_i(row, 1).unwrap_or(1),
        ref_: opt_s(row, 2).unwrap_or_default(),
        fk_offre: opt_i(row, 3),
        offre_ref: opt_s(row, 4),
        lastname: opt_s(row, 5),
        firstname: opt_s(row, 6),
        email: opt_s(row, 7),
        phone: opt_s(row, 8),
        fk_origin: opt_i(row, 9),
        origine: opt_s(row, 10),
        date_birth: opt_s(row, 11),
        statut: opt_i(row, 12).unwrap_or(0) as i16,
        note_private: opt_s(row, 13),
        date_creation: opt_s(row, 14),
    }
}

/// Les offres d'une entité, de la plus récente à la plus ancienne.
///
/// Rend un `Result` comme toutes les listes du noyau : une panne MySQL doit
/// remonter, pas se traduire par une liste vide qu'on croirait complète.
pub fn offres(pool: &Pool, entity: i64, limite: u32) -> Result<Vec<Offre>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_OFFRE} FROM llx_recruitment_recruitmentjobposition j \
         LEFT JOIN llx_societe s ON s.rowid = j.fk_soc \
         LEFT JOIN llx_user u ON u.rowid = j.fk_user_recruiter \
         WHERE j.entity = :entity \
         ORDER BY j.date_creation DESC, j.rowid DESC LIMIT :limite"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "limite" => limite })?;
    Ok(rows.iter().map(map_offre).collect())
}

/// Les candidatures d'une offre, de la plus récente à la plus ancienne.
///
/// Le filtre porte sur les **deux** entités : celle de la candidature (sa propre
/// colonne) et, par construction, celle de l'offre — [`creer_candidature`]
/// refuse une offre d'une autre entité. Le `fk_offre` est donc toujours dans
/// l'entité demandée.
///
/// Aucune limite : une offre a peu de candidatures, et toutes comptent pour un
/// recrutement.
pub fn candidatures(
    pool: &Pool,
    entity: i64,
    fk_offre: i64,
) -> Result<Vec<Candidature>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_CANDIDATURE} FROM llx_recruitment_recruitmentcandidature c \
         LEFT JOIN llx_recruitment_recruitmentjobposition j \
                ON j.rowid = c.fk_recruitmentjobposition \
         LEFT JOIN llx_c_recruitment_origin o ON o.rowid = c.fk_recruitment_origin \
         WHERE c.entity = :entity AND c.fk_recruitmentjobposition = :offre \
         ORDER BY c.date_creation DESC, c.rowid DESC"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "offre" => fk_offre })?;
    Ok(rows.iter().map(map_candidature).collect())
}

/// Combien de candidatures une offre a reçues.
///
/// **Tous statuts comptés**, y compris les refusées et les annulées : c'est la
/// fréquentation de l'offre qu'on mesure, pas le vivier restant. Pour le vivier,
/// filtrer sur `statut` en lisant [`candidatures`].
///
/// Une offre qui n'existe pas rend **0** et non une erreur : c'est un compte,
/// et un compte de rien vaut zéro. Le contrôle d'existence appartient à
/// [`creer_candidature`], qui en a besoin pour écrire.
pub fn compter_candidatures(
    pool: &Pool,
    entity: i64,
    fk_offre: i64,
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let total: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_recruitment_recruitmentcandidature \
         WHERE entity = :entity AND fk_recruitmentjobposition = :offre",
        params! { "entity" => entity, "offre" => fk_offre },
    )?;
    Ok(total.unwrap_or(0))
}

/// Postes prévus (`qty`) et déclarés pourvus (candidatures au statut 5).
///
/// Voir l'entête du module : `prevus` est saisi, `pourvus` est déclaré, et rien
/// dans le schéma ne relie les deux. Les rendre ensemble évite de prendre l'un
/// pour l'autre.
///
/// # Erreurs
///
/// Offre inexistante dans l'entité : sans elle, il n'y a pas de `qty` à lire.
pub fn bilan_postes(
    pool: &Pool,
    entity: i64,
    fk_offre: i64,
) -> Result<BilanPostes, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // Trois noms distincts pour trois trous : `:entity` réutilisé deux fois
    // demanderait au pilote de le répéter, ce qu'il ne fait pas.
    let ligne: Option<(i64, i64)> = conn.exec_first(
        "SELECT j.qty, \
                (SELECT COUNT(*) FROM llx_recruitment_recruitmentcandidature c \
                  WHERE c.entity = :entite_candidature \
                    AND c.fk_recruitmentjobposition = j.rowid \
                    AND c.status = :retenue) \
         FROM llx_recruitment_recruitmentjobposition j \
         WHERE j.entity = :entity AND j.rowid = :offre",
        params! {
            "entity" => entity,
            "entite_candidature" => entity,
            "offre" => fk_offre,
            "retenue" => CANDIDATURE_RETENUE,
        },
    )?;
    match ligne {
        Some((prevus, pourvus)) => Ok(BilanPostes {
            prevus: prevus as i32,
            pourvus,
        }),
        None => Err(format!("offre {fk_offre} introuvable dans l'entité {entity}").into()),
    }
}

// ---------------------------------------------------------------------------
// Écriture
// ---------------------------------------------------------------------------

/// Crée une offre au statut **brouillon** (0) et renvoie son `rowid`.
///
/// Le statut de départ est celui de `create()` dans la source :
/// `$object->status = self::STATUS_DRAFT`. Publier est un acte distinct, et
/// c'est voulu : une offre à moitié rédigée ne doit pas recevoir de
/// candidatures.
///
/// `fk_user_creat` (NOT NULL) reçoit le recruteur : la structure ne porte pas
/// d'auteur distinct, et c'est lui qui engage l'offre.
///
/// # Erreurs
///
/// Tout ce que refuse [`valider_offre`], plus : recruteur inexistant dans
/// l'entité, société inexistante, et toute erreur MySQL remontée telle quelle.
pub fn creer_offre(pool: &Pool, entity: i64, new: &NouvelleOffre) -> Result<i64, Box<dyn Error>> {
    valider_offre(new)?;
    verifier_recruteur(pool, entity, new.fk_user_recruiter)?;
    if let Some(societe) = new.fk_soc {
        verifier_societe(pool, entity, societe)?;
    }

    let date_prevue = date_renseignee(new.date_planned.as_deref());
    let description = texte_renseigne(Some(new.description.as_str()));

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_recruitment_recruitmentjobposition \
         (entity, ref, label, qty, fk_soc, fk_user_recruiter, date_planned, description, \
          status, date_creation, fk_user_creat) \
         VALUES (:entity, :reference, :libelle, :postes, :societe, :recruteur, :date_prevue, \
          :description, :statut, NOW(), :createur)",
        params! {
            "entity" => entity,
            "reference" => new.ref_.trim(),
            "libelle" => new.label.trim(),
            "postes" => new.nb_recruitment_planned,
            "societe" => new.fk_soc,
            "recruteur" => new.fk_user_recruiter,
            "date_prevue" => date_prevue,
            "description" => description,
            "statut" => STATUT_BROUILLON,
            "createur" => new.fk_user_recruiter,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Crée une candidature au statut **reçue** (1) et dit si l'offre était publiée.
///
/// La référence est numérotée par [`divers::suivant_ref`] (`CA0001`, `CA0002`…),
/// par entité. `email_msgid` est laissé à **NULL** et non à la chaîne vide : la
/// colonne porte un index **unique**, et deux candidatures sans message
/// d'origine écriraient deux fois `''`. Deux NULL, eux, cohabitent.
///
/// # Une offre close reçoit quand même la candidature
///
/// Voir l'entête du module : on accepte, on écrit, on signale (mention dans
/// `note_private` + [`CandidatureCreee::offre_hors_publication`]). Refuser
/// ferait perdre une personne réelle.
///
/// # Erreurs
///
/// Tout ce que refuse [`valider_candidature`], plus : offre inexistante dans
/// l'entité, origine inexistante ou désactivée, date de naissance dans le futur,
/// et toute erreur MySQL remontée telle quelle.
pub fn creer_candidature(
    pool: &Pool,
    entity: i64,
    new: &NouvelleCandidature,
) -> Result<CandidatureCreee, Box<dyn Error>> {
    valider_candidature(new)?;

    // Existence de l'offre **dans l'entité** : c'est ce contrôle qui tient lieu
    // de clé étrangère, et c'est lui qui cloisonne la candidature.
    let statut_de_l_offre = statut_offre(pool, entity, new.fk_offre)?;
    if let Some(origine) = new.fk_origin {
        verifier_origine(pool, origine)?;
    }

    // La date de naissance ne peut pas être dans le futur : la référence est la
    // date du serveur, jamais celle du poste. `AAAA-MM-JJ` se compare comme du
    // texte dans l'ordre chronologique, d'où la comparaison directe. La date du
    // serveur n'est demandée que si une naissance est renseignée.
    let naissance = texte_renseigne(new.date_birth.as_deref()).unwrap_or("");
    let aujourdhui = if naissance.is_empty() {
        String::new()
    } else {
        aujourdhui_base(pool)?
    };
    if !naissance.is_empty() && !aujourdhui.is_empty() && naissance > aujourdhui.as_str() {
        return Err(format!(
            "la date de naissance ({naissance}) est dans le futur : le serveur de base \
             est au {aujourdhui}"
        )
        .into());
    }

    // Le signalement : une mention dans la note interne, et un drapeau rendu.
    let hors_publication = statut_de_l_offre != STATUT_PUBLIEE;
    let note = texte_renseigne(new.note.as_deref()).unwrap_or("");
    let note = if hors_publication {
        let mention = format!(
            "Candidature reçue sur une offre non publiée (statut {statut_de_l_offre} : {}).",
            libelle_statut_offre(statut_de_l_offre)
        );
        if note.is_empty() {
            mention
        } else {
            format!("{note}\n{mention}")
        }
    } else {
        note.to_string()
    };
    let note_sql = if note.is_empty() { None } else { Some(note) };

    // Une date absente s'écrit **NULL**, jamais `''` : la colonne est un `date`,
    // et une chaîne vide y est une valeur invalide (refusée en mode strict).
    let naissance_sql = if naissance.is_empty() {
        None
    } else {
        Some(naissance)
    };

    let reference = divers::suivant_ref(
        pool,
        "llx_recruitment_recruitmentcandidature",
        "ref",
        PREFIXE_CANDIDATURE,
        entity,
    )?;

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_recruitment_recruitmentcandidature \
         (entity, ref, fk_recruitmentjobposition, note_private, date_creation, status, \
          firstname, lastname, email, phone, date_birth, fk_recruitment_origin) \
         VALUES (:entity, :reference, :offre, :note, NOW(), :statut, :prenom, :nom, \
          :email, :telephone, :naissance, :origine)",
        params! {
            "entity" => entity,
            "reference" => reference.as_str(),
            "offre" => new.fk_offre,
            "note" => note_sql.as_deref(),
            "statut" => CANDIDATURE_RECUE,
            "prenom" => texte_renseigne(new.firstname.as_deref()),
            "nom" => new.lastname.trim(),
            "email" => texte_renseigne(new.email.as_deref()),
            "telephone" => texte_renseigne(new.phone.as_deref()),
            "naissance" => naissance_sql,
            "origine" => new.fk_origin,
        },
    )?;
    Ok(CandidatureCreee {
        rowid: conn.last_insert_id() as i64,
        offre_hors_publication: hors_publication,
    })
}

/// Publie une offre : **brouillon (0) → publiée (1)**.
///
/// Une seule transition est acceptée. Republier est refusé, comme `validate()`
/// le fait (`if ($this->status == self::STATUS_VALIDATED) return 0;`).
///
/// La référence n'est **pas** renumérotée : Dolibarr le fait à la validation
/// quand la référence commence par `(PROV`, or celle-ci est imposée à la
/// création par l'appelant. La changer ici casserait les liens déjà écrits.
///
/// # Erreurs
///
/// Offre inexistante dans l'entité, offre déjà publiée, offre pourvue (3) ou
/// clôturée (9) : la réouverture (`reopen()`) n'est pas exposée par ce module.
pub fn publier_offre(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let actuel = statut_offre(pool, entity, rowid)?;
    match actuel {
        STATUT_BROUILLON => {}
        STATUT_PUBLIEE => {
            return Err("cette offre est déjà publiée : republier ne change rien, \
                        et Dolibarr refuse aussi (validate() rend 0)"
                .into())
        }
        STATUT_POURVUE => {
            return Err("cette offre est pourvue : elle ne se republie pas, \
                        il faut une nouvelle offre"
                .into())
        }
        STATUT_CLOTUREE => {
            return Err(
                "cette offre est clôturée : sa réouverture (reopen de Dolibarr) \
                        n'est pas exposée par ce module"
                    .into(),
            )
        }
        autre => {
            return Err(format!("statut d'offre inattendu ({autre}) : rien n'est publié").into())
        }
    }

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_recruitment_recruitmentjobposition SET status = :statut \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_PUBLIEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Clôt une offre : **publiée (1) → clôturée (9)**.
///
/// Le statut 9 est celui de `cancel()`, dont le déclencheur s'appelle
/// `RECRUITMENTJOBPOSITION_CLOSE` : dans Dolibarr, clore une offre **est**
/// l'annuler. Le statut 3 (pourvue) n'est pas écrit ici — voir [`bilan_postes`]
/// pour la raison : rien dans le schéma ne permet de savoir qu'un poste est
/// réellement pourvu.
///
/// La garde est celle de la source, mot pour mot : `cancel()` rend 0 si le
/// statut n'est pas `STATUS_VALIDATED`. Une offre en brouillon ne se clôt donc
/// pas — elle n'a jamais été ouverte.
///
/// # Erreurs
///
/// Offre inexistante dans l'entité, ou offre qui n'est pas publiée.
pub fn clore_offre(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let actuel = statut_offre(pool, entity, rowid)?;
    if actuel != STATUT_PUBLIEE {
        return Err(format!(
            "seule une offre publiée se clôture : celle-ci est « {} » ({actuel}) — \
             Dolibarr applique la même règle dans cancel()",
            libelle_statut_offre(actuel)
        )
        .into());
    }

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_recruitment_recruitmentjobposition SET status = :statut \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_CLOTUREE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Fait avancer une candidature dans la séquence.
///
/// Les transitions autorisées sont dans [`transition_permise`] ; la valeur 2 est
/// refusée par [`changer_statut_candidature`] parce qu'elle **n'existe pas**
/// (commentée dans la source), et la valeur 6 (contrat refusé) n'est pas une
/// étape de ce module : elle se pose depuis la fiche du contrat. Le retour au
/// brouillon (0) est refusé aussi : `setDraft` existe chez Dolibarr, mais une
/// candidature reçue n'a pas à redevenir une note de travail.
///
/// # Erreurs
///
/// Candidature inexistante dans l'entité, statut inconnu ou retiré, transition
/// hors séquence (dont « déjà à ce statut »), et toute erreur MySQL.
pub fn changer_statut_candidature(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    nouveau: i16,
) -> Result<(), Box<dyn Error>> {
    match nouveau {
        CANDIDATURE_ENTRETIEN_PLANIFIE_RETIRE => {
            return Err(ErreurStatut::RetireParDolibarr(nouveau).into())
        }
        CANDIDATURE_BROUILLON => {
            return Err(
                "le retour au brouillon n'est pas exposé : Dolibarr le nomme \
                        setDraft, et une candidature reçue ne redevient pas une note \
                        de travail"
                    .into(),
            )
        }
        CANDIDATURE_CONTRAT_REFUSE => {
            return Err(
                "le statut 6 (contrat refusé) se pose depuis la fiche du contrat \
                        dans Dolibarr : ce n'est pas une étape de ce module, même s'il \
                        est relu comme un état possible"
                    .into(),
            )
        }
        CANDIDATURE_RECUE
        | CANDIDATURE_CONTRAT_PROPOSE
        | CANDIDATURE_RETENUE
        | CANDIDATURE_REFUSEE
        | CANDIDATURE_ANNULEE => {}
        autre => return Err(ErreurStatut::Inconnu(autre).into()),
    }

    let mut conn = pool.get_conn()?;
    let actuel: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_recruitment_recruitmentcandidature \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(actuel) = actuel else {
        return Err(format!("candidature {rowid} introuvable dans l'entité {entity}").into());
    };
    let actuel = actuel as i16;

    if actuel == nouveau {
        return Err(format!(
            "la candidature {rowid} est déjà au statut « {} »",
            libelle_statut_candidature(nouveau)
        )
        .into());
    }
    if !transition_permise(actuel, nouveau) {
        return Err(ErreurStatut::HorsSequence {
            depuis: actuel,
            vers: nouveau,
        }
        .into());
    }

    conn.exec_drop(
        "UPDATE llx_recruitment_recruitmentcandidature SET status = :statut \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => nouveau, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}
