//! Emettre le document d'une serie recurrente : de « quand emettre » a « emettre ».
//!
//! [`crate::recurrence`] rend les series dont l'echeance est tombee. Ce module
//! fait le pas suivant : il relit le modele, cree une **vraie** facture dans
//! `llx_facture`, puis rend son identifiant. Il ne valide rien, n'envoie aucun
//! e-mail, ne touche a aucune autre table.
//!
//! # Les colonnes reellement lues
//!
//! Verifiees dans `schema-24.0.1.json` (racine du depot), jamais supposees.
//!
//! | table | colonnes utilisees |
//! |---|---|
//! | `llx_facture_rec` | `rowid`, `entity`, `fk_soc`, `fk_cond_reglement`, `fk_mode_reglement`, `fk_account`, `note_private`, `note_public` |
//! | `llx_facturedet_rec` | `fk_facture`, `fk_product`, `product_type`, `label`, `description`, `tva_tx`, `qty`, `remise_percent`, `subprice`, `fk_unit`, `special_code`, `rang` |
//!
//! La table des lignes s'appelle **`llx_facturedet_rec`** et non
//! `llx_facture_rec_det` : Dolibarr n'a jamais cree cette seconde table (le
//! manifeste ne la connait pas). Elle ne porte pas de colonne `entity` : le
//! cloisonnement vient de `fk_facture`, qui pointe une serie deja filtree par
//! entite.
//!
//! # La reference externe, seule garde contre le doublon
//!
//! Rien, dans le schema, n'empeche deux factures de porter la meme serie. La
//! garde est donc une marque ecrite sur la facture elle-meme :
//!
//! ```text
//! ref_ext = PA-RECUR-<source>-<id>-<echeance>      (varchar(255), nullable)
//! ```
//!
//! La date est celle de l'**echeance emise**, pas le jour du traitement : deux
//! passages pour la meme occurrence produisent la meme cle, quel que soit le
//! jour ou ils tombent. C'est ce qui rend la garde absolue la ou un curseur
//! « emise le jour J » laisserait passer un doublon le lendemain (cron passe,
//! puis marquage de l'echeance perdu, puis reprise manuelle).
//!
//! Cette cle est cherchee **avant** la creation, sous un verrou consultatif
//! MySQL nomme par serie (`GET_LOCK`) : deux processus du noyau qui traitent la
//! meme serie en meme temps sont serialises, et le second trouve la cle du
//! premier. Aucun changement de schema, aucune table nouvelle.
//!
//! # Ce que la facture herite du modele
//!
//! Le tiers, la condition de reglement, la date de facture (l'echeance), les
//! notes, le compte bancaire et le mode de reglement, puis chaque ligne avec son
//! libelle, sa quantite, son prix unitaire HT, sa TVA, sa remise et son unite.
//!
//! Les lignes de structure (titre de lot : `product_type = 9`) sont ecartees,
//! comme le fait deja [`crate::invoice::create_from_proposal`] : elles ne pesent
//! rien dans les totaux et [`crate::invoice::NewInvoiceLine`] les reconstruit a
//! partir d'un autre champ.
//!
//! La facture est ecrite **en brouillon** (statut 0). Un document genere se relit
//! avant d'etre envoye : ce module ne valide jamais, n'attribue aucun numero
//! definitif et ne declenche aucun envoi.
//!
//! # Ce que `create` ne porte pas encore
//!
//! [`crate::invoice::create`] n'ecrit ni `ref_ext`, ni `fk_account`, ni
//! `fk_mode_reglement`. Ces trois colonnes sont donc posees juste apres la
//! creation, sur la facture qui vient d'etre creee. Si la reference externe ne
//! peut pas etre posee, le brouillon est supprime : mieux vaut ne rien avoir
//! qu'une facture que la garde ne reconnaitrait pas.
//!
//! # Exemple
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let pool = db::pool("mysql://root:root@127.0.0.1:3306/dolibarr")?;
//! let jour = db::aujourdhui(&pool)?;
//! for (source, serie, facture) in db::recur_generation::generer_tout(&pool, 1, &jour, 100)? {
//!     println!("{source} {serie} -> brouillon n° {facture}");
//! }
//! # Ok(())
//! # }
//! ```

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, PooledConn, Row, TxOpts};
use rust_decimal::Decimal;

