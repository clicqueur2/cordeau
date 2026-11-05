//! Transferts de stock entre entrepôts : `llx_stocktransfer_stocktransfer` +
//! `llx_stocktransfer_stocktransferline`.
//!
//! Un transfert, c'est de la marchandise qui quitte un entrepôt pour un autre
//! du même Dolibarr : une référence, deux entrepôts, des lignes de quantités et
//! un statut qui suit la pièce (brouillon, expédié, reçu). Aucun montant à
//! facturer : ce module ne touche ni aux commandes, ni aux factures, ni aux
//! tiers — le seul « prix » qu'il manipule est le PMP recopié sur la ligne, à
//! titre de valorisation.
//!
//! Les statuts sont ceux de Dolibarr : `0` brouillon, `1` expédié (la
//! marchandise a quitté la source), `2` reçu (l'arrivée est constatée à
//! destination). Le cycle est irréversible : une pièce expédiée ne redevient pas
//! un brouillon, une pièce reçue ne se « dé-réceptionne » pas. Ce module
//! n'expose donc aucune transition inverse : une erreur de saisie se corrige en
//! supprimant le brouillon, pas en remontant le statut.
//!
//! # Le stock ne bouge pas
//!
//! Un transfert réellement exécuté décrémente l'entrepôt source et incrémente
//! l'entrepôt destination, en **deux mouvements** (une ligne dans
//! `llx_stock_mouvement` et un nouveau `reel` dans `llx_product_stock`, les deux
//! dans la même transaction). Ce module n'écrit dans **aucune** de ces deux
//! tables.
//!
//! Le choix est délibéré. Écrire la moitié d'un transfert — la sortie de la
//! source sans l'entrée à destination, ou l'inverse — laisserait un stock faux
//! et impossible à expliquer : une quantité disparue entre deux entrepôts, sans
//! pièce pour dire où elle est passée. Un transfert se joue en deux temps
//! (départ, puis arrivée), et entre les deux la marchandise est **en route** :
//! elle n'est ni dans la source, ni dans la destination. Tant que ce module ne
//! sait pas tenir cet état « en transit » — ni l'annuler proprement quand une
//! pièce est supprimée ou quand l'arrivée n'est jamais constatée — il n'écrit
//! rien.
//!
//! **Ce module est un registre de transferts, pas un exécutant.** Une pièce
//! reçue ne change pas le stock du produit ; elle dit seulement que la
//! marchandise est arrivée. C'est écrit ici noir sur blanc pour que personne ne
//! prenne ce silence pour un oubli — même parti pris que `reception.rs`, dont le
//! mouvement de stock viendra avec le module de mouvements dédié.
//!
//! # Entité
//!
//! Seul l'en-tête porte `entity`. Les lignes n'ont pas de colonne d'entité :
//! toutes les lectures de lignes joignent donc l'en-tête, sinon une ligne
//! pourrait être lue depuis une autre entité en connaissant son identifiant.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

/// Préfixe des références.
///
/// Dolibarr numérote `TR` + AAMM + 4 chiffres via son module de numérotation,
/// qui n'est pas porté : `divers::suivant_ref` tire donc `TR0001`, `TR0002`…
/// dans l'ordre d'insertion.
///
/// Sur une base déjà numérotée par Dolibarr, `suivant_ref` lit le plus grand
/// nombre trouvé après le préfixe — sur `TR2611-0001` il ne lit que `2611` — et
/// la suite repart de là : les numéros peuvent donc sauter, mais aucun doublon
/// n'est produit.
pub const PREFIX: &str = "TR";

/// Brouillon : modifiable, supprimable, sans effet sur le stock.
pub const STATUT_BROUILLON: i16 = 0;
/// Expédié : la marchandise a quitté l'entrepôt source.
pub const STATUT_EXPEDIE: i16 = 1;
/// Reçu : l'arrivée est constatée à l'entrepôt destination.
pub const STATUT_RECU: i16 = 2;

