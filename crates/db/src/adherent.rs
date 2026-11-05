//! Adhérents d'une association (`llx_adherent`, `llx_adherent_type`).
//!
//! Un adhérent, c'est la fiche d'un membre : identité, coordonnées, type
//! d'adhésion, date de fin de cotisation. Dolibarr y accroche un espace membre
//! (le membre s'y connecte avec son `login` et un mot de passe), une
//! numérotation de référence, et, en aval, `llx_subscription` qui garde
//! l'historique des cotisations.
//!
//! # Les statuts
//!
//! Dolibarr les numérote dans `Adherent::$statuts` :
//!
//! | valeur | sens |
//! |---|---|
//! | -1 | brouillon : fiche saisie, pas encore validée |
//! | 0 | résilié — ici employé comme **suspendu**, donc réversible |
//! | 1 | à jour : adhésion validée |
//! | -2 | exclu (Dolibarr 24 ; jamais écrit par ce module) |
//!
//! Dolibarr n'a qu'un code pour « hors adhésion » : 0. [`suspendre`] et
//! [`reactiver`] s'en servent comme d'un interrupteur, ce qui est exactement ce
//! que fait `Adherent::resiliate()` puis `Adherent::validate()` en PHP.
//!
//! # Le mot de passe : pourquoi il n'est pas dans [`NewAdherent`]
//!
//! Un adhérent peut avoir un accès à l'espace membre, donc un mot de passe. Ce
//! module **n'en accepte aucun** dans son constructeur, et c'est volontaire : un
//! constructeur circule en clair dans le code d'écran, les journaux et les
//! messages d'erreur, et un mot de passe qui traîne là finit par se retrouver
//! dans une trace. La mise en place est donc une opération séparée,
//! [`definir_mot_de_passe`], qui ne reçoit **qu'une empreinte déjà calculée**
//! par l'appelant.
//!
//! L'appelant produit cette empreinte avec `auth::hash` (bcrypt `$2y$`, celui de
//! Dolibarr), exactement comme `user::definir_mot_de_passe` le fait pour un
//! utilisateur : même format, donc un adhérent créé ici se connecte aussi bien
//! depuis Dolibarr que depuis ce noyau.
//!
//! Ce qui est écrit en base :
//!
//! - `pass_crypted` reçoit l'empreinte — c'est la colonne que Dolibarr relit
//!   (`Adherent::setPassword` y écrit `dol_hash($password)`, et `fetch()` la
//!   remonte dans `pass_indatabase_crypted`) ;
//! - `pass` est remis à `NULL` : Dolibarr n'y met le mot de passe **en clair**
//!   que pour les comptes qu'il crée en mode non chiffré. Laisser une valeur
//!   dans cette colonne serait offrir le mot de passe à qui lit la base.
//!
//! # La référence, et son écart avec Dolibarr
//!
//! `llx_adherent.ref` est un `varchar(30)` NOT NULL, unique avec l'entité
//! (`uk_adherent_ref`). La numérotation passe par [`divers::suivant_ref`]
//! (`AD0001`, `AD0002`…), comme les congés (`CP…`) et les tickets (`TK…`).
//!
//! ⚠️ Le module de numérotation par défaut de Dolibarr, `mod_member_simple`,
//! n'écrit **pas** de préfixe : la référence d'un adhérent y est son `rowid`
//! (`1`, `2`, `3`…), et son contrôle d'unicité (`canBeUsed`) suppose des
//! références purement numériques. Sur une instance où les deux écrivent, le
//! module PHP signalera donc un conflit de numérotation. C'est un choix assumé :
//! un `rowid` inconnu avant l'insertion ne peut pas servir de référence à la
//! création, alors que `AD0001` est stable, lisible et tient dans la colonne.
//!
//! # Ce que ce module ne fait pas
//!
//! - **Aucune cotisation n'est enregistrée.** [`cotiser`] prolonge l'adhésion
//!   (`datefin`) et remet le statut à jour, mais n'écrit pas de ligne dans
//!   `llx_subscription`. Cette table porte le montant, la date d'adhésion et le
//!   rattachement bancaire ; une ligne sans montant serait une pièce comptable
//!   fausse. Elle viendra avec l'écran qui saisit le règlement, pas avant.
//! - **Aucune personne morale n'est créée** : [`create`] écrit `morphy = 'phy'`.
//!   [`NewAdherent`] ne porte pas la forme juridique, et la deviner à partir de
//!   la présence d'un `societe` serait faux (une personne physique peut
//!   adhérer au nom de sa société). La constante [`MORPHY_MORALE`] existe pour
//!   le jour où l'écran posera la question.
//! - **Aucun montant n'est manipulé ici** : les tarifs vivent dans
//!   `llx_adherent_type` (`amount`, `minimumamount`) et se lisent côté écran.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool, Row};

