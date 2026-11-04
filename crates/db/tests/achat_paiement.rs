//! Paiements fournisseur : partiel, solde, refus, cloisonnement d'entite.
//!
//! Tout ce qui est ecrit ici vit dans les entites de test 900 401 (les cas
//! nominaux) et 900 402 (le cloisonnement), sur des fournisseurs prefixes
//! `ZZ-PAYF-`. Un garde `Drop` supprime les paiements, les factures et le tiers
//! a la fin de chaque test, **meme quand une assertion echoue** : sans cela un
//! test rouge laisserait des factures fournisseurs payees en travers de la base.

use std::error::Error;
use std::str::FromStr;

use db::achat_ecriture::{LigneAchat, NouvelleFactureAchat};
use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// Entite des cas nominaux.
const ENTITY: i64 = 900_401;
/// Entite voisine : elle ne doit rien voir des factures de `ENTITY`.
const ENTITY_VOISINE: i64 = 900_402;

const PREFIXE: &str = "ZZ-PAYF-";

/// Garde de nettoyage : supprime tout ce que le test a cree, quoi qu'il arrive.
struct Garde {
    pool: Pool,
    entity: i64,
    noms: Vec<String>,
}

impl Garde {
    /// Nettoie les restes d'une execution precedente interrompue avant de rendre
    /// la main : un test ne doit jamais dependre de l'etat laisse par le
    /// precedent.
    fn new(pool: &Pool, entity: i64, noms: &[&str]) -> Self {
        let garde = Self {
            pool: pool.clone(),
            entity,
            noms: noms.iter().map(|n| (*n).to_string()).collect(),
        };
        if let Err(e) = nettoyer(pool, entity, &garde.noms) {
            eprintln!("nettoyage initial incomplet ({e})");
        }
        garde
    }
}

impl Drop for Garde {
    fn drop(&mut self) {
        if let Err(e) = nettoyer(&self.pool, self.entity, &self.noms) {
            eprintln!("nettoyage incomplet ({e}) : des lignes ZZ-PAYF- subsistent");
        }
    }
}

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

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// Supprime, dans l'ordre impose par les cles etrangeres : les affectations, les
/// paiements, les lignes de facture, les factures, puis le tiers.
///
/// Les paiements sont reperes par **leurs liens** et non par un prefixe de
/// reference : un test qui echoue en cours de route laisse parfois un paiement
/// sans lien, et il ne doit pas survivre pour autant.
fn nettoyer(pool: &Pool, entity: i64, noms: &[String]) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;

    let mut societes: Vec<i64> = Vec::new();
    for nom in noms {
        let ids: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom = :nom",
            params! { "entity" => entity, "nom" => nom.as_str() },
        )?;
        societes.extend(ids);
    }

    for soc in societes {
        let factures: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_facture_fourn WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => entity, "soc" => soc },
        )?;
        for f in factures {
            let paiements: Vec<i64> = conn.exec(
                "SELECT DISTINCT fk_paiementfourn FROM llx_paiementfourn_facturefourn \
                 WHERE fk_facturefourn = :f AND fk_paiementfourn IS NOT NULL",
                params! { "f" => f },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_paiementfourn_facturefourn WHERE fk_facturefourn = :f",
                params! { "f" => f },
            )?;
            for p in paiements {
                conn.exec_drop(
                    "DELETE FROM llx_paiementfourn WHERE rowid = :p",
                    params! { "p" => p },
                )?;
            }
            conn.exec_drop(
                "DELETE FROM llx_facture_fourn_det WHERE fk_facture_fourn = :f",
                params! { "f" => f },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_facture_fourn WHERE rowid = :f",
                params! { "f" => f },
            )?;
        }
        conn.exec_drop(
            "DELETE FROM llx_societe WHERE rowid = :id",
            params! { "id" => soc },
        )?;
    }
    Ok(())
}

