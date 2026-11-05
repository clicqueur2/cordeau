//! Expéditions (bons de livraison) : `llx_expedition` + `llx_expeditiondet`.
//!
//! Une expédition, c'est ce qui part chez le client : une référence, un tiers,
//! des lignes de marchandises et un statut qui suit la vie de la pièce
//! (brouillon, validée, expédiée). Elle ne porte aucun montant : ce qui se
//! facture reste la commande ou la facture, jamais le bon de livraison.
//!
//! Les statuts sont ceux de Dolibarr : `0` brouillon, `1` validée, `2`
//! expédiée. Le cycle est irréversible : une pièce validée ne redevient pas un
//! brouillon, une pièce expédiée ne se « dé-expédie » pas. C'est pourquoi ce
//! module n'expose aucune transition inverse : une erreur de saisie se corrige
//! en supprimant le brouillon, pas en remontant le statut.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

/// Préfixe des références.
///
/// Dolibarr numérote `EX` + AAMM + 4 chiffres (`mod_expedition_sherbrooke`),
/// mais ce module de numérotation n'est pas porté : `divers::suivant_ref` tire
/// donc `EX0001`, `EX0002`… dans l'ordre d'insertion.
///
/// Sur une base déjà numérotée par Dolibarr, `suivant_ref` lit le plus grand
/// nombre trouvé après le préfixe — sur `EX2611-0001` il ne lit que `2611` — et
/// la suite repart de là : les numéros peuvent donc sauter, mais aucun doublon
/// n'est produit.
pub const PREFIX: &str = "EX";

/// Brouillon : modifiable, supprimable, sans effet sur le stock.
pub const STATUT_BROUILLON: i16 = 0;
/// Validée : la marchandise est prête, la pièce est figée.
pub const STATUT_VALIDEE: i16 = 1;
/// Expédiée : la marchandise est partie, la pièce ne bouge plus.
pub const STATUT_EXPEDIEE: i16 = 2;

/// `element_type` d'une ligne.
///
/// Dolibarr n'admet que deux valeurs : `commande` quand la ligne est reprise de
/// la commande d'origine (le lien est alors porté par `fk_element` /
/// `fk_elementdet`), `expedition` quand la ligne est saisie sur le bon de
/// livraison. Ce module ne crée que des lignes saisies : il n'a pas la commande
/// sous la main.
const ELEMENT_TYPE: &str = "expedition";

/// Une expédition, avec le nom du tiers et les totaux de ses lignes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Expedition {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub fk_soc: i64,
    /// Raison sociale, relue par jointure : la liste s'affiche sans seconde
    /// requête par ligne.
    pub tiers: Option<String>,
    pub fk_projet: Option<i64>,
    /// Référence du document chez le client (son bon de commande, par ex.).
    pub ref_customer: Option<String>,
    pub date_creation: Option<String>,
    pub date_valid: Option<String>,
    pub date_expedition: Option<String>,
    /// Date de livraison constatée — distincte du départ.
    pub date_livraison: Option<String>,
    pub tracking_number: Option<String>,
    pub fk_statut: i16,
    pub facturee: bool,
    pub poids: Option<Decimal>,
    pub nb_lignes: i64,
    /// Somme des quantités des lignes : c'est le seul « total » d'un bon de
    /// livraison, faute de montants.
    pub quantite_totale: Decimal,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
}

impl Expedition {
    pub fn est_brouillon(&self) -> bool {
        self.fk_statut == STATUT_BROUILLON
    }

    pub fn est_validee(&self) -> bool {
        self.fk_statut == STATUT_VALIDEE
    }

    pub fn est_expediee(&self) -> bool {
        self.fk_statut == STATUT_EXPEDIEE
    }
}

/// Une ligne de bon de livraison.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExpeditionLine {
    pub rowid: i64,
    pub fk_expedition: i64,
    pub fk_product: Option<i64>,
    pub description: Option<String>,
    pub qty: Decimal,
    pub fk_unit: Option<i64>,
    pub fk_entrepot: Option<i64>,
    pub rang: i32,
}

/// Ce qu'il faut pour créer un bon de livraison.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewExpedition {
    pub fk_soc: i64,
    pub fk_projet: Option<i64>,
    pub ref_customer: Option<String>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub lines: Vec<NewExpeditionLine>,
}

impl NewExpedition {
    pub fn new(fk_soc: i64) -> Self {
        Self {
            fk_soc,
            ..Default::default()
        }
    }
}

/// Une ligne à créer. Elle n'a pas de rang : l'ordre du vecteur fait foi.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewExpeditionLine {
    pub fk_product: Option<i64>,
    pub description: String,
    pub qty: Decimal,
    pub fk_unit: Option<i64>,
    pub fk_entrepot: Option<i64>,
}

