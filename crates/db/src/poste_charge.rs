//! Postes de charge : les ressources de production (`llx_workstation_workstation`)
//! et leurs rattachements — machines (`llx_workstation_workstation_resource`) et
//! groupes d'utilisateurs (`llx_workstation_workstation_usergroup`).
//!
//! Un poste de charge, c'est l'établi, la table de découpe, le four, l'atelier :
//! l'endroit où le travail se fait et le temps se consomme. Dolibarr l'appelle
//! `workstation` (« poste de travail ») ; le module MRP s'en sert pour chiffrer
//! une nomenclature, le module Produit pour proposer un poste par défaut sur une
//! ligne d'opération. On garde le vocabulaire de la source dans les noms de
//! tables et de colonnes, et le vocabulaire métier dans la documentation.
//!
//! ## Le terrain, mesuré avant d'écrire (MariaDB `127.0.0.1:3307`, base
//! `dolibarr`, 2026-11-06)
//!
//! Colonnes réelles de `llx_workstation_workstation`, lues dans
//! `information_schema.columns` :
//!
//! ```text
//! rowid                     int(11)       NOT NULL  PRI
//! ref                       varchar(128)  NOT NULL  défaut '(PROV)'   MUL
//! label                     varchar(255)  NULL
//! type                      varchar(7)    NULL
//! note_public               text          NULL
//! entity                    int(11)       NULL      défaut 1
//! note_private              text          NULL
//! date_creation             datetime      NOT NULL
//! tms                       timestamp     NULL      ON UPDATE CURRENT_TIMESTAMP
//! fk_user_creat             int(11)       NOT NULL  MUL
//! fk_user_modif             int(11)       NULL
//! import_key                varchar(14)   NULL
//! status                    smallint(6)   NOT NULL  MUL
//! nb_operators_required     int(11)       NULL
//! thm_operator_estimated    double        NULL
//! thm_machine_estimated     double        NULL
//! ```
//!
//! Les deux tables de jonction n'ont **que** trois colonnes utiles
//! (`fk_workstation` + la cible, plus `rowid` et `tms`) :
//!
//! ```text
//! llx_workstation_workstation_resource  : rowid, tms, fk_resource, fk_workstation
//! llx_workstation_workstation_usergroup : rowid, tms, fk_usergroup, fk_workstation
//! ```
//!
//! Ni l'une ni l'autre n'a de colonne `entity`, et **ni l'une ni l'autre n'a
//! d'index unique** : `SHOW INDEX` ne rend que la clé primaire. Conséquence —
//! voir plus bas — c'est au code de refuser le doublon, la base ne le fera pas.
//!
//! La table des postes est vide dans la base de référence (`COUNT(*) = 0`) :
//! rien n'a été repris d'un existant, et les tests créent tout ce qu'ils
//! manipulent.
//!
//! ## Il n'y a pas de colonne « coût horaire », il y en a deux
//!
//! `NouveauPoste.hourly_cost` est donc un choix de portage, pas une colonne. La
//! source porte deux taux horaires moyens estimés :
//!
//! - `thm_operator_estimated` — « THM estimé de l'opérateur », aide en ligne :
//!   « Coût estimé de l'opérateur par heure » (`langs/fr_FR/mrp.lang`, lignes
//!   119 et 147) ;
//! - `thm_machine_estimated` — « THM machine estimé ».
//!
//! [`creer`] écrit `hourly_cost` dans **`thm_operator_estimated`**, et laisse
//! `thm_machine_estimated` à `NULL`. Pourquoi celle-là : c'est la main-d'œuvre
//! qui fait tourner un poste, la machine vient après, et une valeur mise dans la
//! colonne opérateur reste juste quoi qu'il arrive ensuite — [`cout_horaire`]
//! additionne les deux, donc un coût machine ajouté plus tard dans Dolibarr est
//! compté, sans que ce module n'ait à le connaître.
//!
//! `modifier` ne touche jamais `thm_machine_estimated` : il remplace le coût
//! qu'il gère, pas celui qu'il ignore.
//!
//! ## `description` n'existe pas non plus : c'est `note_public`
//!
//! La table a `note_public` et `note_private`, pas de `description`. La note
//! publique est le texte libre de la fiche, celui qu'on montre ; on y écrit
//! `description`. `note_private` n'est ni lue ni écrite : un commentaire interne
//! saisi dans Dolibarr ne doit pas être écrasé par un formulaire de ce module.
//!
//! ## Deux statuts, aucun brouillon
//!
//! La source déclare exactement deux constantes
//! (`workstation/class/workstation.class.php`, lignes 61-62) :
//!
//! ```text
//! const STATUS_DISABLED = 0;
//! const STATUS_ENABLED  = 1;
//! ```
//!
//! Il n'y a **pas** de `STATUS_DRAFT` : le vocabulaire « brouillon » n'existe pas
//! dans ce module. La valeur de repos à la création est donc `0` —
//! `STATUS_DISABLED`, c'est-à-dire « la fiche existe, elle n'est pas en service »,
//! ce que la suite du noyau traite comme un brouillon :
//!
//! - `createFromClone()` écrit `$object->status = self::STATUS_DISABLED` sur la
//!   copie (ligne 301) ;
//! - la carte n'autorise la suppression par le simple droit d'ajout que si
//!   `$object->status == Workstation::STATUS_DISABLED` (ligne 92) : c'est bien le
//!   statut d'un objet qui n'a encore rien produit ;
//! - `setEnabled()` / `setDisabled()` (lignes 546 et 559) sont les deux
//!   transitions, déclenchées par `setStatus(1)` et `setStatus(0)`
//!   (`workstation_card.php`, lignes 147 et 151).
//!
//! [`valider`] fait donc `0 -> 1`, [`desactiver`] fait `1 -> 0`, et **on refuse
//! la transition qui ne change rien**. La source ne le fait pas :
//! `CommonObject::setStatusCommon()` écrit le statut en aveugle et
//! ré-exécute le trigger. On est plus strict, volontairement : un double clic
//! ou un rejeu de requête passerait sinon pour une seconde validation, et
//! l'appelant n'aurait aucun moyen de le savoir.
//!
//! ## La référence n'est pas unique en base
//!
//! `SHOW INDEX FROM llx_workstation_workstation` rend
//! `idx_workstation_workstation_ref` en `Non_unique = 1` : contrairement aux
//! ressources (`uk_resource_ref`), **rien** n'empêche deux postes de porter la
//! même référence. Or la référence est l'identifiant que l'atelier reconnaît
//! (Dolibarr la pré-remplit avec `getNextNumRef()`, un compteur), et deux fiches
//! homonymes rendent toute recherche ambiguë. [`creer`] refuse donc un doublon
//! explicitement. La comparaison est celle de MySQL sur une colonne
//! `utf8mb4_unicode_ci` : insensible à la casse, ce qui est le comportement
//! voulu ici.
//!
//! Limite assumée : cet index n'étant pas unique, deux créations simultanées
//! peuvent passer toutes les deux le contrôle. Ce n'est pas plus mal que
//! Dolibarr, qui n'en fait aucun.
//!
//! ## Les jonctions sont globales, le cloisonnement passe par le poste
//!
//! Aucune des deux tables de jonction n'a d'`entity`. Toute lecture et toute
//! écriture joint donc `llx_workstation_workstation` (qui, elle, porte `entity`)
//! ou vérifie l'appartenance de la cible. C'est le sens du paramètre `entity`
//! partout dans ce module : sans lui, le poste 12 de la société 1 et le poste 12
//! de la société 7 — les `rowid` ne se recoupent pas, mais peu importe — se
//! disputeraient les mêmes liens.
//!
//! Le doublon de lien est refusé par le code, faute d'index unique : attacher
//! deux fois la même machine au même poste créerait deux lignes que Dolibarr
//! lirait deux fois (`WorkstationResource::getAllResourcesOfWorkstation()` fait
//! un simple `SELECT` puis `array_map('intval')`, sans `DISTINCT`).
//!
//! ## Champs hors périmètre
//!
//! `note_private`, `import_key`, `tms`, `fk_user_modif` ne sont jamais écrits.
//! `fk_user_modif` en particulier : [`modifier`] ne prend pas d'auteur dans sa
//! signature, et y écrire un identifiant inventé laisserait une fausse trace de
//! qui a modifié quoi — mieux vaut `NULL` qu'un mensonge.
//!
//! `type` (`HUMAN`, `MACHINE`, `BOTH`) est **lu** mais pas écrit : la colonne est
//! un `varchar(7)` sans valeur par défaut, et la classe en déclare une valeur par
//! défaut (`'1'`) qui n'est même pas une des clés de son `arrayofkeyval`. Choisir
//! à la place de l'utilisateur un type qui décide de l'affichage des sections de
//! la fiche (groupes masqués pour `MACHINE`, ressources masquées pour `HUMAN` —
//! `workstation_card.php`, lignes 179-196) serait une décision d'écran, pas de
//! données : on la laisse à l'écran.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, PooledConn};
use rust_decimal::Decimal;