use crate::divers;

/// Préfixe de référence des adhérents : `AD0001`, `AD0002`…
pub const PREFIX: &str = "AD";

/// Personne physique : un particulier qui adhère à titre individuel.
pub const MORPHY_PHYSIQUE: &str = "phy";
/// Personne morale : une association ou une société adhérente.
pub const MORPHY_MORALE: &str = "mor";

/// Brouillon : la fiche existe mais n'a pas été validée (`STATUS_DRAFT`).
pub const STATUT_BROUILLON: i16 = -1;
/// Résilié ou suspendu : l'adhésion ne court plus. Un seul code pour les deux,
/// comme dans Dolibarr (`STATUS_RESILIATED`).
pub const STATUT_SUSPENDU: i16 = 0;
/// À jour : adhésion validée (`STATUS_VALIDATED`).
pub const STATUT_A_JOUR: i16 = 1;

/// Un adhérent tel qu'il se lit en base, libellé de son type compris.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Adherent {
    pub rowid: i64,
    pub entity: i64,
    /// Référence de l'adhérent (`AD0001`).
    pub ref_: String,
    /// `gender` : code libre de Dolibarr, jamais imposé par ce module.
    pub genre: Option<String>,
    /// `civility` : `MR`, `MME`…
    pub civilite: Option<String>,
    /// `lastname` — le nom, obligatoire.
    pub nom: String,
    /// `firstname` — facultatif : une association adhérente n'a pas de prénom.
    pub prenom: Option<String>,
    /// Identifiant de connexion à l'espace membre, facultatif et unique par
    /// entité (`uk_adherent_login`).
    pub login: Option<String>,
    pub fk_adherent_type: i64,
    /// Libellé du type (`llx_adherent_type.libelle`).
    pub type_libelle: Option<String>,
    /// `morphy` : [`MORPHY_PHYSIQUE`] ou [`MORPHY_MORALE`].
    pub morphy: String,
    /// Raison sociale déclarée par l'adhérent, personne physique comprise.
    pub societe: Option<String>,
    pub adresse: Option<String>,
    pub cp: Option<String>,
    pub ville: Option<String>,
    /// `country` : identifiant du dictionnaire `llx_c_country`.
    pub pays: Option<i64>,
    pub email: Option<String>,
    pub url: Option<String>,
    pub telephone: Option<String>,
    pub portable: Option<String>,
    /// `birth`, au format `AAAA-MM-JJ`.
    pub naissance: Option<String>,
    pub statut: i16,
    /// Fiche publiée dans l'annuaire public des membres.
    pub public: bool,
    /// `datefin` : dernier jour couvert par la cotisation, `NULL` si aucune
    /// adhésion n'a encore été réglée.
    pub date_fin: Option<String>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    /// `datevalid` : moment de la validation (statut passé à 1).
    pub date_validation: Option<String>,
    pub date_creation: Option<String>,
}

