//! Propositions commerciales fournisseurs (`llx_supplier_proposal`) et leurs
//! lignes (`llx_supplier_proposaldet`).
//!
//! Une proposition fournisseur est une **demande de prix** : elle part de chez
//! nous vers un fournisseur, qui répond par un montant. C'est le frère jumeau de
//! `proposal` (`llx_propal`) à un retournement près : là on **propose** un prix
//! au client, ici on le **demande** au fournisseur. Le sens de la relation
//! commande tout le reste — c'est la fiche du fournisseur qui doit porter
//! `fournisseur = 1`, et c'est son prix qui viendra alimenter `buy_price_ht`.
//!
//! # Statuts
//!
//! Dolibarr en compte quatre, et le cycle ne revient jamais en arrière :
//!
//! | `fk_statut` | sens | fonction qui y mène |
//! |---|---|---|
//! | 0 | brouillon | `create` |
//! | 1 | validée (envoyée au fournisseur) | `valider` |
//! | 2 | close | `cloturer` |
//! | 3 | refusée | `refuser` |
//!
//! « Validée » ne veut pas dire « le fournisseur a répondu » : c'est le geste de
//! saisie, le moment où la demande est assez complète pour partir. La réponse
//! arrive plus tard, et c'est la clôture qui la constate.
//!
//! # Numérotation
//!
//! Dolibarr numérote `DF` + AAMM + 4 chiffres via son module de numérotation,
//! qui n'est pas porté : `divers::suivant_ref` tire donc `DF0001`, `DF0002`…
//! dans l'ordre d'insertion, sous un verrou nommé — deux créations simultanées
//! ne peuvent pas lire le même maximum.
//!
//! Sur une base déjà numérotée par Dolibarr, `suivant_ref` lit le plus grand
//! nombre trouvé **après le préfixe** : sur `DF2611-0001` il ne lit que `2611`,
//! et la suite repart de là. Les numéros peuvent donc sauter, mais aucun doublon
//! n'est produit — l'index unique `uk_supplier_proposal_ref (ref, entity)` le
//! garantit, et une collision est signalée à l'appelant au lieu d'être renommée
//! en silence.
//!
//! # Ce que veut dire « clôturée »
//!
//! Les statuts 2 et 3 posent tous deux `date_cloture`, et c'est voulu : dans les
//! deux cas la demande est **close**, on n'en attend plus rien. Ce qui les
//! distingue est le **résultat**, pas l'état : 2 = le prix est connu (le
//! fournisseur a répondu), 3 = le prix ne viendra pas (trop cher, abandon,
//! rupture de stock). La table ne porte aucune colonne qui dise lequel : un
//! lecteur qui cherche le montant doit lire les lignes, jamais l'en-tête — et
//! `cloturer` constate la fermeture sans prétendre avoir obtenu un prix.
//!
//! # Ce que ce module n'écrit pas, et pourquoi
//!
//! - `fk_user_author`, `fk_user_valid`, `fk_user_cloture` restent `NULL` : ce
//!   noyau ne connaît pas l'utilisateur courant, et une valeur inventée serait
//!   pire qu'une absence.
//! - `price`, `remise_percent`, `remise_absolue`, `remise` d'en-tête restent à
//!   `0` : cette pièce se saisit ligne à ligne, il n'y a pas de prix global ni
//!   de remise d'ensemble. Les remises sont ligne à ligne.
//! - `deposit_percent`, `model_pdf`, `last_main_doc`, `fk_account`,
//!   `fk_currency` / `multicurrency_*` restent à leur défaut : un acompte, un
//!   modèle de PDF ou une devise se décident dans l'interface, au moment de
//!   l'envoi, pas au moment de la saisie.
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

/// Préfixe des références (`DF` = demande fournisseur, + compteur sur 4 chiffres).
pub const PREFIX: &str = "DF";

/// Brouillon : modifiable, supprimable, jamais parti chez le fournisseur.
pub const STATUT_BROUILLON: i16 = 0;
/// Validée : la demande est complète et peut être envoyée.
pub const STATUT_VALIDEE: i16 = 1;
/// Close : le prix est connu, la demande n'attend plus rien.
pub const STATUT_CLOSE: i16 = 2;
/// Refusée : le prix ne viendra pas, la demande est abandonnée.
pub const STATUT_REFUSEE: i16 = 3;

