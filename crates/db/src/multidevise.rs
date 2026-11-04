//! Multidevises : figer un taux sur la piece.
//!
//! Dolibarr n'a **pas** de table de taux. `llx_multicurrency` ne retient que les
//! devises employees (`code`, `name`, `entity`) ; le taux vit sur la piece :
//! `multicurrency_code`, `multicurrency_tx`, et les trois totaux en devise.
//!
//! C'est la bonne facon de faire, et il faut le dire : un taux stocke dans une
//! table globale change avec le marche, donc une facture de mars 2026 vaudrait
//! autre chose en juin. **Un taux fige sur la piece ne bouge plus** — c'est ce
//! que la comptabilite exige.
//!
//! La convention de sens vient du cceur, verifiee dans `price.lib.php` :
//!
//! ```text
//! pu_devise = pu * multicurrency_tx
//! ```
//!
//! Autrement dit `tx` est le nombre d'unites de devise pour **une** unite de la
//! monnaie de la societe. EUR 100 a 1,08 CHF donnent 108,00 CHF.
//!
//! Les montants de base (`total_ht`, `total_ttc`, le grand livre) restent dans
//! la monnaie de la societe : la devise est une **traduction**, pas un second
//! jeu d'ecritures.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Un document qui peut porter une devise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Document {
    Propal,
    Commande,
    Facture,
}

impl Document {
    /// Le nom de l'objet dans l'URL et sur l'ecran.
    pub fn nom(self) -> &'static str {
        match self {
            Document::Propal => "devis",
            Document::Commande => "commande",
            Document::Facture => "facture",
        }
    }

    /// Le nom Dolibarr de l'objet, tel qu'il apparait dans les journaux.
    pub fn element(self) -> &'static str {
        match self {
            Document::Propal => "propal",
            Document::Commande => "commande",
            Document::Facture => "facture",
        }
    }

    pub fn depuis_nom(nom: &str) -> Option<Document> {
        match nom.trim().to_ascii_lowercase().as_str() {
            "devis" | "propal" => Some(Document::Propal),
            "commande" => Some(Document::Commande),
            "facture" => Some(Document::Facture),
            _ => None,
        }
    }

    /// La table d'en-tete du document.
    fn table(self) -> &'static str {
        match self {
            Document::Propal => "llx_propal",
            Document::Commande => "llx_commande",
            Document::Facture => "llx_facture",
        }
    }

    /// La table des lignes, et la colonne qui les rattache.
    fn lignes(self) -> (&'static str, &'static str) {
        match self {
            Document::Propal => ("llx_propaldet", "fk_propal"),
            Document::Commande => ("llx_commandedet", "fk_commande"),
            Document::Facture => ("llx_facturedet", "fk_facture"),
        }
    }
}

/// Une devise du dictionnaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Devise {
    pub code: String,
    pub libelle: String,
    /// Nombre de decimales avec lesquelles la devise se presente.
    pub decimales: u32,
}

/// Une ligne brute d'en-tete : `(code, tx, ht, tva, ttc)`.
type LigneTaux = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);
/// Une piece en devise : `(rowid, reference, taux)`.
pub type PieceEnDevise = (i64, String, TauxFige);

/// Le taux fige sur une piece, et ce qu'il produit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TauxFige {
    pub code: String,
    pub tx: Decimal,
    pub total_ht: Decimal,
    pub total_tva: Decimal,
    pub total_ttc: Decimal,
}

/// Les devises actives du dictionnaire (`llx_c_currencies`).
pub fn devises(pool: &Pool) -> Result<Vec<Devise>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(String, Option<String>, Option<i32>)> = conn.exec(
        "SELECT code_iso, label, max_decimal_unit FROM llx_c_currencies \
         WHERE active = 1 ORDER BY code_iso ASC",
        (),
    )?;
    Ok(rows
        .into_iter()
        .map(|(code, libelle, decimales)| Devise {
            code: code.trim().to_string(),
            libelle: libelle.unwrap_or_default(),
            // Le dictionnaire laisse la colonne vide : deux decimales est la
            // regle des devises qu'on rencontre ici.
            decimales: decimales.unwrap_or(2).clamp(0, 4) as u32,
        })
        .collect())
}

