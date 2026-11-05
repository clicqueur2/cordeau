//! Inventaires (comptage physique) : `llx_inventory` + `llx_inventorydet`.
//!
//! Un inventaire, c'est une **constatation** : à une date, dans un entrepôt,
//! quelqu'un a compté des produits et a trouvé autre chose que ce que le
//! système croyait avoir. La pièce ne dit donc pas « il y a 8 en stock », elle
//! dit « le système dit 8, le magasin en a compté 10, l'écart est de +2 ». C'est
//! cet écart (`qty_regulated`) qui décide de la suite : c'est lui qu'on défend
//! devant un écart d'inventaire, jamais le niveau de stock, qui n'est qu'un
//! souvenir.
//!
//! Les statuts sont ceux de Dolibarr : `0` brouillon, `1` validé. Le cycle est
//! irréversible, comme pour les réceptions : un inventaire validé ne redevient
//! pas un brouillon. Une erreur de saisie se corrige en supprimant le brouillon,
//! pas en remontant le statut.
//!
//! # Aucun mouvement de stock n'est écrit — et c'est un manque assumé
//!
//! Dans Dolibarr, valider un inventaire **crée des mouvements de
//! régularisation** : pour chaque ligne dont l'écart n'est pas nul, une ligne
//! dans `llx_stock_mouvement` et un nouveau `reel` dans `llx_product_stock`,
//! dans la même transaction. Ce module n'écrit dans **aucune** des deux tables.
//! La pièce est **tracée**, le stock ne bouge pas — exactement comme pour les
//! réceptions.
//!
//! Pourquoi ce choix, en toutes lettres : écrire un niveau de stock sans le
//! module de mouvements complet produirait des **quantités fantômes**,
//! c'est-à-dire un stock qui ne s'explique par aucune ligne d'historique. Or
//! c'est la règle du noyau : « le stock affiché est toujours la somme des
//! mouvements, jamais une valeur saisie à part ». Un écart d'inventaire est
//! précisément le seul cas où l'on est tenté de violer cette règle — on
//! « connaît » la bonne valeur, on pourrait l'écrire directement. Ce serait le
//! pire moment : personne ne pourrait plus dire, six mois plus tard, si les
//! 10 unités viennent d'un comptage, d'une saisie manuelle ou d'un bug. Et
//! l'annulation symétrique (que faire si l'inventaire est validé puis
//! contesté ?) n'est pas décidable sans le module de mouvements.
//!
//! Donc : **l'inventaire est constaté, le mouvement de régularisation viendra
//! avec le module de mouvements dédié**. Tant qu'il n'existe pas, valider un
//! inventaire ne change pas le stock du produit. C'est écrit noir sur blanc ici
//! pour que personne ne prenne ce silence pour un oubli.
//!
//! # Ce que ce module ne fait pas, et pourquoi
//!
//! - **Aucune valorisation.** `pmp_real` et `pmp_expected` restent vides : le
//!   prix moyen pondéré se calcule à partir des mouvements, qui n'existent pas
//!   ici. Un PMP inventé serait un chiffre faux de plus.
//! - **Aucun utilisateur.** `fk_user_creat` et `fk_user_valid` ne sont pas
//!   posés : ce noyau n'a pas encore de session utilisateur, et inscrire un
//!   identifiant inventé dans une pièce d'audit serait pire que de n'en mettre
//!   aucun.
//! - **Aucune catégorie de produits.** `categories_product` reste `NULL`, ce qui
//!   veut dire « tous les produits » : c'est le cas de très loin le plus
//!   fréquent, et un inventaire partiel se décrit par ses lignes.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

/// Préfixe des références.
///
/// Dolibarr numérote `IN` + AAMM + 4 chiffres via son module de numérotation,
/// qui n'est pas porté : `divers::suivant_ref` tire donc `IN0001`, `IN0002`…
/// dans l'ordre d'insertion.
///
/// Sur une base déjà numérotée par Dolibarr, `suivant_ref` lit le plus grand
/// nombre trouvé après le préfixe — sur `IN2611-0001` il ne lit que `2611` — et
/// la suite repart de là : les numéros peuvent donc sauter, mais aucun doublon
/// n'est produit.
pub const PREFIX: &str = "IN";

