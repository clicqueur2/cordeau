//! Categories : etiqueter les objets, comme Dolibarr.
//!
//! Dolibarr tient une table de categories et **une table de lien par famille
//! d'objet** : `llx_categorie_product`, `llx_categorie_societe`,
//! `llx_categorie_propal`… Une categorie porte un `type` numerique qui dit a
//! quelle famille elle appartient.
//!
//! Ces numeros ne sont pas devines : ils viennent de `Categorie::$MAP_ID` du
//! cceur (24.0.1), releve sur piece :
//!
//! | famille | type | table de lien | colonne |
//! |---|---|---|---|
//! | produit et service | 0 | `llx_categorie_product` | `fk_product` |
//! | fournisseur | 1 | `llx_categorie_societe` | `fk_soc` |
//! | client | 2 | `llx_categorie_societe` | `fk_soc` |
//! | contact | 4 | `llx_categorie_contact` | `fk_socpeople` |
//! | chantier | 6 | `llx_categorie_project` | `fk_project` |
//! | commande | 16 | `llx_categorie_order` | `fk_order` |
//! | facture | 17 | `llx_categorie_invoice` | `fk_invoice` |
//! | devis | 23 | `llx_categorie_propal` | `fk_propal` |
//!
//! Une categorie de clients et une categorie de produits ne se melangent pas :
//! le type d'une categorie decide des objets qu'elle peut recevoir.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Une famille d'objets etiquetables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Famille {
    Produit,
    Fournisseur,
    Client,
    Contact,
    Chantier,
    Commande,
    Facture,
    Devis,
}

impl Famille {
    /// Toutes les familles, dans l'ordre des types.
    pub const TOUTES: [Famille; 8] = [
        Famille::Produit,
        Famille::Fournisseur,
        Famille::Client,
        Famille::Contact,
        Famille::Chantier,
        Famille::Commande,
        Famille::Facture,
        Famille::Devis,
    ];

    /// Le code de l'URL et de l'ecran.
    pub fn code(self) -> &'static str {
        match self {
            Famille::Produit => "produit",
            Famille::Fournisseur => "fournisseur",
            Famille::Client => "client",
            Famille::Contact => "contact",
            Famille::Chantier => "chantier",
            Famille::Commande => "commande",
            Famille::Facture => "facture",
            Famille::Devis => "devis",
        }
    }

    pub fn depuis_code(code: &str) -> Option<Famille> {
        Famille::TOUTES.into_iter().find(|f| f.code() == code)
    }

    /// Le libelle affiche.
    pub fn libelle(self) -> &'static str {
        match self {
            Famille::Produit => "Produits et services",
            Famille::Fournisseur => "Fournisseurs",
            Famille::Client => "Clients",
            Famille::Contact => "Contacts",
            Famille::Chantier => "Chantiers",
            Famille::Commande => "Commandes",
            Famille::Facture => "Factures",
            Famille::Devis => "Devis",
        }
    }

    /// La valeur de `llx_categorie.type`, relevee dans `Categorie::$MAP_ID`.
    pub fn type_id(self) -> i32 {
        match self {
            Famille::Produit => 0,
            Famille::Fournisseur => 1,
            Famille::Client => 2,
            Famille::Contact => 4,
            Famille::Chantier => 6,
            Famille::Commande => 16,
            Famille::Facture => 17,
            Famille::Devis => 23,
        }
    }

    /// La table de lien.
    pub fn table(self) -> &'static str {
        match self {
            Famille::Produit => "llx_categorie_product",
            Famille::Fournisseur | Famille::Client => "llx_categorie_societe",
            Famille::Contact => "llx_categorie_contact",
            Famille::Chantier => "llx_categorie_project",
            Famille::Commande => "llx_categorie_order",
            Famille::Facture => "llx_categorie_invoice",
            Famille::Devis => "llx_categorie_propal",
        }
    }

    /// La colonne qui porte l'objet dans la table de lien.
    pub fn colonne(self) -> &'static str {
        match self {
            Famille::Produit => "fk_product",
            Famille::Fournisseur | Famille::Client => "fk_soc",
            Famille::Contact => "fk_socpeople",
            Famille::Chantier => "fk_project",
            Famille::Commande => "fk_order",
            Famille::Facture => "fk_invoice",
            Famille::Devis => "fk_propal",
        }
    }
}

