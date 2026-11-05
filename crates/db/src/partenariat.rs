//! Partenariats (`llx_partnership`, `llx_c_partnership_type`).
//!
//! Un partenariat, dans Dolibarr, est la trace d'une relation suivie avec un
//! tiers ou un adhérent : parrainage, mécénat, convention, adhésion d'entreprise.
//! La fiche porte le type de relation (un code du dictionnaire
//! `llx_c_partnership_type`), la période couverte, l'email de contact, et l'URL
//! du site du partenaire quand le contrat prévoit un lien retour — Dolibarr
//! vérifie périodiquement cette URL, d'où `url_to_check` et
//! `count_last_url_check_error`.
//!
//! **Aucun montant.** La table `llx_partnership` n'a pas de colonne de somme :
//! ce qu'un partenaire apporte en argent vit dans une facture, un don ou un
//! contrat, pas ici. Ce module n'en manipule donc aucun.
//!
//! # Les statuts
//!
//! Ils sont repris **tels quels** du code vendu de Dolibarr 24.0.1,
//! `htdocs/partnership/class/partnership.class.php`, lignes 69 à 73 :
//!
//! ```text
//! const STATUS_DRAFT     = 0;
//! const STATUS_VALIDATED = 1;   // Validate (no more draft)
//! const STATUS_APPROVED  = 2;   // Approved
//! const STATUS_REFUSED   = 3;   // Refused
//! const STATUS_CANCELED  = 9;
//! ```
//!
//! Le même fichier les redonne en clair pour l'affichage (ligne 128) :
//! `0 => Draft, 1 => Validated, 2 => Approved, 3 => Refused, 9 => Terminated`.
//!
//! ⚠️ **L'annulation vaut 9, pas 4.** Le chiffre 4 n'existe pas dans ce cycle :
//! une fiche au statut 4 serait une fiche que Dolibarr afficherait sans libellé
//! (`LibStatut()` indexe son tableau sur les cinq constantes ci-dessus). Ce
//! module écrit donc [`STATUT_ANNULE`] = 9 ; toute lecture qui attendrait 4 se
//! tromperait. « Terminated » est le mot de Dolibarr, « annulé » celui de ce
//! module : c'est le même état — le partenariat a existé puis a été clos.
//!
//! Le cycle est celui de Dolibarr, avec une nuance assumée sur l'annulation :
//!
//! | fonction | de | vers |
//! |---|---|---|
//! | [`soumettre`] | 0 brouillon | 1 validé |
//! | [`accepter`] | 1 validé | 2 accepté |
//! | [`refuser`] | 1 validé | 3 refusé |
//! | [`annuler`] | 1 validé **ou** 2 accepté | 9 annulé |
//!
//! `Partnership::cancel()` en PHP n'accepte que le statut 2
//! (`if ($this->status != self::STATUS_APPROVED) return 0;`). Ce module accepte
//! aussi le statut 1 : un partenariat validé qu'on renonce à instruire doit
//! pouvoir être clos, sinon la seule sortie serait de le refuser — ce qui
//! laisserait croire à un refus du partenaire alors qu'on a simplement renoncé.
//!
//! # Les index réels, et ce qu'ils imposent
//!
//! Relevés par `SHOW INDEX FROM llx_partnership` sur l'instance de référence :
//!
//! ```text
//! PRIMARY              (rowid)
//! uk_partnership_ref   (ref, entity)                        UNIQUE
//! uk_fk_type_fk_soc    (fk_type, fk_soc, date_partnership_start)   UNIQUE
//! uk_fk_type_fk_member (fk_type, fk_member, date_partnership_start) UNIQUE
//! idx_partnership_entity (entity)                          non unique
//! ```
//!
//! **Oui, il y a un index unique sur `ref`** — `uk_partnership_ref`, portant sur
//! le couple `(ref, entity)`, pas sur `ref` seule : deux entités peuvent avoir
//! chacune sa référence. [`creer`] traduit le 1062 de cet index en phrase claire
//! plutôt que de laisser remonter un « Duplicate entry » brut.
//!
//! Les deux autres index uniques disent une règle métier qu'il faut connaître :
//! **pour un même tiers (ou un même adhérent), on ne peut pas avoir deux fois le
//! même type de partenariat à la même date de début.** C'est un doublon
//! d'historique, et la base le refuse. Comme `fk_soc` et `fk_member` sont
//! nullables, MySQL laisse passer plusieurs lignes où la colonne vaut `NULL` :
//! l'index n'est alors pas une garantie, d'où le contrôle fait plus haut — voir
//! la règle « un tiers ou un adhérent » ci-dessous.
//!
//! # Un tiers ou un adhérent, jamais aucun des deux
//!
//! `Partnership::create()` (ligne 269) commence par :
//!
//! ```text
//! if ($this->fk_soc <= 0 && $this->fk_member <= 0) {
//!     $this->errors[] = 'ErrorThirpdartyOrMemberidIsMandatory';
//!     return -1;
//! }
//! ```
//!
//! La même garde est reprise ici. Sans elle, une fiche sans tiers ni adhérent
//! échapperait aux deux index uniques (colonnes `NULL`) et pourrait être
//! dupliquée à l'infini sans que rien ne le signale.
//!
//! # La référence
//!
//! Elle est **fournie par l'appelant**, pas calculée ici. En PHP, `create()`
//! laisse la colonne prendre son défaut `(PROV)`, puis `validate()` appelle
//! `getNextNumRef()` (module `PARTNERSHIP_ADDON`, `mod_partnership_standard` par
//! défaut) pour poser le vrai numéro. Ce module n'ouvre pas de brouillon sans
//! référence : `llx_partnership.ref` est `NOT NULL` et unique par entité, et un
//! `(PROV)` recopié à la main ne serait pas un numéro. [`creer`] refuse donc une
//! référence vide, et refuse aussi plus de 128 octets — la colonne est un
//! `varchar(128)`, et contrôler en octets est plus strict qu'en caractères :
//! aucune écriture ne dépend alors du jeu de caractères de la connexion.
//!
//! # Le dictionnaire des types est vide sur l'instance de référence
//!
//! `SELECT rowid, code, label, active FROM llx_c_partnership_type` ne rend
//! **aucune ligne** sur la base de référence : Dolibarr ne sème pas de types de
//! partenariat par défaut, c'est à l'administrateur de les créer. Conséquence
//! directe pour les tests et pour l'écran : [`types`] peut légitimement rendre
//! une liste vide, et [`creer`] refuse alors tout type. La table, elle, existe :
//! `rowid` auto-incrémenté, `entity` (défaut 1), `code` `varchar(32)`, `label`
//! `varchar(128)`, `keyword`, `active` `tinyint` (défaut 1), avec
//! `uk_c_partnership_type (entity, code)`.
//!
//! # Ce que ce module ne fait pas
//!
//! - **Aucun lien retour n'est testé.** `url_to_check` et
//!   `count_last_url_check_error` sont lus à la relecture, mais ce module n'ouvre
//!   aucune connexion HTTP et ne touche pas au compteur : la vérification du lien
//!   est le travail de la tâche planifiée de Dolibarr, pas celui d'une couche base
//!   de données.
//! - **Aucune pièce jointe, aucune ligne de détail.** `llx_partnership` n'a pas
//!   de table de lignes (`fetchLines()` reste vide) ; il n'y a rien à porter.
//! - **Aucun déclencheur Dolibarr n'est exécuté.** Les triggers PHP
//!   (`PARTNERSHIP_CANCEL`, `PARTNERSHIP_VALIDATE`…) ne partent que depuis
//!   `setStatusCommon()`. Une écriture faite ici ne les déclenche pas.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool, Row};

