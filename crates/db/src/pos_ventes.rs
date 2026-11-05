//! Ventes de caisse : de la memoire a la base.
//!
//! [`pos::Panier`] vit en memoire : une vente disparait avec le processus, et
//! rien ne permet de la retrouver, de l'imprimer ni de la comptabiliser. Ce
//! module la fixe dans les tables Dolibarr, **sans creer de table nouvelle**.
//!
//! # Ce qui est ecrit, et ou
//!
//! | table | colonnes utilisees |
//! |---|---|
//! | `llx_facture` | `entity`, `ref`, `fk_soc`, `type`, `datec`, `datef`, `date_lim_reglement`, `fk_statut`, `paye`, `total_ht`, `total_tva`, `total_ttc`, `note_private`, `ref_ext`, `fk_cond_reglement` |
//! | `llx_facturedet` | `fk_facture`, `label`, `description`, `tva_tx`, `qty`, `remise_percent`, `subprice`, `price`, `total_ht`, `total_tva`, `total_ttc`, `product_type`, `rang`, `special_code`, `date_start`, `fk_unit` |
//! | `llx_paiement` | `ref`, `entity`, `datec`, `datep`, `amount`, `fk_paiement`, `note`, `statut` |
//! | `llx_paiement_facture` | `fk_paiement`, `fk_facture`, `amount` |
//! | `llx_bank` / `llx_bank_url` | ecrites par [`crate::payment::create`], jamais ici |
//! | `llx_c_paiement` | `id`, `code`, `libelle` (lecture seule) |
//! | `llx_c_units` | `rowid`, `code` (lecture seule) |
//!
//! Colonnes verifiees dans `schema-24.0.1.json` a la racine du depot, jamais
//! supposees.
//!
//! # La facture vient de `invoice::create`, pas d'un second createur
//!
//! Une vente de caisse **est** une facture client : type 0, une ligne par ligne
//! de panier, totaux calcules par le meme code que le reste du noyau. On ne
//! reecrit donc ni la numerotation, ni le calcul des totaux, ni l'ecriture
//! bancaire : [`invoice::create`] cree la facture et ses lignes,
//! [`payment::create`] enregistre l'encaissement.
//!
//! # La garde : `ref_ext`
//!
//! Rien, dans le schema, n'empeche deux ventes de porter les memes articles au
//! meme moment. La vente est donc marquee sur la facture elle-meme :
//!
//! ```text
//! ref_ext = POS-<AAAAMMJJ>-<HHMMSS>        (varchar(255), nullable)
//! ```
//!
//! La date est celle de la **vente**, pas le jour du traitement, et l'horodatage
//! vient de l'horloge du serveur de base. La cle est cherchee **avant** la
//! creation, sous un verrou consultatif MySQL nomme par entite (`GET_LOCK`) :
//! deux caisses de la meme entite sont serialisees, et si deux ventes tombent
//! dans la meme seconde, la seconde cle recoit un suffixe (`-2`, `-3`...). Une
//! vente se retrouve donc par sa reference externe, sans ambiguite, et
//! [`ventes_du_jour`] ne fait que lire ce prefixe.
//!
//! # Ce que la vente laisse derriere elle
//!
//! - **vente encaissee** (`paye >= total TTC`) : la facture est validee puis
//!   reglee. Dolibarr refuse de regler un brouillon
//!   ([`payment::create`] : « une facture en brouillon ne peut pas etre reglee :
//!   validez-la d'abord ») ; une vente de caisse dont l'argent est dans le tiroir
//!   est un document emis, pas un brouillon a relire. Elle sort donc **validee et
//!   payee**, ce qui est l'etat sans lequel elle ne serait pas comptabilisee ;
//! - **vente non encaissee** (`paye < total TTC`) : la facture reste **en
//!   brouillon**, sans reglement. C'est une vente a credit ou un encaissement
//!   incomplet : elle existe, elle est retrouvable, mais elle n'est pas soldee.
//!
//! Le rendu n'est jamais encaisse : le reglement porte le **TTC de la facture**,
//! pas la coupure tendue. Un client qui paie 100,00 EUR pour 71,96 EUR de
//! marchandise laisse 28,04 EUR de monnaie, et la caisse n'encaisse que 71,96.
//!
//! Aucun e-mail n'est envoye, aucun PDF n'est genere, aucune autre table
//! metier n'est touchee.
//!
//! # Le tiers
//!
//! Dolibarr n'a pas de facture sans tiers : `tiers = None` est **refuse**
//! explicitement plutot que remplace par un tiers invente. Une caisse doit
//! designer son client de passage (le « client comptoir » de l'entite).
//!
//! # Exemple
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let pool = db::pool("mysql://root:root@127.0.0.1:3306/dolibarr")?;
//! let mut panier = pos::Panier::new();
//! panier.ajouter(pos::Ligne::new(
//!     "ART-1",
//!     "Article a 19,99",
//!     rust_decimal::Decimal::from(3),
//!     rust_decimal::Decimal::new(1999, 2),
//!     rust_decimal::Decimal::from(20),
//! )?)?;
//! let facture = db::pos_ventes::enregistrer(
//!     &pool,
//!     1,
//!     Some(1),
//!     &panier,
//!     "especes",
//!     rust_decimal::Decimal::new(7196, 2),
//!     "2026-03-14",
//! )?;
//! println!("vente enregistree : facture n° {facture}");
//! # Ok(())
//! # }
//! ```

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, PooledConn, Row};
use rust_decimal::Decimal;

