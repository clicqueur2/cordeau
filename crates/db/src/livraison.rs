//! Livraisons : `llx_delivery` + `llx_deliverydet`.
//!
//! Dolibarr a deux pièces voisines qu'on confond vite :
//!
//! - l'**expédition** (`llx_expedition`), qui prépare et fait sortir la
//!   marchandise, sans le moindre montant ;
//! - la **livraison** (`llx_delivery`, module « Livraisons »), qui atteste ce
//!   que le client a réellement reçu, **avec des montants** — c'est la pièce que
//!   le client signe et sur laquelle on s'appuie quand il conteste.
//!
//! C'est cette seconde pièce que porte ce module. Elle a donc des prix, une
//! quantité et un total HT, mais **aucune TVA** : `llx_deliverydet` n'a pas de
//! colonne `tva_tx`. Une livraison n'est pas un document fiscal, elle ne se
//! déclare pas : la TVA vit sur la commande et sur la facture, jamais ici.
//!
//! Les statuts sont ceux de Dolibarr : `0` brouillon, `1` validée, `2` livrée.
//! Le cycle est irréversible — une pièce validée ne redevient pas un brouillon,
//! une pièce livrée ne se « dé-livre » pas — et ce module n'expose donc aucune
//! transition inverse : une erreur de saisie se corrige en supprimant le
//! brouillon, pas en remontant le statut.
//!
//! # Le stock ne bouge pas, et c'est un manque assumé
//!
//! Dans Dolibarr, livrer de la marchandise **diminue** le stock : une ligne
//! dans `llx_stock_mouvement` et un nouveau `reel` dans `llx_product_stock`,
//! les deux dans la même transaction. Ce module n'écrit dans **aucune** des
//! deux tables, exactement comme `expedition` et `reception` avant lui, et pour
//! la même raison :
//!
//! - le noyau tient une règle — « le stock affiché est toujours la somme des
//!   mouvements, jamais une valeur saisie à part ». Écrire un niveau de stock
//!   sans le mouvement qui le justifie casse cette règle et laisse des
//!   quantités fantômes que personne ne peut expliquer, encore moins annuler ;
//! - `stock::mouvement` sait déjà faire les deux ensemble, mais il exige un
//!   entrepôt et un produit résolus, une unité, et surtout une **annulation
//!   symétrique**. Or `llx_deliverydet` ne porte **aucun** `fk_entrepot` : la
//!   livraison ne dit pas d'où part la marchandise. Décider à la place de
//!   l'utilisateur, c'est choisir un entrepôt au hasard et vider le mauvais.
//!
//! Donc : **la livraison est tracée, le mouvement de stock viendra avec le
//! module de mouvements dédié**. Tant qu'il n'existe pas, une livraison livrée
//! ne change pas le stock du produit. C'est écrit noir sur blanc ici pour que
//! personne ne prenne ce silence pour un oubli.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

/// Préfixe des références.
///
/// Dolibarr numérote `BL` + AAMM + 4 chiffres (`mod_livraison_jade`), mais ce
/// module de numérotation n'est pas porté : `divers::suivant_ref` tire donc
/// `BL0001`, `BL0002`… dans l'ordre d'insertion.
///
/// Sur une base déjà numérotée par Dolibarr, `suivant_ref` lit le plus grand
/// nombre trouvé après le préfixe — sur `BL2611-0001` il ne lit que `2611` — et
/// la suite repart de là : les numéros peuvent donc sauter, mais aucun doublon
/// n'est produit.
pub const PREFIX: &str = "BL";

/// Brouillon : modifiable, supprimable, sans effet sur le stock.
pub const STATUT_BROUILLON: i16 = 0;
/// Validée : la pièce est figée, la marchandise est annoncée au client.
pub const STATUT_VALIDEE: i16 = 1;
/// Livrée : le client a reçu, la pièce ne bouge plus.
pub const STATUT_LIVREE: i16 = 2;