impl Adherent {
    /// « Prénom Nom », ou le nom seul quand il n'y a pas de prénom.
    pub fn nom_complet(&self) -> String {
        let complet = match self.prenom.as_deref().map(str::trim) {
            Some(prenom) if !prenom.is_empty() => format!("{prenom} {}", self.nom.trim()),
            _ => self.nom.trim().to_string(),
        };
        if complet.trim().is_empty() {
            self.login.clone().unwrap_or_default()
        } else {
            complet.trim().to_string()
        }
    }
}

/// Ce qu'il faut pour créer une fiche d'adhérent.
///
/// Pas de `morphy`, pas de `statut`, pas de mot de passe : voir la
/// documentation du module pour le pourquoi de chaque absence. Tous les champs
/// texte facultatifs acceptent `None` ; une chaîne vide est rangée en `NULL`,
/// pour ne pas faire croire à une saisie.
#[derive(Debug, Clone, Default)]
pub struct NewAdherent {
    pub lastname: String,
    pub firstname: Option<String>,
    pub email: Option<String>,
    pub login: Option<String>,
    pub fk_adherent_type: i64,
    pub societe: Option<String>,
    pub address: Option<String>,
    pub zip: Option<String>,
    pub town: Option<String>,
    pub phone: Option<String>,
    pub phone_mobile: Option<String>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
}

const SELECT_COLS: &str = "a.rowid, a.entity, a.ref, a.gender, a.civility, a.lastname, \
     a.firstname, a.login, a.fk_adherent_type, t.libelle, a.morphy, a.societe, a.address, \
     a.zip, a.town, a.country, a.email, a.url, a.phone, a.phone_mobile, \
     DATE_FORMAT(a.birth, '%Y-%m-%d'), a.statut, a.public, \
     DATE_FORMAT(a.datefin, '%Y-%m-%d'), a.note_private, a.note_public, \
     DATE_FORMAT(a.datevalid, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(a.datec, '%Y-%m-%d %H:%i:%s')";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn map_row(row: &Row) -> Adherent {
    Adherent {
        rowid: opt_i(row, 0).unwrap_or(0),
        entity: opt_i(row, 1).unwrap_or(1),
        ref_: opt_s(row, 2).unwrap_or_default(),
        genre: opt_s(row, 3),
        civilite: opt_s(row, 4),
        nom: opt_s(row, 5).unwrap_or_default(),
        prenom: opt_s(row, 6),
        login: opt_s(row, 7),
        fk_adherent_type: opt_i(row, 8).unwrap_or(0),
        type_libelle: opt_s(row, 9),
        // `morphy` est NOT NULL : une ligne sans valeur serait une ligne
        // fabriquée à la main, on retombe sur la personne physique.
        morphy: opt_s(row, 10).unwrap_or_else(|| MORPHY_PHYSIQUE.to_string()),
        societe: opt_s(row, 11),
        adresse: opt_s(row, 12),
        cp: opt_s(row, 13),
        ville: opt_s(row, 14),
        pays: opt_i(row, 15),
        email: opt_s(row, 16),
        url: opt_s(row, 17),
        telephone: opt_s(row, 18),
        portable: opt_s(row, 19),
        naissance: opt_s(row, 20),
        statut: opt_i(row, 21).unwrap_or(0) as i16,
        public: opt_i(row, 22).unwrap_or(0) != 0,
        date_fin: opt_s(row, 23),
        note_private: opt_s(row, 24),
        note_public: opt_s(row, 25),
        date_validation: opt_s(row, 26),
        date_creation: opt_s(row, 27),
    }
}

/// Un champ texte facultatif dont la chaîne vide vaut « pas de valeur ».
///
/// Dolibarr écrit souvent `''` là où la colonne accepterait `NULL`; une colonne
/// NULL dit vraiment qu'on n'a rien saisi, et évite qu'une recherche sur
/// `email = ''` ne ramène des fiches vides.
fn texte(valeur: Option<&str>) -> Option<&str> {
    valeur.map(str::trim).filter(|v| !v.is_empty())
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

/// Le message MySQL nomme-t-il cet index ?
///
/// Une erreur 1062 ne dit pas *quelle* contrainte a échoué autrement qu'en citant
/// le nom de l'index (`Duplicate entry '…' for key 'uk_adherent_login'`). C'est
/// le seul moyen de distinguer un identifiant déjà pris d'une référence déjà
/// attribuée, et donc de rendre une phrase juste plutôt qu'un diagnostic faux.
fn cite_index(e: &mysql::Error, index: &str) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062 && me.message.contains(index))
}

