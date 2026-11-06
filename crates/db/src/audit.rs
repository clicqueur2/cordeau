//! Audit d'une base Dolibarr, **en lecture seule**.
//!
//! Ce module existe pour une seule situation : brancher le noyau sur une base
//! qui n'est pas la sienne — une copie de production, une base d'un client, une
//! sauvegarde qu'on vient de restaurer — et regarder ce qu'il y voit, **sans
//! rien pouvoir changer**.
//!
//! # La garantie n'est pas une promesse
//!
//! Toutes les requetes de ce fichier sont des `SELECT`. Ce n'est pas suffisant :
//! une fonction appelee par erreur, un `INSERT` ajoute dans six mois, et
//! l'audit ecrit dans la base du client. La protection est donc **celle de la
//! base elle-meme** : la connexion est ouverte dans une **transaction en lecture
//! seule** (`START TRANSACTION READ ONLY`). MariaDB refuse alors toute ecriture,
//! y compris si le code se trompe.
//!
//! C'est le meme principe que le cloisonnement par entite : on ne demande pas au
//! code de ne pas deborder, on l'empeche de pouvoir.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Le nombre de tables du schema Dolibarr 24.0.1.
pub const TABLES_ATTENDUES: i64 = 410;

/// Une table surveillee, avec son compte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compte {
    /// Nom de la table, sans le prefixe `llx_`.
    pub table: String,
    /// Ce que la table contient, en francais.
    pub libelle: String,
    pub lignes: i64,
}

/// Ce que l'audit a vu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rapport {
    pub base: String,
    pub tables: i64,
    pub tables_attendues: i64,
    /// Les societes declarees : `(rowid, nom)`.
    pub entites: Vec<(i64, String)>,
    pub comptes: Vec<Compte>,
    /// Nombre d'utilisateurs actifs, toutes entites.
    pub utilisateurs: i64,
}

impl Rapport {
    /// Le schema est-il complet ?
    pub fn schema_complet(&self) -> bool {
        self.tables >= self.tables_attendues
    }

    /// Les tables surveillees qui sont **vides**. Une base de production n'a
    /// pas de factures vides : une table vide sur une base qu'on croit vivante
    /// signale une copie ratee.
    pub fn vides(&self) -> Vec<&Compte> {
        self.comptes.iter().filter(|c| c.lignes == 0).collect()
    }

    /// Le total des objets commerciaux comptes.
    pub fn total_objets(&self) -> i64 {
        self.comptes.iter().map(|c| c.lignes).sum()
    }
}

/// Les tables que l'audit compte, et le nom qu'on leur donne.
///
/// Cette liste est **courte et explicite** : ce sont les objets dont l'absence
/// se remarque tout de suite. Compter les quatre-vingt-dix tables du noyau
/// donnerait un tableau illisible sans rien apprendre de plus.
pub const SURVEILLEES: &[(&str, &str)] = &[
    ("societe", "Tiers"),
    ("facture", "Factures client"),
    ("propal", "Devis"),
    ("commande", "Commandes client"),
    ("facture_fourn", "Factures fournisseur"),
    ("product", "Produits et services"),
    ("projet", "Projets et chantiers"),
    ("paiement", "Règlements"),
    ("accounting_bookkeeping", "Écritures comptables"),
    ("ticket", "Tickets"),
    ("contrat", "Contrats"),
    ("expedition", "Expéditions"),
    ("user", "Utilisateurs"),
];

/// Une facture telle que le noyau la lit, pour comparaison a l'ecran.
///
/// Un compteur ne prouve rien : « 14 factures » ne dit pas si les montants
/// sortent justes. Cet apercu sert a poser la meme facture cote a cote —
/// Dolibarr d'un cote, Cordeau de l'autre — et a voir si les totaux tombent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApercuFacture {
    pub reference: String,
    pub client: String,
    pub date: String,
    pub total_ht: String,
    pub total_ttc: String,
    pub statut: i16,
}

/// Les dernieres factures, lues exactement comme le fait l'ecran.
///
/// Seules des colonnes existantes sont lues ; aucune jointure facultative. Si
/// un tiers a ete supprime, la facture sort avec un client vide plutot que de
/// disparaitre : une facture orpheline est justement ce qu'il faut voir.
pub fn apercu_factures(
    pool: &Pool,
    entity: i64,
    limite: u32,
) -> Result<Vec<ApercuFacture>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.query_drop("START TRANSACTION READ ONLY")?;

    let resultat = (|| -> Result<Vec<ApercuFacture>, Box<dyn Error>> {
        let rows: Vec<mysql::Row> = conn.exec(
            "SELECT f.ref, COALESCE(s.nom, ''), COALESCE(DATE_FORMAT(f.datef, '%Y-%m-%d'), ''), \
                    CAST(COALESCE(f.total_ht, 0) AS CHAR), \
                    CAST(COALESCE(f.total_ttc, 0) AS CHAR), COALESCE(f.fk_statut, 0) \
             FROM llx_facture f LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
             WHERE f.entity = :entity \
             ORDER BY f.rowid DESC LIMIT :limite",
            params! { "entity" => entity, "limite" => limite.clamp(1, 50) },
        )?;
        Ok(rows
            .into_iter()
            .map(|r| ApercuFacture {
                reference: r
                    .get_opt::<String, _>(0)
                    .and_then(|v| v.ok())
                    .unwrap_or_default(),
                client: r
                    .get_opt::<String, _>(1)
                    .and_then(|v| v.ok())
                    .unwrap_or_default(),
                date: r
                    .get_opt::<String, _>(2)
                    .and_then(|v| v.ok())
                    .unwrap_or_default(),
                total_ht: r
                    .get_opt::<String, _>(3)
                    .and_then(|v| v.ok())
                    .unwrap_or_default(),
                total_ttc: r
                    .get_opt::<String, _>(4)
                    .and_then(|v| v.ok())
                    .unwrap_or_default(),
                statut: r.get_opt::<i64, _>(5).and_then(|v| v.ok()).unwrap_or(0) as i16,
            })
            .collect())
    })();

    let _ = conn.query_drop("ROLLBACK");
    resultat
}