/// Une livraison, avec le nom du tiers et les totaux de ses lignes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Livraison {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub fk_soc: i64,
    /// Raison sociale, relue par jointure : la liste s'affiche sans seconde
    /// requête par ligne.
    pub tiers: Option<String>,
    /// Référence du document chez le client (son bon de commande, par ex.).
    pub ref_customer: Option<String>,
    /// Référence libre de l'appelant, pour raccrocher la pièce à un système
    /// extérieur sans toucher à la numérotation Dolibarr.
    pub ref_ext: Option<String>,
    pub date_creation: Option<String>,
    pub date_valid: Option<String>,
    /// Date de livraison : **prévue** à la création, **réelle** après `livrer`
    /// (qui l'écrase, comme Dolibarr). Une seule colonne pour les deux, c'est
    /// le schéma qui le veut.
    pub date_livraison: Option<String>,
    pub fk_statut: i16,
    /// Somme des totaux de ligne, arrondie une seule fois. C'est ce que le PDF
    /// imprime ; `total()` le relit.
    pub total_ht: Decimal,
    pub nb_lignes: i64,
    /// Somme des quantités : utile au transporteur, qui compte des colis, pas
    /// des euros.
    pub quantite_totale: Decimal,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
}

impl Livraison {
    pub fn est_brouillon(&self) -> bool {
        self.fk_statut == STATUT_BROUILLON
    }

    pub fn est_validee(&self) -> bool {
        self.fk_statut == STATUT_VALIDEE
    }

    pub fn est_livree(&self) -> bool {
        self.fk_statut == STATUT_LIVREE
    }
}

/// Le statut en clair, pour les messages d'erreur : « livrée » se lit mieux que
/// `2` quand on cherche pourquoi une action est refusée.
pub fn libelle_statut(statut: i16) -> &'static str {
    match statut {
        STATUT_BROUILLON => "brouillon",
        STATUT_VALIDEE => "validée",
        STATUT_LIVREE => "livrée",
        _ => "dans un statut inconnu",
    }
}

/// Une ligne de livraison.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LigneLivraison {
    pub rowid: i64,
    pub fk_livraison: i64,
    pub fk_product: Option<i64>,
    pub description: Option<String>,
    pub qty: Decimal,
    /// Prix unitaire HT, conservé à 6 décimales (`price2num(..., 'MU')`).
    pub subprice: Decimal,
    /// Prix unitaire TTC. Tant qu'il n'y a pas de taux de TVA sur la ligne, il
    /// vaut le HT : c'est une égalité, pas un oubli.
    pub subprice_ttc: Decimal,
    /// `qty × subprice`, arrondi au centime une seule fois.
    pub total_ht: Decimal,
    pub rang: i32,
}

/// Ce qu'il faut pour créer une livraison.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewLivraison {
    pub fk_soc: i64,
    pub ref_customer: Option<String>,
    /// Date de livraison prévue. Facultative : un bon de livraison peut partir
    /// sans rendez-vous, et `livrer` posera de toute façon la date réelle.
    pub date_delivery: Option<String>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub lines: Vec<NewLigneLivraison>,
}

impl NewLivraison {
    pub fn new(fk_soc: i64) -> Self {
        Self {
            fk_soc,
            ..Default::default()
        }
    }
}

/// Une ligne à créer. Elle n'a pas de rang : l'ordre du vecteur fait foi.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewLigneLivraison {
    pub fk_product: Option<i64>,
    pub description: String,
    pub qty: Decimal,
    pub subprice: Decimal,
}

impl NewLigneLivraison {
    /// La description vient en premier, puis la quantité et le prix : c'est
    /// l'ordre dans lequel on remplit un bon de livraison devant le client.
    pub fn new(description: &str, qty: Decimal, subprice: Decimal) -> Self {
        Self {
            fk_product: None,
            description: description.to_string(),
            qty,
            subprice,
        }
    }
}

/// Une ligne après passage dans `money` : la saisie, plus le total que Dolibarr
/// en déduit. Calculée une seule fois, avant la transaction, pour qu'une erreur
/// d'arithmétique ne laisse pas d'en-tête sans lignes.
struct LigneCalculee {
    fk_product: Option<i64>,
    description: String,
    qty: Decimal,
    subprice: Decimal,
    total_ht: Decimal,
}

