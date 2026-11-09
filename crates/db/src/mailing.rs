//! Mailings (« e-mailings ») : les campagnes d'envoi groupé.
//!
//! Un mailing, c'est un message préparé — un objet, un corps — et la liste des
//! adresses à qui il part. Dolibarr range l'en-tête dans `llx_mailing` et les
//! destinataires dans `llx_mailing_cibles`.
//!
//! **Aucune référence à numéroter.** `llx_mailing` n'a pas de colonne `ref` :
//! Dolibarr identifie une campagne par son `titre`, que la clé
//! `uk_mailing (titre, entity)` rend unique dans l'entité. Inventer un
//! `PR…`/`MA…` ici produirait une référence que personne d'autre ne lit.
//!
//! **Cycle de vie.** `statut` : `0` brouillon, `1` validée, `2` approuvée,
//! `3` envoyée. C'est le cycle que porte le commentaire de la colonne
//! (« draft:0, valid:1, approved:2, sent:3 ») et celui que ce module expose. Le
//! code PHP de Dolibarr 24.0.1, lui, n'écrit que `0 → 1` (`valid()`) et se sert
//! de `2` et `3` pour « envoyé partiellement » et « envoyé complètement » :
//! `date_appro` y est lue mais jamais écrite. Conséquence à connaître : un
//! mailing passé à `2` par l'expéditeur historique de Dolibarr n'est pas
//! forcément « approuvé » au sens de ce module. Le passage à `3` reste sans
//! ambiguïté, les deux lectures y voient « envoyé ».
//!
//! **Ce module n'envoie rien.** Il n'y a pas de client SMTP ici : composer et
//! remettre un message relève du binaire de courriel ou d'un envoi manuel.
//! [`marquer_envoye`] ne fait qu'enregistrer qu'un envoi a eu lieu — d'où son
//! nom. Écrire `statut = 3` sans que rien ne soit parti serait un mensonge en
//! base ; c'est à l'appelant de ne l'appeler qu'après un envoi réel, et le nom
//! de la fonction le lui rappelle à chaque lecture.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};

/// Le type de message par défaut : la colonne accepte aussi `sms`, et ce module
/// ne présume pas que l'appelant fait de l'e-mail.
pub const MESSTYPE_EMAIL: &str = "email";
/// Le second type que Dolibarr connaît.
pub const MESSTYPE_SMS: &str = "sms";

/// Longueur maximale du titre, en octets.
///
/// La colonne est un `varchar(128)`, donc 128 **caractères** en utf8mb4. On
/// refuse plus tôt, à 128 **octets** : c'est plus strict, et aucune écriture ne
/// dépend alors du jeu de caractères de la connexion.
pub const TITRE_MAX: usize = 128;
/// Longueur maximale de l'objet, même raison.
pub const SUJET_MAX: usize = 128;
/// Longueur maximale de `messtype` (`varchar(16)`).
pub const MESSTYPE_MAX: usize = 16;
/// Longueur maximale du critère de ciblage (`varchar(60)`).
///
/// C'est la colonne la plus étroite des champs facultatifs : sans contrôle, un
/// libellé de sélection un peu long ferait remonter un « Data too long » en
/// anglais, au milieu d'une création par ailleurs valide.
pub const CIBLE_MAX: usize = 60;

/// Brouillon : modifiable, supprimable, jamais parti.
pub const STATUT_BROUILLON: i16 = 0;
/// Validée : le contenu est figé, l'envoi se prépare.
pub const STATUT_VALIDEE: i16 = 1;
/// Approuvée : l'envoi est autorisé, il n'a pas encore eu lieu.
pub const STATUT_APPROUVEE: i16 = 2;
/// Envoyée : un envoi a eu lieu (au moins partiellement).
pub const STATUT_ENVOYEE: i16 = 3;

/// Cible dont l'envoi a échoué (adresse refusée, boîte pleine…).
pub const CIBLE_ERREUR: i16 = -1;
/// Cible pas encore servie.
pub const CIBLE_NON_ENVOYEE: i16 = 0;
/// Cible servie.
pub const CIBLE_ENVOYEE: i16 = 1;
/// Cible servie et message lu.
pub const CIBLE_LUE: i16 = 2;
/// Cible servie, puis désinscription demandée depuis le message.
pub const CIBLE_DESINSCRITE: i16 = 3;