/// Un transfert, avec les références de ses deux entrepôts et le total de ses
/// lignes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Transfert {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub label: Option<String>,
    pub fk_soc: Option<i64>,
    pub fk_project: Option<i64>,
    pub fk_warehouse_source: Option<i64>,
    pub fk_warehouse_destination: Option<i64>,
    /// Référence de l'entrepôt de départ, relue par jointure : la liste
    /// s'affiche sans seconde requête par pièce.
    pub entrepot_source: Option<String>,
    /// Référence de l'entrepôt d'arrivée, relue par jointure.
    pub entrepot_destination: Option<String>,
    pub description: Option<String>,
    pub date_creation: Option<String>,
    pub date_prevue_depart: Option<String>,
    /// Date de départ réelle, posée par `expedier`.
    pub date_reelle_depart: Option<String>,
    pub date_prevue_arrivee: Option<String>,
    /// Date d'arrivée réelle, posée par `recevoir`.
    pub date_reelle_arrivee: Option<String>,
    pub fk_user_creat: i64,
    pub status: i16,
    pub nb_lignes: i64,
    /// Somme des quantités des lignes : c'est le seul « total » d'un transfert,
    /// faute de montants.
    pub quantite_totale: Decimal,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
}

impl Transfert {
    pub fn est_brouillon(&self) -> bool {
        self.status == STATUT_BROUILLON
    }

    pub fn est_expedie(&self) -> bool {
        self.status == STATUT_EXPEDIE
    }

    pub fn est_recu(&self) -> bool {
        self.status == STATUT_RECU
    }
}

/// Une ligne de transfert.
///
/// Elle recopie l'entrepôt source et l'entrepôt destination de l'en-tête :
/// c'est ainsi que Dolibarr la construit, et c'est ce qui permet à un futur
/// mouvement de stock de savoir où aller sans relire l'en-tête.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LigneTransfert {
    pub rowid: i64,
    pub fk_stocktransfer: i64,
    pub fk_product: i64,
    /// Référence du produit, relue par jointure (vide si le produit a disparu).
    pub ref_produit: String,
    pub label_produit: String,
    pub qty: Decimal,
    /// Valeur de la ligne : `qty x pmp`, laissée vide si le PMP n'est pas connu.
    pub amount: Option<Decimal>,
    pub batch: Option<String>,
    pub pmp: Option<Decimal>,
    pub fk_warehouse_source: i64,
    pub fk_warehouse_destination: i64,
    pub rang: i32,
}

/// Ce qu'il faut pour créer un transfert.
///
/// `label` est facultatif (la colonne l'est aussi) : la référence, elle, est
/// unique et suffit à identifier la pièce.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewTransfert {
    pub label: Option<String>,
    pub fk_warehouse_source: i64,
    pub fk_warehouse_destination: i64,
    pub date_prevue_depart: Option<String>,
    pub date_prevue_arrivee: Option<String>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub fk_user_creat: i64,
    /// Une ligne au moins : un transfert sans ligne n'a rien à déplacer.
    pub lines: Vec<NewLigneTransfert>,
}

impl NewTransfert {
    /// Un transfert nu entre deux entrepôts, pour un auteur donné.
    ///
    /// Il reste à lui donner ses lignes : `create` refuse un transfert vide.
    pub fn new(
        fk_warehouse_source: i64,
        fk_warehouse_destination: i64,
        fk_user_creat: i64,
    ) -> Self {
        Self {
            fk_warehouse_source,
            fk_warehouse_destination,
            fk_user_creat,
            ..Default::default()
        }
    }
}

/// Une ligne à créer. Elle n'a pas de rang : l'ordre du vecteur fait foi.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewLigneTransfert {
    pub fk_product: i64,
    pub qty: Decimal,
    /// Numéro de lot, quand la marchandise est suivie par lot.
    pub batch: Option<String>,
    /// Prix moyen pondéré recopié, pour valoriser la ligne.
    pub pmp: Option<Decimal>,
}

impl NewLigneTransfert {
    pub fn new(fk_product: i64, qty: Decimal) -> Self {
        Self {
            fk_product,
            qty,
            ..Default::default()
        }
    }
}

