//! Réceptions (entrées de marchandise fournisseur) : `llx_reception` +
//! `llx_receptiondet_batch`.
//!
//! Une réception, c'est la marchandise qui **entre** : une référence, un
//! fournisseur, des lignes de quantités et un statut qui suit la pièce
//! (brouillon, validée, réceptionnée). C'est le frère jumeau de l'expédition,
//! côté entrant : mêmes statuts, même cycle irréversible, aucun montant — ce
//! qui se facture reste la commande fournisseur ou la facture fournisseur,
//! jamais le bon de réception.
//!
//! Les statuts sont ceux de Dolibarr : `0` brouillon, `1` validée, `2`
//! réceptionnée. Le cycle est irréversible : une pièce validée ne redevient pas
//! un brouillon, une pièce réceptionnée ne se « dé-réceptionne » pas. Ce module
//! n'expose donc aucune transition inverse : une erreur de saisie se corrige en
//! supprimant le brouillon, pas en remontant le statut.
//!
//! # Le stock n'est pas écrit ici, et c'est un manque assumé
//!
//! Dans Dolibarr, réceptionner de la marchandise **augmente** le stock : une
//! ligne dans `llx_stock_mouvement` et un nouveau `reel` dans
//! `llx_product_stock`, les deux dans la même transaction. Ce module n'écrit
//! dans **aucune** des deux tables. Le choix est délibéré :
//!
//! - écrire un niveau de stock sans écrire le mouvement qui le justifie casse
//!   la règle du noyau — « le stock affiché est toujours la somme des
//!   mouvements, jamais une valeur saisie à part » — et laisse des quantités
//!   fantômes que personne ne peut expliquer, encore moins annuler ;
//! - `stock::mouvement` sait déjà faire les deux ensemble, mais il exige un
//!   entrepôt et un produit résolus, une unité, et surtout une **annulation
//!   symétrique** : ce que ce module ne sait pas encore décider (une réception
//!   supprimée doit-elle décrémenter ? une ligne sans entrepôt doit-elle être
//!   refusée ? que faire d'une réception partielle ?). Le porter à moitié
//!   créerait exactement les fantômes qu'on veut éviter.
//!
//! Donc : **la réception est tracée, le mouvement de stock viendra avec le
//! module de mouvements dédié**. Tant qu'il n'existe pas, une réception
//! réceptionnée ne change pas le stock du produit. C'est écrit noir sur blanc
//! ici pour que personne ne prenne ce silence pour un oubli.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

/// Préfixe des références.
///
/// Dolibarr numérote `RE` + AAMM + 4 chiffres via son module de numérotation,
/// qui n'est pas porté : `divers::suivant_ref` tire donc `RE0001`, `RE0002`…
/// dans l'ordre d'insertion.
///
/// Sur une base déjà numérotée par Dolibarr, `suivant_ref` lit le plus grand
/// nombre trouvé après le préfixe — sur `RE2611-0001` il ne lit que `2611` — et
/// la suite repart de là : les numéros peuvent donc sauter, mais aucun doublon
/// n'est produit.
pub const PREFIX: &str = "RE";

/// Brouillon : modifiable, supprimable, sans effet sur le stock.
pub const STATUT_BROUILLON: i16 = 0;
/// Validée : la marchandise est annoncée par le fournisseur, la pièce est figée.
pub const STATUT_VALIDEE: i16 = 1;
/// Réceptionnée : la marchandise est physiquement entrée, la pièce ne bouge plus.
pub const STATUT_RECEPTIONNEE: i16 = 2;

/// `element_type` d'une ligne.
///
/// Dolibarr n'admet que deux valeurs : `commande_fournisseur` quand la ligne
/// est reprise de la commande d'origine (le lien est alors porté par
/// `fk_element` / `fk_elementdet`), `reception` quand la ligne est saisie sur
/// le bon de réception. Ce module ne crée que des lignes saisies : il n'a pas
/// la commande fournisseur sous la main.
const ELEMENT_TYPE: &str = "reception";

/// Statut d'une ligne, écrit à `1` comme le fait Dolibarr.
///
/// `0` sert à annuler une ligne sans la supprimer (elle reste alors dans
/// l'historique) : ce module ne s'en sert jamais, il supprime le brouillon
/// entier plutôt que d'y laisser des lignes mortes.
const STATUT_LIGNE: i32 = 1;