/// Le type d'adhérent existe-t-il, et est-il actif, dans cette entité ?
///
/// `llx_adherent_type` n'a pas de lignes d'entité 0 partagées, contrairement
/// aux dictionnaires `llx_c_*` : chaque entité crée ses types, d'où le filtre
/// strict sur `entity` (c'est aussi ce que fait `divers::options_liste` avec
/// `types_adherent`, pour que le menu déroulant et la validation voient la même
/// liste). Un type désactivé (`statut = 0`) reste en base — les adhésions
/// passées doivent continuer de s'afficher — mais ne doit plus servir à créer
/// une fiche : Dolibarr ne saurait plus le proposer ni le tarifer.
fn verifier_type(pool: &Pool, entity: i64, fk_adherent_type: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT statut FROM llx_adherent_type \
         WHERE entity = :entity AND rowid = :modele_type",
        params! { "entity" => entity, "modele_type" => fk_adherent_type },
    )?;
    match statut {
        None => Err(format!(
            "le type d'adhérent {fk_adherent_type} n'existe pas dans l'entité {entity}"
        )
        .into()),
        Some(s) if s < 1 => {
            Err(format!("le type d'adhérent {fk_adherent_type} est désactivé").into())
        }
        Some(_) => Ok(()),
    }
}

/// Crée une fiche d'adhérent et renvoie son `rowid`.
///
/// La fiche est créée **directement à jour** (`statut = 1`), avec
/// `datevalid = NOW()`. Dolibarr, lui, crée en brouillon (`-1`) puis exige un
/// `validate()` par un utilisateur identifié. Ce module n'a pas de session : il
/// ne peut pas nommer de valideur, donc il ne fabrique pas un circuit de
/// validation que personne ne pourrait signer. `fk_user_valid` reste `NULL`, et
/// cette absence dit la vérité : la validation vient du noyau, sans utilisateur
/// pour la signer.
///
/// `datefin` reste `NULL` : une création n'est pas un règlement de cotisation
/// (voir [`cotiser`]).
///
/// # Erreurs
///
/// Nom vide, email fourni sans arobase, type d'adhérent inconnu ou désactivé,
/// identifiant déjà pris, et toute erreur MySQL remontée telle quelle.
pub fn create(pool: &Pool, entity: i64, new: &NewAdherent) -> Result<i64, Box<dyn Error>> {
    let nom = new.lastname.trim();
    if nom.is_empty() {
        return Err("le nom de l'adhérent est obligatoire".into());
    }

    // Un email saisi doit au moins contenir une arobase : sans elle, la fiche
    // est inutilisable pour écrire au membre, et Dolibarr ne s'en aperçoit
    // qu'au moment de l'envoi, trop tard. Un email vide n'est pas une faute :
    // il vaut absence, traité plus bas.
    let email = texte(new.email.as_deref());
    if let Some(saisi) = email.filter(|e| !e.contains('@')) {
        return Err(format!("l'adresse email « {saisi} » ne contient pas d'arobase").into());
    }

    if new.fk_adherent_type <= 0 {
        return Err("le type d'adhérent (fk_adherent_type) est obligatoire".into());
    }
    verifier_type(pool, entity, new.fk_adherent_type)?;

    let login = texte(new.login.as_deref());
    if let Some(identifiant) = login {
        // `uk_adherent_login (login, entity)` protège d'une écriture
        // concurrente, mais un index unique laisse passer plusieurs `NULL` :
        // la vérification est donc faite ici, pour rendre une phrase claire
        // plutôt qu'une « Duplicate entry » MySQL.
        let mut conn = pool.get_conn()?;
        let pris: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_adherent WHERE entity = :entity AND login = :login LIMIT 1",
            params! { "entity" => entity, "login" => identifiant },
        )?;
        if pris.is_some() {
            return Err(format!(
                "l'identifiant « {identifiant} » est déjà pris par un adhérent de l'entité {entity}"
            )
            .into());
        }
    }

    let reference = divers::suivant_ref(pool, "llx_adherent", "ref", PREFIX, entity)?;
    let mut conn = pool.get_conn()?;
    let insere = conn.exec_drop(
        "INSERT INTO llx_adherent \
         (entity, ref, lastname, firstname, login, fk_adherent_type, morphy, \
          societe, address, zip, town, email, phone, phone_mobile, \
          statut, public, note_private, note_public, datec, datevalid) \
         VALUES (:entity, :ref, :nom, :prenom, :login, :modele_type, :morphy, \
          :societe, :adresse, :cp, :ville, :email, :telephone, :portable, \
          :statut, :public, :note_private, :note_public, NOW(), NOW())",
        params! {
            "entity" => entity,
            "ref" => reference.as_str(),
            "nom" => nom,
            "prenom" => texte(new.firstname.as_deref()),
            "login" => login,
            "modele_type" => new.fk_adherent_type,
            "morphy" => MORPHY_PHYSIQUE,
            "societe" => texte(new.societe.as_deref()),
            "adresse" => texte(new.address.as_deref()),
            "cp" => texte(new.zip.as_deref()),
            "ville" => texte(new.town.as_deref()),
            "email" => email,
            "telephone" => texte(new.phone.as_deref()),
            "portable" => texte(new.phone_mobile.as_deref()),
            "statut" => STATUT_A_JOUR,
            // `public` : la publication dans l'annuaire des membres est un
            // choix de l'adhérent, pas une conséquence de son adhésion.
            "public" => 0i16,
            "note_private" => texte(new.note_private.as_deref()),
            "note_public" => texte(new.note_public.as_deref()),
        },
    );

    // Pas de boucle de reprise, à la différence de `party::create` : la
    // numérotation passe par `divers::suivant_ref`, qui recalcule `MAX(ref) + 1`
    // et rendrait donc **le même** numéro au second essai. Rejouer ne servirait
    // à rien ; on dit ce qui s'est passé.
    match insere {
        Ok(()) => Ok(conn.last_insert_id() as i64),
        Err(e) if cite_index(&e, "uk_adherent_login") => Err(format!(
            "l'identifiant « {} » vient d'être pris par une autre création",
            login.unwrap_or("")
        )
        .into()),
        Err(e) if is_duplicate(&e) => Err(format!(
            "la référence « {reference} » vient d'être prise par une autre création : réessayez"
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

/// Les adhérents d'une entité, du plus récent au plus ancien.
///
/// L'ordre suit `datec` puis `rowid` : deux fiches saisies dans la même seconde
/// gardent un ordre stable, ce qu'un tri sur la seule date ne garantit pas.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Adherent>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_adherent a \
         LEFT JOIN llx_adherent_type t ON t.rowid = a.fk_adherent_type \
         WHERE a.entity = :entity \
         ORDER BY a.datec DESC, a.rowid DESC LIMIT :limite OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limite" => limite, "offset" => offset },
    )?;
    Ok(rows.iter().map(map_row).collect())
}

/// Un adhérent précis, ou `None` s'il n'existe pas dans cette entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Adherent>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_adherent a \
         LEFT JOIN llx_adherent_type t ON t.rowid = a.fk_adherent_type \
         WHERE a.entity = :entity AND a.rowid = :rowid"
    );
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.as_ref().map(map_row))
}