impl NewExpeditionLine {
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
const SELECT_COLS: &str = "e.rowid, e.entity, e.ref, e.fk_soc, s.nom, e.fk_projet, \
     e.ref_customer, DATE_FORMAT(e.date_creation, '%Y-%m-%d'), \
     DATE_FORMAT(e.date_valid, '%Y-%m-%d'), \
     DATE_FORMAT(e.date_expedition, '%Y-%m-%d'), \
     DATE_FORMAT(e.date_delivery, '%Y-%m-%d'), e.tracking_number, e.fk_statut, \
     e.billed, CAST(e.weight AS CHAR), \
     (SELECT COUNT(*) FROM llx_expeditiondet d WHERE d.fk_expedition = e.rowid), \
     (SELECT CAST(COALESCE(SUM(d.qty), 0) AS CHAR) FROM llx_expeditiondet d \
      WHERE d.fk_expedition = e.rowid), \
     e.note_private, e.note_public";

const FROM: &str = "FROM llx_expedition e LEFT JOIN llx_societe s ON s.rowid = e.fk_soc";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

/// Les colonnes `double` ne se décodent pas en `Decimal` : tout passe par
/// `CAST(... AS CHAR)`, seule façon d'éviter un arrondi binaire sur une
/// quantité.
fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Expedition {
    Expedition {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        fk_soc: opt_i(&row, 3).unwrap_or(0),
        tiers: opt_s(&row, 4),
        fk_projet: opt_i(&row, 5),
        ref_customer: opt_s(&row, 6),
        date_creation: opt_s(&row, 7),
        date_valid: opt_s(&row, 8),
        date_expedition: opt_s(&row, 9),
        date_livraison: opt_s(&row, 10),
        tracking_number: opt_s(&row, 11),
        fk_statut: opt_i(&row, 12).unwrap_or(0) as i16,
        facturee: opt_i(&row, 13).unwrap_or(0) != 0,
        poids: opt_dec(&row, 14),
        nb_lignes: opt_i(&row, 15).unwrap_or(0),
        quantite_totale: opt_dec(&row, 16).unwrap_or_default(),
        note_private: opt_s(&row, 17),
        note_public: opt_s(&row, 18),
    }
}

/// Expéditions de l'entité, de la plus récente à la plus ancienne.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Expedition>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} {FROM} WHERE e.entity = ? \
         ORDER BY e.date_creation DESC, e.rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limite, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Une expédition précise, ou `None` si elle n'existe pas dans cette entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Expedition>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} {FROM} WHERE e.entity = ? AND e.rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// Lignes d'une expédition, dans l'ordre de saisie.
///
/// La jointure sur l'en-tête n'est pas décorative : elle rappelle que les
/// lignes n'ont pas de colonne `entity` et ne peuvent donc pas être cloisonnées
/// autrement.
pub fn lines(
    pool: &Pool,
    entity: i64,
    fk_expedition: i64,
) -> Result<Vec<ExpeditionLine>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT d.rowid, d.fk_expedition, d.fk_product, d.description, \
                CAST(d.qty AS CHAR), d.fk_unit, d.fk_entrepot, d.rang \
         FROM llx_expeditiondet d \
         JOIN llx_expedition e ON e.rowid = d.fk_expedition \
         WHERE e.entity = :entity AND d.fk_expedition = :id \
         ORDER BY d.rang ASC, d.rowid ASC",
        params! { "entity" => entity, "id" => fk_expedition },
    )?;
    Ok(rows
        .iter()
        .map(|r| ExpeditionLine {
            rowid: opt_i(r, 0).unwrap_or(0),
            fk_expedition: opt_i(r, 1).unwrap_or(0),
            fk_product: opt_i(r, 2),
            description: opt_s(r, 3),
            qty: opt_dec(r, 4).unwrap_or_default(),
            fk_unit: opt_i(r, 5),
            fk_entrepot: opt_i(r, 6),
            rang: opt_i(r, 7).unwrap_or(0) as i32,
        })
        .collect())
}

