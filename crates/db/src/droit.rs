//! Droits d'acces par ecran, dans le modele Dolibarr.
//!
//! Dolibarr stocke les definitions dans `llx_rights_def` (id, libelle, module,
//! perms) et les attributions dans `llx_user_rights` (fk_user, fk_id).
//!
//! Nos propres droits portent des identifiants reserves (9001 et plus). Pour
//! chaque ecran dont le module Dolibarr est connu, on ecrit **aussi** la
//! definition du coeur et on accepte ses identifiants : un utilisateur a qui
//! Dolibarr PHP a donne « Read invoices » (11) ouvre notre ecran des factures,
//! et une attribution faite chez nous se lit dans Dolibarr.
//!
//! Les identifiants sont releves dans les `mod*.class.php` du coeur 24.0.1.

use std::collections::BTreeSet;
use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Une definition de droit.
pub struct Definition {
    pub id: i64,
    pub libelle: &'static str,
    /// Identifiants du coeur Dolibarr qui ouvrent le meme ecran.
    pub coeur: &'static [i64],
}

/// Droits geres par le noyau : un par ecran de travail.
pub const DEFINITIONS: &[Definition] = &[
    // modSociete : 241 lire clients, 242 lire fournisseurs.
    Definition {
        id: 9001,
        libelle: "Tiers",
        coeur: &[241, 242],
    },
    // modPropale : 21 lire.
    Definition {
        id: 9002,
        libelle: "Devis",
        coeur: &[21],
    },
    // modCommande : 81 lire.
    Definition {
        id: 9003,
        libelle: "Commandes",
        coeur: &[81],
    },
    // modFacture : 11 lire, 16 saisir un reglement.
    Definition {
        id: 9004,
        libelle: "Factures et reglements",
        coeur: &[11, 16],
    },
    // modProjet : 41 lire (partages), 141 lire tout.
    Definition {
        id: 9005,
        libelle: "Chantiers",
        coeur: &[41, 141],
    },
    // modFournisseur : 1181 consulter les fournisseurs.
    Definition {
        id: 9006,
        libelle: "Achats",
        coeur: &[1181],
    },
    // modBanque : 111 lire, 114 rapprocher.
    Definition {
        id: 9007,
        libelle: "Banques",
        coeur: &[111, 114],
    },
    // modComptabilite : 95 lire CA, bilans, resultats.
    Definition {
        id: 9008,
        libelle: "Comptabilite et exports",
        coeur: &[95],
    },
    // Les parametres n'ont pas d'equivalent : c'est notre administration.
    Definition {
        id: 9009,
        libelle: "Parametres",
        coeur: &[],
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
        // Meme definition sous les identifiants du coeur : c'est eux que
        // Dolibarr PHP utilise.
        for coeur in d.coeur {
            conn.exec_drop(
                "INSERT INTO llx_rights_def \
                 (id, entity, libelle, module, module_origin, perms, subperms, type, bydefault, enabled) \
                 VALUES (:id, :entity, :libelle, 'dolibarr_rs', 'dolibarr_rs', 'ecran', :sub, 'r', 0, '1') \
                 ON DUPLICATE KEY UPDATE libelle = VALUES(libelle)",
                params! {
                    "id" => coeur,
                    "entity" => entity,
                    "libelle" => d.libelle,
                    "sub" => coeur.to_string(),
                },
            )?;
        }
    }
    Ok(())
}

/// Cree (ou met a jour) une definition de droit isolee.
///
/// Les modules declaratifs portent leurs propres droits : on les ecrit dans
/// `llx_rights_def` comme les notres, pour qu'une attribution faite ici se
/// lise dans Dolibarr.
pub fn assurer_un(pool: &Pool, entity: i64, id: i64, libelle: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_rights_def \
         (id, entity, libelle, module, module_origin, perms, subperms, type, bydefault, enabled) \
         VALUES (:id, :entity, :libelle, 'dolibarr_rs', 'dolibarr_rs', 'ecran', :sub, 'r', 0, '1') \
         ON DUPLICATE KEY UPDATE libelle = VALUES(libelle)",
        params! {
            "id" => id,
            "entity" => entity,
            "libelle" => libelle,
            "sub" => id.to_string(),
        },
    )?;
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

