//! Nomenclatures (BOM) : de quoi un ouvrage est fait.
//!
//! Une nomenclature répond à la seule question qui compte avant de chiffrer :
//! **« 1 m² de mur en pierre, c'est combien de pierres, de sable, de ciment et
//! d'heures ? »** Le produit fini est un article (`llx_product`), les composants
//! sont d'autres articles avec leur quantité, et l'ensemble porte une quantité
//! produite (`qty`) : les quantités de composants sont lues **pour cette
//! quantité-là**, pas pour une unité abstraite.
//!
//! # Ce que la base contient réellement (mesuré, pas supposé)
//!
//! Interrogation de `information_schema` sur la base `dolibarr` locale :
//!
//! - `llx_bom_bom` : `rowid, entity, ref, bomtype, label, fk_product,
//!   description, note_public, note_private, fk_warehouse, qty, efficiency,
//!   duration, date_creation, date_valid, tms, fk_user_creat, fk_user_modif,
//!   fk_user_valid, import_key, model_pdf, last_main_doc, status` ;
//!   clés `PRIMARY(rowid)` et `uk_bom_bom_ref(ref, entity)` — la référence est
//!   unique **par entité**, pas globalement ;
//! - `llx_bom_bomline` : `rowid, fk_bom, fk_product, fk_bom_child, tms,
//!   description, import_key, qty, qty_frozen, disable_stock_change,
//!   efficiency, fk_unit, position, fk_default_workstation` ; aucun index
//!   unique, seulement `idx_bom_bomline_fk_bom` et `idx_bom_bomline_fk_product`
//!   — deux lignes identiques sont donc permises par le schéma, c'est au code
//!   de ne pas en produire.
//!
//! Contraintes réelles, relevées dans `information_schema.key_column_usage` :
//! `llx_bom_bom.fk_user_creat` → `llx_user.rowid` et
//! `llx_bom_bomline.fk_bom` → `llx_bom_bom.rowid`. **`fk_product` n'en a
//! aucune** : la base accepte une ligne qui pointe vers un produit qui n'existe
//! pas, et une nomenclature s'afficherait alors avec un composant fantôme. Les
//! contrôles d'existence de ce module ne sont donc pas une politesse, c'est le
//! seul garde-fou ; et `fk_user_creat` est vérifié aussi, sinon l'insertion
//! échoue sur une erreur 1452 illisible au lieu de dire quel utilisateur manque.
//!
//! Les quantités sont des `double(24,8)`. On les lit avec `CAST(... AS CHAR)`
//! et on calcule en `Decimal` : jamais de flottant dans un coût.
//!
//! La base de référence ne contient **aucune** nomenclature (`SELECT COUNT(*)
//! FROM llx_bom_bom` = 0) ni aucun prix fournisseur
//! (`llx_product_fournisseur_price` = 0 ligne) : rien n'a donc pu être observé
//! sur des données réelles, tout vient du code source cité ci-dessous.
//!
//! # Les statuts, cités de `htdocs/bom/class/bom.class.php`
//!
//! ```text
//! const STATUS_DRAFT     = 0;
//! const STATUS_VALIDATED = 1;
//! const STATUS_CANCELED  = 9;
//! ```
//!
//! et leur libellé dans `LibStatut()` : `0` → *Draft*, `1` → *Enabled*,
//! `9` → *Disabled*. Le cycle est **irréversible dans ce module** :
//! `0 → 1 → 9`. Dolibarr a bien un `reopen()` (`9 → 1`) et un `setDraft()`
//! (`1 → 0`), mais aucun des deux n'est exposé ici : une nomenclature validée a
//! servi à chiffrer un devis, la rouvrir silencieusement changerait des prix
//! déjà envoyés. Une correction se fait en créant une nouvelle version.
//!
//! `bomtype` vaut `0` (*Manufacturing*) ou `1` (*Disassemble*). Un maçon
//! fabrique : ce module écrit `0` et n'expose pas le démontage.
//!
//! # Le coût de matière, et pourquoi il échoue au lieu d'additionner
//!
//! `BOM::calculateCosts()` (`bom.class.php`, § « BOM costs calculation based on
//! cost_price or pmp of each BOM line ») applique cette cascade :
//!
//! ```text
//! $unit_cost = is_null($tmpproduct->cost_price) ? $tmpproduct->pmp : $tmpproduct->cost_price;
//! if (empty($unit_cost)) {                       // donc 0 ou NULL
//!     if ($productFournisseur->find_min_price_product_fournisseur($line->fk_product) > 0) {
//!         $line->unit_cost = ($fourn_remise_percent != "0")
//!             ? $fourn_unitprice_with_discount
//!             : $fourn_unitprice;
//!     } else {
//!         $line->unit_cost = 0;                  // <-- ici Dolibarr met zéro
//!     }
//! }
//! $line->total_cost = $line->unit_cost * $line->qty / $line->efficiency;
//! ```
//!
//! `cout_matiere` reproduit la cascade (`cost_price`, puis `pmp` quand
//! `cost_price` est NULL, puis le prix fournisseur le moins cher) **sauf la
//! dernière branche** : là où Dolibarr écrit `0`, on renvoie une erreur qui
//! nomme le produit. Un composant à 0 € n'est pas gratuit, c'est un prix
//! inconnu ; l'additionner donnerait un coût de revient **faux**, et un devis
//! bâti dessus serait faux avec l'autorité d'un chiffre. Mieux vaut ne pas
//! savoir que savoir mal.
//!
//! Le prix fournisseur suit `find_min_price_product_fournisseur()`, qui prend
//! le **minimum** de `price / quantity` corrigé de `remise_percent`, parmi les
//! prix des fournisseurs **actifs** (`llx_societe.status = 1`). Deux écarts
//! assumés, écrits ici pour qu'on ne les découvre pas en production :
//!
//! 1. Dolibarr ne filtre pas `llx_product_fournisseur_price.status` ; ce module
//!    exige `status = 1`. Un prix fournisseur désactivé est une ligne rayée :
//!    s'en servir pour établir un coût, c'est ressusciter un prix qu'on a
//!    justement retiré ;
//! 2. les `charges` (frais d'approche) ne sont pas ajoutées : Dolibarr ne les
//!    compte que si la constante `PRODUCT_CHARGES` est active, et la base de
//!    référence n'en porte aucune. On ne facture pas une hypothèse.
//!
//! # Ce qui n'est pas porté, et pourquoi c'est dit
//!
//! - **`fk_bom_child`** : une ligne peut pointer vers une autre nomenclature
//!   (sous-ensemble). Ce module écrit toujours `NULL`. Le coût d'un
//!   sous-ensemble demanderait une récursion et une détection de cycle ; tant
//!   qu'elle n'existe pas, mieux vaut une nomenclature plate et honnête qu'une
//!   récursion à moitié faite qui boucle sur elle-même ;
//! - **`fk_default_workstation` / `duration`** : le coût de la main-d'œuvre
//!   passe par les postes de travail et les taux horaires, hors du périmètre
//!   d'un coût *matière*. Les heures se chiffrent, mais ailleurs ;
//! - **`qty_frozen`, `disable_stock_change`, `efficiency != 1`** : colonnes
//!   écrites à leur valeur par défaut. `efficiency` est malgré tout relu et
//!   appliqué au coût (division), pour que deux nomenclatures modifiées à la
//!   main dans l'interface Dolibarr donnent le même chiffre des deux côtés ;
//! - **la renumérotation `(PROV…)`** : `validate()` remplace une référence
//!   provisoire par une référence définitive. Ici la référence est **exigée à
//!   la création** : on refuse le vide plutôt que d'inventer un numéro, et
//!   `valider` ne renomme donc rien.
//!
//! # Le champ `ref`
//!
//! La consigne nomme le champ `ref` ; en Rust `ref` est un mot réservé, le
//! champ s'appelle donc `ref_`, comme `Product::ref_` et `Entrepot::ref_`
//! ailleurs dans le noyau. Même valeur, même colonne.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