/// Ligne de structure (titre de lot) : `product_type = 9`. Ce module n'en écrit
/// pas — une demande de prix est une liste de fournitures, pas un devis à lots —
/// mais une pièce retouchée dans l'interface Dolibarr peut en contenir, et il
/// faut savoir la reconnaître pour ne pas l'afficher comme une marchandise.
const PRODUCT_TYPE_SECTION: i16 = 9;
/// Code des lignes de sous-total du module Subtotal. Même raison : on ne les
/// écrit pas, on les signale.
const SPECIAL_CODE_SOUS_TOTAL: i32 = 104777;

/// Une demande de prix, avec le nom de son fournisseur.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropositionFournisseur {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    /// Le tiers interrogé. La colonne est nullable côté base, mais une demande
    /// sans destinataire n'a pas de sens : `create` l'exige.
    pub fk_soc: Option<i64>,
    /// Raison sociale relue par jointure, pour que la liste s'affiche sans une
    /// seconde requête par ligne. `None` si la fiche a disparu.
    pub fournisseur: Option<String>,
    pub fk_projet: Option<i64>,
    /// Date de saisie (`datec`), avec l'heure : deux demandes du même jour
    /// doivent rester ordonnables.
    pub datec: Option<String>,
    /// Date de validation : le jour où la demande est partie.
    pub date_valid: Option<String>,
    /// Date de fermeture, commune aux statuts 2 (close) et 3 (refusée).
    pub date_cloture: Option<String>,
    pub fk_statut: i16,
    pub total_ht: Option<Decimal>,
    pub total_tva: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    /// Remise d'en-tête, toujours `0` ici (les remises sont ligne à ligne).
    pub remise_percent: Option<Decimal>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    /// Condition de règlement (`llx_c_payment_term`).
    pub fk_cond_reglement: Option<i64>,
    /// Mode de règlement (`llx_c_paiement`).
    pub fk_mode_reglement: Option<i64>,
    /// Acompte éventuel, en pourcentage : la colonne est un `varchar(63)` que
    /// Dolibarr écrit en texte. Relu en `Decimal` quand la valeur est numérique.
    pub deposit_percent: Option<Decimal>,
    /// Date de livraison **souhaitée**, au format `AAAA-MM-JJ`.
    pub date_livraison: Option<String>,
}

impl PropositionFournisseur {
    pub fn est_brouillon(&self) -> bool {
        self.fk_statut == STATUT_BROUILLON
    }

    pub fn est_validee(&self) -> bool {
        self.fk_statut == STATUT_VALIDEE
    }

    pub fn est_close(&self) -> bool {
        self.fk_statut == STATUT_CLOSE
    }

    pub fn est_refusee(&self) -> bool {
        self.fk_statut == STATUT_REFUSEE
    }

    /// Fermée, quelle qu'en soit l'issue : close ou refusée.
    ///
    /// Les deux statuts portent `date_cloture` et interdisent les mêmes gestes.
    /// Un appelant qui teste « puis-je encore valider ? » doit poser cette
    /// question-là, pas comparer à 2 en oubliant le 3.
    pub fn est_fermee(&self) -> bool {
        self.est_close() || self.est_refusee()
    }
}

/// Une ligne de demande de prix.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LigneDemande {
    pub rowid: i64,
    pub fk_supplier_proposal: i64,
    pub fk_product: Option<i64>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub tva_tx: Option<Decimal>,
    pub qty: Option<Decimal>,
    pub remise_percent: Option<Decimal>,
    /// Prix unitaire hors taxe **tel qu'il a produit les totaux** (6 décimales),
    /// et non la saisie brute : c'est cette valeur qui explique le montant.
    pub subprice: Option<Decimal>,
    /// Prix unitaire stocké séparément par Dolibarr ; égal à `subprice` ici.
    pub price: Option<Decimal>,
    pub total_ht: Option<Decimal>,
    pub total_tva: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    /// Prix d'achat hors taxe : sur une demande de prix, c'est le montant qu'on
    /// propose de payer, donc le prix unitaire demandé.
    pub buy_price_ht: Option<Decimal>,
    pub product_type: i16,
    pub rang: i32,
    pub special_code: i32,
    /// Référence de l'article **chez le fournisseur** : c'est celle qu'il
    /// reconnaîtra dans son propre catalogue, et elle voyage avec la ligne.
    pub ref_fourn: Option<String>,
    /// Unité de la ligne (`llx_c_units`).
    pub fk_unit: Option<i64>,
}

