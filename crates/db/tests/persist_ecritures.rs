//! Persistance des ecritures dans `llx_accounting_bookkeeping`.
//!
//! La base de developpement est partagee : chaque test travaille dans **sa
//! propre entite** (`900 101` et suivantes), y cree son tiers et ses factures,
//! et supprime tout a la fin - **meme si une assertion echoue**, par une garde
//! `Drop`. Le nettoyage des lignes de grand livre passe par le marqueur
//! `import_key LIKE 'ZZ-ECR-%'` que `db::persist_ecritures` pose sur chaque
//! ligne.
//!
//! Aucune assertion ne porte sur un `COUNT(*)` global : les comptes sont
//! toujours filtres par l'entite du test.

use std::error::Error;
use std::str::FromStr;

use comptabilite::{Ecriture, Ligne};
use db::invoice::{NewInvoice, NewInvoiceLine};
use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

const PREFIXE: &str = "ZZ-ECR-";

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test d'integration ignore");
            None
        }
    }
}

fn dec(valeur: &str) -> Decimal {
    valeur.parse().expect("montant de test illisible")
}

// ---------------------------------------------------------------------------
// Nettoyage, meme en cas d'echec
// ---------------------------------------------------------------------------

/// Supprime, au `Drop`, les lignes de grand livre et les documents du test.
///
/// Une assertion qui explose laisse la base propre : le `Drop` s'execute quand
/// meme, et les erreurs de nettoyage sont signalees sans masquer l'echec du
/// test.
struct Nettoyage {
    pool: Pool,
    entity: i64,
    nom: String,
}

impl Nettoyage {
    fn new(pool: &Pool, entity: i64, nom: &str) -> Self {
        Self {
            pool: pool.clone(),
            entity,
            nom: nom.to_string(),
        }
    }

    fn faire(&self) -> Result<(), Box<dyn Error>> {
        let mut conn = self.pool.get_conn()?;
        conn.exec_drop(
            "DELETE FROM llx_accounting_bookkeeping \
             WHERE entity = :entity AND import_key LIKE 'ZZ-ECR-%'",
            params! { "entity" => self.entity },
        )?;

        let societes: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
            params! { "entity" => self.entity, "nom" => self.nom.as_str() },
        )?;
        for soc in societes {
            let ventes: Vec<i64> = conn.exec(
                "SELECT rowid FROM llx_facture WHERE entity = :entity AND fk_soc = :soc",
                params! { "entity" => self.entity, "soc" => soc },
            )?;
            for f in ventes {
                conn.exec_drop(
                    "DELETE FROM llx_element_element WHERE fk_source = :f OR fk_target = :f",
                    params! { "f" => f },
                )?;
                conn.exec_drop(
                    "DELETE FROM llx_facturedet WHERE fk_facture = :f",
                    params! { "f" => f },
                )?;
                conn.exec_drop(
                    "DELETE FROM llx_facture WHERE rowid = :f",
                    params! { "f" => f },
                )?;
            }
            db::party::delete(&self.pool, self.entity, soc)?;
        }
        Ok(())
    }
}

impl Drop for Nettoyage {
    fn drop(&mut self) {
        if let Err(e) = self.faire() {
            eprintln!("nettoyage de l'entite {} impossible : {e}", self.entity);
        }
    }
}

// ---------------------------------------------------------------------------
// Mise en place
// ---------------------------------------------------------------------------

/// Une condition de reglement du dictionnaire : `db::invoice::create` en
/// cherche une dans l'entite, et une entite neuve n'en a aucune.
fn condition_reglement(pool: &Pool) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let id: Option<Option<i64>> = conn.exec_first(
        "SELECT MIN(rowid) FROM llx_c_payment_term WHERE active = 1",
        (),
    )?;
    id.flatten()
        .ok_or_else(|| "aucune condition de reglement active : test impossible".into())
}

