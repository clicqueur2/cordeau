//! Base de connaissances (`llx_knowledgemanagement_knowledgerecord`).
//!
//! Une fiche de connaissance, c'est la réponse qu'on **garde** : la question
//! qu'un client a posée un jour, et ce qu'on lui a répondu. Elle naît souvent
//! d'un ticket résolu, mais ce n'est pas un ticket : le ticket est une
//! réclamation qui se ferme, la fiche est ce qui reste pour la fois suivante —
//! celle qu'on relit avant de répondre au téléphone.
//!
//! Ce module numérote la fiche `KB` + compteur continu (`KB0001`, `KB0002`…),
//! **sans période** contrairement aux devis et aux factures : le compteur ne
//! repart jamais de zéro d'un mois sur l'autre. La numérotation passe par
//! `divers::suivant_ref`, qui cherche le maximum existant sur toute la table.
//!
//! ## Pièges de schéma (relevés sur `schema-24.0.1.json` et le DDL livré)
//!
//! - `question` est un `text` (65 535 octets) : au-delà, le serveur tronque ou
//!   refuse selon son mode SQL. On s'arrête à [`QUESTION_MAX`] avec un message
//!   lisible, plutôt que de laisser la base décider en silence.
//! - `fk_user_creat` est `NOT NULL` **et** porte la clé étrangère
//!   `llx_knowledgemanagement_knowledgerecord_fk_user_creat` vers `llx_user` :
//!   0 y est refusé par la base elle-même, avec un message MySQL incompréhensible
//!   pour l'utilisateur. On vérifie donc avant d'écrire.
//! - l'index sur `ref` (`idx_knowledgemanagement_knowledgerecord_ref`) **n'est
//!   pas unique** : deux saisies simultanées peuvent produire la même référence
//!   sans que la base ne proteste. C'est une raison de plus pour numéroter
//!   sérieusement, et pour rejouer si une base où l'index aurait été rendu
//!   unique répond « doublon ».
//! - `answer` est un `longtext` sans valeur par défaut : `NULL` et `''` sont
//!   deux choses différentes. Une réponse absente s'écrit `NULL`, comme le fait
//!   Dolibarr, sinon la fiche affiche un cadre vide au lieu de « pas encore de
//!   réponse ».
//! - `llx_categorie_knowledgemanagement` porte une clé étrangère vers la fiche
//!   (`fk_categorie_knowledgemanagement_knowledgemanagement_rowid`) : une fiche
//!   étiquetée ne se supprime pas sans retirer ses étiquettes d'abord (voir
//!   [`supprimer`]).

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool, Row};

/// La table des fiches. Le nom est long : il n'est écrit qu'ici.
pub const TABLE: &str = "llx_knowledgemanagement_knowledgerecord";

/// Préfixe des références (`KB0001`). Pas de période : le compteur est continu.
pub const PREFIXE: &str = "KB";

/// Statuts Dolibarr : 0 = brouillon, 1 = publiée.
pub const STATUT_BROUILLON: i16 = 0;
pub const STATUT_PUBLIEE: i16 = 1;

/// Longueur maximale d'une question, en **octets**.
///
/// La colonne est un `text` : 65 535 octets. On refuse dès 65 000 pour garder
/// une marge et rendre une phrase française au lieu d'une troncature décidée
/// par le serveur. Le compte est en octets et non en caractères : « é » en pèse
/// deux, une question pleine d'accents atteint donc la limite deux fois plus
/// vite qu'on ne le croit en la relisant.
pub const QUESTION_MAX: usize = 65_000;

/// Longueur maximale d'une adresse. La colonne `url` est un `varchar(255)`.
pub const URL_MAX: usize = 255;

/// Longueur maximale d'une langue. La colonne `lang` est un `varchar(6)`, juste
/// de quoi écrire un code Dolibarr du type `fr_FR`.
pub const LANGUE_MAX: usize = 6;

/// Plafond de [`rechercher`] : une recherche n'est pas un export de la base.
pub const RECHERCHE_MAX: u32 = 500;

