//! Achats : fournisseurs et factures fournisseurs.
//!
//! Un fournisseur est un tiers comme les autres (`llx_societe`) marque
//! `fournisseur = 1` ; aucune table n'est ajoutee. Les factures fournisseurs
//! vivent dans `llx_facture_fourn` (le nom est bien celui-la en 24.0.1) et leurs
//! lignes dans `llx_facture_fourn_det`. Les commandes fournisseurs existent aussi
//! (`llx_commande_fournisseur`) mais ne sont pas encore couvertes ici.
//!
//! Regles du plan respectees : liste de colonnes explicite, filtre `entity`
//! systematique, montants lus via `CAST(... AS CHAR)` pour ne pas dependre de la
//! conversion `double` -> `Decimal` du pilote. Ce module est en lecture, sauf
//! `creer_fournisseur` qui n'ecrit que dans `llx_societe`.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row};
use rust_decimal::Decimal;

/// `llx_facture_fourn.fk_statut`, valeurs reprises de Dolibarr.
pub const STATUT_BROUILLON: i16 = 0;
pub const STATUT_VALIDEE: i16 = 1;
pub const STATUT_PAYEE: i16 = 2;
pub const STATUT_ABANDONNEE: i16 = 3;

/// Un fournisseur : une fiche `llx_societe` avec `fournisseur = 1`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fournisseur {
    pub rowid: i64,
    pub nom: String,
    pub code_fournisseur: Option<String>,
    pub address: Option<String>,
    pub zip: Option<String>,
    pub town: Option<String>,
    pub siret: Option<String>,
    pub tva_intra: Option<String>,
    pub email: Option<String>,
    pub statut: i16,
}

impl Fournisseur {
    /// Une fiche fermee (`llx_societe.statut = 0`) ne doit plus etre proposee.
    pub fn est_actif(&self) -> bool {
        self.statut != 0
    }
}

/// Une facture fournisseur, telle qu'elle apparait dans la liste des achats.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FactureFourn {
    pub rowid: i64,
    pub ref_: String,
    /// Reference du document chez le fournisseur (`ref_supplier`).
    pub ref_supplier: Option<String>,
    pub datef: Option<String>,
    pub fk_soc: i64,
    /// Nom du fournisseur, lu par jointure ; absent si la fiche a disparu.
    pub fournisseur: Option<String>,
    pub total_ht: Option<Decimal>,
    pub total_ttc: Option<Decimal>,
    pub fk_statut: i16,
    pub paye: bool,
}

impl FactureFourn {
    pub fn est_brouillon(&self) -> bool {
        self.fk_statut == STATUT_BROUILLON
    }
}

const SELECT_COLS: &str = "rowid, nom, code_fournisseur, address, zip, town, \
     siret, tva_intra, email, statut";

const FACTURE_COLS: &str = "f.rowid, f.ref, f.ref_supplier, \
     DATE_FORMAT(f.datef, '%Y-%m-%d'), f.fk_soc, s.nom, \
     CAST(f.total_ht AS CHAR), CAST(f.total_ttc AS CHAR), f.fk_statut, f.paye";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: Row) -> Fournisseur {
    Fournisseur {
        rowid: opt_i(&row, 0).unwrap_or(0),
        nom: opt_s(&row, 1).unwrap_or_default(),
        code_fournisseur: opt_s(&row, 2),
        address: opt_s(&row, 3),
        zip: opt_s(&row, 4),
        town: opt_s(&row, 5),
        siret: opt_s(&row, 6),
        tva_intra: opt_s(&row, 7),
        email: opt_s(&row, 8),
        statut: opt_i(&row, 9).unwrap_or(1) as i16,
    }
}

fn map_facture(row: Row) -> FactureFourn {
    FactureFourn {
        rowid: opt_i(&row, 0).unwrap_or(0),
        ref_: opt_s(&row, 1).unwrap_or_default(),
        ref_supplier: opt_s(&row, 2),
        datef: opt_s(&row, 3),
        fk_soc: opt_i(&row, 4).unwrap_or(0),
        fournisseur: opt_s(&row, 5),
        total_ht: opt_dec(&row, 6),
        total_ttc: opt_dec(&row, 7),
        fk_statut: opt_i(&row, 8).unwrap_or(0) as i16,
        paye: opt_i(&row, 9).unwrap_or(0) != 0,
    }
}

