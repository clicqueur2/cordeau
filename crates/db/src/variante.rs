//! Variantes (déclinaisons) : attributs, valeurs et combinaisons.
//!
//! Un produit « Chemise » se décline en Taile S/M/L et Couleur Bleu/Rouge.
//! Dolibarr range cela en trois étages :
//!
//! | objet | table | rôle |
//! |---|---|---|
//! | attribut | `llx_product_attribute` | l'axe de déclinaison (« Taille ») |
//! | valeur | `llx_product_attribute_value` | une modalité de l'axe (« S ») |
//! | combinaison | `llx_product_attribute_combination` | l'assemblage (S + Bleu) et son produit enfant |
//! | lignes | `llx_product_attribute_combination2val` | une ligne par valeur de la combinaison |
//!
//! ## Où se trouve le cloisonnement par société
//!
//! C'est le point qui se trompe en silence, alors il est écrit ici.
//!
//! Trois des cinq tables portent une colonne `entity` :
//! `llx_product_attribute` (défaut 1), `llx_product_attribute_value` (défaut 1)
//! et `llx_product_attribute_combination` (défaut 1). **Deux n'en ont pas du
//! tout** : `llx_product_attribute_combination2val` et
//! `llx_product_attribute_combination_price_level`. Vérifié sur la base
//! (`information_schema.columns`, 06/11/2026) :
//!
//! ```text
//! llx_product_attribute_combination        -> entity int(11) NOT NULL DEFAULT 1
//! llx_product_attribute_combination2val    -> (aucune colonne entity)
//! llx_product_attribute_combination_price_level -> (aucune colonne entity)
//! ```
//!
//! Conséquence pratique : une ligne de `combination2val` n'est **pas** filtrée
//! par la société. Son cloisonnement vient uniquement de la combinaison à
//! laquelle elle se rattache. Toute lecture passe donc par une jointure sur
//! `llx_product_attribute_combination` avec `c.entity = :entity`, et toute
//! suppression commence par vérifier que la combinaison visée appartient bien à
//! la société : sans ce garde-fou, un identifiant deviné effacerait les valeurs
//! d'une autre société, et une combinaison d'une autre société ressortirait
//! avec le bon identifiant.
//!
//! Les valeurs, elles, se lisent à travers leur attribut : `valeurs()` n'a pas
//! de paramètre `entity` parce que l'identifiant d'attribut suffit et que
//! l'attribut porte déjà la société.
//!
//! ## Deux clés uniques qui ne contiennent pas `entity`
//!
//! Relevé dans `information_schema.statistics` :
//!
//! * `llx_product_attribute` : `uk_product_attribute_ref` porte sur `ref`
//!   **seule** (il existe par ailleurs un index non unique
//!   `idx_product_attribute_entity` sur `entity`). Deux sociétés ne peuvent
//!   donc pas avoir un attribut de même référence, contrairement à ce que
//!   laisserait croire l'index sur `entity`.
//! * `llx_product_attribute_value` : `uk_product_attribute_value` porte sur
//!   `(fk_product_attribute, ref)`, **sans** `entity`. Une référence de valeur
//!   est donc unique pour un attribut donné, quelle que soit la société.
//!
//! Le code suit la base, pas l'intuition : il refuse explicitement ces
//! collisions au lieu de laisser remonter une erreur de doublon MySQL.
//!
//! ## Ce que ce module ne fait pas
//!
//! `llx_product_attribute_combination_price_level` stocke une variation de
//! prix par niveau de prix (client, groupe…). Ce n'est pas traité ici : la
//! création d'une combinaison écrit `variation_price = 0`, la signature ne
//! portant aucun montant. La colonne est néanmoins lue en `Decimal`, jamais en
//! flottant, pour la même raison que dans `product` : un `double(24,8)` qui
//! passe par un `f64` perd des centimes.

use std::collections::BTreeMap;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row};
use rust_decimal::Decimal;

/// Un axe de déclinaison, à créer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NouvelAttribut {
    /// Le code court (« TAILLE »). Unique dans toute la base, cf. doc du module.
    pub ref_: String,
    /// Le libellé affiché (« Taille »).
    pub label: String,
    /// L'ordre d'affichage ; les valeurs négatives sont ramenées à 0.
    pub position: i32,
}

impl NouvelAttribut {
    pub fn new(ref_: &str, label: &str) -> Self {
        Self {
            ref_: ref_.to_string(),
            label: label.to_string(),
            position: 0,
        }
    }
}