/// Crée un bon de livraison en brouillon et renvoie son `rowid`.
///
/// La création est tout ou rien : l'en-tête et ses lignes vivent dans la même
/// transaction. Un bon sans ligne n'est pas une pièce, seulement un numéro
/// consommé — d'où le refus, contrôlé avant toute écriture.
pub fn create(pool: &Pool, entity: i64, new: &NewExpedition) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;

    let mut conn = pool.get_conn()?;
    // Le verrou nommé est porté par le serveur, pas par la connexion : il
    // sérialise la numérotation même si `suivant_ref` tire la référence sur une
    // autre connexion du pool (le pool par défaut en offre dix ; il en faut au
    // moins deux, sinon l'appel attendrait indéfiniment). Le libellé porte
    // l'entité, comme ailleurs dans le noyau.
    let verrou = format!("dolibarr_rs_expedition_{entity}");
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
    new: &NewExpedition,
) -> Result<i64, Box<dyn Error>> {
    verifier_tiers(conn, entity, new.fk_soc)?;
    if let Some(projet) = new.fk_projet {
        verifier_projet(conn, entity, projet)?;
    }

    let reference = crate::divers::suivant_ref(pool, "llx_expedition", "ref", PREFIX, entity)?;

    let mut tx = conn.start_transaction(TxOpts::default())?;
    let insere = tx.exec_drop(
        "INSERT INTO llx_expedition \
         (entity, ref, fk_soc, fk_projet, ref_customer, date_creation, fk_statut, \
          billed, note_private, note_public) \
         VALUES (:entity, :ref, :fk_soc, :fk_projet, :ref_customer, NOW(), :statut, \
          0, :note_private, :note_public)",
        params! {
            "entity" => entity,
            "ref" => reference.as_str(),
            "fk_soc" => new.fk_soc,
            "fk_projet" => new.fk_projet,
            "ref_customer" => new.ref_customer.as_deref(),
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
    // du bon de livraison, et il commence à 0 comme dans Dolibarr.
    for (rang, ligne) in new.lines.iter().enumerate() {
        tx.exec_drop(
            "INSERT INTO llx_expeditiondet \
             (fk_expedition, element_type, fk_product, qty, fk_unit, fk_entrepot, \
              description, rang) \
             VALUES (:fk_expedition, :element_type, :fk_product, :qty, :fk_unit, \
              :fk_entrepot, :description, :rang)",
            params! {
                "fk_expedition" => rowid,
                "element_type" => ELEMENT_TYPE,
                "fk_product" => ligne.fk_product,
                "qty" => ligne.qty.to_string(),
                "fk_unit" => ligne.fk_unit,
                "fk_entrepot" => ligne.fk_entrepot,
                "description" => ligne.description.trim(),
                "rang" => rang as i32,
            },
        )?;
    }
    tx.commit()?;
    Ok(rowid)
}

/// Contrôles de forme, faits avant d'ouvrir la moindre connexion : une erreur
/// de saisie ne doit pas coûter un aller-retour SQL.
fn verifier_entree(new: &NewExpedition) -> Result<(), Box<dyn Error>> {
    if new.fk_soc <= 0 {
        return Err(
            "le tiers (fk_soc) est obligatoire : une expédition se livre à quelqu'un".into(),
        );
    }
    if new.lines.is_empty() {
        return Err(
            "une expédition sans ligne ne se crée pas : il faut au moins une marchandise".into(),
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
        // livraison : le client ne saurait pas ce qu'il reçoit.
        if ligne.description.trim().is_empty() && ligne.fk_product.is_none() {
            return Err(format!("ligne {numero} : il faut une description ou un produit").into());
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
/// traitement (deux bons de livraison pour une seule sortie de stock).
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_expedition WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("l'expédition {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s == STATUT_VALIDEE as i64 => {
            return Err("cette expédition est déjà validée".into())
        }
        Some(s) if s >= STATUT_EXPEDIEE as i64 => {
            return Err("cette expédition est déjà expédiée : elle n'a plus à être validée".into())
        }
        _ => {}
    }

    let lignes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_expeditiondet WHERE fk_expedition = :rowid",
        params! { "rowid" => rowid },
    )?;
    if lignes.unwrap_or(0) == 0 {
        return Err("une expédition sans ligne ne se valide pas".into());
    }

    conn.exec_drop(
        "UPDATE llx_expedition SET fk_statut = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Marque une pièce validée comme expédiée : statut 2, date de départ et
/// numéro de suivi du transporteur.
///
/// Seule une pièce validée part : expédier un brouillon reviendrait à faire
/// sortir de la marchandise jamais contrôlée, et le stock ne saurait plus à
/// quelle pièce la rattacher.
pub fn expedier(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    date: &str,
    tracking: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let date = date.trim();
    // La colonne est un `date` : une chaîne mal formée ferait échouer la requête
    // en mode strict, sans dire à l'appelant ce qu'il a écrit de travers.
    if !date_valide(date) {
        return Err(
            format!("date d'expédition « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }
    let tracking = tracking.map(str::trim).filter(|t| !t.is_empty());

    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_expedition WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("l'expédition {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s == STATUT_BROUILLON as i64 => {
            return Err(
                "cette expédition n'est pas validée : la valider avant de l'expédier".into(),
            )
        }
        Some(s) if s >= STATUT_EXPEDIEE as i64 => {
            return Err("cette expédition est déjà expédiée".into())
        }
        _ => {}
    }

    conn.exec_drop(
        "UPDATE llx_expedition SET fk_statut = :statut, date_expedition = :date_exp, \
         tracking_number = :tracking \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_EXPEDIEE,
            "date_exp" => date,
            "tracking" => tracking,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Supprime un brouillon et ses lignes.
///
/// Un bon validé ou expédié a déjà servi : il prouve une sortie de
/// marchandise, et le supprimer effacerait la preuve. La suppression physique
/// est donc réservée au brouillon.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_expedition WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("l'expédition {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s > STATUT_BROUILLON as i64 => {
            return Err("une expédition validée ou expédiée ne se supprime pas".into())
        }
        _ => {}
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    // Les liens partent avec la pièce : un `llx_element_element` orphelin
    // ferait croire à une expédition qui n'existe plus.
    tx.exec_drop(
        "DELETE FROM llx_element_element WHERE targettype = 'expedition' AND fk_target = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_expeditiondet WHERE fk_expedition = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_expedition WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
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