/// Colonnes relues, dans l'ordre attendu par `map_row`.
///
/// Le nombre de lignes et la quantité totale sont des sous-requêtes plutôt
/// qu'un `GROUP BY` : une pièce sans ligne doit apparaître (total à zéro) et
/// non disparaître de la liste, ce qu'un `JOIN` agrégeant ferait.
const SELECT_COLS: &str = "l.rowid, l.entity, l.ref, l.fk_soc, s.nom, l.ref_customer, \
     l.ref_ext, DATE_FORMAT(l.date_creation, '%Y-%m-%d'), \
     DATE_FORMAT(l.date_valid, '%Y-%m-%d'), \
     DATE_FORMAT(l.date_delivery, '%Y-%m-%d'), l.fk_statut, \
     CAST(l.total_ht AS CHAR), \
     (SELECT COUNT(*) FROM llx_deliverydet d WHERE d.fk_delivery = l.rowid), \
     (SELECT CAST(COALESCE(SUM(d.qty), 0) AS CHAR) FROM llx_deliverydet d \
      WHERE d.fk_delivery = l.rowid), \
     l.note_private, l.note_public";

const FROM: &str = "FROM llx_delivery l LEFT JOIN llx_societe s ON s.rowid = l.fk_soc";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

/// Les colonnes `double` ne se décodent pas en `Decimal` : tout passe par
/// `CAST(... AS CHAR)`, seule façon d'éviter un arrondi binaire sur une
/// quantité ou un prix.
fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Livraison {
    Livraison {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        fk_soc: opt_i(&row, 3).unwrap_or(0),
        tiers: opt_s(&row, 4),
        ref_customer: opt_s(&row, 5),
        ref_ext: opt_s(&row, 6),
        date_creation: opt_s(&row, 7),
        date_valid: opt_s(&row, 8),
        date_livraison: opt_s(&row, 9),
        fk_statut: opt_i(&row, 10).unwrap_or(0) as i16,
        total_ht: opt_dec(&row, 11).unwrap_or_default(),
        nb_lignes: opt_i(&row, 12).unwrap_or(0),
        quantite_totale: opt_dec(&row, 13).unwrap_or_default(),
        note_private: opt_s(&row, 14),
        note_public: opt_s(&row, 15),
    }
}

/// Livraisons de l'entité, de la plus récente à la plus ancienne.
///
/// Le tri porte sur `date_creation` avec `rowid` en second : deux livraisons
/// saisies le même jour gardent un ordre stable, sinon la pagination sauterait
/// des lignes d'une requête à l'autre.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Livraison>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} {FROM} WHERE l.entity = ? \
         ORDER BY l.date_creation DESC, l.rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limite, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Une livraison précise, ou `None` si elle n'existe pas dans cette entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Livraison>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} {FROM} WHERE l.entity = ? AND l.rowid = ?");
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// Lignes d'une livraison, dans l'ordre de saisie.
///
/// La jointure sur l'en-tête n'est pas décorative : les lignes n'ont pas de
/// colonne `entity` et ne peuvent donc pas être cloisonnées autrement.
pub fn lines(
    pool: &Pool,
    entity: i64,
    fk_livraison: i64,
) -> Result<Vec<LigneLivraison>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT d.rowid, d.fk_delivery, d.fk_product, d.description, \
                CAST(d.qty AS CHAR), CAST(d.subprice AS CHAR), \
                CAST(d.subprice_ttc AS CHAR), CAST(d.total_ht AS CHAR), d.rang \
         FROM llx_deliverydet d \
         JOIN llx_delivery l ON l.rowid = d.fk_delivery \
         WHERE l.entity = :entity AND d.fk_delivery = :id \
         ORDER BY d.rang ASC, d.rowid ASC",
        params! { "entity" => entity, "id" => fk_livraison },
    )?;
    Ok(rows
        .iter()
        .map(|r| LigneLivraison {
            rowid: opt_i(r, 0).unwrap_or(0),
            fk_livraison: opt_i(r, 1).unwrap_or(0),
            fk_product: opt_i(r, 2),
            description: opt_s(r, 3),
            qty: opt_dec(r, 4).unwrap_or_default(),
            subprice: opt_dec(r, 5).unwrap_or_default(),
            subprice_ttc: opt_dec(r, 6).unwrap_or_default(),
            total_ht: opt_dec(r, 7).unwrap_or_default(),
            rang: opt_i(r, 8).unwrap_or(0) as i32,
        })
        .collect())
}