fn fournisseur(pool: &Pool, nom: &str) -> Result<i64, Box<dyn Error>> {
    db::achat::creer_fournisseur(pool, ENTITY, nom, None)
}

/// Facture fournisseur de 1 000 € HT a 20 % => **1 200 € TTC**.
///
/// `ref_supplier` est imposee : `uk_facture_fourn_ref_supplier` refuse deux
/// factures sans reference chez le meme fournisseur.
fn facture_de_1200(
    pool: &Pool,
    fk_soc: i64,
    ref_supplier: &str,
    valider: bool,
) -> Result<i64, Box<dyn Error>> {
    let mut new = NouvelleFactureAchat::new(fk_soc);
    new.ref_supplier = Some(ref_supplier.to_string());
    new.datef = Some("2026-03-10".into());
    new.lines = vec![LigneAchat::new(
        "Sable 0/4 livre",
        dec("1"),
        dec("1000"),
        dec("20"),
    )];
    let id = db::achat_ecriture::creer(pool, ENTITY, &new)?;
    if valider {
        db::achat_ecriture::valider(pool, ENTITY, id)?;
    }
    Ok(id)
}

/// `(fk_statut, paye)` relus en base : c'est l'etat que l'ecran affiche.
fn etat_facture(pool: &Pool, id: i64) -> Result<(i64, i64), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let etat: Option<(i64, i64)> = conn.exec_first(
        "SELECT fk_statut, paye FROM llx_facture_fourn WHERE rowid = :id",
        params! { "id" => id },
    )?;
    etat.ok_or_else(|| format!("facture fournisseur {id} introuvable").into())
}

/// Colonnes brutes d'un paiement fournisseur, telles qu'ecrites en base.
struct LignePaiement {
    entity: i64,
    amount: Decimal,
    datep: String,
    fk_paiement: i64,
    fk_bank: i64,
    statut: i16,
}

/// Colonnes brutes du paiement, pour verifier ce qui est reellement ecrit.
fn ligne_paiement(pool: &Pool, id: i64) -> Result<LignePaiement, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ligne: Option<(i64, String, String, i64, i64, i16)> = conn.exec_first(
        "SELECT entity, CAST(amount AS CHAR), DATE_FORMAT(datep, '%Y-%m-%d'), fk_paiement, \
                fk_bank, statut \
         FROM llx_paiementfourn WHERE rowid = :id",
        params! { "id" => id },
    )?;
    let Some((entity, amount, datep, fk_paiement, fk_bank, statut)) = ligne else {
        return Err(format!("paiement fournisseur {id} introuvable").into());
    };
    Ok(LignePaiement {
        entity,
        amount: Decimal::from_str(amount.trim())?,
        datep,
        fk_paiement,
        fk_bank,
        statut,
    })
}

fn compter_paiements(pool: &Pool, entity: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_paiementfourn WHERE entity = :entity",
        params! { "entity" => entity },
    )?;
    Ok(n.unwrap_or(0))
}

