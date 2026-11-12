//! Comptabilite analytique : la marge par chantier.
//!
//! Dolibarr a ajoute les axes analytiques au schema en **23.0**, et ils sont
//! donc dans nos 410 tables : pas besoin d'inventer quoi que ce soit.
//!
//! - `llx_accounting_analytic_axis` : les axes (« Chantiers », « Agences »).
//! - `llx_accounting_analytic_account` : les comptes d'un axe. **Le compte
//!   analytique est le chantier** : son code porte la reference, son libelle le
//!   nom.
//! - `llx_accounting_analytic_distribution` : la part d'une ligne d'ecriture
//!   affectee a un compte analytique. Une ligne peut se partager sur plusieurs,
//!   la somme des pourcentages doit valoir 100.
//!
//! Une decision de signe, ecrite une fois : `amount` est la **contribution au
//! resultat**, donc `credit - debit`. Un produit est positif, une charge
//! negative, et la marge d'un chantier est la somme, sans avoir a se rappeler
//! dans quel sens lire les colonnes.
//!
//! La colonne `sourcetype` est une chaine libre : elle dit d'ou vient la ligne
//! repartie. Nous employons `bookkeeping` pour les lignes de
//! `llx_accounting_bookkeeping`, et nous ne lisons que ce que nous avons ecrit.
//! Un module tiers peut employer un autre mot sans qu'on l'interprete de
//! travers.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Le type de source que nous employons pour les lignes du livre.
pub const SOURCE: &str = "bookkeeping";

/// Une ligne brute d'axe : `(rowid, code, libelle, actif)`.
type LigneAxe = (i64, Option<String>, Option<String>, Option<i32>);
/// Une ligne brute de compte : `(rowid, code, libelle, axe, actif)`.
type LigneCompte = (
    i64,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i32>,
);
/// Une ligne brute d'affectation : `(compte, code, libelle, pourcentage, montant)`.
type LigneRepartition = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);
/// Une ligne brute de marge : `(code, libelle, montant, lignes)`.
type LigneMarge = (String, Option<String>, Option<String>, Option<i64>);

/// Un axe analytique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Axe {
    pub rowid: i64,
    pub code: String,
    pub libelle: String,
    pub actif: bool,
}

/// Un compte analytique — pour nous, un chantier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compte {
    pub rowid: i64,
    pub code: String,
    pub libelle: String,
    pub fk_axe: i64,
    pub actif: bool,
}

/// La part d'une ligne d'ecriture affectee a un compte analytique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repartition {
    pub fk_compte: i64,
    pub compte: String,
    pub libelle: String,
    /// Pourcentage affecte.
    pub pourcentage: Decimal,
    /// Contribution au resultat, positive en produit.
    pub montant: Decimal,
}

/// La marge d'un compte analytique sur une periode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marge {
    pub code: String,
    pub libelle: String,
    /// Produits moins charges, sur les lignes affectees.
    pub montant: Decimal,
    /// Nombre de lignes d'ecriture affectees.
    pub lignes: i64,
}

/// Les axes d'une entite.
pub fn axes(pool: &Pool, entity: i64) -> Result<Vec<Axe>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<LigneAxe> = conn.exec(
        "SELECT rowid, code, label, active FROM llx_accounting_analytic_axis \
         WHERE entity = :entity ORDER BY code ASC",
        params! { "entity" => entity },
    )?;
    Ok(rows
        .into_iter()
        .map(|(rowid, code, libelle, actif)| Axe {
            rowid,
            code: code.unwrap_or_default(),
            libelle: libelle.unwrap_or_default(),
            actif: actif.unwrap_or(0) != 0,
        })
        .collect())
}

/// Les comptes analytiques, tous axes confondus ou un seul.
pub fn comptes(pool: &Pool, entity: i64, axe: Option<i64>) -> Result<Vec<Compte>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = "SELECT rowid, code, label, fk_axis, active \
               FROM llx_accounting_analytic_account \
               WHERE entity = :entity AND (:axe IS NULL OR fk_axis = :axe) \
               ORDER BY code ASC";
    let rows: Vec<LigneCompte> = conn.exec(sql, params! { "entity" => entity, "axe" => axe })?;
    Ok(rows
        .into_iter()
        .map(|(rowid, code, libelle, fk_axe, actif)| Compte {
            rowid,
            code: code.unwrap_or_default(),
            libelle: libelle.unwrap_or_default(),
            fk_axe: fk_axe.unwrap_or(0),
            actif: actif.unwrap_or(0) != 0,
        })
        .collect())
}