/// Brouillon : la fiche est saisie, elle n'est pas encore instruite
/// (`STATUS_DRAFT`).
pub const STATUT_BROUILLON: i16 = 0;
/// Validé : la fiche est complète et soumise à l'instruction
/// (`STATUS_VALIDATED`).
pub const STATUT_VALIDE: i16 = 1;
/// Accepté : le partenariat court (`STATUS_APPROVED`).
pub const STATUT_ACCEPTE: i16 = 2;
/// Refusé : le partenariat n'aura pas lieu, un motif peut le dire
/// (`STATUS_REFUSED`).
pub const STATUT_REFUSE: i16 = 3;
/// Annulé : le partenariat a couru puis a été clos (`STATUS_CANCELED`, **9**).
pub const STATUT_ANNULE: i16 = 9;

/// Longueur maximale de la référence, en octets (`varchar(128)`).
pub const REF_MAX: usize = 128;
/// Longueur maximale de l'email de contact, en octets (`varchar(64)`).
pub const EMAIL_MAX: usize = 64;
/// Longueur maximale de l'URL à vérifier, en octets (`varchar(255)`).
pub const URL_MAX: usize = 255;
/// Longueur maximale du motif de refus ou d'annulation, en octets.
///
/// `reason_decline_or_cancel` est un `text` : 65 535 octets côté MySQL. On
/// s'arrête à 65 000 pour garder une marge, et la coupe tombe sur une frontière
/// de caractère (voir [`tronquer`]) — un accent coupé en deux ferait refuser
/// l'écriture par le serveur.
pub const MOTIF_MAX: usize = 65_000;

