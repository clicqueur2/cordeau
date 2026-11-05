//! Marges d'affaire : `llx_marge_affaire` + `llx_marge_ligne`.
//!
//! Une marge, c'est le prix de vente confronté au prix d'achat **réel** sur une
//! affaire. Ce module tient la pièce : un en-tête (libellé, tiers ou projet,
//! date, statut) et des lignes (`quantité × prix de vente HT` face à
//! `quantité × prix d'achat HT`).
//!
//! # Ce que Dolibarr 24.0.1 contient — et ce qu'il ne contient pas
//!
//! **Mesuré, pas supposé.** Le module Marges de Dolibarr 24.0.1 n'a **aucune
//! table** : `SELECT table_name FROM information_schema.tables WHERE
//! table_schema='dolibarr' AND table_name LIKE '%margin%'` ne renvoie **rien**
//! sur les 410 tables de la base de référence, et
//! `SELECT ... WHERE table_name LIKE 'llx_m%'` ne rend que `llx_mailing`,
//! `llx_menu`, `llx_mrp_*`, `llx_multicurrency*` — aucune `llx_margin_ticket`,
//! aucune `llx_margindet_ticket`.
//!
//! Il n'y a pas non plus de classe d'objet. `htdocs/margin/` ne contient que des
//! pages de **rapport** (`productMargins.php`, `customerMargins.php`,
//! `agentMargins.php`, `checkMargins.php`), un `lib/margins.lib.php` et un
//! `admin/margin.php` : pas de dossier `class/`, pas de dossier `sql/`.
//! Vérifié aussi sur les étiquettes du dépôt amont (`24.0.1`, et `19.0.0` à
//! `23.0.0` pour mémoire) : la classe `Margin` / `MarginDet` et les tables
//! `llx_margin_ticket` n'existent nulle part. Le module Dolibarr **ne stocke
//! pas** de marge : il recalcule à la volée, en SQL, la marge des lignes de
//! facture (`productMargins.php`, lignes 197 à 211 :
//! `SUM(d.total_ht - d.qty * d.buy_price_ht)`).
//!
//! ## Conséquence : ce module crée ses propres tables
//!
//! Les deux tables ci-dessous sont préfixées `llx_marge_` et ne touchent pas au
//! cœur. C'est exactement ce que `REGLE-SCHEMA.md` (décision du 4 novembre 2026)
//! autorise : « un module peut créer ses propres tables », à condition qu'elles
//! soient préfixées et visibles. Une table en plus ne rend pas la base
//! incompatible ; ce qui casse, c'est une table qui manque.
//!
//! Reste **une** ligne à écrire, que ce fichier ne peut pas écrire lui-même : le
//! préfixe doit être déclaré dans `db::PREFIXES_MODULE` (`crates/db/src/lib.rs`),
//! sinon `schema verify` rangera ces deux tables dans « tables inconnues »
//! plutôt que dans « tables de module ». Le module fonctionne quand même ; il
//! est seulement mal étiqueté.
//!
//! [`preparer()`] applique le DDL, de façon idempotente (`CREATE TABLE IF NOT
//! EXISTS`). C'est le seul endroit du dépôt qui crée une table : sans lui, le
//! module ne pourrait pas être essayé sur une base qui n'a pas reçu le script, et
//! les tests ne prouveraient rien. Le DDL est aussi exposé en clair dans
//! [`SQL_CREATION`] pour être recopié dans `sql-ready/` le jour où ce fichier
//! sera repris — il ne l'est pas ici, la consigne étant de ne toucher à aucun
//! fichier existant.
//!
//! # Les formules, et d'où elles viennent
//!
//! Le seul code de calcul de marge qui subsiste en 24.0.1 est
//! `getMarginInfos()` dans `htdocs/margin/lib/margins.lib.php`. Il dit :
//!
//! - ligne 148 : `$marge = $pu_ht_remise - $pa_ht_ret` — la marge est une
//!   **différence de prix unitaires** ;
//! - ligne 153 : `$marge_tx_ret = (100 * $marge) / $pa_ht_ret` — le **taux de
//!   marge** de Dolibarr divise par le prix d'**achat** ;
//! - ligne 157 : `$marque_tx_ret = (100 * $marge) / $pu_ht_remise` — le **taux de
//!   marque** divise par le prix de **vente**.
//!
//! Et les deux gardes, qui sont la réponse de Dolibarr à la division par zéro :
//!
//! - ligne 152 : `if ($pa_ht_ret != 0)` — sinon le taux reste la chaîne vide ;
//! - ligne 156 : `if ($pu_ht_remise != 0)` — idem.
//!
//! Cette chaîne vide est affichée **`n/a`** à l'écran (`productMargins.php`,
//! ligne 379 : `(($marginRate === '') ? 'n/a' : price(...))`). Autrement dit,
//! Dolibarr ne plante pas, n'affiche pas l'infini, et n'affiche pas 0 % : il
//! affiche « non applicable ». C'est ce comportement que reproduit
//! [`taux_marge`] en rendant `None`.
//!
//! ## La divergence assumée sur le nom « taux de marge »
//!
//! Le cahier des charges de ce module demande `taux_marge = marge / prix de
//! vente × 100`. Dans le vocabulaire de Dolibarr, **ce n'est pas le taux de
//! marge, c'est le taux de marque** (`MarkRate=Taux de marque` dans
//! `langs/fr_FR/margins.lang` ligne 10 ; `MarginRate=Taux de marge` ligne 8).
//! Les deux vocabulaires existent et ne désignent pas la même chose.
//!
//! On ne tranche pas en silence : [`taux_marge`] fait ce que demande le cahier
//! des charges (division par le prix de vente) et le dit dans sa doc ;
//! [`taux_marge_dolibarr`] fait ce que fait Dolibarr (division par le prix
//! d'achat), pour que le portage reste comparable à l'écran d'origine. Les deux
//! partagent la même garde et le même arrondi.
//!
//! # Le taux global se calcule sur les totaux, jamais comme une moyenne
//!
//! `productMargins.php` ligne 397 :
//! `$marginRate = ($cumul_achat != 0) ? (100 * $totalMargin / $cumul_achat) : ''`.
//! Dolibarr **cumule** d'abord (`$cumul_achat`, `$totalMargin`), puis divise
//! **une seule fois**. Il ne fait jamais la moyenne des taux de ligne.
//!
//! La raison est arithmétique et sans appel : une ligne à 10 € et une ligne à
//! 10 000 € ne pèsent pas pareil. Vendre 10 € un article acheté 8 € (20 % de
//! marque) et 10 000 € un article acheté 7 000 € (30 %) donne une marge de
//! 3 002 € sur 10 010 € de ventes, soit **29,99 %** — et non la moyenne des deux
//! taux, 25 %. La moyenne des taux décrit une entreprise qui n'existe pas : elle
//! donne le même poids au petit lot et au gros. C'est pour cette raison que
//! [`totaux`] rend le taux calculé sur les totaux, et que
//! [`totaux_lignes`] — sa partie pure, testable sans base — fait de même.
//!
//! Corollaire : le troisième élément de [`Totaux`] est le taux **sur le prix de
//! vente** (le « taux de marque » de Dolibarr), pour rester cohérent avec
//! [`taux_marge`]. Le taux de marge global au sens Dolibarr s'en déduit sans
//! perte : la marge vaut `vente - achat`, et les deux termes sont rendus.
//!
//! # Statuts et transitions
//!
//! `0` brouillon, `1` validée — la convention de tous les objets Dolibarr
//! (`llx_inventory.status`, `llx_contrat.statut`, `llx_resource.fk_statut`).
//!
//! Le cycle est **irréversible**, comme celui d'un inventaire : [`valider`] ne
//! fait que `0 -> 1`, et refuse tout autre départ. Une marge validée est un
//! constat — « sur cette affaire, à cette date, la marge était celle-là » ; la
//! remettre en brouillon effacerait la date et l'auteur de la validation, qui
//! sont la seule trace de qui a figé le chiffre. Une erreur de saisie se corrige
//! en supprimant le **brouillon**, jamais en remontant le statut. C'est aussi
//! pourquoi [`supprimer`] refuse une marge validée et pourquoi [`ajouter_ligne`]
//! refuse d'ajouter une ligne à une marge validée : une pièce figée qui accepte
//! encore des lignes n'est pas figée.
//!
//! # Arrondis
//!
//! Tout passe par `money::round_money` — l'équivalent de `price2num($x, 'MT')`,
//! deux décimales, demi hors de zéro comme PHP. Un seul arrondi par grandeur :
//! les totaux sont sommés **bruts** puis arrondis une fois (`money::sum`), jamais
//! la somme d'arrondis. C'est la règle du crate `money` et la raison pour
//! laquelle la somme des [`marge_ht`] de ligne peut différer d'un centime du
//! `vente - achat` de [`totaux`] : les deux chemins arrondissent à des moments
//! différents, et c'est voulu. Empiler les arrondis ferait dériver un total de
//! plusieurs centimes sur une affaire à cent lignes.
//!
//! # Cloisonnement par société
//!
//! `llx_marge_affaire` porte `entity`. `llx_marge_ligne` **n'en porte pas** :
//! c'est la convention des tables de détail de Dolibarr (`llx_facturedet`,
//! `llx_inventorydet` n'ont pas d'`entity`). Toute lecture de ligne joint donc
//! l'en-tête pour filtrer sur son `entity` ; sans cette jointure, les lignes
//! d'une autre société sortiraient avec le bon identifiant.
//!
//! # Index uniques
//!
//! - `uk_marge_affaire_ref (ref, entity)` : une référence d'affaire ne peut pas
//!   désigner deux marges dans la même société. `ref` est *nullable* et
//!   **jamais numérotée automatiquement** — c'est le numéro d'affaire du client
//!   ou du chantier, celui que le conducteur de travaux reconnaît. MySQL autorise
//!   plusieurs `NULL` dans un index unique : deux marges sans référence
//!   cohabitent, ce qui est le cas normal.
//! - `uk_marge_ligne_rang (fk_marge, rang)` : deux lignes ne peuvent pas
//!   revendiquer la même position. `rang` est attribué par [`ajouter_ligne`]
//!   (`MAX(rang) + 1`) et aucune fonction de ce module ne le réécrit — il n'y a
//!   pas de réordonnancement, donc la contrainte ne peut pas gêner.