use money::round_money;

/// Longueur maximale de `llx_workstation_workstation.ref`, en **octets**.
///
/// La colonne est un `varchar(128)`, qui compte des caractères : la borne en
/// octets est donc plus stricte, jamais plus permissive. Un `varchar(128)` plein
/// d'accents ferait 256 octets, et il n'existe aucun usage d'une référence de
/// poste de 128 caractères accentués — mieux vaut refuser avant l'écriture que
/// d'apprendre la limite par une erreur de troncature.
pub const REF_MAX: usize = 128;

/// Longueur maximale de `llx_workstation_workstation.label`, en **octets**.
///
/// Même raisonnement que [`REF_MAX`], sur un `varchar(255)`.
pub const LABEL_MAX: usize = 255;

/// Statut d'un poste créé par ce module : hors service
/// (`Workstation::STATUS_DISABLED = 0`).
///
/// C'est la valeur de repos de la source — voir la doc du module sur l'absence de
/// statut « brouillon ».
pub const STATUT_DESACTIVE: i16 = 0;

/// Statut d'un poste en service (`Workstation::STATUS_ENABLED = 1`).
pub const STATUT_ACTIF: i16 = 1;

/// Un poste de charge, tel qu'affiché en liste ou repris en formulaire.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Poste {
    pub rowid: i64,
    pub entity: i64,
    /// Référence du poste (`ref`) : saisie, jamais recalculée après création.
    pub ref_: String,
    pub label: String,
    /// `type` : `HUMAN`, `MACHINE` ou `BOTH`. Lu seulement — voir la doc du
    /// module. `None` sur un poste créé ici : la colonne n'a pas de défaut.
    pub type_: Option<String>,
    /// Texte libre de la fiche, rangé dans `note_public`.
    pub description: Option<String>,
    /// `nb_operators_required` : combien de personnes le poste occupe.
    ///
    /// **Ce n'est pas un multiplicateur de coût** : voir [`cout_horaire`].
    pub nb_operators: i32,
    /// `thm_operator_estimated` : coût horaire de la main-d'œuvre du poste.
    pub hourly_cost: Decimal,
    /// `thm_machine_estimated` : coût horaire de la machine, `NULL` si non chiffré.
    pub hourly_cost_machine: Option<Decimal>,
    /// `0` = hors service, `1` = en service. Voir [`STATUT_DESACTIVE`].
    pub statut: i16,
    /// Date de création, `AAAA-MM-JJ HH:MM:SS`.
    pub datec: Option<String>,
    pub fk_user_creat: Option<i64>,
    pub fk_user_modif: Option<i64>,
}