/// Ce qu'il faut pour créer une campagne.
///
/// Les champs obligatoires sont des `String` : un mailing sans titre, sans objet
/// ni corps n'existe pas. Les autres sont facultatifs et deviennent `NULL` en
/// base.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewMailing {
    pub titre: String,
    pub sujet: String,
    pub body: String,
    pub email_from: String,
    pub name_from: Option<String>,
    pub email_replyto: Option<String>,
    /// Vide ou absent : `email`.
    pub messtype: String,
    /// Le critère de sélection d'origine, tel que Dolibarr le note (`varchar(60)`).
    pub cible: Option<String>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
}

impl NewMailing {
    /// Le cas courant : un e-mail signé par une adresse d'expédition.
    pub fn new(titre: &str, sujet: &str, body: &str, email_from: &str) -> Self {
        Self {
            titre: titre.to_string(),
            sujet: sujet.to_string(),
            body: body.to_string(),
            email_from: email_from.to_string(),
            messtype: MESSTYPE_EMAIL.to_string(),
            ..Default::default()
        }
    }
}

/// Une campagne telle qu'elle est stockée.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mailing {
    pub rowid: i64,
    pub entity: i64,
    pub titre: String,
    pub sujet: String,
    pub messtype: String,
    pub statut: i16,
    /// Nombre de destinataires, **dénormalisé** par Dolibarr dans `nbemail`.
    /// C'est la valeur stockée ; pour la vérité du moment, voir
    /// [`compter_destinataires`].
    pub nbemail: i64,
    pub email_from: Option<String>,
    pub name_from: Option<String>,
    pub cible: Option<String>,
    pub date_creation: Option<String>,
    pub date_valid: Option<String>,
    pub date_appro: Option<String>,
    pub date_envoi: Option<String>,
    /// Rempli par [`get`] seulement : la liste renvoie `None`, le corps étant un
    /// `mediumtext` qui peut peser des mégaoctets pour un simple affichage de
    /// titre.
    pub body: Option<String>,
}

impl Mailing {
    pub fn est_brouillon(&self) -> bool {
        self.statut == STATUT_BROUILLON
    }

    pub fn est_validee(&self) -> bool {
        self.statut == STATUT_VALIDEE
    }

    pub fn est_approuvee(&self) -> bool {
        self.statut == STATUT_APPROUVEE
    }

    pub fn est_envoyee(&self) -> bool {
        self.statut == STATUT_ENVOYEE
    }
}

/// Colonnes relues par toutes les lectures de l'en-tête.
///
/// `COALESCE` protège des colonnes nullables que Dolibarr laisse vides sur les
/// vieilles lignes : une campagne sans `titre` s'affiche alors avec un libellé
/// vide au lieu de faire échouer le décodage de toute la liste.
fn select_cols(date_creation: &str) -> String {
    format!(
        "m.rowid, m.entity, COALESCE(m.titre, ''), \
     COALESCE(m.sujet, ''), COALESCE(m.messtype, 'email'), \
     COALESCE(m.statut, 0), COALESCE(m.nbemail, 0), \
     m.email_from, m.name_from, m.cible, \
     DATE_FORMAT(m.{date_creation}, '%Y-%m-%d %H:%i'), \
     DATE_FORMAT(m.date_valid, '%Y-%m-%d %H:%i'), \
     DATE_FORMAT(m.date_appro, '%Y-%m-%d %H:%i'), \
     DATE_FORMAT(m.date_envoi, '%Y-%m-%d %H:%i')"
    )
}

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

/// Les entiers passent par `i64` : `COALESCE(statut, 0)` élargit le `smallint`
/// en `BIGINT`, et décoder directement en `i16` échouerait sur le type SQL.
fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn map_row(row: &Row) -> Mailing {
    Mailing {
        rowid: opt_i(row, 0).unwrap_or(0),
        entity: opt_i(row, 1).unwrap_or(1),
        titre: opt_s(row, 2).unwrap_or_default(),
        sujet: opt_s(row, 3).unwrap_or_default(),
        messtype: opt_s(row, 4).unwrap_or_else(|| MESSTYPE_EMAIL.to_string()),
        statut: opt_i(row, 5).unwrap_or(0) as i16,
        nbemail: opt_i(row, 6).unwrap_or(0),
        email_from: opt_s(row, 7),
        name_from: opt_s(row, 8),
        cible: opt_s(row, 9),
        date_creation: opt_s(row, 10),
        date_valid: opt_s(row, 11),
        date_appro: opt_s(row, 12),
        date_envoi: opt_s(row, 13),
        body: opt_s(row, 14),
    }
}