/// Crée une livraison en brouillon et renvoie son `rowid`.
///
/// La création est tout ou rien : l'en-tête et ses lignes vivent dans la même
/// transaction. Un bon sans ligne n'est pas une pièce, seulement un numéro
/// consommé — d'où le refus, contrôlé avant toute écriture.
///
/// Aucune écriture de stock : voir l'avertissement en tête de module. La
/// livraison constate, elle ne décrémente pas.
pub fn create(pool: &Pool, entity: i64, new: &NewLivraison) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;

    let mut conn = pool.get_conn()?;
    // Le verrou nommé est porté par le serveur, pas par la connexion : il
    // sérialise la numérotation même si `suivant_ref` tire la référence sur une
    // autre connexion du pool (le pool par défaut en offre dix ; il en faut au
    // moins deux, sinon l'appel attendrait indéfiniment). Le libellé porte
    // l'entité, comme ailleurs dans le noyau.
    let verrou = format!("dolibarr_rs_livraison_{entity}");
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
    new: &NewLivraison,
) -> Result<i64, Box<dyn Error>> {
    verifier_tiers(conn, entity, new.fk_soc)?;

    let reference = crate::divers::suivant_ref(pool, "llx_delivery", "ref", PREFIX, entity)?;
    let lignes = calculer_lignes(&new.lines);
    let total_ht = totaux_lignes(&lignes);
    let date_livraison = texte_utile(new.date_delivery.as_deref());

    let mut tx = conn.start_transaction(TxOpts::default())?;
    let insere = tx.exec_drop(
        "INSERT INTO llx_delivery \
         (entity, ref, fk_soc, ref_customer, date_creation, date_delivery, \
          fk_statut, total_ht, note_private, note_public) \
         VALUES (:entity, :ref, :fk_soc, :ref_customer, NOW(), :date_livraison, \
          :statut, :total_ht, :note_private, :note_public)",
        params! {
            "entity" => entity,
            "ref" => reference.as_str(),
            "fk_soc" => new.fk_soc,
            "ref_customer" => new.ref_customer.as_deref(),
            "date_livraison" => date_livraison,
            "statut" => STATUT_BROUILLON,
            // Le total passe en texte : la colonne est un `double`, et un `f64`
            // en arithmétique est précisément ce que ce noyau refuse.
            "total_ht" => total_ht.to_string(),
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
                "la référence {reference} vient d'être prise par un autre écrivain : \
                 relancer la création"
            )
            .into());
        }
        return Err(e.into());
    }
    let rowid = tx.last_insert_id().unwrap_or(0) as i64;

    // Le rang suit l'ordre du vecteur : c'est lui qui fixe l'ordre d'impression
    // du bon, et il commence à 0 comme sur l'expédition.
    for (rang, ligne) in lignes.iter().enumerate() {
        tx.exec_drop(
            "INSERT INTO llx_deliverydet \
             (fk_delivery, fk_product, description, qty, subprice, subprice_ttc, \
              total_ht, rang) \
             VALUES (:fk_delivery, :fk_product, :description, :qty, :subprice, \
              :subprice_ttc, :total_ht, :rang)",
            params! {
                "fk_delivery" => rowid,
                "fk_product" => ligne.fk_product,
                "description" => ligne.description.as_str(),
                "qty" => ligne.qty.to_string(),
                "subprice" => ligne.subprice.to_string(),
                // Sans taux de TVA sur la ligne, le TTC vaut le HT. Écrire la
                // même valeur plutôt que 0 évite un total client à zéro si un
                // état lit cette colonne sans regarder l'autre.
                "subprice_ttc" => ligne.subprice.to_string(),
                "total_ht" => ligne.total_ht.to_string(),
                "rang" => rang as i32,
            },
        )?;
    }
    tx.commit()?;
    Ok(rowid)
}

