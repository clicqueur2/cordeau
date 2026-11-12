//! MRP : ce que la base sait de la demande et du stock, pour le moteur.
//!
//! Le calcul lui-meme vit dans le crate `mrp` (pur, sans base). Ici on lit :
//!
//! - **la demande** : les lignes de commandes clients **validees et non
//!   livrees** (`fk_statut = 1`), moins ce qui est deja parti en expedition ;
//! - **le stock** : la somme des `reel` de `llx_product_stock`, par produit.
//!
//! Ce qu'on ne lit pas encore, et qu'il faut dire plutot que de laisser croire :
//! les **entrees attendues** (commandes fournisseurs en cours, ordres de
//! fabrication) ne sont pas deduites. La table des commandes fournisseurs est
//! vide sur notre base, donc les codes de statut n'ont pas pu etre verifies sur
//! des donnees reelles — et un statut devine fait un besoin faux. Le champ
//! existe dans le moteur (`Article::entrees`) : le brancher sera une requete,
//! pas une refonte.

use std::collections::BTreeMap;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Une ligne de demande : un produit et la quantite reellement a livrer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Demande {
    pub fk_product: i64,
    pub reference: String,
    pub quantite: Decimal,
}

/// La demande a livrer, du plus gros au plus petit.
pub fn demande(pool: &Pool, entity: i64) -> Result<Vec<Demande>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(i64, Option<String>, Option<String>)> = conn.exec(
        "SELECT cd.fk_product, p.ref, \
                CAST(SUM(cd.qty - COALESCE(x.qte, 0)) AS CHAR) \
         FROM llx_commandedet cd \
         JOIN llx_commande c ON c.rowid = cd.fk_commande \
         JOIN llx_product p ON p.rowid = cd.fk_product \
         LEFT JOIN ( \
             SELECT fk_elementdet, SUM(qty) AS qte FROM llx_expeditiondet \
             WHERE element_type = 'commande' GROUP BY fk_elementdet \
         ) x ON x.fk_elementdet = cd.rowid \
         WHERE c.entity = :entity AND c.fk_statut = 1 AND cd.fk_product IS NOT NULL \
         GROUP BY cd.fk_product, p.ref \
         HAVING SUM(cd.qty - COALESCE(x.qte, 0)) > 0 \
         ORDER BY SUM(cd.qty - COALESCE(x.qte, 0)) DESC",
        params! { "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .map(|(fk_product, reference, quantite)| Demande {
            fk_product,
            reference: reference.unwrap_or_default(),
            quantite: Decimal::from_str(quantite.as_deref().unwrap_or("0").trim())
                .unwrap_or_default(),
        })
        .collect())
}

/// Le stock par produit, toutes entrepots confondus.
pub fn stock(pool: &Pool, entity: i64) -> Result<BTreeMap<i64, Decimal>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(i64, Option<String>)> = conn.exec(
        "SELECT ps.fk_product, CAST(SUM(ps.reel) AS CHAR) \
         FROM llx_product_stock ps \
         JOIN llx_product p ON p.rowid = ps.fk_product \
         WHERE p.entity = :entity AND p.tosell = 1 \
         GROUP BY ps.fk_product",
        params! { "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .map(|(id, q)| {
            (
                id,
                Decimal::from_str(q.as_deref().unwrap_or("0").trim()).unwrap_or_default(),
            )
        })
        .collect())
}

/// Les articles a traiter : la demande jointe au stock.
///
/// `lot`, `minimum` et `delai_jours` sont des **hypotheses de calcul** fournies
/// par l'appelant : le schema du coeur ne porte ni lot d'achat ni delai
/// fournisseur sur la fiche produit. On ne les invente pas, on les demande.
pub fn articles(
    pool: &Pool,
    entity: i64,
    lot: Decimal,
    minimum: Decimal,
    delai_jours: i64,
) -> Result<Vec<mrp::Article>, Box<dyn Error>> {
    let stocks = stock(pool, entity)?;
    let demandes = demande(pool, entity)?;

    Ok(demandes
        .into_iter()
        .map(|d| mrp::Article {
            reference: d.reference,
            besoin_brut: d.quantite,
            stock: stocks.get(&d.fk_product).copied().unwrap_or_default(),
            // Non lues : voir l'en-tete du module.
            entrees: Decimal::ZERO,
            lot,
            minimum,
            delai_jours,
        })
        .collect())
}

