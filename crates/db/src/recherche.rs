//! Recherche globale, faite par la base.
//!
//! Charger les listes en memoire et filtrer en Rust tient sur 900 tiers ; sur
//! 50 000 il ne tient plus. La recherche se fait donc en SQL : une requete par
//! objet, toutes menees dans la meme connexion, avec `LIKE` et un terme
//! **echappe** — un `%` tape dans la barre de recherche ne doit pas transformer
//! la requete en « tout ».
//!
//! # Une seule forme de resultat
//!
//! Chaque objet rend `(type, id, libelle, detail, lien)`. L'ecran n'a donc pas
//! dix cas a traiter : il groupe par type et affiche. Ajouter un objet a la
//! recherche, c'est ajouter une ligne a la table ci-dessous.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Une ligne de resultat, tous objets confondus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resultat {
    /// Le type, en clair : « Tiers », « Devis »...
    pub genre: &'static str,
    /// La page qui ouvre l'objet.
    pub base: &'static str,
    pub id: i64,
    pub libelle: String,
    pub detail: String,
}

/// Les objets cherches, avec la colonne qui porte le libelle et celle qui
/// porte le detail.
///
/// `table` est ecrit ici, jamais construit a partir de la saisie : le terme
/// n'arrive que comme **valeur de parametre**.
const OBJETS: &[(&str, &str, &str, &[&str])] = &[
    (
        "Tiers",
        "/tiers/",
        "llx_societe",
        &["nom", "code_client", "email", "town"],
    ),
    (
        "Contact",
        "/contacts/",
        "llx_socpeople",
        &["lastname", "firstname", "email", "phone"],
    ),
    ("Devis", "/devis/", "llx_propal", &["ref", "note_public"]),
    ("Commande", "/commandes/", "llx_commande", &["ref"]),
    (
        "Facture",
        "/factures/",
        "llx_facture",
        &["ref", "ref_client"],
    ),
    ("Contrat", "/contrats/", "llx_contrat", &["ref"]),
    ("Ticket", "/tickets/", "llx_ticket", &["ref", "subject"]),
    ("Produit", "/produits/", "llx_product", &["ref", "label"]),
    ("Chantier", "/chantiers/", "llx_projet", &["ref", "title"]),
];

/// Echappe les jokers de `LIKE`.
///
/// `%` et `_` sont des jokers en SQL : sans echappement, taper « 100% » ramene
/// toute la base. `\` echappe en MariaDB.
pub fn echapper(terme: &str) -> String {
    let mut s = String::with_capacity(terme.len());
    for c in terme.chars() {
        if matches!(c, '\\' | '%' | '_') {
            s.push('\\');
        }
        s.push(c);
    }
    s
}

/// Le libelle affichable et le detail, lus selon l'objet.
///
/// Chaque table nomme ses colonnes differemment ; on projette ici pour que
/// l'ecran n'ait qu'une forme a connaitre.
fn requete(table: &str, colonnes: &[&str]) -> String {
    let libelle = colonnes.first().copied().unwrap_or("''");
    let detail = if colonnes.len() > 1 {
        format!("CONCAT_WS(' · ', {})", colonnes[1..].join(", "))
    } else {
        "''".to_string()
    };
    let filtre = colonnes
        .iter()
        .map(|c| format!("COALESCE({c}, '') LIKE :motif"))
        .collect::<Vec<_>>()
        .join(" OR ");
    format!(
        "SELECT rowid, COALESCE({libelle}, ''), {detail} FROM {table} \
         WHERE entity = :entity AND ({filtre}) \
         ORDER BY rowid DESC LIMIT :limite"
    )
}

/// Cherche `terme` dans tous les objets, pour l'entite.
///
/// Une table absente (module non installe) est sautee : elle n'a rien a
/// chercher, et ce n'est pas une panne.
pub fn globale(
    pool: &Pool,
    entity: i64,
    terme: &str,
    limite: u32,
) -> Result<Vec<Resultat>, Box<dyn Error>> {
    let terme = terme.trim();
    if terme.is_empty() {
        return Ok(Vec::new());
    }
    let motif = format!("%{}%", echapper(terme));
    let mut conn = pool.get_conn()?;
    let mut sortie: Vec<Resultat> = Vec::new();

    for (genre, base, table, colonnes) in OBJETS {
        let presente: Option<i64> = conn.exec_first(
            "SELECT COUNT(*) FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = :t",
            params! { "t" => *table },
        )?;
        if presente.unwrap_or(0) == 0 {
            continue;
        }

        let sql = requete(table, colonnes);
        let lignes: Vec<(i64, String, String)> = conn.exec(
            sql,
            params! { "entity" => entity, "motif" => motif.as_str(), "limite" => limite.clamp(1, 200) },
        )?;
        for (id, libelle, detail) in lignes {
            sortie.push(Resultat {
                genre,
                base,
                id,
                libelle,
                detail,
            });
        }
    }

    Ok(sortie)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_jokers_sont_echappes() {
        assert_eq!(echapper("100%"), "100\\%");
        assert_eq!(echapper("a_b"), "a\\_b");
        assert_eq!(echapper("normal"), "normal");
        assert_eq!(echapper("\\"), "\\\\");
    }

    #[test]
    fn la_requete_ne_contient_aucune_donnee_saisie() {
        // La table et les colonnes viennent du code ; le terme n'apparait que
        // comme parametre. Une requete qui ne cite que des colonnes connues ne
        // peut pas etre detournee par la saisie.
        let sql = requete("llx_societe", &["nom", "code_client", "email"]);
        assert!(sql.contains("COALESCE(nom, '') LIKE :motif"));
        assert!(sql.contains("COALESCE(email, '') LIKE :motif"));
        assert!(!sql.contains("100%"));
    }
}