/// Une modalité d'un attribut, à créer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NouvelleValeur {
    pub fk_attribut: i64,
    /// Le code court (« S »), unique pour cet attribut.
    pub ref_: String,
    /// Le libellé affiché (« Small »).
    pub value: String,
    pub position: i32,
}

impl NouvelleValeur {
    pub fn new(fk_attribut: i64, ref_: &str, value: &str) -> Self {
        Self {
            fk_attribut,
            ref_: ref_.to_string(),
            value: value.to_string(),
            position: 0,
        }
    }
}

/// Un axe de déclinaison tel qu'il est en base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribut {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub label: String,
    pub position: i32,
}

/// Une modalité telle qu'elle est en base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Valeur {
    pub rowid: i64,
    pub fk_attribut: i64,
    pub entity: i64,
    pub ref_: String,
    pub value: String,
    pub position: i32,
}

/// Un assemblage de valeurs, avec son produit enfant.
///
/// Pas de `Eq` ici, contrairement aux autres structures : la variation de poids
/// est un `f64`, et un flottant n'est pas `Eq` (deux `NaN` ne sont pas égaux).
/// La comparaison reste possible, elle n'est simplement pas totale.
#[derive(Debug, Clone, PartialEq)]
pub struct Combinaison {
    pub rowid: i64,
    pub entity: i64,
    pub fk_produit_pere: i64,
    /// Le produit enfant réel. `0` quand aucun n'est rattaché : la colonne est
    /// `NOT NULL` en base, Dolibarr n'y met jamais `NULL`.
    pub fk_produit_fils: i64,
    /// Variation de prix de la combinaison, lue sans flottant.
    pub variation_prix: Decimal,
    /// Variation de poids ; ce n'est pas un montant, d'où le `f64`.
    pub variation_poids: f64,
    pub valeurs: Vec<Valeur>,
}

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

/// Un `double(24,8)` se lit en texte puis en `Decimal` : passer par un `f64`
/// ferait perdre des centimes sur les variations de prix.
fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn opt_f(row: &Row, i: usize) -> Option<f64> {
    row.get_opt::<f64, _>(i).and_then(|r| r.ok())
}

fn map_attribut(row: Row) -> Attribut {
    Attribut {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        label: opt_s(&row, 3).unwrap_or_default(),
        position: opt_i(&row, 4).unwrap_or(0) as i32,
    }
}

/// Lit une valeur à partir de l'indice `base`.
///
/// Les appelants n'ont pas tous la même requête : `valeurs()` commence à la
/// colonne 0, la jointure de `combinaisons()` préfixe par la combinaison. Un
/// décalage explicite évite de dupliquer la lecture.
fn map_valeur(row: &Row, base: usize) -> Valeur {
    Valeur {
        rowid: opt_i(row, base).unwrap_or(0),
        fk_attribut: opt_i(row, base + 1).unwrap_or(0),
        ref_: opt_s(row, base + 2).unwrap_or_default(),
        value: opt_s(row, base + 3).unwrap_or_default(),
        position: opt_i(row, base + 4).unwrap_or(0) as i32,
        entity: opt_i(row, base + 5).unwrap_or(1),
    }
}

fn map_combinaison(row: Row) -> Combinaison {
    Combinaison {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        fk_produit_pere: opt_i(&row, 2).unwrap_or(0),
        fk_produit_fils: opt_i(&row, 3).unwrap_or(0),
        variation_prix: opt_dec(&row, 4).unwrap_or_default(),
        variation_poids: opt_f(&row, 5).unwrap_or(0.0),
        valeurs: Vec::new(),
    }
}

/// Les attributs d'une société, dans l'ordre d'affichage.
pub fn attributs(pool: &Pool, entity: i64) -> Result<Vec<Attribut>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT rowid, entity, ref, label, position \
         FROM llx_product_attribute \
         WHERE entity = :entity \
         ORDER BY position ASC, label ASC, ref ASC",
        params! { "entity" => entity },
    )?;
    Ok(rows.into_iter().map(map_attribut).collect())
}

/// Les valeurs d'un attribut, dans l'ordre d'affichage.
///
/// Pas de `entity` ici : l'attribut la porte déjà, et une valeur n'existe jamais
/// hors de son attribut.
pub fn valeurs(pool: &Pool, fk_attribut: i64) -> Result<Vec<Valeur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT rowid, fk_product_attribute, ref, value, position, entity \
         FROM llx_product_attribute_value \
         WHERE fk_product_attribute = :attr \
         ORDER BY position ASC, ref ASC",
        params! { "attr" => fk_attribut },
    )?;
    Ok(rows.into_iter().map(|r| map_valeur(&r, 0)).collect())
}

