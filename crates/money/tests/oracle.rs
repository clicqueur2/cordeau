//! Oracle de verite : confronte `money::line_totals()` a la vraie fonction PHP
//! de Dolibarr `calcul_price_total()`.
//!
//! La fixture `tests/fixtures/oracle.json` est produite par
//! `tools/oracle/generate.php` (sources Dolibarr 24.0.1, PHP 8.3) et commitee :
//! le test n'a donc pas besoin de PHP. Tolerance **zero** : les montants sont
//! compares au centime exact, les prix unitaires a la 6e decimale exacte.
//!
//! Regenerer : `php tools/oracle/generate.php`

use money::{line_totals, LineInput};
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;

fn decimal(s: &Value, champ: &str, contexte: &str) -> Decimal {
    let brut = s
        .as_str()
        .unwrap_or_else(|| panic!("{contexte} : champ `{champ}` n'est pas une chaine ({s})"));
    Decimal::from_str(brut)
        .unwrap_or_else(|e| panic!("{contexte} : `{champ}` = {brut:?} n'est pas un decimal ({e})"))
}

/// Construit l'entree Rust a partir du bloc `entree` de la fixture.
fn ligne(entree: &Value) -> LineInput {
    let qty = entree["qty"].as_str().expect("qty");
    let pu = entree["pu"].as_str().expect("pu");
    let remise = entree["remise_percent"].as_str().expect("remise_percent");
    let tva = entree["tva_tx"].as_str().expect("tva_tx");

    let mut l = LineInput::new(qty, pu, remise, tva);
    if entree["price_base_type"].as_str().expect("price_base_type") == "TTC" {
        l = l.ttc();
    }
    if entree["info_bits"].as_u64().expect("info_bits") & 1 == 1 {
        l = l.not_collected();
    }
    l
}

fn describe(entree: &Value) -> String {
    format!(
        "qty={} pu={} remise={}% tva={}% base={} info_bits={}",
        entree["qty"].as_str().unwrap_or("?"),
        entree["pu"].as_str().unwrap_or("?"),
        entree["remise_percent"].as_str().unwrap_or("?"),
        entree["tva_tx"].as_str().unwrap_or("?"),
        entree["price_base_type"].as_str().unwrap_or("?"),
        entree["info_bits"].as_u64().unwrap_or(0),
    )
}

#[test]
fn line_totals_est_identique_au_centime_a_calcul_price_total() {
    // Par defaut la fixture commitee. `ORACLE_FIXTURE` permet de rejouer une
    // campagne de stress ponctuelle sans toucher au depot, par exemple :
    //   php tools/oracle/generate.php --mode=aleatoire --count=5000 --out=stress.json
    //   $env:ORACLE_FIXTURE="stress.json"; cargo test -p money --test oracle
    let brut: String = match std::env::var("ORACLE_FIXTURE") {
        Ok(chemin) => std::fs::read_to_string(&chemin)
            .unwrap_or_else(|e| panic!("ORACLE_FIXTURE={chemin} illisible : {e}")),
        Err(_) => include_str!("fixtures/oracle.json").to_string(),
    };
    let fixture: Value = serde_json::from_str(&brut).expect("fixture oracle.json invalide");
    let cas = fixture["cas"].as_array().expect("tableau `cas`");
    let annonces = fixture["oracle"]["nb_cas"].as_u64().expect("oracle.nb_cas") as usize;
    assert_eq!(
        cas.len(),
        annonces,
        "la fixture annonce {annonces} cas mais en contient {}",
        cas.len()
    );

    let mut compares = 0usize;
    let mut exclus = 0usize;
    let mut ecarts: Vec<String> = Vec::new();

    for (index, c) in cas.iter().enumerate() {
        let entree = &c["entree"];
        let attendu = &c["attendu"];
        let contexte = format!("cas #{index} ({})", describe(entree));

        if let Some(raison) = c["exclu"].as_str() {
            // Cas documente comme hors du sous-ensemble implemente : jamais
            // compare, mais jamais silencieux non plus (le test verifie qu'il
            // y a bien une raison ecrite).
            assert!(
                !raison.trim().is_empty(),
                "{contexte} : `exclu` sans justification"
            );
            exclus += 1;
            continue;
        }

        let calcule = line_totals(&ligne(entree));
        compares += 1;

        // Les totaux sont arrondis a 2 decimales (price2num 'MT'), le prix
        // unitaire a 6 (price2num 'MU') : on compare a l'identique, sans
        // tolerance.
        let paires: [(&str, Decimal, Decimal); 4] = [
            (
                "total_ht",
                decimal(&attendu["total_ht"], "total_ht", &contexte),
                calcule.total_ht,
            ),
            (
                "total_tva",
                decimal(&attendu["total_tva"], "total_tva", &contexte),
                calcule.total_vat,
            ),
            (
                "total_ttc",
                decimal(&attendu["total_ttc"], "total_ttc", &contexte),
                calcule.total_ttc,
            ),
            (
                "pu_ht",
                decimal(&attendu["pu_ht"], "pu_ht", &contexte),
                calcule.unit_price_ht,
            ),
        ];

        for (champ, php, rust) in paires {
            if php != rust {
                ecarts.push(format!(
                    "{contexte} : {champ} PHP={php} Rust={rust} (ecart {})",
                    rust - php
                ));
            }
        }

        // Coherence interne : sur une ligne normale, HT + TVA doit redonner TTC.
        // (Dolibarr ne la garantit pas pour `info_bits & 1`, cf. lib.rs.)
        if entree["info_bits"].as_u64().unwrap_or(0) & 1 == 0 {
            assert_eq!(
                calcule.total_ht + calcule.total_vat,
                calcule.total_ttc,
                "{contexte} : HT + TVA != TTC"
            );
        }
    }

    assert!(compares > 0, "aucun cas compare");
    assert!(
        ecarts.is_empty(),
        "{} cas divergent de calcul_price_total() sur {} compares :\n{}",
        ecarts.len(),
        compares,
        ecarts.join("\n")
    );

    eprintln!("oracle : {compares} cas concordants au centime, {exclus} exclus");
}
