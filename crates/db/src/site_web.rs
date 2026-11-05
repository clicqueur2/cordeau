//! Sites web et pages publiées (`llx_website`, `llx_website_page`).
//!
//! Un site, c'est une **adresse** et un ensemble de pages ; une page, c'est une
//! adresse publique et un contenu. Rien de plus. Dolibarr range les deux dans
//! deux tables : `llx_website` porte le site (référence, hôte virtuel, langue,
//! page d'accueil), `llx_website_page` porte les pages et les rattache par
//! `fk_website`.
//!
//! ## Ce que la base dit vraiment (relevé le 5 novembre 2026)
//!
//! Relevé sur la base de développement (`SHOW INDEX FROM …`, MariaDB 11.4.5),
//! parce que le schéma du cœur ne dit pas tout et qu'une note contraire
//! circulait :
//!
//! - `llx_website` porte **`uk_website_ref` sur `(ref, entity)`, unique**. La
//!   référence est donc protégée par la base — mais par un index **composite** :
//!   deux entités peuvent chacune avoir un site « site-principal », et c'est
//!   voulu (deux clients, deux sites du même nom). On vérifie quand même avant
//!   d'écrire, pour rendre une phrase française plutôt qu'une « Duplicate
//!   entry », et on rattrape le code 1062 si deux saisies se croisent.
//! - `llx_website_page` porte **`uk_website_page_url` sur `(fk_website, pageurl)`,
//!   unique**. Conséquence utile : la même adresse `/contact` dans **deux sites
//!   différents** est légitime, et elle est acceptée ; la même adresse deux fois
//!   dans **le même** site est refusée. Ce n'est pas une règle qu'on invente,
//!   c'est l'index qui la porte.
//! - `llx_website_page` **n'a pas de colonne `entity`**. L'entité se rejoint par
//!   `llx_website.entity` : toute lecture de page passe par une jointure, sans
//!   quoi une page d'une autre entité serait lisible avec le bon identifiant.
//!   C'est la raison pour laquelle [`pages`], [`page`], [`renommer_page`],
//!   [`publier_page`], [`depublier_page`] et [`supprimer_page`] prennent
//!   `entity` et joignent le site au lieu de filtrer la page.
//! - `type_container` est `NOT NULL DEFAULT 'page'` **sur les deux tables**, et
//!   `Website::create()` du cœur (`_vendor/dolibarr-ref/htdocs/website/class/website.class.php`)
//!   ne l'écrit pas : un site créé par Dolibarr porte donc `'page'` dans cette
//!   colonne. On écrit `'website'` à la création comme demandé, parce que la
//!   colonne sert à dire ce que contient une ligne et que `'website'` est sa
//!   valeur honnête ; mais **on ne filtre jamais dessus en lecture**, sinon
//!   aucun site créé par Dolibarr ne serait reconnu.
//! - `content` est un `mediumtext` (16 777 215 octets) : il n'est **pas** lu par
//!   [`pages`]. Une page de documentation pèse vite plusieurs mégaoctets, et une
//!   liste de cinquante pages n'a pas à les charger pour afficher des titres.
//! - `llx_categorie_website_page` porte une clé étrangère vers la page : une page
//!   étiquetée ne se supprime pas sans retirer ses étiquettes d'abord.
//! - `llx_website_extrafields` porte les champs complémentaires du site
//!   (`fk_object`) : on les retire avec le site, comme le `delete()` du cœur.
//!
//! ## Ce que ce module décide, et que Dolibarr ne fait pas
//!
//! - **Un site qui a des pages ne se supprime pas.** `Website::delete()` du cœur
//!   efface les pages en cascade ; ici on refuse et on le dit. Un site, c'est un
//!   dossier de travail : le supprimer d'un clic avec ses vingt pages est une
//!   perte qu'aucune confirmation ne rattrape.
//! - **La page d'accueil ne reste pas dans le vide.** `WebsitePage::delete()` du
//!   cœur ne touche pas à `llx_website.fk_default_home` : le site garde alors un
//!   pointeur vers une page disparue. [`supprimer_page`] remet `fk_default_home` à
//!   `NULL` quand c'est cette page-là qu'on supprime.
//! - **Pas de numérotation automatique.** À la différence des fiches de
//!   connaissance (`divers::suivant_ref`), la référence d'un site n'est pas un
//!   compteur : c'est un nom choisi (« maconnerie-architecture »), qui sert
//!   d'adresse et de dossier. On ne le remplace pas par `WS0007`.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool, Row};

/// La table des sites.
pub const TABLE: &str = "llx_website";

/// La table des pages.
pub const TABLE_PAGE: &str = "llx_website_page";

/// Ce qu'on écrit dans `type_container` pour un site.
///
/// Voir l'en-tête : Dolibarr laisse la valeur par défaut (`page`) sur les sites
/// qu'il crée. On écrit la valeur juste, mais on ne s'en sert jamais pour
/// filtrer.
pub const CONTENEUR_SITE: &str = "website";

/// Ce qu'on écrit dans `type_container` pour une page.
pub const CONTENEUR_PAGE: &str = "page";

/// Statuts Dolibarr, identiques sur les deux tables : 0 = hors ligne,
/// 1 = en ligne.
pub const STATUT_HORS_LIGNE: i16 = 0;
pub const STATUT_EN_LIGNE: i16 = 1;