/// Brouillon : modifiable, supprimable, sans effet sur le stock.
pub const STATUT_BROUILLON: i16 = 0;
/// Validé : le comptage est figé, la pièce ne bouge plus.
pub const STATUT_VALIDE: i16 = 1;

/// Longueur maximale du titre, en **octets**.
///
/// La colonne est un `varchar(255)`, qui compte des **caractères** en utf8mb4 :
/// compter les octets est donc plus strict que la base. C'est voulu. En mode
/// non strict, MySQL tronque silencieusement un titre trop long — on
/// enregistrerait « Inventaire annuel du dépôt de Charly — zone » sans le
/// savoir, et la pièce ne se retrouverait plus par son nom. Mieux vaut refuser
/// et laisser l'appelant raccourcir.
pub const TITRE_MAX_OCTETS: usize = 255;

/// Longueur maximale d'un numéro de lot, en octets (`batch varchar(128)`).
pub const LOT_MAX_OCTETS: usize = 128;

/// Un inventaire, avec le nombre de ses lignes et l'écart total.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inventaire {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    /// Le titre est obligatoire : c'est la seule façon de retrouver un
    /// inventaire dans une liste où toutes les références se ressemblent.
    pub title: String,
    /// Entrepôt visé par l'en-tête, quand l'inventaire en couvre un seul.
    pub fk_warehouse: Option<i64>,
    /// Produit visé par l'en-tête, quand l'inventaire en couvre un seul.
    pub fk_product: Option<i64>,
    pub status: i16,
    pub date_creation: Option<String>,
    /// Date du comptage, distincte de la date de saisie.
    pub date_inventory: Option<String>,
    /// Date de validation : c'est elle qui fait foi pour savoir quand le
    /// comptage a été arrêté, pas `date_creation`.
    pub date_validation: Option<String>,
    pub nb_lignes: i64,
    /// Somme des `qty_regulated` des lignes : **l'écart constaté**, pas le stock.
    /// Relue par sous-requête, comme le total d'un bon de réception, pour que la
    /// liste s'affiche sans une requête par ligne.
    pub ecart_total: Decimal,
}

impl Inventaire {
    pub fn est_brouillon(&self) -> bool {
        self.status == STATUT_BROUILLON
    }

    pub fn est_valide(&self) -> bool {
        self.status == STATUT_VALIDE
    }
}

/// Une ligne d'inventaire : ce que le système croyait, ce qui a été compté, et
/// l'écart entre les deux.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LigneInventaire {
    pub rowid: i64,
    pub fk_inventory: i64,
    pub fk_warehouse: i64,
    pub fk_product: i64,
    /// Numéro de lot / de série, quand le produit est suivi.
    pub batch: Option<String>,
    /// Quantité que le système annonçait au moment du comptage.
    pub qty_stock: Decimal,
    /// Quantité réellement comptée. La colonne s'appelle `qty_view` en base ;
    /// le champ dit `qty_viewed` parce que c'est le participe qu'on lit dans le
    /// code appelant.
    pub qty_viewed: Decimal,
    /// `qty_viewed - qty_stock` : positif si le magasin en a trouvé **plus** que
    /// le système, négatif s'il en manque.
    pub qty_regulated: Decimal,
    /// Prix moyen pondéré réel. Toujours vide : voir l'avertissement du module,
    /// la valorisation suppose les mouvements de stock.
    pub pmp_real: Option<Decimal>,
    /// Prix moyen pondéré attendu. Toujours vide, même raison.
    pub pmp_expected: Option<Decimal>,
}

/// Ce qu'il faut pour créer un inventaire.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewInventaire {
    pub title: String,
    /// Entrepôt de l'en-tête : facultatif, un inventaire peut couvrir plusieurs
    /// entrepôts — ce sont alors les lignes qui portent le leur.
    pub fk_warehouse: Option<i64>,
    /// Produit de l'en-tête, facultatif pour la même raison.
    pub fk_product: Option<i64>,
    /// Date du comptage (`AAAA-MM-JJ`). `None` = la date du jour, telle que la
    /// voit le serveur de base : c'est lui qui date la pièce, pas le poste qui
    /// la saisit, dont l'horloge peut être fausse.
    pub date_inventory: Option<String>,
    /// Les lignes, dans l'ordre du vecteur : la table n'a pas de colonne de rang,
    /// c'est l'ordre d'insertion qui fixe l'ordre de lecture.
    pub lines: Vec<NewLigneInventaire>,
}

