//! Commandes fournisseurs (`llx_commande_fournisseur` +
//! `llx_commande_fournisseurdet`).
//!
//! Une commande fournisseur est la pièce qui **engage** l'achat : elle part chez
//! le fournisseur, la marchandise arrive ensuite (réception), la facture arrive
//! en dernier. C'est le frère jumeau de la commande client, côté entrant — avec
//! un cycle plus long, et une différence de fond : ici, c'est nous qui payons.
//!
//! # Statuts
//!
//! Dolibarr en compte six, et le cycle ne revient jamais en arrière :
//!
//! | `fk_statut` | sens | fonction qui y mène |
//! |---|---|---|
//! | 0 | brouillon | `create` |
//! | 1 | validée (geste de saisie) | `valider` |
//! | 2 | approuvée (quelqu'un engage la dépense) | `approuver` |
//! | 3 | commandée (partie chez le fournisseur) | `commander` |
//! | 4 | réceptionnée, partiellement ou totalement | `receptionner` |
//! | 5 | close | hors de ce module |
//!
//! Séparer « validée » et « approuvée » n'est pas une manie de Dolibarr :
//! valider constate que la saisie est complète, approuver engage l'argent. Un
//! noyau qui fusionnerait les deux ferait signer la dépense par celui qui l'a
//! tapée. Aucune transition inverse n'est exposée : une erreur de saisie se
//! corrige en supprimant le brouillon, pas en remontant un statut.
//!
//! # Numérotation
//!
//! Dolibarr numérote `CF` + AAMM + 4 chiffres via son module de numérotation,
//! qui n'est pas porté : `divers::suivant_ref` tire donc `CF0001`, `CF0002`…
//! dans l'ordre d'insertion, sous un verrou nommé — deux créations simultanées
//! ne peuvent pas lire le même maximum.
//!
//! Sur une base déjà numérotée par Dolibarr, `suivant_ref` lit le plus grand
//! nombre trouvé **après le préfixe** : sur `CF2611-0001` il ne lit que `2611`,
//! et la suite repart de là. Les numéros peuvent donc sauter, mais aucun doublon
//! n'est produit — l'index unique `uk_commande_fournisseur_ref (ref, entity)` le
//! garantit, et une collision est signalée à l'appelant au lieu d'être renommée
//! en silence.
//!
//! # Ce que ce module n'écrit pas, et pourquoi
//!
//! - `deposit_percent`, `model_pdf`, `last_main_doc`, `fk_account` restent à
//!   leur valeur par défaut : un acompte ou un modèle de PDF se décident dans
//!   l'interface, pas au moment où l'on saisit une commande.
//! - `fk_user_author` reste `NULL` : ce module ne connaît pas l'utilisateur
//!   courant, et une valeur inventée serait pire qu'une absence.
//! - `source = 0` et `remise_percent = 0` : la commande est saisie directement et
//!   ne porte pas de remise d'en-tête (les remises sont ligne à ligne). Faute de
//!   remise globale, `amount_ht` et `total_ht` portent la même valeur.
//!
//! # Montants
//!
//! Tous les totaux passent par `money::line_totals` : c'est ce qui garantit
//! `ht + tva == ttc` au centime près (la TVA est la différence des deux arrondis,
//! jamais `ht x taux`). Aucun `f64` n'intervient dans l'arithmétique ; les
//! colonnes `double` sont lues en `CAST(... AS CHAR)` et écrites en texte.

use std::error::Error;
use std::str::FromStr;

use money::{line_totals, sum, LineInput};
use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

/// Préfixe des références (`CF` + compteur sur 4 chiffres).
pub const PREFIX: &str = "CF";

/// Brouillon : modifiable, supprimable, sans effet chez le fournisseur.
pub const STATUT_BROUILLON: i16 = 0;
/// Validée : la saisie est complète et cohérente.
pub const STATUT_VALIDEE: i16 = 1;
/// Approuvée : la dépense est engagée, la commande peut partir.
pub const STATUT_APPROUVEE: i16 = 2;
/// Commandée : la pièce est partie chez le fournisseur.
pub const STATUT_COMMANDEE: i16 = 3;
/// Réceptionnée, partiellement ou totalement : la marchandise est entrée.
pub const STATUT_RECEPTIONNEE: i16 = 4;
/// Close : soldée, plus rien à en attendre.
pub const STATUT_CLOSE: i16 = 5;

/// `llx_commande_fournisseur.source` : `0` = saisie directe. Les autres valeurs
/// désignent une commande engendrée par un autre document (proposition
/// fournisseur, commande client), cas que ce module ne traite pas.
const SOURCE_SAISIE: i16 = 0;