/// Brouillon : modifiable, supprimable, sans effet sur aucun document.
pub const STATUT_BROUILLON: i16 = 0;
/// Validée : la nomenclature a servi à chiffrer, elle est figée.
pub const STATUT_VALIDEE: i16 = 1;
/// Désactivée : elle a été validée puis retirée du service, elle reste en trace.
pub const STATUT_DESACTIVEE: i16 = 9;

/// `bomtype` : on fabrique l'ouvrage à partir de ses composants.
pub const TYPE_FABRICATION: i16 = 0;
/// `bomtype` : on démonte l'ouvrage. Non écrit par ce module.
pub const TYPE_DEMONTAGE: i16 = 1;

/// Une nomenclature, avec la référence et le libellé de son produit fini.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Nomenclature {
    pub rowid: i64,
    pub entity: i64,
    /// Colonne `ref` (`ref` est un mot réservé en Rust).
    pub ref_: String,
    pub label: String,
    /// `bomtype` : `0` fabrication, `1` démontage.
    pub type_: i16,
    /// Produit fini. Nullable en base, obligatoire à la création ici.
    pub fk_product: Option<i64>,
    /// Référence du produit fini, relue par jointure.
    pub ref_produit: String,
    /// Libellé du produit fini, relu par jointure.
    pub label_produit: String,
    /// Quantité produite : les quantités de composants valent pour celle-ci.
    pub qty: Decimal,
    pub description: Option<String>,
    /// `status` : `0`, `1` ou `9` (voir les constantes).
    pub statut: i16,
    pub date_creation: Option<String>,
    pub date_valid: Option<String>,
}