use crate::invoice::{self, NewInvoice, NewInvoiceLine};
use crate::recurrence;

/// La seule source que ce module sait facturer.
///
/// `facture_fourn_rec` demanderait une facture fournisseur et `fichinter_rec`
/// une intervention : deux documents differents, qui n'ont pas leur place dans
/// une fonction qui rend un `rowid` de `llx_facture`. Les deux sources restent
/// donc signalees, jamais facturees de travers.
pub const SOURCE_FACTURE: &str = "facture_rec";

/// Prefixe de la reference externe (voir l'entete du module).
const PREFIXE_REF_EXT: &str = "PA-RECUR-";

/// Prefixe du verrou consultatif, complete par entite, source et serie.
const PREFIXE_VERROU: &str = "dolibarr_rs_recur_";

/// Attente maximale du verrou de serie, en secondes.
const ATTENTE_VERROU_S: u32 = 10;

/// Combien de series dues [`echeance_due`] accepte de balayer pour retrouver la
/// sienne.
///
/// [`recurrence::a_emettre`] ne sait rendre qu'une liste bornee ; une journee
/// qui depasserait ce plafond demanderait de toute facon un traitement par lots.
const MAX_SERIES_DUES: usize = 10_000;

/// Ce que [`generer_tout`] a produit : `(source, id_serie, id_facture)`.
pub type Emission = (String, i64, i64);

/// Une ligne de modele, telle qu'elle sera recopiee dans la facture.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LigneModele {
    pub(crate) fk_product: Option<i64>,
    pub(crate) label: String,
    pub(crate) description: Option<String>,
    pub(crate) qty: Decimal,
    pub(crate) subprice: Decimal,
    pub(crate) tva_tx: Decimal,
    pub(crate) remise_percent: Decimal,
    pub(crate) fk_unit: Option<i64>,
}

/// Le modele relu : ce que la serie porte et que la facture doit reprendre.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Modele {
    pub(crate) fk_soc: i64,
    pub(crate) fk_cond_reglement: Option<i64>,
    pub(crate) fk_mode_reglement: Option<i64>,
    pub(crate) fk_account: Option<i64>,
    pub(crate) note_private: Option<String>,
    pub(crate) note_public: Option<String>,
    pub(crate) lignes: Vec<LigneModele>,
}

fn texte(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|v| v.ok())
}

fn identifiant(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|v| v.ok())
}

/// Un montant lu en texte (`CAST(... AS CHAR)`), jamais en flottant.
fn montant(row: &Row, i: usize) -> Decimal {
    texte(row, i)
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .unwrap_or_default()
}

/// La reference externe d'une occurrence : la garde anti-doublon.
fn cle(source: &str, id: i64, echeance: &str) -> String {
    format!("{PREFIXE_REF_EXT}{source}-{id}-{echeance}")
}

/// L'echeance que le moteur considere due pour cette serie.
///
/// On interroge [`recurrence::a_emettre`] plutot que de recalculer une regle :
/// deux implementations du meme calendrier finiraient par diverger, et la
/// reference externe porte cette date.
///
/// # Erreurs
///
/// Date invalide, erreur MySQL, serie introuvable dans les echeances dues (elle
/// n'est pas due, ou elle est close) : on n'emet pas un document pour une
/// echeance qui n'existe pas.
fn echeance_due(
    pool: &Pool,
    entity: i64,
    source: &str,
    id: i64,
    aujourd_hui: &str,
) -> Result<String, Box<dyn Error>> {
    let series = recurrence::a_emettre(pool, entity, aujourd_hui, MAX_SERIES_DUES)?;
    let serie = series
        .iter()
        .find(|serie| serie.source == source && serie.id == id)
        .ok_or_else(|| {
            format!(
                "la serie {source} {id} n'a pas d'echeance due au {aujourd_hui} \
                 (entite {entity}, ou au-dela de {MAX_SERIES_DUES} series)"
            )
        })?;
    match recurrence::echeance(serie)? {
        Some(echeance) => Ok(echeance),
        None => Err(format!("la serie {source} {id} est close : aucune echeance a emettre").into()),
    }
}