/// Ce qu'il faut pour créer un poste de charge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NouveauPoste {
    /// Référence, obligatoire, [`REF_MAX`] octets au plus, unique dans l'entité.
    /// Le suffixe `_` est imposé : `ref` est un mot-clé Rust.
    pub ref_: String,
    /// Libellé, obligatoire (« table de découpe », « four 1 »), [`LABEL_MAX`]
    /// octets au plus.
    pub label: String,
    /// Texte libre facultatif, écrit dans `note_public`. Une chaîne vide vaut
    /// « non renseigné » et laisse la colonne à `NULL` : un formulaire HTML
    /// renvoie toujours du texte, jamais l'absence.
    pub description: Option<String>,
    /// Nombre d'opérateurs, jamais négatif.
    pub nb_operators: i32,
    /// Coût horaire de la main-d'œuvre, jamais négatif, écrit dans
    /// `thm_operator_estimated`.
    pub hourly_cost: Decimal,
    /// Auteur de la fiche, strictement positif **et existant** :
    /// `llx_workstation_workstation.fk_user_creat` porte une vraie clé étrangère
    /// vers `llx_user.rowid` (contrainte
    /// `fk_workstation_workstation_fk_user_creat`, mesurée), donc un identifiant
    /// inconnu est refusé par la base — voir [`creer`], qui traduit l'erreur 1452.
    pub fk_user_creat: i64,
}

/// Ce qui peut changer sur un poste existant.
///
/// **La référence n'y figure pas**, et c'est le point : elle est saisie une fois,
/// puis citée sur les fiches de fabrication et dans les nomenclatures qui
/// pointent dessus. L'impossibilité de la modifier est portée par le type, pas
/// par une consigne en commentaire qu'un appelant distrait ignorerait.
///
/// La modification est **totale** : les quatre champs remplacent les quatre
/// colonnes. Un `description: None` efface la note publique — c'est voulu, et
/// sans danger parce que le libellé, lui, est obligatoire : on ne peut pas vider
/// une fiche par inadvertance avec une structure par défaut, l'appel serait
/// refusé faute de libellé.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModifPoste {
    pub label: String,
    pub description: Option<String>,
    pub nb_operators: i32,
    pub hourly_cost: Decimal,
}

/// Colonnes lues, **sans `entity`** : l'entité est le paramètre de la requête,
/// pas une donnée de la ligne — et un tuple de treize éléments sortirait du
/// domaine de `FromRow` (mysql_common n'implémente `FromRow` que jusqu'à douze
/// colonnes, vérifié dans `mysql_common-0.32.4/src/row/convert/mod.rs`).
const SELECT_COLS: &str = "w.rowid, w.ref, w.label, w.type, w.note_public, \
     w.nb_operators_required, CAST(w.thm_operator_estimated AS CHAR), \
     CAST(w.thm_machine_estimated AS CHAR), w.status, \
     DATE_FORMAT(w.date_creation, '%Y-%m-%d %H:%i:%s'), w.fk_user_creat, w.fk_user_modif";