/// Les decimales d'une devise, ou 2 si elle n'est pas au dictionnaire.
pub fn decimales(pool: &Pool, code: &str) -> Result<u32, Box<dyn Error>> {
    Ok(devises(pool)?
        .into_iter()
        .find(|d| d.code.eq_ignore_ascii_case(code))
        .map(|d| d.decimales)
        .unwrap_or(2))
}

/// Convertit un montant de la monnaie de la societe vers la devise.
///
/// `tx` est le nombre d'unites de devise pour une unite de base — c'est le sens
/// du cceur, et l'inverser doublerait ou diviserait tout.
pub fn convertir(base: Decimal, tx: Decimal, decimales: u32) -> Decimal {
    (base * tx).round_dp(decimales)
}

/// Le taux deja fige sur une piece, s'il y en a un.
pub fn taux_fige(
    pool: &Pool,
    entity: i64,
    doc: Document,
    id: i64,
) -> Result<Option<TauxFige>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT multicurrency_code, CAST(multicurrency_tx AS CHAR), \
                CAST(multicurrency_total_ht AS CHAR), CAST(multicurrency_total_tva AS CHAR), \
                CAST(multicurrency_total_ttc AS CHAR) \
         FROM {} WHERE rowid = :id AND entity = :entity",
        doc.table()
    );
    let row: Option<LigneTaux> =
        conn.exec_first(sql, params! { "id" => id, "entity" => entity })?;

    let Some((code, tx, ht, tva, ttc)) = row else {
        return Ok(None);
    };
    let code = code.unwrap_or_default().trim().to_string();
    if code.is_empty() {
        return Ok(None);
    }
    Ok(Some(TauxFige {
        code,
        tx: dec(tx),
        total_ht: dec(ht),
        total_tva: dec(tva),
        total_ttc: dec(ttc),
    }))
}