/// Ligne de structure (titre de lot) : `product_type = 9`. Elle ne représente
/// aucune marchandise et ne compte pas dans les quantités.
const PRODUCT_TYPE_SECTION: i32 = 9;
/// Ligne de produit ou de service.
const PRODUCT_TYPE_PRODUIT: i32 = 0;
/// Code des lignes de sous-total du module Subtotal. Ces lignes portent
/// `qty = 99` pour forcer l'affichage du montant : les additionner reviendrait à
/// commander 99 unités de rien.
const SPECIAL_CODE_SOUS_TOTAL: i32 = 104777;

/// Une commande fournisseur, avec le nom de son fournisseur.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandeFournisseur {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    /// Référence du document chez le fournisseur (bon de commande, devis
    /// accepté…). C'est elle que le fournisseur reconnaîtra.
    pub ref_supplier: Option<String>,
    pub fk_soc: i64,
    /// Raison sociale relue par jointure : la liste s'affiche sans une seconde
    /// requête par ligne. `None` si la fiche a disparu.
    pub fournisseur: Option<String>,
    pub fk_projet: Option<i64>,
    pub date_creation: Option<String>,
    pub date_valid: Option<String>,
    pub date_approve: Option<String>,
    /// Date à laquelle la commande est partie (colonne `date`).
    pub date_commande: Option<String>,
    /// Date de livraison **prévue**, distincte de l'arrivée réelle.
    pub date_livraison: Option<String>,
    /// Date d'arrivée constatée, posée par `receptionner`.
    pub date_reception: Option<String>,
    pub fk_statut: i16,
    pub billed: bool,
    /// Somme des lignes avant remise d'en-tête ; égale à `total_ht` ici.
    pub amount_ht: Option<Decimal>,
    pub total_tva: Option<Decimal>,
    pub total_ht: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub fk_cond_reglement: Option<i64>,
    pub fk_mode_reglement: Option<i64>,
}

impl CommandeFournisseur {
    pub fn est_brouillon(&self) -> bool {
        self.fk_statut == STATUT_BROUILLON
    }

    pub fn est_validee(&self) -> bool {
        self.fk_statut == STATUT_VALIDEE
    }

    pub fn est_approuvee(&self) -> bool {
        self.fk_statut == STATUT_APPROUVEE
    }

    pub fn est_commandee(&self) -> bool {
        self.fk_statut == STATUT_COMMANDEE
    }

    pub fn est_receptionnee(&self) -> bool {
        self.fk_statut >= STATUT_RECEPTIONNEE
    }
}

/// Une ligne de commande fournisseur.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandeFournisseurLine {
    pub rowid: i64,
    pub fk_commande: i64,
    pub fk_product: Option<i64>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub tva_tx: Option<Decimal>,
    pub qty: Option<Decimal>,
    pub remise_percent: Option<Decimal>,
    /// Prix unitaire hors taxe **tel qu'il a produit les totaux** (6 décimales),
    /// et non la saisie brute : c'est cette valeur qui explique le montant.
    pub subprice: Option<Decimal>,
    pub total_ht: Option<Decimal>,
    pub total_tva: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    pub product_type: i16,
    pub special_code: i32,
    pub rang: i32,
    pub fk_unit: Option<i64>,
}

impl CommandeFournisseurLine {
    /// Titre de lot : à afficher en bande pleine largeur, hors totaux.
    pub fn is_section(&self) -> bool {
        // `product_type` est un `smallint` côté base, la constante un `i32` :
        // on élargit la colonne plutôt que de rétrécir la constante, pour ne
        // pas écrire un `as` qui tronquerait silencieusement un jour.
        i32::from(self.product_type) == PRODUCT_TYPE_SECTION
    }

    /// Sous-total du module Subtotal : un affichage, pas une marchandise.
    pub fn is_sous_total(&self) -> bool {
        self.special_code == SPECIAL_CODE_SOUS_TOTAL
    }
}

/// Ce qu'il faut pour créer une commande fournisseur.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewCommandeFournisseur {
    /// Le tiers commandé. Il doit porter `fournisseur = 1` dans `llx_societe`.
    pub fk_soc: i64,
    pub fk_projet: Option<i64>,
    pub ref_supplier: Option<String>,
    /// Livraison **prévue** (facultative), au format `AAAA-MM-JJ`.
    pub date_livraison: Option<String>,
    pub fk_cond_reglement: Option<i64>,
    pub fk_mode_reglement: Option<i64>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub lines: Vec<NewLigneAchat>,
}