impl LigneDemande {
    /// Titre de lot : à afficher en bande pleine largeur, hors totaux.
    pub fn is_section(&self) -> bool {
        self.product_type == PRODUCT_TYPE_SECTION
    }

    /// Sous-total du module Subtotal : un affichage, pas une fourniture.
    pub fn is_sous_total(&self) -> bool {
        self.special_code == SPECIAL_CODE_SOUS_TOTAL
    }
}

/// Ce qu'il faut pour créer une demande de prix.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewPropositionFournisseur {
    /// Le fournisseur interrogé. Il doit porter `fournisseur = 1`.
    pub fk_soc: i64,
    pub fk_projet: Option<i64>,
    /// Livraison **souhaitée** (facultative), au format `AAAA-MM-JJ`.
    pub date_livraison: Option<String>,
    pub fk_cond_reglement: Option<i64>,
    pub fk_mode_reglement: Option<i64>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub lines: Vec<NewLigneDemande>,
}

impl NewPropositionFournisseur {
    pub fn new(fk_soc: i64) -> Self {
        Self {
            fk_soc,
            ..Default::default()
        }
    }
}

/// Une ligne à créer. Elle n'a pas de rang : l'ordre du vecteur fait foi.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewLigneDemande {
    pub fk_product: Option<i64>,
    pub label: String,
    pub description: Option<String>,
    /// Taux de TVA en pourcentage, borné à `0..=100`.
    pub tva_tx: Decimal,
    /// Quantité, strictement positive.
    pub qty: Decimal,
    pub remise_percent: Decimal,
    /// Prix unitaire **hors taxe** demandé au fournisseur.
    pub subprice: Decimal,
    /// Référence du même article dans le catalogue du fournisseur.
    pub ref_fourn: Option<String>,
    pub fk_unit: Option<i64>,
}