/// Ce qu'il faut pour créer un partenariat.
///
/// `ref_` porte un souligné final parce que `ref` est un mot réservé de Rust ;
/// c'est déjà le nom du champ dans [`crate::adherent::Adherent`].
///
/// `fk_soc` et `fk_member` sont facultatifs **séparément**, mais pas ensemble :
/// voir la documentation du module. `None` et `Some(0)` veulent tous deux dire
/// « pas de tiers » (Dolibarr range `0` là où la colonne accepterait `NULL`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NouveauPartenariat {
    pub ref_: String,
    /// `fk_type` : identifiant dans `llx_c_partnership_type`.
    pub fk_type: i64,
    /// Le tiers partenaire, si le partenariat est conclu avec une société.
    pub fk_soc: Option<i64>,
    /// L'adhérent partenaire, si le partenariat est conclu avec une personne.
    pub fk_member: Option<i64>,
    pub email_partnership: Option<String>,
    /// `date_partnership_start`, au format `AAAA-MM-JJ`. Obligatoire.
    pub date_partnership_start: String,
    /// `date_partnership_end`, au format `AAAA-MM-JJ`. `None` : sans terme.
    pub date_partnership_end: Option<String>,
    /// URL du site du partenaire, vérifiée par la tâche planifiée de Dolibarr.
    pub url_to_check: Option<String>,
    /// Auteur de la création. Dolibarr n'accepte pas de fiche anonyme.
    pub fk_user_creat: i64,
}

/// Un partenariat tel qu'il se lit en base, libellés joints compris.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Partenariat {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    /// `status` : l'une des cinq constantes `STATUT_*`.
    pub statut: i16,
    pub fk_type: i64,
    /// Libellé du type (`llx_c_partnership_type.label`), `None` si le type a
    /// été supprimé du dictionnaire après coup.
    pub type_libelle: Option<String>,
    pub fk_soc: Option<i64>,
    /// Raison sociale du tiers (`llx_societe.nom`), lue dans la même entité.
    pub tiers_nom: Option<String>,
    pub fk_member: Option<i64>,
    pub email: Option<String>,
    /// `date_partnership_start`, `AAAA-MM-JJ`.
    pub date_debut: Option<String>,
    /// `date_partnership_end`, `AAAA-MM-JJ`. `None` : sans terme.
    pub date_fin: Option<String>,
    /// `reason_decline_or_cancel` : le motif du refus ou de l'annulation.
    pub motif: Option<String>,
    pub url_to_check: Option<String>,
    /// Nombre d'échecs constatés à la dernière vérification du lien retour.
    pub count_last_url_check_error: i64,
    pub fk_user_creat: Option<i64>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub date_creation: Option<String>,
}

impl Partenariat {
    pub fn est_brouillon(&self) -> bool {
        self.statut == STATUT_BROUILLON
    }

    pub fn est_valide(&self) -> bool {
        self.statut == STATUT_VALIDE
    }

    pub fn est_accepte(&self) -> bool {
        self.statut == STATUT_ACCEPTE
    }

    pub fn est_refuse(&self) -> bool {
        self.statut == STATUT_REFUSE
    }

    pub fn est_annule(&self) -> bool {
        self.statut == STATUT_ANNULE
    }

    /// Le partenariat court-il encore à la date `aujourdhui` (`AAAA-MM-JJ`) ?
    ///
    /// **Fonction pure** : elle ne consulte pas la base. Un partenariat accepté
    /// sans date de fin court toujours ; sinon la date de fin est **couvrante**,
    /// c'est-à-dire que le dernier jour compte encore — même règle que
    /// [`crate::adherent::cotiser`]. Deux dates `AAAA-MM-JJ` se comparent comme
    /// du texte : l'ordre alphabétique est l'ordre chronologique.
    pub fn court_encore(&self, aujourdhui: &str) -> bool {
        if self.statut != STATUT_ACCEPTE {
            return false;
        }
        match self.date_fin.as_deref() {
            // Pas de terme : le partenariat court toujours.
            None => true,
            Some(fin) => fin >= aujourdhui,
        }
    }
}