impl NewInventaire {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            ..Default::default()
        }
    }
}

/// Une ligne à créer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewLigneInventaire {
    pub fk_warehouse: i64,
    pub fk_product: i64,
    pub batch: Option<String>,
    pub qty_stock: Decimal,
    pub qty_viewed: Decimal,
}

impl NewLigneInventaire {
    /// Une ligne nue : l'écart, lui, ne se saisit pas, il se calcule.
    pub fn new(
        fk_warehouse: i64,
        fk_product: i64,
        qty_stock: Decimal,
        qty_viewed: Decimal,
    ) -> Self {
        Self {
            fk_warehouse,
            fk_product,
            batch: None,
            qty_stock,
            qty_viewed,
        }
    }

    /// Le même avec un numéro de lot.
    pub fn avec_lot(mut self, lot: &str) -> Self {
        self.batch = Some(lot.to_string());
        self
    }
}

/// Colonnes relues, dans l'ordre attendu par `map_row`.
///
/// Le nombre de lignes et l'écart total sont des sous-requêtes plutôt qu'un
/// `GROUP BY` : un inventaire sans ligne doit apparaître (écart à zéro) et non
/// disparaître de la liste, ce qu'un `JOIN` agrégeant ferait.
const SELECT_COLS: &str = "i.rowid, i.entity, i.ref, i.title, i.fk_warehouse, i.fk_product, \
     i.status, DATE_FORMAT(i.date_creation, '%Y-%m-%d'), \
     DATE_FORMAT(i.date_inventory, '%Y-%m-%d'), \
     DATE_FORMAT(i.date_validation, '%Y-%m-%d'), \
     (SELECT COUNT(*) FROM llx_inventorydet d WHERE d.fk_inventory = i.rowid), \
     (SELECT CAST(COALESCE(SUM(d.qty_regulated), 0) AS CHAR) FROM llx_inventorydet d \
      WHERE d.fk_inventory = i.rowid)";

const FROM: &str = "FROM llx_inventory i";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

/// Les colonnes `float` et `double` ne se décodent pas en `Decimal` : tout
/// passe par `CAST(... AS CHAR)`, seule façon d'éviter un arrondi binaire sur
/// une quantité.
fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Inventaire {
    Inventaire {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        title: opt_s(&row, 3).unwrap_or_default(),
        fk_warehouse: opt_i(&row, 4),
        fk_product: opt_i(&row, 5),
        status: opt_i(&row, 6).unwrap_or(0) as i16,
        date_creation: opt_s(&row, 7),
        date_inventory: opt_s(&row, 8),
        date_validation: opt_s(&row, 9),
        nb_lignes: opt_i(&row, 10).unwrap_or(0),
        // La somme sort d'une colonne `double` : elle traîne des miettes
        // binaires (2,0000000000000004) qu'on ramène à la précision des
        // quantités avant de l'afficher.
        ecart_total: arrondi_quantite(opt_dec(&row, 11).unwrap_or_default()),
    }
}

/// Inventaires de l'entité, du plus récent au plus ancien.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Inventaire>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} {FROM} WHERE i.entity = ? \
         ORDER BY i.date_creation DESC, i.rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limite, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Un inventaire précis, ou `None` s'il n'existe pas dans cette entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Inventaire>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} {FROM} WHERE i.entity = ? AND i.rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// Lignes d'un inventaire, dans l'ordre de saisie.