use std::error::Error;
use std::str::FromStr;

use money::{round_money, sum};
use mysql::prelude::*;
use mysql::{params, Pool, PooledConn, Row, TxOpts};
use rust_decimal::Decimal;

/// Préfixe des deux tables de ce module. Voir `REGLE-SCHEMA.md`.
pub const PREFIXE_TABLES: &str = "llx_marge_";

/// Brouillon : la marge se modifie et se supprime.
pub const STATUT_BROUILLON: i16 = 0;

/// Validée : la marge est figée, on ne la modifie plus, on ne la supprime plus.
pub const STATUT_VALIDE: i16 = 1;

/// Longueur maximale de `llx_marge_affaire.libelle`, en octets.
pub const LIBELLE_MAX: usize = 255;

/// Longueur maximale de `llx_marge_affaire.ref`, en octets (`varchar(48)`).
pub const REF_MAX: usize = 48;

/// Longueur maximale de `llx_marge_ligne.designation`, en octets.
pub const DESIGNATION_MAX: usize = 255;

/// Le DDL des deux tables, en clair.
///
/// Exposé pour être recopié dans `sql-ready/` sans le réécrire de mémoire. Le
/// `IF NOT EXISTS` est ce qui rend [`preparer`] idempotent ; un script de
/// déploiement peut le retirer.
///
/// Les types suivent le cœur : `int(11)` pour les identifiants, `double` pour
/// les montants et les quantités (c'est ce que Dolibarr utilise partout —
/// `llx_product.price` est un `double(24,8)`, `llx_inventorydet.qty_stock` un
/// `double`), `utf8mb4` / `utf8mb4_unicode_ci` comme les 410 tables existantes.
pub const SQL_CREATION: [&str; 2] = [
    "CREATE TABLE IF NOT EXISTS llx_marge_affaire (\
       rowid int(11) NOT NULL AUTO_INCREMENT,\
       entity int(11) DEFAULT 0,\
       ref varchar(48) DEFAULT NULL,\
       libelle varchar(255) NOT NULL,\
       fk_soc int(11) DEFAULT NULL,\
       fk_projet int(11) DEFAULT NULL,\
       date_marge date DEFAULT NULL,\
       statut int(11) NOT NULL DEFAULT 0,\
       datec datetime DEFAULT NULL,\
       tms timestamp NULL DEFAULT current_timestamp() ON UPDATE current_timestamp(),\
       fk_user_author int(11) DEFAULT NULL,\
       fk_user_valid int(11) DEFAULT NULL,\
       date_valid datetime DEFAULT NULL,\
       PRIMARY KEY (rowid),\
       UNIQUE KEY uk_marge_affaire_ref (ref, entity),\
       KEY idx_marge_affaire_entity_date (entity, date_marge),\
       KEY idx_marge_affaire_fk_soc (fk_soc),\
       KEY idx_marge_affaire_fk_projet (fk_projet)\
     ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    "CREATE TABLE IF NOT EXISTS llx_marge_ligne (\
       rowid int(11) NOT NULL AUTO_INCREMENT,\
       fk_marge int(11) NOT NULL DEFAULT 0,\
       rang int(11) DEFAULT 0,\
       fk_product int(11) DEFAULT NULL,\
       designation varchar(255) NOT NULL,\
       quantite double DEFAULT 0,\
       prix_vente_ht double DEFAULT 0,\
       prix_achat_ht double DEFAULT 0,\
       datec datetime DEFAULT NULL,\
       tms timestamp NULL DEFAULT current_timestamp() ON UPDATE current_timestamp(),\
       PRIMARY KEY (rowid),\
       UNIQUE KEY uk_marge_ligne_rang (fk_marge, rang),\
       KEY idx_marge_ligne_fk_product (fk_product)\
     ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
];

/// Une marge d'affaire, telle qu'affichée en liste ou reprise en formulaire.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Marge {
    pub rowid: i64,
    pub entity: i64,
    /// Numéro d'affaire saisi, `NULL` s'il n'y en a pas. Jamais un compteur.
    pub ref_: Option<String>,
    pub libelle: String,
    /// Tiers concerné (`llx_societe.rowid`).
    pub fk_soc: Option<i64>,
    /// Projet concerné (`llx_projet.rowid`).
    pub fk_projet: Option<i64>,
    /// Date de la marge, `AAAA-MM-JJ`.
    pub date_marge: Option<String>,
    /// Voir [`STATUT_BROUILLON`] et [`STATUT_VALIDE`].
    pub statut: i16,
    /// Date de création, `AAAA-MM-JJ HH:MM:SS`.
    pub datec: Option<String>,
    pub fk_user_author: Option<i64>,
    /// Auteur de la validation. Toujours `NULL` : [`valider`] ne l'invente pas
    /// (voir sa doc). La colonne existe pour l'écran qui, lui, connaît
    /// l'utilisateur connecté.
    pub fk_user_valid: Option<i64>,
    /// Date de validation, `AAAA-MM-JJ HH:MM:SS`, `NULL` en brouillon.
    pub date_valid: Option<String>,
}