/// Un adhérent par son identifiant de connexion.
///
/// Un identifiant vide rend `None` sans interroger la base : la colonne est
/// nullable, et une comparaison sur `login = ''` ramènerait les fiches dont le
/// login a été vidé à la main plutôt que l'adhérent cherché.
pub fn par_login(
    pool: &Pool,
    entity: i64,
    login: &str,
) -> Result<Option<Adherent>, Box<dyn Error>> {
    let identifiant = login.trim();
    if identifiant.is_empty() {
        return Ok(None);
    }
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_adherent a \
         LEFT JOIN llx_adherent_type t ON t.rowid = a.fk_adherent_type \
         WHERE a.entity = :entity AND a.login = :login LIMIT 1"
    );
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "login" => identifiant })?;
    Ok(row.as_ref().map(map_row))
}

/// Statut courant d'un adhérent, vu par l'entité qui le demande.
fn statut_actuel(
    conn: &mut mysql::PooledConn,
    entity: i64,
    rowid: i64,
) -> Result<i16, Box<dyn Error>> {
    let statut: Option<i64> = conn.exec_first(
        "SELECT statut FROM llx_adherent WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        Some(s) => Ok(s as i16),
        None => Err(format!("adhérent {rowid} introuvable dans l'entité {entity}").into()),
    }
}