/// Relit le modele et ses lignes, dans l'entite demandee.
///
/// # Erreurs
///
/// Serie absente de l'entite, et toute erreur MySQL remontee telle quelle.
pub(crate) fn charger(pool: &Pool, entity: i64, id: i64) -> Result<Modele, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;

    type Entete = (
        i64,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<String>,
    );
    let entete: Option<Entete> = conn.exec_first(
        "SELECT fk_soc, fk_cond_reglement, fk_mode_reglement, fk_account, \
                note_private, note_public \
           FROM llx_facture_rec WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => id },
    )?;
    let (fk_soc, fk_cond_reglement, fk_mode_reglement, fk_account, note_private, note_public) =
        entete.ok_or_else(|| {
            format!("serie {SOURCE_FACTURE} {id} introuvable dans l'entite {entity}")
        })?;

    // `llx_facturedet_rec` n'a pas d'`entity` : l'appartenance vient de la serie,
    // deja verifiee ci-dessus.
    let rows: Vec<Row> = conn.exec(
        "SELECT fk_product, product_type, label, description, CAST(tva_tx AS CHAR), \
                CAST(qty AS CHAR), CAST(remise_percent AS CHAR), CAST(subprice AS CHAR), \
                fk_unit, special_code \
           FROM llx_facturedet_rec WHERE fk_facture = :rowid \
          ORDER BY rang ASC, rowid ASC",
        params! { "rowid" => id },
    )?;

    let mut lignes = Vec::with_capacity(rows.len());
    for row in &rows {
        // Titre de lot : il ne porte ni quantite ni prix, et `NewInvoiceLine` le
        // reconstruit autrement. On l'ecarte, comme la conversion d'un devis.
        if identifiant(row, 1) == Some(i64::from(invoice::PRODUCT_TYPE_SECTION)) {
            continue;
        }
        lignes.push(LigneModele {
            fk_product: identifiant(row, 0),
            label: texte(row, 2).unwrap_or_default(),
            description: texte(row, 3),
            tva_tx: montant(row, 4),
            qty: montant(row, 5),
            remise_percent: montant(row, 6),
            subprice: montant(row, 7),
            fk_unit: identifiant(row, 8),
        });
    }

    Ok(Modele {
        fk_soc,
        fk_cond_reglement,
        fk_mode_reglement,
        fk_account,
        note_private,
        note_public,
        lignes,
    })
}

/// Verrou consultatif MySQL nomme par serie, relache au `Drop`.
///
/// Il ne protege pas la numerotation (c'est deja le role du verrou de
/// [`crate::invoice::create`]) mais la sequence « je regarde si la cle existe,
/// puis je cree » : sans lui, deux passages simultanes liraient tous les deux
/// une table vide.
struct Verrou {
    conn: PooledConn,
    nom: String,
}