/// Cree une facture de vente validee et rend son identifiant.
fn vente_validee(
    pool: &Pool,
    entity: i64,
    fk_soc: i64,
    date: &str,
    lignes: Vec<NewInvoiceLine>,
) -> Result<i64, Box<dyn Error>> {
    let mut facture = NewInvoice::new(fk_soc);
    facture.datef = Some(date.to_string());
    facture.ref_client = Some(format!("{PREFIXE}{date}"));
    facture.fk_cond_reglement = Some(condition_reglement(pool)?);
    facture.lines = lignes;
    let id = db::invoice::create(pool, entity, &facture)?;
    db::invoice::validate(pool, entity, id)?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// Lecture de ce qui est en base
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct EnBase {
    compte: String,
    doc_date: String,
    doc_type: String,
    doc_ref: String,
    label_operation: String,
    debit: Decimal,
    credit: Decimal,
    montant: Decimal,
    sens: String,
    journal: String,
    journal_label: String,
    piece_num: i64,
    import_key: String,
}

const SQL_LIRE: &str = "SELECT numero_compte, DATE_FORMAT(doc_date, '%Y-%m-%d'), doc_type, \
        doc_ref, COALESCE(label_operation, ''), CAST(debit AS CHAR), CAST(credit AS CHAR), \
        CAST(COALESCE(montant, 0) AS CHAR), COALESCE(sens, ''), code_journal, \
        COALESCE(journal_label, ''), piece_num, COALESCE(import_key, '') \
     FROM llx_accounting_bookkeeping";

fn ligne_base(row: &mysql::Row) -> EnBase {
    let s = |i: usize| {
        row.get_opt::<String, _>(i)
            .and_then(|v| v.ok())
            .unwrap_or_default()
    };
    let d = |i: usize| Decimal::from_str(s(i).trim()).unwrap_or_default();
    EnBase {
        compte: s(0).trim().to_string(),
        doc_date: s(1).trim().to_string(),
        doc_type: s(2).trim().to_string(),
        doc_ref: s(3).trim().to_string(),
        label_operation: s(4).trim().to_string(),
        debit: d(5),
        credit: d(6),
        montant: d(7),
        sens: s(8).trim().to_string(),
        journal: s(9).trim().to_string(),
        journal_label: s(10).trim().to_string(),
        piece_num: row.get_opt::<i64, _>(11).and_then(|v| v.ok()).unwrap_or(0),
        import_key: s(12).trim().to_string(),
    }
}

/// Les lignes d'une ecriture, dans l'ordre d'insertion.
fn lignes_de(pool: &Pool, entity: i64, numero: &str) -> Result<Vec<EnBase>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        format!("{SQL_LIRE} WHERE entity = :entity AND ref = :ref ORDER BY rowid ASC"),
        params! { "entity" => entity, "ref" => numero },
    )?;
    Ok(rows.iter().map(ligne_base).collect())
}

/// Toutes les lignes de l'entite : ce que le test a laisse en base.
fn toutes_les_lignes(pool: &Pool, entity: i64) -> Result<Vec<EnBase>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        format!("{SQL_LIRE} WHERE entity = :entity ORDER BY rowid ASC"),
        params! { "entity" => entity },
    )?;
    Ok(rows.iter().map(ligne_base).collect())
}

/// Efface les lignes d'une ecriture.
///
/// Depuis que la **validation d'une facture ecrit son ecriture comptable**,
/// ces tests ne partent plus d'une table vide : ils doivent repartir de zero
/// pour verifier `ecrire` lui-meme, et non la validation.
fn vider_ecriture(
    pool: &Pool,
    entity: i64,
    numero: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "DELETE FROM llx_accounting_bookkeeping WHERE entity = :entity AND ref = :ref",
        params! { "entity" => entity, "ref" => numero },
    )?;
    Ok(())
}

fn compter(pool: &Pool, entity: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_accounting_bookkeeping WHERE entity = :entity",
        params! { "entity" => entity },
    )?;
    Ok(n.unwrap_or(0))
}

// ---------------------------------------------------------------------------
// 1. Une vente a un taux de TVA : trois lignes en base
// ---------------------------------------------------------------------------