/// Colonnes relues par [`list`] et [`get`].
///
/// `COALESCE` sur `count_last_url_check_error` : la colonne a un défaut à `0`
/// mais reste nullable, et une ligne écrite à la main avant le défaut ferait
/// échouer le décodage de toute la liste pour un simple compteur.
const SELECT_COLS: &str = "p.rowid, p.entity, p.ref, p.status, p.fk_type, t.label, \
     p.fk_soc, s.nom, p.fk_member, p.email_partnership, \
     DATE_FORMAT(p.date_partnership_start, '%Y-%m-%d'), \
     DATE_FORMAT(p.date_partnership_end, '%Y-%m-%d'), \
     p.reason_decline_or_cancel, p.url_to_check, \
     COALESCE(p.count_last_url_check_error, 0), p.fk_user_creat, \
     p.note_private, p.note_public, \
     DATE_FORMAT(p.date_creation, '%Y-%m-%d %H:%i:%s')";

/// Les jointures de lecture : le type pour son libellé, le tiers pour sa raison
/// sociale.
///
/// La jointure sur `llx_societe` porte aussi sur l'entité : un `fk_soc` qui
/// pointerait une société d'une autre entité ne doit pas faire apparaître son
/// nom ici, ce serait une fuite entre entités.
const DEPUIS: &str = "FROM llx_partnership p \
     LEFT JOIN llx_c_partnership_type t ON t.rowid = p.fk_type \
     LEFT JOIN llx_societe s ON s.rowid = p.fk_soc AND s.entity = p.entity";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

/// Les entiers passent par `i64` : `COALESCE(...)` élargit un `smallint` ou un
/// `tinyint` en `BIGINT`, et décoder directement en `i16` échouerait.
fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn map_row(row: &Row) -> Partenariat {
    Partenariat {
        rowid: opt_i(row, 0).unwrap_or(0),
        entity: opt_i(row, 1).unwrap_or(1),
        ref_: opt_s(row, 2).unwrap_or_default(),
        statut: opt_i(row, 3).unwrap_or(0) as i16,
        fk_type: opt_i(row, 4).unwrap_or(0),
        type_libelle: opt_s(row, 5),
        fk_soc: opt_i(row, 6),
        tiers_nom: opt_s(row, 7),
        fk_member: opt_i(row, 8),
        email: opt_s(row, 9),
        date_debut: opt_s(row, 10),
        date_fin: opt_s(row, 11),
        motif: opt_s(row, 12),
        url_to_check: opt_s(row, 13),
        count_last_url_check_error: opt_i(row, 14).unwrap_or(0),
        fk_user_creat: opt_i(row, 15),
        note_private: opt_s(row, 16),
        note_public: opt_s(row, 17),
        date_creation: opt_s(row, 18),
    }
}

/// Le libellé français d'un statut, pour les messages d'erreur.
///
/// Une transition refusée doit dire **dans quel état est la fiche** : « statut
/// 3 » ne se lit pas à l'écran, « refusé » si.
fn libelle_statut(statut: i16) -> &'static str {
    match statut {
        STATUT_BROUILLON => "brouillon",
        STATUT_VALIDE => "validé",
        STATUT_ACCEPTE => "accepté",
        STATUT_REFUSE => "refusé",
        STATUT_ANNULE => "annulé",
        _ => "dans un état inconnu",
    }
}

/// « brouillon », « validé ou accepté » : la liste des états attendus.
fn libelles(statuts: &[i16]) -> String {
    let mots: Vec<&str> = statuts.iter().map(|s| libelle_statut(*s)).collect();
    mots.join(" ou ")
}