impl Verrou {
    fn prendre(pool: &Pool, nom: &str) -> Result<Self, Box<dyn Error>> {
        let mut conn = pool.get_conn()?;
        let sql = format!("SELECT GET_LOCK(?, {ATTENTE_VERROU_S})");
        let pris: Option<i64> = conn.exec_first(sql.as_str(), (nom,))?;
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

/// Pose la reference externe (la garde), et ce que `create` ne porte pas.
///
/// `COALESCE` : une valeur absente du modele ne vient pas effacer celle que la
/// facture porte deja.
fn estampiller(
    pool: &Pool,
    entity: i64,
    facture: i64,
    ref_ext: &str,
    modele: &Modele,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_facture \
            SET ref_ext = :ref_ext, \
                fk_account = COALESCE(:compte, fk_account), \
                fk_mode_reglement = COALESCE(:mode, fk_mode_reglement) \
          WHERE entity = :entity AND rowid = :rowid",
        params! {
            "ref_ext" => ref_ext,
            "compte" => modele.fk_account,
            "mode" => modele.fk_mode_reglement,
            "entity" => entity,
            "rowid" => facture,
        },
    )?;
    if conn.affected_rows() == 0 {
        return Err(format!(
            "facture {facture} introuvable apres creation : reference externe non posee"
        )
        .into());
    }
    Ok(())
}

/// Recopie l'unite de chaque ligne du modele sur la ligne ecrite.
///
/// `NewInvoiceLine` ne porte pas `fk_unit` : l'unite — celle qui s'imprime sur le
/// document — est donc ecrite apres coup. Les lignes de la facture sont lues
/// dans l'ordre de `rang`, qui est celui du modele une fois les titres de lot
/// ecartes ; le nombre de lignes doit coincider, sinon on ne devine pas.
fn poser_unites(
    pool: &Pool,
    entity: i64,
    facture: i64,
    lignes: &[LigneModele],
) -> Result<(), Box<dyn Error>> {
    if lignes.iter().all(|ligne| ligne.fk_unit.is_none()) {
        return Ok(());
    }
    let mut conn = pool.get_conn()?;
    let rows: Vec<i64> = conn.exec(
        "SELECT d.rowid FROM llx_facturedet d \
           INNER JOIN llx_facture f ON f.rowid = d.fk_facture \
          WHERE f.entity = :entity AND d.fk_facture = :facture \
          ORDER BY d.rang ASC, d.rowid ASC",
        params! { "entity" => entity, "facture" => facture },
    )?;
    if rows.len() != lignes.len() {
        return Err(format!(
            "{} lignes ecrites pour {} lignes de modele : unites non posees",
            rows.len(),
            lignes.len()
        )
        .into());
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    for (rowid, ligne) in rows.iter().zip(lignes) {
        if let Some(unite) = ligne.fk_unit {
            tx.exec_drop(
                "UPDATE llx_facturedet SET fk_unit = :unite WHERE rowid = :rowid",
                params! { "unite" => unite, "rowid" => rowid },
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Cree la facture d'une serie recurrente et rend son `rowid`.
///
/// Rend `None` quand l'occurrence a **deja** ete emise : une facture de la meme
/// entite porte deja `PA-RECUR-<source>-<id>-<echeance>`. C'est la garde
/// anti-doublon, et elle seule ; rien n'est ecrit dans ce cas.
///
/// Rend `Some(rowid)` quand la facture vient d'etre creee. Elle est **en
/// brouillon**, jamais validee.
///
/// # Erreurs
///
/// Source non facturable ici, identifiant invalide, echeance non due, modele
/// introuvable ou vide, verrou indisponible, et toute erreur MySQL remontee
/// telle quelle.
///
/// Une erreur survenue **avant** la pose de la reference externe ne laisse rien
/// derriere : le brouillon eventuellement cree est retire. Une erreur survenue
/// **apres** (les unites, par exemple) laisse la facture en brouillon, mais avec
/// sa reference externe : elle reste identifiable et ne sera jamais doublee.
pub fn generer(
    pool: &Pool,
    entity: i64,
    source: &str,
    id: i64,
    aujourd_hui: &str,
) -> Result<Option<i64>, Box<dyn Error>> {
    if source != SOURCE_FACTURE {
        return Err(format!(
            "seule une serie de facture client ({SOURCE_FACTURE}) se genere ici, pas {source}"
        )
        .into());
    }
    if id <= 0 {
        return Err(format!("identifiant de serie invalide : {id}").into());
    }

    // L'echeance, pas le jour du traitement : la cle ne bouge pas tant qu'il
    // s'agit de la meme occurrence.
    let echeance = echeance_due(pool, entity, source, id, aujourd_hui)?;
    let ref_ext = cle(source, id, &echeance);
    let modele = charger(pool, entity, id)?;
    if modele.lignes.is_empty() {
        return Err(format!("la serie {source} {id} n'a aucune ligne a facturer").into());
    }

    let mut verrou = Verrou::prendre(pool, &format!("{PREFIXE_VERROU}{entity}_{source}_{id}"))?;
    let deja: Option<i64> = verrou.conn.exec_first(
        "SELECT rowid FROM llx_facture WHERE entity = :entity AND ref_ext = :ref_ext LIMIT 1",
        params! { "entity" => entity, "ref_ext" => ref_ext.as_str() },
    )?;
    if deja.is_some() {
        return Ok(None);
    }

    let mut new = NewInvoice::new(modele.fk_soc);
    new.note_private = modele.note_private.clone();
    new.note_public = modele.note_public.clone();
    // La facture porte la date de l'echeance, comme le fait Dolibarr : une serie
    // mensuelle emise le 3 pour le 1er reste datee du 1er.
    new.datef = Some(echeance.clone());
    new.fk_cond_reglement = modele.fk_cond_reglement.filter(|cond| *cond > 0);
    new.lines = modele
        .lignes
        .iter()
        .map(|ligne| NewInvoiceLine {
            fk_product: ligne.fk_product,
            label: ligne.label.clone(),
            description: ligne.description.clone(),
            qty: ligne.qty,
            unit_price: ligne.subprice,
            tva_tx: ligne.tva_tx,
            remise_percent: ligne.remise_percent,
            section: None,
        })
        .collect();

    let facture = invoice::create(pool, entity, &new)?;

    // La creation est acquise ; sans la cle, elle echapperait a la garde.
    if let Err(erreur) = estampiller(pool, entity, facture, &ref_ext, &modele) {
        let _ = invoice::delete(pool, entity, facture);
        return Err(erreur);
    }
    poser_unites(pool, entity, facture, &modele.lignes)?;

    Ok(Some(facture))
}

/// Traite toutes les series dues de l'entite et rend celles qui ont produit une
/// facture : `(source, id_serie, id_facture)`.
///
/// Une echeance honoree est marquee avec [`recurrence::marquer_emise`], y compris
/// quand `generer` rend `None` : la reference externe prouve que l'occurrence a
/// deja ete emise, donc le curseur peut avancer sans rien creer.
///
/// Une serie en echec **n'arrete pas** les autres : elle est signalee sur la
/// sortie d'erreur et la boucle continue. Elle n'est pas marquee, donc elle sera
/// representee au prochain passage — c'est voulu, une emission ratee doit rester
/// visible.
///
/// # Erreurs
///
/// Seules celles de [`recurrence::a_emettre`] : date invalide, base inaccessible.
pub fn generer_tout(
    pool: &Pool,
    entity: i64,
    aujourd_hui: &str,
    max: usize,
) -> Result<Vec<Emission>, Box<dyn Error>> {
    let series = recurrence::a_emettre(pool, entity, aujourd_hui, max)?;
    let mut faites = Vec::new();

    for serie in &series {
        match generer(pool, entity, &serie.source, serie.id, aujourd_hui) {
            Ok(id_facture) => {
                if let Err(erreur) =
                    recurrence::marquer_emise(pool, entity, &serie.source, serie.id, aujourd_hui)
                {
                    eprintln!(
                        "[dolibarr-rs] serie {} {} : document emis, echeance non marquee : {erreur}",
                        serie.source, serie.id
                    );
                }
                if let Some(id) = id_facture {
                    faites.push((serie.source.clone(), serie.id, id));
                }
            }
            Err(erreur) => {
                eprintln!(
                    "[dolibarr-rs] serie {} {} ignoree : {erreur}",
                    serie.source, serie.id
                );
            }
        }
    }
    Ok(faites)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cle_porte_la_source_la_serie_et_l_echeance() {
        assert_eq!(
            cle("facture_rec", 42, "2026-11-01"),
            "PA-RECUR-facture_rec-42-2026-11-01"
        );
        // Deux occurrences differentes d'une meme serie ne se confondent pas.
        assert_ne!(
            cle("facture_rec", 42, "2026-11-01"),
            cle("facture_rec", 42, "2026-12-01")
        );
        assert!(cle("facture_rec", 42, "2026-11-01").starts_with("PA-RECUR-"));
    }
}
