//! Ressources (`llx_resource`) et liens vers les objets métier
//! (`llx_element_resources`).
//!
//! Une ressource, c'est un moyen matériel qu'une société se partage : une salle,
//! un fourgon, une imprimante, un échafaudage. Dolibarr la range dans
//! `llx_resource` ; le lien qui l'attache à une proposition, une commande, une
//! facture, un projet, un ticket, un contrat ou une intervention vit dans
//! `llx_element_resources`, une table de jonction polymorphe.
//!
//! ## Pas de `suivant_ref` : la référence d'une ressource est un nom
//!
//! Partout ailleurs dans le noyau — devis, factures, projets — la référence est
//! un compteur (`PR2611-0004`, `PRJ2611-0002-1`) attribué par le module. **Ici,
//! non.** `llx_resource.ref` porte ce que l'entreprise appelle la ressource :
//! un numéro d'inventaire (`INV-2026-014`), une immatriculation, un nom de
//! machine (`Fourgon 1`). C'est une saisie, pas un calcul : la numéroter
//! automatiquement effacerait le seul identifiant que le magasinier reconnaît.
//! Dolibarr ne la numérote pas non plus (le module Ressources n'a pas de
//! `documents` de numérotation) et la colonne est même *nullable*.
//!
//! Conséquence directe : c'est la **clé unique** `uk_resource_ref (ref, entity)`
//! qui tient l'unicité, et non un compteur. Deux ressources du même nom sont
//! refusées par la base ; `creer()` traduit l'erreur 1062 en message lisible
//! plutôt que de laisser remonter « Duplicate entry ».
//!
//! ## `llx_element_resources` n'a pas de colonne `entity`
//!
//! C'est le point à ne pas manquer. La table de jonction ne connaît que
//! `resource_id`, `resource_type`, `element_id` et `element_type` : elle est
//! donc **globale**, tous cloisonnements confondus. Une ressource de la société 7
//! et une ressource de la société 1 portent des `rowid` qui ne se recoupent pas,
//! mais rien, dans la jonction, ne dit à quelle société appartient le lien.
//!
//! D'où la règle unique de ce module : **toute lecture et toute écriture de lien
//! joint `llx_resource`**. Sans cette jointure (`INNER JOIN llx_resource r ON
//! r.rowid = er.resource_id ... WHERE r.entity = :entity`), une ressource d'une
//! autre société sortirait avec le bon identifiant, et l'écran d'une société
//! afficherait les liens d'une autre. C'est le sens du paramètre `entity` de
//! `attacher`, `detacher`, `ressources_de` et `combien_de_lies` : il ne filtre
//! pas la jonction — il la rend possible.
//!
//! Corollaire : `attacher` refuse une ressource absente de l'entité demandée,
//! même si son `rowid` existe ailleurs. Attacher depuis la société 1 une
//! ressource de la société 7 produirait un lien que personne ne pourrait
//! rattacher à quoi que ce soit.
//!
//! ## Ce que Dolibarr écrit dans `llx_element_resources`
//!
//! Vérifié dans le source 24.0.1 (`CommonObject::add_element_resource()`,
//! `resource/element_resource.php`, `core/tpl/resource_add.tpl.php`) :
//!
//! - `resource_type` vaut **`dolresource`** — la valeur par défaut du formulaire
//!   (`empty($resource_type) ? 'dolresource' : $resource_type`) et celle sur
//!   laquelle `element_resource.php` joint. On écrit donc [`TYPE_RESSOURCE`] ;
//!   une autre valeur rendrait le lien invisible dans Dolibarr.
//! - `element_type` reçoit `$object->element`, c'est-à-dire le nom d'élément de
//!   la classe : `propal`, `commande`, `facture`, `project`, `ticket`, `contrat`,
//!   `fichinter`. Voir plus bas le cas `projet` / `project`.
//! - `busy` et `mandatory` valent 0 par défaut ; c'est l'agenda qui les
//!   renseigne ensuite. `attacher()` les écrit à 0.
//! - `duree` n'est **jamais** renseignée par Dolibarr (la méthode n'en prend
//!   même pas le paramètre) : cette colonne n'a d'autre écrivain que ce module.
//!   Elle est de type `double` — donc approximative au-delà de quinze chiffres
//!   significatifs : on écrit la chaîne du `Decimal` et on la relit par
//!   `CAST(... AS CHAR)`, comme tous les montants du noyau.
//! - `fk_user_create` reste `NULL` : Dolibarr ne l'écrit pas non plus.
//! - Dolibarr ne vérifie **pas** que l'élément visé existe : il insère le couple
//!   (`element_id`, `element_type`) tel quel. On ne fait pas mieux, et pour la
//!   même raison — il faudrait, pour chaque type, connaître sa table. Un lien
//!   orphelin est donc possible ; il est silencieusement ignoré à la lecture,
//!   puisque Dolibarr joint en `INNER JOIN`.
//!
//! ## Le cas `projet` / `project`
//!
//! Le mot français est `projet`, le nom d'élément de Dolibarr est `project`
//! (`Project::$element = 'project'`). Les deux sont acceptés à l'entrée, et
//! c'est **`project`** qui est écrit : c'est la valeur sur laquelle
//! `element_resource.php` joint (`er.element_type = $objstat->element`). Écrire
//! `projet` créerait un lien que Dolibarr ne verrait pas, et deux orthographes
//! du même élément contourneraient la clé unique
//! `idx_element_resources_idx1 (resource_id, resource_type, element_id,
//! element_type)`.
//!
//! ## Statut
//!
//! `creer()` pose `fk_statut = 1` (ressource validée, donc utilisable) et
//! `datec = NOW()`. `date_valid` et `fk_user_valid` restent `NULL` : la création
//! n'est pas une validation par un utilisateur, et Dolibarr les remplit dans
//! `Dolresource::validate()`. Les inventer ferait croire à une validation qui
//! n'a pas eu lieu.
//!
//! ## Champs hors périmètre
//!
//! `photo_filename`, `note_public`, `note_private`, `fk_country`, `fk_state`,
//! `import_key` et `extraparams` ne sont ni lus ni écrits : la fiche de ce module
//! couvre l'identité, le contact et le type. Un `modifier()` ne les écrase donc
//! pas — il n'y touche pas du tout, ce qui est le comportement voulu pour des
//! champs alimentés ailleurs.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, PooledConn, Row};
use rust_decimal::Decimal;