/// Fournisseurs de l'entite, tri par nom.
///
/// Le filtre est `fournisseur = 1` : les tiers qui sont a la fois client et
/// fournisseur (`client = 3`) apparaissent donc aussi, ce qui est voulu — un
/// fournisseur qui achete chez nous reste un fournisseur.
pub fn fournisseurs(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<Fournisseur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_societe \
         WHERE entity = ? AND fournisseur = 1 \
         ORDER BY nom ASC, rowid ASC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limit, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Un fournisseur par son `rowid`, `None` s'il n'existe pas dans l'entite.
pub fn fournisseur_par_id(
    pool: &Pool,
    entity: i64,
    rowid: i64,
) -> Result<Option<Fournisseur>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_societe \
         WHERE entity = ? AND rowid = ? AND fournisseur = 1"
    );
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// Factures fournisseurs de l'entite, les plus recentes d'abord.
pub fn factures_fournisseur(
    pool: &Pool,
    entity: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<FactureFourn>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {FACTURE_COLS} FROM llx_facture_fourn f \
         LEFT JOIN llx_societe s ON s.rowid = f.fk_soc AND s.entity = f.entity \
         WHERE f.entity = ? \
         ORDER BY f.datef DESC, f.rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limit, offset))?;
    Ok(rows.into_iter().map(map_facture).collect())
}

/// Total des achats de l'entite : somme des TTC des factures fournisseurs qui
/// ne sont plus des brouillons.
///
/// Les brouillons sont exclus parce qu'ils ne sont pas encore engages. Les
/// factures abandonnees (`fk_statut = 3`) sont comptees : elles ont ete validees
/// un jour, et les sortir du total demanderait de les distinguer a l'ecran. Le
/// resultat est arrondi au centime et vaut zero quand l'entite n'a aucune facture
/// fournisseur.
pub fn total_achats(pool: &Pool, entity: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let brut: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(total_ttc), 0) AS CHAR) FROM llx_facture_fourn \
         WHERE entity = :entity AND fk_statut > 0",
        params! { "entity" => entity },
    )?;
    Ok(brut
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .map(money::round_money)
        .unwrap_or(Decimal::ZERO))
}

/// Code fournisseur au format de `mod_codeclient_leopard`, comme le code client
/// de `party` : `CU` + `AAMM` + `-` + 5 chiffres.
///
/// Le numero suivant vient du plus grand suffixe de la periode en cours, plus un
/// decalage d'essai : en cas de collision (ecriture concurrente, index unique
/// `uk_societe_code_fournisseur`), on reessaie sans rejouer le meme numero.
fn next_code_fournisseur(
    conn: &mut mysql::PooledConn,
    entity: i64,
    offset: u32,
) -> Result<String, Box<dyn Error>> {
    let period: Option<String> = conn.query_first("SELECT DATE_FORMAT(NOW(), '%y%m')")?;
    let period = period.unwrap_or_else(|| "0001".to_string());
    let prefix = format!("CU{period}-");

    let dernier: Option<Option<i64>> = conn.exec_first(
        "SELECT MAX(CAST(SUBSTRING(code_fournisseur, 8) AS UNSIGNED)) \
         FROM llx_societe WHERE entity = ? AND code_fournisseur LIKE ?",
        (entity, format!("{prefix}%")),
    )?;
    let suivant = dernier.flatten().unwrap_or(0) + 1 + i64::from(offset);
    Ok(format!("{prefix}{suivant:05}"))
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

/// Insere la fiche fournisseur. `code_client` reste NULL : la fiche n'est pas un
/// client, et l'index unique `uk_societe_code_client` accepte plusieurs NULL.
fn inserer(
    conn: &mut mysql::PooledConn,
    entity: i64,
    nom: &str,
    code: &str,
) -> Result<i64, mysql::Error> {
    conn.exec_drop(
        "INSERT INTO llx_societe \
         (entity, nom, client, fournisseur, code_client, code_fournisseur, statut, datec) \
         VALUES (:entity, :nom, 0, 1, NULL, :code, 1, NOW())",
        params! { "entity" => entity, "nom" => nom, "code" => code },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Cree un fournisseur (`fournisseur = 1`, `client = 0`) et renvoie son `rowid`.
///
/// Sans `code`, le code fournisseur est genere au format `CU AAMM - 00001`. Avec
/// un `code`, il est repris tel quel et un doublon est refuse par une erreur
/// explicite plutot que renumerote en silence.
pub fn creer_fournisseur(
    pool: &Pool,
    entity: i64,
    nom: &str,
    code: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let nom = nom.trim();
    if nom.is_empty() {
        return Err("le nom du fournisseur est obligatoire".into());
    }
    let impose = code.map(str::trim).filter(|c| !c.is_empty());

    let mut conn = pool.get_conn()?;

    if let Some(code) = impose {
        return match inserer(&mut conn, entity, nom, code) {
            Ok(id) => Ok(id),
            Err(e) if is_duplicate(&e) => {
                Err(format!("le code fournisseur {code} est deja utilise").into())
            }
            Err(e) => Err(e.into()),
        };
    }

    for attempt in 0..5u32 {
        let code = next_code_fournisseur(&mut conn, entity, attempt)?;
        match inserer(&mut conn, entity, nom, &code) {
            Ok(id) => return Ok(id),
            Err(e) if is_duplicate(&e) && attempt < 4 => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer un code fournisseur unique".into())
}