use crate::invoice::{self, NewInvoice, NewInvoiceLine};
use crate::payment::{self, NewPayment};

/// Prefixe de la reference externe qui identifie une vente de caisse.
pub const PREFIXE_REF_EXT: &str = "POS-";

/// Code Dolibarr de l'unite « piece » (`llx_c_units`) : une caisse vend des
/// articles a l'unite.
pub const CODE_UNITE_PIECE: &str = "P";

/// Prefixe du verrou consultatif, complete par entite.
const PREFIXE_VERROU: &str = "dolibarr_rs_pos_";

/// Attente maximale du verrou de caisse, en secondes.
const ATTENTE_VERROU_S: u32 = 10;

/// Combien de suffixes on accepte avant de renoncer a une reference unique.
///
/// Une caisse ne fait pas cent ventes dans la meme seconde ; au-dela, c'est un
/// signe que quelque chose ne tourne pas rond, pas une raison d'ecrire un
/// doublon.
const MAX_ESSAIS_REFERENCE: u32 = 100;

/// Une vente de caisse telle qu'elle est relue en base, pour le journal de
/// caisse et l'ecran de cloture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VenteCaisse {
    /// Facture qui porte la vente (`llx_facture.rowid`).
    pub facture_id: i64,
    /// Numero de la facture (`FA2611-0001`...).
    pub reference: String,
    /// Total TTC de la vente, tel qu'ecrit sur la facture.
    pub total_ttc: Decimal,
    /// Ce qui a ete reellement encaisse sur cette facture (somme des
    /// affectations de reglement).
    pub paye: Decimal,
    /// Code du ou des moyens de paiement (`ESP`, `CB`, `CHQ`...), joints par
    /// `+` quand la vente a ete reglee en plusieurs fois.
    pub moyen: String,
}

/// Un moyen de paiement resolu dans le dictionnaire Dolibarr.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Mode {
    id: i64,
    code: String,
}

/// Verrou consultatif MySQL, relache a la fin de la portee.
///
/// Le nom est propre a l'entite : deux caisses de deux entites differentes ne
/// s'attendent pas l'une l'autre.
struct Verrou {
    conn: PooledConn,
    nom: String,
}

impl Verrou {
    fn prendre(pool: &Pool, nom: &str) -> Result<Self, Box<dyn Error>> {
        let mut conn = pool.get_conn()?;
        let pris: Option<i64> =
            conn.exec_first("SELECT GET_LOCK(?, ?)", (nom, ATTENTE_VERROU_S))?;
        if pris != Some(1) {
            return Err(format!("verrou « {nom} » indisponible ({ATTENTE_VERROU_S} s)").into());
        }
        Ok(Self {
            conn,
            nom: nom.to_string(),
        })
    }
}

impl Drop for Verrou {
    fn drop(&mut self) {
        let _: Result<Option<i64>, mysql::Error> = self
            .conn
            .exec_first("SELECT RELEASE_LOCK(?)", (self.nom.as_str(),));
    }
}