/// Colonnes relues, dans l'ordre attendu par `map_row`.
///
/// Le nombre de lignes et la quantité totale sont des sous-requêtes plutôt
/// qu'un `GROUP BY` : une pièce sans ligne doit apparaître (total à zéro) et non
/// disparaître de la liste, ce qu'un `JOIN` agrégeant ferait.
const SELECT_COLS: &str = "t.rowid, t.entity, t.ref, t.label, t.fk_soc, t.fk_project, \
     t.fk_warehouse_source, t.fk_warehouse_destination, es.ref, ed.ref, \
     t.description, DATE_FORMAT(t.date_creation, '%Y-%m-%d'), \
     DATE_FORMAT(t.date_prevue_depart, '%Y-%m-%d'), \
     DATE_FORMAT(t.date_reelle_depart, '%Y-%m-%d'), \
     DATE_FORMAT(t.date_prevue_arrivee, '%Y-%m-%d'), \
     DATE_FORMAT(t.date_reelle_arrivee, '%Y-%m-%d'), \
     t.fk_user_creat, t.status, t.note_private, t.note_public, \
     (SELECT COUNT(*) FROM llx_stocktransfer_stocktransferline l \
      WHERE l.fk_stocktransfer = t.rowid), \
     (SELECT CAST(COALESCE(SUM(l.qty), 0) AS CHAR) \
      FROM llx_stocktransfer_stocktransferline l \
      WHERE l.fk_stocktransfer = t.rowid)";

/// Les deux entrepôts sont joints : la liste et la fiche affichent un lieu, pas
/// un identifiant.
const FROM: &str = "FROM llx_stocktransfer_stocktransfer t \
     LEFT JOIN llx_entrepot es ON es.rowid = t.fk_warehouse_source \
     LEFT JOIN llx_entrepot ed ON ed.rowid = t.fk_warehouse_destination";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

/// Les colonnes `double` ne se décodent pas en `Decimal` : tout passe par
/// `CAST(... AS CHAR)`, seule façon d'éviter un arrondi binaire sur une
/// quantité. Aucun arrondi monétaire ici : une quantité n'est pas de l'argent,
/// 0,125 reste 0,125.
fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i)
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .map(|d| d.normalize())
}

fn map_row(row: Row) -> Transfert {
    Transfert {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        label: opt_s(&row, 3),
        fk_soc: opt_i(&row, 4),
        fk_project: opt_i(&row, 5),
        fk_warehouse_source: opt_i(&row, 6),
        fk_warehouse_destination: opt_i(&row, 7),
        entrepot_source: opt_s(&row, 8),
        entrepot_destination: opt_s(&row, 9),
        description: opt_s(&row, 10),
        date_creation: opt_s(&row, 11),
        date_prevue_depart: opt_s(&row, 12),
        date_reelle_depart: opt_s(&row, 13),
        date_prevue_arrivee: opt_s(&row, 14),
        date_reelle_arrivee: opt_s(&row, 15),
        fk_user_creat: opt_i(&row, 16).unwrap_or(0),
        status: opt_i(&row, 17).unwrap_or(0) as i16,
        note_private: opt_s(&row, 18),
        note_public: opt_s(&row, 19),
        nb_lignes: opt_i(&row, 20).unwrap_or(0),
        quantite_totale: opt_dec(&row, 21).unwrap_or_default(),
    }
}

/// Transferts de l'entité, du plus récent au plus ancien.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Transfert>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} {FROM} WHERE t.entity = ? \
         ORDER BY t.date_creation DESC, t.rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limite, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Un transfert précis, ou `None` s'il n'existe pas dans cette entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Transfert>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} {FROM} WHERE t.entity = ? AND t.rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// Lignes d'un transfert, dans l'ordre de saisie.