/// Un texte optionnel dont la chaîne vide vaut « pas de valeur ».
///
/// Dolibarr écrit souvent `''` là où la colonne accepterait `NULL` ; une colonne
/// NULL dit vraiment qu'on n'a rien saisi, et évite qu'une comparaison sur
/// `email_partnership = ''` ne ramène des fiches vides.
fn texte(valeur: &Option<String>) -> Option<&str> {
    valeur.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

/// Une date `AAAA-MM-JJ` réelle : `2027-02-31` a le bon format mais n'existe pas
/// au calendrier, et MySQL refuserait l'écriture au milieu de l'insertion.
///
/// Même contrôle que `adherent::date_valide`, recopié ici : la fonction y est
/// privée au module, et la dupliquer est moins coûteux que d'ouvrir une API
/// publique pour quinze lignes.
fn date_valide(date: &str) -> bool {
    let octets = date.as_bytes();
    if octets.len() != 10 || octets[4] != b'-' || octets[7] != b'-' {
        return false;
    }
    if !octets
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    // Tous les octets restants sont des chiffres : la soustraction est sûre.
    let nombre = |debut: usize, fin: usize| {
        octets[debut..fin]
            .iter()
            .fold(0u32, |cumul, c| cumul * 10 + u32::from(c - b'0'))
    };
    let (annee, mois, jour) = (nombre(0, 4), nombre(5, 7), nombre(8, 10));

    annee >= 1 && jour >= 1 && jour <= jours_dans_le_mois(annee, mois)
}

/// Nombre de jours d'un mois, `0` si le mois n'existe pas.
fn jours_dans_le_mois(annee: u32, mois: u32) -> u32 {
    match mois {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if annee.is_multiple_of(400)
            || (annee.is_multiple_of(4) && !annee.is_multiple_of(100)) =>
        {
            29
        }
        2 => 28,
        _ => 0,
    }
}

/// Borne un texte à `max` octets sans couper un caractère.
///
/// `reason_decline_or_cancel` est un `text` : un accent saisi en UTF-8 occupe
/// deux octets, et une coupe au milieu laisserait un octet invalide que MySQL
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

/// Le message MySQL nomme-t-il cet index ?
///
/// Une erreur 1062 ne dit pas *quelle* contrainte a échoué autrement qu'en citant
/// le nom de l'index (`Duplicate entry '…' for key 'uk_partnership_ref'`). C'est
/// le seul moyen de rendre une phrase juste plutôt qu'un diagnostic faux, la
/// table portant trois index uniques aux sens très différents.
fn cite_index(e: &mysql::Error, index: &str) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062 && me.message.contains(index))
}

/// Le type de partenariat existe-t-il, et est-il actif ?
///
/// `llx_c_partnership_type` porte une colonne `entity`, mais [`types`] n'a pas
/// de paramètre d'entité (signature imposée) : la validation ne filtre donc pas
/// non plus sur l'entité, sans quoi le menu proposerait des types que la
/// création refuserait. Les deux lisent la même ligne, ils ne peuvent pas
/// diverger. Sur une instance multicompany, le menu montre l'union des
/// dictionnaires ; c'est une limite connue, sans conséquence sur une instance
/// mono-entité, qui est le cas de Dolibarr par défaut.
fn verifier_type(pool: &Pool, fk_type: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // Le paramètre lié s'appelle `modele_type`, jamais `type` : `params!` refuse
    // cette clé.
    let actif: Option<i64> = conn.exec_first(
        "SELECT active FROM llx_c_partnership_type WHERE rowid = :modele_type",
        params! { "modele_type" => fk_type },
    )?;
    match actif {
        None => Err(format!(
            "le type de partenariat {fk_type} n'existe pas dans llx_c_partnership_type"
        )
        .into()),
        Some(a) if a < 1 => Err(format!("le type de partenariat {fk_type} est désactivé").into()),
        Some(_) => Ok(()),
    }
}

/// Un identifiant de tiers ou d'adhérent ramené à sa forme en base.
///
/// `None` et `0` veulent dire « pas de rattachement » (Dolibarr range `0` là où
/// la colonne accepterait `NULL`). Un négatif, lui, est une faute d'appel : le
/// taire en l'écrivant `NULL` masquerait un bug d'écran.
fn rattachement(valeur: Option<i64>, champ: &str) -> Result<Option<i64>, Box<dyn Error>> {
    match valeur {
        None | Some(0) => Ok(None),
        Some(v) if v < 0 => {
            Err(format!("{champ} = {v} est invalide : un identifiant est positif").into())
        }
        Some(v) => Ok(Some(v)),
    }
}