impl Nomenclature {
    pub fn est_brouillon(&self) -> bool {
        self.statut == STATUT_BROUILLON
    }

    pub fn est_validee(&self) -> bool {
        self.statut == STATUT_VALIDEE
    }

    pub fn est_desactivee(&self) -> bool {
        self.statut == STATUT_DESACTIVEE
    }
}

/// Une ligne de nomenclature : un composant et sa quantité.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Composant {
    pub rowid: i64,
    pub fk_bom: i64,
    pub fk_product: i64,
    pub ref_produit: String,
    pub label_produit: String,
    pub qty: Decimal,
    /// Unité de la ligne (`llx_c_units.rowid`), facultative.
    pub fk_unit: Option<i64>,
    /// La colonne `description` de la ligne : le commentaire de l'auteur.
    pub commentaire: Option<String>,
    /// Rang d'affichage, à partir de 1, comme Dolibarr (`line_max() + 1`).
    pub position: i32,
}

/// Ce qu'il faut pour créer une nomenclature.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NouvelleNomenclature {
    pub ref_: String,
    pub label: String,
    /// Produit fini : ce que la nomenclature fabrique.
    pub fk_product: i64,
    /// Quantité produite (`1` = « pour un m² », `10` = « pour 10 m² »...).
    pub qty: Decimal,
    pub description: Option<String>,
    /// `llx_bom_bom.fk_user_creat` est `NOT NULL` **et** clé étrangère vers
    /// `llx_user.rowid` : un identifiant inventé est refusé par la base.
    pub fk_user_creat: i64,
}

impl NouvelleNomenclature {
    pub fn new(ref_: &str, label: &str, fk_product: i64, qty: Decimal, fk_user_creat: i64) -> Self {
        Self {
            ref_: ref_.to_string(),
            label: label.to_string(),
            fk_product,
            qty,
            description: None,
            fk_user_creat,
        }
    }
}

/// Un composant à écrire. Il n'a pas de rang : l'ordre du vecteur fait foi.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NouveauComposant {
    pub fk_product: i64,
    pub qty: Decimal,
    pub fk_unit: Option<i64>,
    pub commentaire: Option<String>,
}

impl NouveauComposant {
    pub fn new(fk_product: i64, qty: Decimal) -> Self {
        Self {
            fk_product,
            qty,
            fk_unit: None,
            commentaire: None,
        }
    }
}

/// Colonnes relues par `map_nomenclature`, dans cet ordre.
const SELECT_NOMENCLATURE: &str = "b.rowid, b.entity, b.ref, COALESCE(b.label, ''), b.bomtype, \
     b.fk_product, COALESCE(p.ref, ''), COALESCE(p.label, ''), CAST(b.qty AS CHAR), \
     b.description, b.status, \
     DATE_FORMAT(b.date_creation, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(b.date_valid, '%Y-%m-%d %H:%i:%s')";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