///
/// La table n'a pas de colonne de rang : `rowid` fait foi, c'est-à-dire l'ordre
/// du vecteur à la création. La jointure sur l'en-tête n'est pas décorative non
/// plus : les lignes n'ont pas de colonne `entity` et ne peuvent donc pas être
/// cloisonnées autrement.
pub fn lines(
    pool: &Pool,
    entity: i64,
    fk_inventory: i64,
) -> Result<Vec<LigneInventaire>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT d.rowid, d.fk_inventory, d.fk_warehouse, d.fk_product, d.batch, \
                CAST(d.qty_stock AS CHAR), CAST(d.qty_view AS CHAR), \
                CAST(d.qty_regulated AS CHAR), CAST(d.pmp_real AS CHAR), \
                CAST(d.pmp_expected AS CHAR) \
         FROM llx_inventorydet d \
         JOIN llx_inventory i ON i.rowid = d.fk_inventory \
         WHERE i.entity = :entity AND d.fk_inventory = :id \
         ORDER BY d.rowid ASC",
        params! { "entity" => entity, "id" => fk_inventory },
    )?;
    Ok(rows
        .iter()
        .map(|r| LigneInventaire {
            rowid: opt_i(r, 0).unwrap_or(0),
            fk_inventory: opt_i(r, 1).unwrap_or(0),
            fk_warehouse: opt_i(r, 2).unwrap_or(0),
            fk_product: opt_i(r, 3).unwrap_or(0),
            batch: opt_s(r, 4),
            qty_stock: arrondi_quantite(opt_dec(r, 5).unwrap_or_default()),
            qty_viewed: arrondi_quantite(opt_dec(r, 6).unwrap_or_default()),
            qty_regulated: arrondi_quantite(opt_dec(r, 7).unwrap_or_default()),
            pmp_real: opt_dec(r, 8),
            pmp_expected: opt_dec(r, 9),
        })
        .collect())
}

/// Crée un inventaire en brouillon et renvoie son `rowid`.
///
/// La création est tout ou rien : l'en-tête et ses lignes vivent dans la même
/// transaction. Un inventaire sans ligne n'est pas une pièce, seulement un
/// numéro consommé — d'où le refus, contrôlé avant toute écriture.
///
/// `qty_regulated` est calculé ici, jamais saisi : `qty_viewed - qty_stock`.
/// C'est le seul chiffre qui décide de la suite, et il ne doit pas pouvoir
/// contredire les deux colonnes dont il dérive.
///
/// Aucune écriture de stock : voir l'avertissement en tête de module.
pub fn create(pool: &Pool, entity: i64, new: &NewInventaire) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;

    let mut conn = pool.get_conn()?;
    // Le verrou nommé est porté par le serveur, pas par la connexion : il
    // sérialise la numérotation même si `suivant_ref` tire la référence sur une
    // autre connexion du pool (le pool par défaut en offre dix ; il en faut au
    // moins deux, sinon l'appel attendrait indéfiniment). Le libellé porte
    // l'entité, comme ailleurs dans le noyau.
    let verrou = format!("dolibarr_rs_inventaire_{entity}");
    let pris: Option<i64> = conn.exec_first("SELECT GET_LOCK(?, 10)", (verrou.as_str(),))?;
    if pris != Some(1) {
        return Err("verrou de numérotation indisponible (10 s)".into());
    }

    let resultat = creer_sous_verrou(&mut conn, pool, entity, new);
    let _: Option<i64> = conn.exec_first("SELECT RELEASE_LOCK(?)", (verrou.as_str(),))?;
    resultat
}

