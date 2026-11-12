//! Lettrage : rapprocher la creance de son encaissement, et l'ecrire en base.
//!
//! `comptabilite::Lettrage` fait le calcul en memoire, sur un grand livre
//! derive. Ici, on lettre les **ecritures telles qu'elles sont en base** : la
//! ligne est identifiee par son `rowid`, et le resultat s'ecrit dans
//! `lettering_code` et `date_lettering`. C'est la seule facon de dire plus tard
//! « cette facture est soldee, et voila par quel reglement ».
//!
//! L'algorithme est le meme : **FIFO**, le debit le plus ancien mange le credit
//! le plus ancien. Ce n'est pas un rapprochement intelligent — c'est celui d'un
//! comptable presse, et il faut le savoir : deux reglements partiels de meme
//! montant se lettrent dans l'ordre des dates, pas selon la bonne facture.
//!
//! Ce qui n'est jamais lettre : les lignes deja lettres. Rejouer un lettrage ne
//! defait pas un lettrage existant ; il ne prend que le reste ouvert.

use std::collections::VecDeque;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Une ligne brute lue en base : `(rowid, date, debit, credit, libelle)`.
type LigneBrute = (i64, Option<String>, String, String, Option<String>);
/// Un compte ouvert : `(compte, solde, nombre de lignes)`.
pub type CompteOuvert = (String, Decimal, i64);

/// Une ligne d'ecriture, telle qu'elle est en base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ligne {
    pub rowid: i64,
    pub date: String,
    pub debit: Decimal,
    pub credit: Decimal,
    pub libelle: String,
}

/// Un debit et un credit rapproches, par leur `rowid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paire {
    pub debit: i64,
    pub credit: i64,
    pub montant: Decimal,
}

/// Ce que le lettrage d'un compte a donne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bilan {
    pub compte: String,
    /// Le code pose sur les lignes rapprochees.
    pub code: String,
    pub paires: Vec<Paire>,
    /// Ce qui reste ouvert.
    pub restes: Vec<Ligne>,
}

impl Bilan {
    /// Le montant rapproche.
    pub fn total(&self) -> Decimal {
        self.paires.iter().map(|p| p.montant).sum()
    }

    /// Le solde qui reste a rapprocher, en valeur absolue.
    pub fn reste(&self) -> Decimal {
        let d: Decimal = self.restes.iter().map(|l| l.debit).sum();
        let c: Decimal = self.restes.iter().map(|l| l.credit).sum();
        (d - c).abs()
    }
}

/// Le prefixe du code de lettrage, suivi du compte et du rang.
pub const PREFIXE: &str = "PA-LET-";

/// Les lignes non lettres d'un compte, du plus ancien au plus recent.
pub fn lignes(pool: &Pool, entity: i64, compte: &str) -> Result<Vec<Ligne>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<LigneBrute> = conn.exec(
        "SELECT rowid, DATE_FORMAT(doc_date, '%Y-%m-%d'), \
                CAST(debit AS CHAR), CAST(credit AS CHAR), label_operation \
         FROM llx_accounting_bookkeeping \
         WHERE entity = :entity AND numero_compte = :compte \
           AND (lettering_code IS NULL OR lettering_code = '') \
           AND (debit <> 0 OR credit <> 0) \
         ORDER BY doc_date ASC, rowid ASC",
        params! { "entity" => entity, "compte" => compte },
    )?;

    Ok(rows
        .into_iter()
        .map(|(rowid, date, debit, credit, libelle)| Ligne {
            rowid,
            date: date.unwrap_or_default(),
            debit: dec(&debit),
            credit: dec(&credit),
            libelle: libelle.unwrap_or_default(),
        })
        .collect())
}

/// Les comptes qui portent encore quelque chose a rapprocher, du plus gros
/// solde au plus petit.
///
/// On ne rend que les comptes dont le solde **non lettre** est non nul : un
/// compte solde n'a rien a faire dans une corbeille de lettrage.
pub fn comptes_ouverts(pool: &Pool, entity: i64) -> Result<Vec<CompteOuvert>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(String, Option<String>, Option<i64>)> = conn.exec(
        "SELECT numero_compte, CAST(SUM(debit) - SUM(credit) AS CHAR), COUNT(*) \
         FROM llx_accounting_bookkeeping \
         WHERE entity = :entity \
           AND (lettering_code IS NULL OR lettering_code = '') \
         GROUP BY numero_compte \
         HAVING ABS(SUM(debit) - SUM(credit)) > 0.004 \
         ORDER BY ABS(SUM(debit) - SUM(credit)) DESC",
        params! { "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .map(|(compte, solde, n)| (compte, dec(solde.as_deref().unwrap_or("0")), n.unwrap_or(0)))
        .collect())
}