/// Une ligne brute de `llx_workstation_workstation` : douze colonnes, donc un
/// alias — `clippy::type_complexity` refuse un tuple de cette taille écrit en
/// clair dans une signature.
type LignePoste = (
    i64,            // rowid
    String,         // ref
    Option<String>, // label (nullable en base, même si Dolibarr le dit obligatoire)
    Option<String>, // type
    Option<String>, // note_public
    Option<i64>,    // nb_operators_required
    Option<String>, // thm_operator_estimated
    Option<String>, // thm_machine_estimated
    i64,            // status
    Option<String>, // date_creation
    Option<i64>,    // fk_user_creat
    Option<i64>,    // fk_user_modif
);

/// Une saisie facultative vide vaut « non renseigné ».
fn texte(valeur: Option<&str>) -> Option<&str> {
    valeur.map(str::trim).filter(|s| !s.is_empty())
}

/// Un montant lu en `CAST(... AS CHAR)` : `None` si la colonne est `NULL` ou si
/// la chaîne n'est pas un décimal (colonne `double`, donc jamais exacte au-delà
/// de quinze chiffres significatifs — on lit la chaîne, on ne fait pas d'arrondi
/// intermédiaire).
fn dec_opt(valeur: Option<&str>) -> Option<Decimal> {
    valeur.and_then(|s| Decimal::from_str(s.trim()).ok())
}

/// Fabrique un [`Poste`] à partir d'une ligne et de l'entité interrogée.
fn vers_poste(
    entity: i64,
    (
        rowid,
        ref_,
        label,
        type_,
        note,
        operateurs,
        cout_operateur,
        cout_machine,
        statut,
        datec,
        createur,
        modifieur,
    ): LignePoste,
) -> Poste {
    Poste {
        rowid,
        entity,
        ref_,
        // `label` est nullable en base : on ne fait pas échouer toute une liste
        // parce qu'une ligne écrite hors de ce module n'en a pas.
        label: label.unwrap_or_default(),
        type_,
        description: note,
        nb_operators: operateurs.unwrap_or(0) as i32,
        hourly_cost: dec_opt(cout_operateur.as_deref()).unwrap_or_default(),
        hourly_cost_machine: dec_opt(cout_machine.as_deref()),
        statut: statut as i16,
        datec,
        fk_user_creat: createur,
        fk_user_modif: modifieur,
    }
}

/// Erreur 1452 : violation de clé étrangère. Ici, un `fk_user_creat` qui ne
/// désigne aucun utilisateur.
fn auteur_inconnu(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1452)
}

/// Statut du poste, et du même coup : existe-t-il dans cette entité ?
///
/// Sert de garde à toutes les opérations qui visent un poste précis — une seule
/// requête, un seul message d'erreur, et le cloisonnement par entité est dans le
/// `WHERE`.
fn statut_du_poste(conn: &mut PooledConn, entity: i64, rowid: i64) -> Result<i16, Box<dyn Error>> {
    let trouve: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_workstation_workstation \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match trouve {
        Some(statut) => Ok(statut as i16),
        None => Err(format!("le poste {rowid} n'existe pas dans l'entité {entity}").into()),
    }
}

fn ressource_existe(
    conn: &mut PooledConn,
    entity: i64,
    rowid: i64,
) -> Result<bool, Box<dyn Error>> {
    let trouve: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_resource WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    Ok(trouve.is_some())
}

fn groupe_existe(conn: &mut PooledConn, entity: i64, rowid: i64) -> Result<bool, Box<dyn Error>> {
    let trouve: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_usergroup WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    Ok(trouve.is_some())
}

/// Les postes de l'entité, par ordre alphabétique de référence.
///
/// L'ordre est celui d'un atelier, pas celui des dernières fiches créées : on
/// cherche une machine par son nom.
pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Poste>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_workstation_workstation w \
         WHERE w.entity = :entity \
         ORDER BY w.ref ASC, w.rowid ASC LIMIT :limit OFFSET :offset"
    );
    let rows: Vec<LignePoste> = conn.exec(
        sql,
        params! { "entity" => entity, "limit" => limit, "offset" => offset },
    )?;
    Ok(rows.into_iter().map(|l| vers_poste(entity, l)).collect())
}

/// Un poste précis, ou `None` s'il n'existe pas dans l'entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Poste>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_workstation_workstation w \
         WHERE w.entity = :entity AND w.rowid = :rowid"
    );
    let ligne: Option<LignePoste> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(ligne.map(|l| vers_poste(entity, l)))
}