/// Le total de la demande et du stock, pour l'en-tete de l'ecran.
pub fn totaux(articles: &[mrp::Article]) -> (Decimal, Decimal) {
    (
        articles.iter().map(|a| a.besoin_brut).sum(),
        articles.iter().map(|a| a.stock).sum(),
    )
}

#[cfg(test)]
mod tests_base {
    //! La recette, en test : elle tourne sur la vraie base quand `DOLIBARR_DSN`
    //! est posee, en entite 999, et nettoie derriere elle.

    use super::*;

    // Numero propre a ce module : deux modules qui partagent une
    // entite s'effacent mutuellement leurs lignes.
    const ENTITE: i64 = 996;
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
            "DELETE FROM llx_commandedet WHERE fk_commande IN \
               (SELECT rowid FROM (SELECT rowid FROM llx_commande WHERE entity = :e) x)",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_commande WHERE entity = :e",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_product_stock WHERE fk_product IN \
               (SELECT rowid FROM (SELECT rowid FROM llx_product WHERE entity = :e) y)",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_product WHERE entity = :e",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_entrepot WHERE entity = :e",
            params! { "e" => ENTITE },
        );
    }

    /// Pose un produit, du stock et une commande validee a livrer.
    fn poser(pool: &Pool, reference: &str, stock: &str, commande: &str) {
        let mut conn = pool.get_conn().expect("connexion");
        let soc: i64 = conn
            .query_first("SELECT rowid FROM llx_societe ORDER BY rowid LIMIT 1")
            .expect("requete tiers")
            .expect("aucun tiers en base");

        conn.exec_drop(
            "INSERT INTO llx_product (ref, label, entity, tosell, tobuy, fk_product_type) \
             VALUES (:ref, :label, :e, 1, 1, 0)",
            params! { "ref" => reference, "label" => reference, "e" => ENTITE },
        )
        .expect("produit");
        let produit = conn.last_insert_id() as i64;

        if stock != "0" {
            // L'entrepot doit exister : la colonne porte une cle etrangere.
            // La base de recette n'en a aucun, on en cree un pour l'entite 999.
            let mut entrepot: i64 = conn
                .exec_first(
                    "SELECT rowid FROM llx_entrepot WHERE entity = :e LIMIT 1",
                    params! { "e" => ENTITE },
                )
                .expect("requete entrepot")
                .unwrap_or(0);
            if entrepot == 0 {
                conn.exec_drop(
                    "INSERT INTO llx_entrepot (ref, description, lieu, entity, statut) \
                     VALUES ('RECETTE-MRP', 'Entrepot de recette', 'Recette', :e, 1)",
                    params! { "e" => ENTITE },
                )
                .expect("entrepot");
                entrepot = conn.last_insert_id() as i64;
            }
            conn.exec_drop(
                "INSERT INTO llx_product_stock (fk_product, fk_entrepot, reel) \
                 VALUES (:p, :w, :q)",
                params! { "p" => produit, "w" => entrepot, "q" => stock },
            )
            .expect("stock");
        }

        if commande != "0" {
            conn.exec_drop(
                "INSERT INTO llx_commande (ref, entity, fk_soc, fk_statut, date_commande) \
                 VALUES (:ref, :e, :soc, 1, NOW())",
                params! { "ref" => format!("RECETTE-MRP-{reference}"), "e" => ENTITE, "soc" => soc },
            )
            .expect("commande");
            let cmd = conn.last_insert_id() as i64;
            conn.exec_drop(
                "INSERT INTO llx_commandedet (fk_commande, fk_product, qty, product_type, tva_tx) \
                 VALUES (:c, :p, :q, 0, 20)",
                params! { "c" => cmd, "p" => produit, "q" => commande },
            )
            .expect("ligne de commande");
        }
    }

    /// La demande vient des commandes validees, et le stock la reduit.
    #[test]
    fn le_besoin_net_deduit_le_stock() {
        let _verrou = verrou_entite();
        let Some(p) = pool() else { return };
        nettoyer(&p);

        poser(&p, "RECETTE-A", "30", "100"); // 100 demandes, 30 en stock
        poser(&p, "RECETTE-B", "0", "10"); // 10 demandes, rien en stock
        poser(&p, "RECETTE-C", "50", "0"); // du stock, aucune demande

        let liste = articles(&p, ENTITE, Decimal::ZERO, Decimal::ZERO, 0).expect("articles");

        // Deux articles seulement : celui qui n'a pas de demande n'apparait pas.
        assert_eq!(liste.len(), 2, "{liste:?}");
        let a = liste.iter().find(|a| a.reference == "RECETTE-A").unwrap();
        assert_eq!(a.besoin_brut, Decimal::from(100));
        assert_eq!(a.stock, Decimal::from(30));

        let propositions: Vec<_> = liste
            .iter()
            .map(|a| mrp::proposer(a, "2026-12-31", "2026-11-04"))
            .collect();
        let pa = propositions
            .iter()
            .find(|x| x.reference == "RECETTE-A")
            .unwrap();
        assert_eq!(pa.besoin_net, Decimal::from(70), "100 - 30");

        let pb = propositions
            .iter()
            .find(|x| x.reference == "RECETTE-B")
            .unwrap();
        assert_eq!(pb.besoin_net, Decimal::from(10));

        // Avec un lot de 25, on ne commande pas 70 mais 75.
        let avec_lot = articles(&p, ENTITE, Decimal::from(25), Decimal::ZERO, 7).expect("articles");
        let pa2 = avec_lot
            .iter()
            .find(|a| a.reference == "RECETTE-A")
            .unwrap();
        let prop = mrp::proposer(pa2, "2026-12-31", "2026-11-04");
        assert_eq!(prop.quantite, Decimal::from(75));

        nettoyer(&p);
    }

    /// Une commande en brouillon ne cree aucune demande.
    #[test]
    fn un_brouillon_ne_demande_rien() {
        let _verrou = verrou_entite();
        let Some(p) = pool() else { return };
        nettoyer(&p);
        poser(&p, "RECETTE-D", "0", "0");

        let mut conn = p.get_conn().expect("connexion");
        let soc: i64 = conn
            .query_first("SELECT rowid FROM llx_societe ORDER BY rowid LIMIT 1")
            .unwrap()
            .unwrap();
        let produit: i64 = conn
            .exec_first(
                "SELECT rowid FROM llx_product WHERE entity = :e AND ref = 'RECETTE-D'",
                params! { "e" => ENTITE },
            )
            .unwrap()
            .unwrap();
        conn.exec_drop(
            "INSERT INTO llx_commande (ref, entity, fk_soc, fk_statut, date_commande) \
             VALUES ('RECETTE-MRP-B', :e, :soc, 0, NOW())",
            params! { "e" => ENTITE, "soc" => soc },
        )
        .unwrap();
        let cmd = conn.last_insert_id() as i64;
        conn.exec_drop(
            "INSERT INTO llx_commandedet (fk_commande, fk_product, qty, product_type, tva_tx) \
             VALUES (:c, :p, 5, 0, 20)",
            params! { "c" => cmd, "p" => produit },
        )
        .unwrap();

        assert!(
            demande(&p, ENTITE).unwrap().is_empty(),
            "un brouillon ne doit pas entrer dans la demande"
        );

        nettoyer(&p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_totaux_additionnent_demande_et_stock() {
        let a = vec![
            mrp::Article {
                reference: "A".into(),
                besoin_brut: Decimal::from(10),
                stock: Decimal::from(4),
                entrees: Decimal::ZERO,
                lot: Decimal::ZERO,
                minimum: Decimal::ZERO,
                delai_jours: 0,
            },
            mrp::Article {
                reference: "B".into(),
                besoin_brut: Decimal::from(5),
                stock: Decimal::from(1),
                entrees: Decimal::ZERO,
                lot: Decimal::ZERO,
                minimum: Decimal::ZERO,
                delai_jours: 0,
            },
        ];
        assert_eq!(totaux(&a), (Decimal::from(15), Decimal::from(5)));
    }

    #[test]
    fn un_stock_vide_ne_donne_aucun_total() {
        assert_eq!(totaux(&[]), (Decimal::ZERO, Decimal::ZERO));
    }
}