/// Lit l'etat de la base **sans pouvoir y ecrire**.
///
/// # Erreurs
///
/// Connexion impossible, ou base qui a perdu une table surveillee en cours de
/// route. Une table absente n'est pas comptee zero en silence : elle rend une
/// erreur, parce qu'un audit qui affiche « 0 » sur une table inexistante ferait
/// croire a une base vide plutot qu'a une base cassee.
pub fn lire(pool: &Pool) -> Result<Rapport, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;

    // La garde : la base elle-meme refuse les ecritures pendant toute la duree
    // de la transaction. Aucune ligne de ce module ne peut donc modifier quoi
    // que ce soit, meme par accident.
    conn.query_drop("START TRANSACTION READ ONLY")?;

    let resultat = (|| -> Result<Rapport, Box<dyn Error>> {
        let base: Option<String> = conn.query_first("SELECT DATABASE()")?;
        let tables: Option<i64> = conn.exec_first(
            "SELECT COUNT(*) FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE'",
            (),
        )?;

        // Les societes de travail, pas les tiers : `llx_societe` porte les deux,
        // et confondre les deux donnerait une liste de trois cents lignes ou
        // l'on ne verrait pas quelles entites existent. Une entite est un
        // numero, et son nom vit dans `llx_const`.
        let entites: Vec<(i64, String)> = conn.exec(
            "SELECT c.entity, COALESCE(MAX(CASE WHEN c.name = 'MAIN_INFO_SOCIETE_NOM' \
                                                 THEN c.value END), CONCAT('Entite ', c.entity)) \
             FROM llx_const c WHERE c.entity > 0 GROUP BY c.entity ORDER BY c.entity",
            (),
        )?;

        let utilisateurs: Option<i64> =
            conn.exec_first("SELECT COUNT(*) FROM llx_user WHERE statut = 1", ())?;

        let mut comptes = Vec::with_capacity(SURVEILLEES.len());
        for (table, libelle) in SURVEILLEES {
            // Le nom de table vient d'une constante de ce fichier, pas d'une
            // entree utilisateur : pas d'injection possible. On verifie quand
            // meme qu'il ne contient que des caracteres surs, parce qu'une
            // constante se modifie un jour.
            if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(format!("nom de table refuse : {table}").into());
            }
            let existe: Option<i64> = conn.exec_first(
                "SELECT COUNT(*) FROM information_schema.TABLES \
                 WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = :nom",
                params! { "nom" => format!("llx_{table}") },
            )?;
            if existe.unwrap_or(0) == 0 {
                return Err(format!(
                    "la table llx_{table} ({libelle}) est absente : ce n'est pas une base Dolibarr complète"
                )
                .into());
            }
            let lignes: Option<i64> =
                conn.exec_first(format!("SELECT COUNT(*) FROM llx_{table}").as_str(), ())?;
            comptes.push(Compte {
                table: (*table).to_string(),
                libelle: (*libelle).to_string(),
                lignes: lignes.unwrap_or(0),
            });
        }

        Ok(Rapport {
            base: base.unwrap_or_default(),
            tables: tables.unwrap_or(0),
            tables_attendues: TABLES_ATTENDUES,
            entites,
            comptes,
            utilisateurs: utilisateurs.unwrap_or(0),
        })
    })();

    // On ferme la transaction, sans jamais rien valider : `ROLLBACK` suffit et
    // ne peut rien ecrire.
    let _ = conn.query_drop("ROLLBACK");
    resultat
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rapport_de_test(lignes: &[i64]) -> Rapport {
        Rapport {
            base: "test".to_string(),
            tables: 410,
            tables_attendues: 410,
            entites: vec![(1, "Test".to_string())],
            comptes: lignes
                .iter()
                .enumerate()
                .map(|(i, n)| Compte {
                    table: format!("t{i}"),
                    libelle: format!("Table {i}"),
                    lignes: *n,
                })
                .collect(),
            utilisateurs: 1,
        }
    }

    #[test]
    fn les_tables_vides_sont_signalees() {
        let r = rapport_de_test(&[3, 0, 7, 0]);
        let vides: Vec<&str> = r.vides().iter().map(|c| c.table.as_str()).collect();
        assert_eq!(vides, ["t1", "t3"]);
    }

    #[test]
    fn le_schema_se_juge_au_nombre_de_tables() {
        assert!(rapport_de_test(&[1]).schema_complet());
        let mut incomplet = rapport_de_test(&[1]);
        incomplet.tables = 400;
        assert!(!incomplet.schema_complet());
    }

    #[test]
    fn le_total_additionne_les_objets() {
        assert_eq!(rapport_de_test(&[3, 0, 7, 0]).total_objets(), 10);
        assert_eq!(rapport_de_test(&[]).total_objets(), 0);
    }

    #[test]
    fn la_liste_surveillee_est_sans_doublon() {
        let mut vus = std::collections::BTreeSet::new();
        for (table, libelle) in SURVEILLEES {
            assert!(vus.insert(*table), "table en double : {table}");
            assert!(!libelle.trim().is_empty(), "{table} sans libelle");
        }
    }
}