/// Un texte facultatif ramené à `None` s'il est vide : une chaîne blanche
/// stockée en base se relit comme une valeur, pas comme une absence.
fn texte_utile(valeur: Option<&str>) -> Option<&str> {
    valeur.map(str::trim).filter(|t| !t.is_empty())
}

fn map_nomenclature(row: Row) -> Nomenclature {
    Nomenclature {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        label: opt_s(&row, 3).unwrap_or_default(),
        type_: opt_i(&row, 4).unwrap_or(0) as i16,
        fk_product: opt_i(&row, 5),
        ref_produit: opt_s(&row, 6).unwrap_or_default(),
        label_produit: opt_s(&row, 7).unwrap_or_default(),
        // `qty` est nullable en base ; un NULL se lit comme zéro plutôt que de
        // faire échouer la liste entière sur une ligne écrite à la main.
        qty: opt_dec(&row, 8).unwrap_or_default(),
        description: opt_s(&row, 9),
        statut: opt_i(&row, 10).unwrap_or(0) as i16,
        date_creation: opt_s(&row, 11),
        date_valid: opt_s(&row, 12),
    }
}

/// Les nomenclatures d'une entité, par référence croissante.
pub fn list(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Nomenclature>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_NOMENCLATURE} FROM llx_bom_bom b \
         LEFT JOIN llx_product p ON p.rowid = b.fk_product \
         WHERE b.entity = ? ORDER BY b.ref ASC, b.rowid ASC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limit, offset))?;
    Ok(rows.into_iter().map(map_nomenclature).collect())
}

/// Une nomenclature par son identifiant, dans son entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Nomenclature>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_NOMENCLATURE} FROM llx_bom_bom b \
         LEFT JOIN llx_product p ON p.rowid = b.fk_product \
         WHERE b.entity = ? AND b.rowid = ?"
    );
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_nomenclature))
}

/// Les composants d'une nomenclature, dans l'ordre d'affichage.
///
/// Pas de filtre d'entité : `llx_bom_bomline` n'en a pas, la ligne appartient à
/// l'en-tête et hérite de son entité. C'est aussi ce que fait `fetchLines()`.
pub fn composants(pool: &Pool, fk_bom: i64) -> Result<Vec<Composant>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT l.rowid, l.fk_bom, l.fk_product, COALESCE(p.ref, ''), COALESCE(p.label, ''), \
                CAST(l.qty AS CHAR), l.fk_unit, l.description, l.position \
         FROM llx_bom_bomline l \
         LEFT JOIN llx_product p ON p.rowid = l.fk_product \
         WHERE l.fk_bom = ? ORDER BY l.position ASC, l.rowid ASC",
        (fk_bom,),
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Composant {
            rowid: opt_i(&r, 0).unwrap_or(0),
            fk_bom: opt_i(&r, 1).unwrap_or(0),
            fk_product: opt_i(&r, 2).unwrap_or(0),
            ref_produit: opt_s(&r, 3).unwrap_or_default(),
            label_produit: opt_s(&r, 4).unwrap_or_default(),
            qty: opt_dec(&r, 5).unwrap_or_default(),
            fk_unit: opt_i(&r, 6),
            commentaire: opt_s(&r, 7),
            position: opt_i(&r, 8).unwrap_or(0) as i32,
        })
        .collect())
}

/// Référence d'un produit de l'entité, ou `None` s'il n'y est pas.
fn reference_produit(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_product: i64,
) -> Result<Option<String>, Box<dyn Error>> {
    let reference: Option<String> = conn.exec_first(
        "SELECT ref FROM llx_product WHERE entity = :entity AND rowid = :produit",
        params! { "entity" => entity, "produit" => fk_product },
    )?;
    Ok(reference)
}