impl NewLigneDemande {
    /// Une ligne au taux et à la quantité donnés, sans remise.
    ///
    /// L'ordre reprend celui de `NewProposalLine::new` et `NewLigneAchat::new` :
    /// libellé, quantité, prix unitaire, taux. Le taux vient en dernier parce
    /// qu'il est le seul qu'on oublie, et qu'un oubli se voit mieux en fin de
    /// liste.
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

/// Une ligne après passage dans `money` : la saisie, plus les trois montants que
/// Dolibarr en déduit. Calculée une seule fois, avant la transaction, pour
/// qu'une erreur d'arithmétique ne laisse pas d'en-tête sans lignes.
struct LigneCalculee {
    fk_product: Option<i64>,
    label: String,
    description: Option<String>,
    tva_tx: Decimal,
    qty: Decimal,
    remise_percent: Decimal,
    subprice: Decimal,
    product_type: i16,
    ref_fourn: Option<String>,
    fk_unit: Option<i64>,
    total_ht: Decimal,
    total_tva: Decimal,
    total_ttc: Decimal,
}

/// Colonnes relues, dans l'ordre attendu par `map_row`.
const SELECT_COLS: &str = "p.rowid, p.entity, p.ref, p.fk_soc, s.nom, p.fk_projet, \
     DATE_FORMAT(p.datec, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(p.date_valid, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(p.date_cloture, '%Y-%m-%d %H:%i:%s'), p.fk_statut, \
     CAST(p.total_ht AS CHAR), CAST(p.total_tva AS CHAR), CAST(p.total_ttc AS CHAR), \
     CAST(p.remise_percent AS CHAR), p.note_private, p.note_public, \
     p.fk_cond_reglement, p.fk_mode_reglement, CAST(p.deposit_percent AS CHAR), \
     DATE_FORMAT(p.date_livraison, '%Y-%m-%d')";

const FROM: &str = "FROM llx_supplier_proposal p \
     LEFT JOIN llx_societe s ON s.rowid = p.fk_soc";

/// `llx_supplier_proposaldet` n'a pas de colonne `entity` : le cloisonnement
/// passe par la jointure sur l'en-tête, d'où l'alias.
const LINE_COLS: &str = "d.rowid, d.fk_supplier_proposal, d.fk_product, d.label, \
     d.description, CAST(d.tva_tx AS CHAR), CAST(d.qty AS CHAR), \
     CAST(d.remise_percent AS CHAR), CAST(d.subprice AS CHAR), CAST(d.price AS CHAR), \
     CAST(d.total_ht AS CHAR), CAST(d.total_tva AS CHAR), CAST(d.total_ttc AS CHAR), \
     CAST(d.buy_price_ht AS CHAR), d.product_type, d.rang, d.special_code, \
     d.ref_fourn, d.fk_unit";

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

fn map_row(row: Row) -> PropositionFournisseur {
    PropositionFournisseur {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        fk_soc: opt_i(&row, 3),
        fournisseur: opt_s(&row, 4),
        fk_projet: opt_i(&row, 5),
        datec: opt_s(&row, 6),
        date_valid: opt_s(&row, 7),
        date_cloture: opt_s(&row, 8),
        fk_statut: opt_i(&row, 9).unwrap_or(0) as i16,
        total_ht: opt_dec(&row, 10),
        total_tva: opt_dec(&row, 11),
        total_ttc: opt_dec(&row, 12),
        remise_percent: opt_dec(&row, 13),
        note_private: opt_s(&row, 14),
        note_public: opt_s(&row, 15),
        fk_cond_reglement: opt_i(&row, 16),
        fk_mode_reglement: opt_i(&row, 17),
        deposit_percent: opt_dec(&row, 18),
        date_livraison: opt_s(&row, 19),
    }
}

fn map_ligne(row: Row) -> LigneDemande {
    LigneDemande {
        rowid: opt_i(&row, 0).unwrap_or(0),
        fk_supplier_proposal: opt_i(&row, 1).unwrap_or(0),
        fk_product: opt_i(&row, 2),
        label: opt_s(&row, 3),
        description: opt_s(&row, 4),
        tva_tx: opt_dec(&row, 5),
        qty: opt_dec(&row, 6),
        remise_percent: opt_dec(&row, 7),
        subprice: opt_dec(&row, 8),
        price: opt_dec(&row, 9),
        total_ht: opt_dec(&row, 10),
        total_tva: opt_dec(&row, 11),
        total_ttc: opt_dec(&row, 12),
        buy_price_ht: opt_dec(&row, 13),
        product_type: opt_i(&row, 14).unwrap_or(0) as i16,
        rang: opt_i(&row, 15).unwrap_or(0) as i32,
        special_code: opt_i(&row, 16).unwrap_or(0) as i32,
        ref_fourn: opt_s(&row, 17),
        fk_unit: opt_i(&row, 18),
    }
}

/// Le statut en clair, pour les messages d'erreur : « refusée » se lit mieux que
/// `3` quand on cherche pourquoi un geste est rejeté.
pub fn libelle_statut(statut: i16) -> &'static str {
    match statut {
        STATUT_BROUILLON => "brouillon",
        STATUT_VALIDEE => "validée",
        STATUT_CLOSE => "close",
        STATUT_REFUSEE => "refusée",
        _ => "dans un statut inconnu",
    }
}

/// Demandes de l'entité, de la plus récente à la plus ancienne.
///
/// Le tri porte sur `datec` avec `rowid` en second : deux demandes saisies le
/// même jour gardent un ordre stable, sinon la pagination sauterait des lignes
/// d'une requête à l'autre.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<PropositionFournisseur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} {FROM} WHERE p.entity = ? \
         ORDER BY p.datec DESC, p.rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limite, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Une demande précise, ou `None` si elle n'existe pas dans cette entité.
pub fn get(
    pool: &Pool,
    entity: i64,
    rowid: i64,
) -> Result<Option<PropositionFournisseur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} {FROM} WHERE p.entity = ? AND p.rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// Lignes d'une demande, dans l'ordre de saisie.
pub fn lines(pool: &Pool, entity: i64, rowid: i64) -> Result<Vec<LigneDemande>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {LINE_COLS} FROM llx_supplier_proposaldet d \
         JOIN llx_supplier_proposal p ON p.rowid = d.fk_supplier_proposal \
         WHERE p.entity = ? AND d.fk_supplier_proposal = ? \
         ORDER BY d.rang ASC, d.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, rowid))?;
    Ok(rows.into_iter().map(map_ligne).collect())
}