/// Longueur maximale de `llx_resource.ref`, en **octets**.
///
/// La colonne est un `varchar(255)`, qui compte des *caractères* : la règle en
/// octets est donc plus stricte, jamais plus permissive. C'est voulu — un nom
/// d'inventaire de 255 caractères accentués n'a aucun usage, et la borne basse
/// évite de dépendre du jeu de caractères de la colonne.
pub const REF_MAX: usize = 255;

/// Valeur de `llx_element_resources.resource_type` pour une ressource du module
/// Ressources de Dolibarr. Voir la doc du module.
pub const TYPE_RESSOURCE: &str = "dolresource";

/// Statut d'une ressource créée par ce module : validée (`llx_resource.fk_statut`).
pub const STATUT_VALIDE: i16 = 1;

/// Les types d'élément auxquels une ressource peut être attachée, tels qu'on les
/// saisit. Sert aussi aux messages d'erreur.
pub const TYPES_ELEMENT: &[&str] = &[
    "propal",
    "commande",
    "facture",
    "projet",
    "ticket",
    "contrat",
    "fichinter",
];

/// Les noms acceptés et la valeur réellement écrite en base.
///
/// Une table fermée, et non une liste de chaînes recopiées : `projet` et
/// `project` désignent le même élément et doivent atterrir sur la même valeur,
/// sinon la clé unique ne verrait pas le doublon.
const ELEMENTS: &[(&str, &str)] = &[
    ("propal", "propal"),
    ("commande", "commande"),
    ("facture", "facture"),
    ("projet", "project"),
    ("project", "project"),
    ("ticket", "ticket"),
    ("contrat", "contrat"),
    ("fichinter", "fichinter"),
];