/// Contrôles faits **avant toute écriture** : une nomenclature refusée ne doit
/// pas laisser d'en-tête orphelin ni consommer de référence.
fn verifier_entree(
    conn: &mut mysql::PooledConn,
    entity: i64,
    entete: &NouvelleNomenclature,
    composants: &[NouveauComposant],
) -> Result<(), Box<dyn Error>> {
    let reference = entete.ref_.trim();
    if reference.is_empty() {
        return Err(
            "la référence de la nomenclature est obligatoire : sans elle, \
                    la pièce n'est pas citable dans un devis"
                .into(),
        );
    }
    if entete.label.trim().is_empty() {
        return Err("le libellé de la nomenclature est obligatoire : \
                    « PR-001 » ne dit pas ce qu'on fabrique"
            .into());
    }
    // Strictement positive : une nomenclature de 0 m² ne décrit aucun ouvrage et
    // diviserait le coût unitaire par zéro.
    if entete.qty <= Decimal::ZERO {
        return Err(format!(
            "la quantité produite doit être strictement positive (reçu {}) : \
             une nomenclature pour zéro ouvrage ne décrit rien",
            entete.qty
        )
        .into());
    }

    if composants.is_empty() {
        return Err("une nomenclature sans composant ne décrit rien : \
                    ajoutez au moins un article (pierre, sable, ciment, heure…)"
            .into());
    }

    let reference_fini = reference_produit(conn, entity, entete.fk_product)?.ok_or_else(|| {
        format!(
            "le produit fini {} n'existe pas dans l'entité {entity}",
            entete.fk_product
        )
    })?;

    for (rang, composant) in composants.iter().enumerate() {
        if composant.qty <= Decimal::ZERO {
            return Err(format!(
                "le composant n°{} (produit {}) a une quantité non strictement positive ({}) : \
                 une ligne à zéro ne coûte rien et ne sert rien",
                rang + 1,
                composant.fk_product,
                composant.qty
            )
            .into());
        }
        if composant.fk_product == entete.fk_product {
            return Err(format!(
                "une pièce ne se contient pas : le produit {reference_fini} ({}) \
                 est le produit fini de cette nomenclature, il ne peut pas en être \
                 un composant — le coût se calculerait sur lui-même",
                entete.fk_product
            )
            .into());
        }
        if reference_produit(conn, entity, composant.fk_product)?.is_none() {
            return Err(format!(
                "le composant {} (ligne n°{}) n'existe pas dans l'entité {entity}",
                composant.fk_product,
                rang + 1
            )
            .into());
        }
    }

    // `llx_bom_bom.fk_user_creat` porte une clé étrangère vers `llx_user` : un
    // identifiant inventé ferait échouer l'insertion sur une erreur 1452 que
    // personne ne sait lire. On le dit en français, et avant d'écrire.
    let auteur: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_user WHERE rowid = :auteur",
        params! { "auteur" => entete.fk_user_creat },
    )?;
    if auteur.is_none() {
        return Err(format!(
            "l'utilisateur {} n'existe pas : fk_user_creat est une clé étrangère \
             vers llx_user, la base refuserait la nomenclature",
            entete.fk_user_creat
        )
        .into());
    }

    // `uk_bom_bom_ref(ref, entity)` : la référence est unique par entité. On le
    // dit en français plutôt que de laisser remonter une erreur 1062.
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_bom_bom WHERE entity = :entity AND ref = :ref",
        params! { "entity" => entity, "ref" => reference },
    )?;
    if deja.is_some() {
        return Err(format!(
            "une nomenclature porte déjà la référence {reference} dans l'entité {entity}"
        )
        .into());
    }

    Ok(())
}