#[test]
fn un_paiement_partiel_laisse_un_reste() -> Result<(), Box<dyn Error>> {
    let nom = format!("{PREFIXE}Partiel");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = Garde::new(&pool, ENTITY, &[nom.as_str()]);

    let fk_soc = fournisseur(&pool, &nom)?;
    let id = facture_de_1200(&pool, fk_soc, "PAYF-PART-1", true)?;

    // Rien de paye au depart : le reste du est le TTC entier.
    assert_eq!(
        db::achat_paiement::reste_a_payer(&pool, ENTITY, id)?,
        dec("1200")
    );
    assert!(db::achat_paiement::paiements(&pool, ENTITY, id)?.is_empty());

    let p = db::achat_paiement::payer(&pool, ENTITY, id, dec("500"), "VIR", "2026-03-12")?;
    assert!(p > 0);

    assert_eq!(
        db::achat_paiement::reste_a_payer(&pool, ENTITY, id)?,
        dec("700.00"),
        "1 200 TTC moins 500 payes"
    );

    // La facture reste a « validee, non payee ».
    let (statut, paye) = etat_facture(&pool, id)?;
    assert_eq!(statut, i64::from(db::achat::STATUT_VALIDEE));
    assert_eq!(paye, 0, "un paiement partiel ne solde pas la facture");

    // Ce qui est reellement ecrit dans llx_paiementfourn.
    let brut = ligne_paiement(&pool, p)?;
    assert_eq!(brut.entity, ENTITY);
    assert_eq!(brut.amount, dec("500"));
    assert_eq!(brut.datep, "2026-03-12");
    assert_eq!(
        brut.fk_paiement,
        db::payment::MODE_VIREMENT,
        "« VIR » doit se resoudre dans le dictionnaire llx_c_paiement"
    );
    assert_eq!(
        brut.fk_bank, 0,
        "aucune ligne bancaire n'est ecrite : le rapprochement reste a faire"
    );
    assert_eq!(brut.statut, db::achat_paiement::STATUT_PAIEMENT_VALIDE);

    // Relecture par l'API.
    let paiements = db::achat_paiement::paiements(&pool, ENTITY, id)?;
    assert_eq!(paiements.len(), 1);
    assert!(
        paiements[0].ref_.starts_with("PAYF"),
        "ref: {}",
        paiements[0].ref_
    );
    assert_eq!(paiements[0].montant, dec("500"));
    assert_eq!(paiements[0].montant_affecte, dec("500"));
    assert_eq!(paiements[0].mode.as_deref(), Some("VIR"));
    assert_eq!(paiements[0].datep.as_deref(), Some("2026-03-12"));
    assert_eq!(
        paiements[0].statut,
        db::achat_paiement::STATUT_PAIEMENT_VALIDE
    );

    drop(garde);
    Ok(())
}

#[test]
fn deux_paiements_soldent_la_facture() -> Result<(), Box<dyn Error>> {
    let nom = format!("{PREFIXE}Solde");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = Garde::new(&pool, ENTITY, &[nom.as_str()]);

    let fk_soc = fournisseur(&pool, &nom)?;
    let id = facture_de_1200(&pool, fk_soc, "PAYF-SOLDE-1", true)?;

    db::achat_paiement::payer(&pool, ENTITY, id, dec("500"), "VIR", "2026-03-12")?;
    assert_eq!(
        db::achat_paiement::reste_a_payer(&pool, ENTITY, id)?,
        dec("700")
    );
    assert_eq!(
        etat_facture(&pool, id)?.0,
        i64::from(db::achat::STATUT_VALIDEE)
    );

    let second = db::achat_paiement::payer(&pool, ENTITY, id, dec("700"), "CHQ", "2026-04-02")?;
    assert!(second > 0);

    assert_eq!(
        db::achat_paiement::reste_a_payer(&pool, ENTITY, id)?,
        Decimal::ZERO,
        "500 + 700 soldent bien 1 200"
    );
    let (statut, paye) = etat_facture(&pool, id)?;
    assert_eq!(paye, 1, "la facture est soldee");
    assert_eq!(statut, i64::from(db::achat::STATUT_PAYEE));

    let paiements = db::achat_paiement::paiements(&pool, ENTITY, id)?;
    assert_eq!(paiements.len(), 2);
    assert_ne!(paiements[0].ref_, paiements[1].ref_);
    assert_eq!(
        paiements
            .iter()
            .map(|p| p.montant_affecte)
            .fold(Decimal::ZERO, |a, b| a + b),
        dec("1200"),
        "la somme des affectations vaut le TTC"
    );
    assert_eq!(paiements[0].mode.as_deref(), Some("VIR"));
    assert_eq!(paiements[1].mode.as_deref(), Some("CHQ"));

    // Une facture soldee refuse un nouveau paiement.
    let err = db::achat_paiement::payer(&pool, ENTITY, id, dec("1"), "VIR", "2026-04-03")
        .expect_err("une facture soldee n'encaisse plus rien");
    assert!(
        err.to_string().contains("deja soldee"),
        "message inattendu : {err}"
    );

    drop(garde);
    Ok(())
}