impl NewCommandeFournisseur {
    pub fn new(fk_soc: i64) -> Self {
        Self {
            fk_soc,
            ..Default::default()
        }
    }
}

/// Une ligne à créer. Elle n'a pas de rang : l'ordre du vecteur fait foi.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewLigneAchat {
    pub fk_product: Option<i64>,
    pub label: String,
    pub description: Option<String>,
    /// Taux de TVA en pourcentage, borné à `0..=100`.
    pub tva_tx: Decimal,
    /// Quantité, strictement positive.
    pub qty: Decimal,
    pub remise_percent: Decimal,
    /// Prix unitaire **hors taxe**.
    pub subprice: Decimal,
    pub fk_unit: Option<i64>,
}

impl NewLigneAchat {
    /// Une ligne au taux et à la quantité donnés, sans remise.
    ///
    /// L'ordre reprend celui de `NewProposalLine::new` : libellé, quantité,
    /// prix unitaire, taux. Le taux vient en dernier parce qu'il est le seul
    /// qu'on oublie, et qu'un oubli se voit mieux en fin de liste.
    pub fn new(label: &str, qty: Decimal, subprice: Decimal, tva_tx: Decimal) -> Self {
        Self {
            label: label.to_string(),
            qty,
            subprice,
            tva_tx,
            ..Default::default()
        }
    }
}

/// Une ligne après passage dans `money` : la saisie, plus les trois montants
/// que Dolibarr en déduit. Calculée une seule fois, avant la transaction, pour
/// qu'une erreur d'arithmétique ne laisse pas d'en-tête sans lignes.
struct LigneCalculee {
    fk_product: Option<i64>,
    label: String,
    description: Option<String>,
    tva_tx: Decimal,
    qty: Decimal,
    remise_percent: Decimal,
    subprice: Decimal,
    fk_unit: Option<i64>,
    total_ht: Decimal,
    total_tva: Decimal,
    total_ttc: Decimal,
}

/// Colonnes relues, dans l'ordre attendu par `map_row`.
const SELECT_COLS: &str = "c.rowid, c.entity, c.ref, c.ref_supplier, c.fk_soc, s.nom, \
     c.fk_projet, DATE_FORMAT(c.date_creation, '%Y-%m-%d'), \
     DATE_FORMAT(c.date_valid, '%Y-%m-%d'), \
     DATE_FORMAT(c.date_approve, '%Y-%m-%d'), \
     DATE_FORMAT(c.date_commande, '%Y-%m-%d'), \
     DATE_FORMAT(c.date_livraison, '%Y-%m-%d'), \
     DATE_FORMAT(c.date_reception, '%Y-%m-%d'), c.fk_statut, c.billed, \
     CAST(c.amount_ht AS CHAR), CAST(c.total_tva AS CHAR), \
     CAST(c.total_ht AS CHAR), CAST(c.total_ttc AS CHAR), \
     c.note_private, c.note_public, c.fk_cond_reglement, c.fk_mode_reglement";

const FROM: &str = "FROM llx_commande_fournisseur c \
     LEFT JOIN llx_societe s ON s.rowid = c.fk_soc";

/// `llx_commande_fournisseurdet` n'a pas de colonne `entity` : le cloisonnement
/// passe par la jointure sur l'en-tête, d'où l'alias.
const LINE_COLS: &str = "d.rowid, d.fk_commande, d.fk_product, d.label, d.description, \
     CAST(d.tva_tx AS CHAR), CAST(d.qty AS CHAR), CAST(d.remise_percent AS CHAR), \
     CAST(d.subprice AS CHAR), CAST(d.total_ht AS CHAR), CAST(d.total_tva AS CHAR), \
     CAST(d.total_ttc AS CHAR), d.product_type, d.special_code, d.rang, d.fk_unit";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

/// Les colonnes `float`, `real` et `double` ne se décodent pas en `Decimal` :
/// tout passe par `CAST(... AS CHAR)`, seule façon d'éviter un arrondi binaire
/// sur une quantité ou un prix.
fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> CommandeFournisseur {
    CommandeFournisseur {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        ref_supplier: opt_s(&row, 3),
        fk_soc: opt_i(&row, 4).unwrap_or(0),
        fournisseur: opt_s(&row, 5),
        fk_projet: opt_i(&row, 6),
        date_creation: opt_s(&row, 7),
        date_valid: opt_s(&row, 8),
        date_approve: opt_s(&row, 9),
        date_commande: opt_s(&row, 10),
        date_livraison: opt_s(&row, 11),
        date_reception: opt_s(&row, 12),
        fk_statut: opt_i(&row, 13).unwrap_or(0) as i16,
        billed: opt_i(&row, 14).unwrap_or(0) != 0,
        amount_ht: opt_dec(&row, 15),
        total_tva: opt_dec(&row, 16),
        total_ht: opt_dec(&row, 17),
        total_ttc: opt_dec(&row, 18),
        note_private: opt_s(&row, 19),
        note_public: opt_s(&row, 20),
        fk_cond_reglement: opt_i(&row, 21),
        fk_mode_reglement: opt_i(&row, 22),
    }
}