/// Crée la nomenclature et tous ses composants, ou rien.
///
/// L'en-tête et les lignes vivent dans la même transaction : une nomenclature
/// sans composant n'est pas une pièce, seulement une référence consommée — et
/// c'est exactement ce que le contrôle d'entrée refuse avant d'écrire.
///
/// Aucune ligne n'est écrite avec un `fk_bom_child` : voir l'en-tête du module.
pub fn creer(
    pool: &Pool,
    entity: i64,
    entete: &NouvelleNomenclature,
    composants: &[NouveauComposant],
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    verifier_entree(&mut conn, entity, entete, composants)?;

    let mut tx = conn.start_transaction(TxOpts::default())?;
    let insere = tx.exec_drop(
        "INSERT INTO llx_bom_bom \
         (entity, ref, bomtype, label, fk_product, description, qty, date_creation, \
          fk_user_creat, status) \
         VALUES (:entity, :ref, :bomtype, :label, :produit, :description, :qty, NOW(), \
          :auteur, :statut)",
        params! {
            "entity" => entity,
            "ref" => entete.ref_.trim(),
            "bomtype" => TYPE_FABRICATION,
            "label" => entete.label.trim(),
            "produit" => entete.fk_product,
            "description" => texte_utile(entete.description.as_deref()),
            "qty" => entete.qty.to_string(),
            "auteur" => entete.fk_user_creat,
            // Écrit explicitement : la colonne n'a pas de défaut en base, et un
            // brouillon est le seul état dans lequel `addLine()` accepte d'écrire.
            "statut" => STATUT_BROUILLON,
        },
    );
    if let Err(e) = insere {
        tx.rollback()?;
        // 1062 : la référence vient d'être prise entre le contrôle et l'insert
        // (l'interface Dolibarr écrit sans passer par nous). On le dit en
        // français ; le contrôle préalable rend le cas improbable, pas
        // impossible.
        let message = e.to_string();
        if message.contains("1062") {
            return Err(format!(
                "une nomenclature porte déjà la référence {} dans l'entité {entity}",
                entete.ref_.trim()
            )
            .into());
        }
        return Err(e.into());
    }
    // `Transaction::last_insert_id()` rend un `Option<u64>`, contrairement à
    // `Conn::last_insert_id()` qui rend un `u64` : on ne mélange pas les deux.
    let rowid = tx.last_insert_id().unwrap_or(0) as i64;

    for (rang, composant) in composants.iter().enumerate() {
        // `position` suit Dolibarr : `line_max() + 1`, donc à partir de 1.
        // `efficiency`, `qty_frozen` et `disable_stock_change` gardent leur
        // valeur par défaut (1, 0, 0) : pas de perte déclarée, pas de stock gelé.
        tx.exec_drop(
            "INSERT INTO llx_bom_bomline \
             (fk_bom, fk_product, qty, fk_unit, description, position) \
             VALUES (:bom, :produit, :qty, :unite, :commentaire, :position)",
            params! {
                "bom" => rowid,
                "produit" => composant.fk_product,
                "qty" => composant.qty.to_string(),
                "unite" => composant.fk_unit,
                "commentaire" => texte_utile(composant.commentaire.as_deref()),
                "position" => rang as i32 + 1,
            },
        )?;
    }

    tx.commit()?;
    Ok(rowid)
}

/// Statut courant d'une nomenclature, ou une erreur qui la nomme.
fn statut(conn: &mut mysql::PooledConn, entity: i64, rowid: i64) -> Result<i16, Box<dyn Error>> {
    let statut: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_bom_bom WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        Some(s) => Ok(s as i16),
        None => Err(format!("la nomenclature {rowid} n'existe pas dans l'entité {entity}").into()),
    }
}