/// Le caractère d'échappement de `LIKE`, utilisé dans le SQL par
/// `ESCAPE '<ce caractère>'` (voir `echapper_like`).
///
/// `!` et non l'antislash habituel : en MySQL, l'antislash est **aussi** le
/// caractère d'échappement des littéraux de chaîne, et son sens change avec le
/// mode SQL `NO_BACKSLASH_ESCAPES`. Un caractère ordinaire ne dépend d'aucun
/// réglage du serveur.
const ECHAPPEMENT: char = '!';

/// Une fiche de connaissance, telle qu'on la relit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fiche {
    pub rowid: i64,
    /// Référence Dolibarr (`KB0001`). Le suffixe `_` est imposé : `ref` est un
    /// mot-clé Rust.
    pub ref_: String,
    pub question: String,
    /// Réponse (`answer`). `None` = pas encore rédigée, jamais `''`.
    pub reponse: Option<String>,
    /// Lien cliquable vers une procédure ou une documentation.
    pub url: Option<String>,
    /// Code de langue Dolibarr (`fr_FR`).
    pub langue: Option<String>,
    /// 0 = brouillon, 1 = publiée.
    pub statut: i16,
    /// Date de création, `AAAA-MM-JJ HH:MM`.
    pub date: Option<String>,
    /// Ticket d'où la fiche est née, s'il y en a un.
    pub fk_ticket: Option<i64>,
    /// Référence de ce ticket, relue pour l'affichage (`TK2611-0012`).
    pub ticket_ref: Option<String>,
    /// Catégorie de tickets (`llx_c_ticket_category`), si la fiche en porte une.
    pub fk_c_ticket_category: Option<i64>,
    /// Auteur de la fiche. Jamais nul en base, mais on le relit en `Option`
    /// comme tout le reste : une ligne abîmée ne doit pas faire tomber la liste.
    pub fk_user_creat: Option<i64>,
}

impl Fiche {
    /// Publiée ? C'est `status = 1` chez Dolibarr, et c'est ce que voient les
    /// utilisateurs qui consultent la base de connaissances.
    pub fn est_publiee(&self) -> bool {
        self.statut == STATUT_PUBLIEE
    }
}

/// Ce qu'il faut pour créer une fiche.
#[derive(Debug, Clone, Default)]
pub struct NewFiche {
    /// Obligatoire, non vide, 65 000 octets au plus.
    pub question: String,
    /// Facultative : une fiche peut se créer avant d'être rédigée.
    pub answer: Option<String>,
    /// Facultative, mais si elle est fournie elle doit commencer par `http`.
    pub url: Option<String>,
    /// Facultative (`fr_FR`, `en_US`…), 6 caractères au plus.
    pub lang: Option<String>,
    /// Ticket d'origine. Doit exister dans l'entité ; 0 ou `None` = aucun lien.
    pub fk_ticket: Option<i64>,
    /// Catégorie de tickets. Doit exister ; 0 ou `None` = aucune catégorie.
    pub fk_c_ticket_category: Option<i64>,
    /// Auteur. Obligatoire en création : la colonne est `NOT NULL` et porte une
    /// clé étrangère vers `llx_user`.
    pub fk_user_creat: i64,
}

/// Colonnes relues, avec la référence du ticket pour l'affichage.
const SELECT_COLS: &str = "k.rowid, k.ref, k.question, k.answer, k.url, k.lang, k.status, \
     DATE_FORMAT(k.date_creation, '%Y-%m-%d %H:%i'), k.fk_ticket, t.ref, \
     k.fk_c_ticket_category, k.fk_user_creat";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn map_row(row: Row) -> Fiche {
    Fiche {
        rowid: opt_i(&row, 0).unwrap_or(0),
        ref_: opt_s(&row, 1).unwrap_or_default(),
        question: opt_s(&row, 2).unwrap_or_default(),
        reponse: opt_s(&row, 3),
        url: opt_s(&row, 4),
        langue: opt_s(&row, 5),
        statut: opt_i(&row, 6).unwrap_or(0) as i16,
        date: opt_s(&row, 7),
        fk_ticket: opt_i(&row, 8),
        ticket_ref: opt_s(&row, 9),
        fk_c_ticket_category: opt_i(&row, 10),
        fk_user_creat: opt_i(&row, 11),
    }
}