/// Crée un attribut, ou rend celui qui porte déjà cette référence.
///
/// Rejouer la création est sans effet : un appelant qui resynchronise une
/// gamme de produits ne doit pas se retrouver avec une erreur au deuxième
/// passage. En revanche une référence déjà prise par une **autre** société est
/// refusée avec un message explicite, parce que la clé unique de Dolibarr
/// porte sur `ref` seule et que l'erreur MySQL brute ne dirait pas pourquoi.
pub fn creer_attribut(
    pool: &Pool,
    entity: i64,
    nouvel: &NouvelAttribut,
) -> Result<i64, Box<dyn Error>> {
    let reference = nouvel.ref_.trim();
    let label = nouvel.label.trim();
    if reference.is_empty() {
        return Err("la référence de l'attribut est obligatoire".into());
    }
    if label.is_empty() {
        return Err("le libellé de l'attribut est obligatoire".into());
    }

    let mut conn = pool.get_conn()?;
    let deja: Option<(i64, i64)> = conn.exec_first(
        "SELECT rowid, entity FROM llx_product_attribute WHERE ref = :ref",
        params! { "ref" => reference },
    )?;
    match deja {
        Some((rowid, societe)) if societe == entity => return Ok(rowid),
        Some((rowid, societe)) => {
            return Err(format!(
                "la référence « {reference} » est déjà utilisée par l'attribut {rowid} \
                 de la société {societe} : la clé unique de Dolibarr porte sur `ref` seule, \
                 pas sur (entity, ref)"
            )
            .into())
        }
        None => {}
    }

    conn.exec_drop(
        "INSERT INTO llx_product_attribute (entity, ref, label, position) \
         VALUES (:entity, :ref, :label, :position)",
        params! {
            "entity" => entity,
            "ref" => reference,
            "label" => label,
            // `position` est NOT NULL : une valeur négative n'a pas de sens.
            "position" => nouvel.position.max(0),
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Crée une valeur d'attribut.
///
/// Refuse une référence vide, un libellé vide, un attribut inexistant dans
/// cette société, et une référence déjà prise pour le même attribut — la clé
/// unique `(fk_product_attribute, ref)` ne contient pas `entity`, le contrôle
/// est donc fait au niveau de l'attribut.
pub fn creer_valeur(
    pool: &Pool,
    entity: i64,
    nouvelle: &NouvelleValeur,
) -> Result<i64, Box<dyn Error>> {
    let reference = nouvelle.ref_.trim();
    let libelle = nouvelle.value.trim();
    if reference.is_empty() {
        return Err("la référence de la valeur est obligatoire".into());
    }
    if libelle.is_empty() {
        return Err("le libellé de la valeur est obligatoire".into());
    }

    let mut conn = pool.get_conn()?;

    // L'attribut doit exister **et** être de cette société : sinon on
    // accrocherait une valeur à l'axe d'une autre société.
    let attribut: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_product_attribute WHERE rowid = :attr AND entity = :entity",
        params! { "attr" => nouvelle.fk_attribut, "entity" => entity },
    )?;
    if attribut.is_none() {
        return Err(format!(
            "l'attribut {} n'existe pas dans la société {entity}",
            nouvelle.fk_attribut
        )
        .into());
    }

    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_product_attribute_value \
         WHERE fk_product_attribute = :attr AND ref = :ref",
        params! { "attr" => nouvelle.fk_attribut, "ref" => reference },
    )?;
    if let Some(rowid) = deja {
        return Err(format!(
            "la référence « {reference} » est déjà prise par la valeur {rowid} de cet attribut"
        )
        .into());
    }

    conn.exec_drop(
        "INSERT INTO llx_product_attribute_value \
           (fk_product_attribute, ref, value, entity, position) \
         VALUES (:attr, :ref, :value, :entity, :position)",
        params! {
            "attr" => nouvelle.fk_attribut,
            "ref" => reference,
            "value" => libelle,
            "entity" => entity,
            "position" => nouvelle.position.max(0),
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Crée une combinaison : l'ensemble des valeurs, plus le produit enfant réel.
///
/// Refuse :
/// * une liste de valeurs vide — une combinaison vide ne décline rien ;
/// * une valeur inexistante ;
/// * une valeur d'une autre société ;
/// * deux valeurs du même attribut — « Taille S + Taille M » n'est pas une
///   combinaison mais une contradiction ;
/// * un produit père (ou fils) absent de cette société, pour ne pas créer une
///   combinaison orpheline que personne ne pourrait rattacher ensuite.
///
/// Les colonnes `variation_price` et `variation_weight` sont `NOT NULL` sans
/// valeur par défaut et le serveur tourne en `STRICT_TRANS_TABLES` : elles sont
/// donc écrites explicitement à 0. La signature ne porte aucun montant, la
/// variation de prix d'une combinaison se règle par les niveaux de prix, hors
/// périmètre de ce module.
pub fn creer_combinaison(
    pool: &Pool,
    entity: i64,
    fk_produit_pere: i64,
    fk_valeurs: &[i64],
    fk_produit_fils: Option<i64>,
) -> Result<i64, Box<dyn Error>> {
    if fk_valeurs.is_empty() {
        return Err("une combinaison doit porter au moins une valeur : \
                    une liste vide ne décline rien"
            .into());
    }

    let mut conn = pool.get_conn()?;

    let pere: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_product WHERE rowid = :produit AND entity = :entity",
        params! { "produit" => fk_produit_pere, "entity" => entity },
    )?;
    if pere.is_none() {
        return Err(format!(
            "le produit père {fk_produit_pere} n'existe pas dans la société {entity}"
        )
        .into());
    }

    if let Some(fils) = fk_produit_fils {
        let existe: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_product WHERE rowid = :produit AND entity = :entity",
            params! { "produit" => fils, "entity" => entity },
        )?;
        if existe.is_none() {
            return Err(
                format!("le produit enfant {fils} n'existe pas dans la société {entity}").into(),
            );
        }
    }

    // Une valeur par attribut au plus : la clé de la table est l'attribut.
    let mut valeur_par_attribut: BTreeMap<i64, i64> = BTreeMap::new();
    for &fk_valeur in fk_valeurs {
        let ligne: Option<(i64, i64)> = conn.exec_first(
            "SELECT fk_product_attribute, entity FROM llx_product_attribute_value \
             WHERE rowid = :valeur",
            params! { "valeur" => fk_valeur },
        )?;
        let Some((fk_attribut, societe_valeur)) = ligne else {
            return Err(format!("la valeur {fk_valeur} n'existe pas").into());
        };
        if societe_valeur != entity {
            return Err(format!(
                "la valeur {fk_valeur} appartient à la société {societe_valeur}, pas à {entity}"
            )
            .into());
        }
        if let Some(deja) = valeur_par_attribut.insert(fk_attribut, fk_valeur) {
            if deja == fk_valeur {
                return Err(format!("la valeur {fk_valeur} est donnée deux fois").into());
            }
            return Err(format!(
                "deux valeurs du même attribut ({fk_attribut}) : {deja} et {fk_valeur} ; \
                 une combinaison ne retient qu'une modalité par axe"
            )
            .into());
        }
    }

    let mut tx = conn.start_transaction(mysql::TxOpts::default())?;
    tx.exec_drop(
        "INSERT INTO llx_product_attribute_combination \
           (fk_product_parent, fk_product_child, variation_price, \
            variation_price_percentage, variation_weight, entity) \
         VALUES (:pere, :fils, 0, NULL, 0, :entity)",
        params! {
            "pere" => fk_produit_pere,
            // `fk_product_child` est NOT NULL : 0 veut dire « pas de fils ».
            "fils" => fk_produit_fils.unwrap_or(0),
            "entity" => entity,
        },
    )?;
    // `last_insert_id` rend un `Option` dans cette version de `mysql` : un
    // `as i64` sur un `Option` ne compile pas. Zero signale une insertion
    // perdue, et une combinaison a 0 ne se relit pas — mieux vaut echouer ici.
    let rowid = tx
        .last_insert_id()
        .ok_or("combinaison inseree sans identifiant")? as i64;

    for (fk_attribut, fk_valeur) in &valeur_par_attribut {
        // Cette table n'a pas d'`entity` : la ligne hérite du cloisonnement de
        // sa combinaison, déjà filtrée par la société juste au-dessus.
        tx.exec_drop(
            "INSERT INTO llx_product_attribute_combination2val \
               (fk_prod_combination, fk_prod_attr, fk_prod_attr_val) \
             VALUES (:combinaison, :attr, :valeur)",
            params! {
                "combinaison" => rowid,
                "attr" => fk_attribut,
                "valeur" => fk_valeur,
            },
        )?;
    }
    tx.commit()?;
    Ok(rowid)
}

/// Les combinaisons d'un produit, valeurs comprises.
///
/// La jointure passe par la combinaison, seule porteuse de `entity` : c'est ce
/// qui empêche de rendre les valeurs d'une autre société. Les valeurs arrivent
/// triées par position d'attribut, puis de valeur, puis par référence : c'est
/// l'ordre de lecture d'un bon de commande (« Taille : S, Couleur : Bleu »),
/// et il ne dépend pas de l'ordre d'insertion.
pub fn combinaisons(
    pool: &Pool,
    entity: i64,
    fk_produit: i64,
) -> Result<Vec<Combinaison>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT c.rowid, c.entity, c.fk_product_parent, c.fk_product_child, \
                CAST(c.variation_price AS CHAR), c.variation_weight \
         FROM llx_product_attribute_combination c \
         WHERE c.entity = :entity AND c.fk_product_parent = :produit \
         ORDER BY c.rowid ASC",
        params! { "entity" => entity, "produit" => fk_produit },
    )?;

    let mut resultat: Vec<Combinaison> = rows.into_iter().map(map_combinaison).collect();
    if resultat.is_empty() {
        return Ok(resultat);
    }

    // Une seule requête pour toutes les valeurs de toutes les combinaisons :
    // une requête par combinaison ferait N+1 allers-retours.
    let lignes: Vec<Row> = conn.exec(
        "SELECT l.fk_prod_combination, v.rowid, v.fk_product_attribute, v.ref, v.value, \
                v.position, v.entity \
         FROM llx_product_attribute_combination2val l \
         JOIN llx_product_attribute_combination c ON c.rowid = l.fk_prod_combination \
         JOIN llx_product_attribute_value v ON v.rowid = l.fk_prod_attr_val \
         JOIN llx_product_attribute a ON a.rowid = v.fk_product_attribute \
         WHERE c.entity = :entity AND c.fk_product_parent = :produit \
         ORDER BY a.position ASC, v.position ASC, v.ref ASC",
        params! { "entity" => entity, "produit" => fk_produit },
    )?;

    let mut par_combinaison: BTreeMap<i64, Vec<Valeur>> = BTreeMap::new();
    for ligne in &lignes {
        let id = opt_i(ligne, 0).unwrap_or(0);
        par_combinaison
            .entry(id)
            .or_default()
            .push(map_valeur(ligne, 1));
    }
    for combinaison in &mut resultat {
        if let Some(valeurs) = par_combinaison.remove(&combinaison.rowid) {
            combinaison.valeurs = valeurs;
        }
    }
    Ok(resultat)
}

