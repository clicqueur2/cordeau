//! Donnees pour un bordereau SEPA : ce qu'il reste a payer aux fournisseurs
//! (virement `pain.001`) et qui peut etre preleve (mandats, `pain.008`).
//!
//! Le crate `sepa` assemble le XML ; ce module ne fait que **lire** la base et
//! rendre des montants en **centimes entiers**, parce qu'un fichier bancaire ne
//! supporte pas d'arrondi flottant. Aucune ecriture, aucun ecran.
//!
//! Colonnes Dolibarr utilisees, verifiees dans `schema-24.0.1.json` et sur la
//! base (entites 1 et 7) :
//!
//! - `llx_facture_fourn` : `rowid`, `ref`, `entity`, `fk_soc`, `fk_statut`,
//!   `paye`, `total_ttc`, `multicurrency_total_ttc`, `date_lim_reglement` ;
//! - `llx_societe` : `rowid`, `nom`, `entity` (nom du fournisseur) ;
//! - `llx_societe_rib` : `iban_prefix` (c'est la colonne qui porte l'IBAN chez
//!   Dolibarr, la meme que pour les comptes bancaires), `bic`, `default_rib`,
//!   `rum`, `date_rum`, `frstrecur`, `status` ;
//! - `llx_paiementfourn` / `llx_paiementfourn_facturefourn` : `amount`,
//!   `statut` (paiements deja faits, pour le reste a payer).

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

/// `llx_facture_fourn.fk_statut` : 0 brouillon, 1 validee (ouverte), 2 payee,
/// 3 abandonnee. Seule la valeur 1 est une dette exigible.
pub const STATUT_FACTURE_VALIDEE: i16 = 1;

/// Une facture fournisseur qui attend son virement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactureAPayer {
    /// `llx_facture_fourn.rowid`.
    pub id: i64,
    /// Reference Dolibarr (`ref`), pas la reference du fournisseur.
    pub ref_: String,
    /// `llx_societe.nom` ; vide si la fiche du tiers a disparu.
    pub fournisseur: String,
    /// IBAN du fournisseur, `None` s'il n'a pas de RIB en base.
    pub iban: Option<String>,
    /// BIC du meme RIB, `None` s'il n'est pas renseigne.
    pub bic: Option<String>,
    /// Reste a payer TTC, en centimes entiers (12 345 = 123,45 EUR).
    pub montant_centimes: i64,
    /// `date_lim_reglement` au format `AAAA-MM-JJ`, `None` si Dolibarr n'en a
    /// pas pose (un fournisseur sans condition de reglement).
    pub date_echeance: Option<String>,
}

impl FactureAPayer {
    /// La reference, sous le nom attendu par un fichier `pain.001`
    /// (`EndToEndId`). Le champ lui-meme s'appelle `ref_` parce que `ref` est un
    /// mot reserve de Rust.
    pub fn reference(&self) -> &str {
        &self.ref_
    }
}

/// Un mandat de prelevement SEPA signe et encore valable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mandat {
    /// `llx_societe_rib.rowid` : chez Dolibarr, le mandat vit sur le RIB.
    pub id: i64,
    /// `llx_societe.nom`.
    pub tiers: String,
    /// IBAN prelevable (`llx_societe_rib.iban_prefix`).
    pub iban: Option<String>,
    /// BIC du RIB.
    pub bic: Option<String>,
    /// Reference unique du mandat (`rum`), imposee par la banque.
    pub reference: String,
    /// Date de signature du mandat (`date_rum`), `AAAA-MM-JJ`.
    pub signe_le: Option<String>,
    /// `true` si la sequence SEPA est `RCUR` (recurrent), `false` pour `FRST`
    /// (premier prelevement).
    pub recurrent: bool,
}

