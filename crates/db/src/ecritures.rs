//! Pont entre les documents et le moteur de partie double.
//!
//! `crate::grand_livre` **derive** des lignes a partir des documents : c'est une
//! lecture, pas une tenue. Le crate `comptabilite` sait tenir une ecriture, mais
//! il ne connait que des comptes et des montants : pas de facture, pas de ligne
//! de facture, pas de taux de TVA. Ce module est le pont. Il lit un document
//! valide, le ventile en comptes, et rend une ecriture que
//! `comptabilite::verifier` accepte ou refuse.
//!
//! # Le plan de comptes est celui du noyau, pas un second
//!
//! Les comptes sont **ceux de `crate::grand_livre`** : 411000, 401000, 706000,
//! 601000, 445710, 445660. Deux consequences assumees :
//!
//! - les ventes vont toutes en **706000** « Prestations de services », y compris
//!   les lignes de marchandise (`product_type = 0`). Distinguer 701000 ou 707000
//!   obligerait le grand livre derive a suivre, sinon les deux livres ne se
//!   rapprocheraient plus compte par compte ;
//! - les achats vont tous en **601000** « Achats », y compris les achats non
//!   stockes de matieres (606), pour la meme raison.
//!
//! La TVA collectee va en 445710 et la deductible en 445660, **sans sous-compte
//! par taux** : le plan du noyau n'en a pas.
//!
//! # La TVA est ventilee par taux, sur un seul compte
//!
//! Une facture a 20 % et a 10 % produit **deux lignes** de TVA de meme compte
//! 445710, une par taux, le taux ecrit dans le libelle. C'est ce qu'un comptable
//! relit pour remplir une CA3, et cela evite d'inventer des sous-comptes. La
//! somme des lignes de TVA vaut la TVA portee par l'en-tete.
//!
//! # Le numero d'ecriture est stable
//!
//! `VE-<reference>` pour une vente, `AC-<reference>` pour un achat. Deux appels
//! successifs sur la meme facture rendent le meme numero : c'est ce qui permettra
//! a l'appelant de refuser le doublon (`comptabilite::Erreur::DejaAjoutee`) sans
//! avoir a tenir un registre. Les references Dolibarr sont uniques **par
//! entite** : un grand livre ne doit donc pas melanger deux entites, ce qui est
//! de toute facon la regle de cloisonnement.
//!
//! # Ce que ce pont ne fait pas
//!
//! - **aucun a-nouveau** : une ecriture par document, rien avant la premiere
//!   facture. La reprise des soldes d'ouverture reste a la charge de l'appelant ;
//! - **aucun lettrage** : la creance 411 d'une facture n'est pas rapprochee de
//!   son encaissement. Le moteur fournit `Lettrage`, c'est l'appelant qui le
//!   mene ;
//! - **aucune cloture**, aucun exercice, aucun resultat, aucune extourne ;
//! - **aucune ecriture de banque** : `ecritures_du_journal` ne produit que `VE`
//!   et `AC`. Les reglements (`BQ`) et les operations diverses (`OD`, `AN`)
//!   restent hors de son perimetre ;
//! - **aucune persistance** : les ecritures sont rendues, pas ecrites. Rien
//!   n'entre dans `llx_accounting_bookkeeping` ; `crate::fec` continue d'alimenter
//!   sa propre table a partir de son journal derive.

use std::collections::BTreeMap;
use std::error::Error;

use comptabilite::{Ecriture, Ligne};
use mysql::prelude::*;
use mysql::{params, Pool, Row};
use rust_decimal::Decimal;

use crate::invoice::{
    STATUT_ABANDONNEE, STATUT_BROUILLON, STATUT_PAYEE, STATUT_VALIDEE, TYPE_AVOIR,
};

// ---------------------------------------------------------------------------
// Plan de comptes : les comptes du grand livre derive, pas d'autres.
// ---------------------------------------------------------------------------

/// Clients : la creance ouverte par une facture de vente.
pub const COMPTE_CLIENTS: &str = "411000";
/// Fournisseurs : la dette ouverte par une facture d'achat.
pub const COMPTE_FOURNISSEURS: &str = "401000";
/// Prestations de services : le produit d'une vente.
pub const COMPTE_VENTES: &str = "706000";
/// Achats : la charge d'une facture fournisseur.
pub const COMPTE_ACHATS: &str = "601000";
/// TVA collectee sur les ventes.
pub const COMPTE_TVA_COLLECTEE: &str = "445710";
/// TVA deductible sur les achats.
pub const COMPTE_TVA_DEDUCTIBLE: &str = "445660";