///
/// La jointure sur l'en-tête n'est pas décorative : elle rappelle que les lignes
/// n'ont pas de colonne `entity` et ne peuvent donc pas être cloisonnées
/// autrement.
pub fn lines(
    pool: &Pool,
    entity: i64,
    fk_transfert: i64,
) -> Result<Vec<LigneTransfert>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT l.rowid, l.fk_stocktransfer, l.fk_product, COALESCE(p.ref, ''), \
                COALESCE(p.label, ''), CAST(l.qty AS CHAR), CAST(l.amount AS CHAR), \
                CAST(l.pmp AS CHAR), l.batch, l.fk_warehouse_source, \
                l.fk_warehouse_destination, l.rang \
         FROM llx_stocktransfer_stocktransferline l \
         JOIN llx_stocktransfer_stocktransfer t ON t.rowid = l.fk_stocktransfer \
         LEFT JOIN llx_product p ON p.rowid = l.fk_product \
         WHERE t.entity = :entity AND l.fk_stocktransfer = :id \
         ORDER BY l.rang ASC, l.rowid ASC",
        params! { "entity" => entity, "id" => fk_transfert },
    )?;
    Ok(rows
        .iter()
        .map(|r| LigneTransfert {
            rowid: opt_i(r, 0).unwrap_or(0),
            fk_stocktransfer: opt_i(r, 1).unwrap_or(0),
            fk_product: opt_i(r, 2).unwrap_or(0),
            ref_produit: opt_s(r, 3).unwrap_or_default(),
            label_produit: opt_s(r, 4).unwrap_or_default(),
            qty: opt_dec(r, 5).unwrap_or_default(),
            amount: opt_dec(r, 6),
            pmp: opt_dec(r, 7),
            batch: opt_s(r, 8),
            fk_warehouse_source: opt_i(r, 9).unwrap_or(0),
            fk_warehouse_destination: opt_i(r, 10).unwrap_or(0),
            rang: opt_i(r, 11).unwrap_or(0) as i32,
        })
        .collect())
}

/// Somme des quantités d'un transfert.
///
/// Rend zéro pour une pièce sans ligne — c'est la somme, pas un contrôle
/// d'existence — mais refuse une pièce absente de l'entité : un identifiant
/// mal recopié ne doit pas se lire comme « un transfert vide ».
pub fn quantite_totale(
    pool: &Pool,
    entity: i64,
    fk_transfert: i64,
) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_stocktransfer_stocktransfer \
         WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => fk_transfert },
    )?;
    if existe.is_none() {
        return Err(
            format!("le transfert {fk_transfert} n'existe pas dans l'entité {entity}").into(),
        );
    }
    let total: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(l.qty), 0) AS CHAR) \
         FROM llx_stocktransfer_stocktransferline l \
         JOIN llx_stocktransfer_stocktransfer t ON t.rowid = l.fk_stocktransfer \
         WHERE t.entity = :entity AND l.fk_stocktransfer = :id",
        params! { "entity" => entity, "id" => fk_transfert },
    )?;
    Ok(total
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .map(|d| d.normalize())
        .unwrap_or(Decimal::ZERO))
}