/// Reste a payer d'une facture fournisseur, en devise de la societe : le TTC
/// moins les paiements deja enregistres.
///
/// `llx_facture_fourn` ne stocke pas de colonne "reste a payer" : Dolibarr le
/// recalcule a chaque affichage en sommant `llx_paiementfourn_facturefourn`.
/// On fait pareil, sinon un acompte deja verse serait paye deux fois.
///
/// Les paiements en brouillon (`llx_paiementfourn.statut = 0`) sont exclus :
/// ils ne sont pas encore partis en banque. `COALESCE(..., 1)` traite un
/// statut NULL comme valide, ce qu'il est en pratique.
const RESTE_SQL: &str = "CAST(f.total_ttc - COALESCE((SELECT SUM(pf.amount) \
     FROM llx_paiementfourn_facturefourn pf \
     JOIN llx_paiementfourn p ON p.rowid = pf.fk_paiementfourn \
     WHERE pf.fk_facturefourn = f.rowid AND p.entity = f.entity \
       AND COALESCE(p.statut, 1) <> 0), 0) AS CHAR)";

/// Le RIB a utiliser pour un fournisseur : le RIB par defaut s'il y en a un,
/// sinon le plus recent. La sous-requete est correlee sur `f.fk_soc` et
/// restreinte a l'entite, et exige un IBAN non vide : un RIB sans IBAN ne peut
/// pas alimenter un virement.
const RIB_SQL: &str = "LEFT JOIN llx_societe_rib r ON r.rowid = (\
     SELECT r2.rowid FROM llx_societe_rib r2 \
     WHERE r2.fk_soc = f.fk_soc AND r2.entity = f.entity \
       AND r2.iban_prefix IS NOT NULL AND TRIM(r2.iban_prefix) <> '' \
     ORDER BY r2.default_rib DESC, r2.rowid DESC LIMIT 1)";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

/// Un texte vide vaut `None` : un IBAN blanc ne doit pas passer pour un IBAN.
fn non_vide(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Montant en centimes entiers, arrondi au centime le plus proche (demi hors de
/// zero, comme le `round()` de PHP utilise par Dolibarr).
///
/// Le calcul se fait en `Decimal`, jamais en `f64` : 0,1 + 0,2 en flottant ne
/// font pas 0,3, et une banque refuse un fichier dont le controle de somme ne
/// tombe pas juste.
fn en_centimes(montant: Decimal) -> i64 {
    (montant * Decimal::ONE_HUNDRED)
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or(0)
}

fn map_facture(row: &Row) -> FactureAPayer {
    FactureAPayer {
        id: opt_i(row, 0).unwrap_or(0),
        ref_: opt_s(row, 1).unwrap_or_default(),
        fournisseur: opt_s(row, 2).unwrap_or_default(),
        iban: non_vide(opt_s(row, 3)),
        bic: non_vide(opt_s(row, 4)),
        montant_centimes: opt_dec(row, 5).map(en_centimes).unwrap_or(0),
        date_echeance: opt_s(row, 6),
    }
}

fn map_mandat(row: &Row) -> Mandat {
    let sequence = opt_s(row, 6).unwrap_or_else(|| "FRST".to_string());
    Mandat {
        id: opt_i(row, 0).unwrap_or(0),
        tiers: opt_s(row, 1).unwrap_or_default(),
        iban: non_vide(opt_s(row, 2)),
        bic: non_vide(opt_s(row, 3)),
        reference: opt_s(row, 4).unwrap_or_default(),
        signe_le: opt_s(row, 5),
        // `frstrecur` stocke la sequence SEPA telle quelle, comme Dolibarr.
        recurrent: !sequence.trim().eq_ignore_ascii_case("FRST"),
    }
}

/// Factures fournisseurs validees, non soldees, de l'entite, triees par
/// echeance (les sans echeance a la fin).
///
/// Trois filtres, chacun pour une raison precise :
///
/// - `fk_statut = 1 AND paye = 0` : une facture en brouillon n'est pas une
///   dette, une facture soldee n'a plus rien a payer ;
/// - le reste a payer strictement positif : une facture partiellement payee
///   reste due pour son solde seulement ;
/// - `entity` : un bordereau de Pierre Angulaire ne doit jamais contenir une
///   facture de 2AR.
///
/// Le montant est lu sur `total_ttc`, en devise de la societe. Quand la facture
/// est en devise (`fk_multicurrency > 0`), `multicurrency_total_ttc` porte le
/// montant dans la devise du document : ce n'est pas ce qu'un virement SEPA en
/// euros doit payer, donc on ne le prend pas. Le change applique est deja
/// compris dans `total_ttc`.
pub fn factures_fournisseurs_a_payer(
    pool: &Pool,
    entity: i64,
    limite: u32,
) -> Result<Vec<FactureAPayer>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT t.id, t.ref, t.fournisseur, t.iban, t.bic, t.reste, t.echeance FROM (\
           SELECT f.rowid AS id, f.ref AS ref, COALESCE(s.nom, '') AS fournisseur, \
                  r.iban_prefix AS iban, r.bic AS bic, {RESTE_SQL} AS reste, \
                  DATE_FORMAT(f.date_lim_reglement, '%Y-%m-%d') AS echeance \
           FROM llx_facture_fourn f \
           LEFT JOIN llx_societe s ON s.rowid = f.fk_soc AND s.entity = f.entity \
           {RIB_SQL} \
           WHERE f.entity = :entity AND f.fk_statut = :statut AND f.paye = 0\
         ) t \
         WHERE t.reste > 0.005 \
         ORDER BY t.echeance IS NULL, t.echeance ASC, t.id ASC \
         LIMIT :limite"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! {
            "entity" => entity,
            "statut" => STATUT_FACTURE_VALIDEE,
            "limite" => limite,
        },
    )?;
    Ok(rows.iter().map(map_facture).collect())
}