#[test]
fn refuse_un_paiement_superieur_au_reste() -> Result<(), Box<dyn Error>> {
    let nom = format!("{PREFIXE}Trop");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = Garde::new(&pool, ENTITY, &[nom.as_str()]);

    let fk_soc = fournisseur(&pool, &nom)?;
    let id = facture_de_1200(&pool, fk_soc, "PAYF-TROP-1", true)?;

    db::achat_paiement::payer(&pool, ENTITY, id, dec("500"), "VIR", "2026-03-12")?;
    assert_eq!(
        db::achat_paiement::reste_a_payer(&pool, ENTITY, id)?,
        dec("700")
    );

    // 800 sur un reste de 700 : refuse, et le message dit les deux montants.
    let err = db::achat_paiement::payer(&pool, ENTITY, id, dec("800"), "VIR", "2026-03-13")
        .expect_err("on ne peut pas payer plus que le reste du");
    let message = err.to_string();
    assert!(
        message.contains("depasse le reste a payer"),
        "message inattendu : {message}"
    );
    assert!(
        message.contains("700"),
        "le reste doit etre cite : {message}"
    );
    assert!(
        message.contains("800"),
        "le montant doit etre cite : {message}"
    );

    // Rien n'a bouge : ni le solde, ni le nombre de paiements.
    assert_eq!(
        db::achat_paiement::reste_a_payer(&pool, ENTITY, id)?,
        dec("700")
    );
    assert_eq!(db::achat_paiement::paiements(&pool, ENTITY, id)?.len(), 1);
    assert_eq!(etat_facture(&pool, id)?.1, 0);

    drop(garde);
    Ok(())
}

#[test]
fn refuse_les_paiements_impossibles() -> Result<(), Box<dyn Error>> {
    let nom = format!("{PREFIXE}Refus");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = Garde::new(&pool, ENTITY, &[nom.as_str()]);

    let fk_soc = fournisseur(&pool, &nom)?;

    // Facture en brouillon : l'argent ne peut pas sortir avant validation.
    let brouillon = facture_de_1200(&pool, fk_soc, "PAYF-REFUS-1", false)?;
    let err = db::achat_paiement::payer(&pool, ENTITY, brouillon, dec("100"), "VIR", "2026-03-12")
        .expect_err("un brouillon n'est pas payable");
    assert!(
        err.to_string().contains("brouillon"),
        "message inattendu : {err}"
    );

    // Facture abandonnee : meme refus, avec son propre message. Aucune API ne
    // pose ce statut, on l'ecrit donc en base, comme le ferait Dolibarr.
    let abandonnee = facture_de_1200(&pool, fk_soc, "PAYF-REFUS-2", true)?;
    {
        let mut conn = pool.get_conn()?;
        conn.exec_drop(
            "UPDATE llx_facture_fourn SET fk_statut = :s WHERE rowid = :id",
            params! { "s" => db::achat::STATUT_ABANDONNEE, "id" => abandonnee },
        )?;
    }
    let err = db::achat_paiement::payer(&pool, ENTITY, abandonnee, dec("100"), "VIR", "2026-03-12")
        .expect_err("une facture abandonnee n'est pas payable");
    assert!(
        err.to_string().contains("abandonnee"),
        "message inattendu : {err}"
    );

    // Les cas qui ne dependent pas du statut de la facture.
    let valide = facture_de_1200(&pool, fk_soc, "PAYF-REFUS-3", true)?;
    for montant in ["0", "-5"] {
        let err =
            db::achat_paiement::payer(&pool, ENTITY, valide, dec(montant), "VIR", "2026-03-12")
                .expect_err("un montant nul ou negatif doit etre refuse");
        assert!(
            err.to_string().contains("superieur a zero"),
            "message inattendu pour {montant} : {err}"
        );
    }

    // Date illisible, moyen inconnu, facture inexistante.
    assert!(
        db::achat_paiement::payer(&pool, ENTITY, valide, dec("10"), "VIR", "12/03/2026").is_err(),
        "une date francaise doit etre refusee"
    );
    assert!(
        db::achat_paiement::payer(&pool, ENTITY, valide, dec("10"), "ZORGLUB", "2026-03-12")
            .is_err(),
        "un mode de paiement inconnu doit etre refuse"
    );
    assert!(
        db::achat_paiement::payer(&pool, ENTITY, 999_999, dec("10"), "VIR", "2026-03-12").is_err(),
        "une facture inexistante doit etre refusee"
    );

    // Aucun de ces refus n'a ecrit de paiement.
    for f in [brouillon, abandonnee, valide] {
        assert!(
            db::achat_paiement::paiements(&pool, ENTITY, f)?.is_empty(),
            "un refus ne doit rien laisser en base"
        );
    }
    assert_eq!(
        db::achat_paiement::reste_a_payer(&pool, ENTITY, valide)?,
        dec("1200")
    );

    drop(garde);
    Ok(())
}