/// Les campagnes de l'entité, de la plus récente à la plus ancienne.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Mailing>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // `NULL` à la place du corps : voir `Mailing::body`. Le nombre de colonnes
    // reste le même que dans `get`, donc `map_row` est partagé.
    let date_creation = crate::colonnes::date_creation(pool, "llx_mailing")?;
    let sql = format!(
        "SELECT {}, NULL FROM llx_mailing m WHERE m.entity = ? \
         ORDER BY m.{date_creation} DESC, m.rowid DESC LIMIT ? OFFSET ?",
        select_cols(&date_creation)
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limite, offset))?;
    Ok(rows.iter().map(map_row).collect())
}

/// Une campagne précise, corps compris, ou `None` si elle n'existe pas dans
/// cette entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Mailing>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {}, m.body FROM llx_mailing m \
         WHERE m.entity = ? AND m.rowid = ?",
        select_cols(&crate::colonnes::date_creation(pool, "llx_mailing")?)
    );
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.as_ref().map(map_row))
}

/// Les champs ramenés à leur forme écrite en base.
///
/// Valider et normaliser d'un seul geste évite le classique « vérifié sur la
/// chaîne brute, écrit sur la chaîne nettoyée » : ici, ce qui a été contrôlé est
/// exactement ce qui part en base.
struct Champs<'a> {
    titre: &'a str,
    sujet: &'a str,
    messtype: &'a str,
    expediteur: &'a str,
    /// Nettoyé de ses espaces, et `None` s'il est vide : une étiquette blanche ne
    /// mérite pas une chaîne vide en base.
    cible: Option<&'a str>,
}

fn normaliser(new: &NewMailing) -> Result<Champs<'_>, Box<dyn Error>> {
    let titre = new.titre.trim();
    if titre.is_empty() {
        return Err("le titre du mailing est obligatoire : c'est lui qui l'identifie".into());
    }
    if titre.len() > TITRE_MAX {
        return Err(format!(
            "le titre fait {} octets : {TITRE_MAX} au maximum",
            titre.len()
        )
        .into());
    }

    let sujet = new.sujet.trim();
    if sujet.is_empty() {
        return Err("l'objet du message est obligatoire".into());
    }
    if sujet.len() > SUJET_MAX {
        return Err(format!(
            "l'objet du message fait {} octets : {SUJET_MAX} au maximum",
            sujet.len()
        )
        .into());
    }

    // Un mailing sans corps partirait vide chez le client : c'est le seul
    // contrôle qui porte sur le contenu, et il ne se rattrape pas après l'envoi.
    if new.body.trim().is_empty() {
        return Err("le corps du message est obligatoire : un mailing vide ne s'envoie pas".into());
    }

    let expediteur = new.email_from.trim();
    if expediteur.is_empty() {
        return Err("l'adresse d'expédition est obligatoire".into());
    }
    // On ne valide pas l'adresse au sens RFC 5322 : le contrôle qui attrape
    // vraiment les fautes de frappe, c'est l'arobase manquante.
    if !expediteur.contains('@') {
        return Err(format!(
            "« {expediteur} » n'est pas une adresse d'expédition : il lui manque l'arobase"
        )
        .into());
    }

    let messtype = match new.messtype.trim() {
        "" => MESSTYPE_EMAIL,
        renseigne => renseigne,
    };
    if messtype.len() > MESSTYPE_MAX {
        return Err(
            format!("le type de message « {messtype} » dépasse {MESSTYPE_MAX} octets").into(),
        );
    }

    // Le critère de ciblage n'est qu'une étiquette : on le mesure, on ne
    // l'interprète pas.
    let cible = new
        .cible
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    let trop_longue = cible.filter(|c| c.len() > CIBLE_MAX);
    if let Some(c) = trop_longue {
        return Err(format!(
            "le critère de ciblage fait {} octets : {CIBLE_MAX} au maximum",
            c.len()
        )
        .into());
    }

    Ok(Champs {
        titre,
        sujet,
        messtype,
        expediteur,
        cible,
    })
}