/// Une categorie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Categorie {
    pub rowid: i64,
    pub libelle: String,
    pub description: String,
    pub couleur: Option<String>,
    pub parent: i64,
    pub visible: bool,
    /// Nombre d'objets etiquetes, quand on le demande.
    pub objets: i64,
}

/// Les categories d'une famille.
pub fn categories(
    pool: &Pool,
    entity: i64,
    famille: Famille,
) -> Result<Vec<Categorie>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // Le comptage se fait dans la table de lien de la famille : une categorie
    // peut exister sans etiqueter personne, et c'est un cas normal.
    let sql = format!(
        "SELECT c.rowid, c.label, COALESCE(c.description, ''), c.color, c.fk_parent, c.visible, \
                (SELECT COUNT(*) FROM {} l WHERE l.fk_categorie = c.rowid) \
         FROM llx_categorie c \
         WHERE c.entity = :entity AND c.type = :type \
         ORDER BY c.label ASC",
        famille.table()
    );
    let rows: Vec<mysql::Row> = conn.exec(
        sql,
        params! { "entity" => entity, "type" => famille.type_id() },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| Categorie {
            rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
            libelle: r
                .get_opt::<String, _>(1)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            description: r
                .get_opt::<String, _>(2)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            couleur: r.get_opt::<String, _>(3).and_then(|v| v.ok()),
            parent: r.get_opt::<i64, _>(4).and_then(|v| v.ok()).unwrap_or(0),
            visible: r.get_opt::<i32, _>(5).and_then(|v| v.ok()).unwrap_or(1) != 0,
            objets: r.get_opt::<i64, _>(6).and_then(|v| v.ok()).unwrap_or(0),
        })
        .collect())
}