/// Crée une demande de prix en brouillon et renvoie son `rowid`.
///
/// Tout est vérifié avant la moindre écriture : un numéro consommé pour une
/// demande refusée serait un trou dans la numérotation, et une pièce sans ligne
/// ne serait qu'une enveloppe vide envoyée à un fournisseur.
///
/// Le verrou nommé est porté par le serveur et non par la connexion : il
/// sérialise la numérotation même si `suivant_ref` tire la référence sur une
/// autre connexion du pool (le pool en offre dix ; il en faut au moins deux,
/// sinon l'appel attendrait indéfiniment).
pub fn create(
    pool: &Pool,
    entity: i64,
    new: &NewPropositionFournisseur,
) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;

    let mut conn = pool.get_conn()?;
    let verrou = format!("dolibarr_rs_proposition_fournisseur_{entity}");
    let pris: Option<i64> = conn.exec_first("SELECT GET_LOCK(?, 10)", (verrou.as_str(),))?;
    if pris != Some(1) {
        return Err(
            "verrou de numérotation indisponible (10 s) : une autre création est en cours".into(),
        );
    }

    let resultat = creer_sous_verrou(&mut conn, pool, entity, new);
    let _: Option<i64> = conn.exec_first("SELECT RELEASE_LOCK(?)", (verrou.as_str(),))?;
    resultat
}

fn creer_sous_verrou(
    conn: &mut mysql::PooledConn,
    pool: &Pool,
    entity: i64,
    new: &NewPropositionFournisseur,
) -> Result<i64, Box<dyn Error>> {
    verifier_fournisseur(conn, entity, new.fk_soc)?;
    if let Some(projet) = new.fk_projet {
        verifier_projet(conn, entity, projet)?;
    }

    let reference =
        crate::divers::suivant_ref(pool, "llx_supplier_proposal", "ref", PREFIX, entity)?;
    let lignes = preparer_lignes(conn, entity, &new.lines)?;
    let (total_ht, total_tva, total_ttc) = totaux_lignes(&lignes);

    let mut tx = conn.start_transaction(TxOpts::default())?;
    let insere = tx.exec_drop(
        "INSERT INTO llx_supplier_proposal \
         (entity, ref, fk_soc, fk_projet, datec, date_livraison, fk_statut, \
          remise_percent, total_ht, total_tva, total_ttc, note_private, \
          note_public, fk_cond_reglement, fk_mode_reglement) \
         VALUES (:entity, :ref, :fk_soc, :fk_projet, NOW(), :date_livraison, \
          :statut, 0, :total_ht, :total_tva, :total_ttc, :note_private, \
          :note_public, :cond, :mode)",
        params! {
            "entity" => entity,
            "ref" => reference.as_str(),
            "fk_soc" => new.fk_soc,
            "fk_projet" => new.fk_projet,
            "date_livraison" => texte_utile(new.date_livraison.as_deref()),
            "statut" => STATUT_BROUILLON,
            "total_ht" => total_ht.to_string(),
            "total_tva" => total_tva.to_string(),
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
            "INSERT INTO llx_supplier_proposaldet \
             (fk_supplier_proposal, fk_product, label, description, tva_tx, qty, \
              remise_percent, subprice, price, total_ht, total_tva, total_ttc, \
              buy_price_ht, product_type, rang, special_code, ref_fourn, fk_unit) \
             VALUES (:fk_supplier_proposal, :fk_product, :label, :description, \
              :tva_tx, :qty, :remise_percent, :subprice, :subprice, :total_ht, \
              :total_tva, :total_ttc, :subprice, :product_type, :rang, 0, \
              :ref_fourn, :fk_unit)",
            params! {
                "fk_supplier_proposal" => rowid,
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
                "product_type" => ligne.product_type,
                "rang" => (rang + 1) as i32,
                "ref_fourn" => ligne.ref_fourn.as_deref(),
                "fk_unit" => ligne.fk_unit,
            },
        )?;
    }
    tx.commit()?;
    Ok(rowid)
}