/// Une ligne de marge : un produit, une quantité, deux prix HT.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LigneMarge {
    pub rowid: i64,
    pub fk_marge: i64,
    /// Position dans l'affaire, attribuée à l'ajout. Voir l'index
    /// `uk_marge_ligne_rang` en tête de module.
    pub rang: i64,
    /// Produit du catalogue, `NULL` pour une désignation libre.
    pub fk_product: Option<i64>,
    /// Désignation affichée : le libellé du produit s'il est connu, sinon la
    /// saisie. Toujours renseignée, même quand `fk_product` l'est aussi — la
    /// ligne doit rester lisible si le produit est renommé ou supprimé.
    pub designation: String,
    pub quantite: Decimal,
    /// Prix de vente unitaire hors taxe.
    pub prix_vente_ht: Decimal,
    /// Prix d'achat unitaire hors taxe — le coût **réel**, pas le tarif fournisseur.
    pub prix_achat_ht: Decimal,
}

/// (vente HT, achat HT, taux global).
///
/// Un alias plutôt qu'un `(Decimal, Decimal, Option<Decimal>)` répété dans trois
/// signatures : au point d'appel, on ne devine plus lequel des deux montants
/// vient en premier.
pub type Totaux = (Decimal, Decimal, Option<Decimal>);

/// Ce qu'il faut pour créer une marge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NouvelleMarge {
    /// Libellé de l'affaire, obligatoire, [`LIBELLE_MAX`] octets au plus.
    pub libelle: String,
    /// Numéro d'affaire, facultatif. Voir `uk_marge_affaire_ref` en tête de module.
    pub ref_: Option<String>,
    /// Tiers concerné. **L'un des deux au moins** (`fk_soc`, `fk_projet`) doit
    /// être renseigné : une marge ne se rattache pas à rien.
    pub fk_soc: Option<i64>,
    /// Projet concerné. Voir `fk_soc`.
    pub fk_projet: Option<i64>,
    /// `AAAA-MM-JJ`. `None` laisse la base poser `CURDATE()`.
    pub date_marge: Option<String>,
    /// Auteur de la saisie. Doit être strictement positif.
    ///
    /// Aucune clé étrangère ne lie cette colonne à `llx_user` (le schéma de
    /// Dolibarr n'en pose pas) : on vérifie la positivité, pas l'existence —
    /// c'est au formulaire appelant de proposer des utilisateurs réels.
    pub fk_user_author: i64,
}