#[test]
fn trois_lignes_en_base_pour_une_vente() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_101;
    let nom = format!("{PREFIXE}Persist Vente");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = Nettoyage::new(&pool, entity, &nom);
    garde.faire()?;

    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;
    // 1 000,00 HT a 20 % : 200,00 de TVA, 1 200,00 TTC.
    let id = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-05-12",
        vec![NewInvoiceLine::new(
            "Reprise de maconnerie",
            dec("1"),
            dec("1000"),
            dec("20"),
        )],
    )?;

    let ecriture = db::ecritures::facture_vente(&pool, entity, id)?;
    // La validation a deja ecrit l'ecriture : c'est le comportement voulu. On
    // repart de zero pour verifier `ecrire` lui-meme.
    assert!(
        db::persist_ecritures::deja_ecrite(&pool, entity, &ecriture.numero)?,
        "la validation doit avoir ecrit l'ecriture"
    );
    vider_ecriture(&pool, entity, &ecriture.numero)?;
    assert!(!db::persist_ecritures::deja_ecrite(
        &pool,
        entity,
        &ecriture.numero
    )?);

    let n = db::persist_ecritures::ecrire(&pool, entity, &ecriture)?;
    assert_eq!(n, 3, "client, produit et TVA : trois lignes");

    let lignes = lignes_de(&pool, entity, &ecriture.numero)?;
    assert_eq!(lignes.len(), 3, "trois lignes en base, pas une de plus");

    let piece = lignes[0].piece_num;
    for l in &lignes {
        assert_eq!(l.doc_date, "2026-05-12", "date du document");
        assert_eq!(l.doc_type, db::persist_ecritures::DOC_TYPE);
        assert_eq!(
            l.doc_ref,
            ecriture.numero.trim_start_matches("VE-"),
            "la piece est la reference du document"
        );
        assert_eq!(l.journal, "VE");
        assert_eq!(l.journal_label, "Journal des ventes");
        assert_eq!(l.piece_num, piece, "une ecriture = un numero de piece");
        assert!(
            l.import_key.starts_with(db::persist_ecritures::PREFIXE_CLE),
            "marqueur de ligne : {}",
            l.import_key
        );
        assert_eq!(l.import_key.chars().count(), 14, "varchar(14)");
        assert!(
            !l.label_operation.is_empty(),
            "un libelle d'operation est ecrit"
        );
        // Un seul sens par ligne, et `montant` en valeur absolue.
        assert!(
            l.debit.is_zero() || l.credit.is_zero(),
            "debit ou credit, jamais les deux : {l:?}"
        );
        assert_eq!(l.montant, l.debit + l.credit, "montant = la ligne : {l:?}");
        match l.sens.as_str() {
            "D" => assert_eq!(l.montant, l.debit, "{l:?}"),
            "C" => assert_eq!(l.montant, l.credit, "{l:?}"),
            autre => panic!("sens inattendu : {autre}"),
        }
    }

    // Le debit (client) vaut le credit (produit + TVA).
    let (client, produit, tva) = (&lignes[0], &lignes[1], &lignes[2]);
    assert_eq!(client.compte, db::ecritures::COMPTE_CLIENTS);
    assert_eq!(client.debit, dec("1200.00"));
    assert_eq!(client.credit, Decimal::ZERO);
    assert_eq!(client.sens, "D");
    assert_eq!(produit.compte, db::ecritures::COMPTE_VENTES);
    assert_eq!(produit.credit, dec("1000.00"));
    assert_eq!(produit.sens, "C");
    assert_eq!(tva.compte, db::ecritures::COMPTE_TVA_COLLECTEE);
    assert_eq!(tva.credit, dec("200.00"));
    assert!(!produit.label_operation.contains("TVA"));
    assert!(
        tva.label_operation.contains("TVA 20"),
        "le taux est dans le libelle : {}",
        tva.label_operation
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Idempotence : reecrire ne double pas
// ---------------------------------------------------------------------------

#[test]
fn reecrire_ne_double_pas_les_lignes() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_102;
    let nom = format!("{PREFIXE}Persist Doublon");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = Nettoyage::new(&pool, entity, &nom);
    garde.faire()?;

    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;
    let id = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-05-13",
        vec![NewInvoiceLine::new("Pose", dec("2"), dec("500"), dec("10"))],
    )?;
    let ecriture = db::ecritures::facture_vente(&pool, entity, id)?;

    assert_eq!(db::persist_ecritures::ecrire(&pool, entity, &ecriture)?, 3);
    assert_eq!(
        db::persist_ecritures::ecrire(&pool, entity, &ecriture)?,
        3,
        "le second passage rend le meme nombre de lignes"
    );
    assert_eq!(
        compter(&pool, entity)?,
        3,
        "et la base en contient toujours trois"
    );

    // Troisieme passage, pour etre sur que la suppression ciblee ne derive pas.
    assert_eq!(db::persist_ecritures::ecrire(&pool, entity, &ecriture)?, 3);
    assert_eq!(compter(&pool, entity)?, 3);

    Ok(())
}