/// Une date `AAAA-MM-JJ` réelle : `2027-02-31` a le bon format mais n'existe
/// pas au calendrier, et MySQL refuserait l'écriture au milieu de la mise à
/// jour.
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

/// Enregistre une réadhesion : l'adhésion court désormais jusqu'au `jusqu_a`
/// inclus, et le statut repasse à « à jour ».
///
/// `datefin` est un `datetime`. On y écrit la date seule ; MySQL la range à
/// minuit, comme Dolibarr (`idate()` jette l'heure). [`a_jour`] compare ensuite
/// à `CURDATE()` : le dernier jour de l'adhésion est donc **couvert**, ce qui
/// est bien ce qu'on annonce au membre.
///
/// # Deux refus, et pourquoi
///
/// 1. **Une date passée est refusée.** Réadhérer est un acte de trésorerie :
///    une cotisation encaissée aujourd'hui ne peut pas produire une adhésion
///    déjà expirée. C'est presque toujours une faute de saisie (l'année
///    précédente au lieu de la suivante), et l'accepter effacerait le bénéfice
///    du paiement sans que personne ne s'en aperçoive.
/// 2. **Raccourcir une adhésion en cours est refusé.** Si `datefin` porte déjà
///    une date plus lointaine, écrire une date plus proche retirerait des mois
///    déjà payés. Pour raccourcir volontairement, il faut passer par l'écran
///    Dolibarr, où l'acte est explicite.
///
/// # Erreurs
///
/// Date mal formée ou inexistante au calendrier, date antérieure à aujourd'hui,
/// date qui raccourcirait l'adhésion, adhérent introuvable dans l'entité.
pub fn cotiser(pool: &Pool, entity: i64, rowid: i64, jusqu_a: &str) -> Result<(), Box<dyn Error>> {
    let fin = jusqu_a.trim();
    if !date_valide(fin) {
        return Err(format!(
            "date de fin d'adhésion invalide : « {jusqu_a} » — une date s'écrit AAAA-MM-JJ"
        )
        .into());
    }
    // Deux dates `AAAA-MM-JJ` se comparent comme du texte : l'ordre
    // alphabétique est l'ordre chronologique, aucune conversion n'est utile.
    // `crate::aujourdhui` interroge le serveur de base plutôt que l'horloge du
    // poste : c'est la base qui datera l'écriture, c'est donc elle qui fait foi.
    let aujourdhui = crate::aujourdhui(pool)?;
    if fin < aujourdhui.as_str() {
        return Err(format!(
            "on ne réadhère pas dans le passé : {fin} précède aujourd'hui ({aujourdhui})"
        )
        .into());
    }

    let mut conn = pool.get_conn()?;
    let actuel: Option<Option<String>> = conn.exec_first(
        "SELECT DATE_FORMAT(datefin, '%Y-%m-%d') FROM llx_adherent \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(date_fin_actuelle) = actuel else {
        return Err(format!("adhérent {rowid} introuvable dans l'entité {entity}").into());
    };
    if let Some(deja) = date_fin_actuelle.filter(|d| d.as_str() > fin) {
        return Err(
            format!("l'adhésion court déjà jusqu'au {deja} : {fin} la raccourcirait").into(),
        );
    }

    conn.exec_drop(
        "UPDATE llx_adherent SET datefin = :fin, statut = :statut, tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "fin" => fin,
            "statut" => STATUT_A_JOUR,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Suspend une adhésion : le statut passe à [`STATUT_SUSPENDU`].
///
/// `datefin` n'est pas touchée : la date de fin dit jusqu'où la cotisation a été
/// payée, la suspension dit qu'on ne l'honore plus. Mélanger les deux ferait
/// perdre l'information le jour où l'adhérent est réintégré.
///
/// Dolibarr refuse lui aussi de résilier deux fois (`Adherent::resiliate` sort
/// à 0 si le statut est déjà `STATUS_RESILIATED`) : la même garde est reprise
/// ici, mais avec une phrase explicite plutôt qu'un code de retour muet.
///
/// # Erreurs
///
/// Adhérent introuvable dans l'entité, adhérent déjà suspendu.
pub fn suspendre(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut == STATUT_SUSPENDU {
        return Err(format!("l'adhérent {rowid} est déjà suspendu").into());
    }
    conn.exec_drop(
        "UPDATE llx_adherent SET statut = :statut, tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_SUSPENDU,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Réintègre un adhérent suspendu : le statut repasse à [`STATUT_A_JOUR`].
///
/// Réactiver ne prolonge **pas** l'adhésion : si `datefin` est dépassée, la
/// fiche reste « hors cotisation » au sens de [`a_jour`], et c'est [`cotiser`]
/// qui la remettra dans le cycle. Séparer les deux gestes évite qu'un clic de
/// réintégration n'offre une année d'adhésion gratuite.
///
/// # Erreurs
///
/// Adhérent introuvable dans l'entité, adhérent déjà à jour.
pub fn reactiver(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut == STATUT_A_JOUR {
        return Err(format!("l'adhérent {rowid} est déjà à jour : rien à réactiver").into());
    }
    conn.exec_drop(
        "UPDATE llx_adherent SET statut = :statut, tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_A_JOUR, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Combien d'adhérents sont à jour, sur combien au total : `(à jour, total)`.
///
/// « À jour » veut dire `statut = 1` **et** une cotisation qui court encore :
/// `datefin` à `NULL` (jamais de cotisation enregistrée) ou postérieure à
/// aujourd'hui.
///
/// ⚠️ Écart assumé avec `adherents/list.php` de Dolibarr, qui écrit :
/// `(datefin >= now) OR (datefin IS NULL AND t.subscription = '0')`. Dolibarr
/// n'accorde donc la mention « à jour » sans `datefin` qu'aux types qui **ne
/// demandent pas de cotisation** ; les autres attendent leur premier règlement.
/// La règle appliquée ici est plus large : elle compte la fiche créée comme à
/// jour, ce qui est cohérent avec [`create`] (qui la crée au statut 1). Le jour
/// où la cotisation sera saisie, la condition `subscription = '0'` est à
/// ajouter ici, en une ligne.
///
/// Deux `COUNT(*)` plutôt qu'un `SUM(CASE …)` : un `SUM` MySQL rend un DECIMAL,
/// qu'il faudrait reconvertir, alors que `COUNT(*)` est un entier lu sans
/// détour.
pub fn a_jour(pool: &Pool, entity: i64) -> Result<(usize, usize), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let total: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_adherent WHERE entity = :entity",
        params! { "entity" => entity },
    )?;
    let a_jour: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_adherent \
         WHERE entity = :entity AND statut = :statut \
           AND (datefin IS NULL OR datefin >= CURDATE())",
        params! { "entity" => entity, "statut" => STATUT_A_JOUR },
    )?;
    Ok((
        a_jour.unwrap_or(0).max(0) as usize,
        total.unwrap_or(0).max(0) as usize,
    ))
}

/// Les types d'adhérent actifs, pour un menu déroulant : `(rowid, libellé)`.
///
/// Seuls les types actifs sortent : un type désactivé reste en base pour ne pas
/// casser les fiches déjà créées, mais il ne doit plus être proposé à la saisie.
///
/// Failible, contrairement à ce qu'un simple « liste pour un menu » laisserait
/// croire : une base injoignable doit se voir. Un menu vide et un menu qu'on
/// n'a pas pu remplir ne se ressemblent que trop, et celui-là ferait croire
/// qu'aucun type n'existe — donc qu'aucune adhésion n'est possible.
pub fn types(pool: &Pool, entity: i64) -> Result<Vec<(i64, String)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(i64, String)> = conn.exec(
        "SELECT rowid, libelle FROM llx_adherent_type \
         WHERE entity = :entity AND statut = 1 ORDER BY libelle ASC, rowid ASC",
        params! { "entity" => entity },
    )?;
    Ok(rows)
}

/// Pose le mot de passe d'un adhérent, à partir d'une **empreinte déjà
/// calculée**.
///
/// `hache` doit venir de `auth::hash(&mot_de_passe)`, comme pour un utilisateur
/// (`user::definir_mot_de_passe`). Aucun mot de passe en clair n'entre ici :
/// c'est la raison d'être de cette fonction séparée (voir la documentation du
/// module).
///
/// L'empreinte est écrite dans `pass_crypted`, la colonne que Dolibarr relit, et
/// `pass` est vidée : c'est là que Dolibarr range les mots de passe en clair de
/// ses vieux comptes, et rien ne justifie d'y laisser une trace.
///
/// # Erreurs
///
/// Empreinte vide ou non reconnue par `auth` (bcrypt `$2y$`, md5, sha1),
/// adhérent introuvable, adhérent sans identifiant de connexion.
pub fn definir_mot_de_passe(
    pool: &Pool,
    entity: i64,
    fk_adherent: i64,
    hache: &str,
) -> Result<(), Box<dyn Error>> {
    let empreinte = hache.trim();
    if empreinte.is_empty() {
        return Err("le hachage du mot de passe est vide : le calculer avec auth::hash".into());
    }
    // Garde-fou : un appelant distrait passera le mot de passe lui-même. En
    // refusant tout ce que `auth` ne reconnaît pas comme une empreinte, on
    // empêche un mot de passe en clair d'atterrir en base — exactement ce que
    // cette fonction existe pour éviter.
    if auth::Encoding::from_column(None, empreinte) == auth::Encoding::Unknown {
        return Err(
            "le hachage fourni n'est pas une empreinte reconnue (attendu : auth::hash, bcrypt $2y$)"
                .into(),
        );
    }

    let mut conn = pool.get_conn()?;
    let login: Option<Option<String>> = conn.exec_first(
        "SELECT login FROM llx_adherent WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_adherent },
    )?;
    let Some(login) = login else {
        return Err(format!("adhérent {fk_adherent} introuvable dans l'entité {entity}").into());
    };
    // Sans identifiant de connexion, l'empreinte ne sert à rien : l'espace
    // membre s'ouvre avec le couple login / mot de passe, et Dolibarr exige le
    // login à la création (`ADHERENT_LOGIN_NOT_REQUIRED` mis à part). Mieux
    // vaut le dire que de stocker un secret inutilisable.
    if login.as_deref().unwrap_or("").trim().is_empty() {
        return Err(format!(
            "l'adhérent {fk_adherent} n'a pas d'identifiant (login) : un mot de passe ne servirait à rien"
        )
        .into());
    }

    conn.exec_drop(
        "UPDATE llx_adherent SET pass_crypted = :hache, pass = NULL, tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "hache" => empreinte, "entity" => entity, "rowid" => fk_adherent },
    )?;
    Ok(())
}