/// Cree une categorie, ou rend celle qui porte deja ce libelle dans la famille.
pub fn creer(
    pool: &Pool,
    entity: i64,
    famille: Famille,
    libelle: &str,
    description: &str,
    couleur: Option<&str>,
    parent: i64,
) -> Result<i64, Box<dyn Error>> {
    let libelle = libelle.trim();
    if libelle.is_empty() {
        return Err("le libelle est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;

    if let Some(id) = conn.exec_first::<i64, _, _>(
        "SELECT rowid FROM llx_categorie \
         WHERE entity = :entity AND type = :type AND label = :label",
        params! { "entity" => entity, "type" => famille.type_id(), "label" => libelle },
    )? {
        return Ok(id);
    }

    conn.exec_drop(
        "INSERT INTO llx_categorie \
           (entity, fk_parent, label, type, description, color, visible, position, date_creation) \
         VALUES (:entity, :parent, :label, :type, :description, :color, 1, 0, NOW())",
        params! {
            "entity" => entity,
            // `fk_parent` est NOT NULL : 0 veut dire « racine ».
            "parent" => parent.max(0),
            "label" => libelle,
            "type" => famille.type_id(),
            "description" => description.trim(),
            "color" => couleur.map(|c| c.trim().to_string()).filter(|c| !c.is_empty()),
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Etiquette un objet. Rejouer l'affectation ne cree pas de doublon.
pub fn affecter(
    pool: &Pool,
    entity: i64,
    famille: Famille,
    fk_categorie: i64,
    fk_objet: i64,
) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;

    // La categorie doit exister, etre de cette entite et de cette famille :
    // sinon on etiquetterait un client avec une categorie de produits.
    let bonne: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_categorie \
         WHERE rowid = :id AND entity = :entity AND type = :type",
        params! { "id" => fk_categorie, "entity" => entity, "type" => famille.type_id() },
    )?;
    if bonne.is_none() {
        return Err(format!("la categorie {fk_categorie} n'existe pas dans cette famille").into());
    }

    let table = famille.table();
    let colonne = famille.colonne();
    let sql =
        format!("SELECT fk_categorie FROM {table} WHERE fk_categorie = :cat AND {colonne} = :obj");
    let existe: Option<i64> = conn.exec_first(
        sql.as_str(),
        params! { "cat" => fk_categorie, "obj" => fk_objet },
    )?;
    if existe.is_some() {
        return Ok(());
    }

    let sql = format!("INSERT INTO {table} (fk_categorie, {colonne}) VALUES (:cat, :obj)");
    conn.exec_drop(
        sql.as_str(),
        params! { "cat" => fk_categorie, "obj" => fk_objet },
    )?;
    Ok(())
}

/// Retire une etiquette.
pub fn retirer(
    pool: &Pool,
    famille: Famille,
    fk_categorie: i64,
    fk_objet: i64,
) -> Result<usize, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let table = famille.table();
    let colonne = famille.colonne();
    let sql = format!("DELETE FROM {table} WHERE fk_categorie = :cat AND {colonne} = :obj");
    conn.exec_drop(
        sql.as_str(),
        params! { "cat" => fk_categorie, "obj" => fk_objet },
    )?;
    Ok(conn.affected_rows() as usize)
}

/// Les categories qui etiquettent un objet donne.
pub fn d_un_objet(
    pool: &Pool,
    famille: Famille,
    fk_objet: i64,
) -> Result<Vec<Categorie>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT c.rowid, c.label, COALESCE(c.description, ''), c.color \
         FROM llx_categorie c \
         JOIN {} l ON l.fk_categorie = c.rowid \
         WHERE l.{} = :obj \
         ORDER BY c.label ASC",
        famille.table(),
        famille.colonne()
    );
    let rows: Vec<(i64, String, String, Option<String>)> =
        conn.exec(sql, params! { "obj" => fk_objet })?;

    Ok(rows
        .into_iter()
        .map(|(rowid, libelle, description, couleur)| Categorie {
            rowid,
            libelle,
            description,
            couleur,
            parent: 0,
            visible: true,
            objets: 1,
        })
        .collect())
}

#[cfg(test)]
mod tests_base {
    //! La recette, en test : sur la vraie base quand `DOLIBARR_DSN` est posee.

    use super::*;