/// Un texte facultatif : blanc vaut `None`, jamais `''`.
///
/// La distinction compte : `answer` et `url` sont nullables, et une chaîne vide
/// s'afficherait comme une valeur saisie (« réponse : » suivi de rien) alors
/// qu'il n'y a rien.
fn texte(v: Option<&str>) -> Option<String> {
    v.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Textes validés, prêts à être écrits.
#[derive(Debug)]
struct Textes {
    question: String,
    answer: Option<String>,
    url: Option<String>,
    lang: Option<String>,
}

/// Valide et nettoie les quatre champs de texte d'une fiche.
///
/// Partagé par [`create`] et [`modifier`] : les deux écrivent les mêmes colonnes,
/// ils doivent refuser les mêmes choses. Un contrôle qui n'existerait qu'à la
/// création laisserait passer par la modification ce qu'on interdit à la saisie.
fn valider(new: &NewFiche) -> Result<Textes, Box<dyn Error>> {
    let question = new.question.trim();
    if question.is_empty() {
        return Err("la question de la fiche est obligatoire".into());
    }
    if question.len() > QUESTION_MAX {
        return Err(format!(
            "la question fait {} octets : la colonne `question` est un `text` limité à \
             65 535 octets, on refuse au-delà de {QUESTION_MAX}",
            question.len()
        )
        .into());
    }

    let url = match texte(new.url.as_deref()) {
        None => None,
        Some(adresse) => {
            if adresse.len() > URL_MAX {
                return Err(format!(
                    "l'adresse fait {} caractères : la colonne `url` est un varchar({URL_MAX})",
                    adresse.len()
                )
                .into());
            }
            // Le champ est un lien cliquable, pas un chemin de fichier ni une
            // note : sans schéma, le navigateur en fait une adresse relative au
            // serveur Dolibarr, qui ne mène nulle part.
            if !adresse.to_ascii_lowercase().starts_with("http") {
                return Err(format!(
                    "l'adresse « {adresse} » doit commencer par http (ou https) : \
                     c'est un lien cliquable, pas un chemin de fichier"
                )
                .into());
            }
            Some(adresse)
        }
    };

    let lang = texte(new.lang.as_deref());
    if let Some(trop_longue) = lang.as_ref().filter(|l| l.len() > LANGUE_MAX) {
        return Err(format!(
            "la langue « {trop_longue} » dépasse {LANGUE_MAX} caractères : la colonne `lang` \
             est un varchar(6), juste de quoi écrire un code du type fr_FR"
        )
        .into());
    }

    Ok(Textes {
        question: question.to_string(),
        answer: texte(new.answer.as_deref()),
        url,
        lang,
    })
}

/// Échappe les caractères que `LIKE` interprète, pour qu'ils redeviennent des
/// caractères comme les autres.
///
/// Trois caractères seulement : `%` (n'importe quelle suite), `_` (n'importe
/// quel caractère) et le caractère d'échappement lui-même, sans quoi on ne
/// pourrait plus écrire un `!` littéral.
///
/// Sans cet échappement, un utilisateur qui cherche « 20 % » ramène toute la
/// base (le motif devient `%%%`), et un « _ » seul fait de même. C'est une
/// erreur banale et **invisible** : la recherche a l'air de marcher, elle rend
/// juste trop de résultats, et personne ne s'en aperçoit avant qu'un utilisateur
/// ne se plaigne de retrouver des fiches qui n'ont rien à voir.
fn echapper_like(terme: &str) -> String {
    let mut sortie = String::with_capacity(terme.len() + 8);
    for c in terme.chars() {
        if c == ECHAPPEMENT || c == '%' || c == '_' {
            sortie.push(ECHAPPEMENT);
        }
        sortie.push(c);
    }
    sortie
}

/// Le motif `LIKE` d'une recherche « contient » : le terme échappé, encadré de
/// deux jokers.
///
/// L'échappement se fait **avant** l'encadrement : encadrer d'abord reviendrait
/// à échapper aussi les deux jokers ajoutés, et la recherche ne trouverait plus
/// que les fiches commençant et finissant par le terme.
fn motif_recherche(terme: &str) -> String {
    format!("%{}%", echapper_like(terme.trim()))
}

/// Vérifie qu'un ticket existe dans l'entité.
///
/// `None`, 0 ou une valeur négative valent « aucun lien » : c'est ce que
/// renvoient les formulaires quand on ne choisit pas de ticket (0), et refuser
/// ce 0 obligerait chaque écran à le traduire avant d'appeler.
fn verifier_ticket(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_ticket: Option<i64>,
) -> Result<Option<i64>, Box<dyn Error>> {
    let Some(fk) = fk_ticket.filter(|f| *f > 0) else {
        return Ok(None);
    };
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_ticket WHERE entity = :entity AND rowid = :ticket",
        params! { "entity" => entity, "ticket" => fk },
    )?;
    if existe.is_none() {
        return Err(format!("le ticket {fk} n'existe pas dans l'entité {entity}").into());
    }
    Ok(Some(fk))
}

