//! Journal d'evenements : `llx_actioncomm` de Dolibarr.
//!
//! Chaque evenement porte son `elementtype` (`propal`, `facture`, `societe`…)
//! et le `rowid` de l'objet concerne, exactement comme les evenements crees par
//! Dolibarr : ils s'affichent donc dans l'onglet « Agenda » de la fiche.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Ligne brute de `llx_actioncomm`.
type LigneEvenement = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
);

/// Code du type d'action utilise par Dolibarr pour les evenements automatiques.
pub const CODE_AUTOMATIQUE: &str = "AC_OTH_AUTO";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Evenement {
    pub rowid: i64,
    pub datep: Option<String>,
    pub label: String,
    pub elementtype: Option<String>,
    pub fk_element: Option<i64>,
    pub fk_soc: Option<i64>,
}

fn id_type(conn: &mut mysql::PooledConn) -> Option<i64> {
    conn.exec_first(
        "SELECT id FROM llx_c_actioncomm WHERE code = ? LIMIT 1",
        (CODE_AUTOMATIQUE,),
    )
    .ok()
    .flatten()
}

/// Enregistre un evenement date de maintenant.
pub fn noter(
    conn: &mut mysql::PooledConn,
    entity: i64,
    fk_soc: Option<i64>,
    elementtype: &str,
    fk_element: i64,
    label: &str,
) -> Result<i64, Box<dyn Error>> {
    let type_action = id_type(conn);
    let reference: Option<String> =
        conn.query_first("SELECT CONCAT('EVT', DATE_FORMAT(NOW(), '%Y%m%d%H%i%s'))")?;
    let reference = reference.unwrap_or_else(|| "EVT".to_string());

    conn.exec_drop(
        "INSERT INTO llx_actioncomm \
         (ref, datep, datec, label, fk_action, fk_soc, elementtype, fk_element, entity) \
         VALUES (:ref, NOW(), NOW(), :label, :action, :soc, :elementtype, :element, :entity)",
        params! {
            "ref" => reference,
            "label" => label,
            "action" => type_action,
            "soc" => fk_soc,
            "elementtype" => elementtype,
            "element" => fk_element,
            "entity" => entity,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Les derniers evenements de l'entite, tous objets confondus : c'est la vue
/// « Agenda » de Dolibarr.
pub fn recents(pool: &Pool, entity: i64, limit: u32) -> Result<Vec<Evenement>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<LigneEvenement> = conn.exec(
        "SELECT id, DATE_FORMAT(datep, '%Y-%m-%d %H:%i'), label, elementtype, \
              fk_element, fk_soc \
         FROM llx_actioncomm WHERE entity = :entity \
         ORDER BY datep DESC, id DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(lignes
        .into_iter()
        .map(|(rowid, datep, label, et, fe, fs)| Evenement {
            rowid,
            datep,
            label: label.unwrap_or_default(),
            elementtype: et,
            fk_element: fe,
            fk_soc: fs,
        })
        .collect())
}

/// Evenements rattaches a un objet, du plus recent au plus ancien.
pub fn pour_element(
    pool: &Pool,
    entity: i64,
    elementtype: &str,
    fk_element: i64,
) -> Result<Vec<Evenement>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<LigneEvenement> = conn.exec(
        "SELECT id, DATE_FORMAT(datep, '%Y-%m-%d %H:%i'), label, elementtype, \
              fk_element, fk_soc \
             FROM llx_actioncomm \
             WHERE entity = :entity AND elementtype = :elementtype AND fk_element = :element \
             ORDER BY datep DESC, id DESC",
        params! { "entity" => entity, "elementtype" => elementtype, "element" => fk_element },
    )?;
    Ok(lignes
        .into_iter()
        .map(|(rowid, datep, label, et, fe, fs)| Evenement {
            rowid,
            datep,
            label: label.unwrap_or_default(),
            elementtype: et,
            fk_element: fe,
            fk_soc: fs,
        })
        .collect())
}

/// Tous les evenements d'un tiers.
pub fn pour_tiers(
    pool: &Pool,
    entity: i64,
    fk_soc: i64,
    limite: u32,
) -> Result<Vec<Evenement>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<LigneEvenement> = conn.exec(
        "SELECT id, DATE_FORMAT(datep, '%Y-%m-%d %H:%i'), label, elementtype, \
              fk_element, fk_soc \
             FROM llx_actioncomm \
             WHERE entity = :entity AND fk_soc = :soc \
             ORDER BY datep DESC, id DESC LIMIT :limite",
        params! { "entity" => entity, "soc" => fk_soc, "limite" => limite },
    )?;
    Ok(lignes
        .into_iter()
        .map(|(rowid, datep, label, et, fe, fs)| Evenement {
            rowid,
            datep,
            label: label.unwrap_or_default(),
            elementtype: et,
            fk_element: fe,
            fk_soc: fs,
        })
        .collect())
}

pub fn supprimer_pour_element(
    pool: &Pool,
    entity: i64,
    elementtype: &str,
    fk_element: i64,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_actioncomm \
         WHERE entity = :entity AND elementtype = :elementtype AND fk_element = :element",
        params! { "entity" => entity, "elementtype" => elementtype, "element" => fk_element },
    )?;
    Ok(())
}

/// Cree un evenement d'agenda saisi a la main (type Rendez-vous).
pub fn creer_evenement(
    pool: &Pool,
    entity: i64,
    label: &str,
    date: Option<&str>,
    fk_soc: Option<i64>,
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let type_action: Option<i64> = conn.exec_first(
        "SELECT id FROM llx_c_actioncomm WHERE code = 'AC_RDV' LIMIT 1",
        (),
    )?;
    let reference: Option<String> =
        conn.query_first("SELECT CONCAT('EVT', DATE_FORMAT(NOW(), '%Y%m%d%H%i%s'))")?;
    conn.exec_drop(
        "INSERT INTO llx_actioncomm \
         (ref, datep, datec, label, fk_action, fk_soc, entity) \
         VALUES (:ref, COALESCE(:datep, NOW()), NOW(), :label, :action, :soc, :entity)",
        params! {
            "ref" => reference.unwrap_or_else(|| "EVT".to_string()),
            "datep" => date,
            "label" => label,
            "action" => type_action,
            "soc" => fk_soc,
            "entity" => entity,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> Option<Pool> {
        let dsn = std::env::var("DOLIBARR_DSN").ok()?;
        match crate::pool(&dsn) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("connexion impossible ({e}) : test ignore");
                None
            }
        }
    }

    #[test]
    fn note_et_relit_un_evenement() {
        let Some(p) = pool() else { return };
        // Element fictif propre a ce processus : deux executions de la suite
        // peuvent se croiser sur la meme base (hook + main).
        let element = 999_000_000 + std::process::id() as i64;
        // Restes d'une execution precedente interrompue.
        supprimer_pour_element(&p, 1, "facture", element).expect("nettoyage");
        let mut conn = p.get_conn().expect("connexion");
        let id = noter(&mut conn, 1, None, "facture", element, "Test evenement").expect("ecriture");
        assert!(id > 0);

        let evts = pour_element(&p, 1, "facture", element).expect("lecture");
        assert_eq!(evts.len(), 1);
        assert_eq!(evts[0].label, "Test evenement");
        assert_eq!(evts[0].elementtype.as_deref(), Some("facture"));

        supprimer_pour_element(&p, 1, "facture", element).expect("nettoyage");
        assert!(pour_element(&p, 1, "facture", element)
            .expect("relecture")
            .is_empty());
    }
}