/// Un montant lu en base : `CAST(... AS CHAR)` puis `Decimal`, pour ne pas
/// dependre du type flottant du pilote.
fn opt_dec(row: &Row, i: usize) -> Decimal {
    row.get_opt::<String, _>(i)
        .and_then(|r| r.ok())
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .unwrap_or_default()
}

fn opt_s(row: &Row, i: usize) -> String {
    row.get_opt::<String, _>(i)
        .and_then(|r| r.ok())
        .unwrap_or_default()
}

/// `2026-03-14` devient `20260314`. Toute autre forme est refusee : une date
/// approximative produirait une reference externe que personne ne saurait
/// relire.
fn date_compacte(date: &str) -> Result<String, Box<dyn Error>> {
    let d = date.trim();
    let octets = d.as_bytes();
    let valide = octets.len() == 10
        && octets[4] == b'-'
        && octets[7] == b'-'
        && octets
            .iter()
            .enumerate()
            .all(|(i, o)| i == 4 || i == 7 || o.is_ascii_digit());
    if !valide {
        return Err(format!("date invalide : « {date} » (attendu AAAA-MM-JJ)").into());
    }
    Ok(d.chars().filter(|c| *c != '-').collect())
}

/// Motif `LIKE` qui designe les ventes d'un jour.
fn motif(jour: &str) -> String {
    format!("{PREFIXE_REF_EXT}{jour}-%")
}

/// Reference externe d'une vente : `POS-<AAAAMMJJ>-<HHMMSS>`.
///
/// Sous verrou (voir [`enregistrer`]), donc la recherche « existe deja » et la
/// creation ne peuvent pas s'intercaler.
fn reference_vente(
    conn: &mut PooledConn,
    entity: i64,
    jour: &str,
) -> Result<String, Box<dyn Error>> {
    let heure: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%H%i%s')")?;
    let heure = heure.unwrap_or_else(|| "000000".to_string());
    let base = format!("{PREFIXE_REF_EXT}{jour}-{heure}");

    for essai in 0..MAX_ESSAIS_REFERENCE {
        let candidat = if essai == 0 {
            base.clone()
        } else {
            format!("{base}-{essai}")
        };
        let existe: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_facture \
             WHERE entity = :entity AND ref_ext = :ref_ext LIMIT 1",
            params! { "entity" => entity, "ref_ext" => candidat.as_str() },
        )?;
        if existe.is_none() {
            return Ok(candidat);
        }
    }
    Err(format!("impossible d'attribuer une reference de vente unique a {base}").into())
}

/// Resout le moyen de paiement saisi au comptoir dans `llx_c_paiement`.
///
/// Le libelle de la caisse est libre (« especes », « CB », « cheque »...) ; on
/// accepte aussi un identifiant numerique. Le dictionnaire est lu, jamais code
/// en dur : les identifiants dependent de l'installation, seuls les codes
/// Dolibarr (`ESP`, `CB`, `CHQ`, `VIR`, `PRE`) sont stables.
fn mode_paiement(pool: &Pool, moyen: &str) -> Result<Mode, Box<dyn Error>> {
    let cible = pos::normalise_moyen(moyen);
    if cible.is_empty() {
        return Err("moyen de paiement vide".into());
    }

    let modes = crate::dict::modes_paiement(pool)?;
    if modes.is_empty() {
        return Err("aucun mode de paiement actif dans llx_c_paiement".into());
    }

    if let Ok(id) = cible.parse::<i64>() {
        if let Some(m) = modes.iter().find(|m| m.id == id) {
            return Ok(Mode {
                id: m.id,
                code: m.code.clone(),
            });
        }
    }

    let candidats: &[&str] = match cible.as_str() {
        "especes" | "espece" | "cash" | "liquide" => &["ESP", "LIQ"],
        "cb" | "carte" | "carte bancaire" | "bancaire" | "visa" | "mastercard" => &["CB", "CRD"],
        "cheque" | "chq" => &["CHQ"],
        "virement" | "vir" | "transfert" => &["VIR"],
        "prelevement" | "prelev" => &["PRE"],
        _ => &[],
    };
    for code in candidats {
        if let Some(m) = modes.iter().find(|m| m.code.eq_ignore_ascii_case(code)) {
            return Ok(Mode {
                id: m.id,
                code: m.code.clone(),
            });
        }
    }

    // Dernier recours : le libelle du dictionnaire contient ce que la caisse a
    // saisi (« Carte bancaire » contient « carte »).
    if let Some(m) = modes
        .iter()
        .find(|m| pos::normalise_moyen(&m.libelle).contains(&cible))
    {
        return Ok(Mode {
            id: m.id,
            code: m.code.clone(),
        });
    }

    let connus: Vec<&str> = modes.iter().map(|m| m.code.as_str()).collect();
    Err(format!(
        "moyen de paiement inconnu : « {moyen} » (codes disponibles : {})",
        connus.join(", ")
    )
    .into())
}