/// Le statut en clair, pour les messages d'erreur : « réceptionnée » se lit
/// mieux que `4` quand on cherche pourquoi une action est refusée.
pub fn libelle_statut(statut: i16) -> &'static str {
    match statut {
        STATUT_BROUILLON => "brouillon",
        STATUT_VALIDEE => "validée",
        STATUT_APPROUVEE => "approuvée",
        STATUT_COMMANDEE => "commandée",
        STATUT_RECEPTIONNEE => "réceptionnée",
        STATUT_CLOSE => "close",
        _ => "dans un statut inconnu",
    }
}

/// Commandes fournisseurs de l'entité, de la plus récente à la plus ancienne.
///
/// Le tri porte sur `date_creation` avec `rowid` en second : deux commandes
/// saisies le même jour gardent un ordre stable, sinon la pagination sauterait
/// des lignes d'une requête à l'autre.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<CommandeFournisseur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} {FROM} WHERE c.entity = ? \
         ORDER BY c.date_creation DESC, c.rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limite, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Une commande précise, ou `None` si elle n'existe pas dans cette entité.
pub fn get(
    pool: &Pool,
    entity: i64,
    rowid: i64,
) -> Result<Option<CommandeFournisseur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} {FROM} WHERE c.entity = ? AND c.rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// Lignes d'une commande, dans l'ordre de saisie.
pub fn lines(
    pool: &Pool,
    entity: i64,
    rowid: i64,
) -> Result<Vec<CommandeFournisseurLine>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {LINE_COLS} FROM llx_commande_fournisseurdet d \
         JOIN llx_commande_fournisseur c ON c.rowid = d.fk_commande \
         WHERE c.entity = ? AND d.fk_commande = ? ORDER BY d.rang ASC, d.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, rowid))?;
    Ok(rows
        .into_iter()
        .map(|row| CommandeFournisseurLine {
            rowid: opt_i(&row, 0).unwrap_or(0),
            fk_commande: opt_i(&row, 1).unwrap_or(0),
            fk_product: opt_i(&row, 2),
            label: opt_s(&row, 3),
            description: opt_s(&row, 4),
            tva_tx: opt_dec(&row, 5),
            qty: opt_dec(&row, 6),
            remise_percent: opt_dec(&row, 7),
            subprice: opt_dec(&row, 8),
            total_ht: opt_dec(&row, 9),
            total_tva: opt_dec(&row, 10),
            total_ttc: opt_dec(&row, 11),
            product_type: opt_i(&row, 12).unwrap_or(0) as i16,
            special_code: opt_i(&row, 13).unwrap_or(0) as i32,
            rang: opt_i(&row, 14).unwrap_or(0) as i32,
            fk_unit: opt_i(&row, 15),
        })
        .collect())
}