/// Cree un axe, ou rend celui qui porte deja ce code.
pub fn creer_axe(
    pool: &Pool,
    entity: i64,
    code: &str,
    libelle: &str,
) -> Result<i64, Box<dyn Error>> {
    let code = code.trim();
    if code.is_empty() || libelle.trim().is_empty() {
        return Err("code et libelle sont obligatoires".into());
    }
    let mut conn = pool.get_conn()?;
    if let Some(id) = conn.exec_first::<i64, _, _>(
        "SELECT rowid FROM llx_accounting_analytic_axis WHERE entity = :entity AND code = :code",
        params! { "entity" => entity, "code" => code },
    )? {
        return Ok(id);
    }
    conn.exec_drop(
        // k_user_author est NOT NULL sans defaut : on met 0, comme le fait
        // deja l'ecriture comptable de ce noyau. Ce n'est pas une cle etrangere,
        // c'est un champ libre ou 0 veut dire « inconnu ».
        "INSERT INTO llx_accounting_analytic_axis \
           (label, code, active, entity, datec, fk_user_author) \
         VALUES (:label, :code, 1, :entity, NOW(), 0)",
        params! { "label" => libelle.trim(), "code" => code, "entity" => entity },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Cree un compte analytique (un chantier), ou rend celui qui porte deja ce code.
pub fn creer_compte(
    pool: &Pool,
    entity: i64,
    fk_axe: i64,
    code: &str,
    libelle: &str,
) -> Result<i64, Box<dyn Error>> {
    let code = code.trim();
    if code.is_empty() || libelle.trim().is_empty() {
        return Err("code et libelle sont obligatoires".into());
    }
    let mut conn = pool.get_conn()?;
    if let Some(id) = conn.exec_first::<i64, _, _>(
        "SELECT rowid FROM llx_accounting_analytic_account \
         WHERE entity = :entity AND code = :code",
        params! { "entity" => entity, "code" => code },
    )? {
        return Ok(id);
    }
    conn.exec_drop(
        "INSERT INTO llx_accounting_analytic_account \
           (label, code, fk_axis, active, entity, datec, fk_user_author) \
         VALUES (:label, :code, :axe, 1, :entity, NOW(), 0)",
        params! {
            "label" => libelle.trim(),
            "code" => code,
            "axe" => fk_axe,
            "entity" => entity,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// La contribution d'une ligne au resultat : `credit - debit`.
///
/// Un produit est positif, une charge negative. C'est la convention qui rend la
/// marge lisible : la marge d'un chantier est la somme des contributions, sans
/// avoir a se rappeler dans quel sens lire les colonnes.
pub fn contribution(debit: Decimal, credit: Decimal) -> Decimal {
    credit - debit
}

/// Affecte une ligne d'ecriture a des comptes analytiques.
///
/// Remplace les affectations precedentes de cette ligne : rejouer une
/// repartition ne la double pas. Refuse une somme de pourcentages differente de
/// 100, un compte inconnu, une ligne qui n'appartient pas a l'entite.
pub fn repartir(
    pool: &Pool,
    entity: i64,
    fk_source_line: i64,
    parts: &[(i64, Decimal)],
) -> Result<usize, Box<dyn Error>> {
    if let Err(e) = verifier_parts(parts) {
        return Err(e.into());
    }

    let mut conn = pool.get_conn()?;
    let debit_credit: Option<(String, String)> = conn.exec_first(
        "SELECT CAST(debit AS CHAR), CAST(credit AS CHAR) \
         FROM llx_accounting_bookkeeping WHERE rowid = :id AND entity = :entity",
        params! { "id" => fk_source_line, "entity" => entity },
    )?;
    let Some((debit, credit)) = debit_credit else {
        return Err(format!("ligne d'ecriture {fk_source_line} introuvable").into());
    };
    let base = contribution(
        Decimal::from_str(debit.trim()).unwrap_or_default(),
        Decimal::from_str(credit.trim()).unwrap_or_default(),
    );

    // Tous les comptes doivent exister sur cette entite.
    for (fk, _) in parts {
        let existe: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_accounting_analytic_account \
             WHERE rowid = :id AND entity = :entity",
            params! { "id" => fk, "entity" => entity },
        )?;
        if existe.is_none() {
            return Err(format!("compte analytique {fk} introuvable").into());
        }
    }

    let mut tx = conn.start_transaction(mysql::TxOpts::default())?;
    tx.exec_drop(
        "DELETE FROM llx_accounting_analytic_distribution \
         WHERE entity = :entity AND sourcetype = :source AND fk_source_line = :ligne",
        params! { "entity" => entity, "source" => SOURCE, "ligne" => fk_source_line },
    )?;
    for (fk, pourcentage) in parts {
        let montant = (base * *pourcentage / Decimal::from(100)).round_dp(8);
        tx.exec_drop(
            "INSERT INTO llx_accounting_analytic_distribution \
               (fk_source_line, sourcetype, fk_analytic_account, percentage, amount, entity, \
                datec, fk_user_author) \
             VALUES (:ligne, :source, :compte, :pct, :montant, :entity, NOW(), 0)",
            params! {
                "ligne" => fk_source_line,
                "source" => SOURCE,
                "compte" => fk,
                "pct" => pourcentage.to_string(),
                "montant" => montant.to_string(),
                "entity" => entity,
            },
        )?;
    }
    tx.commit()?;
    Ok(parts.len())
}

/// Les affectations d'une ligne d'ecriture.
pub fn repartitions(
    pool: &Pool,
    entity: i64,
    fk_source_line: i64,
) -> Result<Vec<Repartition>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<LigneRepartition> = conn.exec(
        "SELECT d.fk_analytic_account, a.code, a.label, \
                CAST(d.percentage AS CHAR), CAST(d.amount AS CHAR) \
         FROM llx_accounting_analytic_distribution d \
         JOIN llx_accounting_analytic_account a ON a.rowid = d.fk_analytic_account \
         WHERE d.entity = :entity AND d.sourcetype = :source AND d.fk_source_line = :ligne \
         ORDER BY a.code ASC",
        params! { "entity" => entity, "source" => SOURCE, "ligne" => fk_source_line },
    )?;
    Ok(rows
        .into_iter()
        .map(|(fk_compte, compte, libelle, pct, montant)| Repartition {
            fk_compte,
            compte: compte.unwrap_or_default(),
            libelle: libelle.unwrap_or_default(),
            pourcentage: Decimal::from_str(pct.as_deref().unwrap_or("0").trim())
                .unwrap_or_default(),
            montant: Decimal::from_str(montant.as_deref().unwrap_or("0").trim())
                .unwrap_or_default(),
        })
        .collect())
}

/// La marge par compte analytique sur une periode, du plus gros au plus petit.
pub fn marge(
    pool: &Pool,
    entity: i64,
    debut: &str,
    fin: &str,
) -> Result<Vec<Marge>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<LigneMarge> = conn.exec(
        "SELECT a.code, a.label, CAST(COALESCE(SUM(d.amount), 0) AS CHAR), COUNT(*) \
         FROM llx_accounting_analytic_distribution d \
         JOIN llx_accounting_analytic_account a ON a.rowid = d.fk_analytic_account \
         JOIN llx_accounting_bookkeeping b ON b.rowid = d.fk_source_line \
         WHERE d.entity = :entity AND d.sourcetype = :source \
           AND b.doc_date BETWEEN :debut AND :fin \
         GROUP BY a.rowid, a.code, a.label \
         ORDER BY SUM(d.amount) DESC",
        params! { "entity" => entity, "source" => SOURCE, "debut" => debut, "fin" => fin },
    )?;
    Ok(rows
        .into_iter()
        .map(|(code, libelle, montant, lignes)| Marge {
            code,
            libelle: libelle.unwrap_or_default(),
            montant: Decimal::from_str(montant.as_deref().unwrap_or("0").trim())
                .unwrap_or_default(),
            lignes: lignes.unwrap_or(0),
        })
        .collect())
}

/// Le nombre de lignes du livre qui ne sont affectees a aucun compte.
///
/// C'est le chiffre qui dit si la marge veut dire quelque chose : une marge
/// calculee sur 10 % des lignes n'est pas une marge, c'est un echantillon.
pub fn non_reparties(
    pool: &Pool,
    entity: i64,
    debut: &str,
    fin: &str,
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_accounting_bookkeeping b \
         WHERE b.entity = :entity AND b.doc_date BETWEEN :debut AND :fin \
           AND NOT EXISTS ( \
             SELECT 1 FROM llx_accounting_analytic_distribution d \
             WHERE d.fk_source_line = b.rowid AND d.entity = b.entity AND d.sourcetype = :source)",
        params! { "entity" => entity, "debut" => debut, "fin" => fin, "source" => SOURCE },
    )?;
    Ok(n.unwrap_or(0))
}

/// Le total des lignes du livre sur la periode, pour calculer le taux affecte.
pub fn lignes_periode(
    pool: &Pool,
    entity: i64,
    debut: &str,
    fin: &str,
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_accounting_bookkeeping \
         WHERE entity = :entity AND doc_date BETWEEN :debut AND :fin",
        params! { "entity" => entity, "debut" => debut, "fin" => fin },
    )?;
    Ok(n.unwrap_or(0))
}