/// Longueur maximale d'une référence de site, en **octets** : `ref` est un
/// varchar(128).
///
/// Le compte est en octets, pas en caractères : « é » en pèse deux, donc une
/// référence pleine d'accents atteint la limite deux fois plus vite qu'on ne le
/// croit en la relisant à l'écran.
pub const REF_MAX: usize = 128;

/// Longueur maximale de la langue d'un site : `llx_website.lang` est un
/// varchar(8), deux octets de plus que celui d'une page.
pub const LANGUE_SITE_MAX: usize = 8;

/// Longueur maximale de la langue d'une page : `llx_website_page.lang` est un
/// varchar(6), juste de quoi écrire `fr_FR`.
pub const LANGUE_PAGE_MAX: usize = 6;

/// Longueur maximale d'une adresse de page : `pageurl` est un varchar(255).
pub const ADRESSE_MAX: usize = 255;

/// Longueur maximale des textes courts : titre, description, mots-clés,
/// `virtualhost`, image, méta-robots — tous des varchar(255).
pub const COURT_MAX: usize = 255;

/// Longueur maximale d'un contenu, en octets.
///
/// La colonne est un `mediumtext` : 16 777 215 octets. On refuse dès 16 000 000
/// pour garder une marge et rendre une phrase française, plutôt que de laisser
/// le serveur tronquer ou refuser avec un message en anglais.
pub const CONTENU_MAX: usize = 16_000_000;

/// Plafond de [`sites`] : une liste n'est pas un export de la base.
pub const SITES_MAX: u32 = 500;

/// Un site, tel qu'on le relit dans la liste.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Site {
    pub rowid: i64,
    /// Référence du site, qui sert aussi d'adresse et de nom de dossier. Le
    /// suffixe `_` est imposé : `ref` est un mot-clé Rust.
    pub ref_: String,
    pub description: Option<String>,
    /// Code de langue (`fr_FR`, `en_US`…).
    pub langue: Option<String>,
    /// 0 = hors ligne, 1 = en ligne.
    pub statut: i16,
    /// Hôte virtuel, quand le site répond à son propre nom de domaine.
    pub virtualhost: Option<String>,
    /// Page d'accueil (`fk_default_home`), si elle est désignée.
    pub accueil: Option<i64>,
    /// Date de création, `AAAA-MM-JJ HH:MM`.
    pub date: Option<String>,
    /// Nombre de pages, **compté par jointure** et non stocké : un compteur
    /// qu'on oublierait de décrémenter mentirait à la première suppression.
    pub pages: i64,
    /// Vues cumulées (`pageviews_total`), telle que Dolibarr les compte.
    pub vues: i64,
}

impl Site {
    /// En ligne ? C'est `status = 1` chez Dolibarr.
    pub fn est_en_ligne(&self) -> bool {
        self.statut == STATUT_EN_LIGNE
    }
}

/// Une page, telle qu'on la relit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    pub rowid: i64,
    pub fk_website: i64,
    /// Référence du site, relue par jointure : c'est le nom qu'on affiche à côté
    /// de la page, pas la peine de le redemander.
    pub reference_site: Option<String>,
    /// L'adresse publique de la page (`pageurl`) : `contact`, `realisations/…`.
    /// C'est l'identifiant que les visiteurs tapent et que les moteurs indexent.
    pub adresse: String,
    pub titre: Option<String>,
    pub description: Option<String>,
    pub mots_cles: Option<String>,
    /// Code de langue de la page, qui peut différer de celui du site.
    pub langue: Option<String>,
    /// Consigne aux robots (`index, follow`…).
    pub meta_robots: Option<String>,
    /// Signature de l'auteur, affichée sur la page.
    pub alias_auteur: Option<String>,
    /// 0 = hors ligne, 1 = en ligne.
    pub statut: i16,
    /// Date de création, `AAAA-MM-JJ HH:MM`.
    pub date: Option<String>,
    /// Contenu HTML.
    ///
    /// `None` veut dire **non lu** dans [`pages`] (le contenu peut peser des
    /// mégaoctets) ; dans [`page`], il est lu, et `None` signifie alors que la
    /// colonne est vide en base.
    pub contenu: Option<String>,
}

impl Page {
    /// En ligne ? Une page hors ligne reste en base : on la retire du site sans
    /// la perdre.
    pub fn est_en_ligne(&self) -> bool {
        self.statut == STATUT_EN_LIGNE
    }
}

/// Ce qu'il faut pour créer un site.
#[derive(Debug, Clone, Default)]
pub struct NewSite {
    /// Obligatoire, non vide, 128 octets au plus. Espaces de tête et de queue
    /// retirés avant écriture : une référence qui commence par une espace est
    /// introuvable dans une liste triée.
    pub ref_: String,
    /// Facultative : un site peut se créer avant d'être décrit.
    pub description: Option<String>,
    /// Facultative (`fr_FR`), 8 caractères au plus.
    pub lang: Option<String>,
    /// Facultatif : le nom de domaine servi, 255 caractères au plus.
    pub virtualhost: Option<String>,
}