/// Crée un partenariat en brouillon et renvoie son `rowid`.
///
/// `status` vaut [`STATUT_BROUILLON`] et `date_creation` vaut `NOW()`, comme
/// `Partnership::create()` en PHP (`$this->status = 0;` puis `createCommon`).
///
/// # Erreurs
///
/// - référence vide, ou de plus de [`REF_MAX`] octets ;
/// - type inconnu ou désactivé dans `llx_c_partnership_type` ;
/// - ni `fk_soc` ni `fk_member` (voir la documentation du module), ou un
///   identifiant négatif ;
/// - date de début vide ou inexistante au calendrier ;
/// - date de fin antérieure à la date de début ;
/// - `fk_user_creat` nul ou négatif ;
/// - email fourni sans arobase, ou plus long que [`EMAIL_MAX`] octets ;
/// - `url_to_check` plus longue que [`URL_MAX`] octets ;
/// - référence déjà prise, ou partenariat déjà existant pour ce tiers (ou cet
///   adhérent) à cette date de début — les deux index uniques de la table ;
/// - toute erreur MySQL remontée telle quelle.
pub fn creer(pool: &Pool, entity: i64, new: &NouveauPartenariat) -> Result<i64, Box<dyn Error>> {
    let reference = new.ref_.trim();
    if reference.is_empty() {
        return Err(
            "la référence du partenariat est obligatoire : c'est elle qui l'identifie".into(),
        );
    }
    if reference.len() > REF_MAX {
        return Err(format!(
            "la référence fait {} octets : {REF_MAX} au maximum",
            reference.len()
        )
        .into());
    }

    if new.fk_type <= 0 {
        return Err("le type de partenariat (fk_type) est obligatoire".into());
    }
    verifier_type(pool, new.fk_type)?;

    let fk_soc = rattachement(new.fk_soc, "fk_soc")?;
    let fk_member = rattachement(new.fk_member, "fk_member")?;
    if fk_soc.is_none() && fk_member.is_none() {
        return Err(
            "un partenariat se conclut avec un tiers (fk_soc) ou un adhérent (fk_member) : les deux sont vides"
                .into(),
        );
    }

    let debut = new.date_partnership_start.trim();
    if debut.is_empty() {
        return Err("la date de début du partenariat est obligatoire".into());
    }
    if !date_valide(debut) {
        return Err(format!(
            "date de début invalide : « {debut} » — une date s'écrit AAAA-MM-JJ et doit exister au calendrier"
        )
        .into());
    }
    let fin = texte(&new.date_partnership_end);
    if let Some(fin) = fin {
        if !date_valide(fin) {
            return Err(format!(
                "date de fin invalide : « {fin} » — une date s'écrit AAAA-MM-JJ et doit exister au calendrier"
            )
            .into());
        }
        // Deux dates `AAAA-MM-JJ` se comparent comme du texte : l'ordre
        // alphabétique est l'ordre chronologique.
        if fin < debut {
            return Err(format!(
                "la date de fin ({fin}) précède la date de début ({debut}) : le partenariat aurait une durée négative"
            )
            .into());
        }
    }

    if new.fk_user_creat <= 0 {
        return Err(
            "l'auteur de la création (fk_user_creat) est obligatoire : une fiche ne se crée pas sans utilisateur"
                .into(),
        );
    }

    let email = texte(&new.email_partnership);
    if let Some(saisi) = email {
        // On ne valide pas l'adresse au sens RFC 5322 : le contrôle qui attrape
        // vraiment les fautes de frappe, c'est l'arobase manquante.
        if !saisi.contains('@') {
            return Err(format!("l'adresse email « {saisi} » ne contient pas d'arobase").into());
        }
        if saisi.len() > EMAIL_MAX {
            return Err(format!(
                "l'adresse email fait {} octets : {EMAIL_MAX} au maximum",
                saisi.len()
            )
            .into());
        }
    }

    let url = texte(&new.url_to_check);
    if let Some(lien) = url.filter(|l| l.len() > URL_MAX) {
        return Err(format!(
            "l'URL à vérifier fait {} octets : {URL_MAX} au maximum",
            lien.len()
        )
        .into());
    }

    let mut conn = pool.get_conn()?;
    let insere = conn.exec_drop(
        "INSERT INTO llx_partnership \
         (entity, ref, status, fk_type, fk_soc, fk_member, email_partnership, \
          date_partnership_start, date_partnership_end, date_creation, \
          fk_user_creat, url_to_check) \
         VALUES (:entity, :ref, :statut, :modele_type, :fk_soc, :fk_member, :email, \
          :debut, :fin, NOW(), :auteur, :url)",
        params! {
            "entity" => entity,
            "ref" => reference,
            "statut" => STATUT_BROUILLON,
            "modele_type" => new.fk_type,
            "fk_soc" => fk_soc,
            "fk_member" => fk_member,
            "email" => email,
            "debut" => debut,
            "fin" => fin,
            "auteur" => new.fk_user_creat,
            "url" => url,
        },
    );

    match insere {
        Ok(()) => Ok(conn.last_insert_id() as i64),
        Err(e) if cite_index(&e, "uk_partnership_ref") => Err(format!(
            "la référence « {reference} » est déjà prise dans l'entité {entity}"
        )
        .into()),
        Err(e) if cite_index(&e, "uk_fk_type_fk_soc") || cite_index(&e, "uk_fk_type_fk_member") => {
            Err(format!(
                "un partenariat de ce type existe déjà pour ce partenaire à la date du {debut} : \
                 deux fois le même type au même jour n'a pas de sens"
            )
            .into())
        }
        Err(e) => Err(e.into()),
    }
}