    // Numero propre a ce module : deux modules qui partagent une
    // entite s'effacent mutuellement leurs lignes.
    const ENTITE: i64 = 998;
    /// Les tests de ce module partagent une entite et une base MySQL : ils
    /// doivent s'executer **l'un apres l'autre**. Deux tests simultanes se
    /// supprimaient mutuellement leurs lignes (`nettoyer` efface toute
    /// l'entite), et la meme reference de recette entrait en collision — d'ou
    /// des echecs aleatoires qui n'avaient rien a voir avec le code teste.
    fn verrou_entite() -> std::sync::MutexGuard<'static, ()> {
        static VERROU: std::sync::Mutex<()> = std::sync::Mutex::new(());
        VERROU.lock().unwrap_or_else(|e| e.into_inner())
    }

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

    fn nettoyer(pool: &Pool) {
        let mut conn = pool.get_conn().expect("connexion");
        let _ = conn.exec_drop(
            "DELETE FROM llx_categorie_societe WHERE fk_categorie IN \
               (SELECT rowid FROM (SELECT rowid FROM llx_categorie WHERE entity = :e) x)",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_categorie WHERE entity = :e",
            params! { "e" => ENTITE },
        );
    }

    #[test]
    fn une_categorie_se_cree_s_affecte_et_se_retire() {
        let _verrou = verrou_entite();
        let Some(p) = pool() else { return };
        nettoyer(&p);

        let mut conn = p.get_conn().expect("connexion");
        let soc: i64 = conn
            .query_first("SELECT rowid FROM llx_societe ORDER BY rowid LIMIT 1")
            .expect("requete tiers")
            .expect("aucun tiers en base");

        let cat = creer(
            &p,
            ENTITE,
            Famille::Client,
            "Bricoleurs",
            "",
            Some("#8a5a2b"),
            0,
        )
        .expect("creation");
        // Recreer le meme libelle rend la meme categorie.
        assert_eq!(
            creer(&p, ENTITE, Famille::Client, "Bricoleurs", "", None, 0).unwrap(),
            cat
        );
        assert_eq!(categories(&p, ENTITE, Famille::Client).unwrap().len(), 1);

        // Une categorie de produits du meme libelle est une autre categorie.
        let cat_prod = creer(&p, ENTITE, Famille::Produit, "Bricoleurs", "", None, 0).unwrap();
        assert_ne!(cat_prod, cat, "les familles sont séparées");

        // L'objet doit exister pour la cle etrangere : on utilise un vrai tiers.
        affecter(&p, ENTITE, Famille::Client, cat, soc).expect("affectation");

        let sur_lui = d_un_objet(&p, Famille::Client, soc).expect("categories de l'objet");
        assert_eq!(sur_lui.len(), 1);
        assert_eq!(sur_lui[0].libelle, "Bricoleurs");

        // Rejouer ne cree pas de doublon.
        affecter(&p, ENTITE, Famille::Client, cat, soc).expect("rejeu");
        assert_eq!(d_un_objet(&p, Famille::Client, soc).unwrap().len(), 1);

        // Une categorie de produits ne peut pas etiqueter un tiers.
        assert!(affecter(&p, ENTITE, Famille::Client, cat_prod, soc).is_err());

        // Une categorie d'une autre entite est refusee.
        assert!(affecter(&p, ENTITE + 1, Famille::Client, cat, soc).is_err());

        // Le comptage suit.
        let listes = categories(&p, ENTITE, Famille::Client).unwrap();
        assert_eq!(listes[0].objets, 1);
        assert_eq!(listes[0].couleur.as_deref(), Some("#8a5a2b"));

        assert_eq!(retirer(&p, Famille::Client, cat, soc).unwrap(), 1);
        assert_eq!(retirer(&p, Famille::Client, cat, soc).unwrap(), 0);

        nettoyer(&p);
        assert!(categories(&p, ENTITE, Famille::Client).unwrap().is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_types_viennent_du_coeur() {
        // Releves dans Categorie::$MAP_ID de Dolibarr 24.0.1.
        assert_eq!(Famille::Produit.type_id(), 0);
        assert_eq!(Famille::Client.type_id(), 2);
        assert_eq!(Famille::Contact.type_id(), 4);
        assert_eq!(Famille::Chantier.type_id(), 6);
        assert_eq!(Famille::Commande.type_id(), 16);
        assert_eq!(Famille::Facture.type_id(), 17);
        assert_eq!(Famille::Devis.type_id(), 23);
    }

    #[test]
    fn client_et_fournisseur_partagent_la_table_des_tiers() {
        assert_eq!(Famille::Client.table(), "llx_categorie_societe");
        assert_eq!(Famille::Fournisseur.table(), "llx_categorie_societe");
        assert_eq!(Famille::Client.colonne(), "fk_soc");
    }

    #[test]
    fn chaque_famille_a_sa_table_et_sa_colonne() {
        for f in Famille::TOUTES {
            assert!(f.table().starts_with("llx_categorie_"), "{f:?}");
            assert!(f.colonne().starts_with("fk_"), "{f:?}");
        }
    }

    #[test]
    fn les_familles_se_relisent_par_leur_code() {
        for f in Famille::TOUTES {
            assert_eq!(Famille::depuis_code(f.code()), Some(f));
        }
        assert_eq!(Famille::depuis_code("licorne"), None);
    }

    #[test]
    fn les_types_sont_tous_differents_sauf_produit_et_service() {
        let mut vus = std::collections::BTreeSet::new();
        for f in Famille::TOUTES {
            let t = f.type_id();
            // Client et fournisseur sont deux familles distinctes dans Dolibarr
            // (2 et 1) : aucun doublon ne doit subsister ici.
            assert!(vus.insert(t), "type en double : {t} ({f:?})");
        }
    }
}