/// Crée un poste de charge et rend son `rowid`.
///
/// Refuse, **avant toute écriture** : une référence vide, une référence de plus
/// de [`REF_MAX`] octets, une référence déjà portée par un autre poste de
/// l'entité, un libellé vide ou de plus de [`LABEL_MAX`] octets, un
/// `nb_operators` négatif, un `hourly_cost` négatif et un auteur non strictement
/// positif.
///
/// Le statut écrit est [`STATUT_DESACTIVE`] : le poste existe mais n'est pas en
/// service, exactement ce que la source écrit sur une copie
/// (`createFromClone()`). `date_creation` vaut `NOW()` ; `fk_user_modif`,
/// `note_private`, `import_key` et `thm_machine_estimated` restent `NULL`.
///
/// Un auteur inexistant n'est pas intercepté par nos contrôles — on ne vérifie
/// que la positivité — mais par la clé étrangère `llx_user` ; l'erreur 1452 est
/// traduite en message lisible plutôt que rendue brute, parce qu'un
/// « Cannot add or update a child row » ne dit pas à l'appelant quel champ il a
/// mal rempli.
pub fn creer(pool: &Pool, entity: i64, new: &NouveauPoste) -> Result<i64, Box<dyn Error>> {
    let reference = new.ref_.trim();
    if reference.is_empty() {
        return Err("la référence du poste de charge est obligatoire".into());
    }
    if reference.len() > REF_MAX {
        return Err(format!(
            "la référence du poste dépasse {REF_MAX} octets ({} reçus)",
            reference.len()
        )
        .into());
    }
    let libelle = new.label.trim();
    if libelle.is_empty() {
        return Err("le libellé du poste de charge est obligatoire : \
                    « poste », « atelier 2 » ou « four 1 » — un poste sans nom \
                    ne se retrouve pas dans une liste"
            .into());
    }
    if libelle.len() > LABEL_MAX {
        return Err(format!(
            "le libellé du poste dépasse {LABEL_MAX} octets ({} reçus)",
            libelle.len()
        )
        .into());
    }
    if new.nb_operators < 0 {
        return Err(format!(
            "le nombre d'opérateurs ({}) ne peut pas être négatif",
            new.nb_operators
        )
        .into());
    }
    if new.hourly_cost < Decimal::ZERO {
        return Err(format!(
            "le coût horaire ({}) ne peut pas être négatif",
            new.hourly_cost
        )
        .into());
    }
    if new.fk_user_creat <= 0 {
        return Err(format!(
            "l'auteur du poste (fk_user_creat = {}) est invalide",
            new.fk_user_creat
        )
        .into());
    }

    let mut conn = pool.get_conn()?;

    // L'index `idx_workstation_workstation_ref` n'est pas unique : la base
    // laisserait passer deux postes homonymes. Voir la doc du module.
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_workstation_workstation \
         WHERE entity = :entity AND ref = :reference",
        params! { "entity" => entity, "reference" => reference },
    )?;
    if deja.is_some() {
        return Err(format!(
            "un poste de charge porte déjà la référence « {reference} » \
             dans l'entité {entity}"
        )
        .into());
    }

    let insere = conn.exec_drop(
        "INSERT INTO llx_workstation_workstation \
         (entity, ref, label, note_public, nb_operators_required, \
          thm_operator_estimated, date_creation, fk_user_creat, status) \
         VALUES (:entity, :reference, :libelle, :description, :operateurs, \
                 :cout, NOW(), :auteur, :statut)",
        params! {
            "entity" => entity,
            "reference" => reference,
            "libelle" => libelle,
            "description" => texte(new.description.as_deref()),
            "operateurs" => new.nb_operators,
            "cout" => new.hourly_cost.to_string(),
            "auteur" => new.fk_user_creat,
            "statut" => STATUT_DESACTIVE,
        },
    );

    match insere {
        Ok(()) => Ok(conn.last_insert_id() as i64),
        Err(e) if auteur_inconnu(&e) => Err(format!(
            "l'auteur {} n'existe pas dans llx_user : la colonne fk_user_creat \
             est une clé étrangère, et un poste sans auteur réel n'a pas de trace",
            new.fk_user_creat
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

/// Met à jour un poste existant. **La référence n'est jamais modifiée** : elle
/// n'est même pas dans [`ModifPoste`].
///
/// Les mêmes contrôles qu'à la création s'appliquent sur les champs fournis
/// (libellé non vide et assez court, `nb_operators` positif ou nul, coût positif
/// ou nul). Aucun contrôle de statut : Dolibarr laisse modifier un poste en
/// service (`update()` ne regarde pas `status`), et interdire ici ce que la
/// source autorise ferait diverger les deux.
///
/// Les colonnes hors périmètre — `note_private`, `thm_machine_estimated`,
/// `type`, `import_key`, `fk_user_modif` — ne sont pas citées par le `UPDATE`,
/// donc pas écrasées.
pub fn modifier(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    modif: &ModifPoste,
) -> Result<(), Box<dyn Error>> {
    let libelle = modif.label.trim();
    if libelle.is_empty() {
        return Err("le libellé du poste de charge est obligatoire".into());
    }
    if libelle.len() > LABEL_MAX {
        return Err(format!(
            "le libellé du poste dépasse {LABEL_MAX} octets ({} reçus)",
            libelle.len()
        )
        .into());
    }
    if modif.nb_operators < 0 {
        return Err(format!(
            "le nombre d'opérateurs ({}) ne peut pas être négatif",
            modif.nb_operators
        )
        .into());
    }
    if modif.hourly_cost < Decimal::ZERO {
        return Err(format!(
            "le coût horaire ({}) ne peut pas être négatif",
            modif.hourly_cost
        )
        .into());
    }

    let mut conn = pool.get_conn()?;
    // Existence et cloisonnement : lève si le poste n'est pas dans l'entité.
    statut_du_poste(&mut conn, entity, rowid)?;

    conn.exec_drop(
        "UPDATE llx_workstation_workstation \
         SET label = :libelle, note_public = :description, \
             nb_operators_required = :operateurs, thm_operator_estimated = :cout \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "libelle" => libelle,
            "description" => texte(modif.description.as_deref()),
            "operateurs" => modif.nb_operators,
            "cout" => modif.hourly_cost.to_string(),
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Met le poste en service : `0 -> 1` (`setEnabled()`, `setStatus(1)`).
///
/// Refuse un poste déjà en service, et refuse un statut qui ne serait ni `0` ni
/// `1` : une valeur inattendue en base signale une écriture qui n'est pas passée
/// par ce module, et la « réparer » en la validant effacerait la trace du
/// problème. Voir la doc du module sur le refus des transitions sans effet.
///
/// L'`UPDATE` porte le statut attendu dans son `WHERE` : si deux clients valident
/// en même temps, un seul écrit et l'autre reçoit le refus au lieu d'un succès
/// silencieux.
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_du_poste(&mut conn, entity, rowid)?;
    if statut == STATUT_ACTIF {
        return Err(format!("le poste {rowid} est déjà validé").into());
    }
    if statut != STATUT_DESACTIVE {
        return Err(format!(
            "le poste {rowid} porte un statut inconnu ({statut}) : ni {STATUT_DESACTIVE} \
             ni {STATUT_ACTIF} — il ne se valide pas par-dessus"
        )
        .into());
    }

    conn.exec_drop(
        "UPDATE llx_workstation_workstation SET status = :statut \
         WHERE entity = :entity AND rowid = :rowid AND status = :attendu",
        params! {
            "statut" => STATUT_ACTIF,
            "entity" => entity,
            "rowid" => rowid,
            "attendu" => STATUT_DESACTIVE,
        },
    )?;
    if conn.affected_rows() != 1 {
        return Err(format!(
            "le poste {rowid} a changé d'état pendant la validation : rien n'a été écrit"
        )
        .into());
    }
    Ok(())
}

/// Remet le poste hors service : `1 -> 0` (`setDisabled()`, `setStatus(0)`).
///
/// Un poste hors service garde son historique : il ne se supprime pas, il sort du
/// planning. Refuse un poste déjà hors service, exactement comme [`valider`]
/// refuse un poste déjà en service : la transition doit changer quelque chose.
pub fn desactiver(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_du_poste(&mut conn, entity, rowid)?;
    if statut == STATUT_DESACTIVE {
        return Err(
            format!("le poste {rowid} est déjà hors service : il n'y a rien à désactiver").into(),
        );
    }
    if statut != STATUT_ACTIF {
        return Err(format!(
            "le poste {rowid} porte un statut inconnu ({statut}) : ni {STATUT_DESACTIVE} \
             ni {STATUT_ACTIF} — il ne se désactive pas par-dessus"
        )
        .into());
    }

    conn.exec_drop(
        "UPDATE llx_workstation_workstation SET status = :statut \
         WHERE entity = :entity AND rowid = :rowid AND status = :attendu",
        params! {
            "statut" => STATUT_DESACTIVE,
            "entity" => entity,
            "rowid" => rowid,
            "attendu" => STATUT_ACTIF,
        },
    )?;
    if conn.affected_rows() != 1 {
        return Err(format!(
            "le poste {rowid} a changé d'état pendant la désactivation : rien n'a été écrit"
        )
        .into());
    }
    Ok(())
}

/// Attache une machine (une ligne de `llx_resource`) au poste et rend le `rowid`
/// du lien.
///
/// Refuse : un poste absent de l'entité, une ressource absente de l'entité (même
/// si son `rowid` existe ailleurs — un lien vers une machine d'une autre société
/// ne serait rattachable à rien), et un doublon.
///
/// Le doublon est cherché par nous : `llx_workstation_workstation_resource` n'a
/// **aucun index unique** (seule la clé primaire, mesuré). Deux attachements
/// simultanés peuvent donc créer deux lignes, que Dolibarr lira deux fois. C'est
/// le seul endroit du module où la base ne peut pas nous couvrir.
pub fn attacher_ressource(
    pool: &Pool,
    entity: i64,
    fk_poste: i64,
    fk_ressource: i64,
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    statut_du_poste(&mut conn, entity, fk_poste)?;
    if !ressource_existe(&mut conn, entity, fk_ressource)? {
        return Err(format!(
            "la ressource {fk_ressource} n'existe pas dans l'entité {entity} : \
             on n'attache pas une machine fantôme"
        )
        .into());
    }

    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_workstation_workstation_resource \
         WHERE fk_workstation = :poste AND fk_resource = :ressource",
        params! { "poste" => fk_poste, "ressource" => fk_ressource },
    )?;
    if deja.is_some() {
        return Err(
            format!("la ressource {fk_ressource} est déjà attachée au poste {fk_poste}").into(),
        );
    }

    conn.exec_drop(
        "INSERT INTO llx_workstation_workstation_resource (fk_workstation, fk_resource) \
         VALUES (:poste, :ressource)",
        params! { "poste" => fk_poste, "ressource" => fk_ressource },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Détache une machine du poste. Refuse s'il n'y a pas de lien.
///
/// Le lien est cherché par jointure sur le poste, donc dans l'entité demandée
/// seulement : détacher depuis la société 1 le lien d'un poste de la société 7
/// échoue et ne détruit rien.
pub fn detacher_ressource(
    pool: &Pool,
    entity: i64,
    fk_poste: i64,
    fk_ressource: i64,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lien: Option<i64> = conn.exec_first(
        "SELECT wr.rowid FROM llx_workstation_workstation_resource wr \
         INNER JOIN llx_workstation_workstation w ON w.rowid = wr.fk_workstation \
         WHERE w.entity = :entity AND wr.fk_workstation = :poste \
           AND wr.fk_resource = :ressource",
        params! { "entity" => entity, "poste" => fk_poste, "ressource" => fk_ressource },
    )?;
    let Some(lien) = lien else {
        return Err(format!(
            "aucun lien entre le poste {fk_poste} et la ressource {fk_ressource} \
             dans l'entité {entity}"
        )
        .into());
    };

    conn.exec_drop(
        "DELETE FROM llx_workstation_workstation_resource WHERE rowid = :lien",
        params! { "lien" => lien },
    )?;
    if conn.affected_rows() != 1 {
        return Err(format!("le lien {lien} n'a pas pu être supprimé").into());
    }
    Ok(())
}

/// Attache un groupe d'utilisateurs au poste et rend le `rowid` du lien.
///
/// Même logique que [`attacher_ressource`] : le groupe doit exister dans
/// l'entité (`llx_usergroup.entity`), et le doublon est refusé par le code faute
/// d'index unique sur la jonction. Dolibarr s'en sert pour dire qui peut tenir le
/// poste (`WorkstationUserGroup::getAllGroupsOfWorkstation()`).
pub fn attacher_groupe(
    pool: &Pool,
    entity: i64,
    fk_poste: i64,
    fk_groupe: i64,
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    statut_du_poste(&mut conn, entity, fk_poste)?;
    if !groupe_existe(&mut conn, entity, fk_groupe)? {
        return Err(format!(
            "le groupe d'utilisateurs {fk_groupe} n'existe pas dans l'entité {entity}"
        )
        .into());
    }

    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_workstation_workstation_usergroup \
         WHERE fk_workstation = :poste AND fk_usergroup = :groupe",
        params! { "poste" => fk_poste, "groupe" => fk_groupe },
    )?;
    if deja.is_some() {
        return Err(format!("le groupe {fk_groupe} est déjà attaché au poste {fk_poste}").into());
    }

    conn.exec_drop(
        "INSERT INTO llx_workstation_workstation_usergroup (fk_workstation, fk_usergroup) \
         VALUES (:poste, :groupe)",
        params! { "poste" => fk_poste, "groupe" => fk_groupe },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Détache un groupe du poste. Refuse s'il n'y a pas de lien.
pub fn detacher_groupe(
    pool: &Pool,
    entity: i64,
    fk_poste: i64,
    fk_groupe: i64,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lien: Option<i64> = conn.exec_first(
        "SELECT wu.rowid FROM llx_workstation_workstation_usergroup wu \
         INNER JOIN llx_workstation_workstation w ON w.rowid = wu.fk_workstation \
         WHERE w.entity = :entity AND wu.fk_workstation = :poste \
           AND wu.fk_usergroup = :groupe",
        params! { "entity" => entity, "poste" => fk_poste, "groupe" => fk_groupe },
    )?;
    let Some(lien) = lien else {
        return Err(format!(
            "aucun lien entre le poste {fk_poste} et le groupe {fk_groupe} \
             dans l'entité {entity}"
        )
        .into());
    };

    conn.exec_drop(
        "DELETE FROM llx_workstation_workstation_usergroup WHERE rowid = :lien",
        params! { "lien" => lien },
    )?;
    if conn.affected_rows() != 1 {
        return Err(format!("le lien {lien} n'a pas pu être supprimé").into());
    }
    Ok(())
}

/// Les machines attachées au poste, dans l'ordre d'attachement.
///
/// `fk_resource` est *nullable* dans la table : une ligne sans cible est ignorée
/// plutôt que rendue comme un zéro, qui désignerait une ressource réelle.
pub fn ressources_de(pool: &Pool, entity: i64, fk_poste: i64) -> Result<Vec<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    statut_du_poste(&mut conn, entity, fk_poste)?;
    let lignes: Vec<Option<i64>> = conn.exec(
        "SELECT fk_resource FROM llx_workstation_workstation_resource \
         WHERE fk_workstation = :poste ORDER BY rowid ASC",
        params! { "poste" => fk_poste },
    )?;
    Ok(lignes.into_iter().flatten().collect())
}

/// Les groupes d'utilisateurs attachés au poste, dans l'ordre d'attachement.
pub fn groupes_de(pool: &Pool, entity: i64, fk_poste: i64) -> Result<Vec<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    statut_du_poste(&mut conn, entity, fk_poste)?;
    let lignes: Vec<Option<i64>> = conn.exec(
        "SELECT fk_usergroup FROM llx_workstation_workstation_usergroup \
         WHERE fk_workstation = :poste ORDER BY rowid ASC",
        params! { "poste" => fk_poste },
    )?;
    Ok(lignes.into_iter().flatten().collect())
}

/// Coût horaire du poste : **la somme de ses deux taux horaires estimés**
/// (`thm_operator_estimated + thm_machine_estimated`), arrondie au centime.
///
/// ### Ce que ce coût contient
///
/// Il contient la main-d'œuvre **et** la machine, parce que c'est ainsi que
/// Dolibarr chiffre : `BOM::calculateCosts()` écrit
/// `$line->total_cost = $qtyhourforline * ($workstation->thm_operator_estimated +
/// $workstation->thm_machine_estimated)` (`bom/class/bom.class.php`, ligne 1479),
/// et la vue de ligne fait la même somme terme à terme
/// (`bom/tpl/objectline_view.tpl.php`, lignes 208-213). Rendre une seule des deux
/// colonnes ferait diverger le coût affiché par ce module de celui que le noyau
/// calcule, ce qui est le pire des deux mondes.
///
/// ### Ce que ce coût ne contient pas, et pourquoi
///
/// **Il n'inclut pas les ressources attachées.** Ce n'est pas un oubli de
/// portage : `llx_resource` n'a aucune colonne de coût — ses 27 colonnes, comptées
/// dans `information_schema.columns`, sont l'identité, l'adresse, le contact, le
/// type, des notes et un drapeau de statut ; ni prix, ni tarif horaire, ni
/// valeur d'acquisition. Il n'existe donc rien à additionner. Inventer un tarif
/// par ressource demanderait une table nouvelle, et c'est une décision de schéma
/// qui n'appartient pas à un portage : elle serait perdue à la première mise à
/// jour de Dolibarr.
///
/// **Il n'est pas multiplié par `nb_operators_required`.** Le nombre d'opérateurs
/// est un effectif, pas un facteur de coût : le calcul de nomenclature cité
/// ci-dessus ne le lit même pas. Un poste qui exige trois personnes et coûte
/// 40 €/h vaut 40 €/h, pas 120 — les 40 € sont le coût du poste tel qu'il est
/// déjà renseigné, et celui qui l'a saisi savait combien de personnes il
/// comptait. Multiplier ici doublerait silencieusement tous les chiffrages.
///
/// ### Statut
///
/// Aucun contrôle de statut : un poste hors service garde son coût. C'est ce
/// coût-là qui justifie les prix déjà calculés ; le refuser rendrait illisible un
/// historique dont on a justement besoin. À l'appelant de lire `statut` s'il veut
/// n'afficher que les postes en service.
pub fn cout_horaire(pool: &Pool, entity: i64, fk_poste: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let somme: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(thm_operator_estimated, 0) \
                     + COALESCE(thm_machine_estimated, 0) AS CHAR) \
         FROM llx_workstation_workstation \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_poste },
    )?;
    let Some(somme) = somme else {
        return Err(format!("le poste {fk_poste} n'existe pas dans l'entité {entity}").into());
    };

    // Colonne `double` : on lit la chaîne que MySQL rend et on l'arrondit une
    // seule fois, au centime — jamais de `f64` intermédiaire, sinon 0,1 + 0,2
    // ferait 0,30000000000000004 et le coût d'un poste ne tomberait jamais juste.
    let brut = Decimal::from_str(somme.trim())?;
    Ok(round_money(brut))
}