// ---------------------------------------------------------------------------
// 3. deja_ecrite
// ---------------------------------------------------------------------------

#[test]
fn deja_ecrite_est_vrai_apres_ecriture() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_103;
    let nom = format!("{PREFIXE}Persist Deja");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = Nettoyage::new(&pool, entity, &nom);
    garde.faire()?;

    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;
    let id = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-05-14",
        vec![NewInvoiceLine::new(
            "Enduit",
            dec("1"),
            dec("100"),
            dec("20"),
        )],
    )?;
    let ecriture = db::ecritures::facture_vente(&pool, entity, id)?;

    // La validation ecrit l'ecriture ; on efface pour tester la detection.
    vider_ecriture(&pool, entity, &ecriture.numero)?;
    assert!(
        !db::persist_ecritures::deja_ecrite(&pool, entity, &ecriture.numero)?,
        "rien en base avant l'ecriture"
    );
    db::persist_ecritures::ecrire(&pool, entity, &ecriture)?;
    assert!(
        db::persist_ecritures::deja_ecrite(&pool, entity, &ecriture.numero)?,
        "l'ecriture est tenue apres l'appel"
    );
    // Une autre entite ne la voit pas : le cloisonnement tient.
    assert!(!db::persist_ecritures::deja_ecrite(
        &pool,
        entity + 1,
        &ecriture.numero
    )?);
    assert!(!db::persist_ecritures::deja_ecrite(&pool, entity, "")?);

    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Une ecriture desequilibree n'ecrit rien
// ---------------------------------------------------------------------------

fn ligne(compte: &str, debit: &str, credit: &str) -> Ligne {
    Ligne {
        compte: compte.to_string(),
        libelle: "Facture FA9 - Tiers".to_string(),
        debit: dec(debit),
        credit: dec(credit),
        journal: "VE".to_string(),
        date: "2026-05-15".to_string(),
    }
}

