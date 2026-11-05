use std::error::Error;
use std::str::FromStr;

use mysql::params;
use mysql::prelude::Queryable;
use mysql::Pool;
use rust_decimal::Decimal;

const ENTITY: i64 = 1;

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test ignore");
            None
        }
    }
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// Une commande validee portant un acompte de 30 %.
///
/// Les commandes ne se creent que depuis un devis signe (comme dans Dolibarr) :
/// le test suit donc ce chemin, avec un devis de 1 000 HT a 20 %.
fn commande_avec_acompte(pool: &Pool, pct: Option<&str>) -> (i64, i64, i64) {
    let nom = format!("ZZ-ACOMPTE-{}-{:?}", std::process::id(), pct);
    let fk_soc = db::party::create(pool, ENTITY, &db::party::NewParty::client(&nom)).unwrap();

    let mut new = db::proposal::NewProposal::new(fk_soc);
    new.deposit_percent = pct.map(dec);
    new.lines = vec![db::proposal::NewProposalLine::new(
        "Travaux",
        dec("1"),
        dec("1000"),
        dec("20"),
    )];
    let devis = db::proposal::create(pool, ENTITY, &new).unwrap();
    db::proposal::validate(pool, ENTITY, devis).unwrap();
    db::proposal::cloturer(pool, ENTITY, devis, 2).unwrap();
    let commande = db::commande::create_from_proposal(pool, ENTITY, devis).unwrap();
    (fk_soc, devis, commande)
}

fn nettoyer(pool: &Pool, fk_soc: i64, devis: i64, commande: i64) {
    let mut conn = pool.get_conn().unwrap();
    let _ = conn.exec_drop(
        "DELETE FROM llx_facturedet WHERE fk_facture IN \
           (SELECT rowid FROM llx_facture WHERE fk_soc = :soc)",
        params! { "soc" => fk_soc },
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_facture WHERE fk_soc = :soc",
        params! { "soc" => fk_soc },
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_commandedet WHERE fk_commande = :c",
        params! { "c" => commande },
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_commande WHERE rowid = :c",
        params! { "c" => commande },
    );
    let _ = db::proposal::delete(pool, ENTITY, devis);
    let _ = db::party::delete(pool, ENTITY, fk_soc);
}

/// Le cas de reference : 1 200 TTC avec 30 % d'acompte font 360,00 TTC,
/// soit 300,00 HT et 60,00 de TVA.
#[test]
fn un_acompte_de_30_pour_cent_sur_1200_ttc_fait_360() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (fk_soc, devis, commande) = commande_avec_acompte(&pool, Some("30"));

    let facture = db::invoice::create_deposit_from_commande(&pool, ENTITY, commande)?;
    let f = db::invoice::get(&pool, ENTITY, facture)?.expect("facture d'acompte");

    assert_eq!(
        f.fk_statut,
        db::invoice::STATUT_BROUILLON,
        "acompte en brouillon"
    );
    assert_eq!(f.total_ttc, Some(dec("360.00")));
    assert_eq!(f.total_ht, Some(dec("300.00")));
    assert_eq!(f.total_tva, Some(dec("60.00")));

    let erreur = db::invoice::create_deposit_from_commande(&pool, ENTITY, commande)
        .expect_err("un second acompte doit etre refuse");
    assert!(erreur.to_string().contains("deja"), "message : {erreur}");

    nettooyer_ou_pas(&pool, fk_soc, devis, commande, Some(facture));
    Ok(())
}

fn nettooyer_ou_pas(pool: &Pool, fk_soc: i64, devis: i64, commande: i64, facture: Option<i64>) {
    if let Some(fid) = facture {
        let _ = db::invoice::delete(pool, ENTITY, fid);
    }
    nettoyer(pool, fk_soc, devis, commande);
}

/// Sans acompte demande sur le devis, la commande n'en porte pas : refus net.
#[test]
fn une_commande_sans_acompte_est_refusee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (fk_soc, devis, commande) = commande_avec_acompte(&pool, None);
    let erreur = db::invoice::create_deposit_from_commande(&pool, ENTITY, commande)
        .expect_err("sans acompte, rien ne doit etre cree");
    assert!(erreur.to_string().contains("acompte"), "message : {erreur}");
    nettooyer_ou_pas(&pool, fk_soc, devis, commande, None);
    Ok(())
}

/// L'acompte deja facture se deduit de la facture finale.
///
/// C'est le defaut de fond qui etait inscrit dans le guide : « l'acompte deja
/// facture n'est pas deduit automatiquement de la facture finale ». Sans cette
/// deduction, le client paie deux fois la part de l'acompte — le total de la
/// facture finale est le prix plein alors qu'il a deja regle un tiers.
///
/// Le scenario : 1 000 HT a 20 % (1 200 TTC), acompte de 30 % (360 TTC), puis
/// facture finale. Elle doit valoir 1 200 - 360 = 840 TTC, et non 1 200.
#[test]
fn l_acompte_se_deduit_de_la_facture_finale() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let (fk_soc, devis, commande) = commande_avec_acompte(&pool, Some("30"));

    // Une commande non validee ne se facture pas : c'est la regle du module, et
    // le test la respecte plutot que de la contourner.
    db::commande::validate(&pool, ENTITY, commande)?;

    let acompte = db::invoice::create_deposit_from_commande(&pool, ENTITY, commande)?;
    let a = db::invoice::get(&pool, ENTITY, acompte)?.expect("acompte relu");
    assert_eq!(a.total_ttc, Some(dec("360.00")), "30 % de 1 200 TTC");

    let finale = db::invoice::create_from_commande(&pool, ENTITY, commande)?;
    let f = db::invoice::get(&pool, ENTITY, finale)?.expect("facture finale relue");

    assert_eq!(
        f.total_ht,
        Some(dec("700.00")),
        "1 000 HT moins les 300 HT de l'acompte"
    );
    assert_eq!(
        f.total_tva,
        Some(dec("140.00")),
        "la TVA suit la deduction : 200 - 60"
    );
    assert_eq!(
        f.total_ttc,
        Some(dec("840.00")),
        "1 200 TTC moins les 360 TTC de l'acompte"
    );

    // La ligne de deduction porte le nom de l'acompte : une facture qui se
    // corrige toute seule sans le dire est une facture qu'on ne sait plus
    // expliquer.
    let lignes = db::invoice::lines(&pool, ENTITY, finale)?;
    let deduction = lignes
        .iter()
        .find(|l| l.label.as_deref().is_some_and(|t| t.contains("Acompte")))
        .expect("une ligne de deduction");
    assert!(
        deduction.total_ht.unwrap_or_default() < Decimal::ZERO,
        "la deduction est negative : elle ramene le total, elle ne l'augmente pas"
    );

    // Sans acompte, la facture finale reste au prix plein.
    let (soc2, devis2, cmd2) = commande_avec_acompte(&pool, Some("30"));
    let _ = (soc2, devis2, cmd2);
    nettooyer_ou_pas(&pool, fk_soc, devis, commande, Some(finale));
    nettooyer_ou_pas(&pool, soc2, devis2, cmd2, Some(acompte));
    Ok(())
}