/// Une réception, avec le nom du fournisseur et le total de ses lignes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reception {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub fk_soc: i64,
    /// Raison sociale, relue par jointure : la liste s'affiche sans seconde
    /// requête par ligne.
    pub tiers: Option<String>,
    pub fk_projet: Option<i64>,
    /// Référence du bon de livraison chez le fournisseur.
    pub ref_supplier: Option<String>,
    /// Référence libre de l'appelant, pour raccrocher la pièce à un système
    /// extérieur sans toucher à la numérotation Dolibarr.
    pub ref_ext: Option<String>,
    pub date_creation: Option<String>,
    pub date_valid: Option<String>,
    /// Date de livraison **prévue** — distincte de la date d'entrée réelle.
    pub date_livraison_prevue: Option<String>,
    /// Date d'entrée constatée, posée par `receptionner`.
    pub date_reception: Option<String>,
    pub tracking_number: Option<String>,
    pub fk_statut: i16,
    pub facturee: bool,
    pub nb_lignes: i64,
    /// Somme des quantités des lignes : c'est le seul « total » d'un bon de
    /// réception, faute de montants.
    pub quantite_totale: Decimal,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
}

impl Reception {
    pub fn est_brouillon(&self) -> bool {
        self.fk_statut == STATUT_BROUILLON
    }

    pub fn est_validee(&self) -> bool {
        self.fk_statut == STATUT_VALIDEE
    }

    pub fn est_receptionnee(&self) -> bool {
        self.fk_statut == STATUT_RECEPTIONNEE
    }
}

/// Une ligne de bon de réception.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReceptionLine {
    pub rowid: i64,
    pub fk_reception: i64,
    pub fk_product: Option<i64>,
    pub description: Option<String>,
    pub qty: Decimal,
    pub fk_unit: Option<i64>,
    /// Entrepôt visé. Il est conservé même si le stock n'est pas écrit ici :
    /// c'est la donnée dont le futur module de mouvements aura besoin.
    pub fk_entrepot: Option<i64>,
    pub fk_projet: Option<i64>,
    /// Numéro de lot / de série.
    pub batch: Option<String>,
    /// Date de durabilité minimale (à consommer de préférence avant).
    pub eatby: Option<String>,
    /// Date limite de consommation.
    pub sellby: Option<String>,
    /// Coût d'achat unitaire de la ligne, pour valoriser l'entrée.
    pub cost_price: Option<Decimal>,
    pub rang: i32,
}

/// Ce qu'il faut pour créer un bon de réception.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewReception {
    pub fk_soc: i64,
    pub fk_projet: Option<i64>,
    pub ref_supplier: Option<String>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub lines: Vec<NewReceptionLine>,
}

impl NewReception {
    pub fn new(fk_soc: i64) -> Self {
        Self {
            fk_soc,
            ..Default::default()
        }
    }
}

/// Une ligne à créer. Elle n'a pas de rang : l'ordre du vecteur fait foi.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewReceptionLine {
    pub fk_product: Option<i64>,
    pub description: String,
    pub qty: Decimal,
    pub fk_unit: Option<i64>,
    pub fk_entrepot: Option<i64>,
    pub batch: Option<String>,
    pub eatby: Option<String>,
    pub sellby: Option<String>,
    pub cost_price: Option<Decimal>,
}

impl NewReceptionLine {
    pub fn new(description: &str, qty: Decimal) -> Self {
        Self {
            description: description.to_string(),
            qty,
            ..Default::default()
        }
    }
}

/// Colonnes relues, dans l'ordre attendu par `map_row`.
///
/// Le nombre de lignes et la quantité totale sont des sous-requêtes plutôt
/// qu'un `GROUP BY` : une pièce sans ligne doit apparaître (total à zéro) et
/// non disparaître de la liste, ce qu'un `JOIN` agrégeant ferait.
const SELECT_COLS: &str = "r.rowid, r.entity, r.ref, r.fk_soc, s.nom, r.fk_projet, \
     r.ref_supplier, r.ref_ext, DATE_FORMAT(r.date_creation, '%Y-%m-%d'), \
     DATE_FORMAT(r.date_valid, '%Y-%m-%d'), \
     DATE_FORMAT(r.date_delivery, '%Y-%m-%d'), \
     DATE_FORMAT(r.date_reception, '%Y-%m-%d'), r.tracking_number, r.fk_statut, \
     r.billed, \
     (SELECT COUNT(*) FROM llx_receptiondet_batch d WHERE d.fk_reception = r.rowid), \
     (SELECT CAST(COALESCE(SUM(d.qty), 0) AS CHAR) FROM llx_receptiondet_batch d \
      WHERE d.fk_reception = r.rowid), \
     r.note_private, r.note_public";

const FROM: &str = "FROM llx_reception r LEFT JOIN llx_societe s ON s.rowid = r.fk_soc";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