/// Ce qu'il faut pour créer une page.
#[derive(Debug, Clone, Default)]
pub struct NewPage {
    /// Le site de la page. Doit exister dans l'entité.
    pub fk_website: i64,
    /// Obligatoire, non vide, 255 octets au plus, **unique dans le site**.
    pub pageurl: String,
    /// Titre affiché dans l'onglet et dans les résultats de recherche.
    pub title: Option<String>,
    /// Méta-description.
    pub description: Option<String>,
    /// Méta-mots-clés.
    pub keywords: Option<String>,
    /// Code de langue, 6 caractères au plus.
    pub lang: Option<String>,
    /// Obligatoire, non vide : une page sans contenu est une adresse qui ne
    /// montre rien.
    pub content: String,
    /// 0 = hors ligne, 1 = en ligne. Toute autre valeur est refusée.
    pub status: i16,
}

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

/// Un texte facultatif : blanc vaut `None`, jamais `''`.
///
/// La distinction compte pour l'affichage : une colonne `NULL` se tait, une
/// chaîne vide affiche « Titre : » suivi de rien, ce qui ressemble à une saisie
/// oubliée.
fn texte(v: Option<&str>) -> Option<String> {
    v.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Le doublon MySQL (code 1062), pour rattraper une course entre deux saisies.
fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

/// Textes d'un site validés, prêts à être écrits.
struct SiteValide {
    reference: String,
    description: Option<String>,
    lang: Option<String>,
    virtualhost: Option<String>,
}

/// Valide et nettoie les champs d'un site.
fn valider_site(new: &NewSite) -> Result<SiteValide, Box<dyn Error>> {
    let reference = new.ref_.trim();
    if reference.is_empty() {
        return Err(
            "la référence du site est obligatoire : c'est le nom sous lequel \
                    on le retrouve, et celui qui sert d'adresse"
                .into(),
        );
    }
    if reference.len() > REF_MAX {
        return Err(format!(
            "la référence du site fait {} octets : la colonne `ref` est un varchar({REF_MAX})",
            reference.len()
        )
        .into());
    }

    let description = texte(new.description.as_deref());
    if let Some(trop_longue) = description.as_ref().filter(|d| d.len() > COURT_MAX) {
        return Err(format!(
            "la description fait {} octets : la colonne `description` est un varchar({COURT_MAX})",
            trop_longue.len()
        )
        .into());
    }

    let lang = texte(new.lang.as_deref());
    if let Some(trop_longue) = lang.as_ref().filter(|l| l.len() > LANGUE_SITE_MAX) {
        return Err(format!(
            "la langue « {trop_longue} » dépasse {LANGUE_SITE_MAX} caractères : la colonne \
             `lang` d'un site est un varchar(8), de quoi écrire un code du type fr_FR"
        )
        .into());
    }

    let virtualhost = texte(new.virtualhost.as_deref());
    if let Some(trop_long) = virtualhost.as_ref().filter(|v| v.len() > COURT_MAX) {
        return Err(format!(
            "le nom de domaine fait {} octets : la colonne `virtualhost` est un varchar({COURT_MAX})",
            trop_long.len()
        )
        .into());
    }

    Ok(SiteValide {
        reference: reference.to_string(),
        description,
        lang,
        virtualhost,
    })
}

/// Textes d'une page validés, prêts à être écrits.
struct PageValide {
    adresse: String,
    titre: Option<String>,
    description: Option<String>,
    mots_cles: Option<String>,
    lang: Option<String>,
    contenu: String,
    statut: i16,
}

/// Valide et nettoie les champs d'une page.
///
/// Partagé par [`creer_page`] et [`renommer_page`] pour ce qui concerne
/// l'adresse : les deux écrivent la même colonne et doivent refuser les mêmes
/// choses. Un contrôle qui n'existerait qu'à la création laisserait passer par
/// le renommage ce qu'on interdit à la saisie.
fn valider_adresse(brut: &str) -> Result<String, Box<dyn Error>> {
    let adresse = brut.trim();
    if adresse.is_empty() {
        return Err(
            "l'adresse de la page est obligatoire : c'est son identifiant public, \
                    celui qui apparaît dans la barre du navigateur"
                .into(),
        );
    }
    if adresse.len() > ADRESSE_MAX {
        return Err(format!(
            "l'adresse fait {} octets : la colonne `pageurl` est un varchar({ADRESSE_MAX})",
            adresse.len()
        )
        .into());
    }
    Ok(adresse.to_string())
}

/// Valide et nettoie les champs d'une page.
fn valider_page(new: &NewPage) -> Result<PageValide, Box<dyn Error>> {
    let adresse = valider_adresse(&new.pageurl)?;

    let contenu = new.content.trim();
    if contenu.is_empty() {
        return Err(
            "le contenu de la page est obligatoire : une page sans contenu \
                    est une adresse qui ne montre rien"
                .into(),
        );
    }
    if contenu.len() > CONTENU_MAX {
        return Err(format!(
            "le contenu fait {} octets : la colonne `content` est un mediumtext limité à \
             16 777 215 octets, on refuse au-delà de {CONTENU_MAX}",
            contenu.len()
        )
        .into());
    }

    let lang = texte(new.lang.as_deref());
    if let Some(trop_longue) = lang.as_ref().filter(|l| l.len() > LANGUE_PAGE_MAX) {
        return Err(format!(
            "la langue « {trop_longue} » dépasse {LANGUE_PAGE_MAX} caractères : la colonne \
             `lang` d'une page est un varchar(6), juste de quoi écrire fr_FR"
        )
        .into());
    }

    let titre = texte(new.title.as_deref());
    if let Some(trop_long) = titre.as_ref().filter(|t| t.len() > COURT_MAX) {
        return Err(format!(
            "le titre fait {} octets : la colonne `title` est un varchar({COURT_MAX})",
            trop_long.len()
        )
        .into());
    }
    let description = texte(new.description.as_deref());
    if let Some(trop_longue) = description.as_ref().filter(|d| d.len() > COURT_MAX) {
        return Err(format!(
            "la description fait {} octets : la colonne `description` est un varchar({COURT_MAX})",
            trop_longue.len()
        )
        .into());
    }
    let mots_cles = texte(new.keywords.as_deref());
    if let Some(trop_longs) = mots_cles.as_ref().filter(|m| m.len() > COURT_MAX) {
        return Err(format!(
            "les mots-clés font {} octets : la colonne `keywords` est un varchar({COURT_MAX})",
            trop_longs.len()
        )
        .into());
    }

    // Un statut ni 0 ni 1 serait écrit tel quel et Dolibarr le lirait comme
    // « en ligne » (tout ce qui n'est pas 0 est vrai) : on refuse plutôt que de
    // publier par accident.
    if new.status != STATUT_HORS_LIGNE && new.status != STATUT_EN_LIGNE {
        return Err(format!(
            "le statut {} n'existe pas : une page est hors ligne (0) ou en ligne (1)",
            new.status
        )
        .into());
    }

    Ok(PageValide {
        adresse,
        titre,
        description,
        mots_cles,
        lang,
        contenu: contenu.to_string(),
        statut: new.status,
    })
}

/// Colonnes d'une page, sans son contenu. L'index 12 est réservé au contenu,
/// que seule [`page`] ajoute.
const SELECT_PAGE: &str = "p.rowid, p.fk_website, w.ref, p.pageurl, p.title, p.description, \
     p.keywords, p.lang, p.metarobots, p.author_alias, p.status, \
     DATE_FORMAT(p.date_creation, '%Y-%m-%d %H:%i')";

fn map_page(row: &Row) -> Page {
    Page {
        rowid: opt_i(row, 0).unwrap_or(0),
        fk_website: opt_i(row, 1).unwrap_or(0),
        reference_site: opt_s(row, 2),
        adresse: opt_s(row, 3).unwrap_or_default(),
        titre: opt_s(row, 4),
        description: opt_s(row, 5),
        mots_cles: opt_s(row, 6),
        langue: opt_s(row, 7),
        meta_robots: opt_s(row, 8),
        alias_auteur: opt_s(row, 9),
        // Un statut `NULL` se lit hors ligne : on ne suppose jamais en ligne ce
        // qui n'est pas écrit.
        statut: opt_i(row, 10).unwrap_or(0) as i16,
        date: opt_s(row, 11),
        contenu: None,
    }
}

fn map_site(row: Row) -> Site {
    Site {
        rowid: opt_i(&row, 0).unwrap_or(0),
        ref_: opt_s(&row, 1).unwrap_or_default(),
        description: opt_s(&row, 2),
        langue: opt_s(&row, 3),
        statut: opt_i(&row, 4).unwrap_or(0) as i16,
        virtualhost: opt_s(&row, 5),
        // `fk_default_home` vaut 0 chez Dolibarr quand aucune page d'accueil
        // n'est désignée : 0 et NULL disent la même chose, on les ramène à
        // `None` pour que l'appelant n'ait qu'un cas à traiter.
        accueil: opt_i(&row, 6).filter(|id| *id > 0),
        date: opt_s(&row, 7),
        pages: opt_i(&row, 8).unwrap_or(0),
        vues: opt_i(&row, 9).unwrap_or(0),
    }
}

/// Crée un site et renvoie son `rowid`.
///
/// Le site naît **en ligne** (`status = 1`), comme le fait Dolibarr, et
/// `date_creation` est posée à `NOW()` : c'est la date à laquelle on a ouvert le
/// site dans la base, pas une date reprise d'un formulaire.
///
/// # Unicité de la référence
///
/// La base porte `uk_website_ref (ref, entity)`, unique : deux sites de même
/// référence **dans la même entité** sont impossibles, et c'est l'index qui le
/// garantit. On vérifie quand même avant, pour rendre une phrase lisible (« un
/// site porte déjà cette référence ») au lieu d'une « Duplicate entry » ; et on
/// rattrape le 1062 si deux saisies se croisent entre la vérification et
/// l'écriture. Deux entités différentes peuvent avoir le même nom de site : la
/// référence est unique **par entité**, ce qui est le comportement voulu.
///
/// # Erreurs
///
/// Référence vide ou de plus de 128 octets, référence déjà prise dans l'entité,
/// description ou nom de domaine de plus de 255 octets, langue de plus de
/// 8 caractères, et toute erreur MySQL remontée telle quelle.
pub fn creer_site(pool: &Pool, entity: i64, new: &NewSite) -> Result<i64, Box<dyn Error>> {
    let site = valider_site(new)?;
    let reference = site.reference.as_str();
    let mut conn = pool.get_conn()?;

    let deja: Option<String> = conn.exec_first(
        "SELECT ref FROM llx_website WHERE entity = :entity AND ref = :reference",
        params! { "entity" => entity, "reference" => reference },
    )?;
    if deja.is_some() {
        return Err(format!(
            "un site porte déjà la référence « {reference} » dans l'entité {entity} : \
             la référence est unique par entité, on ne peut pas en avoir deux"
        )
        .into());
    }

    let insere = conn.exec_drop(
        "INSERT INTO llx_website \
         (entity, type_container, ref, description, lang, virtualhost, status, \
          position, date_creation) \
         VALUES (:entity, :conteneur, :reference, :description, :lang, :virtualhost, \
                 :statut, 0, NOW())",
        params! {
            "entity" => entity,
            // `type_container` a un défaut (`page`) sur cette table : ne pas
            // l'écrire ferait passer le site pour une page. Voir l'en-tête.
            "conteneur" => CONTENEUR_SITE,
            "reference" => reference,
            "description" => site.description.as_deref(),
            "lang" => site.lang.as_deref(),
            "virtualhost" => site.virtualhost.as_deref(),
            "statut" => STATUT_EN_LIGNE,
        },
    );
    match insere {
        Ok(()) => Ok(conn.last_insert_id() as i64),
        Err(e) if is_duplicate(&e) => Err(format!(
            "un site porte déjà la référence « {reference} » dans l'entité {entity} : \
             une autre saisie vient de la prendre"
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

/// Crée une page et renvoie son `rowid`.
///
/// La page est rattachée au site par `fk_website` ; l'entité n'est **pas** une
/// colonne de `llx_website_page`, elle vient du site. Le site doit donc exister
/// dans l'entité, sinon la page serait orpheline ou rattachée au site d'un autre.
///
/// L'adresse est **l'identifiant public** de la page : elle est unique dans son
/// site (`uk_website_page_url (fk_website, pageurl)`), et la même adresse dans
/// deux sites différents reste permise. On vérifie avant d'écrire pour rendre
/// une phrase claire, et on rattrape le 1062 si deux saisies se croisent.
///
/// Le statut vient de l'appelant (`0` ou `1`) : un brouillon se crée hors ligne
/// et se publie par [`publier_page`]. L'auteur n'est pas demandé — la colonne
/// `fk_user_creat` est nullable et ne porte aucune clé étrangère sur cette
/// table, contrairement aux fiches de connaissance.
///
/// # Erreurs
///
/// Site inexistant dans l'entité, adresse vide ou de plus de 255 octets, adresse
/// déjà prise dans le site, contenu vide ou de plus de 16 000 000 octets, titre,
/// description ou mots-clés de plus de 255 octets, langue de plus de
/// 6 caractères, statut autre que 0 ou 1.
pub fn creer_page(pool: &Pool, entity: i64, new: &NewPage) -> Result<i64, Box<dyn Error>> {
    let page = valider_page(new)?;
    let mut conn = pool.get_conn()?;

    // Pas de filtre sur `type_container` : Dolibarr ne l'écrit pas pour un site,
    // la colonne vaut donc `page` sur les siens. Filtrer dessus reviendrait à
    // refuser tous les sites créés par le cœur.
    let site: Option<String> = conn.exec_first(
        "SELECT ref FROM llx_website WHERE entity = :entity AND rowid = :site",
        params! { "entity" => entity, "site" => new.fk_website },
    )?;
    let Some(reference_site) = site else {
        return Err(format!(
            "le site {} n'existe pas dans l'entité {entity} : une page se rattache \
             toujours à un site",
            new.fk_website
        )
        .into());
    };

    let prise: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_website_page WHERE fk_website = :site AND pageurl = :adresse",
        params! { "site" => new.fk_website, "adresse" => page.adresse.as_str() },
    )?;
    if let Some(autre) = prise {
        return Err(format!(
            "l'adresse « {} » est déjà prise dans le site « {reference_site} » par la page \
             {autre} : c'est l'identifiant public de la page, deux pages ne peuvent pas \
             la partager",
            page.adresse
        )
        .into());
    }

    let insere = conn.exec_drop(
        "INSERT INTO llx_website_page \
         (fk_website, type_container, pageurl, title, description, keywords, lang, \
          content, status, date_creation) \
         VALUES (:site, :conteneur, :adresse, :titre, :description, :mots_cles, :lang, \
                 :contenu, :statut, NOW())",
        params! {
            "site" => new.fk_website,
            "conteneur" => CONTENEUR_PAGE,
            "adresse" => page.adresse.as_str(),
            "titre" => page.titre.as_deref(),
            "description" => page.description.as_deref(),
            "mots_cles" => page.mots_cles.as_deref(),
            "lang" => page.lang.as_deref(),
            "contenu" => page.contenu.as_str(),
            "statut" => page.statut,
        },
    );
    match insere {
        Ok(()) => Ok(conn.last_insert_id() as i64),
        Err(e) if is_duplicate(&e) => Err(format!(
            "l'adresse « {} » vient d'être prise dans le site « {reference_site} » par une \
             autre saisie : c'est l'identifiant public de la page",
            page.adresse
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

/// Les sites de l'entité, du plus petit nom au plus grand, avec leur nombre de
/// pages.
///
/// Le comptage se fait par sous-requête sur `llx_website_page` : aucune colonne
/// de compteur n'existe, et un compteur tenu à la main mentirait dès la première
/// suppression faite ailleurs (Dolibarr, une reprise SQL).
///
/// `limite` est ramenée entre 1 et [`SITES_MAX`] : une limite à 0 n'a pas de sens
/// (elle ne veut pas dire « tout ») et une limite à dix millions n'est pas une
/// liste mais un vidage de table.
pub fn sites(pool: &Pool, entity: i64, limite: u32) -> Result<Vec<Site>, Box<dyn Error>> {
    let limite = limite.clamp(1, SITES_MAX);
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT w.rowid, w.ref, w.description, w.lang, w.status, w.virtualhost, \
                w.fk_default_home, DATE_FORMAT(w.date_creation, '%Y-%m-%d %H:%i'), \
                (SELECT COUNT(*) FROM llx_website_page p WHERE p.fk_website = w.rowid), \
                w.pageviews_total \
         FROM llx_website w \
         WHERE w.entity = :entity \
         ORDER BY w.ref ASC, w.rowid ASC LIMIT :limite",
        params! { "entity" => entity, "limite" => limite },
    )?;
    Ok(rows.into_iter().map(map_site).collect())
}

/// Les pages d'un site, **sans leur contenu**, de l'adresse la plus petite à la
/// plus grande.
///
/// L'entité est vérifiée par jointure : `llx_website_page` n'a pas de colonne
/// `entity`, et demander les pages d'un site d'une autre entité rend donc une
/// liste vide plutôt que ces pages-là.
///
/// Le tri suit l'adresse : c'est l'ordre dans lequel on range un plan de site, et
/// il ne bouge pas quand une page est modifiée.
pub fn pages(pool: &Pool, entity: i64, fk_website: i64) -> Result<Vec<Page>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_PAGE} FROM {TABLE_PAGE} p \
         JOIN {TABLE} w ON w.rowid = p.fk_website \
         WHERE w.entity = :entity AND p.fk_website = :site \
         ORDER BY p.pageurl ASC, p.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "site" => fk_website })?;
    Ok(rows.iter().map(map_page).collect())
}

/// Une page précise, **avec son contenu**.
///
/// C'est la seule lecture qui charge le `mediumtext` : elle sert à éditer la
/// page ou à la servir, pas à lister.
pub fn page(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Page>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_PAGE}, p.content FROM {TABLE_PAGE} p \
         JOIN {TABLE} w ON w.rowid = p.fk_website \
         WHERE w.entity = :entity AND p.rowid = :rowid"
    );
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.map(|r| {
        let mut trouvee = map_page(&r);
        trouvee.contenu = opt_s(&r, 12);
        trouvee
    }))
}