/// Supprime une combinaison et ses lignes, et rend le nombre de combinaisons
/// effectivement supprimées (0 si elle n'est pas de cette société).
///
/// L'ordre compte : on vérifie l'appartenance **avant** de toucher aux lignes,
/// parce que `combination2val` ne porte pas d'`entity`. Supprimer directement
/// par identifiant effacerait les valeurs d'une combinaison d'une autre société.
/// Les niveaux de prix éventuels partent aussi, pour ne pas laisser de lignes
/// orphelines que rien ne viendrait plus rattacher.
pub fn supprimer_combinaison(
    pool: &Pool,
    entity: i64,
    rowid: i64,
) -> Result<usize, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let mut tx = conn.start_transaction(mysql::TxOpts::default())?;

    let existe: Option<i64> = tx.exec_first(
        "SELECT rowid FROM llx_product_attribute_combination \
         WHERE rowid = :id AND entity = :entity",
        params! { "id" => rowid, "entity" => entity },
    )?;
    if existe.is_none() {
        // Rien à faire, et surtout rien à écrire : la transaction retombe.
        return Ok(0);
    }

    tx.exec_drop(
        "DELETE FROM llx_product_attribute_combination2val \
         WHERE fk_prod_combination = :id",
        params! { "id" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_product_attribute_combination_price_level \
         WHERE fk_product_attribute_combination = :id",
        params! { "id" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_product_attribute_combination \
         WHERE rowid = :id AND entity = :entity",
        params! { "id" => rowid, "entity" => entity },
    )?;
    let supprimees = tx.affected_rows() as usize;
    tx.commit()?;
    Ok(supprimees)
}