/// Crée un transfert en brouillon et renvoie son `rowid`.
///
/// La création est tout ou rien : l'en-tête et ses lignes vivent dans la même
/// transaction. Un transfert sans ligne n'est pas une pièce, seulement un numéro
/// consommé — d'où le refus, contrôlé avant toute écriture.
///
/// Refus explicites, tous avant la moindre écriture : auteur absent, source et
/// destination confondues, aucune ligne, quantité non strictement positive,
/// entrepôt ou produit inconnu de l'entité, date prévue illisible.
pub fn create(pool: &Pool, entity: i64, new: &NewTransfert) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;

    let mut conn = pool.get_conn()?;
    // Le verrou nommé est porté par le serveur, pas par la connexion : il
    // sérialise la numérotation même si `suivant_ref` tire la référence sur une
    // autre connexion du pool (le pool par défaut en offre dix ; il en faut au
    // moins deux, sinon l'appel attendrait indéfiniment). Le libellé porte
    // l'entité, comme ailleurs dans le noyau.
    let verrou = format!("dolibarr_rs_transfert_stock_{entity}");
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
    new: &NewTransfert,
) -> Result<i64, Box<dyn Error>> {
    verifier_entrepot(conn, entity, new.fk_warehouse_source, "source")?;
    verifier_entrepot(conn, entity, new.fk_warehouse_destination, "destination")?;
    verifier_utilisateur(conn, new.fk_user_creat)?;
    for (index, ligne) in new.lines.iter().enumerate() {
        verifier_produit(conn, entity, ligne.fk_product, index + 1)?;
    }

    let reference = crate::divers::suivant_ref(
        pool,
        "llx_stocktransfer_stocktransfer",
        "ref",
        PREFIX,
        entity,
    )?;

    // Un libellé vide n'est pas un libellé : la colonne accepte NULL, autant y
    // mettre NULL plutôt qu'une chaîne d'espaces qui s'afficherait comme un
    // libellé présent mais invisible.
    let label = new
        .label
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty());

    let mut tx = conn.start_transaction(TxOpts::default())?;
    let insere = tx.exec_drop(
        "INSERT INTO llx_stocktransfer_stocktransfer \
         (entity, ref, label, fk_warehouse_source, fk_warehouse_destination, \
          date_creation, date_prevue_depart, date_prevue_arrivee, note_private, \
          note_public, fk_user_creat, status) \
         VALUES (:entity, :ref, :label, :source, :destination, NOW(), :depart, \
          :arrivee, :note_private, :note_public, :auteur, :statut)",
        params! {
            "entity" => entity,
            "ref" => reference.as_str(),
            "label" => label,
            "source" => new.fk_warehouse_source,
            "destination" => new.fk_warehouse_destination,
            "depart" => new.date_prevue_depart.as_deref(),
            "arrivee" => new.date_prevue_arrivee.as_deref(),
            "note_private" => new.note_private.as_deref(),
            "note_public" => new.note_public.as_deref(),
            "auteur" => new.fk_user_creat,
            "statut" => STATUT_BROUILLON,
        },
    );
    if let Err(e) = insere {
        tx.rollback()?;
        // 1062 : `ref` déjà prise. Le verrou rend le cas improbable ; s'il
        // survient, c'est qu'un autre écrivain (l'interface Dolibarr) ne le
        // prend pas — relancer la création suffit, on ne boucle donc pas ici.
        if is_duplicate(&e) {
            return Err(format!(
                "la référence {reference} vient d'être prise par un autre écrivain : \
                 relancer la création"
            )
            .into());
        }
        return Err(e.into());
    }
    let rowid = tx.last_insert_id().unwrap_or(0) as i64;

    // Le rang suit l'ordre du vecteur : c'est lui qui fixe l'ordre d'impression
    // du bon, et il commence à 0 comme dans Dolibarr.
    for (rang, ligne) in new.lines.iter().enumerate() {
        // `amount` est la valeur de la marchandise déplacée. Sans PMP connu, on
        // la laisse vide : mieux vaut une valorisation absente qu'un zéro qui
        // affirmerait que la marchandise ne vaut rien.
        let montant = ligne
            .pmp
            .map(|pmp| money::round_money(ligne.qty * pmp))
            .map(|d| d.to_string());
        let lot = ligne
            .batch
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty());
        tx.exec_drop(
            "INSERT INTO llx_stocktransfer_stocktransferline \
             (amount, qty, fk_warehouse_source, fk_warehouse_destination, \
              fk_stocktransfer, fk_product, batch, pmp, rang) \
             VALUES (:amount, :qty, :source, :destination, :transfert, :produit, \
              :lot, :pmp, :rang)",
            params! {
                "amount" => montant,
                "qty" => ligne.qty.to_string(),
                "source" => new.fk_warehouse_source,
                "destination" => new.fk_warehouse_destination,
                "transfert" => rowid,
                "produit" => ligne.fk_product,
                "lot" => lot,
                "pmp" => ligne.pmp.map(|d| d.to_string()),
                "rang" => rang as i32,
            },
        )?;
    }
    tx.commit()?;
    Ok(rowid)
}