/// Fige un taux sur une piece et sur ses lignes.
///
/// Refuse un taux nul ou negatif, une devise inconnue, un document introuvable.
/// Refuse aussi de **changer** un taux deja fige : une piece comptable ne se
/// re-evalue pas apres coup. `refiger` le permet explicitement, et seulement
/// sur un brouillon.
pub fn figer(
    pool: &Pool,
    entity: i64,
    doc: Document,
    id: i64,
    code: &str,
    tx: Decimal,
    refiger: bool,
) -> Result<TauxFige, Box<dyn Error>> {
    let code = code.trim().to_ascii_uppercase();
    if tx <= Decimal::ZERO {
        return Err("le taux doit etre strictement positif".into());
    }
    let decimales = decimales(pool, &code)?;
    let conn_devise: Option<String> = {
        let mut c = pool.get_conn()?;
        c.exec_first(
            "SELECT code_iso FROM llx_c_currencies WHERE code_iso = :code AND active = 1",
            params! { "code" => code.as_str() },
        )?
    };
    if conn_devise.is_none() {
        return Err(format!("devise inconnue ou inactive : {code}").into());
    }

    let mut conn = pool.get_conn()?;

    // Les totaux de base, et le statut : un brouillon est le seul document
    // qu'on puisse encore reevaluer. La colonne s'appelle `fk_statut` sur les
    // trois objets.
    let sql_doc = format!(
        "SELECT CAST(total_ht AS CHAR), CAST(total_tva AS CHAR), CAST(total_ttc AS CHAR), \
                fk_statut, multicurrency_code \
         FROM {} WHERE rowid = :id AND entity = :entity",
        doc.table()
    );
    let Some((ht, tva, ttc, statut, deja)) = conn
        .exec_first::<(String, String, String, i32, Option<String>), _, _>(
            sql_doc,
            params! { "id" => id, "entity" => entity },
        )?
    else {
        return Err(format!("{} {id} introuvable", doc.nom()).into());
    };

    let deja_code = deja.unwrap_or_default().trim().to_string();
    if !deja_code.is_empty() && !refiger {
        return Err(format!(
            "un taux est deja fige sur cette piece ({deja_code}) : une piece comptable ne se re-evalue pas"
        )
        .into());
    }
    if refiger && statut > 0 {
        return Err("un document valide ne se re-evalue pas, meme en forcant".into());
    }

    let base_ht = dec(Some(ht));
    let base_tva = dec(Some(tva));
    let base_ttc = dec(Some(ttc));

    let fige = TauxFige {
        code: code.clone(),
        tx,
        total_ht: convertir(base_ht, tx, decimales),
        total_tva: convertir(base_tva, tx, decimales),
        total_ttc: convertir(base_ttc, tx, decimales),
    };

    // 1. La devise entre dans la liste des devises employees par l'entite.
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_multicurrency WHERE code = :code AND entity = :entity",
        params! { "code" => code.as_str(), "entity" => entity },
    )?;
    if existe.is_none() {
        conn.exec_drop(
            "INSERT INTO llx_multicurrency (date_create, code, name, entity) \
             VALUES (NOW(), :code, :nom, :entity)",
            params! { "code" => code.as_str(), "nom" => code.as_str(), "entity" => entity },
        )?;
    }

    // 2. L'en-tete.
    let sql = format!(
        "UPDATE {} SET multicurrency_code = :code, multicurrency_tx = :tx, \
                multicurrency_total_ht = :ht, multicurrency_total_tva = :tva, \
                multicurrency_total_ttc = :ttc \
         WHERE rowid = :id AND entity = :entity",
        doc.table()
    );
    conn.exec_drop(
        sql,
        params! {
            "code" => code.as_str(),
            "tx" => tx.to_string(),
            "ht" => fige.total_ht.to_string(),
            "tva" => fige.total_tva.to_string(),
            "ttc" => fige.total_ttc.to_string(),
            "id" => id,
            "entity" => entity,
        },
    )?;

    // 3. Les lignes : prix unitaire, totaux, tout en devise.
    let (table_lignes, fk) = doc.lignes();
    let sql = format!(
        "UPDATE {table_lignes} SET multicurrency_code = :code, \
                multicurrency_subprice = ROUND(subprice * :tx, {decimales}), \
                multicurrency_subprice_ttc = ROUND(subprice_ttc * :tx, {decimales}), \
                multicurrency_total_ht = ROUND(total_ht * :tx, {decimales}), \
                multicurrency_total_tva = ROUND(total_tva * :tx, {decimales}), \
                multicurrency_total_ttc = ROUND(total_ttc * :tx, {decimales}) \
         WHERE {fk} = :id"
    );
    conn.exec_drop(
        sql,
        params! { "code" => code.as_str(), "tx" => tx.to_string(), "id" => id },
    )?;

    Ok(fige)
}

/// Les pieces qui portent un taux fige, du plus recent au plus ancien.
pub fn portant_une_devise(
    pool: &Pool,
    entity: i64,
    doc: Document,
    limite: u32,
) -> Result<Vec<PieceEnDevise>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT rowid, ref, multicurrency_code, CAST(multicurrency_tx AS CHAR), \
                CAST(multicurrency_total_ht AS CHAR), CAST(multicurrency_total_tva AS CHAR), \
                CAST(multicurrency_total_ttc AS CHAR) \
         FROM {} \
         WHERE entity = :entity AND multicurrency_code IS NOT NULL \
           AND multicurrency_code <> '' \
         ORDER BY rowid DESC LIMIT :limite",
        doc.table()
    );
    let rows: Vec<mysql::Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limite" => limite.clamp(1, 200) },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let t = |i: usize| r.get_opt::<String, _>(i).and_then(|v| v.ok());
            (
                r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
                t(1).unwrap_or_default(),
                TauxFige {
                    code: t(2).unwrap_or_default(),
                    tx: dec(t(3)),
                    total_ht: dec(t(4)),
                    total_tva: dec(t(5)),
                    total_ttc: dec(t(6)),
                },
            )
        })
        .collect())
}