/// Passe les lignes saisies dans `money`.
///
/// `subprice` est réécrit avec le prix unitaire **retenu par le calcul** (6
/// décimales) et non la saisie brute, puis le total est calculé **sur cette
/// valeur** : c'est ce qui permet à quiconque de refaire le total à la main en
/// multipliant la ligne affichée par la quantité. Un prix saisi à 12 décimales
/// et stocké tel quel donnerait un total que personne ne peut reproduire.
fn calculer_lignes(lignes: &[NewLigneLivraison]) -> Vec<LigneCalculee> {
    lignes
        .iter()
        .map(|l| {
            let unitaire = money::round_unit_price(l.subprice);
            LigneCalculee {
                fk_product: l.fk_product,
                description: l.description.trim().to_string(),
                qty: l.qty,
                subprice: unitaire,
                total_ht: money::round_money(unitaire * l.qty),
            }
        })
        .collect()
}

/// Total d'en-tête : somme des lignes, arrondie **une seule fois**.
fn totaux_lignes(lignes: &[LigneCalculee]) -> Decimal {
    let hts: Vec<Decimal> = lignes.iter().map(|l| l.total_ht).collect();
    money::sum(&hts)
}

/// Contrôles de forme, faits avant d'ouvrir la moindre connexion : une erreur
/// de saisie ne doit ni coûter un aller-retour SQL, ni consommer un numéro.
fn verifier_entree(new: &NewLivraison) -> Result<(), Box<dyn Error>> {
    if new.fk_soc <= 0 {
        return Err(
            "le tiers (fk_soc) est obligatoire : une livraison se livre à quelqu'un".into(),
        );
    }
    if new.lines.is_empty() {
        return Err(
            "une livraison sans ligne ne se crée pas : il faut au moins une marchandise".into(),
        );
    }
    // `date_delivery` est une colonne `datetime` : une chaîne mal formée ferait
    // échouer l'insertion en mode strict, sans dire à l'appelant ce qu'il a
    // écrit de travers.
    let fautive = texte_utile(new.date_delivery.as_deref()).filter(|d| !date_valide(d));
    if let Some(date) = fautive {
        return Err(
            format!("date de livraison « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }

    for (index, ligne) in new.lines.iter().enumerate() {
        let numero = index + 1;
        if ligne.qty <= Decimal::ZERO {
            return Err(format!(
                "ligne {numero} : la quantité doit être strictement positive (reçu {})",
                ligne.qty
            )
            .into());
        }
        // Un prix négatif n'est pas une remise : c'est un signe inversé, et il
        // sortirait un total d'en-tête inférieur à la somme des marchandises.
        if ligne.subprice < Decimal::ZERO {
            return Err(format!(
                "ligne {numero} : le prix unitaire ne peut pas être négatif (reçu {})",
                ligne.subprice
            )
            .into());
        }
        // Une ligne sans texte et sans produit serait illisible sur le bon : le
        // client ne saurait pas ce qu'il a reçu, ni sur quoi porter un litige.
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

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

/// Le statut courant, ou une erreur « n'existe pas » déjà rédigée.
///
/// Les quatre transitions ci-dessous ont toutes besoin de ce contrôle : le
/// factoriser évite que l'une d'elles l'oublie et écrive sur la pièce d'une
/// autre entité.
fn statut_actuel(
    conn: &mut mysql::PooledConn,
    entity: i64,
    rowid: i64,
) -> Result<i16, Box<dyn Error>> {
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_delivery WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => Err(format!("la livraison {rowid} n'existe pas dans l'entité {entity}").into()),
        Some(s) => Ok(s as i16),
    }
}

/// Valide un brouillon : `fk_statut` passe de 0 à 1, `date_valid` est posée.
///
/// Valider deux fois n'est pas une opération idempotente : c'est un appelant
/// qui a perdu le fil de son écran, et le laisser passer masquerait un double
/// traitement (deux bons de livraison pour une seule sortie de marchandise).
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut != STATUT_BROUILLON {
        return Err(format!(
            "cette livraison est {} : seule une livraison en brouillon se valide",
            libelle_statut(statut)
        )
        .into());
    }

    // Une pièce sans ligne se validerait pour ne rien livrer : le contrôle est
    // refait ici parce que la base est partagée avec l'interface Dolibarr, qui
    // peut avoir vidé les lignes entre la création et la validation.
    let lignes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_deliverydet WHERE fk_delivery = :rowid",
        params! { "rowid" => rowid },
    )?;
    if lignes.unwrap_or(0) == 0 {
        return Err("une livraison sans ligne ne se valide pas".into());
    }

    conn.exec_drop(
        "UPDATE llx_delivery SET fk_statut = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDEE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Marque une pièce validée comme livrée : statut 2 et date de livraison réelle.
///
/// Seule une pièce validée se livre : livrer un brouillon reviendrait à faire
/// signer au client une marchandise que personne n'a contrôlée, et le futur
/// mouvement de stock ne saurait plus à quelle pièce le rattacher.
///
/// `date` **écrase** la date de livraison prévue saisie à la création, comme le
/// fait `Delivery::setDelivered()` dans Dolibarr : la colonne ne garde que la
/// dernière vérité, celle du jour où le camion est passé.
///
/// Comme annoncé en tête de module, cette fonction **n'écrit pas** dans
/// `llx_product_stock` ni dans `llx_stock_mouvement` : elle constate la sortie,
/// elle ne la comptabilise pas.
pub fn livrer(pool: &Pool, entity: i64, rowid: i64, date: &str) -> Result<(), Box<dyn Error>> {
    let date = date.trim();
    // La colonne est un `datetime` : une chaîne mal formée ferait échouer la
    // requête en mode strict, sans dire à l'appelant ce qu'il a écrit de travers.
    if !date_valide(date) {
        return Err(
            format!("date de livraison « {date} » invalide : format attendu AAAA-MM-JJ").into(),
        );
    }

    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    match statut {
        STATUT_BROUILLON => {
            return Err("cette livraison n'est pas validée : la valider avant de la livrer".into())
        }
        STATUT_LIVREE => return Err("cette livraison est déjà livrée".into()),
        _ => {}
    }

    conn.exec_drop(
        "UPDATE llx_delivery SET fk_statut = :statut, date_delivery = :date_livraison \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_LIVREE,
            "date_livraison" => date,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Supprime un brouillon et ses lignes.
///
/// Une livraison validée ou livrée a déjà servi : elle prouve ce que le client
/// a reçu — et bientôt un mouvement de stock. Le supprimer effacerait la
/// preuve, y compris celle qu'on opposerait à une contestation. La suppression
/// physique est donc réservée au brouillon.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut != STATUT_BROUILLON {
        return Err(format!(
            "une livraison {} ne se supprime pas : seule une livraison en brouillon part",
            libelle_statut(statut)
        )
        .into());
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    // Les liens partent avec la pièce : un `llx_element_element` orphelin
    // ferait croire à une livraison qui n'existe plus.
    tx.exec_drop(
        "DELETE FROM llx_element_element WHERE targettype = 'delivery' AND fk_target = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_deliverydet WHERE fk_delivery = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_delivery WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Total HT d'une livraison, tel que le PDF l'imprime.
///
/// Il est relu sur l'en-tête — c'est-à-dire sur la valeur calculée une seule
/// fois à la création, à partir des lignes — et non recalculé ici : deux façons
/// de calculer un total finissent toujours par diverger, et un écran qui
/// n'additionne pas comme le document qu'il affiche est un piège. Pour
/// contrôler l'en-tête, on additionne `lines()` : le test le fait.
pub fn total(pool: &Pool, entity: i64, fk_livraison: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let row: Option<Row> = conn.exec_first(
        "SELECT CAST(total_ht AS CHAR) FROM llx_delivery \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_livraison },
    )?;
    let Some(row) = row else {
        return Err(
            format!("la livraison {fk_livraison} n'existe pas dans l'entité {entity}").into(),
        );
    };
    Ok(opt_dec(&row, 0).unwrap_or_default())
}

/// Un texte facultatif ramené à `None` s'il est vide : une chaîne blanche
/// stockée en base se relit comme une valeur, pas comme une absence.
fn texte_utile(valeur: Option<&str>) -> Option<&str> {
    valeur.map(str::trim).filter(|t| !t.is_empty())
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