/// Met une page en ligne (`status` 1).
pub fn publier_page(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    changer_statut_page(pool, entity, rowid, STATUT_EN_LIGNE)
}

/// Retire une page du site sans la supprimer (`status` 0) : elle reste en base,
/// avec son contenu, prête à être republiée.
pub fn depublier_page(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    changer_statut_page(pool, entity, rowid, STATUT_HORS_LIGNE)
}

/// Bascule le statut d'une page, en refusant de « changer » vers l'état déjà
/// atteint.
///
/// Republier une page déjà en ligne ferait croire à une action qui n'a rien
/// changé ; surtout, cela masquerait un double clic ou un appel rejoué. On rend
/// une erreur explicite, comme pour les fiches de connaissance.
fn changer_statut_page(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    cible: i16,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT p.status FROM llx_website_page p \
         JOIN llx_website w ON w.rowid = p.fk_website \
         WHERE w.entity = :entity AND p.rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(statut) = statut else {
        return Err(format!("la page {rowid} n'existe pas dans l'entité {entity}").into());
    };
    if statut as i16 == cible {
        let message = if cible == STATUT_EN_LIGNE {
            "cette page est déjà en ligne"
        } else {
            "cette page est déjà hors ligne"
        };
        return Err(message.into());
    }
    conn.exec_drop(
        "UPDATE llx_website_page SET status = :statut WHERE rowid = :rowid",
        params! { "statut" => cible, "rowid" => rowid },
    )?;
    Ok(())
}