/// Ce qu'il faut pour ajouter une ligne.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NouvelleLigneMarge {
    /// Produit du catalogue, facultatif. S'il est fourni et que `designation`
    /// est vide, le libellé du produit est repris depuis `llx_product`.
    pub fk_product: Option<i64>,
    /// Désignation libre. Obligatoire si `fk_product` est absent.
    pub designation: String,
    /// Quantité, strictement positive (voir [`ajouter_ligne`]).
    pub quantite: Decimal,
    /// Prix de vente unitaire HT, positif ou nul.
    pub prix_vente_ht: Decimal,
    /// Prix d'achat unitaire HT, positif ou nul.
    pub prix_achat_ht: Decimal,
}

const SELECT_MARGE: &str = "a.rowid, a.entity, a.ref, a.libelle, a.fk_soc, a.fk_projet, \
     DATE_FORMAT(a.date_marge, '%Y-%m-%d'), a.statut, \
     DATE_FORMAT(a.datec, '%Y-%m-%d %H:%i:%s'), a.fk_user_author, \
     a.fk_user_valid, DATE_FORMAT(a.date_valid, '%Y-%m-%d %H:%i:%s')";

const SELECT_LIGNE: &str = "l.rowid, l.fk_marge, l.rang, l.fk_product, l.designation, \
     CAST(l.quantite AS CHAR), CAST(l.prix_vente_ht AS CHAR), CAST(l.prix_achat_ht AS CHAR)";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_marge(row: &Row) -> Marge {
    Marge {
        rowid: opt_i(row, 0).unwrap_or(0),
        entity: opt_i(row, 1).unwrap_or(1),
        ref_: opt_s(row, 2),
        libelle: opt_s(row, 3).unwrap_or_default(),
        fk_soc: opt_i(row, 4),
        fk_projet: opt_i(row, 5),
        date_marge: opt_s(row, 6),
        statut: opt_i(row, 7).unwrap_or(0) as i16,
        datec: opt_s(row, 8),
        fk_user_author: opt_i(row, 9),
        fk_user_valid: opt_i(row, 10),
        date_valid: opt_s(row, 11),
    }
}

fn map_ligne(row: &Row) -> LigneMarge {
    LigneMarge {
        rowid: opt_i(row, 0).unwrap_or(0),
        fk_marge: opt_i(row, 1).unwrap_or(0),
        rang: opt_i(row, 2).unwrap_or(0),
        fk_product: opt_i(row, 3),
        designation: opt_s(row, 4).unwrap_or_default(),
        quantite: opt_dec(row, 5).unwrap_or_default(),
        prix_vente_ht: opt_dec(row, 6).unwrap_or_default(),
        prix_achat_ht: opt_dec(row, 7).unwrap_or_default(),
    }
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

/// Contrôle de forme d'une date `AAAA-MM-JJ`.
///
/// Volontairement faible : on vérifie la forme, pas le calendrier (le 31 février
/// passerait). Le but n'est pas de refaire le calendrier mais d'éviter qu'une
/// saisie « demain » ou « 05/11/2026 » arrive jusqu'à MySQL, où l'erreur serait
/// illisible (« Incorrect date value ») au lieu de nommer le champ fautif.
fn verifier_date(valeur: Option<&str>) -> Result<(), Box<dyn Error>> {
    let Some(d) = valeur else {
        return Ok(());
    };
    let octets = d.as_bytes();
    let forme = octets.len() == 10
        && octets[4] == b'-'
        && octets[7] == b'-'
        && octets
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if forme {
        Ok(())
    } else {
        Err(format!("la date « {d} » n'est pas au format AAAA-MM-JJ (exemple : 2026-11-05)").into())
    }
}

/// Applique le DDL de [`SQL_CREATION`], sans erreur si les tables existent déjà.
///
/// C'est l'étape d'installation du module. Idempotente par construction
/// (`CREATE TABLE IF NOT EXISTS`) : l'appeler à chaque démarrage est sans effet
/// sur une base déjà à jour, et ne touche à aucune table du cœur. Voir la doc du
/// module pour la raison d'être de cette fonction et la ligne à ajouter dans
/// `db::PREFIXES_MODULE`.
pub fn preparer(pool: &Pool) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    for ddl in SQL_CREATION {
        conn.query_drop(ddl)?;
    }
    Ok(())
}

/// Marges de l'entité, de la plus récente à la plus ancienne.
///
/// L'ordre est celui d'un historique : on cherche la marge du mois dernier, pas
/// la première saisie. `date_marge` d'abord (c'est la date de l'affaire),
/// `rowid` ensuite pour départager deux marges du même jour.
pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Marge>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_MARGE} FROM llx_marge_affaire a \
         WHERE a.entity = :entity \
         ORDER BY a.date_marge DESC, a.rowid DESC LIMIT :limit OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limit" => limit, "offset" => offset },
    )?;
    Ok(rows.iter().map(map_marge).collect())
}

/// Une marge précise, ou `None` si elle n'existe pas dans cette société.
pub fn get(pool: &Pool, entity: i64, fk_marge: i64) -> Result<Option<Marge>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_MARGE} FROM llx_marge_affaire a \
         WHERE a.entity = :entity AND a.rowid = :rowid"
    );
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => fk_marge })?;
    Ok(row.as_ref().map(map_marge))
}

fn marge_existe(
    conn: &mut PooledConn,
    entity: i64,
    fk_marge: i64,
) -> Result<Option<i16>, Box<dyn Error>> {
    let statut: Option<i64> = conn.exec_first(
        "SELECT statut FROM llx_marge_affaire WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_marge },
    )?;
    Ok(statut.map(|s| s as i16))
}