/// Crée une commande fournisseur en brouillon et renvoie son `rowid`.
///
/// Tout est vérifié avant la moindre écriture : un numéro consommé pour une
/// commande refusée serait un trou dans la numérotation, et une commande sans
/// ligne ne serait qu'une enveloppe vide.
///
/// Le verrou nommé est porté par le serveur et non par la connexion : il
/// sérialise la numérotation même si `suivant_ref` tire la référence sur une
/// autre connexion du pool (le pool en offre dix ; il en faut au moins deux,
/// sinon l'appel attendrait indéfiniment).
pub fn create(
    pool: &Pool,
    entity: i64,
    new: &NewCommandeFournisseur,
) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;

    let mut conn = pool.get_conn()?;
    let verrou = format!("dolibarr_rs_commande_fournisseur_{entity}");
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
    new: &NewCommandeFournisseur,
) -> Result<i64, Box<dyn Error>> {
    verifier_fournisseur(conn, entity, new.fk_soc)?;
    if let Some(projet) = new.fk_projet {
        verifier_projet(conn, entity, projet)?;
    }

    let reference =
        crate::divers::suivant_ref(pool, "llx_commande_fournisseur", "ref", PREFIX, entity)?;
    let lignes = calculer_lignes(&new.lines);
    let (total_ht, total_tva, total_ttc) = totaux_lignes(&lignes);

    let mut tx = conn.start_transaction(TxOpts::default())?;
    let insere = tx.exec_drop(
        "INSERT INTO llx_commande_fournisseur \
         (entity, ref, fk_soc, fk_projet, ref_supplier, date_creation, \
          date_livraison, fk_statut, source, amount_ht, remise_percent, \
          total_tva, total_ht, total_ttc, note_private, note_public, \
          fk_cond_reglement, fk_mode_reglement) \
         VALUES (:entity, :ref, :fk_soc, :fk_projet, :ref_supplier, NOW(), \
          :date_livraison, :statut, :source, :amount_ht, 0, \
          :total_tva, :total_ht, :total_ttc, :note_private, :note_public, \
          :cond, :mode)",
        params! {
            "entity" => entity,
            "ref" => reference.as_str(),
            "fk_soc" => new.fk_soc,
            "fk_projet" => new.fk_projet,
            "ref_supplier" => texte_utile(new.ref_supplier.as_deref()),
            "date_livraison" => texte_utile(new.date_livraison.as_deref()),
            "statut" => STATUT_BROUILLON,
            "source" => SOURCE_SAISIE,
            // Sans remise d'en-tête, le total avant remise est le total tout court.
            "amount_ht" => total_ht.to_string(),
            "total_tva" => total_tva.to_string(),
            "total_ht" => total_ht.to_string(),
            "total_ttc" => total_ttc.to_string(),
            "note_private" => texte_utile(new.note_private.as_deref()),
            "note_public" => texte_utile(new.note_public.as_deref()),
            "cond" => new.fk_cond_reglement,
            "mode" => new.fk_mode_reglement,
        },
    );
    if let Err(e) = insere {
        tx.rollback()?;
        // 1062 : `ref` déjà prise. Le verrou rend le cas improbable ; s'il
        // survient, c'est qu'un autre écrivain (l'interface Dolibarr) ne le
        // prend pas — relancer la création suffit, on ne boucle donc pas.
        if is_duplicate(&e) {
            return Err(format!(
                "la référence {reference} vient d'être prise par un autre écrivain : relancer la création"
            )
            .into());
        }
        return Err(e.into());
    }
    let rowid = tx.last_insert_id().unwrap_or(0) as i64;

    // Le rang suit l'ordre du vecteur : c'est lui qui fixe l'ordre d'impression,
    // et il commence à 1 comme dans les autres pièces du noyau.
    for (rang, ligne) in lignes.iter().enumerate() {
        tx.exec_drop(
            "INSERT INTO llx_commande_fournisseurdet \
             (fk_commande, fk_product, label, description, tva_tx, qty, \
              remise_percent, subprice, total_ht, total_tva, total_ttc, \
              product_type, rang, fk_unit) \
             VALUES (:fk_commande, :fk_product, :label, :description, :tva_tx, \
              :qty, :remise_percent, :subprice, :total_ht, :total_tva, \
              :total_ttc, :product_type, :rang, :fk_unit)",
            params! {
                "fk_commande" => rowid,
                "fk_product" => ligne.fk_product,
                "label" => ligne.label.as_str(),
                "description" => ligne.description.as_deref(),
                "tva_tx" => ligne.tva_tx.to_string(),
                "qty" => ligne.qty.to_string(),
                "remise_percent" => ligne.remise_percent.to_string(),
                "subprice" => ligne.subprice.to_string(),
                "total_ht" => ligne.total_ht.to_string(),
                "total_tva" => ligne.total_tva.to_string(),
                "total_ttc" => ligne.total_ttc.to_string(),
                "product_type" => PRODUCT_TYPE_PRODUIT,
                "rang" => (rang + 1) as i32,
                "fk_unit" => ligne.fk_unit,
            },
        )?;
    }
    tx.commit()?;
    Ok(rowid)
}