/// Les colonnes `float` et `double` ne se décodent pas en `Decimal` : tout
/// passe par `CAST(... AS CHAR)`, seule façon d'éviter un arrondi binaire sur
/// une quantité ou un coût.
fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Reception {
    Reception {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        fk_soc: opt_i(&row, 3).unwrap_or(0),
        tiers: opt_s(&row, 4),
        fk_projet: opt_i(&row, 5),
        ref_supplier: opt_s(&row, 6),
        ref_ext: opt_s(&row, 7),
        date_creation: opt_s(&row, 8),
        date_valid: opt_s(&row, 9),
        date_livraison_prevue: opt_s(&row, 10),
        date_reception: opt_s(&row, 11),
        tracking_number: opt_s(&row, 12),
        fk_statut: opt_i(&row, 13).unwrap_or(0) as i16,
        facturee: opt_i(&row, 14).unwrap_or(0) != 0,
        nb_lignes: opt_i(&row, 15).unwrap_or(0),
        quantite_totale: opt_dec(&row, 16).unwrap_or_default(),
        note_private: opt_s(&row, 17),
        note_public: opt_s(&row, 18),
    }
}

/// Réceptions de l'entité, de la plus récente à la plus ancienne.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Reception>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} {FROM} WHERE r.entity = ? \
         ORDER BY r.date_creation DESC, r.rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limite, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Une réception précise, ou `None` si elle n'existe pas dans cette entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Reception>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} {FROM} WHERE r.entity = ? AND r.rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// Lignes d'une réception, dans l'ordre de saisie.
///
/// La jointure sur l'en-tête n'est pas décorative : les lignes n'ont pas de
/// colonne `entity` et ne peuvent donc pas être cloisonnées autrement.
pub fn lines(
    pool: &Pool,
    entity: i64,
    fk_reception: i64,
) -> Result<Vec<ReceptionLine>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT d.rowid, d.fk_reception, d.fk_product, d.description, \
                CAST(d.qty AS CHAR), d.fk_unit, d.fk_entrepot, d.fk_projet, \
                d.batch, DATE_FORMAT(d.eatby, '%Y-%m-%d'), \
                DATE_FORMAT(d.sellby, '%Y-%m-%d'), CAST(d.cost_price AS CHAR), d.rang \
         FROM llx_receptiondet_batch d \
         JOIN llx_reception r ON r.rowid = d.fk_reception \
         WHERE r.entity = :entity AND d.fk_reception = :id \
         ORDER BY d.rang ASC, d.rowid ASC",
        params! { "entity" => entity, "id" => fk_reception },
    )?;
    Ok(rows
        .iter()
        .map(|r| ReceptionLine {
            rowid: opt_i(r, 0).unwrap_or(0),
            fk_reception: opt_i(r, 1).unwrap_or(0),
            fk_product: opt_i(r, 2),
            description: opt_s(r, 3),
            qty: opt_dec(r, 4).unwrap_or_default(),
            fk_unit: opt_i(r, 5),
            fk_entrepot: opt_i(r, 6),
            fk_projet: opt_i(r, 7),
            batch: opt_s(r, 8),
            eatby: opt_s(r, 9),
            sellby: opt_s(r, 10),
            cost_price: opt_dec(r, 11),
            rang: opt_i(r, 12).unwrap_or(0) as i32,
        })
        .collect())
}