/// Contrôles de forme, faits avant d'ouvrir la moindre connexion : une erreur de
/// saisie ne doit pas coûter un aller-retour SQL.
fn verifier_entree(new: &NewTransfert) -> Result<(), Box<dyn Error>> {
    if new.fk_user_creat <= 0 {
        return Err("l'auteur du transfert (fk_user_creat) est obligatoire : \
                    la colonne est NOT NULL et porte une clé étrangère vers llx_user"
            .into());
    }
    // Transférer d'un entrepôt vers lui-même, c'est faire sortir puis rentrer la
    // même marchandise : deux mouvements pour un stock identique, et une pièce
    // qui laissera croire à un déplacement. Ce n'est pas un transfert, et le
    // refuser ici évite d'avoir à l'expliquer plus tard dans l'historique.
    if new.fk_warehouse_source == new.fk_warehouse_destination {
        return Err(format!(
            "l'entrepôt de départ et l'entrepôt d'arrivée sont le même ({}) : \
             ce n'est pas un transfert",
            new.fk_warehouse_source
        )
        .into());
    }
    if new.lines.is_empty() {
        return Err(
            "un transfert sans ligne ne se crée pas : il n'y aurait rien à déplacer".into(),
        );
    }
    for (index, ligne) in new.lines.iter().enumerate() {
        let numero = index + 1;
        if ligne.qty <= Decimal::ZERO {
            return Err(
                format!("ligne {numero} : la quantité doit être strictement positive").into(),
            );
        }
    }

    for (champ, date) in [
        ("départ prévu", new.date_prevue_depart.as_deref()),
        ("arrivée prévue", new.date_prevue_arrivee.as_deref()),
    ] {
        if let Some(date) = date {
            let date = date.trim();
            if !date_valide(date) {
                return Err(format!(
                    "date de {champ} « {date} » invalide : format attendu AAAA-MM-JJ"
                )
                .into());
            }
        }
    }
    // Une arrivée prévue avant le départ n'est pas une contrainte SQL, c'est une
    // incohérence de saisie : la refuser tout de suite évite un transfert qu'on
    // croira en retard alors qu'il part après son arrivée. Les dates sont au
    // format ISO, l'ordre alphabétique est donc l'ordre chronologique.
    if let (Some(depart), Some(arrivee)) = (
        new.date_prevue_depart.as_deref().map(str::trim),
        new.date_prevue_arrivee.as_deref().map(str::trim),
    ) {
        if arrivee < depart {
            return Err(
                format!("l'arrivée prévue ({arrivee}) précède le départ prévu ({depart})").into(),
            );
        }
    }
    Ok(())
}

fn verifier_entrepot(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_entrepot: i64,
    role: &str,
) -> Result<(), Box<dyn Error>> {
    let trouve: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_entrepot WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_entrepot },
    )?;
    if trouve.is_none() {
        return Err(format!(
            "l'entrepôt de {role} {fk_entrepot} n'existe pas dans l'entité {entity}"
        )
        .into());
    }
    Ok(())
}

/// L'auteur doit exister — `fk_user_creat` porte une clé étrangère vers
/// `llx_user` — mais **sans filtre d'entité** : un utilisateur rattaché à
/// l'entité 1 saisit légitimement des pièces dans l'entité 7, et le filtrer
/// reviendrait à lui interdire de travailler.
fn verifier_utilisateur(conn: &mut mysql::PooledConn, fk_user: i64) -> Result<(), Box<dyn Error>> {
    let auteur: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_user WHERE rowid = :rowid",
        params! { "rowid" => fk_user },
    )?;
    if auteur.is_none() {
        return Err(format!("l'utilisateur {fk_user} n'existe pas").into());
    }
    Ok(())
}

fn verifier_produit(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_product: i64,
    numero: usize,
) -> Result<(), Box<dyn Error>> {
    let produit: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_product WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_product },
    )?;
    if produit.is_none() {
        return Err(format!(
            "ligne {numero} : le produit {fk_product} n'existe pas dans l'entité {entity}"
        )
        .into());
    }
    Ok(())
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