/// Les partenariats d'une entité, du plus récent au plus ancien.
///
/// L'ordre suit `date_creation` puis `rowid` : deux fiches saisies dans la même
/// seconde gardent un ordre stable, ce qu'un tri sur la seule date ne garantit
/// pas.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Partenariat>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} {DEPUIS} WHERE p.entity = :entity \
         ORDER BY p.date_creation DESC, p.rowid DESC LIMIT :limite OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limite" => limite, "offset" => offset },
    )?;
    Ok(rows.iter().map(map_row).collect())
}

/// Un partenariat précis, ou `None` s'il n'existe pas dans cette entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Partenariat>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql =
        format!("SELECT {SELECT_COLS} {DEPUIS} WHERE p.entity = :entity AND p.rowid = :rowid");
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.as_ref().map(map_row))
}

/// Statut courant d'un partenariat, vu par l'entité qui le demande.
fn statut_actuel(
    conn: &mut mysql::PooledConn,
    entity: i64,
    rowid: i64,
) -> Result<i16, Box<dyn Error>> {
    let statut: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_partnership WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        Some(s) => Ok(s as i16),
        None => Err(format!("partenariat {rowid} introuvable dans l'entité {entity}").into()),
    }
}

/// Change le statut, à condition que la fiche soit dans l'un des états attendus.
///
/// Le contrôle et l'écriture sont dans la même connexion, mais pas dans la même
/// transaction : `status` ne bouge que si le statut lu est le bon, et deux
/// transitions simultanées ne peuvent pas se croiser sans que l'une des deux
/// échoue au contrôle. Dolibarr n'a pas de verrou non plus ici.
///
/// `motif` n'est écrit que s'il est fourni : annuler sans motif ne doit pas
/// effacer le motif d'un refus antérieur, et réciproquement.
fn transitionner(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    depuis: &[i16],
    vers: i16,
    action: &str,
    motif: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let actuel = statut_actuel(&mut conn, entity, rowid)?;
    if !depuis.contains(&actuel) {
        return Err(format!(
            "impossible de {action} : le partenariat {rowid} est {} (statut {actuel}), \
             or il faut qu'il soit {}",
            libelle_statut(actuel),
            libelles(depuis)
        )
        .into());
    }

    match motif {
        Some(motif) => conn.exec_drop(
            "UPDATE llx_partnership SET status = :vers, reason_decline_or_cancel = :motif \
             WHERE entity = :entity AND rowid = :rowid",
            params! { "vers" => vers, "motif" => motif, "entity" => entity, "rowid" => rowid },
        )?,
        None => conn.exec_drop(
            "UPDATE llx_partnership SET status = :vers \
             WHERE entity = :entity AND rowid = :rowid",
            params! { "vers" => vers, "entity" => entity, "rowid" => rowid },
        )?,
    }
    Ok(())
}

/// Soumet un brouillon : statut 0 → 1.
///
/// C'est le geste qui sort la fiche du brouillon, comme `Partnership::validate()`
/// en PHP (qui exige un utilisateur pour signer ; ce module n'a pas de session,
/// et ne pose donc aucun `fk_user_valid`).
///
/// # Erreurs
///
/// Partenariat introuvable dans l'entité, ou déjà dans un autre état — le
/// message dit lequel.
pub fn soumettre(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    transitionner(
        pool,
        entity,
        rowid,
        &[STATUT_BROUILLON],
        STATUT_VALIDE,
        "soumettre",
        None,
    )
}

/// Accepte un partenariat validé : statut 1 → 2.
///
/// Accepter un brouillon reviendrait à engager l'entreprise sans que personne
/// n'ait relu la fiche : la soumission est le contrôle, l'acceptation la
/// décision, et l'ordre entre les deux est le seul qui ait un sens.
///
/// # Erreurs
///
/// Partenariat introuvable dans l'entité, ou déjà dans un autre état.
pub fn accepter(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    transitionner(
        pool,
        entity,
        rowid,
        &[STATUT_VALIDE],
        STATUT_ACCEPTE,
        "accepter",
        None,
    )
}