/// Une ressource, telle qu'affichée en liste ou reprise en formulaire.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ressource {
    pub rowid: i64,
    pub entity: i64,
    /// Nom de la ressource (`ref`) : saisie, jamais un compteur.
    pub ref_: String,
    /// Numéro d'inventaire ou de série.
    pub asset_number: Option<String>,
    pub description: Option<String>,
    /// Code du type, tel que stocké (`llx_c_type_resource.code`).
    pub fk_code_type_resource: Option<String>,
    /// Libellé lisible du type, joint à la lecture : « Voitures », « Salles ».
    pub type_label: Option<String>,
    pub address: Option<String>,
    pub zip: Option<String>,
    pub town: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub url: Option<String>,
    /// Nombre maximal d'utilisateurs simultanés (`max_users`), `NULL` si libre.
    pub max_users: Option<i32>,
    /// `1` = validée. Voir [`STATUT_VALIDE`].
    pub fk_statut: i16,
    /// Date de création, `AAAA-MM-JJ HH:MM:SS`.
    pub datec: Option<String>,
    pub fk_user_author: Option<i64>,
    /// Durée d'immobilisation du lien, en heures.
    ///
    /// **Renseignée par [`ressources_de`] uniquement** : c'est une colonne de
    /// `llx_element_resources`, elle n'existe pas sur la fiche de la ressource.
    /// `list()` et `get()` laissent `None`.
    pub duree: Option<Decimal>,
    /// Vrai si la ressource est occupée sur ce lien.
    ///
    /// Même remarque que `duree` : colonne de la jonction, donc `false` quand la
    /// ressource vient de `list()` ou `get()`. Écrite à 0 par [`attacher`].
    pub busy: bool,
}

/// Ce qu'il faut pour créer une ressource.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewRessource {
    /// Nom de la ressource (`ref`), obligatoire, 255 octets au plus. Le suffixe
    /// `_` est imposé : `ref` est un mot-clé Rust.
    pub ref_: String,
    pub asset_number: Option<String>,
    pub description: Option<String>,
    /// Code d'un type de `llx_c_type_resource`. S'il est fourni, il doit exister
    /// et être actif. `None` laisse la colonne à `NULL`, ce que Dolibarr accepte.
    pub fk_code_type_resource: Option<String>,
    pub address: Option<String>,
    pub zip: Option<String>,
    pub town: Option<String>,
    pub phone: Option<String>,
    /// Adresse électronique : refusée si elle ne contient pas d'arobase.
    pub email: Option<String>,
    /// Adresse web : refusée si elle ne commence pas par `http`.
    pub url: Option<String>,
    /// Nombre maximal d'utilisateurs, jamais négatif.
    pub max_users: Option<i32>,
    /// Auteur de la fiche. Doit être strictement positif.
    ///
    /// `llx_resource.fk_user_author` ne porte **aucune** clé étrangère vers
    /// `llx_user` (vérifié au DDL 24.0.1) : un identifiant inexistant passerait
    /// donc sans bruit. On vérifie la positivité, pas l'existence — c'est au
    /// formulaire appelant de proposer une liste d'utilisateurs réels.
    pub fk_user_author: i64,
}

/// Ce qui peut changer sur une ressource existante.
///
/// **La référence n'y figure pas**, et c'est le point : elle est saisie une fois
/// (numéro d'inventaire, nom de machine), puis imprimée sur les fiches et citée
/// dans les échanges. L'impossibilité de la modifier est portée par le type, pas
/// par une consigne en commentaire qu'un appelant distrait ignorerait.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModifRessource {
    pub asset_number: Option<String>,
    pub description: Option<String>,
    pub fk_code_type_resource: Option<String>,
    pub address: Option<String>,
    pub zip: Option<String>,
    pub town: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub url: Option<String>,
    pub max_users: Option<i32>,
}

const SELECT_COLS: &str = "r.rowid, r.entity, r.ref, r.asset_number, r.description, \
     r.fk_code_type_resource, t.label, r.address, r.zip, r.town, r.phone, \
     r.email, r.url, r.max_users, r.fk_statut, \
     DATE_FORMAT(r.datec, '%Y-%m-%d %H:%i:%s'), r.fk_user_author";