/// Mandats SEPA actifs de l'entite, tries par tiers.
///
/// Il n'existe pas de table `llx_prelevement_mandat` en 24.0.1 : le mandat est
/// porte par le RIB du tiers (`llx_societe_rib`), avec `rum` (reference unique
/// du mandat), `date_rum` (date de signature) et `frstrecur` (sequence SEPA).
/// Les tables `llx_prelevement_bons` et `llx_prelevement_lignes` ne decrivent
/// que des **executions** de prelevement, pas des mandats.
///
/// Un mandat est retenu quand il a une reference (`rum`) et une date de
/// signature (`date_rum`) : sans les deux, la banque rejette le fichier. Le
/// champ `status` n'est pas filtre : Dolibarr le laisse a 1 par defaut et s'en
/// sert pour des moyens de paiement en ligne (Stripe), pas pour invalider un
/// mandat bancaire.
pub fn mandats_actifs(
    pool: &Pool,
    entity: i64,
    limite: u32,
) -> Result<Vec<Mandat>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT r.rowid, COALESCE(s.nom, ''), r.iban_prefix, r.bic, r.rum, \
                DATE_FORMAT(r.date_rum, '%Y-%m-%d'), COALESCE(r.frstrecur, 'FRST') \
         FROM llx_societe_rib r \
         LEFT JOIN llx_societe s ON s.rowid = r.fk_soc AND s.entity = r.entity \
         WHERE r.entity = :entity \
           AND r.rum IS NOT NULL AND TRIM(r.rum) <> '' \
           AND r.date_rum IS NOT NULL \
         ORDER BY s.nom ASC, r.rowid ASC \
         LIMIT :limite",
        params! { "entity" => entity, "limite" => limite },
    )?;
    Ok(rows.iter().map(map_mandat).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_montant_passe_en_centimes_sans_flottant() {
        assert_eq!(en_centimes(Decimal::from_str("123.45").unwrap()), 12_345);
        assert_eq!(en_centimes(Decimal::from_str("0").unwrap()), 0);
        // 0,1 + 0,2 en f64 donnerait 30,000000000000004 centimes.
        let somme = Decimal::from_str("0.1").unwrap() + Decimal::from_str("0.2").unwrap();
        assert_eq!(en_centimes(somme), 30);
        // Demi hors de zero, comme le round() de PHP.
        assert_eq!(en_centimes(Decimal::from_str("1.005").unwrap()), 101);
        assert_eq!(en_centimes(Decimal::from_str("0.004").unwrap()), 0);
        // Un avoir (montant negatif) reste negatif : le tri est fait plus loin.
        assert_eq!(en_centimes(Decimal::from_str("-12.50").unwrap()), -1_250);
    }

    #[test]
    fn une_chaine_vide_n_est_pas_un_iban() {
        assert_eq!(non_vide(Some("   ".to_string())), None);
        assert_eq!(non_vide(None), None);
        assert_eq!(
            non_vide(Some(" FR7616807003483848689521643 ".to_string())),
            Some("FR7616807003483848689521643".to_string())
        );
    }
}