/// Vérifie qu'une catégorie de tickets existe.
///
/// Pas de filtre sur `entity`, à la différence des tickets : `llx_c_ticket_category`
/// est un dictionnaire (`llx_c_*`), et Dolibarr le partage entre les entités. Un
/// filtre strict refuserait une catégorie que l'écran Dolibarr propose pourtant,
/// ce qui est pire que le risque d'accepter une catégorie d'une autre entité —
/// risque nul en pratique, le dictionnaire étant commun.
fn verifier_categorie(
    conn: &mut mysql::PooledConn,
    fk_c_ticket_category: Option<i64>,
) -> Result<Option<i64>, Box<dyn Error>> {
    let Some(fk) = fk_c_ticket_category.filter(|f| *f > 0) else {
        return Ok(None);
    };
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_c_ticket_category WHERE rowid = :categorie",
        params! { "categorie" => fk },
    )?;
    if existe.is_none() {
        return Err(format!("la catégorie de tickets {fk} n'existe pas").into());
    }
    Ok(Some(fk))
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

/// Les fiches de l'entité, de la plus récente à la plus ancienne.
///
/// Le tri porte sur `date_creation` puis sur `rowid` : deux fiches écrites dans
/// la même seconde doivent garder un ordre stable, sinon la première ligne de la
/// liste change d'un affichage à l'autre sans qu'aucune donnée n'ait bougé.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Fiche>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM {TABLE} k \
         LEFT JOIN llx_ticket t ON t.rowid = k.fk_ticket \
         WHERE k.entity = ? \
         ORDER BY k.date_creation DESC, k.rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limite, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Une fiche précise, avec la référence de son ticket s'il y en a un.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Fiche>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM {TABLE} k \
         LEFT JOIN llx_ticket t ON t.rowid = k.fk_ticket \
         WHERE k.entity = ? AND k.rowid = ?"
    );
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// Crée une fiche et renvoie son `rowid`.
///
/// La fiche naît **publiée** (`status = 1`) : Dolibarr n'a pas de circuit de
/// validation pour la base de connaissances — une fiche écrite est une fiche
/// utilisable, il n'y a personne à faire signer. Pour la retirer de la
/// circulation sans la perdre, il y a [`depublier`].
///
/// `date_creation` est posée à `NOW()`, jamais reprise d'un formulaire : c'est
/// la date à laquelle la connaissance est entrée dans la base, pas celle de la
/// question d'origine.
///
/// # Erreurs
///
/// Question vide ou de plus de 65 000 octets, adresse qui ne commence pas par
/// `http` ou plus longue que 255 caractères, langue de plus de 6 caractères,
/// auteur absent (`<= 0`) ou inconnu de l'entité, ticket ou catégorie
/// inexistants, et toute erreur MySQL remontée telle quelle.
pub fn create(pool: &Pool, entity: i64, new: &NewFiche) -> Result<i64, Box<dyn Error>> {
    let textes = valider(new)?;

    if new.fk_user_creat <= 0 {
        return Err("l'auteur de la fiche (fk_user_creat) est obligatoire : \
                    la colonne est NOT NULL et porte une clé étrangère vers llx_user"
            .into());
    }

    let mut conn = pool.get_conn()?;
    let auteur: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity AND rowid = :auteur",
        params! { "entity" => entity, "auteur" => new.fk_user_creat },
    )?;
    if auteur.is_none() {
        return Err(format!(
            "l'utilisateur {} n'existe pas dans l'entité {entity} : une fiche de \
             connaissance a toujours un auteur",
            new.fk_user_creat
        )
        .into());
    }

    let ticket = verifier_ticket(&mut conn, entity, new.fk_ticket)?;
    let categorie = verifier_categorie(&mut conn, new.fk_c_ticket_category)?;

    // L'index sur `ref` n'est pas unique dans le schéma du cœur, donc une
    // collision est peu probable — mais pas impossible sur une base où il
    // l'aurait été rendu, ou si deux saisies tombent dans la même seconde. On
    // rejoue alors avec le numéro suivant plutôt que de rendre une « Duplicate
    // entry » incompréhensible.
    for essai in 0..5u32 {
        let reference = crate::divers::suivant_ref(pool, TABLE, "ref", PREFIXE, entity)?;
        let insere = conn.exec_drop(
            "INSERT INTO llx_knowledgemanagement_knowledgerecord \
             (entity, ref, date_creation, lang, fk_user_creat, question, answer, url, \
              fk_ticket, fk_c_ticket_category, status) \
             VALUES (:entity, :ref, NOW(), :lang, :auteur, :question, :answer, :url, \
              :ticket, :categorie, :statut)",
            params! {
                "entity" => entity,
                "ref" => reference.as_str(),
                "lang" => textes.lang.as_deref(),
                "auteur" => new.fk_user_creat,
                "question" => textes.question.as_str(),
                "answer" => textes.answer.as_deref(),
                "url" => textes.url.as_deref(),
                "ticket" => ticket,
                "categorie" => categorie,
                "statut" => STATUT_PUBLIEE,
            },
        );
        match insere {
            Ok(()) => return Ok(conn.last_insert_id() as i64),
            Err(e) if is_duplicate(&e) && essai < 4 => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une référence de fiche unique".into())
}

/// Remplace le texte d'une fiche : question, réponse, adresse, langue,
/// catégorie.
///
/// La **référence** et la `date_creation` ne sont jamais réécrites : la première
/// a pu être notée ou citée dans un mail, la seconde est l'âge de la
/// connaissance. `fk_ticket` et `fk_user_creat` restent également en place —
/// `new.fk_user_creat` est donc **ignoré** ici, l'auteur d'origine restant la
/// trace de qui a ouvert la fiche. Une correction de texte n'efface pas
/// l'histoire de la fiche.
///
/// # Erreurs
///
/// Les mêmes contrôles de texte qu'à la création, une catégorie inconnue, ou une
/// fiche introuvable dans l'entité.
pub fn modifier(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    new: &NewFiche,
) -> Result<(), Box<dyn Error>> {
    let textes = valider(new)?;
    let mut conn = pool.get_conn()?;

    // L'existence se vérifie par un SELECT, pas par `affected_rows` : MySQL
    // compte les lignes **changées**, pas les lignes trouvées. Enregistrer deux
    // fois le même texte renverrait 0 et ferait croire à une fiche disparue.
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_knowledgemanagement_knowledgerecord \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    if existe.is_none() {
        return Err(format!("la fiche {rowid} n'existe pas dans l'entité {entity}").into());
    }
    let categorie = verifier_categorie(&mut conn, new.fk_c_ticket_category)?;

    conn.exec_drop(
        "UPDATE llx_knowledgemanagement_knowledgerecord \
         SET question = :question, answer = :answer, url = :url, lang = :lang, \
             fk_c_ticket_category = :categorie \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "question" => textes.question.as_str(),
            "answer" => textes.answer.as_deref(),
            "url" => textes.url.as_deref(),
            "lang" => textes.lang.as_deref(),
            "categorie" => categorie,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Supprime une fiche.
///
/// Les étiquettes de catégorie (`llx_categorie_knowledgemanagement`) partent
/// d'abord : elles portent une clé étrangère vers la fiche, et un `DELETE`
/// direct échouerait sur la contrainte en laissant l'utilisateur devant une
/// erreur MySQL. Les champs complémentaires (`..._extrafields`) suivent dans la
/// même transaction, comme le fait le `delete()` de Dolibarr.
///
/// # Erreurs
///
/// Fiche introuvable : on le dit, plutôt que de rendre un succès sur une
/// suppression qui n'a rien fait.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // L'existence se vérifie avant d'ouvrir la transaction : sans cela, un
    // identifiant d'une autre entité ferait partir ses étiquettes avant d'échouer
    // sur la fiche introuvable, et on laisserait une fiche à moitié déshabillée.
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_knowledgemanagement_knowledgerecord \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    if existe.is_none() {
        return Err(format!("la fiche {rowid} n'existe pas dans l'entité {entity}").into());
    }
    let mut tx = conn.start_transaction(mysql::TxOpts::default())?;
    tx.exec_drop(
        "DELETE FROM llx_categorie_knowledgemanagement WHERE fk_knowledgemanagement = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_knowledgemanagement_knowledgerecord_extrafields WHERE fk_object = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_knowledgemanagement_knowledgerecord WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Cherche dans la **question** et dans la **réponse**, sans tenir compte de la
/// casse.
///
/// # L'échappement de `LIKE` — à lire avant de toucher à cette fonction
///
/// Le terme vient de l'utilisateur et part directement dans un `LIKE`. Les trois
/// caractères que `LIKE` interprète (`%`, `_`, et le caractère d'échappement)
/// sont donc échappés par `echapper_like` avant d'être encadrés de deux jokers :
/// un « % » tapé par l'utilisateur devient un vrai pour cent, pas « n'importe
/// quelle suite de caractères ».
///
/// Sans cela, une recherche sur `%` seul ramène **toute la base** (motif `%%%`),
/// et une recherche sur `_` ramène tout ce qui contient au moins un caractère.
/// C'est une erreur banale et invisible : rien ne plante, la page s'affiche, on
/// croit simplement que la recherche est « large ». Elle ne l'est pas : elle
/// ignore le terme saisi.
///
/// L'insensibilité à la casse est forcée par `LOWER()` des deux côtés, et non
/// laissée au classement de la colonne : une base restaurée avec un classement
/// `_bin` chercherait soudain en respectant la casse, et « Mortier » ne
/// trouverait plus « mortier ».
///
/// Un terme vide ou blanc rend une liste vide : une recherche vide n'est pas une
/// demande de tout afficher. `limite` est plafonnée à [`RECHERCHE_MAX`].
///
/// Le statut n'est **pas** filtré : une fiche dépubliée reste trouvable. C'est
/// voulu — on cherche aussi ce qu'on a retiré, pour le remettre — et c'est à
/// l'écran de ne montrer que le publié s'il s'adresse au client.
pub fn rechercher(
    pool: &Pool,
    entity: i64,
    terme: &str,
    limite: u32,
) -> Result<Vec<Fiche>, Box<dyn Error>> {
    if terme.trim().is_empty() {
        return Ok(Vec::new());
    }
    let motif = motif_recherche(terme);
    let limite = limite.clamp(1, RECHERCHE_MAX);

    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM {TABLE} k \
         LEFT JOIN llx_ticket t ON t.rowid = k.fk_ticket \
         WHERE k.entity = :entity \
           AND (LOWER(k.question) LIKE LOWER(:motif) ESCAPE '{ECHAPPEMENT}' \
                OR LOWER(COALESCE(k.answer, '')) LIKE LOWER(:motif) ESCAPE '{ECHAPPEMENT}') \
         ORDER BY k.date_creation DESC, k.rowid DESC LIMIT :limite"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "motif" => motif.as_str(), "limite" => limite },
    )?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Publie une fiche (`status` 0 → 1).
pub fn publier(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    changer_statut(pool, entity, rowid, STATUT_PUBLIEE)
}

/// Dépublie une fiche (`status` 1 → 0) : elle quitte la base de connaissances
/// sans être supprimée, ce qui laisse la trace de la question.
pub fn depublier(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    changer_statut(pool, entity, rowid, STATUT_BROUILLON)
}

/// Bascule un statut, en refusant de « changer » vers l'état déjà atteint.
///
/// Republier une fiche déjà publiée ferait croire à une action qui n'a rien
/// changé ; surtout, cela masquerait un double clic ou un appel rejoué. On rend
/// une erreur explicite.
fn changer_statut(pool: &Pool, entity: i64, rowid: i64, cible: i16) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_knowledgemanagement_knowledgerecord \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(statut) = statut else {
        return Err(format!("la fiche {rowid} n'existe pas dans l'entité {entity}").into());
    };
    if statut as i16 == cible {
        let message = if cible == STATUT_PUBLIEE {
            "cette fiche est déjà publiée"
        } else {
            "cette fiche est déjà en brouillon"
        };
        return Err(message.into());
    }
    conn.exec_drop(
        "UPDATE llx_knowledgemanagement_knowledgerecord SET status = :statut \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => cible, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Crée une fiche à partir d'un ticket, et renvoie son `rowid`.
///
/// La fiche **reprend le ticket** : le lien est posé dans `fk_ticket`, la
/// catégorie de tickets du ticket est reprise quand son code se résout dans le
/// dictionnaire, et l'auteur est celui du ticket s'il existe encore — sinon le
/// plus ancien utilisateur de l'entité, parce que la colonne est `NOT NULL` et
/// qu'une fiche sans auteur ne peut pas s'écrire.
///
/// **Le ticket lui-même n'est pas modifié.** Une fiche de connaissance n'est pas
/// une réponse au client : elle ne ferme pas le ticket, ne lui ajoute pas de
/// message et ne change pas son statut. Le ticket reste une réclamation à
/// traiter ; la fiche est ce qu'on garde pour la fois suivante. Confondre les
/// deux ferait croire au client qu'on lui a répondu alors qu'on a seulement
/// rangé la réponse dans un classeur.
///
/// La réponse est obligatoire ici, contrairement à [`create`] : une fiche tirée
/// d'un ticket résolu sans réponse ne garde rien.
///
/// # Erreurs
///
/// Ticket inexistant dans l'entité, réponse vide, aucun utilisateur dans
/// l'entité, et les contrôles de [`create`] (question, adresse, longueurs).
pub fn depuis_ticket(
    pool: &Pool,
    entity: i64,
    fk_ticket: i64,
    question: &str,
    answer: &str,
) -> Result<i64, Box<dyn Error>> {
    if answer.trim().is_empty() {
        return Err("une fiche de connaissance sans réponse ne garde rien : \
                    la réponse est obligatoire"
            .into());
    }
    let mut conn = pool.get_conn()?;
    let ticket: Option<(Option<i64>, Option<String>)> = conn.exec_first(
        "SELECT fk_user_create, category_code FROM llx_ticket \
         WHERE entity = :entity AND rowid = :ticket",
        params! { "entity" => entity, "ticket" => fk_ticket },
    )?;
    let Some((auteur_ticket, code_categorie)) = ticket else {
        return Err(format!("le ticket {fk_ticket} n'existe pas dans l'entité {entity}").into());
    };

    // L'auteur du ticket peut avoir été supprimé depuis : dans ce cas il ne
    // satisfait plus la clé étrangère, et on retombe sur un utilisateur réel.
    let auteur_existe: Option<i64> = match auteur_ticket.filter(|a| *a > 0) {
        Some(a) => conn.exec_first(
            "SELECT rowid FROM llx_user WHERE entity = :entity AND rowid = :auteur",
            params! { "entity" => entity, "auteur" => a },
        )?,
        None => None,
    };
    let auteur = match auteur_existe {
        Some(a) => a,
        None => {
            let plus_ancien: Option<Option<i64>> = conn.exec_first(
                "SELECT MIN(rowid) FROM llx_user WHERE entity = :entity",
                params! { "entity" => entity },
            )?;
            plus_ancien.flatten().ok_or_else(|| {
                format!("aucun utilisateur dans l'entité {entity} : impossible de nommer un auteur")
            })?
        }
    };

    // Le ticket porte un code de catégorie (`category_code`), la fiche un
    // identifiant : la correspondance se fait dans le dictionnaire. Les codes y
    // sont uniques par entité (`uk_code (code, entity)`), et ce dictionnaire est
    // commun aux entités : on prend la première ligne qui porte le code. Si le
    // code ne s'y trouve pas, la fiche reste sans catégorie — mieux vaut une
    // fiche juste qu'un refus pour une catégorie devenue obsolète.
    let categorie: Option<i64> = match texte(code_categorie.as_deref()) {
        Some(code) => conn.exec_first(
            "SELECT rowid FROM llx_c_ticket_category WHERE code = :code \
             ORDER BY rowid ASC LIMIT 1",
            params! { "code" => code },
        )?,
        None => None,
    };

    let new = NewFiche {
        question: question.to_string(),
        answer: Some(answer.to_string()),
        url: None,
        lang: None,
        fk_ticket: Some(fk_ticket),
        fk_c_ticket_category: categorie,
        fk_user_creat: auteur,
    };
    create(pool, entity, &new)
}

#[cfg(test)]
mod tests {
    //! Ce qui se vérifie sans base : l'échappement et les contrôles de saisie.

    use super::*;

    fn fiche(question: &str) -> NewFiche {
        NewFiche {
            question: question.to_string(),
            fk_user_creat: 1,
            ..Default::default()
        }
    }

    #[test]
    fn l_echappement_ne_laisse_passer_aucun_joker() {
        assert_eq!(echapper_like("20 %"), "20 !%");
        assert_eq!(echapper_like("a_b"), "a!_b");
        assert_eq!(echapper_like("%_%"), "!%!_!%");
        // Le caractère d'échappement lui-même doit être doublé, sinon on ne
        // pourrait plus chercher un « ! » littéral.
        assert_eq!(echapper_like("attention !"), "attention !!");
        assert_eq!(echapper_like("mortier bâtard"), "mortier bâtard");
        assert_eq!(echapper_like(""), "");
    }

    #[test]
    fn le_motif_encadre_apres_avoir_echappe() {
        // Le pour cent final doit rester un pour cent : c'est l'encadrement qui
        // apporte les jokers, pas le terme.
        assert_eq!(motif_recherche("20 %"), "%20 !%%");
        assert_eq!(motif_recherche("  chaux  "), "%chaux%");
        assert_eq!(motif_recherche("   "), "%%");
    }

    #[test]
    fn une_question_vide_ou_blanche_est_refusee() {
        for q in ["", "   ", "\n\t", " \u{a0} "] {
            assert!(valider(&fiche(q)).is_err(), "question acceptée : {q:?}");
        }
    }

    #[test]
    fn une_question_de_plus_de_65_000_octets_est_refusee() {
        let juste = "a".repeat(QUESTION_MAX);
        assert!(valider(&fiche(&juste)).is_ok());

        let trop = "a".repeat(QUESTION_MAX + 1);
        let erreur = valider(&fiche(&trop)).expect_err("question trop longue acceptée");
        assert!(erreur.to_string().contains("octets"), "message : {erreur}");

        // Le compte est en octets : « é » en pèse deux, donc 32 501 accents
        // dépassent la limite alors qu'ils ne font que 32 501 caractères.
        let accents = "é".repeat(QUESTION_MAX / 2 + 1);
        assert!(valider(&fiche(&accents)).is_err());
    }

    #[test]
    fn une_url_doit_commencer_par_http() {
        let mut f = fiche("Question");
        f.url = Some("maconnerie-architecture.fr/faq".to_string());
        let erreur = valider(&f).expect_err("chemin accepté comme adresse");
        assert!(erreur.to_string().contains("http"), "message : {erreur}");

        f.url = Some("ftp://exemple.test/notice.pdf".to_string());
        assert!(valider(&f).is_err());

        // La casse du schéma ne doit pas compter : un lien collé depuis un
        // navigateur arrive souvent en majuscules.
        f.url = Some("HTTPS://EXEMPLE.TEST/faq".to_string());
        assert!(valider(&f).is_ok());

        // Blanc vaut absence, pas adresse invalide.
        f.url = Some("   ".to_string());
        assert_eq!(valider(&f).unwrap().url, None);
    }

    #[test]
    fn une_url_de_plus_de_255_caracteres_est_refusee() {
        let mut f = fiche("Question");
        f.url = Some(format!("https://exemple.test/{}", "a".repeat(URL_MAX)));
        assert!(valider(&f).is_err());
    }

    #[test]
    fn une_langue_de_plus_de_six_caracteres_est_refusee() {
        let mut f = fiche("Question");
        f.lang = Some("français".to_string());
        assert!(valider(&f).is_err());

        f.lang = Some("fr_FR".to_string());
        assert_eq!(valider(&f).unwrap().lang.as_deref(), Some("fr_FR"));

        f.lang = Some("".to_string());
        assert_eq!(valider(&f).unwrap().lang, None);
    }

    #[test]
    fn une_reponse_blanche_vaut_absence() {
        let mut f = fiche("Question");
        f.answer = Some("   \n ".to_string());
        assert_eq!(valider(&f).unwrap().answer, None);
    }

    #[test]
    fn les_statuts_sont_ceux_de_dolibarr() {
        assert_eq!(STATUT_BROUILLON, 0);
        assert_eq!(STATUT_PUBLIEE, 1);
    }
}