/// Crée une campagne en brouillon et renvoie son `rowid`.
///
/// Aucune référence n'est attribuée : Dolibarr n'en a pas pour les mailings, et
/// la clé `uk_mailing (titre, entity)` fait du titre l'identifiant lisible.
/// `evenunsubscribe` reste à `0` — une campagne créée ici ne force jamais
/// l'envoi à une adresse désinscrite.
pub fn create(pool: &Pool, entity: i64, new: &NewMailing) -> Result<i64, Box<dyn Error>> {
    let champs = normaliser(new)?;
    let mut conn = pool.get_conn()?;
    let date_creation = crate::colonnes::date_creation(pool, "llx_mailing")?;
    let insere = conn.exec_drop(
        format!(
            "INSERT INTO llx_mailing \
         (entity, messtype, titre, sujet, body, cible, nbemail, email_from, name_from, \
          email_replyto, statut, {date_creation}, note_private, note_public) \
         VALUES (:entity, :messtype, :titre, :sujet, :body, :cible, 0, :email_from, \
          :name_from, :email_replyto, :statut, NOW(), :note_private, :note_public)"
        ),
        params! {
            "entity" => entity,
            "messtype" => champs.messtype,
            "titre" => champs.titre,
            "sujet" => champs.sujet,
            // Le corps est écrit tel quel : en HTML, l'indentation compte.
            "body" => new.body.as_str(),
            "cible" => champs.cible,
            "email_from" => champs.expediteur,
            "name_from" => new.name_from.as_deref(),
            "email_replyto" => new.email_replyto.as_deref(),
            "statut" => STATUT_BROUILLON,
            "note_private" => new.note_private.as_deref(),
            "note_public" => new.note_public.as_deref(),
        },
    );
    if let Err(e) = insere {
        // 1062 : `uk_mailing (titre, entity)`. Le dire clairement évite de
        // laisser remonter un « Duplicate entry » brut à l'écran.
        if is_duplicate(&e) {
            return Err(format!(
                "une campagne intitulée « {} » existe déjà dans l'entité {entity} : le titre identifie un mailing",
                champs.titre
            )
            .into());
        }
        return Err(e.into());
    }
    Ok(conn.last_insert_id() as i64)
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

fn statut_de(conn: &mut mysql::PooledConn, entity: i64, rowid: i64) -> Result<i16, Box<dyn Error>> {
    let statut: Option<i64> = conn.exec_first(
        "SELECT statut FROM llx_mailing WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        Some(s) => Ok(s as i16),
        None => Err(format!("le mailing {rowid} n'existe pas dans l'entité {entity}").into()),
    }
}

/// Valide un brouillon : `statut` passe de 0 à 1, `date_valid` est posée.
///
/// Valider deux fois n'est pas une opération idempotente, c'est un appelant qui
/// a perdu le fil de son écran : le laisser passer masquerait un double
/// traitement (deux préparations d'envoi pour la même campagne).
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    match statut_de(&mut conn, entity, rowid)? {
        STATUT_BROUILLON => {}
        STATUT_VALIDEE => return Err("ce mailing est déjà validé".into()),
        STATUT_APPROUVEE => {
            return Err("ce mailing est déjà approuvé : il n'a plus à être validé".into())
        }
        _ => return Err("ce mailing est déjà envoyé : il ne se valide plus".into()),
    }
    conn.exec_drop(
        "UPDATE llx_mailing SET statut = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Approuve une campagne validée : `statut` passe de 1 à 2, `date_appro` est
/// posée.
///
/// Approuver un brouillon reviendrait à autoriser l'envoi d'un texte que
/// personne n'a relu : la validation est le contrôle, l'approbation la décision,
/// et l'ordre entre les deux est le seul qui ait un sens.
pub fn approuver(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    match statut_de(&mut conn, entity, rowid)? {
        STATUT_VALIDEE => {}
        STATUT_BROUILLON => {
            return Err("ce mailing n'est pas validé : le valider avant de l'approuver".into())
        }
        STATUT_APPROUVEE => return Err("ce mailing est déjà approuvé".into()),
        _ => return Err("ce mailing est déjà envoyé : il ne s'approuve plus".into()),
    }
    conn.exec_drop(
        "UPDATE llx_mailing SET statut = :statut, date_appro = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_APPROUVEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Enregistre qu'un envoi a eu lieu : `statut` passe de 2 à 3, `date_envoi` est
/// posée.
///
/// **Cette fonction n'envoie rien.** Aucun SMTP n'est ouvert ici : le message
/// est parti par le binaire de courriel, par un autre outil, ou à la main. Le
/// nom dit « marquer » et non « envoyer » pour que l'appelant ne puisse pas s'y
/// tromper. Appelée sans envoi réel, elle inscrit un fait faux en base — pire
/// que de ne rien écrire, parce que la comptabilité des campagnes deviendrait
/// incohérente sans que rien ne le signale.
pub fn marquer_envoye(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    match statut_de(&mut conn, entity, rowid)? {
        STATUT_APPROUVEE => {}
        STATUT_BROUILLON | STATUT_VALIDEE => {
            return Err(
                "ce mailing n'est pas approuvé : il ne peut pas être marqué comme envoyé".into(),
            )
        }
        _ => return Err("ce mailing est déjà marqué comme envoyé".into()),
    }
    conn.exec_drop(
        "UPDATE llx_mailing SET statut = :statut, date_envoi = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_ENVOYEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Supprime un brouillon et ses destinataires.
///
/// Une campagne validée, approuvée ou envoyée a déjà servi de preuve : elle dit
/// qui devait recevoir quoi et quand. La suppression physique est donc réservée
/// au brouillon, seul état dont l'existence n'engage rien.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    if statut_de(&mut conn, entity, rowid)? != STATUT_BROUILLON {
        return Err(
            "seul un brouillon se supprime : ce mailing est validé, approuvé ou envoyé".into(),
        );
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    // Les cibles d'abord : sans en-tête, `llx_mailing_cibles` ne serait plus
    // rattachable à une entité, donc plus filtrable ni supprimable proprement.
    tx.exec_drop(
        "DELETE FROM llx_mailing_cibles WHERE fk_mailing = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_mailing WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Le filtre des destinataires réellement joignables, partagé par
/// [`destinataires`] et [`compter_destinataires`].
///
/// Deux exclusions, celles que Dolibarr constate lui-même :
///
/// - `statut = -1` : l'envoi précédent a échoué sur cette adresse ;
/// - `statut = 3` : le destinataire s'est désinscrit depuis le message
///   (`public/emailing/mailing-unsubscribe.php` écrit ce `3`) ;
///
/// plus les adresses vides, qu'un import approximatif laisse passer.
///
/// Enfin `llx_mailing_unsubscribe` : la désinscription globale, tous mailings
/// confondus, que Dolibarr enregistre en plus du `3`. Sauf `evenunsubscribe = 1`,
/// le drapeau de la campagne qui autorise explicitement l'envoi à une adresse
/// désinscrite — la laisser passer est alors voulu, pas un oubli.
const FILTRE_CIBLES: &str = "AND mc.statut <> -1 AND mc.statut <> 3 \
     AND TRIM(mc.email) <> '' \
     AND (m.evenunsubscribe = 1 OR NOT EXISTS ( \
           SELECT 1 FROM llx_mailing_unsubscribe u \
           WHERE u.entity = m.entity AND u.email = TRIM(mc.email)))";

/// Les adresses des destinataires d'une campagne, dédupliquées et sans les
/// vides, dans l'ordre de saisie.
///
/// C'est **l'audience** de la campagne, pas la liste de ce qui reste à servir :
/// une adresse déjà servie (`statut` 1 ou 2) en fait partie, parce qu'elle a
/// bien été visée. L'expéditeur de Dolibarr, lui, ne reprend que `statut < 1`
/// (`comm/mailing/card.php`) ; si l'appelant veut la même chose, c'est à lui de
/// la demander, et ce module ne l'expose pas — il n'a rien à écrire dans les
/// cibles, seulement à les lire.
///
/// La déduplication ne se contente pas de `DISTINCT` sur la colonne : elle porte
/// sur l'adresse **nettoyée** (`TRIM`), parce que « jean@exemple.fr » et
/// « jean@exemple.fr » avec une espace de tête sont la même boîte et Dolibarr
/// enverrait deux fois. La clé `uk_mailing_cibles (fk_mailing, email)` empêche
/// déjà le doublon exact ; elle ne voit pas les espaces.
pub fn destinataires(
    pool: &Pool,
    entity: i64,
    fk_mailing: i64,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT TRIM(mc.email) FROM llx_mailing_cibles mc \
         JOIN llx_mailing m ON m.rowid = mc.fk_mailing \
         WHERE m.entity = :entity AND mc.fk_mailing = :id {FILTRE_CIBLES} \
         GROUP BY TRIM(mc.email) ORDER BY MIN(mc.rowid)"
    );
    let rows: Vec<String> = conn.exec(sql, params! { "entity" => entity, "id" => fk_mailing })?;
    Ok(rows)
}

/// Le nombre de destinataires réellement joignables, c'est-à-dire la longueur de
/// [`destinataires`].
///
/// Même filtre, même jointure : le compteur ne peut pas diverger de la liste.
/// Il évite en revanche de charger cent mille adresses pour afficher un nombre.
pub fn compter_destinataires(
    pool: &Pool,
    entity: i64,
    fk_mailing: i64,
) -> Result<usize, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT COUNT(DISTINCT TRIM(mc.email)) FROM llx_mailing_cibles mc \
         JOIN llx_mailing m ON m.rowid = mc.fk_mailing \
         WHERE m.entity = :entity AND mc.fk_mailing = :id {FILTRE_CIBLES}"
    );
    let nombre: Option<i64> =
        conn.exec_first(sql, params! { "entity" => entity, "id" => fk_mailing })?;
    Ok(nombre.unwrap_or(0).max(0) as usize)
}