/// Passe les lignes saisies dans `money::line_totals`.
///
/// `subprice` est réécrit avec le prix unitaire **retenu par le calcul** (6
/// décimales) et non la saisie brute : c'est la valeur qui, multipliée par la
/// quantité, redonne le total affiché. Un prix saisi à 12 décimales et stocké
/// tel quel ferait un total que personne ne peut refaire à la main.
fn calculer_lignes(lignes: &[NewLigneAchat]) -> Vec<LigneCalculee> {
    lignes
        .iter()
        .map(|l| {
            let t = line_totals(&LineInput::new(
                &l.qty.to_string(),
                &l.subprice.to_string(),
                &l.remise_percent.to_string(),
                &l.tva_tx.to_string(),
            ));
            LigneCalculee {
                fk_product: l.fk_product,
                label: l.label.trim().to_string(),
                description: texte_utile(l.description.as_deref()).map(str::to_string),
                tva_tx: l.tva_tx,
                qty: l.qty,
                remise_percent: l.remise_percent,
                subprice: t.unit_price_ht,
                fk_unit: l.fk_unit,
                total_ht: t.total_ht,
                total_tva: t.total_vat,
                total_ttc: t.total_ttc,
            }
        })
        .collect()
}

/// Totaux d'en-tête : somme des lignes, arrondie une seule fois.
fn totaux_lignes(lignes: &[LigneCalculee]) -> (Decimal, Decimal, Decimal) {
    let hts: Vec<Decimal> = lignes.iter().map(|l| l.total_ht).collect();
    let tvas: Vec<Decimal> = lignes.iter().map(|l| l.total_tva).collect();
    let ttcs: Vec<Decimal> = lignes.iter().map(|l| l.total_ttc).collect();
    (sum(&hts), sum(&tvas), sum(&ttcs))
}