fn creer_sous_verrou(
    conn: &mut mysql::PooledConn,
    pool: &Pool,
    entity: i64,
    new: &NewInventaire,
) -> Result<i64, Box<dyn Error>> {
    // L'en-tête peut viser un entrepôt et un produit : ils doivent exister, et
    // ce contrôle passe par la base, pas par la liste déroulante de l'écran qui
    // peut être périmée.
    verifier_entrepot_entete(conn, entity, new.fk_warehouse)?;
    verifier_produit_entete(conn, entity, new.fk_product)?;
    for (index, ligne) in new.lines.iter().enumerate() {
        let numero = index + 1;
        if !entrepot_existe(conn, entity, ligne.fk_warehouse)? {
            return Err(format!(
                "ligne {numero} : l'entrepôt {} n'existe pas dans l'entité {entity}",
                ligne.fk_warehouse
            )
            .into());
        }
        if !produit_existe(conn, entity, ligne.fk_product)? {
            return Err(format!(
                "ligne {numero} : le produit {} n'existe pas dans l'entité {entity}",
                ligne.fk_product
            )
            .into());
        }
    }

    let reference = crate::divers::suivant_ref(pool, "llx_inventory", "ref", PREFIX, entity)?;
    let date_comptage = texte_utile(new.date_inventory.as_deref());

    let mut tx = conn.start_transaction(TxOpts::default())?;
    let insere = tx.exec_drop(
        "INSERT INTO llx_inventory \
         (entity, ref, date_creation, fk_warehouse, fk_product, status, title, \
          date_inventory) \
         VALUES (:entity, :ref, NOW(), :fk_warehouse, :fk_product, :statut, :title, \
          COALESCE(CAST(:date_inventory AS DATETIME), NOW()))",
        params! {
            "entity" => entity,
            "ref" => reference.as_str(),
            "fk_warehouse" => new.fk_warehouse,
            "fk_product" => new.fk_product,
            "statut" => STATUT_BROUILLON,
            "title" => new.title.trim(),
            "date_inventory" => date_comptage,
        },
    );
    if let Err(e) = insere {
        tx.rollback()?;
        // 1062 : `ref` déjà prise. Le verrou rend le cas improbable ; s'il
        // survient, c'est qu'un autre écrivain (l'interface Dolibarr) ne le
        // prend pas — relancer la création suffit, on ne boucle donc pas ici.
        if is_duplicate(&e) {
            return Err(format!(
                "la référence {reference} vient d'être prise par un autre écrivain : relancer la création"
            )
            .into());
        }
        return Err(e.into());
    }
    let rowid = tx.last_insert_id().unwrap_or(0) as i64;

    for ligne in &new.lines {
        let lot = texte_utile(ligne.batch.as_deref());
        let stock = arrondi_quantite(ligne.qty_stock);
        let vue = arrondi_quantite(ligne.qty_viewed);
        // L'écart est calculé sur les valeurs déjà arrondies : ainsi
        // `qty_viewed - qty_stock == qty_regulated` reste vrai à la relecture,
        // au lieu de dépendre de la précision que MySQL a bien voulu garder.
        let ecart = arrondi_quantite(vue - stock);
        tx.exec_drop(
            "INSERT INTO llx_inventorydet \
             (datec, fk_inventory, fk_warehouse, fk_product, batch, qty_stock, \
              qty_view, qty_regulated) \
             VALUES (NOW(), :fk_inventory, :fk_warehouse, :fk_product, :batch, \
              :qty_stock, :qty_view, :qty_regulated)",
            params! {
                "fk_inventory" => rowid,
                "fk_warehouse" => ligne.fk_warehouse,
                "fk_product" => ligne.fk_product,
                "batch" => lot,
                // Les quantités passent en texte : les colonnes sont des
                // `double`, et un `f64` en arithmétique est précisément ce que
                // ce noyau refuse.
                "qty_stock" => stock.to_string(),
                "qty_view" => vue.to_string(),
                "qty_regulated" => ecart.to_string(),
            },
        )?;
    }
    tx.commit()?;
    Ok(rowid)
}