/// Valide une nomenclature : elle passe de brouillon à figée.
///
/// Valider deux fois n'est pas anodine : c'est un appelant qui a perdu le fil de
/// son écran, et le laisser passer masquerait un double traitement. Dolibarr,
/// lui, retourne `0` sans rien faire (`if ($this->status == self::STATUS_VALIDATED)
/// return 0;`) ; on préfère le dire.
///
/// Une nomenclature désactivée (`9`) ne se revalide pas non plus : Dolibarr a un
/// `reopen()` distinct pour ça, et une réactivation silencieuse remettrait en
/// service des prix qu'on a retirés exprès.
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    match statut(&mut conn, entity, rowid)? {
        STATUT_VALIDEE => return Err("cette nomenclature est déjà validée".into()),
        STATUT_DESACTIVEE => {
            return Err(
                "cette nomenclature est désactivée : elle ne se revalide pas, \
                        créez-en une nouvelle version"
                    .into(),
            )
        }
        _ => {}
    }

    let lignes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_bom_bomline WHERE fk_bom = :rowid",
        params! { "rowid" => rowid },
    )?;
    if lignes.unwrap_or(0) == 0 {
        return Err("une nomenclature sans composant ne se valide pas : \
                    elle ne décrit aucun ouvrage"
            .into());
    }

    // Pas de renommage de référence : elle a été exigée à la création, il n'y a
    // donc aucune référence `(PROV…)` à remplacer.
    conn.exec_drop(
        "UPDATE llx_bom_bom SET status = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Désactive une nomenclature validée : elle quitte le service sans disparaître.
///
/// Seule une nomenclature validée se désactive — c'est la règle de
/// `BOM::setCanceled()` (`if ($this->status != self::STATUS_VALIDATED) return -1;`).
/// Un brouillon n'a jamais servi : il se supprime.
pub fn desactiver(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    match statut(&mut conn, entity, rowid)? {
        STATUT_BROUILLON => return Err("un brouillon ne se désactive pas : il se supprime".into()),
        STATUT_DESACTIVEE => return Err("cette nomenclature est déjà désactivée".into()),
        _ => {}
    }

    conn.exec_drop(
        "UPDATE llx_bom_bom SET status = :statut WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_DESACTIVEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Supprime un brouillon et ses composants.
///
/// Une nomenclature validée a servi à chiffrer un devis : la supprimer
/// effacerait la justification d'un prix déjà envoyé. Une nomenclature
/// désactivée a été validée un jour : elle reste une trace, elle ne
/// s'efface pas non plus. Seul le brouillon, qui n'a rien produit, disparaît.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    match statut(&mut conn, entity, rowid)? {
        STATUT_VALIDEE => {
            return Err("une nomenclature validée ne se supprime pas : \
                        elle justifie un chiffrage déjà diffusé"
                .into())
        }
        STATUT_DESACTIVEE => {
            return Err("une nomenclature désactivée ne se supprime pas : \
                        elle a été validée un jour, elle reste une trace"
                .into())
        }
        _ => {}
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    // Les lignes partent avec l'en-tête : une ligne orpheline serait un composant
    // fantôme, invisible dans l'application et pourtant présent dans les sommes.
    tx.exec_drop(
        "DELETE FROM llx_bom_bomline WHERE fk_bom = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_bom_bom WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Ce qu'il faut pour chiffrer une ligne : la quantité, la perte déclarée et les
/// trois prix candidats de la cascade Dolibarr.
#[derive(Debug, Clone)]
struct LigneCout {
    reference: String,
    label: String,
    qty: Decimal,
    efficiency: Decimal,
    cost_price: Option<Decimal>,
    pmp: Decimal,
    prix_fournisseur: Option<Decimal>,
}

/// Lit les lignes d'une nomenclature avec leurs prix candidats, en une requête.
///
/// Le prix fournisseur est calculé en SQL comme le fait
/// `find_min_price_product_fournisseur()` : `price / quantity` corrigé de
/// `remise_percent`, minimum des fournisseurs actifs. `quantity = 0` (ou NULL)
/// signifie « prix pour le lot entier », Dolibarr prend alors `price` tel quel.
fn lignes_de_cout(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_bom: i64,
) -> Result<Vec<LigneCout>, Box<dyn Error>> {
    let rows: Vec<Row> = conn.exec(
        "SELECT COALESCE(p.ref, ''), COALESCE(p.label, ''), CAST(l.qty AS CHAR), \
                CAST(COALESCE(l.efficiency, 1) AS CHAR), \
                CAST(p.cost_price AS CHAR), CAST(p.pmp AS CHAR), \
                (SELECT CAST(MIN(CASE WHEN pfp.quantity IS NULL OR pfp.quantity = 0 \
                                      THEN pfp.price \
                                      ELSE pfp.price / pfp.quantity END \
                                 * (1 - pfp.remise_percent / 100)) AS CHAR) \
                   FROM llx_product_fournisseur_price pfp \
                   JOIN llx_societe s ON s.rowid = pfp.fk_soc AND s.status = 1 \
                  WHERE pfp.fk_product = l.fk_product AND pfp.entity = :entity \
                    AND pfp.status = 1) \
         FROM llx_bom_bomline l \
         JOIN llx_product p ON p.rowid = l.fk_product \
         WHERE l.fk_bom = :bom \
         ORDER BY l.position ASC, l.rowid ASC",
        params! { "entity" => entity, "bom" => fk_bom },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| LigneCout {
            reference: opt_s(&r, 0).unwrap_or_default(),
            label: opt_s(&r, 1).unwrap_or_default(),
            qty: opt_dec(&r, 2).unwrap_or_default(),
            efficiency: opt_dec(&r, 3).unwrap_or(Decimal::ONE),
            cost_price: opt_dec(&r, 4),
            pmp: opt_dec(&r, 5).unwrap_or_default(),
            prix_fournisseur: opt_dec(&r, 6),
        })
        .collect())
}

/// Prix d'achat d'un composant, selon la cascade de `BOM::calculateCosts()`.
///
/// Renvoie `Err` — jamais zéro — quand aucun prix n'est connu : voir l'en-tête
/// du module. Le message nomme le produit, sinon on ne sait pas lequel corriger.
fn prix_achat(ligne: &LigneCout) -> Result<Decimal, Box<dyn Error>> {
    // `cost_price` fait foi ; `pmp` ne prend le relais que s'il est NULL, comme
    // `is_null($cost_price) ? $pmp : $cost_price`.
    let candidat = match ligne.cost_price {
        Some(cout) => cout,
        None => ligne.pmp,
    };
    if candidat > Decimal::ZERO {
        return Ok(candidat);
    }
    match ligne.prix_fournisseur {
        Some(prix) if prix > Decimal::ZERO => Ok(prix),
        _ => Err(format!(
            "aucun prix d'achat connu pour le composant {} ({}) : ni cost_price, \
             ni PMP, ni prix fournisseur — un coût de revient partiel serait un \
             coût faux, renseignez le prix du produit avant de chiffrer",
            ligne.reference, ligne.label
        )
        .into()),
    }
}

/// Coût de matière d'une nomenclature : `Σ quantité × prix d'achat`.
///
/// C'est un coût **total pour la quantité produite de l'en-tête**, pas un coût
/// unitaire. Dolibarr expose aussi `unit_cost = total_cost / qty` ; cette
/// division est laissée à l'appelant, qui sait s'il veut parler au m² ou au lot.
///
/// Échoue si un seul composant n'a pas de prix : voir `prix_achat`.
pub fn cout_matiere(
    pool: &Pool,
    entity: i64,
    fk_nomenclature: i64,
) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    statut(&mut conn, entity, fk_nomenclature)?;

    let lignes = lignes_de_cout(&mut conn, entity, fk_nomenclature)?;
    if lignes.is_empty() {
        return Err(format!(
            "la nomenclature {fk_nomenclature} n'a aucun composant : son coût de \
             matière n'est pas zéro, il est inconnu"
        )
        .into());
    }

    let mut total = Decimal::ZERO;
    for ligne in &lignes {
        let prix = prix_achat(ligne)?;
        // Dolibarr borne `efficiency` à `]0, 1]` à l'écriture (`if ($efficiency <= 0
        // || $efficiency > 1) $efficiency = 1;`). On refait le contrôle à la
        // lecture : une valeur posée à la main en base ne doit pas diviser par zéro.
        let rendement = if ligne.efficiency > Decimal::ZERO && ligne.efficiency <= Decimal::ONE {
            ligne.efficiency
        } else {
            Decimal::ONE
        };
        total += ligne.qty * prix / rendement;
    }

    // Arrondi final seulement : arrondir chaque ligne ferait dériver un total de
    // plusieurs centimes sur une nomenclature à vingt composants.
    Ok(money::round_money(total))
}

/// Quantité totale commandée par une nomenclature : `Σ quantité des composants`.
///
/// On ne passe **pas** par `money::round_money` : une quantité n'est pas un
/// montant, 1,235 m³ ne devient pas 1,24 m³. Les valeurs sont lues une par une
/// puis additionnées en `Decimal`, ce qui évite le bruit du `double` que
/// donnerait un `SUM()` côté SQL (`0.1 + 0.2` = `0.30000000000000004`).
///
/// Une nomenclature sans composant vaut zéro : c'est une quantité réellement
/// nulle. Le coût, lui, échoue dans ce cas — un ouvrage vide n'est pas gratuit,
/// il est mal décrit.
pub fn quantite_totale(
    pool: &Pool,
    entity: i64,
    fk_nomenclature: i64,
) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    statut(&mut conn, entity, fk_nomenclature)?;

    let quantites: Vec<Option<String>> = conn.exec(
        "SELECT CAST(qty AS CHAR) FROM llx_bom_bomline WHERE fk_bom = :bom",
        params! { "bom" => fk_nomenclature },
    )?;

    Ok(quantites
        .into_iter()
        .filter_map(|q| q.and_then(|s| Decimal::from_str(s.trim()).ok()))
        .sum())
}