/// Verifie qu'une repartition est coherente, sans toucher a la base.
///
/// Pure, donc verifiable : c'est la regle qui refuse une affectation partielle.
pub fn verifier_parts(parts: &[(i64, Decimal)]) -> Result<(), String> {
    if parts.is_empty() {
        return Err("aucune part a affecter".into());
    }
    if parts.iter().any(|(_, p)| *p <= Decimal::ZERO) {
        return Err("une part doit etre strictement positive".into());
    }
    let mut vus = std::collections::BTreeSet::new();
    for (fk, _) in parts {
        if !vus.insert(*fk) {
            return Err(format!("le compte {fk} apparait deux fois"));
        }
    }
    let total: Decimal = parts.iter().map(|(_, p)| *p).sum();
    if total != Decimal::from(100) {
        return Err(format!("les pourcentages doivent valoir 100, pas {total}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests_base {
    //! La recette, en test : elle tourne sur la vraie base quand `DOLIBARR_DSN`
    //! est posee, et elle s'ignore sinon. Elle travaille en **entite 999**, qui
    //! n'existe pas dans les donnees reelles, et nettoie derriere elle.

    use super::*;

    // Numero propre a ce module : deux modules qui partagent une entite
    // s'effacent mutuellement leurs lignes.
    const ENTITE: i64 = 999;
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
            "DELETE FROM llx_accounting_analytic_distribution WHERE entity = :e",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_accounting_analytic_account WHERE entity = :e",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_accounting_analytic_axis WHERE entity = :e",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_accounting_bookkeeping WHERE entity = :e",
            params! { "e" => ENTITE },
        );
    }

    /// Pose une ligne du livre et rend son numero.
    fn poser_ligne(pool: &Pool, debit: &str, credit: &str, compte: &str) -> i64 {
        let mut conn = pool.get_conn().expect("connexion");
        conn.exec_drop(
            "INSERT INTO llx_accounting_bookkeeping \
               (entity, ref, piece_num, doc_date, doc_type, doc_ref, fk_doc, fk_docdet, \
                numero_compte, label_compte, label_operation, debit, credit, montant, sens, \
                fk_user_author, code_journal) \
             VALUES (:e, 'RECETTE-AN', 999, '2026-05-10', 'recette', 'X', 0, 0, \
                     :compte, 'compte', 'operation', :debit, :credit, :montant, 'D', 1, 'VE')",
            params! {
                "e" => ENTITE,
                "compte" => compte,
                "debit" => debit,
                "credit" => credit,
                "montant" => credit,
            },
        )
        .expect("insertion de la ligne");
        conn.last_insert_id() as i64
    }

    /// Le parcours complet : axe, comptes, affectation, marge, nettoyage.
    #[test]
    fn un_chantier_a_sa_marge() {
        let _verrou = verrou_entite();
        let Some(p) = pool() else { return };
        nettoyer(&p);

        let axe = creer_axe(&p, ENTITE, "T-AN", "Test analytique").expect("axe");
        let a = creer_compte(&p, ENTITE, axe, "CH-A", "Chantier A").expect("compte A");
        let b = creer_compte(&p, ENTITE, axe, "CH-B", "Chantier B").expect("compte B");

        // Recrer avec le meme code rend le meme compte, sans doublon.
        assert_eq!(
            creer_compte(&p, ENTITE, axe, "CH-A", "Chantier A").unwrap(),
            a
        );
        assert_eq!(comptes(&p, ENTITE, Some(axe)).unwrap().len(), 2);

        // Une vente de 1500 sur A, un achat de 100 partage 60/40.
        let vente = poser_ligne(&p, "0", "1500", "706000");
        let achat = poser_ligne(&p, "100", "0", "601000");

        repartir(&p, ENTITE, vente, &[(a, Decimal::from(100))]).expect("vente");
        repartir(
            &p,
            ENTITE,
            achat,
            &[(a, Decimal::from(60)), (b, Decimal::from(40))],
        )
        .expect("achat");

        // A : +1500 - 60 = 1440. B : -40.
        let m = marge(&p, ENTITE, "2026-01-01", "2026-12-31").expect("marge");
        let de = |code: &str| m.iter().find(|x| x.code == code).map(|x| x.montant);
        assert_eq!(de("CH-A"), Some(Decimal::from(1440)));
        assert_eq!(de("CH-B"), Some(Decimal::from(-40)));

        // Rejouer une affectation ne la double pas.
        repartir(&p, ENTITE, vente, &[(a, Decimal::from(100))]).expect("rejeu");
        assert_eq!(
            repartitions(&p, ENTITE, vente).expect("repartitions").len(),
            1
        );
        assert_eq!(
            marge(&p, ENTITE, "2026-01-01", "2026-12-31").unwrap().len(),
            2
        );

        // Les deux lignes sont affectees : rien d'orphelin.
        assert_eq!(
            lignes_periode(&p, ENTITE, "2026-01-01", "2026-12-31").unwrap(),
            2
        );
        assert_eq!(
            non_reparties(&p, ENTITE, "2026-01-01", "2026-12-31").unwrap(),
            0
        );

        // Une repartition fausse est refusee et ne change rien.
        assert!(repartir(&p, ENTITE, vente, &[(a, Decimal::from(70))]).is_err());
        assert!(repartir(
            &p,
            ENTITE,
            vente,
            &[(a, Decimal::from(100)), (b, Decimal::from(1))]
        )
        .is_err());
        // Un compte d'une autre entite est refuse.
        assert!(repartir(&p, ENTITE, vente, &[(999_999, Decimal::from(100))]).is_err());

        // Une ligne hors de la periode ne compte pas.
        assert!(!marge(&p, ENTITE, "2027-01-01", "2027-12-31")
            .unwrap()
            .iter()
            .any(|x| x.montant != Decimal::ZERO));

        nettoyer(&p);
        assert_eq!(
            non_reparties(&p, ENTITE, "2026-01-01", "2026-12-31").unwrap(),
            0
        );
        assert!(comptes(&p, ENTITE, None).unwrap().is_empty());
    }

    /// Une ligne non affectee est comptee comme telle : c'est ce qui dit si la
    /// marge veut dire quelque chose.
    #[test]
    fn une_ligne_sans_affectation_est_comptee() {
        let _verrou = verrou_entite();
        let Some(p) = pool() else { return };
        nettoyer(&p);

        let axe = creer_axe(&p, ENTITE, "T-AN2", "Test analytique 2").expect("axe");
        let a = creer_compte(&p, ENTITE, axe, "CH-C", "Chantier C").expect("compte");
        let l1 = poser_ligne(&p, "0", "500", "706000");
        poser_ligne(&p, "0", "200", "706000");
        repartir(&p, ENTITE, l1, &[(a, Decimal::from(100))]).expect("affectation");

        assert_eq!(
            lignes_periode(&p, ENTITE, "2026-01-01", "2026-12-31").unwrap(),
            2
        );
        assert_eq!(
            non_reparties(&p, ENTITE, "2026-01-01", "2026-12-31").unwrap(),
            1
        );

        nettoyer(&p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn une_repartition_complete_est_acceptee() {
        assert!(verifier_parts(&[(1, d("70")), (2, d("30"))]).is_ok());
        assert!(verifier_parts(&[(1, d("100"))]).is_ok());
    }

    #[test]
    fn une_repartition_incomplete_est_refusee() {
        let e = verifier_parts(&[(1, d("70")), (2, d("20"))]).unwrap_err();
        assert!(e.contains("100"), "{e}");
    }

    #[test]
    fn une_repartition_superieure_a_cent_est_refusee() {
        assert!(verifier_parts(&[(1, d("110"))]).is_err());
    }

    #[test]
    fn une_part_nulle_ou_negative_est_refusee() {
        assert!(verifier_parts(&[(1, d("100")), (2, d("0"))]).is_err());
        assert!(verifier_parts(&[(1, d("120")), (2, d("-20"))]).is_err());
    }

    #[test]
    fn le_meme_compte_deux_fois_est_refuse() {
        let e = verifier_parts(&[(1, d("50")), (1, d("50"))]).unwrap_err();
        assert!(e.contains("deux fois"), "{e}");
    }

    #[test]
    fn une_repartition_vide_est_refusee() {
        assert!(verifier_parts(&[]).is_err());
    }

    /// La contribution au resultat : un produit est positif, une charge
    /// negative. C'est la convention qui rend la marge lisible.
    #[test]
    fn la_contribution_est_positive_en_produit() {
        // Ligne de vente : 1500 au credit, rien au debit.
        assert_eq!(contribution(Decimal::ZERO, d("1500")), d("1500"));
        // Ligne d'achat : 100 au debit, rien au credit.
        assert_eq!(contribution(d("100"), Decimal::ZERO), d("-100"));
        // Les deux sur un chantier : 1400 de marge.
        assert_eq!(
            contribution(Decimal::ZERO, d("1500")) + contribution(d("100"), Decimal::ZERO),
            d("1400")
        );
    }
}