/// Contrôles de forme, faits avant d'ouvrir la moindre connexion : une erreur
/// de saisie ne doit pas coûter un aller-retour SQL.
fn verifier_entree(new: &NewInventaire) -> Result<(), Box<dyn Error>> {
    let titre = new.title.trim();
    if titre.is_empty() {
        return Err(
            "un inventaire sans titre ne se crée pas : c'est son titre qui le distingue des autres"
                .into(),
        );
    }
    let octets = titre.len();
    if octets > TITRE_MAX_OCTETS {
        return Err(format!(
            "titre trop long : {octets} octets pour {TITRE_MAX_OCTETS} au maximum (la base \
             tronquerait le reste sans le dire)"
        )
        .into());
    }
    if new.lines.is_empty() {
        return Err("un inventaire sans ligne ne se crée pas : il ne constate aucun écart".into());
    }
    let date_fausse = texte_utile(new.date_inventory.as_deref()).filter(|d| !date_valide(d));
    if let Some(date) = date_fausse {
        return Err(
            format!("date d'inventaire « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }

    // L'index unique `uk_inventorydet(fk_inventory, fk_warehouse, fk_product,
    // batch)` interdit deux lignes identiques. On le vérifie ici pour rendre un
    // message qui nomme la ligne fautive, plutôt que de laisser remonter un 1062
    // au milieu de la transaction.
    let mut vues: BTreeSet<(i64, i64, Option<&str>)> = BTreeSet::new();
    for (index, ligne) in new.lines.iter().enumerate() {
        let numero = index + 1;
        if ligne.qty_stock < Decimal::ZERO {
            return Err(format!(
                "ligne {numero} : la quantité en stock ne peut pas être négative ({})",
                ligne.qty_stock
            )
            .into());
        }
        if ligne.qty_viewed < Decimal::ZERO {
            return Err(format!(
                "ligne {numero} : la quantité comptée ne peut pas être négative ({})",
                ligne.qty_viewed
            )
            .into());
        }
        if ligne.fk_warehouse <= 0 {
            return Err(format!("ligne {numero} : l'entrepôt est obligatoire").into());
        }
        if ligne.fk_product <= 0 {
            return Err(format!("ligne {numero} : le produit est obligatoire").into());
        }
        let lot = texte_utile(ligne.batch.as_deref());
        let lot_trop_long = lot.filter(|l| l.len() > LOT_MAX_OCTETS);
        if let Some(lot) = lot_trop_long {
            return Err(format!(
                "ligne {numero} : numéro de lot trop long ({} octets pour {LOT_MAX_OCTETS})",
                lot.len()
            )
            .into());
        }
        if !vues.insert((ligne.fk_warehouse, ligne.fk_product, lot)) {
            return Err(format!(
                "ligne {numero} : ce produit est déjà compté pour cet entrepôt et ce lot — \
                 une seule ligne par produit et par lot, sinon l'écart serait compté deux fois"
            )
            .into());
        }
    }
    Ok(())
}

fn produit_existe(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_product: i64,
) -> Result<bool, Box<dyn Error>> {
    let trouve: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_product WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_product },
    )?;
    Ok(trouve.is_some())
}

/// Contrôle l'entrepôt de l'en-tête, s'il y en a un.
///
/// `let Some(..) else` plutôt qu'un `if let` imbriqué dans un `if` : le noyau
/// passe `clippy -D warnings`, et deux niveaux d'imbrication pour un contrôle
/// qui tient en deux lignes se lisent mal de toute façon.
fn verifier_entrepot_entete(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_warehouse: Option<i64>,
) -> Result<(), Box<dyn Error>> {
    let Some(entrepot) = fk_warehouse else {
        return Ok(());
    };
    if !entrepot_existe(conn, entity, entrepot)? {
        return Err(format!("l'entrepôt {entrepot} n'existe pas dans l'entité {entity}").into());
    }
    Ok(())
}

/// Contrôle le produit de l'en-tête, s'il y en a un.
fn verifier_produit_entete(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_product: Option<i64>,
) -> Result<(), Box<dyn Error>> {
    let Some(produit) = fk_product else {
        return Ok(());
    };
    if !produit_existe(conn, entity, produit)? {
        return Err(format!("le produit {produit} n'existe pas dans l'entité {entity}").into());
    }
    Ok(())
}

fn entrepot_existe(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_entrepot: i64,
) -> Result<bool, Box<dyn Error>> {
    let trouve: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_entrepot WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_entrepot },
    )?;
    Ok(trouve.is_some())
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