/// Colonnes de `SELECT_COLS` plus celles de la jonction, pour `ressources_de`.
const SELECT_COLS_LIES: &str = "r.rowid, r.entity, r.ref, r.asset_number, r.description, \
     r.fk_code_type_resource, t.label, r.address, r.zip, r.town, r.phone, \
     r.email, r.url, r.max_users, r.fk_statut, \
     DATE_FORMAT(r.datec, '%Y-%m-%d %H:%i:%s'), r.fk_user_author, \
     CAST(er.duree AS CHAR), er.busy";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: &Row) -> Ressource {
    Ressource {
        rowid: opt_i(row, 0).unwrap_or(0),
        entity: opt_i(row, 1).unwrap_or(1),
        ref_: opt_s(row, 2).unwrap_or_default(),
        asset_number: opt_s(row, 3),
        description: opt_s(row, 4),
        fk_code_type_resource: opt_s(row, 5),
        type_label: opt_s(row, 6),
        address: opt_s(row, 7),
        zip: opt_s(row, 8),
        town: opt_s(row, 9),
        phone: opt_s(row, 10),
        email: opt_s(row, 11),
        url: opt_s(row, 12),
        max_users: opt_i(row, 13).map(|n| n as i32),
        fk_statut: opt_i(row, 14).unwrap_or(0) as i16,
        datec: opt_s(row, 15),
        fk_user_author: opt_i(row, 16),
        duree: None,
        busy: false,
    }
}

/// Une ressource lue avec son lien : les deux dernières colonnes de
/// `SELECT_COLS_LIES` sont `duree` et `busy`.
fn map_row_lie(row: &Row) -> Ressource {
    let mut res = map_row(row);
    res.duree = opt_dec(row, 17);
    res.busy = opt_i(row, 18).unwrap_or(0) != 0;
    res
}

/// Une saisie facultative vide vaut « non renseigné » : un formulaire HTML
/// renvoie toujours une chaîne, jamais l'absence. On n'écrit donc jamais `''`
/// dans une colonne qu'on aurait laissée à `NULL`.
fn texte(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|s| !s.is_empty())
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

/// Traduit un nom d'élément saisi en la valeur que Dolibarr écrit. `None` si le
/// type n'est pas dans la table fermée `ELEMENTS`.
fn type_element(saisie: &str) -> Option<&'static str> {
    let saisi = saisie.trim().to_ascii_lowercase();
    ELEMENTS
        .iter()
        .find(|(nom, _)| *nom == saisi)
        .map(|(_, ecrit)| *ecrit)
}

/// Vérifie qu'un code de type existe et est actif, et rend le code **tel que le
/// dictionnaire l'écrit** : `res_rooms` saisi en minuscules est enregistré
/// `RES_ROOMS`, comme dans `llx_c_type_resource`.
fn type_ressource(
    conn: &mut PooledConn,
    code: Option<&str>,
) -> Result<Option<String>, Box<dyn Error>> {
    let Some(code) = code else {
        return Ok(None);
    };
    let ligne: Option<(String, i64)> = conn.exec_first(
        "SELECT code, active FROM llx_c_type_resource WHERE code = :modele_type",
        params! { "modele_type" => code },
    )?;
    match ligne {
        None => Err(format!(
            "le type de ressource « {code} » n'existe pas dans llx_c_type_resource"
        )
        .into()),
        Some((_, 0)) => Err(format!(
            "le type de ressource « {code} » est désactivé (active = 0) : \
             il ne peut pas être affecté à une ressource"
        )
        .into()),
        Some((canonique, _)) => Ok(Some(canonique)),
    }
}

fn verifier_email(email: Option<&str>) -> Result<(), Box<dyn Error>> {
    if let Some(e) = email {
        if !e.contains('@') {
            return Err(format!(
                "l'adresse « {e} » n'est pas une adresse électronique : il manque l'arobase"
            )
            .into());
        }
    }
    Ok(())
}

fn verifier_url(url: Option<&str>) -> Result<(), Box<dyn Error>> {
    if let Some(u) = url {
        if !u.to_ascii_lowercase().starts_with("http") {
            return Err(format!(
                "l'adresse web « {u} » doit commencer par http (http:// ou https://)"
            )
            .into());
        }
    }
    Ok(())
}

fn verifier_max_users(max_users: Option<i32>) -> Result<(), Box<dyn Error>> {
    if let Some(n) = max_users {
        if n < 0 {
            return Err(
                format!("le nombre maximal d'utilisateurs ({n}) ne peut pas être négatif").into(),
            );
        }
    }
    Ok(())
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

/// Ressources de l'entité, par ordre alphabétique de référence.
///
/// L'ordre est celui d'un inventaire, pas celui des dernières fiches créées :
/// on cherche une machine par son nom, pas par sa date de saisie.
pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Ressource>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_resource r \
         LEFT JOIN llx_c_type_resource t ON t.code = r.fk_code_type_resource \
         WHERE r.entity = :entity \
         ORDER BY r.ref ASC, r.rowid ASC LIMIT :limit OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limit" => limit, "offset" => offset },
    )?;
    Ok(rows.iter().map(map_row).collect())
}