/// Change l'adresse d'une page, en refusant celle qui est déjà prise dans le
/// même site.
///
/// L'adresse est l'identifiant public de la page : la déplacer vers une adresse
/// occupée mettrait deux pages sous le même nom, et le serveur ne saurait plus
/// laquelle servir. La base le refuserait (`uk_website_page_url`), avec un
/// message que personne ne lit ; ici on refuse avant, en nommant la page qui
/// occupe l'adresse.
///
/// Renommer vers l'adresse actuelle est également refusé : il n'y a rien à
/// faire, et le dire vaut mieux que de laisser croire à un déplacement.
///
/// # Erreurs
///
/// Page introuvable dans l'entité, adresse vide ou de plus de 255 octets,
/// adresse déjà portée par la page elle-même, adresse déjà prise par une autre
/// page du site.
pub fn renommer_page(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    nouvelle_url: &str,
) -> Result<(), Box<dyn Error>> {
    let adresse = valider_adresse(nouvelle_url)?;
    let mut conn = pool.get_conn()?;

    let trouvee: Option<(i64, Option<String>, String)> = conn.exec_first(
        "SELECT p.fk_website, w.ref, p.pageurl FROM llx_website_page p \
         JOIN llx_website w ON w.rowid = p.fk_website \
         WHERE w.entity = :entity AND p.rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some((site, reference_site, actuelle)) = trouvee else {
        return Err(format!("la page {rowid} n'existe pas dans l'entité {entity}").into());
    };
    let nom_du_site = reference_site.unwrap_or_else(|| site.to_string());

    if actuelle == adresse {
        return Err(format!(
            "la page {rowid} porte déjà l'adresse « {adresse} » : il n'y a rien à renommer"
        )
        .into());
    }

    let prise: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_website_page \
         WHERE fk_website = :site AND pageurl = :adresse AND rowid <> :rowid",
        params! { "site" => site, "adresse" => adresse.as_str(), "rowid" => rowid },
    )?;
    if let Some(autre) = prise {
        return Err(format!(
            "l'adresse « {adresse} » est déjà prise dans le site « {nom_du_site} » par la \
             page {autre} : c'est l'identifiant public de la page, on ne la déplace pas dessus"
        )
        .into());
    }

    let change = conn.exec_drop(
        "UPDATE llx_website_page SET pageurl = :adresse WHERE rowid = :rowid",
        params! { "adresse" => adresse.as_str(), "rowid" => rowid },
    );
    match change {
        Ok(()) => Ok(()),
        Err(e) if is_duplicate(&e) => Err(format!(
            "l'adresse « {adresse} » vient d'être prise dans le site « {nom_du_site} » par \
             une autre saisie"
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

/// Supprime une page, ses étiquettes et son rôle de page d'accueil.
///
/// Trois choses partent avant la page elle-même :
///
/// - ses étiquettes de catégorie (`llx_categorie_website_page`), qui portent une
///   clé étrangère vers elle : un `DELETE` direct échouerait sur la contrainte,
///   en laissant l'utilisateur devant une erreur MySQL ;
/// - son rôle de page d'accueil : si `llx_website.fk_default_home` pointe sur
///   elle, on le remet à `NULL`. Dolibarr ne le fait pas et laisse le site
///   pointer sur une page disparue ;
/// - la page.
///
/// Le tout dans une transaction : une page à moitié déshabillée serait pire
/// qu'une page encore là.
///
/// # Erreurs
///
/// Page introuvable dans l'entité : on le dit, plutôt que de rendre un succès sur
/// une suppression qui n'a rien fait.
pub fn supprimer_page(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // L'existence se vérifie avant d'ouvrir la transaction : sans cela, un
    // identifiant d'une autre entité ferait partir ses étiquettes avant
    // d'échouer sur la page introuvable.
    let trouvee: Option<(i64, Option<i64>)> = conn.exec_first(
        "SELECT p.fk_website, w.fk_default_home FROM llx_website_page p \
         JOIN llx_website w ON w.rowid = p.fk_website \
         WHERE w.entity = :entity AND p.rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some((site, accueil)) = trouvee else {
        return Err(format!("la page {rowid} n'existe pas dans l'entité {entity}").into());
    };

    let mut tx = conn.start_transaction(mysql::TxOpts::default())?;
    if accueil == Some(rowid) {
        tx.exec_drop(
            "UPDATE llx_website SET fk_default_home = NULL WHERE rowid = :site",
            params! { "site" => site },
        )?;
    }
    tx.exec_drop(
        "DELETE FROM llx_categorie_website_page WHERE fk_website_page = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_website_page WHERE rowid = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Supprime un site **vide**, et ses champs complémentaires.
///
/// Un site qui a des pages est refusé, avec le nombre de pages dans le message.
/// C'est une décision de ce module, à l'opposé de `Website::delete()` du cœur,
/// qui efface les pages en cascade : supprimer un site, c'est jetable en un clic
/// et irrattrapable, alors qu'on ne le fait presque jamais exprès.
///
/// Il faut donc vider le site page par page ([`supprimer_page`]) avant de le
/// supprimer : deux gestes, deux fois l'occasion de voir ce qu'on jette.
///
/// # Erreurs
///
/// Site introuvable dans l'entité, ou site ayant encore des pages.
pub fn supprimer_site(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let site: Option<String> = conn.exec_first(
        "SELECT ref FROM llx_website WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(reference) = site else {
        return Err(format!("le site {rowid} n'existe pas dans l'entité {entity}").into());
    };

    let pages: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_website_page WHERE fk_website = :rowid",
        params! { "rowid" => rowid },
    )?;
    let pages = pages.unwrap_or(0);
    if pages > 0 {
        return Err(format!(
            "le site « {reference} » a {pages} page(s) : on ne supprime pas un site avec \
             ses pages par accident. Supprimez ou déplacez les pages d'abord"
        )
        .into());
    }

    let mut tx = conn.start_transaction(mysql::TxOpts::default())?;
    tx.exec_drop(
        "DELETE FROM llx_website_extrafields WHERE fk_object = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_website WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Ce qui se vérifie sans base : les contrôles de saisie et les constantes.

    use super::*;

    fn site_neuf(reference: &str) -> NewSite {
        NewSite {
            ref_: reference.to_string(),
            ..Default::default()
        }
    }

    fn page_neuve(adresse: &str, contenu: &str) -> NewPage {
        NewPage {
            fk_website: 1,
            pageurl: adresse.to_string(),
            content: contenu.to_string(),
            status: STATUT_EN_LIGNE,
            ..Default::default()
        }
    }

    #[test]
    fn les_statuts_sont_ceux_de_dolibarr() {
        assert_eq!(STATUT_HORS_LIGNE, 0);
        assert_eq!(STATUT_EN_LIGNE, 1);
    }

    #[test]
    fn les_conteneurs_sont_ceux_de_dolibarr() {
        assert_eq!(CONTENEUR_SITE, "website");
        assert_eq!(CONTENEUR_PAGE, "page");
        // `type_container` est un varchar(16) : les deux valeurs y tiennent.
        assert!(CONTENEUR_SITE.len() <= 16);
        assert!(CONTENEUR_PAGE.len() <= 16);
    }

    #[test]
    fn une_reference_de_site_vide_est_refusee() {
        for reference in ["", "   ", "\n\t", " \u{a0} "] {
            assert!(
                valider_site(&site_neuf(reference)).is_err(),
                "référence acceptée : {reference:?}"
            );
        }
    }

    #[test]
    fn une_reference_de_site_de_plus_de_128_octets_est_refusee() {
        let juste = "a".repeat(REF_MAX);
        assert_eq!(valider_site(&site_neuf(&juste)).unwrap().reference, juste);

        let trop = "a".repeat(REF_MAX + 1);
        let erreur = valider_site(&site_neuf(&trop)).expect_err("référence trop longue acceptée");
        assert!(erreur.to_string().contains("octets"), "message : {erreur}");

        // Le compte est en octets : « é » en pèse deux.
        let accents = "é".repeat(REF_MAX / 2 + 1);
        assert!(valider_site(&site_neuf(&accents)).is_err());
    }

    #[test]
    fn les_espaces_autour_d_une_reference_sont_retires() {
        let propre = valider_site(&site_neuf("  maconnerie  ")).unwrap();
        assert_eq!(propre.reference, "maconnerie");
    }

    #[test]
    fn une_langue_de_site_de_plus_de_huit_octets_est_refusee() {
        let mut s = site_neuf("site");
        s.lang = Some("français".to_string());
        assert!(valider_site(&s).is_err());

        s.lang = Some("fr_FR".to_string());
        assert_eq!(valider_site(&s).unwrap().lang.as_deref(), Some("fr_FR"));

        // Blanc vaut absence, pas langue invalide.
        s.lang = Some("   ".to_string());
        assert_eq!(valider_site(&s).unwrap().lang, None);
    }

    #[test]
    fn un_nom_de_domaine_de_plus_de_255_octets_est_refuse() {
        let mut s = site_neuf("site");
        s.virtualhost = Some(format!("{}.test", "a".repeat(COURT_MAX)));
        assert!(valider_site(&s).is_err());
    }

    #[test]
    fn une_adresse_vide_est_refusee() {
        for adresse in ["", "   ", "\n\t"] {
            assert!(
                valider_page(&page_neuve(adresse, "<p>Bonjour</p>")).is_err(),
                "adresse acceptée : {adresse:?}"
            );
        }
    }

    #[test]
    fn une_adresse_de_plus_de_255_octets_est_refusee() {
        let trop = "a".repeat(ADRESSE_MAX + 1);
        let erreur = valider_page(&page_neuve(&trop, "<p>Bonjour</p>"))
            .expect_err("adresse trop longue acceptée");
        assert!(erreur.to_string().contains("octets"), "message : {erreur}");
    }

    #[test]
    fn un_contenu_vide_est_refuse() {
        for contenu in ["", "   ", "\n\t "] {
            let erreur =
                valider_page(&page_neuve("contact", contenu)).expect_err("contenu vide accepté");
            assert!(erreur.to_string().contains("contenu"), "message : {erreur}");
        }
    }

    #[test]
    fn un_statut_inconnu_est_refuse() {
        let mut p = page_neuve("contact", "<p>Bonjour</p>");
        p.status = 2;
        let erreur = valider_page(&p).expect_err("statut 2 accepté");
        assert!(erreur.to_string().contains("statut"), "message : {erreur}");

        p.status = -1;
        assert!(valider_page(&p).is_err());

        p.status = STATUT_HORS_LIGNE;
        assert_eq!(valider_page(&p).unwrap().statut, STATUT_HORS_LIGNE);
    }

    #[test]
    fn une_langue_de_page_de_plus_de_six_caracteres_est_refusee() {
        let mut p = page_neuve("contact", "<p>Bonjour</p>");
        p.lang = Some("français".to_string());
        assert!(valider_page(&p).is_err());

        p.lang = Some("fr_FR".to_string());
        assert_eq!(valider_page(&p).unwrap().lang.as_deref(), Some("fr_FR"));
    }

    #[test]
    fn un_texte_court_blanc_vaut_absence() {
        let mut p = page_neuve("contact", "<p>Bonjour</p>");
        p.title = Some("  ".to_string());
        p.description = Some("\n".to_string());
        p.keywords = Some(String::new());
        let validee = valider_page(&p).unwrap();
        assert_eq!(validee.titre, None);
        assert_eq!(validee.description, None);
        assert_eq!(validee.mots_cles, None);
    }
}