/// Crée une marge en brouillon et rend son `rowid`.
///
/// Refuse, **avant toute écriture** : un libellé vide ou de plus de
/// [`LIBELLE_MAX`] octets, une référence de plus de [`REF_MAX`] octets, ni tiers
/// ni projet, une date qui n'a pas la forme `AAAA-MM-JJ`, un auteur non
/// strictement positif, et un tiers ou un projet négatif ou nul.
///
/// `statut` vaut [`STATUT_BROUILLON`] et `datec` `NOW()`. `fk_user_valid` et
/// `date_valid` restent `NULL` : créer n'est pas valider, et les inventer ferait
/// croire à une validation qui n'a pas eu lieu.
pub fn creer(pool: &Pool, entity: i64, new: &NouvelleMarge) -> Result<i64, Box<dyn Error>> {
    let libelle = new.libelle.trim();
    if libelle.is_empty() {
        return Err("le libellé de la marge est obligatoire".into());
    }
    if libelle.len() > LIBELLE_MAX {
        return Err(format!(
            "le libellé de la marge dépasse {LIBELLE_MAX} octets ({} reçus)",
            libelle.len()
        )
        .into());
    }
    let reference = texte(new.ref_.as_deref());
    if let Some(r) = reference.filter(|r| r.len() > REF_MAX) {
        return Err(format!(
            "la référence « {r} » dépasse {REF_MAX} octets ({} reçus)",
            r.len()
        )
        .into());
    }
    if new.fk_soc.is_none() && new.fk_projet.is_none() {
        return Err(
            "une marge doit se rattacher à un tiers (fk_soc) ou à un projet \
                    (fk_projet) : sans l'un des deux, elle ne mesure rien"
                .into(),
        );
    }
    for (nom, valeur) in [("fk_soc", new.fk_soc), ("fk_projet", new.fk_projet)] {
        if let Some(v) = valeur.filter(|v| *v <= 0) {
            return Err(format!("l'identifiant {nom} ({v}) est invalide").into());
        }
    }
    if new.fk_user_author <= 0 {
        return Err(format!(
            "l'auteur de la marge (fk_user_author = {}) est invalide",
            new.fk_user_author
        )
        .into());
    }
    let date_marge = texte(new.date_marge.as_deref());
    verifier_date(date_marge)?;

    let mut conn = pool.get_conn()?;
    let insere = conn.exec_drop(
        "INSERT INTO llx_marge_affaire \
         (entity, ref, libelle, fk_soc, fk_projet, date_marge, statut, datec, fk_user_author) \
         VALUES \
         (:entity, :reference, :libelle, :soc, :projet, COALESCE(:date_marge, CURDATE()), \
          :statut, NOW(), :auteur)",
        params! {
            "entity" => entity,
            "reference" => reference,
            "libelle" => libelle,
            "soc" => new.fk_soc,
            "projet" => new.fk_projet,
            "date_marge" => date_marge,
            "statut" => STATUT_BROUILLON,
            "auteur" => new.fk_user_author,
        },
    );

    match insere {
        Ok(()) => Ok(conn.last_insert_id() as i64),
        Err(e) if is_duplicate(&e) => Err(format!(
            "une marge porte déjà la référence « {} » dans l'entité {entity}",
            reference.unwrap_or_default()
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

/// Ajoute une ligne à une marge en brouillon et rend son `rowid`.
///
/// Refuse : une marge absente de l'entité, une marge qui n'est plus en brouillon
/// (voir la doc du module sur l'irréversibilité), une quantité **nulle ou
/// négative**, un prix de vente négatif, un prix d'achat négatif, une désignation
/// vide quand aucun produit n'est fourni, et une désignation trop longue.
///
/// La quantité nulle est refusée alors qu'un prix de vente nul est accepté : les
/// deux cas ne veulent pas dire la même chose. Une ligne à quantité zéro n'est
/// pas une vente, c'est une ligne oubliée dans un formulaire — elle
/// n'ajouterait rien au total et fausserait le compte des lignes. Une ligne à
/// prix de vente zéro, elle, existe : offert, geste commercial, échantillon. On
/// la garde, et c'est [`taux_marge`] qui rend `None` faute de base de calcul.
///
/// Si `fk_product` est fourni sans désignation, le libellé du produit est
/// recopié depuis `llx_product.label`. La copie est voulue : une ligne de marge
/// est une pièce, elle doit rester lisible même si le produit est renommé ou
/// retiré du catalogue. Si le produit n'existe pas dans l'entité, la ligne est
/// refusée.
pub fn ajouter_ligne(
    pool: &Pool,
    entity: i64,
    fk_marge: i64,
    new: &NouvelleLigneMarge,
) -> Result<i64, Box<dyn Error>> {
    if new.quantite.is_zero() {
        return Err("la quantité d'une ligne de marge ne peut pas être nulle : \
                    une ligne à zéro n'est pas une vente"
            .into());
    }
    if new.quantite < Decimal::ZERO {
        return Err(format!(
            "la quantité d'une ligne de marge ne peut pas être négative ({}) : \
             une remise se saisit sur le prix, pas sur la quantité",
            new.quantite
        )
        .into());
    }
    if new.prix_vente_ht < Decimal::ZERO {
        return Err(format!(
            "le prix de vente d'une ligne de marge ne peut pas être négatif ({})",
            new.prix_vente_ht
        )
        .into());
    }
    if new.prix_achat_ht < Decimal::ZERO {
        return Err(format!(
            "le prix d'achat d'une ligne de marge ne peut pas être négatif ({})",
            new.prix_achat_ht
        )
        .into());
    }

    let mut conn = pool.get_conn()?;
    match marge_existe(&mut conn, entity, fk_marge)? {
        None => {
            return Err(format!("la marge {fk_marge} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s != STATUT_BROUILLON => {
            return Err("une marge validée ne reçoit plus de ligne : elle est figée".into())
        }
        _ => {}
    }

    let saisie = new.designation.trim();
    let designation = if saisie.is_empty() {
        let Some(produit) = new.fk_product else {
            return Err("une ligne de marge sans produit doit porter une désignation".into());
        };
        let label: Option<String> = conn.exec_first(
            "SELECT label FROM llx_product WHERE entity = :entity AND rowid = :produit",
            params! { "entity" => entity, "produit" => produit },
        )?;
        match label {
            None => {
                return Err(format!(
                    "le produit {produit} n'existe pas dans l'entité {entity} : \
                     impossible d'en reprendre la désignation"
                )
                .into())
            }
            Some(l) if l.trim().is_empty() => {
                return Err(format!(
                    "le produit {produit} n'a pas de libellé : donnez une désignation"
                )
                .into())
            }
            Some(l) => l,
        }
    } else {
        saisie.to_string()
    };
    if designation.len() > DESIGNATION_MAX {
        return Err(format!(
            "la désignation dépasse {DESIGNATION_MAX} octets ({} reçus)",
            designation.len()
        )
        .into());
    }

    // `rang` est attribué puis inséré sous la protection de
    // `uk_marge_ligne_rang` : deux saisies simultanées peuvent lire le même
    // `MAX(rang)`, la seconde est alors refusée par la base (1062) et retentée.
    for _ in 0..5u32 {
        let rang: i64 = conn
            .exec_first(
                "SELECT COALESCE(MAX(rang), 0) + 1 FROM llx_marge_ligne WHERE fk_marge = :marge",
                params! { "marge" => fk_marge },
            )?
            .unwrap_or(1);
        let insere = conn.exec_drop(
            "INSERT INTO llx_marge_ligne \
             (fk_marge, rang, fk_product, designation, quantite, prix_vente_ht, \
              prix_achat_ht, datec) \
             VALUES (:marge, :rang, :produit, :designation, :qte, :pv, :pa, NOW())",
            params! {
                "marge" => fk_marge,
                "rang" => rang,
                "produit" => new.fk_product,
                "designation" => designation.as_str(),
                "qte" => new.quantite.to_string(),
                "pv" => new.prix_vente_ht.to_string(),
                "pa" => new.prix_achat_ht.to_string(),
            },
        );
        match insere {
            Ok(()) => return Ok(conn.last_insert_id() as i64),
            Err(e) if is_duplicate(&e) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer un rang unique à la ligne de marge".into())
}

/// Les lignes d'une marge, dans l'ordre de saisie (`rang`).
///
/// La lecture joint `llx_marge_affaire` : c'est cette jointure qui applique le
/// cloisonnement, `llx_marge_ligne` n'ayant pas de colonne `entity`. Une marge
/// absente de l'entité est une **erreur**, pas une liste vide : rendre `[]`
/// laisserait croire à une affaire sans ligne.
pub fn lignes(pool: &Pool, entity: i64, fk_marge: i64) -> Result<Vec<LigneMarge>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    if marge_existe(&mut conn, entity, fk_marge)?.is_none() {
        return Err(format!("la marge {fk_marge} n'existe pas dans l'entité {entity}").into());
    }
    lignes_de(&mut conn, entity, fk_marge)
}

fn lignes_de(
    conn: &mut PooledConn,
    entity: i64,
    fk_marge: i64,
) -> Result<Vec<LigneMarge>, Box<dyn Error>> {
    let sql = format!(
        "SELECT {SELECT_LIGNE} FROM llx_marge_ligne l \
         INNER JOIN llx_marge_affaire a ON a.rowid = l.fk_marge \
         WHERE a.entity = :entity AND l.fk_marge = :marge \
         ORDER BY l.rang ASC, l.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "marge" => fk_marge })?;
    Ok(rows.iter().map(map_ligne).collect())
}

/// Passe la marge de brouillon à validée (`0 -> 1`) et date la validation.
///
/// Refuse si la marge est introuvable dans l'entité, si elle est **déjà**
/// validée, si elle porte un statut inconnu, ou si elle n'a **aucune ligne** : une
/// marge sans ligne ne mesure rien, et la valider figerait un chiffre vide qu'on
/// ne pourrait plus corriger.
///
/// Aucun retour en arrière : voir la doc du module.
///
/// `fk_user_valid` reste `NULL`. La tentation est d'y recopier `fk_user_author`,
/// mais ce serait affirmer que l'auteur de la saisie est celui de la validation
/// — or dans un atelier à deux, c'est souvent faux, et c'est précisément la
/// question qu'on se pose six mois plus tard. Le dépôt tranche déjà ainsi :
/// `inventaire::valider` et `contrat::valider` posent la date et le statut, pas
/// l'utilisateur, faute d'un paramètre qui le porte. La date, elle, est un fait
/// qu'on connaît : `date_valid = NOW()`.
pub fn valider(pool: &Pool, entity: i64, fk_marge: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    match marge_existe(&mut conn, entity, fk_marge)? {
        None => {
            return Err(format!("la marge {fk_marge} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s == STATUT_VALIDE => {
            return Err("cette marge est déjà validée : elle ne se revalide pas".into())
        }
        Some(s) if s != STATUT_BROUILLON => {
            return Err(format!(
                "cette marge a un statut inattendu ({s}) : seul un brouillon se valide"
            )
            .into())
        }
        _ => {}
    }

    let nb: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_marge_ligne WHERE fk_marge = :marge",
        params! { "marge" => fk_marge },
    )?;
    if nb.unwrap_or(0) == 0 {
        return Err("une marge sans ligne ne se valide pas : elle ne mesure rien".into());
    }

    conn.exec_drop(
        "UPDATE llx_marge_affaire \
         SET statut = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDE, "entity" => entity, "rowid" => fk_marge },
    )?;
    Ok(())
}

/// Supprime un brouillon et ses lignes.
///
/// Une marge validée ne se supprime pas : elle atteste qu'à une date, sur une
/// affaire, la marge était celle-là. C'est souvent la seule trace d'un chiffre
/// contesté six mois plus tard — le supprimer effacerait la preuve. La
/// suppression physique est donc réservée au brouillon, et les lignes partent
/// avec l'en-tête dans la même transaction : le schéma Dolibarr ne pose pas de
/// clé étrangère, un reste de ligne ne serait jamais signalé et fausserait le
/// prochain calcul.
pub fn supprimer(pool: &Pool, entity: i64, fk_marge: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    match marge_existe(&mut conn, entity, fk_marge)? {
        None => {
            return Err(format!("la marge {fk_marge} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s != STATUT_BROUILLON => {
            return Err("une marge validée ne se supprime pas : elle atteste d'un constat".into())
        }
        _ => {}
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    tx.exec_drop(
        "DELETE FROM llx_marge_ligne WHERE fk_marge = :marge",
        params! { "marge" => fk_marge },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_marge_affaire WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_marge },
    )?;
    tx.commit()?;
    Ok(())
}

/// Marge d'une ligne : `(prix de vente − prix d'achat) × quantité`, arrondie au
/// centime.
///
/// C'est la marge **de la ligne**, pas la marge unitaire : Dolibarr calcule la
/// marge unitaire (`margins.lib.php` ligne 148) et la multiplie par la quantité
/// dans son SQL (`productMargins.php` ligne 205,
/// `d.total_ht - (d.buy_price_ht * d.qty)`). On fait le produit d'abord et on
/// arrondit **une fois**.
///
/// Écart connu avec Dolibarr, d'un centime au plus : Dolibarr part de
/// `d.total_ht`, déjà arrondi à deux décimales, alors qu'on part des prix
/// unitaires bruts. Sur `1,5 × 19,99`, Dolibarr a `29,99` en poche et nous
/// `29,985` ; après arrondi les deux donnent `29,99`. L'écart n'apparaît que sur
/// des quantités élevées, et il est préférable de ne pas empiler d'arrondi.
pub fn marge_ht(ligne: &LigneMarge) -> Decimal {
    round_money((ligne.prix_vente_ht - ligne.prix_achat_ht) * ligne.quantite)
}

/// La marge d'**une** unite : le prix de vente moins le prix d'achat, sans
/// quantite.
///
/// C'est la grandeur sur laquelle se calculent les deux taux. Diviser une marge
/// totale par un prix unitaire donnerait un pourcentage qui double avec la
/// quantite — un taux de 133 % au lieu de 66 % sur deux pieces.
pub fn marge_unitaire(ligne: &LigneMarge) -> Decimal {
    round_money(ligne.prix_vente_ht - ligne.prix_achat_ht)
}

/// Taux de marge au sens du cahier des charges : `marge / prix de vente × 100`.
///
/// **Ce n'est pas le « taux de marge » de Dolibarr**, qui divise par le prix
/// d'achat (voir [`taux_marge_dolibarr`] et la doc du module). C'est le **taux de
/// marque** du vocabulaire Dolibarr (`MarkRate=Taux de marque`).
///
/// Rend `None` quand le prix de vente est nul, **et c'est un résultat, pas une
/// panne** : ni `0`, ni l'infini, ni un plantage. Dolibarr fait exactement ce
/// choix (garde `if ($pu_ht_remise != 0)`, `margins.lib.php` ligne 156) et
/// affiche `n/a` à l'écran (`productMargins.php` ligne 379). Rendre `0`
/// prétendrait que la marge est nulle alors qu'elle peut être très négative
/// (un article offert acheté 100 € a une marge de −100 €, soit un taux
/// indéfini, pas 0 %) ; rendre l'infini ferait déborder la colonne `Decimal` de
/// l'affichage. `None` dit la seule chose vraie : la base de calcul manque.
pub fn taux_marge(ligne: &LigneMarge) -> Option<Decimal> {
    if ligne.prix_vente_ht.is_zero() {
        return None;
    }
    Some(taux_sur(ligne.prix_vente_ht, marge_unitaire(ligne)))
}

/// Taux de marge au sens de Dolibarr : `marge / prix d'achat × 100`.
///
/// Fidèle à `margins.lib.php` ligne 153 (`(100 * $marge) / $pa_ht_ret`) et à
/// `productMargins.php` ligne 330 (`$marginRate = (100 * $marge / $pa)`). Rend
/// `None` quand le prix d'achat est nul, comme les gardes lignes 152 et 336.
///
/// Existe pour que le portage reste comparable à l'écran d'origine : les deux
/// taux ne diffèrent que par leur dénominateur, et confondre les deux fait
/// afficher 20 % là où Dolibarr affiche 25 %.
pub fn taux_marge_dolibarr(ligne: &LigneMarge) -> Option<Decimal> {
    if ligne.prix_achat_ht.is_zero() {
        return None;
    }
    Some(taux_sur(ligne.prix_achat_ht, marge_unitaire(ligne)))
}

/// `valeur / base × 100`, arrondi au centime — `price2num($x, 'MT')`.
///
/// L'appelant garantit `base` non nulle : c'est le seul contrat de cette
/// fonction privée, et c'est pour cela qu'elle n'est pas publique.
fn taux_sur(base: Decimal, valeur: Decimal) -> Decimal {
    round_money(valeur * Decimal::from(100) / base)
}

/// Totaux d'une liste de lignes : (vente HT, achat HT, taux global).
///
/// Partie pure de [`totaux`], pour être vérifiable sans base. Le taux global est
/// `(vente - achat) / vente × 100` — calculé sur les **totaux**, jamais comme la
/// moyenne des taux de ligne (voir la démonstration en tête de module) — et vaut
/// `None` si la vente totale est nulle, pour la même raison que [`taux_marge`].
///
/// Chaque colonne est sommée **brute** puis arrondie une seule fois
/// (`money::sum`). La somme des [`marge_ht`] de ligne peut donc différer d'un
/// centime de `vente - achat` : deux chemins d'arrondi, un seul est exact.
pub fn totaux_lignes(lignes: &[LigneMarge]) -> Totaux {
    let ventes: Vec<Decimal> = lignes
        .iter()
        .map(|l| l.prix_vente_ht * l.quantite)
        .collect();
    let achats: Vec<Decimal> = lignes
        .iter()
        .map(|l| l.prix_achat_ht * l.quantite)
        .collect();
    let vente = sum(&ventes);
    let achat = sum(&achats);
    let taux = if vente.is_zero() {
        None
    } else {
        Some(taux_sur(vente, vente - achat))
    };
    (vente, achat, taux)
}

/// Totaux d'une marge lue en base : (vente HT, achat HT, taux global).
///
/// Refuse une marge absente de l'entité plutôt que de rendre des zéros : un
/// total nul sur une affaire inconnue se confondrait avec une affaire dont tout
/// a été offert.
pub fn totaux(pool: &Pool, entity: i64, fk_marge: i64) -> Result<Totaux, Box<dyn Error>> {
    let lignes = lignes(pool, entity, fk_marge)?;
    Ok(totaux_lignes(&lignes))
}

#[cfg(test)]
mod tests {
    use super::*;
    // `Decimal::from_str` est une méthode de trait : le trait doit être en portée
    // au point d'appel, d'où cette importation en plus du glob.
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).expect("décimal invalide")
    }

    fn ligne(qte: &str, pv: &str, pa: &str) -> LigneMarge {
        LigneMarge {
            rowid: 1,
            fk_marge: 1,
            rang: 1,
            fk_product: None,
            designation: "Ligne de test".to_string(),
            quantite: dec(qte),
            prix_vente_ht: dec(pv),
            prix_achat_ht: dec(pa),
        }
    }

    /// Un pourcentage ne doit pas dependre du nombre de pieces.
    ///
    /// Ce test existe parce que le calcul etait faux : la marge **totale** etait
    /// divisee par un prix **unitaire**, si bien qu'une ligne de deux pieces
    /// annoncait 133 % de taux de marge au lieu de 66 %. Les tests a quantite 1
    /// ne pouvaient pas le voir.
    #[test]
    fn le_taux_ne_depend_pas_de_la_quantite() {
        // `ligne(quantite, prix_vente, prix_achat)` : 400 de marge unitaire.
        let une = ligne("1", "1000", "600");
        let deux = ligne("2", "1000", "600");
        let dix = ligne("10", "1000", "600");

        // La marge, elle, suit la quantite : c'est normal.
        assert_eq!(marge_ht(&deux), Decimal::from(800));

        // Le taux, non.
        assert_eq!(taux_marge(&une), taux_marge(&deux));
        assert_eq!(taux_marge(&deux), taux_marge(&dix));
        assert_eq!(taux_marge_dolibarr(&une), taux_marge_dolibarr(&dix));

        // Les deux valeurs attendues, posees a la main : 400 de marge sur
        // 1 000 de vente font 40 % de marque ; sur 600 d'achat, 66,67 %.
        assert_eq!(taux_marge(&deux), Some(Decimal::from(40)));
        assert_eq!(taux_marge_dolibarr(&deux), Some(dec("66.67")));
    }

    #[test]
    fn marge_d_une_ligne() {
        // 10 unités vendues 100 €, achetées 60 € : 40 € de marge unitaire,
        // 400 € sur la ligne. La quantité multiplie la marge, elle ne la dilue pas.
        assert_eq!(marge_ht(&ligne("10", "100", "60")), dec("400.00"));
        // Quantité décimale : 7,5 × 33,33 − 7,5 × 20,00 = 249,975 − 150 = 99,975
        // -> 99,98 (demi hors de zéro, comme PHP).
        assert_eq!(marge_ht(&ligne("7.5", "33.33", "20.00")), dec("99.98"));
    }

    #[test]
    fn marge_negative_quand_l_achat_depasse_la_vente() {
        // Vendre 90 ce qu'on a payé 100 est une perte : −10, et le signe doit
        // survivre à l'arrondi (round_money est symétrique, demi hors de zéro).
        assert_eq!(marge_ht(&ligne("3", "90", "100")), dec("-30.00"));
    }

    #[test]
    fn taux_de_marge_sur_le_prix_de_vente() {
        // (100 − 80) / 100 = 20 %. Le taux de marge Dolibarr (sur l'achat)
        // donnerait 25 % : c'est bien le taux de marque qui est rendu ici.
        let l = ligne("1", "100", "80");
        assert_eq!(taux_marge(&l), Some(dec("20.00")));
        assert_eq!(taux_marge_dolibarr(&l), Some(dec("25.00")));
    }

    #[test]
    fn taux_nul_quand_le_prix_de_vente_est_zero() {
        // Article offert, acheté 100 : la marge vaut −100, le taux n'a pas de
        // base. `None`, pas `0` et pas une division par zéro.
        let l = ligne("1", "0", "100");
        assert_eq!(marge_ht(&l), dec("-100.00"));
        assert_eq!(taux_marge(&l), None);
        // Le taux Dolibarr, lui, existe : le prix d'achat n'est pas nul.
        assert_eq!(taux_marge_dolibarr(&l), Some(dec("-100.00")));
    }

    #[test]
    fn taux_dolibarr_nul_quand_le_prix_d_achat_est_zero() {
        // Cadeau d'un fournisseur, revendu 50 : marge 50, taux de marque 100 %,
        // taux de marge indéfini (division par un achat nul).
        let l = ligne("1", "50", "0");
        assert_eq!(taux_marge(&l), Some(dec("100.00")));
        assert_eq!(taux_marge_dolibarr(&l), None);
    }

    #[test]
    fn taux_global_pondere_deux_poids_tres_differents() {
        // 10 € de vente à 20 % de marque, 10 000 € à 30 % : marge 3 002 sur
        // 10 010 de ventes = 29,99 %. Le petit lot ne pèse presque rien.
        let petit = ligne("1", "10", "8");
        let gros = ligne("1", "10000", "7000");
        let (vente, achat, taux) = totaux_lignes(&[petit, gros]);
        assert_eq!(vente, dec("10010.00"));
        assert_eq!(achat, dec("7008.00"));
        assert_eq!(taux, Some(dec("29.99")));
    }

    #[test]
    fn la_moyenne_des_taux_n_est_pas_le_taux_global() {
        let petit = ligne("1", "10", "8");
        let gros = ligne("1", "10000", "7000");
        let moyenne = (taux_marge(&petit).unwrap() + taux_marge(&gros).unwrap()) / dec("2");
        let (_, _, global) = totaux_lignes(&[petit, gros]);
        assert_eq!(moyenne, dec("25.00"));
        assert_eq!(global, Some(dec("29.99")));
        assert_ne!(
            global,
            Some(moyenne),
            "moyenner les taux donnerait un chiffre qu'aucune des deux affaires ne produit"
        );
    }

    #[test]
    fn totaux_d_une_liste_vide() {
        // Aucune ligne : vente et achat à zéro, et pas de taux — on ne divise pas
        // par une vente nulle, même quand elle vaut zéro par absence.
        assert_eq!(totaux_lignes(&[]), (dec("0"), dec("0"), None));
    }
}
