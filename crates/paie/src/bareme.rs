//! Format de barème : le fichier que le comptable remplit.
//!
//! Le moteur ne connait aucun taux. Il recoit un barème **versionné et daté**,
//! dans un fichier JSON lisible et relisible, pour trois raisons :
//!
//! 1. un taux change une fois par an : la grille de 2026 doit rester lisible en
//!    2027, avec la date à laquelle elle s'appliquait ;
//! 2. un comptable doit pouvoir la **relire** sans ouvrir le code, et la
//!    corriger d'un seul chiffre ;
//! 3. un barème douteux doit être **refusé au chargement**, pas découvert au
//!    moment où un bulletin faux part chez le salarié.
//!
//! # Le format
//!
//! ```json
//! {
//!   "libelle": "Grille 2026 — version du 01/01/2026",
//!   "effet_le": "2026-01-01",
//!   "fictif": true,
//!   "cotisations": [
//!     { "code": "MALADIE", "libelle": "Maladie", "assiette": "brut",
//!       "taux": "0", "plafond": null }
//!   ]
//! }
//! ```
//!
//! `fictif` existe pour une raison précise : tant qu'il vaut `true`, aucun
//! bulletin ne doit servir à payer quelqu'un. Un barème de test qui ne se
//! distingue pas d'un barème réel est un piège.

use std::str::FromStr;

use rust_decimal::Decimal;
use serde_json::Value;

use crate::{Assiette, Bareme, Cotisation, Erreur};

/// Ecrit un barème au format JSON, lisible par un humain.
pub fn vers_json(bareme: &Bareme, fictif: bool) -> String {
    let mut cotisations = Vec::new();
    for c in &bareme.cotisations {
        let plafond = match c.plafond {
            Some(p) => format!("\"{}\"", p),
            None => "null".to_string(),
        };
        cotisations.push(format!(
            "    {{ \"code\": {}, \"libelle\": {}, \"assiette\": {}, \
             \"taux\": \"{}\", \"plafond\": {} }}",
            echappe(&c.code),
            echappe(&c.libelle),
            echappe(c.assiette.code()),
            c.taux,
            plafond
        ));
    }
    format!(
        "{{\n  \"libelle\": {},\n  \"effet_le\": {},\n  \"fictif\": {},\n  \
         \"cotisations\": [\n{}\n  ]\n}}\n",
        echappe(&bareme.libelle),
        echappe(&bareme.effet_le),
        fictif,
        cotisations.join(",\n")
    )
}

/// Relit un barème, en refusant tout ce qui produirait un bulletin faux.
pub fn depuis_json(texte: &str) -> Result<(Bareme, bool), Erreur> {
    let racine: Value =
        serde_json::from_str(texte).map_err(|e| Erreur::Format(format!("JSON invalide : {e}")))?;

    let libelle = chaine(&racine, "libelle")?;
    if libelle.trim().is_empty() {
        return Err(Erreur::Format("libelle vide".into()));
    }
    let effet_le = chaine(&racine, "effet_le")?;
    if !date_valide(&effet_le) {
        return Err(Erreur::Format(format!(
            "effet_le attendu AAAA-MM-JJ, recu {effet_le}"
        )));
    }
    let fictif = racine
        .get("fictif")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let liste = racine
        .get("cotisations")
        .and_then(Value::as_array)
        .ok_or_else(|| Erreur::Format("cotisations absentes ou pas une liste".into()))?;
    if liste.is_empty() {
        return Err(Erreur::BaremeVide);
    }

    let mut cotisations = Vec::new();
    for (i, c) in liste.iter().enumerate() {
        let code = chaine(c, "code")?;
        let assiette = Assiette::depuis_code(&chaine(c, "assiette")?)?;
        let taux = decimal(c, "taux")
            .map_err(|_| Erreur::Format(format!("cotisation {i} : taux illisible")))?;
        if taux.is_sign_negative() {
            return Err(Erreur::TauxInvalide(code));
        }
        let plafond = match c.get("plafond") {
            Some(Value::Null) | None => None,
            Some(v) => {
                let p = Decimal::from_str(v.as_str().unwrap_or(&v.to_string()))
                    .map_err(|_| Erreur::Format(format!("cotisation {i} : plafond illisible")))?;
                if p.is_sign_negative() {
                    return Err(Erreur::TauxInvalide(format!("{code} (plafond)")));
                }
                Some(p)
            }
        };
        cotisations.push(Cotisation {
            code,
            libelle: c
                .get("libelle")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            assiette,
            taux,
            plafond,
        });
    }

    Ok((
        Bareme {
            libelle,
            effet_le,
            cotisations,
        },
        fictif,
    ))
}