#[test]
fn une_ecriture_desequilibree_n_ecrit_rien() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_104;
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = Nettoyage::new(&pool, entity, &format!("{PREFIXE}Persist Desequilibre"));

    let desequilibree = Ecriture {
        numero: format!("{PREFIXE}VE-FA9"),
        date: "2026-05-15".to_string(),
        libelle: "Facture FA9".to_string(),
        lignes: vec![
            ligne("411000", "1200", "0"),
            // 1 000 au credit pour 1 200 au debit : il manque 200.
            ligne("706000", "0", "1000"),
        ],
    };

    let erreur = db::persist_ecritures::ecrire(&pool, entity, &desequilibree);
    assert!(erreur.is_err(), "le moteur doit refuser");
    assert_eq!(
        compter(&pool, entity)?,
        0,
        "une ecriture refusee ne laisse aucune ligne"
    );

    // Une ecriture juste, puis une reecriture fausse du meme numero : la
    // premiere reste en base, intacte.
    let juste = Ecriture {
        lignes: vec![
            ligne("411000", "1200", "0"),
            ligne("706000", "0", "1000"),
            ligne("445710", "0", "200"),
        ],
        libelle: "Facture FA9 - TVA 20 %".to_string(),
        ..desequilibree.clone()
    };
    assert_eq!(db::persist_ecritures::ecrire(&pool, entity, &juste)?, 3);
    assert!(
        db::persist_ecritures::ecrire(&pool, entity, &desequilibree).is_err(),
        "la reecriture fausse est refusee"
    );
    assert_eq!(
        compter(&pool, entity)?,
        3,
        "et n'a pas efface l'ecriture juste"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Journal : tout ou rien
// ---------------------------------------------------------------------------

#[test]
fn un_journal_desequilibre_n_ecrit_rien() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_105;
    let nom = format!("{PREFIXE}Persist Journal KO");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = Nettoyage::new(&pool, entity, &nom);
    garde.faire()?;

    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;
    vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-05-16",
        vec![NewInvoiceLine::new(
            "Lot A",
            dec("1"),
            dec("1000"),
            dec("20"),
        )],
    )?;
    let cassee = vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-05-17",
        vec![NewInvoiceLine::new(
            "Lot B",
            dec("1"),
            dec("500"),
            dec("20"),
        )],
    )?;

    // On casse l'en-tete de la seconde facture : son TTC ne vaut plus ses
    // lignes, le pont la refuse. Une seule facture fausse doit suffire a
    // arreter tout le journal.
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_facture SET total_ttc = total_ttc + 1 WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => cassee },
    )?;
    drop(conn);

    // On mesure un **ecart**, pas un absolu : les factures validees ont deja
    // ecrit leurs ecritures, et un total absolu ne dirait plus rien.
    let avant = compter(&pool, entity)?;
    let resultat = db::persist_ecritures::ecrire_journal(&pool, entity, "VE", 1000);
    assert!(resultat.is_err(), "le journal contient une facture fausse");
    assert_eq!(
        compter(&pool, entity)?,
        avant,
        "une seule facture fausse doit arreter tout le journal"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 6. Journal : ce qui est en base est equilibre
// ---------------------------------------------------------------------------

#[test]
fn le_journal_ecrit_est_equilibre() -> Result<(), Box<dyn Error>> {
    let entity: i64 = 900_106;
    let nom = format!("{PREFIXE}Persist Journal OK");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = Nettoyage::new(&pool, entity, &nom);
    garde.faire()?;

    let fk_soc = db::party::create(&pool, entity, &db::party::NewParty::client(&nom))?;
    vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-05-20",
        vec![NewInvoiceLine::new(
            "Lot A",
            dec("1"),
            dec("1000"),
            dec("20"),
        )],
    )?;
    // Deux taux dans la meme facture : deux lignes de TVA, donc quatre lignes.
    vente_validee(
        &pool,
        entity,
        fk_soc,
        "2026-05-21",
        vec![
            NewInvoiceLine::new("Lot B", dec("1"), dec("500"), dec("10")),
            NewInvoiceLine::new("Lot C", dec("1"), dec("250"), dec("20")),
        ],
    )?;

    let n = db::persist_ecritures::ecrire_journal(&pool, entity, "VE", 1000)?;
    assert_eq!(n, 7, "3 lignes + 4 lignes (deux taux de TVA)");

    let lignes = toutes_les_lignes(&pool, entity)?;
    assert_eq!(lignes.len(), 7);

    let debit: Decimal = lignes.iter().map(|l| l.debit).sum();
    let credit: Decimal = lignes.iter().map(|l| l.credit).sum();
    assert_eq!(
        debit, credit,
        "ce qui est en base apres ecrire_journal est equilibre"
    );
    // 1 200 + 550 + 300 = 2 050 de debit (deux creances clients).
    assert_eq!(debit, dec("2050.00"));

    // Rejouer le journal ne double rien.
    let n2 = db::persist_ecritures::ecrire_journal(&pool, entity, "VE", 1000)?;
    assert_eq!(n2, 7);
    assert_eq!(toutes_les_lignes(&pool, entity)?.len(), 7);

    // Un journal que le pont ne produit pas n'ecrit rien.
    assert_eq!(
        db::persist_ecritures::ecrire_journal(&pool, entity, "BQ", 1000)?,
        0
    );
    assert_eq!(toutes_les_lignes(&pool, entity)?.len(), 7);

    Ok(())
}