/// Condition de reglement d'une vente de caisse : le comptant.
///
/// [`invoice::create`] resout la condition « de l'entite ou globale » ; une
/// entite de caisse recente n'en a aucune, et Dolibarr refuse alors d'ecrire la
/// facture (`MIN(rowid)` nul). Une vente se paie sur place : on prend donc une
/// condition de l'entite, sinon la globale, sinon la premiere active du
/// dictionnaire, en preferant le code `RECEP` (a reception). L'echeance reste le
/// jour de la vente, que [`enregistrer`] impose explicitement.
fn condition_comptant(pool: &Pool, entity: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let condition: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_c_payment_term WHERE active = 1 \
         ORDER BY CASE WHEN entity = :entity THEN 0 WHEN entity = 0 THEN 1 ELSE 2 END, \
                  CASE WHEN code = 'RECEP' THEN 0 ELSE 1 END, \
                  rowid ASC LIMIT 1",
        params! { "entity" => entity },
    )?;
    condition.ok_or_else(|| "aucune condition de reglement active dans llx_c_payment_term".into())
}

/// Pose la reference externe (la garde) sur la facture qui vient d'etre creee.
fn estampiller(
    pool: &Pool,
    entity: i64,
    facture: i64,
    ref_ext: &str,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_facture SET ref_ext = :ref_ext, tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "ref_ext" => ref_ext, "entity" => entity, "rowid" => facture },
    )?;
    if conn.affected_rows() == 0 {
        return Err(format!(
            "facture {facture} introuvable apres creation : reference externe non posee"
        )
        .into());
    }
    Ok(())
}

/// Pose l'unite « piece » sur chaque ligne de la facture.
///
/// [`NewInvoiceLine`] ne porte pas `fk_unit` : l'unite — celle qui s'imprime sur
/// le ticket — est donc ecrite apres coup, dans l'ordre de `rang`, qui est celui
/// du panier. Le nombre de lignes doit coincider : on ne devine pas a quoi
/// rattacher une unite.
fn poser_unites(
    pool: &Pool,
    entity: i64,
    facture: i64,
    attendues: usize,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let unite = crate::dict::unite_par_code(&mut conn, CODE_UNITE_PIECE)?.ok_or_else(|| {
        format!("l'unite « {CODE_UNITE_PIECE} » (piece) est absente de llx_c_units")
    })?;
    let lignes: Vec<i64> = conn.exec(
        "SELECT d.rowid FROM llx_facturedet d \
           INNER JOIN llx_facture f ON f.rowid = d.fk_facture \
          WHERE f.entity = :entity AND d.fk_facture = :facture \
          ORDER BY d.rang ASC, d.rowid ASC",
        params! { "entity" => entity, "facture" => facture },
    )?;
    if lignes.len() != attendues {
        return Err(format!(
            "{} lignes ecrites pour {attendues} lignes de panier : unites non posees",
            lignes.len()
        )
        .into());
    }
    for rowid in &lignes {
        conn.exec_drop(
            "UPDATE llx_facturedet SET fk_unit = :unite WHERE rowid = :rowid",
            params! { "unite" => unite, "rowid" => *rowid },
        )?;
    }
    Ok(())
}

/// Total TTC de la facture, tel qu'il a ete calcule par [`invoice::create`].
///
/// On relit le montant ecrit plutot que de recalculer : c'est lui qui fait foi
/// pour le reglement et pour la cloture.
fn total_facture(pool: &Pool, entity: i64, facture: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let total: Option<String> = conn.exec_first(
        "SELECT CAST(total_ttc AS CHAR) FROM llx_facture \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => facture },
    )?;
    let total = total.ok_or_else(|| format!("facture {facture} introuvable"))?;
    Ok(Decimal::from_str(total.trim()).unwrap_or_default())
}

