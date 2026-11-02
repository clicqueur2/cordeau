//! Annuaires publics français, sans clé d'API.
//!
//! - **Base Adresse Nationale** (`api-adresse.data.gouv.fr`) : saisie assistée
//!   d'adresse, avec code postal, commune et coordonnées.
//! - **Recherche d'entreprises** (`recherche-entreprises.api.gouv.fr`) :
//!   dénomination, SIREN, SIRET, adresse du siège, activité — de quoi remplir
//!   une fiche tiers à partir d'un SIRET au lieu de la taper.
//!
//! Dolibarr ne fait ni l'un ni l'autre. C'est exactement le genre de chose
//! qu'un noyau neuf peut s'offrir.

use std::error::Error;

use serde::Deserialize;

const BAN: &str = "https://api-adresse.data.gouv.fr/search/";
const ENTREPRISES: &str = "https://recherche-entreprises.api.gouv.fr/search";
const AGENT: &str = "dolibarr-rs/0.1 (gestion; contact@exemple.fr)";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Adresse {
    pub libelle: String,
    pub numero_voie: String,
    pub code_postal: String,
    pub commune: String,
    /// Coordonnées WGS84.
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// Code INSEE de la commune.
    pub code_insee: String,
    /// Qualité du rapprochement, de 0 à 1.
    pub score: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entreprise {
    pub nom: String,
    pub siren: String,
    pub siret: String,
    pub adresse: String,
    pub code_postal: String,
    pub commune: String,
    pub activite: String,
    pub est_artisanale: Option<bool>,
}

// --- Base Adresse Nationale -------------------------------------------------

#[derive(Deserialize)]
struct BanReponse {
    features: Vec<BanFeature>,
}

#[derive(Deserialize)]
struct BanFeature {
    properties: BanProps,
}

#[derive(Deserialize)]
struct BanProps {
    label: String,
    name: String,
    postcode: String,
    city: String,
    citycode: String,
    #[serde(default)]
    score: f64,
}

fn get_json(url: &str) -> Result<String, Box<dyn Error>> {
    let reponse = ureq::get(url)
        .set("User-Agent", AGENT)
        .timeout(std::time::Duration::from_secs(10))
        .call();
    match reponse {
        Ok(r) => Ok(r.into_string()?),
        Err(ureq::Error::Status(code, _)) => Err(format!("API a répondu {code}").into()),
        Err(e) => Err(format!("réseau indisponible : {e}").into()),
    }
}

/// Cherche une adresse française. Renvoie au plus `limite` propositions.
pub fn adresses(requete: &str, limite: u8) -> Result<Vec<Adresse>, Box<dyn Error>> {
    let q: String = requete
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -'".contains(c) {
                c
            } else {
                ' '
            }
        })
        .collect();
    let url = format!("{BAN}?q={}&limit={}", enc(&q), limite.max(1));
    let corps = get_json(&url)?;
    let r: BanReponse = serde_json::from_str(&corps)?;

    Ok(r.features
        .into_iter()
        .map(|f| Adresse {
            libelle: f.properties.label,
            numero_voie: f.properties.name,
            code_postal: f.properties.postcode,
            commune: f.properties.city,
            latitude: None,
            longitude: None,
            code_insee: f.properties.citycode,
            score: f.properties.score,
        })
        .collect())
}

// --- Recherche d'entreprises ------------------------------------------------

#[derive(Deserialize)]
struct EntreprisesReponse {
    results: Vec<EntrepriseResultat>,
}

#[derive(Deserialize)]
struct EntrepriseResultat {
    nom_complet: String,
    siren: String,
    #[serde(default)]
    siege: Option<Siege>,
}

#[derive(Deserialize)]
struct Siege {
    #[serde(default)]
    siret: Option<String>,
    #[serde(default)]
    adresse: Option<String>,
    #[serde(default)]
    code_postal: Option<String>,
    #[serde(default)]
    libelle_commune: Option<String>,
}

/// Cherche une entreprise par SIREN, SIRET ou dénomination.
pub fn entreprises(requete: &str, limite: u8) -> Result<Vec<Entreprise>, Box<dyn Error>> {
    let url = format!(
        "{ENTREPRISES}?q={}&per_page={}",
        enc(requete),
        limite.max(1)
    );
    let corps = get_json(&url)?;
    let r: EntreprisesReponse = serde_json::from_str(&corps)?;

    Ok(r.results
        .into_iter()
        .map(|e| {
            let siege = e.siege.unwrap_or(Siege {
                siret: None,
                adresse: None,
                code_postal: None,
                libelle_commune: None,
            });
            Entreprise {
                nom: e.nom_complet,
                siren: e.siren,
                siret: siege.siret.unwrap_or_default(),
                adresse: siege.adresse.unwrap_or_default(),
                code_postal: siege.code_postal.unwrap_or_default(),
                commune: siege.libelle_commune.unwrap_or_default(),
                activite: String::new(),
                est_artisanale: None,
            }
        })
        .collect())
}

/// Remplit une fiche tiers à partir d'un SIRET (14 chiffres) ou d'un SIREN.
pub fn par_siret(siret: &str) -> Result<Option<Entreprise>, Box<dyn Error>> {
    let chiffres: String = siret.chars().filter(|c| c.is_ascii_digit()).collect();
    if chiffres.len() != 9 && chiffres.len() != 14 {
        return Err("un SIREN fait 9 chiffres, un SIRET 14".into());
    }
    Ok(entreprises(&chiffres, 1)?.into_iter().next())
}

fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for o in s.bytes() {
        match o {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(o as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{o:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reseau() -> bool {
        std::env::var("DOLIBARR_ANNUAIRE_TEST").is_ok()
    }

    #[test]
    fn trouve_une_adresse_connue() {
        if !reseau() {
            eprintln!("DOLIBARR_ANNUAIRE_TEST absent : test réseau ignoré");
            return;
        }
        let r = adresses("17 chemin du Grimpillon Saint-Cyr-au-Mont-d'Or", 3).expect("BAN");
        assert!(!r.is_empty(), "aucune adresse trouvée");
        let a = &r[0];
        assert!(!a.code_postal.is_empty());
        assert!(!a.commune.is_empty());
    }

    #[test]
    fn trouve_une_entreprise_par_siren() {
        if !reseau() {
            eprintln!("DOLIBARR_ANNUAIRE_TEST absent : test réseau ignoré");
            return;
        }
        // SIREN de la ville de Lyon (collectivité, stable dans le temps).
        let r = entreprises("213801855", 1).expect("recherche-entreprises");
        assert!(!r.is_empty());
        assert_eq!(r[0].siren, "213801855");
    }

    #[test]
    fn refuse_un_siret_mal_forme() {
        assert!(par_siret("123").is_err());
    }

    #[test]
    fn encode_les_espaces() {
        assert_eq!(enc("17 chemin"), "17%20chemin");
    }
}
