//! Droits d'acces par ecran, dans le modele Dolibarr.
//!
//! Dolibarr stocke les definitions dans `llx_rights_def` (id, libelle, module,
//! perms) et les attributions dans `llx_user_rights` (fk_user, fk_id). Ces
//! tables sont **vides** tant que les modules n'ont pas ete actives : on cree
//! donc nos propres definitions, sous le module `dolibarr_rs`, avec des
//! identifiants reserves (9001 et plus) qui ne peuvent pas entrer en collision
//! avec un identifiant du coeur.

use std::collections::BTreeSet;
use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Une definition de droit.
pub struct Definition {
    pub id: i64,
    pub libelle: &'static str,
}

/// Droits geres par le noyau : un par ecran de travail.
pub const DEFINITIONS: &[Definition] = &[
    Definition {
        id: 9001,
        libelle: "Tiers",
    },
    Definition {
        id: 9002,
        libelle: "Devis",
    },
    Definition {
        id: 9003,
        libelle: "Commandes",
    },
    Definition {
        id: 9004,
        libelle: "Factures et reglements",
    },
    Definition {
        id: 9005,
        libelle: "Chantiers",
    },
    Definition {
        id: 9006,
        libelle: "Achats",
    },
    Definition {
        id: 9007,
        libelle: "Banques",
    },
    Definition {
        id: 9008,
        libelle: "Comptabilite et exports",
    },
    Definition {
        id: 9009,
        libelle: "Parametres",
    },
];

/// Cree les definitions manquantes. Idempotent.
pub fn assurer_definitions(pool: &Pool, entity: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    for d in DEFINITIONS {
        conn.exec_drop(
            "INSERT INTO llx_rights_def \
             (id, entity, libelle, module, module_origin, perms, subperms, type, bydefault, enabled) \
             VALUES (:id, :entity, :libelle, 'dolibarr_rs', 'dolibarr_rs', 'ecran', :sub, 'r', 0, '1') \
             ON DUPLICATE KEY UPDATE libelle = VALUES(libelle)",
            params! {
                "id" => d.id,
                "entity" => entity,
                "libelle" => d.libelle,
                "sub" => d.id.to_string(),
            },
        )?;
    }
    Ok(())
}

/// Droits attribues a un utilisateur.
pub fn droits(pool: &Pool, fk_user: i64) -> Result<BTreeSet<i64>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT fk_id FROM llx_user_rights WHERE fk_user = :user",
        params! { "user" => fk_user },
    )?;
    Ok(ids.into_iter().collect())
}

/// Attribue ou retire un droit. Une attribution en double n'est pas une erreur.
pub fn definir(
    pool: &Pool,
    entity: i64,
    fk_user: i64,
    id_droit: i64,
    actif: bool,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    if actif {
        conn.exec_drop(
            "INSERT IGNORE INTO llx_user_rights (entity, fk_user, fk_id) \
             VALUES (:entity, :user, :droit)",
            params! { "entity" => entity, "user" => fk_user, "droit" => id_droit },
        )?;
    } else {
        conn.exec_drop(
            "DELETE FROM llx_user_rights WHERE fk_user = :user AND fk_id = :droit",
            params! { "user" => fk_user, "droit" => id_droit },
        )?;
    }
    Ok(())
}

/// Droit exige par un chemin d'URL, s'il y en a un.
///
/// Le prefixe le plus long gagne : `/parametres/utilisateurs` reste dans
/// « Parametres ».
pub fn droit_du_chemin(chemin: &str) -> Option<i64> {
    let carte: &[(&str, i64)] = &[
        ("/chantiers", 9005),
        ("/achats", 9006),
        ("/banques", 9007),
        ("/factures", 9004),
        ("/reglements", 9004),
        ("/avoirs", 9004),
        ("/devis", 9002),
        ("/commandes", 9003),
        ("/comptabilite", 9008),
        ("/export", 9008),
        ("/tiers", 9001),
        ("/produits", 9001),
        ("/parametres", 9009),
    ];
    carte
        .iter()
        .filter(|(prefixe, _)| chemin.starts_with(prefixe))
        .max_by_key(|(prefixe, _)| prefixe.len())
        .map(|(_, droit)| *droit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_prefixe_le_plus_long_gagne() {
        assert_eq!(droit_du_chemin("/factures/12/pdf"), Some(9004));
        assert_eq!(droit_du_chemin("/parametres/utilisateurs"), Some(9009));
        assert_eq!(droit_du_chemin("/export/ventes.csv"), Some(9008));
        assert_eq!(droit_du_chemin("/"), None);
    }
}