/// Enregistre une vente de caisse en base et rend l'identifiant de sa facture.
///
/// - une facture client (type 0) est creee par [`invoice::create`], une ligne
///   par ligne de panier (libelle, quantite, prix HT, TVA, remise du panier),
///   puis l'unite « piece » est posee sur chaque ligne ;
/// - la reference externe `POS-<AAAAMMJJ>-<HHMMSS>` est posee sur la facture ;
/// - si `paye >= total TTC`, la vente est **validee puis reglee** par
///   [`payment::create`] : la caisse n'ecrit pas d'ecriture bancaire elle-meme.
///   Le reglement porte le TTC, jamais le rendu (voir l'entete du module) ;
/// - sinon la facture reste **en brouillon**, sans reglement.
///
/// `entity` cloisonne tout : une vente de l'entite 7 est invisible depuis une
/// autre entite.
///
/// # Erreurs
///
/// Date qui n'est pas au format `AAAA-MM-JJ`, panier vide, tiers absent, moyen
/// de paiement inconnu, unite « piece » absente du dictionnaire, verrou de
/// caisse indisponible et toute erreur MySQL remontee telle quelle.
///
/// Une erreur survenue **avant** la pose de la reference externe ne laisse rien
/// derriere : le brouillon eventuellement cree est retire. Une erreur survenue
/// **apres** laisse la vente en base, identifiable par sa reference externe.
pub fn enregistrer(
    pool: &Pool,
    entity: i64,
    tiers: Option<i64>,
    panier: &pos::Panier,
    moyen: &str,
    paye: Decimal,
    date: &str,
) -> Result<i64, Box<dyn Error>> {
    if entity <= 0 {
        return Err(format!("entite invalide : {entity}").into());
    }
    let fk_soc = tiers.ok_or(
        "une vente de caisse doit designer son tiers (client comptoir) : \
         Dolibarr n'a pas de facture sans tiers",
    )?;
    if panier.est_vide() {
        return Err("un panier vide ne produit pas de vente".into());
    }
    let jour = date_compacte(date)?;
    let mode = mode_paiement(pool, moyen)?;
    let remise = panier.remise_pct_actuelle();

    let mut new = NewInvoice::new(fk_soc);
    new.datef = Some(date.trim().to_string());
    // Une vente de caisse se regle sur place : l'echeance, c'est le jour meme.
    new.date_lim_reglement = Some(date.trim().to_string());
    new.date_prestation = Some(date.trim().to_string());
    new.fk_cond_reglement = Some(condition_comptant(pool, entity)?);
    new.note_private = Some(format!(
        "Vente de caisse du {} (moyen : {})",
        date.trim(),
        mode.code
    ));
    new.lines = panier
        .lignes()
        .iter()
        .map(|ligne| NewInvoiceLine {
            fk_product: None,
            label: ligne.libelle.clone(),
            // La reference de l'article reste sur la ligne : c'est elle qu'on
            // cherche quand un ticket est conteste.
            description: Some(ligne.reference.clone()),
            qty: ligne.quantite,
            unit_price: ligne.prix_unitaire_ht,
            tva_tx: ligne.taux_tva,
            // La remise du panier est globale ; la reporter sur chaque ligne
            // garde le total HT identique a celui du panier.
            remise_percent: remise,
            section: None,
        })
        .collect();

    // Le verrou couvre la recherche de reference **et** la creation : sans lui,
    // deux caisses pourraient tirer la meme cle avant que l'une des deux ne
    // l'ecrive.
    let mut verrou = Verrou::prendre(pool, &format!("{PREFIXE_VERROU}{entity}"))?;
    let ref_ext = reference_vente(&mut verrou.conn, entity, &jour)?;

    let facture = invoice::create(pool, entity, &new)?;
    if let Err(erreur) = estampiller(pool, entity, facture, &ref_ext) {
        // Sans sa cle, la vente echapperait a la garde : mieux vaut ne rien
        // avoir qu'une facture que personne ne reconnaitrait.
        let _ = invoice::delete(pool, entity, facture);
        return Err(erreur);
    }
    poser_unites(pool, entity, facture, panier.nombre_lignes())?;
    drop(verrou);

    // Le montant qui fait foi est celui de la facture, pas celui du panier :
    // les deux calculs different d'un centime sur certains taux, et c'est la
    // facture qui sera comptabilisee.
    let total = total_facture(pool, entity, facture)?;
    if total > Decimal::ZERO && paye >= total {
        // Une vente dont l'argent est encaisse est un document emis : Dolibarr
        // refuse de regler un brouillon, donc on la valide ici, puis on
        // l'encaisse. Aucun e-mail, aucun PDF.
        invoice::validate(pool, entity, facture)?;
        payment::create(
            pool,
            entity,
            &NewPayment {
                fk_facture: facture,
                montant: total,
                fk_paiement: mode.id,
                num_paiement: None,
                note: Some(format!("Encaissement caisse ({})", mode.code)),
                date: Some(date.trim().to_string()),
                fk_compte: None,
            },
        )?;
    }

    Ok(facture)
}