fn chaine(v: &Value, cle: &str) -> Result<String, Erreur> {
    v.get(cle)
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .ok_or_else(|| Erreur::Format(format!("champ {cle} manquant")))
}

fn decimal(v: &Value, cle: &str) -> Result<Decimal, Erreur> {
    let brut = match v.get(cle) {
        Some(Value::String(s)) => s.clone(),
        Some(autre) => autre.to_string(),
        None => return Err(Erreur::Format(format!("champ {cle} manquant"))),
    };
    Decimal::from_str(brut.trim()).map_err(|_| Erreur::Format(format!("{cle} illisible")))
}

/// Un barème s'applique à une date : sans date valide, il ne s'applique à rien.
fn date_valide(d: &str) -> bool {
    d.len() == 10
        && d.chars().enumerate().all(|(i, c)| match i {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        })
}

fn echappe(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            autre => out.push(autre),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn un_bareme() -> Bareme {
        Bareme {
            libelle: "Grille de test (fictive)".into(),
            effet_le: "2026-01-01".into(),
            cotisations: vec![
                Cotisation {
                    code: "MALADIE".into(),
                    libelle: "Maladie".into(),
                    assiette: Assiette::Brut,
                    taux: Decimal::from_str("0.75").unwrap(),
                    plafond: None,
                },
                Cotisation {
                    code: "RETRAITE".into(),
                    libelle: "Retraite plafonnée".into(),
                    assiette: Assiette::BrutPlafonne,
                    taux: Decimal::from_str("6.9").unwrap(),
                    plafond: Some(Decimal::from_str("3779").unwrap()),
                },
            ],
        }
    }

    #[test]
    fn un_bareme_ecrit_se_relit_a_l_identique() {
        let avant = un_bareme();
        let texte = vers_json(&avant, true);
        let (apres, fictif) = depuis_json(&texte).unwrap();
        assert_eq!(apres.libelle, avant.libelle);
        assert_eq!(apres.effet_le, avant.effet_le);
        assert_eq!(apres.cotisations.len(), 2);
        assert_eq!(apres.cotisations[0].taux, avant.cotisations[0].taux);
        assert_eq!(apres.cotisations[1].plafond, avant.cotisations[1].plafond);
        assert_eq!(apres.cotisations[1].assiette, Assiette::BrutPlafonne);
        assert!(fictif, "un bareme ecrit comme fictif doit le rester");
    }

    #[test]
    fn le_json_est_lisible_par_un_humain() {
        let texte = vers_json(&un_bareme(), true);
        assert!(texte.contains("\"libelle\": \"Grille de test (fictive)\""));
        assert!(texte.contains("\"assiette\": \"brut_plafonne\""));
        assert!(texte.contains("\"plafond\": \"3779\""));
        assert!(texte.contains("\"fictif\": true"));
    }

    #[test]
    fn un_fichier_illisible_est_refuse() {
        assert!(matches!(
            depuis_json("{ pas du json"),
            Err(Erreur::Format(_))
        ));
        assert!(matches!(
            depuis_json(r#"{"libelle":"x","effet_le":"2026-01-01","cotisations":[]}"#),
            Err(Erreur::BaremeVide)
        ));
    }

    #[test]
    fn une_assiette_inconnue_est_refusee() {
        let texte = r#"{"libelle":"x","effet_le":"2026-01-01","cotisations":[
            {"code":"A","libelle":"A","assiette":"inventee","taux":"1"}]}"#;
        assert!(matches!(
            depuis_json(texte),
            Err(Erreur::AssietteInconnue(_))
        ));
    }

    #[test]
    fn un_taux_negatif_ou_une_date_fausse_sont_refuses() {
        let negatif = r#"{"libelle":"x","effet_le":"2026-01-01","cotisations":[
            {"code":"A","libelle":"A","assiette":"brut","taux":"-1"}]}"#;
        assert!(matches!(depuis_json(negatif), Err(Erreur::TauxInvalide(_))));

        let date = r#"{"libelle":"x","effet_le":"01/01/2026","cotisations":[
            {"code":"A","libelle":"A","assiette":"brut","taux":"1"}]}"#;
        assert!(matches!(depuis_json(date), Err(Erreur::Format(_))));
    }

    #[test]
    fn un_bareme_sans_marque_fictive_est_marque_fictif_par_prudence() {
        // Si le champ manque, on ne suppose pas qu'il est reel : c'est le sens
        // de la valeur par defaut.
        let texte = r#"{"libelle":"x","effet_le":"2026-01-01","cotisations":[
            {"code":"A","libelle":"A","assiette":"brut","taux":"1"}]}"#;
        let (_, fictif) = depuis_json(texte).unwrap();
        assert!(fictif);
    }
}