/// Journal des ventes.
pub const JOURNAL_VENTES: &str = "VE";
/// Journal des achats.
pub const JOURNAL_ACHATS: &str = "AC";

/// `Facture::TYPE_PROFORMA` : un proforma n'est pas une creance. Il n'est pas
/// exporte par `crate::invoice` (qui ne cree que 0, 2, 3 et 5), mais la base
/// peut en contenir.
const TYPE_PROFORMA: i16 = 4;

// ---------------------------------------------------------------------------
// Lecture
// ---------------------------------------------------------------------------

/// L'en-tete d'un document, commun aux deux sens.
struct Document {
    reference: String,
    date: String,
    type_document: i16,
    statut: i16,
    tiers: String,
    total_ttc: Decimal,
}

/// Une ligne de document, reduite a ce qui entre en comptabilite.
struct LigneDocument {
    /// Taux de TVA applique (`tva_tx`), qui sert de cle de ventilation.
    taux: Decimal,
    ht: Decimal,
    tva: Decimal,
}

fn texte(row: &Row, i: usize) -> String {
    row.get_opt::<String, _>(i)
        .and_then(|v| v.ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn montant(row: &Row, i: usize) -> Decimal {
    texte(row, i)
        .parse::<Decimal>()
        .map(money::round_money)
        .unwrap_or_default()
}

/// Reference de l'ecriture : `<journal>-<reference du document>`.
fn numero(journal: &str, reference: &str) -> String {
    format!("{journal}-{reference}")
}

fn libelle_document(doc: &Document) -> String {
    if doc.tiers.is_empty() {
        doc.reference.clone()
    } else {
        format!("{} - {}", doc.reference, doc.tiers)
    }
}

/// Un statut qui ne s'ecrit pas est refuse, avec le motif.
///
/// Les statuts de `llx_facture_fourn` suivent la meme echelle que ceux de
/// `llx_facture` (cf. `crate::achat`) : 0 brouillon, 1 validee, 2 payee,
/// 3 abandonnee.
fn controler(doc: &Document) -> Result<(), Box<dyn Error>> {
    if doc.statut == STATUT_BROUILLON {
        return Err(format!("facture {} : un brouillon ne s'ecrit pas", doc.reference).into());
    }
    if doc.statut == STATUT_ABANDONNEE {
        return Err(format!(
            "facture {} : une facture abandonnee ne s'ecrit pas",
            doc.reference
        )
        .into());
    }
    if doc.statut != STATUT_VALIDEE && doc.statut != STATUT_PAYEE {
        return Err(format!(
            "facture {} : statut {} inconnu, elle ne s'ecrit pas",
            doc.reference, doc.statut
        )
        .into());
    }
    if doc.type_document == TYPE_PROFORMA {
        return Err(format!(
            "facture {} : un proforma n'est pas une creance, il ne s'ecrit pas",
            doc.reference
        )
        .into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Construction de l'ecriture
// ---------------------------------------------------------------------------

/// Sens voulu pour une ligne, avant que le signe du montant ne tranche.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sens {
    Debit,
    Credit,
}

/// Le sens et les comptes d'un cote (vente ou achat).
struct Plan {
    journal: &'static str,
    compte_tiers: &'static str,
    /// Produit pour une vente, charge pour un achat.
    compte_contrepartie: &'static str,
    compte_tva: &'static str,
    sens_tiers: Sens,
    sens_contrepartie: Sens,
    sens_tva: Sens,
}

impl Plan {
    /// Vente : la creance au debit, le produit et la TVA au credit.
    const fn vente() -> Self {
        Self {
            journal: JOURNAL_VENTES,
            compte_tiers: COMPTE_CLIENTS,
            compte_contrepartie: COMPTE_VENTES,
            compte_tva: COMPTE_TVA_COLLECTEE,
            sens_tiers: Sens::Debit,
            sens_contrepartie: Sens::Credit,
            sens_tva: Sens::Credit,
        }
    }

    /// Achat : la charge et la TVA au debit, la dette au credit.
    const fn achat() -> Self {
        Self {
            journal: JOURNAL_ACHATS,
            compte_tiers: COMPTE_FOURNISSEURS,
            compte_contrepartie: COMPTE_ACHATS,
            compte_tva: COMPTE_TVA_DEDUCTIBLE,
            sens_tiers: Sens::Credit,
            sens_contrepartie: Sens::Debit,
            sens_tva: Sens::Debit,
        }
    }
}

/// Ajoute une ligne au sens demande.
///
/// Un montant negatif bascule la ligne dans l'autre sens : un avoir inverse le
/// sens de l'ecriture, il ne produit jamais un debit negatif (que le moteur
/// refuse). Un montant nul n'ajoute rien.
fn pousser(
    out: &mut Vec<Ligne>,
    compte: &str,
    libelle: &str,
    journal: &str,
    date: &str,
    sens: Sens,
    montant: Decimal,
) {
    let montant = money::round_money(montant);
    if montant.is_zero() {
        return;
    }
    let negatif = montant.is_sign_negative();
    let (debit, credit) = match (sens, negatif) {
        (Sens::Debit, false) => (montant, Decimal::ZERO),
        (Sens::Debit, true) => (Decimal::ZERO, -montant),
        (Sens::Credit, false) => (Decimal::ZERO, montant),
        (Sens::Credit, true) => (-montant, Decimal::ZERO),
    };
    out.push(Ligne {
        compte: compte.to_string(),
        libelle: libelle.to_string(),
        debit,
        credit,
        journal: journal.to_string(),
        date: date.to_string(),
    });
}

/// Un taux ecrit comme un comptable l'ecrit : `20` et non `20.00`, `5,5` et non
/// `5.50`.
fn taux_en_texte(taux: Decimal) -> String {
    let texte = money::round_money(taux).to_string();
    match texte.split_once('.') {
        Some((entier, decimales)) => {
            let decimales = decimales.trim_end_matches('0');
            if decimales.is_empty() {
                entier.to_string()
            } else {
                format!("{entier},{decimales}")
            }
        }
        None => texte,
    }
}

/// Ventile un document en ecriture, et refuse ce qui ne tient pas.
///
/// Le controle porte sur le **TTC de l'en-tete** : c'est la creance (ou la
/// dette) reelle, donc le montant du debit (ou du credit) du compte de tiers.
/// La contrepartie est la somme des lignes HT et TVA arrondies **une seule
/// fois**, chacune de son cote. Si l'en-tete et les lignes divergent d'un
/// centime, rien ne s'ecrit : une facture dont l'en-tete ne vaut pas ses lignes
/// est une facture a corriger, pas une ecriture a inventer.
fn construire(
    doc: &Document,
    lignes: &[LigneDocument],
    plan: &Plan,
) -> Result<Ecriture, Box<dyn Error>> {
    let signe = if doc.type_document == TYPE_AVOIR {
        -Decimal::ONE
    } else {
        Decimal::ONE
    };

    // Ventilation : tout le HT sur le compte de contrepartie, la TVA par taux.
    // Les lignes a zero (titres de lot, sous-totaux du module Subtotal) sont
    // ignorees : elles n'ont pas de montant a ecrire.
    let mut ht = Decimal::ZERO;
    let mut tva_par_taux: BTreeMap<Decimal, Decimal> = BTreeMap::new();
    for ligne in lignes {
        if ligne.ht.is_zero() && ligne.tva.is_zero() {
            continue;
        }
        ht += ligne.ht;
        if !ligne.tva.is_zero() {
            *tva_par_taux.entry(ligne.taux).or_insert(Decimal::ZERO) += ligne.tva;
        }
    }

    let ht = money::round_money(ht) * signe;
    let tva_par_taux: BTreeMap<Decimal, Decimal> = tva_par_taux
        .into_iter()
        .map(|(taux, montant)| (taux, money::round_money(montant) * signe))
        .collect();
    let tva_totale: Decimal = tva_par_taux.values().copied().sum();
    let ttc_lignes = money::round_money(ht + tva_totale);
    let ttc = money::round_money(doc.total_ttc) * signe;

    if ttc != ttc_lignes {
        let ecart = money::round_money(ttc - ttc_lignes);
        return Err(format!(
            "facture {} : le TTC {} ne vaut pas ses lignes ({} HT + {} de TVA = {}, ecart {})",
            doc.reference,
            ttc,
            ht,
            money::round_money(tva_totale),
            ttc_lignes,
            ecart
        )
        .into());
    }

    let libelle = libelle_document(doc);
    let mut out: Vec<Ligne> = Vec::with_capacity(2 + tva_par_taux.len());
    pousser(
        &mut out,
        plan.compte_tiers,
        &libelle,
        plan.journal,
        &doc.date,
        plan.sens_tiers,
        ttc,
    );
    pousser(
        &mut out,
        plan.compte_contrepartie,
        &libelle,
        plan.journal,
        &doc.date,
        plan.sens_contrepartie,
        ht,
    );
    for (taux, montant) in &tva_par_taux {
        let libelle_tva = format!("{libelle} - TVA {} %", taux_en_texte(*taux));
        pousser(
            &mut out,
            plan.compte_tva,
            &libelle_tva,
            plan.journal,
            &doc.date,
            plan.sens_tva,
            *montant,
        );
    }

    if out.len() < 2 {
        return Err(format!(
            "facture {} : montant nul, il n'y a pas d'ecriture a tenir",
            doc.reference
        )
        .into());
    }

    let ecriture = Ecriture {
        numero: numero(plan.journal, &doc.reference),
        date: doc.date.clone(),
        libelle,
        lignes: out,
    };
    // Le moteur tranche : une ecriture qui ne tient pas ne sort pas d'ici.
    comptabilite::verifier(&ecriture)?;
    Ok(ecriture)
}

// ---------------------------------------------------------------------------
// Ventes
// ---------------------------------------------------------------------------

const SQL_ENTETE_VENTE: &str = "SELECT f.ref, \
        COALESCE(DATE_FORMAT(f.datef, '%Y-%m-%d'), DATE_FORMAT(f.datec, '%Y-%m-%d'), ''), \
        f.type, f.fk_statut, COALESCE(s.nom, ''), CAST(f.total_ttc AS CHAR) \
     FROM llx_facture f LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
     WHERE f.entity = :entity AND f.rowid = :rowid";

const SQL_LIGNES_VENTE: &str = "SELECT CAST(d.tva_tx AS CHAR), CAST(d.total_ht AS CHAR), \
        CAST(d.total_tva AS CHAR) \
     FROM llx_facturedet d WHERE d.fk_facture = :rowid \
     ORDER BY d.rang ASC, d.rowid ASC";

/// L'ecriture d'une facture de vente : creance, produit, TVA collectee.
///
/// Debit 411000 du TTC, credit 706000 du HT, credit 445710 de la TVA (une ligne
/// par taux). Un avoir (`type = 2`) inverse les sens. Une facture en brouillon,
/// abandonnee ou proforma est refusee.
///
/// Le numero est `VE-<reference>`, stable d'un appel a l'autre.
pub fn facture_vente(
    pool: &Pool,
    entity: i64,
    facture_id: i64,
) -> Result<Ecriture, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let doc = lire_document(&mut conn, entity, facture_id, SQL_ENTETE_VENTE, "facture")?;
    controler(&doc)?;
    // `llx_facturedet` n'a pas de colonne `entity` : le cloisonnement est
    // assure par l'en-tete, lu avec `entity = :entity`.
    let lignes = lire_lignes(&mut conn, facture_id, SQL_LIGNES_VENTE)?;
    construire(&doc, &lignes, &Plan::vente())
}

// ---------------------------------------------------------------------------
// Achats
// ---------------------------------------------------------------------------

const SQL_ENTETE_ACHAT: &str = "SELECT f.ref, \
        COALESCE(DATE_FORMAT(f.datef, '%Y-%m-%d'), DATE_FORMAT(f.datec, '%Y-%m-%d'), ''), \
        f.type, f.fk_statut, COALESCE(s.nom, ''), CAST(f.total_ttc AS CHAR) \
     FROM llx_facture_fourn f LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
     WHERE f.entity = :entity AND f.rowid = :rowid";

/// Attention : sur une ligne fournisseur, la TVA est dans `tva`, pas dans
/// `total_tva` (heritage du schema Dolibarr).
const SQL_LIGNES_ACHAT: &str = "SELECT CAST(d.tva_tx AS CHAR), CAST(d.total_ht AS CHAR), \
        CAST(d.tva AS CHAR) \
     FROM llx_facture_fourn_det d WHERE d.fk_facture_fourn = :rowid \
     ORDER BY d.rang ASC, d.rowid ASC";

/// L'ecriture d'une facture d'achat : charge, TVA deductible, dette.
///
/// Debit 601000 du HT, debit 445660 de la TVA (une ligne par taux), credit
/// 401000 du TTC. Un avoir fournisseur (`type = 2`) inverse les sens.
///
/// Le numero est `AC-<reference>`, stable d'un appel a l'autre.
pub fn facture_achat(
    pool: &Pool,
    entity: i64,
    facture_id: i64,
) -> Result<Ecriture, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let doc = lire_document(
        &mut conn,
        entity,
        facture_id,
        SQL_ENTETE_ACHAT,
        "facture fournisseur",
    )?;
    controler(&doc)?;
    let lignes = lire_lignes(&mut conn, facture_id, SQL_LIGNES_ACHAT)?;
    construire(&doc, &lignes, &Plan::achat())
}

// ---------------------------------------------------------------------------
// Lecture commune
// ---------------------------------------------------------------------------

fn lire_document(
    conn: &mut mysql::PooledConn,
    entity: i64,
    facture_id: i64,
    sql: &str,
    nature: &str,
) -> Result<Document, Box<dyn Error>> {
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => facture_id })?;
    let Some(row) = row else {
        return Err(format!("{nature} {facture_id} introuvable dans l'entite {entity}").into());
    };
    let doc = Document {
        reference: texte(&row, 0),
        date: texte(&row, 1),
        type_document: row.get_opt::<i64, _>(2).and_then(|v| v.ok()).unwrap_or(0) as i16,
        statut: row.get_opt::<i64, _>(3).and_then(|v| v.ok()).unwrap_or(0) as i16,
        tiers: texte(&row, 4),
        total_ttc: montant(&row, 5),
    };
    if doc.reference.is_empty() {
        return Err(format!("{nature} {facture_id} : reference vide, pas de numero stable").into());
    }
    if doc.date.is_empty() {
        return Err(format!(
            "{nature} {} : aucune date de facturation, l'ecriture n'a pas de date",
            doc.reference
        )
        .into());
    }
    Ok(doc)
}