/// Contrôles de forme, faits avant d'ouvrir la moindre connexion : une erreur de
/// saisie ne doit ni coûter un aller-retour SQL, ni consommer un numéro.
fn verifier_entree(new: &NewCommandeFournisseur) -> Result<(), Box<dyn Error>> {
    if new.fk_soc <= 0 {
        return Err(
            "le fournisseur (fk_soc) est obligatoire : une commande s'adresse à quelqu'un".into(),
        );
    }
    if new.lines.is_empty() {
        return Err(
            "une commande fournisseur sans ligne ne se crée pas : il n'y aurait rien à commander"
                .into(),
        );
    }
    // `date_livraison` est une colonne `datetime` : une chaîne mal formée ferait
    // échouer l'insertion en mode strict, sans dire à l'appelant ce qu'il a
    // écrit de travers.
    let fautive = texte_utile(new.date_livraison.as_deref()).filter(|d| !date_valide(d));
    if let Some(date) = fautive {
        return Err(
            format!("date de livraison « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }

    // Un pourcentage acceptable : 0 à 100 inclus. Un taux négatif rendrait un TTC
    // inférieur au HT, et au-delà de 100 % la TVA dépasserait la marchandise :
    // ce sont des fautes de saisie, pas des cas à laisser filer jusqu'au PDF.
    let plage = Decimal::ZERO..=Decimal::ONE_HUNDRED;
    for (index, ligne) in new.lines.iter().enumerate() {
        let numero = index + 1;
        if ligne.qty <= Decimal::ZERO {
            return Err(format!(
                "ligne {numero} : la quantité doit être strictement positive (reçu {})",
                ligne.qty
            )
            .into());
        }
        if !plage.contains(&ligne.tva_tx) {
            return Err(format!(
                "ligne {numero} : le taux de TVA doit être compris entre 0 et 100 (reçu {})",
                ligne.tva_tx
            )
            .into());
        }
        if !plage.contains(&ligne.remise_percent) {
            return Err(format!(
                "ligne {numero} : la remise doit être comprise entre 0 et 100 (reçu {})",
                ligne.remise_percent
            )
            .into());
        }
        // Une ligne sans texte et sans produit serait illisible : le fournisseur
        // ne saurait pas ce qu'on lui commande.
        if ligne.label.trim().is_empty() && ligne.fk_product.is_none() {
            return Err(format!("ligne {numero} : il faut un libellé ou un produit").into());
        }
    }
    Ok(())
}

/// Le tiers commandé doit exister **et** être un fournisseur.
///
/// Les deux refus sont distincts à dessein : « n'existe pas » est une erreur
/// d'identifiant, « n'est pas un fournisseur » est une fiche à compléter. Un
/// message unique laisserait l'utilisateur chercher au mauvais endroit.
fn verifier_fournisseur(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_soc: i64,
) -> Result<(), Box<dyn Error>> {
    let tiers: Option<(i64, i64)> = conn.exec_first(
        "SELECT rowid, COALESCE(fournisseur, 0) FROM llx_societe \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_soc },
    )?;
    let Some((_, fournisseur)) = tiers else {
        return Err(format!(
            "le tiers {fk_soc} n'existe pas dans l'entité {entity} : une commande fournisseur s'adresse à un tiers de cette entité"
        )
        .into());
    };
    if fournisseur != 1 {
        return Err(format!(
            "le tiers {fk_soc} n'est pas un fournisseur (llx_societe.fournisseur = 0) : cocher « fournisseur » sur sa fiche avant de le commander"
        )
        .into());
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

fn statut_actuel(
    conn: &mut mysql::PooledConn,
    entity: i64,
    rowid: i64,
) -> Result<i16, Box<dyn Error>> {
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_commande_fournisseur \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => Err(
            format!("la commande fournisseur {rowid} n'existe pas dans l'entité {entity}").into(),
        ),
        Some(s) => Ok(s as i16),
    }
}

/// Valide un brouillon : `fk_statut` passe de 0 à 1, `date_valid` est posée.
///
/// Valider deux fois n'est pas idempotent : c'est un appelant qui a perdu le fil
/// de son écran, et le laisser passer masquerait un double traitement.
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut != STATUT_BROUILLON {
        return Err(format!(
            "cette commande fournisseur est {} : seule une commande en brouillon se valide",
            libelle_statut(statut)
        )
        .into());
    }

    // La création refuse déjà zéro ligne, mais l'interface Dolibarr peut en
    // avoir supprimé une depuis : valider une pièce vide ferait une commande
    // à 0,00 € qui partirait quand même chez le fournisseur.
    let lignes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_commande_fournisseurdet WHERE fk_commande = :rowid",
        params! { "rowid" => rowid },
    )?;
    if lignes.unwrap_or(0) == 0 {
        return Err("une commande fournisseur sans ligne ne se valide pas".into());
    }

    conn.exec_drop(
        "UPDATE llx_commande_fournisseur SET fk_statut = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Approuve une commande validée : `fk_statut` passe de 1 à 2, `date_approve`
/// est posée.
///
/// C'est le geste qui engage la dépense. Il exige donc une pièce déjà validée :
/// approuver un brouillon reviendrait à signer un document dont personne n'a
/// vérifié les lignes.
pub fn approuver(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    match statut {
        STATUT_VALIDEE => {}
        STATUT_BROUILLON => {
            return Err(
                "cette commande fournisseur n'est pas validée : la valider avant de l'approuver"
                    .into(),
            )
        }
        autre => {
            return Err(format!(
                "cette commande fournisseur est déjà {} : elle n'a plus à être approuvée",
                libelle_statut(autre)
            )
            .into())
        }
    }

    conn.exec_drop(
        "UPDATE llx_commande_fournisseur SET fk_statut = :statut, date_approve = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_APPROUVEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Marque une commande approuvée comme partie chez le fournisseur :
/// `fk_statut` passe de 2 à 3 et `date_commande` reçoit la date d'envoi.
///
/// La date est fournie par l'appelant, pas prise au serveur : un bon de commande
/// antidaté ou envoyé la veille reste une réalité que la base doit pouvoir
/// enregistrer telle quelle.
pub fn commander(pool: &Pool, entity: i64, rowid: i64, date: &str) -> Result<(), Box<dyn Error>> {
    let date = date.trim();
    if !date_valide(date) {
        return Err(
            format!("date de commande « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }

    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    match statut {
        STATUT_APPROUVEE => {}
        STATUT_BROUILLON | STATUT_VALIDEE => {
            return Err(format!(
                "cette commande fournisseur est {} : l'approuver avant de la commander",
                libelle_statut(statut)
            )
            .into())
        }
        autre => {
            return Err(format!(
                "cette commande fournisseur est déjà {} : elle n'a plus à être commandée",
                libelle_statut(autre)
            )
            .into())
        }
    }

    conn.exec_drop(
        "UPDATE llx_commande_fournisseur SET fk_statut = :statut, date_commande = :date_cmd \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_COMMANDEE,
            "date_cmd" => date,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Constate l'arrivée de la marchandise : `fk_statut` passe de 3 à 4 et
/// `date_reception` reçoit la date d'entrée.
///
/// Dolibarr ne distingue pas la réception partielle de la totale à ce niveau :
/// le statut 4 couvre les deux, et le détail se lit dans les réceptions liées.
pub fn receptionner(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    date: &str,
) -> Result<(), Box<dyn Error>> {
    let date = date.trim();
    if !date_valide(date) {
        return Err(
            format!("date de réception « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }

    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    match statut {
        STATUT_COMMANDEE => {}
        STATUT_BROUILLON | STATUT_VALIDEE | STATUT_APPROUVEE => {
            return Err(format!(
                "cette commande fournisseur est {} : la commander avant de la réceptionner",
                libelle_statut(statut)
            )
            .into())
        }
        autre => {
            return Err(format!(
                "cette commande fournisseur est déjà {} : elle n'a plus à être réceptionnée",
                libelle_statut(autre)
            )
            .into())
        }
    }

    conn.exec_drop(
        "UPDATE llx_commande_fournisseur SET fk_statut = :statut, date_reception = :date_rec \
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
/// Une commande validée a déjà servi : elle prouve un engagement, et bientôt une
/// réception et une facture. La supprimer effacerait la preuve. La suppression
/// physique est donc réservée au brouillon.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut != STATUT_BROUILLON {
        return Err(format!(
            "une commande fournisseur {} ne se supprime pas : seul un brouillon se supprime",
            libelle_statut(statut)
        )
        .into());
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    // Les liens partent avec la pièce : un `llx_element_element` orphelin ferait
    // croire à une commande qui n'existe plus. Dolibarr nomme ce type
    // `order_supplier`.
    tx.exec_drop(
        "DELETE FROM llx_element_element \
         WHERE (targettype = 'order_supplier' AND fk_target = :rowid) \
            OR (sourcetype = 'order_supplier' AND fk_source = :rowid)",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_commande_fournisseurdet WHERE fk_commande = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_commande_fournisseur WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Quantité **commandée** : somme des quantités des lignes de la commande.
///
/// # Pourquoi ce n'est pas « reste à recevoir »
///
/// `llx_commande_fournisseur` ne porte **aucune** quantité déjà reçue : ni sur
/// l'en-tête, ni sur les lignes. Dolibarr ne la stocke pas davantage ; il la
/// **déduit** à la lecture, en rapprochant les réceptions liées ligne à ligne
/// dans `llx_receptiondet_batch` (une ligne de réception reprend une ligne de
/// commande via `element_type = 'commande_fournisseur'` et `fk_element` /
/// `fk_elementdet`).
///
/// Ce noyau ne peut donc pas calculer un reste à recevoir, pour deux raisons
/// vérifiables dans le code voisin :
///
/// - la création de commande n'écrit que l'en-tête et les lignes, jamais un lien
///   vers une réception — il n'y a rien à rapprocher ;
/// - `reception::create` écrit ses lignes en `element_type = 'reception'`, sans
///   référencer la commande : aucune réception n'est aujourd'hui rattachable à
///   une ligne de commande.
///
/// Un « reste à recevoir » calculé ici rendrait donc *toujours* la quantité
/// commandée, en prétendant que rien n'est arrivé — un chiffre faux qui aurait
/// l'air juste. Le nom de la fonction dit ce qu'elle prouve : la quantité
/// **commandée**. Le jour où les réceptions porteront leurs liens, une
/// `reste_a_recevoir()` pourra être écrite à côté, avec la soustraction — et non
/// en changeant le sens de celle-ci.
///
/// Les lignes de structure (`product_type = 9`) et les sous-totaux du module
/// Subtotal (`special_code = 104777`, qui portent `qty = 99` pour forcer
/// l'affichage) sont exclus : ils ne représentent aucune marchandise.
pub fn quantite_commandee(pool: &Pool, entity: i64, rowid: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_commande_fournisseur WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    if existe.is_none() {
        return Err(
            format!("la commande fournisseur {rowid} n'existe pas dans l'entité {entity}").into(),
        );
    }

    let total: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(d.qty), 0) AS CHAR) \
         FROM llx_commande_fournisseurdet d \
         JOIN llx_commande_fournisseur c ON c.rowid = d.fk_commande \
         WHERE c.entity = :entity AND d.fk_commande = :rowid \
           AND COALESCE(d.product_type, 0) <> :section \
           AND COALESCE(d.special_code, 0) <> :sous_total",
        params! {
            "entity" => entity,
            "rowid" => rowid,
            "section" => PRODUCT_TYPE_SECTION,
            "sous_total" => SPECIAL_CODE_SOUS_TOTAL,
        },
    )?;
    Ok(total
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .unwrap_or_default())
}

/// Un texte facultatif ramené à `None` s'il est vide : une chaîne blanche
/// stockée en base se relit comme une valeur, pas comme une absence.
fn texte_utile(valeur: Option<&str>) -> Option<&str> {
    valeur.map(str::trim).filter(|t| !t.is_empty())
}

/// Une date `AAAA-MM-JJ` réelle : `2026-02-31` existe au format mais pas au
/// calendrier, et MySQL le refuserait au milieu de la mise à jour.
///
/// Même règle que `reception` : la fonction y est privée, et ce module ne peut
/// pas l'importer sans élargir l'interface d'un module voisin pour un seul
/// contrôle de saisie.
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