/// Crée un bon de réception en brouillon et renvoie son `rowid`.
///
/// La création est tout ou rien : l'en-tête et ses lignes vivent dans la même
/// transaction. Un bon sans ligne n'est pas une pièce, seulement un numéro
/// consommé — d'où le refus, contrôlé avant toute écriture.
///
/// Aucune écriture de stock : voir l'avertissement en tête de module.
pub fn create(pool: &Pool, entity: i64, new: &NewReception) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;

    let mut conn = pool.get_conn()?;
    // Le verrou nommé est porté par le serveur, pas par la connexion : il
    // sérialise la numérotation même si `suivant_ref` tire la référence sur une
    // autre connexion du pool (le pool par défaut en offre dix ; il en faut au
    // moins deux, sinon l'appel attendrait indéfiniment). Le libellé porte
    // l'entité, comme ailleurs dans le noyau.
    let verrou = format!("dolibarr_rs_reception_{entity}");
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
    new: &NewReception,
) -> Result<i64, Box<dyn Error>> {
    verifier_tiers(conn, entity, new.fk_soc)?;
    if let Some(projet) = new.fk_projet {
        verifier_projet(conn, entity, projet)?;
    }

    let reference = crate::divers::suivant_ref(pool, "llx_reception", "ref", PREFIX, entity)?;

    let mut tx = conn.start_transaction(TxOpts::default())?;
    let insere = tx.exec_drop(
        "INSERT INTO llx_reception \
         (entity, ref, fk_soc, fk_projet, ref_supplier, date_creation, fk_statut, \
          billed, note_private, note_public) \
         VALUES (:entity, :ref, :fk_soc, :fk_projet, :ref_supplier, NOW(), :statut, \
          0, :note_private, :note_public)",
        params! {
            "entity" => entity,
            "ref" => reference.as_str(),
            "fk_soc" => new.fk_soc,
            "fk_projet" => new.fk_projet,
            "ref_supplier" => new.ref_supplier.as_deref(),
            "statut" => STATUT_BROUILLON,
            "note_private" => new.note_private.as_deref(),
            "note_public" => new.note_public.as_deref(),
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

    // Le rang suit l'ordre du vecteur : c'est lui qui fixe l'ordre d'impression
    // du bon de réception, et il commence à 0 comme dans Dolibarr.
    for (rang, ligne) in new.lines.iter().enumerate() {
        let lot = texte_utile(ligne.batch.as_deref());
        let a_consommer = date_facultative(ligne.eatby.as_deref());
        let limite = date_facultative(ligne.sellby.as_deref());
        // Le coût passe en texte : la colonne est un `double`, et un `f64` en
        // arithmétique est précisément ce que ce noyau refuse.
        let cout = ligne.cost_price.map(|c| c.to_string());
        tx.exec_drop(
            "INSERT INTO llx_receptiondet_batch \
             (fk_reception, element_type, fk_product, description, qty, fk_unit, \
              fk_entrepot, fk_projet, batch, eatby, sellby, status, datec, \
              cost_price, rang) \
             VALUES (:fk_reception, :element_type, :fk_product, :description, :qty, \
              :fk_unit, :fk_entrepot, :fk_projet, :batch, :eatby, :sellby, :statut, \
              NOW(), :cost_price, :rang)",
            params! {
                "fk_reception" => rowid,
                "element_type" => ELEMENT_TYPE,
                "fk_product" => ligne.fk_product,
                "description" => ligne.description.trim(),
                "qty" => ligne.qty.to_string(),
                "fk_unit" => ligne.fk_unit,
                "fk_entrepot" => ligne.fk_entrepot,
                // Le projet de la ligne est celui de l'en-tête : Dolibarr le
                // recopie de l'élément d'origine, et une ligne sans projet
                // serait invisible dans les analyses par projet alors que sa
                // pièce y figure.
                "fk_projet" => new.fk_projet,
                "batch" => lot,
                "eatby" => a_consommer,
                "sellby" => limite,
                "statut" => STATUT_LIGNE,
                "cost_price" => cout,
                "rang" => rang as i32,
            },
        )?;
    }
    tx.commit()?;
    Ok(rowid)
}

/// Contrôles de forme, faits avant d'ouvrir la moindre connexion : une erreur
/// de saisie ne doit pas coûter un aller-retour SQL.
fn verifier_entree(new: &NewReception) -> Result<(), Box<dyn Error>> {
    if new.fk_soc <= 0 {
        return Err(
            "le fournisseur (fk_soc) est obligatoire : une réception vient de quelqu'un".into(),
        );
    }
    if new.lines.is_empty() {
        return Err(
            "une réception sans ligne ne se crée pas : il faut au moins une marchandise".into(),
        );
    }
    for (index, ligne) in new.lines.iter().enumerate() {
        let numero = index + 1;
        if ligne.qty <= Decimal::ZERO {
            return Err(
                format!("ligne {numero} : la quantité doit être strictement positive").into(),
            );
        }
        // Une ligne sans texte et sans produit serait illisible sur le bon de
        // réception : le magasin ne saurait pas ce qu'il range.
        if ligne.description.trim().is_empty() && ligne.fk_product.is_none() {
            return Err(format!("ligne {numero} : il faut une description ou un produit").into());
        }
        // `eatby` et `sellby` sont des colonnes `date` : une chaîne mal formée
        // ferait échouer l'insertion en mode strict, sans dire à l'appelant ce
        // qu'il a écrit de travers.
        for (nom, valeur) in [("eatby", &ligne.eatby), ("sellby", &ligne.sellby)] {
            let fautive = date_facultative(valeur.as_deref()).filter(|d| !date_valide(d));
            if let Some(date) = fautive {
                return Err(format!(
                    "ligne {numero} : {nom} « {date} » invalide, format attendu AAAA-MM-JJ"
                )
                .into());
            }
        }
    }
    Ok(())
}

fn verifier_tiers(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_soc: i64,
) -> Result<(), Box<dyn Error>> {
    let tiers: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_soc },
    )?;
    if tiers.is_none() {
        return Err(format!("le tiers {fk_soc} n'existe pas dans l'entité {entity}").into());
    }
    Ok(())
}