/// Apparie les lignes d'un compte, sans rien ecrire. Testable.
pub fn apparier(lignes: &[Ligne]) -> (Vec<Paire>, Vec<Ligne>) {
    let mut debits: VecDeque<(usize, Decimal)> = VecDeque::new();
    let mut credits: VecDeque<(usize, Decimal)> = VecDeque::new();
    for (i, l) in lignes.iter().enumerate() {
        if l.debit > Decimal::ZERO {
            debits.push_back((i, l.debit));
        }
        if l.credit > Decimal::ZERO {
            credits.push_back((i, l.credit));
        }
    }

    let mut paires = Vec::new();
    while let (Some((i_debit, reste_debit)), Some((i_credit, reste_credit))) =
        (debits.front().copied(), credits.front().copied())
    {
        let montant = reste_debit.min(reste_credit);
        paires.push(Paire {
            debit: lignes[i_debit].rowid,
            credit: lignes[i_credit].rowid,
            montant,
        });

        // On consomme la tete ; si elle est soldee, elle sort de la file.
        if let Some(tete) = debits.front_mut() {
            tete.1 -= montant;
            if tete.1 <= Decimal::ZERO {
                debits.pop_front();
            }
        }
        if let Some(tete) = credits.front_mut() {
            tete.1 -= montant;
            if tete.1 <= Decimal::ZERO {
                credits.pop_front();
            }
        }
    }

    let mut restes: Vec<Ligne> = Vec::new();
    for (i, reste) in debits {
        let mut l = lignes[i].clone();
        l.debit = reste;
        restes.push(l);
    }
    for (i, reste) in credits {
        let mut l = lignes[i].clone();
        l.credit = reste;
        restes.push(l);
    }
    restes.sort_by_key(|l| (l.date.clone(), l.rowid));

    (paires, restes)
}

/// Lettre un compte : appariement FIFO, puis ecriture du code sur les lignes.
///
/// Le code porte le compte et un rang qui avance : deux lettrages successifs
/// du meme compte ne se confondent pas.
pub fn lettrer(pool: &Pool, entity: i64, compte: &str) -> Result<Bilan, Box<dyn Error>> {
    let lignes = lignes(pool, entity, compte)?;
    let (paires, restes) = apparier(&lignes);

    let code = format!("{PREFIXE}{}", compte.trim());
    if !paires.is_empty() {
        let mut conn = pool.get_conn()?;
        let mut tx = conn.start_transaction(mysql::TxOpts::default())?;
        for p in &paires {
            let code_ligne = format!("{code}-{}", p.montant.normalize());
            tx.exec_drop(
                "UPDATE llx_accounting_bookkeeping \
                 SET lettering_code = :code, date_lettering = NOW() \
                 WHERE entity = :entity AND rowid IN (:debit, :credit)",
                params! {
                    "code" => code_ligne,
                    "entity" => entity,
                    "debit" => p.debit,
                    "credit" => p.credit,
                },
            )?;
        }
        tx.commit()?;
    }

    Ok(Bilan {
        compte: compte.to_string(),
        code,
        paires,
        restes,
    })
}

fn dec(v: &str) -> Decimal {
    Decimal::from_str(v.trim()).unwrap_or_default()
}

#[cfg(test)]
mod tests_base {
    //! La recette, en test : sur la vraie base quand `DOLIBARR_DSN` est posee,
    //! en entite 999, avec nettoyage.

    use super::*;