/// Transforme les lignes demandées en lignes à écrire.
///
/// `subprice` est réécrit avec le prix unitaire **retenu par le calcul** (6
/// décimales) et non la saisie brute : c'est la valeur qui, multipliée par la
/// quantité, redonne le total affiché. Il alimente aussi `price` et
/// `buy_price_ht` — sur une demande de prix, le prix demandé **est** le prix
/// d'achat espéré, et laisser `buy_price_ht` à zéro obligerait tout lecteur à
/// refaire la correspondance.
fn preparer_lignes(
    conn: &mut mysql::PooledConn,
    entity: i64,
    lignes: &[NewLigneDemande],
) -> Result<Vec<LigneCalculee>, Box<dyn Error>> {
    let mut pretes: Vec<LigneCalculee> = Vec::with_capacity(lignes.len());

    for l in lignes {
        let t = line_totals(&LineInput::new(
            &l.qty.to_string(),
            &l.subprice.to_string(),
            &l.remise_percent.to_string(),
            &l.tva_tx.to_string(),
        ));

        // `product_type` distingue un produit (0) d'un service (1) : il décide
        // de la colonne « quantité » sur le PDF. On le lit sur la fiche article
        // plutôt que de le supposer, sinon un service s'imprimerait avec une
        // quantité à commander.
        let product_type = match l.fk_product {
            None => 0i16,
            Some(id) => {
                let pt: Option<i64> = conn.exec_first(
                    "SELECT fk_product_type FROM llx_product \
                     WHERE entity = :entity AND rowid = :rowid",
                    params! { "entity" => entity, "rowid" => id },
                )?;
                match pt {
                    Some(v) => v as i16,
                    // `fk_product = 0` n'est pas un produit absent : c'est une
                    // **ligne libre**, la façon dont Dolibarr écrit une
                    // fourniture sans fiche article. La refuser rendrait toute
                    // demande hors catalogue impossible.
                    None if id <= 0 => 0i16,
                    None => {
                        return Err(
                            format!("le produit {id} n'existe pas dans l'entité {entity}").into(),
                        )
                    }
                }
            }
        };

        pretes.push(LigneCalculee {
            fk_product: l.fk_product,
            label: l.label.trim().to_string(),
            description: texte_utile(l.description.as_deref()).map(str::to_string),
            tva_tx: l.tva_tx,
            qty: l.qty,
            remise_percent: l.remise_percent,
            subprice: t.unit_price_ht,
            product_type,
            ref_fourn: texte_utile(l.ref_fourn.as_deref()).map(str::to_string),
            fk_unit: l.fk_unit,
            total_ht: t.total_ht,
            total_tva: t.total_vat,
            total_ttc: t.total_ttc,
        });
    }
    Ok(pretes)
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
fn verifier_entree(new: &NewPropositionFournisseur) -> Result<(), Box<dyn Error>> {
    if new.fk_soc <= 0 {
        return Err(
            "le fournisseur (fk_soc) est obligatoire : une demande de prix s'adresse à un tiers"
                .into(),
        );
    }
    if new.lines.is_empty() {
        return Err(
            "une demande de prix sans ligne ne se crée pas : il n'y aurait rien à faire chiffrer"
                .into(),
        );
    }
    // `date_livraison` est une colonne `date` : une chaîne mal formée ferait
    // échouer l'insertion en mode strict, sans dire à l'appelant ce qu'il a
    // écrit de travers.
    let fautive = texte_utile(new.date_livraison.as_deref()).filter(|d| !date_valide(d));
    if let Some(date) = fautive {
        return Err(
            format!("date de livraison « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }

    // Un pourcentage acceptable : 0 à 100 inclus. Un taux négatif rendrait un
    // TTC inférieur au HT, et au-delà de 100 % la TVA dépasserait la
    // marchandise : ce sont des fautes de saisie, pas des cas à laisser filer
    // jusqu'au PDF envoyé au fournisseur.
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
        // ne saurait pas quoi chiffrer.
        if ligne.label.trim().is_empty() && ligne.fk_product.is_none() {
            return Err(format!("ligne {numero} : il faut un libellé ou un produit").into());
        }
    }
    Ok(())
}

/// Le tiers interrogé doit exister **et** être un fournisseur.
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
            "le tiers {fk_soc} n'existe pas dans l'entité {entity} : une demande de prix s'adresse à un fournisseur de cette entité"
        )
        .into());
    };
    if fournisseur != 1 {
        return Err(format!(
            "le tiers {fk_soc} n'est pas un fournisseur (llx_societe.fournisseur = 0) : cocher « fournisseur » sur sa fiche avant de lui demander un prix"
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
        "SELECT fk_statut FROM llx_supplier_proposal \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => Err(
            format!("la demande fournisseur {rowid} n'existe pas dans l'entité {entity}").into(),
        ),
        Some(s) => Ok(s as i16),
    }
}