/// Les ventes de caisse d'un jour, dans l'ordre ou elles ont ete enregistrees.
///
/// Une vente est reconnue a sa reference externe `POS-<AAAAMMJJ>-...` : c'est
/// elle qui la distingue d'une facture ordinaire, sans table ni colonne
/// supplementaire. `date` est le jour de la vente (`AAAA-MM-JJ`), pas le jour ou
/// la question est posee.
pub fn ventes_du_jour(
    pool: &Pool,
    entity: i64,
    date: &str,
) -> Result<Vec<VenteCaisse>, Box<dyn Error>> {
    let jour = date_compacte(date)?;
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT f.rowid, f.ref, CAST(f.total_ttc AS CHAR), \
                CAST(COALESCE((SELECT SUM(pf.amount) \
                                 FROM llx_paiement_facture pf \
                                 INNER JOIN llx_paiement p ON p.rowid = pf.fk_paiement \
                                WHERE p.entity = f.entity AND pf.fk_facture = f.rowid), 0) \
                     AS CHAR), \
                COALESCE((SELECT GROUP_CONCAT(DISTINCT c.code ORDER BY c.code SEPARATOR '+') \
                            FROM llx_paiement_facture pf \
                            INNER JOIN llx_paiement p ON p.rowid = pf.fk_paiement \
                            LEFT JOIN llx_c_paiement c ON c.id = p.fk_paiement \
                           WHERE pf.fk_facture = f.rowid), '') \
           FROM llx_facture f \
          WHERE f.entity = :entity AND f.ref_ext LIKE :motif \
          ORDER BY f.rowid ASC",
        params! { "entity" => entity, "motif" => motif(&jour) },
    )?;
    Ok(rows
        .iter()
        .map(|row| VenteCaisse {
            facture_id: row.get_opt::<i64, _>(0).and_then(|r| r.ok()).unwrap_or(0),
            reference: opt_s(row, 1),
            total_ttc: opt_dec(row, 2),
            paye: opt_dec(row, 3),
            moyen: opt_s(row, 4),
        })
        .collect())
}

/// Ce que la caisse a encaisse d'un jour : `(somme TTC, nombre de ventes)`.
///
/// Un seul aller-retour : la caisse peut dire « j'ai encaisse X en N ventes »
/// sans relire les lignes des factures. Les ventes non encaissees comptent
/// aussi — elles existent, et c'est justement l'ecart que la cloture doit
/// montrer.
pub fn total_du_jour(
    pool: &Pool,
    entity: i64,
    date: &str,
) -> Result<(Decimal, usize), Box<dyn Error>> {
    let jour = date_compacte(date)?;
    let mut conn = pool.get_conn()?;
    let ligne: Option<(Option<String>, i64)> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(total_ttc), 0) AS CHAR), COUNT(*) \
           FROM llx_facture WHERE entity = :entity AND ref_ext LIKE :motif",
        params! { "entity" => entity, "motif" => motif(&jour) },
    )?;
    let (somme, nombre) = ligne.unwrap_or((None, 0));
    let somme = somme
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .unwrap_or_default();
    let nombre = usize::try_from(nombre).unwrap_or(0);
    Ok((somme, nombre))
}