/// Expédie un brouillon : `0` → `1`, `date_reelle_depart` est posée.
///
/// Expédier deux fois n'est pas une opération idempotente : c'est un appelant
/// qui a perdu le fil de son écran, et le laisser passer masquerait un double
/// traitement — deux départs annoncés pour une seule marchandise.
///
/// Comme annoncé en tête de module, cette fonction **n'écrit pas** dans
/// `llx_product_stock` ni dans `llx_stock_mouvement` : elle constate le départ,
/// elle ne le comptabilise pas.
pub fn expedier(pool: &Pool, entity: i64, rowid: i64, date: &str) -> Result<(), Box<dyn Error>> {
    let date = date.trim();
    // La colonne est un `date` : une chaîne mal formée ferait échouer la requête
    // en mode strict, sans dire à l'appelant ce qu'il a écrit de travers.
    if !date_valide(date) {
        return Err(
            format!("date de départ « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }

    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_stocktransfer_stocktransfer \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("le transfert {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s == STATUT_EXPEDIE as i64 => return Err("ce transfert est déjà expédié".into()),
        Some(s) if s >= STATUT_RECU as i64 => {
            return Err("ce transfert est déjà reçu : il n'a plus à être expédié".into())
        }
        _ => {}
    }

    conn.exec_drop(
        "UPDATE llx_stocktransfer_stocktransfer \
         SET status = :statut, date_reelle_depart = :date_depart \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_EXPEDIE,
            "date_depart" => date,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Réceptionne un transfert expédié : `1` → `2`, `date_reelle_arrivee` est
/// posée.
///
/// Seule une pièce expédiée arrive : constater l'arrivée d'une marchandise qui
/// n'est jamais partie reviendrait à faire apparaître du stock à destination, et
/// le futur mouvement de stock ne saurait plus à quel départ le rattacher.
///
/// Comme annoncé en tête de module, cette fonction **n'écrit pas** dans
/// `llx_product_stock` ni dans `llx_stock_mouvement`. Un transfert réellement
/// exécuté devrait décrémenter la source **et** incrémenter la destination, en
/// deux mouvements ; n'écrire que l'entrée laisserait la source pleine et
/// dupliquerait la marchandise. Ici, la réception **constate** l'arrivée, elle ne
/// la comptabilise pas : le module est un registre de transferts, pas un
/// exécutant. Le stock du produit ne bouge donc pas, et c'est voulu.
pub fn recevoir(pool: &Pool, entity: i64, rowid: i64, date: &str) -> Result<(), Box<dyn Error>> {
    let date = date.trim();
    if !date_valide(date) {
        return Err(
            format!("date d'arrivée « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }

    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_stocktransfer_stocktransfer \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("le transfert {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s == STATUT_BROUILLON as i64 => {
            return Err("ce transfert n'est pas expédié : \
                        l'expédier avant de constater son arrivée"
                .into())
        }
        Some(s) if s >= STATUT_RECU as i64 => return Err("ce transfert est déjà reçu".into()),
        _ => {}
    }

    conn.exec_drop(
        "UPDATE llx_stocktransfer_stocktransfer \
         SET status = :statut, date_reelle_arrivee = :date_arrivee \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_RECU,
            "date_arrivee" => date,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Supprime un brouillon et ses lignes.
///
/// Un transfert expédié ou reçu a déjà servi : il prouve qu'une marchandise a
/// quitté un entrepôt, et le supprimer effacerait la preuve. La suppression
/// physique est donc réservée au brouillon.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT status FROM llx_stocktransfer_stocktransfer \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => {
            return Err(format!("le transfert {rowid} n'existe pas dans l'entité {entity}").into())
        }
        Some(s) if s > STATUT_BROUILLON as i64 => {
            return Err("un transfert expédié ou reçu ne se supprime pas".into())
        }
        _ => {}
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    // Les liens partent avec la pièce : un `llx_element_element` orphelin ferait
    // croire à un transfert qui n'existe plus.
    tx.exec_drop(
        "DELETE FROM llx_element_element \
         WHERE targettype = 'stocktransfer' AND fk_target = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_stocktransfer_stocktransferline WHERE fk_stocktransfer = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_stocktransfer_stocktransfer WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Une date `AAAA-MM-JJ` réelle : `2026-02-31` existe au format mais pas au
/// calendrier, et MySQL la refuserait au milieu de la mise à jour.
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