/// Valide un brouillon : `status` passe de 0 à 1, `date_validation` est posée.
///
/// Valider deux fois n'est pas une opération idempotente : c'est un appelant
/// qui a perdu le fil de son écran. Sur un inventaire, le laisser passer
/// reviendrait à accepter deux constats du même comptage à deux dates
/// différentes — et, le jour où les mouvements existeront, deux
/// régularisations pour un seul écart.
///
/// Comme annoncé en tête de module, cette fonction **n'écrit pas** dans
/// `llx_product_stock` ni dans `llx_stock_mouvement` : elle fige le constat,
/// elle ne le comptabilise pas.
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_inventory WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("l'inventaire {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s == STATUT_VALIDE as i64 => return Err("cet inventaire est déjà validé".into()),
        Some(s) if s != STATUT_BROUILLON as i64 => {
            return Err(format!(
                "cet inventaire a un statut inattendu ({s}) : seul un brouillon se valide"
            )
            .into())
        }
        _ => {}
    }

    let lignes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_inventorydet WHERE fk_inventory = :rowid",
        params! { "rowid" => rowid },
    )?;
    if lignes.unwrap_or(0) == 0 {
        return Err("un inventaire sans ligne ne se valide pas : il ne constate rien".into());
    }

    conn.exec_drop(
        "UPDATE llx_inventory SET status = :statut, date_validation = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Supprime un brouillon et ses lignes.
///
/// Un inventaire validé a déjà servi : il atteste qu'un comptage a eu lieu à
/// une date, et c'est souvent la seule trace d'un écart contesté. Le supprimer
/// effacerait la preuve. La suppression physique est donc réservée au
/// brouillon.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_inventory WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("l'inventaire {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s != STATUT_BROUILLON as i64 => {
            return Err("un inventaire validé ne se supprime pas : il atteste d'un comptage".into())
        }
        _ => {}
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    // Les lignes partent avant l'en-tête : le schéma Dolibarr ne pose pas de clé
    // étrangère, un reste de ligne ne serait donc jamais signalé — et il
    // fausserait le prochain inventaire par son écart orphelin.
    tx.exec_drop(
        "DELETE FROM llx_inventorydet WHERE fk_inventory = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_inventory WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Écart total d'un inventaire : somme des `qty_regulated` de ses lignes.
///
/// C'est l'écart **constaté**, pas le stock : +2 veut dire que le magasin a
/// trouvé deux unités de plus que le système, pas qu'il y en a deux. Un
/// inventaire dont toutes les lignes sont justes rend `0`, un inventaire sans
/// ligne aussi — d'où le refus explicite d'un identifiant inconnu, qui sinon se
/// confondrait avec un inventaire parfaitement juste.
///
/// La somme est arrondie à la précision des quantités : les colonnes sont des
/// `double`, et une somme de dix lignes traîne sinon des miettes binaires
/// (1,9999999999999998) que personne ne veut relire.
pub fn ecarts(pool: &Pool, entity: i64, fk_inventory: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_inventory WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_inventory },
    )?;
    if existe.is_none() {
        return Err(
            format!("l'inventaire {fk_inventory} n'existe pas dans l'entité {entity}").into(),
        );
    }

    let somme: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(d.qty_regulated), 0) AS CHAR) \
         FROM llx_inventorydet d \
         JOIN llx_inventory i ON i.rowid = d.fk_inventory \
         WHERE i.entity = :entity AND d.fk_inventory = :id",
        params! { "entity" => entity, "id" => fk_inventory },
    )?;
    let somme = somme
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .unwrap_or_default();
    Ok(arrondi_quantite(somme))
}

/// Arrondi des quantités, à la précision des colonnes `double` de Dolibarr.
///
/// On réutilise `money::round_unit_price` (6 décimales) et **surtout pas**
/// `money::round_money` (2 décimales) : un montant s'arrête au centime, une
/// quantité non. Trois dixièmes de mètre cube de sable, cinq centièmes d'hectare
/// — arrondir une quantité au centième corromprait l'écart d'inventaire,
/// précisément le chiffre que la pièce existe pour établir.
fn arrondi_quantite(quantite: Decimal) -> Decimal {
    money::round_unit_price(quantite)
}

/// Un texte facultatif ramené à `None` s'il est vide : une chaîne blanche
/// stockée en base se relit comme une valeur, pas comme une absence.
fn texte_utile(valeur: Option<&str>) -> Option<&str> {
    valeur.map(str::trim).filter(|t| !t.is_empty())
}

/// Une date `AAAA-MM-JJ` réelle : `2026-02-31` existe au format mais pas au
/// calendrier, et MySQL le refuserait au milieu de l'insertion.
///
/// Cette fonction existe déjà, à l'identique, dans `reception`, `expedition`,
/// `adherent`, `commande_fournisseur`, `proposition_fournisseur` et
/// `persist_ecritures` : c'est la convention du noyau, chaque module garde sa
/// copie plutôt que de dépendre d'un utilitaire commun qui grossirait sans
/// qu'on sache plus qui l'utilise.
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
