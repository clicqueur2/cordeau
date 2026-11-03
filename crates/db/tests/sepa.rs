//! Bordereau SEPA : lectures en base reelle, sans ecriture.
//!
//! Ces tests ne creent rien. La base de developpement peut n'avoir aucune
//! facture fournisseur ni aucun mandat : les assertions sont donc ecrites pour
//! rester vraies sur une liste vide, tout en verifiant les invariants qui
//! comptent (tri, signe des montants, cloisonnement par entite).

use std::collections::BTreeSet;
use std::error::Error;

use mysql::prelude::*;
use mysql::Pool;

const ENTITE_PA: i64 = 7;
const ENTITE_2AR: i64 = 1;

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

/// La liste se lit sans erreur et sort dans l'ordre d'echeance, les factures
/// sans echeance a la fin. Un bordereau se paie dans cet ordre-la.
#[test]
fn factures_a_payer_triees_par_echeance() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    for entite in [ENTITE_PA, ENTITE_2AR] {
        let factures = db::sepa::factures_fournisseurs_a_payer(&pool, entite, 500)?;
        let mut precedente: Option<&str> = None;
        let mut sans_echeance_vue = false;
        for f in &factures {
            assert!(!f.ref_.is_empty(), "facture sans reference : {}", f.id);
            match f.date_echeance.as_deref() {
                Some(d) => {
                    assert!(
                        !sans_echeance_vue,
                        "une facture datee arrive apres une facture sans echeance ({})",
                        f.ref_
                    );
                    if let Some(p) = precedente {
                        assert!(p <= d, "echeances dans le desordre : {p} puis {d}");
                    }
                    precedente = Some(d);
                }
                None => sans_echeance_vue = true,
            }
        }
    }
    Ok(())
}

/// Aucun montant nul ni negatif : une banque refuse une operation a zero euro,
/// et le reste a payer est deja filtre a la lecture (paiements deduits).
#[test]
fn factures_a_payer_montants_strictement_positifs() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    for entite in [ENTITE_PA, ENTITE_2AR] {
        for f in db::sepa::factures_fournisseurs_a_payer(&pool, entite, 500)? {
            assert!(
                f.montant_centimes > 0,
                "montant non payables : {} centimes pour {}",
                f.montant_centimes,
                f.ref_
            );
            // Un IBAN vide doit arriver en None, jamais en chaine blanche.
            if let Some(iban) = &f.iban {
                assert!(!iban.trim().is_empty(), "IBAN blanc sur {}", f.ref_);
            }
        }
    }
    Ok(())
}

/// Cloisonnement : une facture lue pour l'entite 7 appartient vraiment a
/// l'entite 7 en base, et les deux entites ne rendent jamais le meme
/// identifiant.
#[test]
fn factures_a_payer_ne_fuit_pas_hors_de_l_entite() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let mut conn = pool.get_conn()?;
    let pa = db::sepa::factures_fournisseurs_a_payer(&pool, ENTITE_PA, 500)?;
    let deux_ar = db::sepa::factures_fournisseurs_a_payer(&pool, ENTITE_2AR, 500)?;

    for f in &pa {
        let entite: Option<i64> = conn.exec_first(
            "SELECT entity FROM llx_facture_fourn WHERE rowid = ?",
            (f.id,),
        )?;
        assert_eq!(
            entite,
            Some(ENTITE_PA),
            "la facture {} n'appartient pas a l'entite 7",
            f.ref_
        );
    }
    for f in &deux_ar {
        let entite: Option<i64> = conn.exec_first(
            "SELECT entity FROM llx_facture_fourn WHERE rowid = ?",
            (f.id,),
        )?;
        assert_eq!(
            entite,
            Some(ENTITE_2AR),
            "la facture {} n'appartient pas a l'entite 1",
            f.ref_
        );
    }

    let ids_pa: BTreeSet<i64> = pa.iter().map(|f| f.id).collect();
    let ids_deux_ar: BTreeSet<i64> = deux_ar.iter().map(|f| f.id).collect();
    let communes: Vec<&i64> = ids_pa.intersection(&ids_deux_ar).collect();
    assert!(
        communes.is_empty(),
        "identifiants presents dans les deux entites : {communes:?}"
    );
    Ok(())
}

/// Les mandats : reference non vide, et cloisonnement par entite. Un mandat sans
/// reference ne peut pas produire un fichier pain.008 acceptable.
#[test]
fn mandats_actifs_coherents_et_cloisonnes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let mut conn = pool.get_conn()?;
    let pa = db::sepa::mandats_actifs(&pool, ENTITE_PA, 500)?;
    let deux_ar = db::sepa::mandats_actifs(&pool, ENTITE_2AR, 500)?;

    for m in &pa {
        assert!(!m.reference.trim().is_empty(), "mandat sans RUM : {}", m.id);
        let entite: Option<i64> = conn.exec_first(
            "SELECT entity FROM llx_societe_rib WHERE rowid = ?",
            (m.id,),
        )?;
        assert_eq!(entite, Some(ENTITE_PA), "mandat {} hors entite 7", m.id);
    }
    for m in &deux_ar {
        let entite: Option<i64> = conn.exec_first(
            "SELECT entity FROM llx_societe_rib WHERE rowid = ?",
            (m.id,),
        )?;
        assert_eq!(entite, Some(ENTITE_2AR), "mandat {} hors entite 1", m.id);
    }

    let ids_pa: BTreeSet<i64> = pa.iter().map(|m| m.id).collect();
    let ids_deux_ar: BTreeSet<i64> = deux_ar.iter().map(|m| m.id).collect();
    assert!(
        ids_pa.intersection(&ids_deux_ar).next().is_none(),
        "un meme RIB sort pour les deux entites"
    );
    Ok(())
}