fn verifier_projet(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_projet: i64,
) -> Result<(), Box<dyn Error>> {
    let projet: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_projet WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_projet },
    )?;
    if projet.is_none() {
        return Err(format!("le projet {fk_projet} n'existe pas dans l'entité {entity}").into());
    }
    Ok(())
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

/// Valide un brouillon : `fk_statut` passe de 0 à 1, `date_valid` est posée.
///
/// Valider deux fois n'est pas une opération idempotente : c'est un appelant
/// qui a perdu le fil de son écran, et le laisser passer masquerait un double
/// traitement (deux bons de réception pour une seule entrée de marchandise).
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_reception WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("la réception {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s == STATUT_VALIDEE as i64 => {
            return Err("cette réception est déjà validée".into())
        }
        Some(s) if s >= STATUT_RECEPTIONNEE as i64 => {
            return Err(
                "cette réception est déjà réceptionnée : elle n'a plus à être validée".into(),
            )
        }
        _ => {}
    }

    let lignes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_receptiondet_batch WHERE fk_reception = :rowid",
        params! { "rowid" => rowid },
    )?;
    if lignes.unwrap_or(0) == 0 {
        return Err("une réception sans ligne ne se valide pas".into());
    }

    conn.exec_drop(
        "UPDATE llx_reception SET fk_statut = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Marque une pièce validée comme réceptionnée : statut 2 et date d'entrée.
///
/// Seule une pièce validée entre : réceptionner un brouillon reviendrait à
/// constater l'arrivée d'une marchandise que personne n'a annoncée, et le futur
/// mouvement de stock ne saurait plus à quelle pièce le rattacher.
///
/// Comme annoncé en tête de module, cette fonction **n'écrit pas** dans
/// `llx_product_stock` ni dans `llx_stock_mouvement` : elle constate l'entrée,
/// elle ne la comptabilise pas.
pub fn receptionner(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    date: &str,
) -> Result<(), Box<dyn Error>> {
    let date = date.trim();
    // La colonne est un `date` : une chaîne mal formée ferait échouer la requête
    // en mode strict, sans dire à l'appelant ce qu'il a écrit de travers.
    if !date_valide(date) {
        return Err(
            format!("date de réception « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }

    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_reception WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("la réception {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s == STATUT_BROUILLON as i64 => {
            return Err(
                "cette réception n'est pas validée : la valider avant de la réceptionner".into(),
            )
        }
        Some(s) if s >= STATUT_RECEPTIONNEE as i64 => {
            return Err("cette réception est déjà réceptionnée".into())
        }
        _ => {}
    }

    conn.exec_drop(
        "UPDATE llx_reception SET fk_statut = :statut, date_reception = :date_rec \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_RECEPTIONNEE,
            "date_rec" => date,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Supprime un brouillon et ses lignes.
///
/// Un bon validé ou réceptionné a déjà servi : il prouve une entrée de
/// marchandise — et bientôt un mouvement de stock. Le supprimer effacerait la
/// preuve. La suppression physique est donc réservée au brouillon.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_reception WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("la réception {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s > STATUT_BROUILLON as i64 => {
            return Err("une réception validée ou réceptionnée ne se supprime pas".into())
        }
        _ => {}
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    // Les liens partent avec la pièce : un `llx_element_element` orphelin
    // ferait croire à une réception qui n'existe plus.
    tx.exec_drop(
        "DELETE FROM llx_element_element WHERE targettype = 'reception' AND fk_target = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_receptiondet_batch WHERE fk_reception = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_reception WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Un texte facultatif ramené à `None` s'il est vide : une chaîne blanche
/// stockée en base se relit comme une valeur, pas comme une absence.
fn texte_utile(valeur: Option<&str>) -> Option<&str> {
    valeur.map(str::trim).filter(|t| !t.is_empty())
}

/// Une date facultative, ramenée à `None` si elle est vide.
fn date_facultative(valeur: Option<&str>) -> Option<&str> {
    texte_utile(valeur)
}

/// Une date `AAAA-MM-JJ` réelle : `2026-02-31` existe au format mais pas au
/// calendrier, et MySQL le refuserait au milieu de la mise à jour.
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
