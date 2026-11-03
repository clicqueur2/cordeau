//! Droits d'acces par ecran, dans le modele Dolibarr.
//!
//! Dolibarr stocke les definitions dans `llx_rights_def` (id, libelle, module,
//! perms) et les attributions dans `llx_user_rights` (fk_user, fk_id). Ces
//! tables sont **vides** tant que les modules n'ont pas ete actives : on cree
//! donc nos propres definitions, sous le module `dolibarr_rs`, avec des
//! identifiants reserves (9001 et plus).
//!
//! Pour les ecrans dont on connait l'identifiant du coeur, on ecrit aussi la
//! definition Dolibarr et on **accepte les deux** : un utilisateur a qui
//! Dolibarr PHP a donne « Read invoices » (id 11) ouvre notre ecran des
//! factures, et une attribution faite chez nous se lit dans Dolibarr.

use std::collections::BTreeSet;
use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Une definition de droit.
pub struct Definition {
    pub id: i64,
    pub libelle: &'static str,
    /// Identifiant du coeur Dolibarr, quand il est connu.
    pub coeur: Option<i64>,
}

/// Droits geres par le noyau : un par ecran de travail.
///
/// Les identifiants du coeur sont ceux declares par les modules PHP :
/// `modFacture` donne 11 (lire), 12 (creer), 16 (paiement).
pub const DEFINITIONS: &[Definition] = &[
    Definition {
        id: 9001,
        libelle: "Tiers",
        coeur: None,
    },
    Definition {
        id: 9002,
        libelle: "Devis",
        coeur: None,
    },
    Definition {
        id: 9003,
        libelle: "Commandes",
        coeur: None,
    },
    Definition {
        id: 9004,
        libelle: "Factures et reglements",
        coeur: Some(11),
    },
    Definition {
        id: 9005,
        libelle: "Chantiers",
        coeur: None,
    },
    Definition {
        id: 9006,
        libelle: "Achats",
        coeur: None,
    },
    Definition {
        id: 9007,
        libelle: "Banques",
        coeur: None,
    },
    Definition {
        id: 9008,
        libelle: "Comptabilite et exports",
        coeur: None,
    },
    Definition {
        id: 9009,
        libelle: "Parametres",
        coeur: None,
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
        // Meme definition sous l'identifiant du coeur, quand il existe : c'est
        // cet identifiant que Dolibarr PHP utilise.
        if let Some(coeur) = d.coeur {
            conn.exec_drop(
                "INSERT INTO llx_rights_def \
                 (id, entity, libelle, module, module_origin, perms, subperms, type, bydefault, enabled) \
                 VALUES (:id, :entity, :libelle, 'facture', 'facture', 'lire', '', 'a', 0, '1') \
                 ON DUPLICATE KEY UPDATE libelle = VALUES(libelle)",
                params! {
                    "id" => coeur,
                    "entity" => entity,
                    "libelle" => d.libelle,
                },
            )?;
        }
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

/// Les droits qui ouvrent un chemin d'URL : le notre, et celui du coeur.
///
/// Le prefixe le plus long gagne : `/parametres/utilisateurs` reste dans
/// « Parametres ».
pub fn droits_du_chemin(chemin: &str) -> &'static [i64] {
    let carte: &[(&str, &'static [i64])] = &[
        ("/chantiers", &[9005]),
        ("/achats", &[9006]),
        ("/banques", &[9007]),
        ("/factures", &[9004, 11]),
        ("/reglements", &[9004, 11, 16]),
        ("/avoirs", &[9004, 11]),
        ("/devis", &[9002]),
        ("/commandes", &[9003]),
        ("/comptabilite", &[9008]),
        ("/export", &[9008]),
        ("/tiers", &[9001]),
        ("/contacts", &[9001]),
        ("/produits", &[9001]),
        ("/stock", &[9001]),
        ("/tickets", &[9003]),
        ("/parametres", &[9009]),
    ];
    carte
        .iter()
        .filter(|(prefixe, _)| chemin.starts_with(prefixe))
        .max_by_key(|(prefixe, _)| prefixe.len())
        .map(|(_, droits)| *droits)
        .unwrap_or(&[])
}

/// Premier droit exigible : pratique pour les tests et les affichages.
pub fn droit_du_chemin(chemin: &str) -> Option<i64> {
    droits_du_chemin(chemin).first().copied()
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

    #[test]
    fn les_identifiants_du_coeur_ouvrent_aussi() {
        // Un utilisateur a qui Dolibarr a donne « Read invoices » (11) passe.
        assert!(droits_du_chemin("/factures/12").contains(&11));
        // « Issue payments » (16) ouvre la saisie des reglements.
        assert!(droits_du_chemin("/reglements").contains(&16));
        // Les parametres restent hors du coeur : un seul identifiant.
        assert_eq!(droits_du_chemin("/parametres"), &[9009]);
    }
}