/// Une ressource précise, ou `None` si elle n'existe pas dans l'entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Ressource>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_resource r \
         LEFT JOIN llx_c_type_resource t ON t.code = r.fk_code_type_resource \
         WHERE r.entity = :entity AND r.rowid = :rowid"
    );
    let row: Option<Row> = conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.as_ref().map(map_row))
}

/// Crée une ressource et rend son `rowid`.
///
/// Refuse, **avant toute écriture** : une référence vide ou de plus de
/// [`REF_MAX`] octets, une référence déjà prise dans l'entité, un type absent ou
/// désactivé de `llx_c_type_resource` (le message cite le code refusé), une
/// adresse électronique sans arobase, une adresse web qui ne commence pas par
/// `http`, un `max_users` négatif et un auteur non strictement positif.
///
/// `fk_statut` vaut 1 et `datec` `NOW()` ; `date_valid` et `fk_user_valid`
/// restent `NULL` (voir la doc du module).
pub fn creer(pool: &Pool, entity: i64, new: &NewRessource) -> Result<i64, Box<dyn Error>> {
    let reference = new.ref_.trim();
    if reference.is_empty() {
        return Err("la référence de la ressource est obligatoire".into());
    }
    if reference.len() > REF_MAX {
        return Err(format!(
            "la référence de la ressource dépasse {REF_MAX} octets ({} reçus)",
            reference.len()
        )
        .into());
    }
    if new.fk_user_author <= 0 {
        return Err(format!(
            "l'auteur de la ressource (fk_user_author = {}) est invalide",
            new.fk_user_author
        )
        .into());
    }

    let email = texte(new.email.as_deref());
    verifier_email(email)?;
    let url = texte(new.url.as_deref());
    verifier_url(url)?;
    verifier_max_users(new.max_users)?;

    let mut conn = pool.get_conn()?;
    let modele = type_ressource(&mut conn, texte(new.fk_code_type_resource.as_deref()))?;

    let insere = conn.exec_drop(
        "INSERT INTO llx_resource \
         (entity, ref, asset_number, description, fk_code_type_resource, address, zip, \
          town, phone, email, url, max_users, datec, fk_user_author, fk_statut) \
         VALUES \
         (:entity, :reference, :asset_number, :description, :modele_type, :address, :zip, \
          :town, :phone, :email, :url, :max_users, NOW(), :auteur, :statut)",
        params! {
            "entity" => entity,
            "reference" => reference,
            "asset_number" => texte(new.asset_number.as_deref()),
            "description" => texte(new.description.as_deref()),
            "modele_type" => modele.as_deref(),
            "address" => texte(new.address.as_deref()),
            "zip" => texte(new.zip.as_deref()),
            "town" => texte(new.town.as_deref()),
            "phone" => texte(new.phone.as_deref()),
            "email" => email,
            "url" => url,
            "max_users" => new.max_users,
            "auteur" => new.fk_user_author,
            "statut" => STATUT_VALIDE,
        },
    );

    match insere {
        Ok(()) => Ok(conn.last_insert_id() as i64),
        Err(e) if is_duplicate(&e) => Err(format!(
            "une ressource porte déjà la référence « {reference} » dans l'entité {entity}"
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

/// Met à jour une ressource existante. **La référence n'est jamais modifiée** :
/// elle n'est même pas dans [`ModifRessource`].
///
/// Les mêmes contrôles qu'à la création s'appliquent (type actif, arobase,
/// `http`, `max_users` positif ou nul). Les colonnes hors périmètre — notes,
/// photo, pays, `extraparams` — ne sont pas touchées : le `UPDATE` ne les cite
/// pas.
pub fn modifier(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    modif: &ModifRessource,
) -> Result<(), Box<dyn Error>> {
    let email = texte(modif.email.as_deref());
    verifier_email(email)?;
    let url = texte(modif.url.as_deref());
    verifier_url(url)?;
    verifier_max_users(modif.max_users)?;

    let mut conn = pool.get_conn()?;
    if !ressource_existe(&mut conn, entity, rowid)? {
        return Err(format!("la ressource {rowid} n'existe pas dans l'entité {entity}").into());
    }
    let modele = type_ressource(&mut conn, texte(modif.fk_code_type_resource.as_deref()))?;

    conn.exec_drop(
        "UPDATE llx_resource SET asset_number = :asset_number, \
          description = :description, fk_code_type_resource = :modele_type, \
          address = :address, zip = :zip, town = :town, phone = :phone, \
          email = :email, url = :url, max_users = :max_users \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "asset_number" => texte(modif.asset_number.as_deref()),
            "description" => texte(modif.description.as_deref()),
            "modele_type" => modele.as_deref(),
            "address" => texte(modif.address.as_deref()),
            "zip" => texte(modif.zip.as_deref()),
            "town" => texte(modif.town.as_deref()),
            "phone" => texte(modif.phone.as_deref()),
            "email" => email,
            "url" => url,
            "max_users" => modif.max_users,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Attache une ressource à un élément métier et rend le `rowid` du lien.
///
/// Refuse : une ressource absente de l'entité, un type d'élément hors de
/// [`TYPES_ELEMENT`], un `element_id` non strictement positif, une durée
/// négative, et un doublon **exact** (même ressource, même élément, même type
/// d'élément). Le doublon est d'abord cherché, puis la clé unique
/// `idx_element_resources_idx1` sert de filet en cas de course entre deux
/// clients : l'erreur 1062 est traduite en message lisible, pas laissée brute.
///
/// `entity` ne filtre pas la jonction (elle n'en a pas) : il désigne la société à
/// laquelle la ressource doit appartenir. Voir la doc du module.
///
/// `busy` et `mandatory` sont écrits à 0, comme Dolibarr ; c'est l'agenda qui
/// les renseigne ensuite. `fk_user_create` reste `NULL`.
pub fn attacher(
    pool: &Pool,
    entity: i64,
    fk_ressource: i64,
    element_id: i64,
    element_type: &str,
    duree: Option<Decimal>,
) -> Result<i64, Box<dyn Error>> {
    let Some(type_element) = type_element(element_type) else {
        return Err(format!(
            "le type d'élément « {element_type} » n'est pas géré : {}",
            TYPES_ELEMENT.join(", ")
        )
        .into());
    };
    if element_id <= 0 {
        return Err(format!("l'élément à attacher ({element_id}) est invalide").into());
    }
    if let Some(d) = duree {
        if d < Decimal::ZERO {
            return Err(format!(
                "la durée d'attachement ({d}) ne peut pas être négative"
            )
            .into());
        }
    }

    let mut conn = pool.get_conn()?;
    if !ressource_existe(&mut conn, entity, fk_ressource)? {
        return Err(format!(
            "la ressource {fk_ressource} n'existe pas dans l'entité {entity}"
        )
        .into());
    }

    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_element_resources \
         WHERE resource_id = :ressource AND resource_type = :type_ressource \
           AND element_id = :element AND element_type = :type_element",
        params! {
            "ressource" => fk_ressource,
            "type_ressource" => TYPE_RESSOURCE,
            "element" => element_id,
            "type_element" => type_element,
        },
    )?;
    if deja.is_some() {
        return Err(format!(
            "la ressource {fk_ressource} est déjà attachée à l'élément \
             {type_element} {element_id}"
        )
        .into());
    }

    let insere = conn.exec_drop(
        "INSERT INTO llx_element_resources \
         (resource_id, resource_type, element_id, element_type, busy, mandatory, duree) \
         VALUES (:ressource, :type_ressource, :element, :type_element, 0, 0, :duree)",
        params! {
            "ressource" => fk_ressource,
            "type_ressource" => TYPE_RESSOURCE,
            "element" => element_id,
            "type_element" => type_element,
            "duree" => duree.map(|d| d.to_string()),
        },
    );

    match insere {
        Ok(()) => Ok(conn.last_insert_id() as i64),
        Err(e) if is_duplicate(&e) => Err(format!(
            "la ressource {fk_ressource} est déjà attachée à l'élément \
             {type_element} {element_id}"
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

/// Détache une ressource d'un élément. Refuse s'il n'y a pas de lien.
///
/// Le lien est cherché par jointure sur `llx_resource`, donc dans l'entité
/// demandée seulement : détacher depuis la société 1 le lien d'une ressource de
/// la société 7 échoue, et ne détruit rien.
pub fn detacher(
    pool: &Pool,
    entity: i64,
    fk_ressource: i64,
    element_id: i64,
    element_type: &str,
) -> Result<(), Box<dyn Error>> {
    let Some(type_element) = type_element(element_type) else {
        return Err(format!(
            "le type d'élément « {element_type} » n'est pas géré : {}",
            TYPES_ELEMENT.join(", ")
        )
        .into());
    };

    let mut conn = pool.get_conn()?;
    let lien: Option<i64> = conn.exec_first(
        "SELECT er.rowid FROM llx_element_resources er \
         INNER JOIN llx_resource r ON r.rowid = er.resource_id \
         WHERE r.entity = :entity AND er.resource_id = :ressource \
           AND er.resource_type = :type_ressource AND er.element_id = :element \
           AND er.element_type = :type_element",
        params! {
            "entity" => entity,
            "ressource" => fk_ressource,
            "type_ressource" => TYPE_RESSOURCE,
            "element" => element_id,
            "type_element" => type_element,
        },
    )?;
    let Some(lien) = lien else {
        return Err(format!(
            "aucun lien entre la ressource {fk_ressource} et l'élément \
             {type_element} {element_id} dans l'entité {entity}"
        )
        .into());
    };

    conn.exec_drop(
        "DELETE FROM llx_element_resources WHERE rowid = :lien",
        params! { "lien" => lien },
    )?;
    if conn.affected_rows() != 1 {
        return Err(format!("le lien {lien} n'a pas pu être supprimé").into());
    }
    Ok(())
}

/// Ressources attachées à un élément, avec la durée et le drapeau `busy` de
/// chaque lien.
///
/// `element_type` accepte les deux orthographes du projet (`projet` et
/// `project`) ; la valeur interrogée est celle que Dolibarr écrit.
///
/// La jointure sur `llx_resource` n'est pas décorative : `llx_element_resources`
/// n'a pas d'entité, et `resource_id` est unique toutes sociétés confondues —
/// c'est `r.entity` qui cloisonne. Voir la doc du module.
pub fn ressources_de(
    pool: &Pool,
    entity: i64,
    element_id: i64,
    element_type: &str,
) -> Result<Vec<Ressource>, Box<dyn Error>> {
    let Some(type_element) = type_element(element_type) else {
        return Err(format!(
            "le type d'élément « {element_type} » n'est pas géré : {}",
            TYPES_ELEMENT.join(", ")
        )
        .into());
    };
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS_LIES} FROM llx_element_resources er \
         INNER JOIN llx_resource r ON r.rowid = er.resource_id \
         LEFT JOIN llx_c_type_resource t ON t.code = r.fk_code_type_resource \
         WHERE r.entity = :entity AND er.resource_type = :type_ressource \
           AND er.element_id = :element AND er.element_type = :type_element \
         ORDER BY r.ref ASC, r.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! {
            "entity" => entity,
            "type_ressource" => TYPE_RESSOURCE,
            "element" => element_id,
            "type_element" => type_element,
        },
    )?;
    Ok(rows.iter().map(map_row_lie).collect())
}

/// Nombre de liens qui partent d'une ressource, toutes entités exclues : la
/// ressource doit appartenir à l'entité demandée, sinon la question n'a pas de
/// sens et la fonction le dit.
///
/// Sert à refuser la suppression d'une ressource encore attachée.
pub fn combien_de_lies(
    pool: &Pool,
    entity: i64,
    fk_ressource: i64,
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    if !ressource_existe(&mut conn, entity, fk_ressource)? {
        return Err(format!(
            "la ressource {fk_ressource} n'existe pas dans l'entité {entity}"
        )
        .into());
    }
    let total: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_element_resources \
         WHERE resource_id = :ressource AND resource_type = :type_ressource",
        params! { "ressource" => fk_ressource, "type_ressource" => TYPE_RESSOURCE },
    )?;
    Ok(total.unwrap_or(0))
}