fn lire_lignes(
    conn: &mut mysql::PooledConn,
    facture_id: i64,
    sql: &str,
) -> Result<Vec<LigneDocument>, Box<dyn Error>> {
    let rows: Vec<Row> = conn.exec(sql, params! { "rowid" => facture_id })?;
    Ok(rows
        .iter()
        .map(|row| LigneDocument {
            taux: montant(row, 0),
            ht: montant(row, 1),
            tva: montant(row, 2),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Journal complet
// ---------------------------------------------------------------------------

/// Toutes les ecritures derivees d'un journal, dans l'ordre des dates.
///
/// `journal` designe `VE` ou `AC` (la casse et les espaces sont sans effet).
/// Les brouillons, les factures abandonnees et les proformas ne sont pas lus ;
/// les avoirs le sont, en sens inverse.
///
/// `limite` borne le nombre d'ecritures, pas le nombre de lignes ; `0` ne rend
/// rien (les appelants qui veulent tout passent un grand nombre).
///
/// Un journal que ce pont ne produit pas (`BQ`, `OD`, `AN`) rend une liste
/// vide : ce n'est pas une erreur, c'est son perimetre.
///
/// Une seule facture incoherente fait echouer tout le journal : une ecriture
/// fausse ne doit pas se glisser au milieu des justes.
pub fn ecritures_du_journal(
    pool: &Pool,
    entity: i64,
    journal: &str,
    limite: u32,
) -> Result<Vec<Ecriture>, Box<dyn Error>> {
    let code = journal.trim().to_ascii_uppercase();
    if code.is_empty() {
        return Err("journal vide".into());
    }

    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = match code.as_str() {
        JOURNAL_VENTES => conn.exec(
            "SELECT f.rowid FROM llx_facture f \
             WHERE f.entity = :entity AND f.fk_statut IN (:validee, :payee) \
               AND COALESCE(f.type, 0) <> :proforma \
             ORDER BY f.datef ASC, f.ref ASC LIMIT :limite",
            params! {
                "entity" => entity,
                "validee" => STATUT_VALIDEE,
                "payee" => STATUT_PAYEE,
                "proforma" => TYPE_PROFORMA,
                "limite" => limite,
            },
        )?,
        JOURNAL_ACHATS => conn.exec(
            "SELECT f.rowid FROM llx_facture_fourn f \
             WHERE f.entity = :entity AND f.fk_statut IN (:validee, :payee) \
               AND COALESCE(f.type, 0) <> :proforma \
             ORDER BY f.datef ASC, f.ref ASC LIMIT :limite",
            params! {
                "entity" => entity,
                "validee" => STATUT_VALIDEE,
                "payee" => STATUT_PAYEE,
                "proforma" => TYPE_PROFORMA,
                "limite" => limite,
            },
        )?,
        _ => return Ok(Vec::new()),
    };
    drop(conn);

    let mut ecritures: Vec<Ecriture> = Vec::with_capacity(ids.len());
    for id in ids {
        ecritures.push(match code.as_str() {
            JOURNAL_ACHATS => facture_achat(pool, entity, id)?,
            _ => facture_vente(pool, entity, id)?,
        });
    }

    // L'ordre des dates ne depend pas du tri de la base : on le pose ici, et le
    // numero tranche a date egale.
    ecritures.sort_by(|a, b| {
        (a.date.as_str(), a.numero.as_str()).cmp(&(b.date.as_str(), b.numero.as_str()))
    });
    Ok(ecritures)
}