#[test]
fn cloisonne_les_entites() -> Result<(), Box<dyn Error>> {
    let nom = format!("{PREFIXE}Entite");
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = Garde::new(&pool, ENTITY, &[nom.as_str()]);

    // Restes eventuels d'une execution interrompue sur l'entite voisine : elle
    // ne recoit que ce test, le prefixe suffit a la nettoyer.
    {
        let mut conn = pool.get_conn()?;
        conn.exec_drop(
            "DELETE FROM llx_paiementfourn WHERE entity = :entity AND ref LIKE 'PAYF%'",
            params! { "entity" => ENTITY_VOISINE },
        )?;
    }
    let avant = compter_paiements(&pool, ENTITY_VOISINE)?;

    let fk_soc = fournisseur(&pool, &nom)?;
    let id = facture_de_1200(&pool, fk_soc, "PAYF-ENT-1", true)?;

    // Depuis l'entite voisine, la facture est introuvable : on ne paye pas, et
    // on ne lit rien non plus.
    let err = db::achat_paiement::payer(&pool, ENTITY_VOISINE, id, dec("100"), "VIR", "2026-03-12")
        .expect_err("une facture d'une autre entite reste invisible");
    assert!(
        err.to_string().contains("n'existe pas dans l'entite"),
        "message inattendu : {err}"
    );
    assert!(
        db::achat_paiement::reste_a_payer(&pool, ENTITY_VOISINE, id).is_err(),
        "le reste du d'une autre entite est introuvable"
    );
    assert!(
        db::achat_paiement::paiements(&pool, ENTITY_VOISINE, id)?.is_empty(),
        "aucun paiement ne doit apparaitre dans l'entite voisine"
    );
    assert_eq!(
        compter_paiements(&pool, ENTITY_VOISINE)?,
        avant,
        "aucun paiement ne doit avoir ete ecrit dans l'entite voisine"
    );

    // La meme operation, dans la bonne entite, passe.
    db::achat_paiement::payer(&pool, ENTITY, id, dec("100"), "VIR", "2026-03-12")?;
    assert_eq!(
        db::achat_paiement::reste_a_payer(&pool, ENTITY, id)?,
        dec("1100")
    );
    assert_eq!(db::achat_paiement::paiements(&pool, ENTITY, id)?.len(), 1);
    // ... et le paiement porte bien l'entite 900 401.
    let p = db::achat_paiement::paiements(&pool, ENTITY, id)?;
    assert_eq!(ligne_paiement(&pool, p[0].rowid)?.entity, ENTITY);

    drop(garde);
    Ok(())
}