    // Numero propre a ce module : deux modules qui partagent une
    // entite s'effacent mutuellement leurs lignes.
    const ENTITE: i64 = 997;
    /// Les tests de ce module partagent une entite et une base MySQL : ils
    /// doivent s'executer **l'un apres l'autre**. Deux tests simultanes se
    /// supprimaient mutuellement leurs lignes (`nettoyer` efface toute
    /// l'entite), et la meme reference de recette entrait en collision — d'ou
    /// des echecs aleatoires qui n'avaient rien a voir avec le code teste.
    fn verrou_entite() -> std::sync::MutexGuard<'static, ()> {
        static VERROU: std::sync::Mutex<()> = std::sync::Mutex::new(());
        VERROU.lock().unwrap_or_else(|e| e.into_inner())
    }

    const COMPTE: &str = "411000";

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
            "DELETE FROM llx_accounting_bookkeeping WHERE entity = :e",
            params! { "e" => ENTITE },
        );
    }

    fn poser(pool: &Pool, date: &str, debit: &str, credit: &str) -> i64 {
        let mut conn = pool.get_conn().expect("connexion");
        conn.exec_drop(
            "INSERT INTO llx_accounting_bookkeeping \
               (entity, ref, piece_num, doc_date, doc_type, doc_ref, fk_doc, fk_docdet, \
                numero_compte, label_compte, label_operation, debit, credit, montant, sens, \
                fk_user_author, code_journal) \
             VALUES (:e, 'RECETTE-LET', 1, :date, 'recette', 'X', 0, 0, :compte, 'Clients', \
                     'recette', :debit, :credit, :debit, 'D', 1, 'VE')",
            params! {
                "e" => ENTITE,
                "date" => date,
                "compte" => COMPTE,
                "debit" => debit,
                "credit" => credit,
            },
        )
        .expect("insertion");
        conn.last_insert_id() as i64
    }

    /// Le parcours complet : deux lignes se lettrent, la troisieme reste ouverte,
    /// et le code est bien ecrit en base.
    #[test]
    fn un_lettrage_ecrit_le_code_et_laisse_le_reste_ouvert() {
        let _verrou = verrou_entite();
        let Some(p) = pool() else { return };
        nettoyer(&p);

        let facture = poser(&p, "2026-01-10", "100", "0");
        let reglement = poser(&p, "2026-02-10", "0", "100");
        let autre = poser(&p, "2026-03-10", "50", "0");

        let bilan = lettrer(&p, ENTITE, COMPTE).expect("lettrage");
        assert_eq!(bilan.paires.len(), 1, "un debit et un credit de 100");
        assert_eq!(bilan.total(), dec("100"));
        assert_eq!(bilan.restes.len(), 1);
        assert_eq!(bilan.restes[0].rowid, autre);
        assert_eq!(bilan.reste(), dec("50"));

        // Les deux lignes rapprochees portent le code, la troisieme non.
        let mut conn = p.get_conn().expect("connexion");
        let codes: Vec<(i64, Option<String>)> = conn
            .exec(
                "SELECT rowid, lettering_code FROM llx_accounting_bookkeeping \
                 WHERE entity = :e ORDER BY rowid",
                params! { "e" => ENTITE },
            )
            .unwrap();
        let code_de = |id: i64| {
            codes
                .iter()
                .find(|(r, _)| *r == id)
                .and_then(|(_, c)| c.clone())
                .unwrap_or_default()
        };
        assert!(
            code_de(facture).starts_with(PREFIXE),
            "{:?}",
            code_de(facture)
        );
        assert_eq!(
            code_de(facture),
            code_de(reglement),
            "le meme code des deux cotes"
        );
        assert_eq!(
            code_de(autre),
            "",
            "la ligne ouverte ne doit pas etre lettree"
        );

        // Le compte reste ouvert pour 50.
        let ouverts = comptes_ouverts(&p, ENTITE).expect("comptes");
        let ligne = ouverts
            .iter()
            .find(|(c, _, _)| c == COMPTE)
            .expect("le compte doit rester ouvert");
        assert_eq!(ligne.1, dec("50"));

        // Rejouer ne reprend pas les lignes deja lettrees.
        let encore = lettrer(&p, ENTITE, COMPTE).expect("second lettrage");
        assert!(encore.paires.is_empty(), "rien de neuf a rapprocher");
        assert_eq!(encore.restes.len(), 1);

        nettoyer(&p);
    }

    #[test]
    fn un_compte_sans_ligne_ne_produit_aucune_paire() {
        let _verrou = verrou_entite();
        let Some(p) = pool() else { return };
        nettoyer(&p);
        let bilan = lettrer(&p, ENTITE, COMPTE).expect("lettrage");
        assert!(bilan.paires.is_empty());
        assert!(bilan.restes.is_empty());
        nettoyer(&p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(rowid: i64, date: &str, debit: &str, credit: &str) -> Ligne {
        Ligne {
            rowid,
            date: date.to_string(),
            debit: dec(debit),
            credit: dec(credit),
            libelle: String::new(),
        }
    }

    /// Une facture de 100 et son reglement de 100 : une paire, rien qui reste.
    #[test]
    fn une_facture_et_son_reglement_font_une_paire() {
        let lignes = vec![
            l(1, "2026-01-10", "100", "0"),
            l(2, "2026-02-10", "0", "100"),
        ];
        let (paires, restes) = apparier(&lignes);
        assert_eq!(paires.len(), 1);
        assert_eq!(paires[0].montant, dec("100"));
        assert_eq!((paires[0].debit, paires[0].credit), (1, 2));
        assert!(restes.is_empty());
    }

    /// Le plus ancien debit mange le plus ancien credit, pas le plus proche.
    #[test]
    fn l_appariement_est_fifo() {
        let lignes = vec![
            l(1, "2026-01-01", "100", "0"),
            l(2, "2026-02-01", "50", "0"),
            l(3, "2026-03-01", "0", "120"),
        ];
        let (paires, restes) = apparier(&lignes);
        // 100 du premier debit, puis 20 sur le second ; il reste 30.
        assert_eq!(paires.len(), 2);
        assert_eq!(paires[0].debit, 1);
        assert_eq!(paires[0].montant, dec("100"));
        assert_eq!(paires[1].debit, 2);
        assert_eq!(paires[1].montant, dec("20"));
        assert_eq!(restes.len(), 1);
        assert_eq!(restes[0].rowid, 2);
        assert_eq!(restes[0].debit, dec("30"));
    }

    #[test]
    fn un_reglement_partiel_laisse_un_reste() {
        let lignes = vec![
            l(1, "2026-01-01", "100", "0"),
            l(2, "2026-02-01", "0", "40"),
        ];
        let (paires, restes) = apparier(&lignes);
        assert_eq!(paires.len(), 1);
        assert_eq!(paires[0].montant, dec("40"));
        assert_eq!(restes.len(), 1);
        assert_eq!(restes[0].debit, dec("60"));
    }

    /// Deux debits, deux credits : les paires se croisent dans l'ordre des dates.
    #[test]
    fn deux_debits_et_deux_credits_se_croisent() {
        let lignes = vec![
            l(1, "2026-01-01", "100", "0"),
            l(2, "2026-02-01", "100", "0"),
            l(3, "2026-03-01", "0", "150"),
            l(4, "2026-04-01", "0", "50"),
        ];
        let (paires, restes) = apparier(&lignes);
        assert_eq!(paires.len(), 3);
        assert!(restes.is_empty());
        let total: Decimal = paires.iter().map(|p| p.montant).sum();
        assert_eq!(total, dec("200"));
    }

    #[test]
    fn un_compte_sans_mouvement_ne_produit_rien() {
        let (paires, restes) = apparier(&[]);
        assert!(paires.is_empty());
        assert!(restes.is_empty());
    }

    #[test]
    fn un_seul_debit_sans_credit_reste_ouvert() {
        let lignes = vec![l(1, "2026-01-01", "100", "0")];
        let (paires, restes) = apparier(&lignes);
        assert!(paires.is_empty());
        assert_eq!(restes.len(), 1);
        assert_eq!(restes[0].debit, dec("100"));
    }

    #[test]
    fn le_solde_du_bilan_est_la_valeur_absolue_du_reste() {
        let bilan = Bilan {
            compte: "411000".into(),
            code: "PA-LET-411000".into(),
            paires: vec![Paire {
                debit: 1,
                credit: 2,
                montant: dec("40"),
            }],
            restes: vec![l(1, "2026-01-01", "60", "0")],
        };
        assert_eq!(bilan.total(), dec("40"));
        assert_eq!(bilan.reste(), dec("60"));
    }
}