fn dec(v: Option<String>) -> Decimal {
    v.as_deref()
        .map(str::trim)
        .and_then(|s| Decimal::from_str(s).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests_base {
    //! La recette, en test : elle tourne sur la vraie base quand `DOLIBARR_DSN`
    //! est posee. Elle travaille en entite 999 et nettoie derriere elle.

    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    const ENTITE: i64 = 999;

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
            "DELETE FROM llx_facturedet WHERE fk_facture IN \
               (SELECT rowid FROM (SELECT rowid FROM llx_facture WHERE entity = :e) x)",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_facture WHERE entity = :e",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_multicurrency WHERE entity = :e",
            params! { "e" => ENTITE },
        );
    }

    /// Pose une facture brouillon avec une ligne, et rend son numero.
    fn poser_facture(pool: &Pool) -> i64 {
        let mut conn = pool.get_conn().expect("connexion");
        // Les cles etrangeres vers le tiers et l'utilisateur sont globales : on
        // prend des lignes qui existent deja.
        let soc: i64 = conn
            .query_first("SELECT rowid FROM llx_societe ORDER BY rowid LIMIT 1")
            .expect("requete tiers")
            .expect("aucun tiers en base : test impossible");
        let usr: i64 = conn
            .query_first("SELECT rowid FROM llx_user ORDER BY rowid LIMIT 1")
            .expect("requete utilisateur")
            .expect("aucun utilisateur en base : test impossible");

        conn.exec_drop(
            "INSERT INTO llx_facture \
               (ref, entity, fk_soc, fk_user_author, datec, datef, total_ht, total_tva, \
                total_ttc, fk_statut, paye) \
             VALUES ('RECETTE-MD-1', :e, :soc, :usr, NOW(), NOW(), 100, 20, 120, 0, 0)",
            params! { "e" => ENTITE, "soc" => soc, "usr" => usr },
        )
        .expect("insertion de la facture");
        let id = conn.last_insert_id() as i64;

        conn.exec_drop(
            "INSERT INTO llx_facturedet \
               (fk_facture, product_type, description, subprice, subprice_ttc, tva_tx, \
                total_ht, total_tva, total_ttc, fk_product) \
             VALUES (:id, 9, 'Ligne de recette', 100, 120, 20, 100, 20, 120, NULL)",
            params! { "id" => id },
        )
        .expect("insertion de la ligne");
        id
    }

    #[test]
    fn un_taux_se_fige_sur_la_piece_et_sur_ses_lignes() {
        let Some(p) = pool() else { return };
        nettoyer(&p);
        let id = poser_facture(&p);

        // Rien de fige au depart.
        assert_eq!(taux_fige(&p, ENTITE, Document::Facture, id).unwrap(), None);

        let fige = figer(&p, ENTITE, Document::Facture, id, "CHF", d("1.08"), false)
            .expect("figer le taux");

        // 100 * 1,08 = 108 ; 20 * 1,08 = 21,60 ; 120 * 1,08 = 129,60.
        assert_eq!(fige.code, "CHF");
        assert_eq!(fige.total_ht, d("108.00"));
        assert_eq!(fige.total_tva, d("21.60"));
        assert_eq!(fige.total_ttc, d("129.60"));

        // Relu depuis la base, pas depuis la memoire.
        let relu = taux_fige(&p, ENTITE, Document::Facture, id)
            .unwrap()
            .expect("un taux doit etre fige");
        assert_eq!(relu.total_ttc, d("129.60"));

        // La ligne porte le prix unitaire et les totaux en devise.
        let mut conn = p.get_conn().expect("connexion");
        let ligne: (String, String, String) = conn
            .exec_first(
                "SELECT CAST(multicurrency_subprice AS CHAR), \
                        CAST(multicurrency_total_ht AS CHAR), \
                        CAST(multicurrency_total_ttc AS CHAR) \
                 FROM llx_facturedet WHERE fk_facture = :id",
                params! { "id" => id },
            )
            .unwrap()
            .expect("une ligne");
        assert_eq!(dec(Some(ligne.0)), d("108.00"));
        assert_eq!(dec(Some(ligne.1)), d("108.00"));
        assert_eq!(dec(Some(ligne.2)), d("129.60"));

        // Les montants de base n'ont pas bouge : la devise est une traduction.
        let base: (String, String) = conn
            .exec_first(
                "SELECT CAST(total_ht AS CHAR), CAST(total_ttc AS CHAR) \
                 FROM llx_facture WHERE rowid = :id",
                params! { "id" => id },
            )
            .unwrap()
            .expect("la facture");
        assert_eq!(dec(Some(base.0)), d("100"));
        assert_eq!(dec(Some(base.1)), d("120"));

        // La devise est entree dans la liste des devises employees.
        let n: Option<i64> = conn
            .exec_first(
                "SELECT COUNT(*) FROM llx_multicurrency WHERE entity = :e AND code = 'CHF'",
                params! { "e" => ENTITE },
            )
            .unwrap();
        assert_eq!(n, Some(1));

        // Figer deux fois : refuse.
        assert!(figer(&p, ENTITE, Document::Facture, id, "USD", d("1.1"), false).is_err());

        nettoyer(&p);
    }

    #[test]
    fn un_document_valide_ne_se_revalue_pas() {
        let Some(p) = pool() else { return };
        nettoyer(&p);
        let id = poser_facture(&p);

        let mut conn = p.get_conn().expect("connexion");
        conn.exec_drop(
            "UPDATE llx_facture SET fk_statut = 1 WHERE rowid = :id",
            params! { "id" => id },
        )
        .unwrap();

        // Meme en forcant : une piece validee est arretee.
        let e = figer(&p, ENTITE, Document::Facture, id, "CHF", d("1.08"), true).unwrap_err();
        assert!(e.to_string().contains("valide"), "{e}");

        nettoyer(&p);
    }

    #[test]
    fn un_taux_nul_ou_une_devise_inconnue_sont_refuses() {
        let Some(p) = pool() else { return };
        nettoyer(&p);
        let id = poser_facture(&p);

        assert!(figer(&p, ENTITE, Document::Facture, id, "CHF", d("0"), false).is_err());
        assert!(figer(&p, ENTITE, Document::Facture, id, "CHF", d("-1"), false).is_err());
        assert!(figer(&p, ENTITE, Document::Facture, id, "XXX", d("1"), false).is_err());
        assert!(figer(&p, ENTITE, Document::Facture, 999_999, "CHF", d("1"), false).is_err());

        // Aucun de ces refus n'a laisse de trace.
        assert_eq!(taux_fige(&p, ENTITE, Document::Facture, id).unwrap(), None);

        nettoyer(&p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    /// Le sens du taux : `tx` multiplie. EUR 100 a 1,08 = 108,00 CHF.
    #[test]
    fn le_taux_multiplie_le_montant_de_base() {
        assert_eq!(convertir(d("100"), d("1.08"), 2), d("108.00"));
        assert_eq!(convertir(d("1200"), d("1.08"), 2), d("1296.00"));
    }

    /// Le sens inverse : 1 EUR vaut 0,92 USD, 100 EUR valent 92 USD.
    #[test]
    fn un_taux_inferieur_a_un_diminue() {
        assert_eq!(convertir(d("100"), d("0.92"), 2), d("92.00"));
    }

    #[test]
    fn le_resultat_est_arrondi_aux_decimales_de_la_devise() {
        // 33,33 * 1,234 = 41,13322 -> 41,13 en deux decimales.
        assert_eq!(convertir(d("33.33"), d("1.234"), 2), d("41.13"));
        // Un yen n'a pas de decimale : 41 pour la meme conversion.
        assert_eq!(convertir(d("33.33"), d("1.234"), 0), d("41"));
    }

    #[test]
    fn les_documents_se_relisent_depuis_le_nom() {
        assert_eq!(Document::depuis_nom("devis"), Some(Document::Propal));
        assert_eq!(Document::depuis_nom("propal"), Some(Document::Propal));
        assert_eq!(Document::depuis_nom(" facture "), Some(Document::Facture));
        assert_eq!(Document::depuis_nom("commande"), Some(Document::Commande));
        assert_eq!(Document::depuis_nom("avoir"), None);
    }

    #[test]
    fn chaque_document_a_sa_table_et_ses_lignes() {
        assert_eq!(Document::Facture.table(), "llx_facture");
        assert_eq!(Document::Facture.lignes(), ("llx_facturedet", "fk_facture"));
        assert_eq!(Document::Propal.lignes(), ("llx_propaldet", "fk_propal"));
        assert_eq!(
            Document::Commande.lignes(),
            ("llx_commandedet", "fk_commande")
        );
    }
}