/// Valide un brouillon : `fk_statut` passe de 0 à 1, `date_valid` est posée.
///
/// C'est le moment où la demande est jugée complète et peut partir. Valider deux
/// fois n'est pas idempotent : c'est un appelant qui a perdu le fil de son écran,
/// et le laisser passer masquerait un double envoi au fournisseur.
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut != STATUT_BROUILLON {
        return Err(format!(
            "cette demande fournisseur est {} : seule une demande en brouillon se valide",
            libelle_statut(statut)
        )
        .into());
    }

    // La création refuse déjà zéro ligne, mais l'interface Dolibarr peut en avoir
    // supprimé une depuis : valider une pièce vide ferait une demande à 0,00 €
    // qui partirait quand même chez le fournisseur.
    let lignes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_supplier_proposaldet WHERE fk_supplier_proposal = :rowid",
        params! { "rowid" => rowid },
    )?;
    if lignes.unwrap_or(0) == 0 {
        return Err("une demande fournisseur sans ligne ne se valide pas".into());
    }

    conn.exec_drop(
        "UPDATE llx_supplier_proposal SET fk_statut = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Clôt une demande validée : `fk_statut` passe de 1 à 2, `date_cloture` est
/// posée.
///
/// « Clôturée » veut dire que **la demande est close** — on n'en attend plus
/// rien — et non que le prix est connu. Le prix connu et l'abandon finissent au
/// même endroit, la table ne porte aucune colonne qui les distingue : à
/// l'appelant de choisir `cloturer` (le fournisseur a répondu) ou `refuser` (il
/// ne répondra pas), et au lecteur d'aller chercher le montant dans les lignes.
pub fn cloturer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    match statut {
        STATUT_VALIDEE => {}
        STATUT_BROUILLON => {
            return Err(
                "cette demande fournisseur n'est pas validée : la valider avant de la clore".into(),
            )
        }
        autre => {
            return Err(format!(
                "cette demande fournisseur est déjà {} : elle n'a plus à être close",
                libelle_statut(autre)
            )
            .into())
        }
    }

    conn.exec_drop(
        "UPDATE llx_supplier_proposal SET fk_statut = :statut, date_cloture = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_CLOSE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Refuse une demande validée : `fk_statut` passe de 1 à 3, `date_cloture` est
/// posée.
///
/// Même date que la clôture, parce que c'est le même fait : la demande est
/// fermée. Seul le résultat diffère — ici, le prix ne viendra pas (trop cher,
/// rupture, abandon du chantier).
pub fn refuser(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    match statut {
        STATUT_VALIDEE => {}
        STATUT_BROUILLON => {
            return Err(
                "cette demande fournisseur n'est pas validée : la valider avant de la refuser"
                    .into(),
            )
        }
        autre => {
            return Err(format!(
                "cette demande fournisseur est déjà {} : elle n'a plus à être refusée",
                libelle_statut(autre)
            )
            .into())
        }
    }

    conn.exec_drop(
        "UPDATE llx_supplier_proposal SET fk_statut = :statut, date_cloture = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_REFUSEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Supprime un brouillon et ses lignes.
///
/// Une demande validée est déjà partie chez un fournisseur : elle prouve qu'on
/// l'a interrogé, et bientôt le prix qu'il a donné. La supprimer effacerait
/// cette trace. La suppression physique est donc réservée au brouillon.
///
/// Rien d'autre n'est effacé : ce module n'écrit ni lien `llx_element_element`,
/// ni catégorie `llx_categorie_supplier_proposal`, ni extrachamp. Une pièce
/// retouchée dans l'interface Dolibarr pourrait en porter — ils resteraient
/// alors orphelins, ce qui est le prix d'un module qui ne touche que ses
/// propres tables.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut != STATUT_BROUILLON {
        return Err(format!(
            "une demande fournisseur {} ne se supprime pas : seul un brouillon se supprime",
            libelle_statut(statut)
        )
        .into());
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    // Les lignes d'abord : `llx_supplier_proposaldet.fk_supplier_proposal` porte
    // une clé étrangère vers l'en-tête, l'ordre inverse échouerait.
    tx.exec_drop(
        "DELETE FROM llx_supplier_proposaldet WHERE fk_supplier_proposal = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_supplier_proposal WHERE entity = :entity AND rowid = :rowid",
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

/// Une date `AAAA-MM-JJ` réelle : `2026-02-31` existe au format mais pas au
/// calendrier, et MySQL le refuserait au milieu de l'insertion.
///
/// Même règle que `commande_fournisseur` : la fonction y est privée, et ce
/// module ne peut pas l'importer sans élargir l'interface d'un module voisin
/// pour un seul contrôle de saisie.
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