/// Refuse un partenariat validé : statut 1 → 3, avec un motif facultatif.
///
/// Le motif est borné à [`MOTIF_MAX`] octets, sans couper un caractère. Un motif
/// vide est permis — Dolibarr fait de même (`refused($user, $reasondeclinenote
/// = '')`) — et laisse la colonne à `NULL` plutôt que d'y écrire une chaîne
/// vide qui ferait croire à une justification.
///
/// # Erreurs
///
/// Partenariat introuvable dans l'entité, ou déjà dans un autre état.
pub fn refuser(pool: &Pool, entity: i64, rowid: i64, motif: &str) -> Result<(), Box<dyn Error>> {
    let borne = tronquer(motif.trim(), MOTIF_MAX);
    let motif = if borne.is_empty() { None } else { Some(borne) };
    transitionner(
        pool,
        entity,
        rowid,
        &[STATUT_VALIDE],
        STATUT_REFUSE,
        "refuser",
        motif,
    )
}

/// Annule un partenariat validé ou accepté : statut 1 ou 2 → 9, avec un motif
/// facultatif.
///
/// ⚠️ **9, et non 4** : voir la documentation du module, [`STATUT_ANNULE`] est
/// `STATUS_CANCELED` dans Dolibarr.
///
/// Annuler ne supprime rien : la fiche reste, l'historique reste, seul l'état
/// change. C'est ce qui distingue une annulation d'un `DELETE`, et c'est
/// pourquoi un partenariat n'est jamais effacé par ce module.
///
/// # Erreurs
///
/// Partenariat introuvable dans l'entité, ou dans un état qui ne s'annule pas
/// (brouillon, déjà refusé, déjà annulé).
pub fn annuler(pool: &Pool, entity: i64, rowid: i64, motif: &str) -> Result<(), Box<dyn Error>> {
    let borne = tronquer(motif.trim(), MOTIF_MAX);
    let motif = if borne.is_empty() { None } else { Some(borne) };
    transitionner(
        pool,
        entity,
        rowid,
        &[STATUT_VALIDE, STATUT_ACCEPTE],
        STATUT_ANNULE,
        "annuler",
        motif,
    )
}

/// Combien de partenariats courent encore : acceptés, et dont la date de fin est
/// nulle ou postérieure à aujourd'hui.
///
/// La comparaison est `date_partnership_end >= CURDATE()` : le **dernier jour
/// compte**, comme pour les adhésions — un partenariat qui se termine
/// aujourd'hui court encore aujourd'hui. `CURDATE()` est celle du serveur de
/// base, pas celle du poste : c'est la base qui date les écritures, c'est donc
/// elle qui fait foi.
///
/// `date_partnership_start` n'est pas comparée : une fiche acceptée dont la date
/// de début est dans le futur compte ici. C'est voulu — un partenariat signé
/// pour l'an prochain est bien un partenariat en cours, et le décompte sert à
/// savoir combien de dossiers sont vivants, pas combien ont commencé.
pub fn en_cours(pool: &Pool, entity: i64) -> Result<usize, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let nombre: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_partnership \
         WHERE entity = :entity AND status = :statut \
           AND (date_partnership_end IS NULL OR date_partnership_end >= CURDATE())",
        params! { "entity" => entity, "statut" => STATUT_ACCEPTE },
    )?;
    Ok(nombre.unwrap_or(0).max(0) as usize)
}

/// Les types de partenariat actifs, pour un menu déroulant : `(rowid, libellé)`.
///
/// Seuls les types actifs sortent : un type désactivé reste en base pour ne pas
/// casser les fiches déjà créées, mais il ne doit plus être proposé à la saisie —
/// et [`verifier_type`] le refuse aussi, pour que le menu et la validation
/// voient exactement la même liste.
///
/// **Failible**, contrairement à ce qu'un simple « liste pour un menu » laisserait
/// croire : une base injoignable doit se voir. Un menu vide et un menu qu'on n'a
/// pas pu remplir ne se ressemblent que trop, et celui-là ferait croire qu'aucun
/// type n'existe — donc qu'aucun partenariat n'est possible. C'est la règle déjà
/// suivie par `adherent::types`, et elle vaut ici d'autant plus que le
/// dictionnaire des types de partenariat est **vide par défaut** dans Dolibarr
/// (voir la documentation du module) : une liste vide y est un état normal, qu'il
/// ne faut pas confondre avec une panne.
///
/// Pas de filtre sur l'entité : la signature n'en porte pas, et [`verifier_type`]
/// ne filtre pas non plus. Voir la note de [`verifier_type`].
pub fn types(pool: &Pool) -> Result<Vec<(i64, String)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(i64, String)> = conn.exec(
        "SELECT rowid, label FROM llx_c_partnership_type \
         WHERE active = 1 ORDER BY label ASC, rowid ASC",
        (),
    )?;
    Ok(rows)
}