/// Les droits qui ouvrent un chemin d'URL : le notre et ceux du coeur.
///
/// Le prefixe le plus long gagne : `/parametres/utilisateurs` reste dans
/// « Parametres ».
pub fn droits_du_chemin(chemin: &str) -> &'static [i64] {
    let carte: &[(&str, &'static [i64])] = &[
        ("/chantiers", &[9005, 41, 141]),
        ("/achats", &[9006, 1181]),
        ("/banques", &[9007, 111, 114]),
        ("/factures", &[9004, 11]),
        ("/reglements", &[9004, 11, 16]),
        ("/avoirs", &[9004, 11]),
        ("/devis", &[9002, 21]),
        ("/commandes", &[9003, 81]),
        ("/comptabilite", &[9008, 95]),
        ("/export", &[9008, 95]),
        ("/tiers/doublons", &[9001, 241]),
        ("/tiers", &[9001, 241, 242]),
        ("/contacts", &[9001, 241, 242]),
        ("/produits", &[9001, 31]),
        ("/stock", &[9001, 31]),
        ("/lots", &[9001, 31]),
        ("/tickets", &[9003, 56001]),
        ("/documents", &[9009, 2501]),
        ("/agenda", &[9009, 2401, 2411]),
        ("/parametres", &[9009]),
        // Listes en lecture seule : chacune reprend le droit de son module
        // Dolibarr, pour qu'une autorisation donnee chez Dolibarr ouvre l'ecran.
        ("/liste/Adherent", &[9001, 241]),
        ("/liste/Categorie", &[9001, 31]),
        ("/liste/Tax", &[9009]),
        ("/liste/Bookmark", &[9009, 331]),
        ("/liste/Mailing", &[9009, 221]),
        ("/liste/SupplierOrder", &[9006, 1152, 1182]),
        ("/liste/PaymentByBankTransfer", &[9007, 561]),
        ("/liste/Prelevement", &[9007, 151]),
        ("/liste/Ficheinter", &[9003, 61]),
        ("/liste/Salaries", &[9009, 511, 512]),
        ("/liste/Loan", &[9008, 521]),
        ("/liste/Asset", &[9008, 51001]),
        ("/liste/Resource", &[9009, 63001]),
        ("/liste/Accounting", &[9008, 50411]),
        ("/liste/ExpenseReport", &[9009, 4001]),
        ("/liste/Holiday", &[9009]),
        ("/liste/Don", &[9001, 241]),
        ("/liste/Reception", &[9006, 1181]),
        ("/liste/ProductBatch", &[9001, 31]),
        // Ecrans qui ont remplace une liste generique : ils reprennent les
        // memes identifiants, pour qu'une autorisation deja donnee continue de
        // valoir.
        ("/contrats", &[9002, 21]),
        ("/salaires", &[9009, 511, 512]),
        ("/recrutement", &[9009, 4001]),
        ("/ressources", &[9009, 63001]),
        ("/reservations", &[9009, 2401]),
        ("/inscriptions", &[9003, 61]),
        ("/variantes", &[9001, 31]),
        ("/colonnes", &[9009]),
        ("/style", &[9009]),
        ("/mrp", &[9001, 31]),
        ("/receptions", &[9002, 21]),
        ("/livraisons", &[9002, 21]),
        ("/expeditions", &[9002, 21]),
        ("/interventions", &[9002, 21]),
        ("/relances", &[9004, 11]),
        ("/recurrences", &[9004, 11]),
        ("/statistiques", &[9008, 95]),
        ("/echeancier", &[9008, 95]),
        ("/categories", &[9009]),
        ("/presets", &[9009]),
        ("/recherche", &[9001, 241]),
        ("/campagnes", &[9009, 221]),
        ("/reglages", &[9009]),
        ("/utilisateurs", &[9009]),
        ("/securite", &[9009]),
        ("/marges", &[9008, 95]),
        ("/nomenclatures", &[9001, 31]),
        ("/notes-frais", &[9009, 4001]),
        ("/conges", &[9009]),
        ("/mailings", &[9009, 221]),
        ("/connaissances", &[9009]),
        ("/sites", &[9009]),
        ("/prets", &[9008, 521]),
        ("/inventaires", &[9001, 31]),
        ("/postes", &[9009, 63001]),
        ("/adherents", &[9001, 241]),
        ("/partenariats", &[9001, 241]),
        ("/dons", &[9001, 241]),
        ("/transferts", &[9001, 31]),
        ("/achats-fournisseur", &[9006, 1181]),
        ("/immobilisations", &[9008, 51001]),
        ("/caisse", &[9009, 90]),
    ];
    carte
        .iter()
        .filter(|(prefixe, _)| chemin.starts_with(prefixe))
        .max_by_key(|(prefixe, _)| prefixe.len())
        .map(|(_, droits)| *droits)
        .unwrap_or(&[])
}

/// Les droits d'**ecriture** qui ouvrent un chemin.
///
/// Dolibarr separe lire / creer / valider / supprimer (une facture a 11 pour
/// lire et 12 pour creer ou modifier). Notre noyau n'avait qu'un droit par
/// ecran : qui pouvait lire pouvait donc valider et facturer. Ces identifiants
/// sont les notres (9001 et suivants sont la lecture) et n'existent pas chez
/// Dolibarr, ce qui est voulu : on n'invente pas la semantique de ses ids.
pub fn droits_ecriture_du_chemin(chemin: &str) -> &'static [i64] {
    let carte: &[(&str, &'static [i64])] = &[
        ("/chantiers", &[9016]),
        ("/achats", &[9015]),
        ("/banques", &[9017]),
        ("/factures", &[9014]),
        ("/reglements", &[9014]),
        ("/avoirs", &[9014]),
        ("/devis", &[9012]),
        ("/commandes", &[9013]),
        ("/comptabilite", &[9018]),
        ("/export", &[9018]),
        ("/tiers", &[9011]),
        ("/contacts", &[9011]),
        ("/produits", &[9011]),
        ("/stock", &[9011]),
        ("/tickets", &[9013]),
        ("/documents", &[9019]),
        ("/agenda", &[9019]),
        ("/campagnes", &[9014]),
        ("/recurrences", &[9014]),
        ("/liste", &[9019]),
        ("/parametres", &[9019]),
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
        // « Read invoices » (11) chez Dolibarr ouvre nos factures.
        assert!(droits_du_chemin("/factures/12").contains(&11));
        // « Issue payments » (16) ouvre la saisie des reglements.
        assert!(droits_du_chemin("/reglements").contains(&16));
        // « Read thirdparties » (241) ouvre les tiers et les contacts.
        assert!(droits_du_chemin("/tiers/3").contains(&241));
        // « Read projects » (41) ouvre les chantiers.
        assert!(droits_du_chemin("/chantiers").contains(&41));
        // Les parametres restent hors du coeur : un seul identifiant.
        assert_eq!(droits_du_chemin("/parametres"), &[9009]);
    }
}
