//! En-tetes de securite HTTP : ce que le navigateur doit refuser de faire.
//!
//! Un formulaire de facturation sans ces en-tetes fonctionne. Il fonctionne
//! **trop bien** : il accepte d'etre affiche dans un cadre pose par un autre
//! site, il charge un script venu d'ailleurs, il laisse le navigateur dire a
//! Google d'ou vient le visiteur. Rien de tout cela ne se voit a l'ecran.
//!
//! Les noms de constantes sont ceux de Dolibarr (`MAIN_SECURITY_FORCE*`) : une
//! base configuree par l'interface PHP doit produire les memes en-tetes ici, et
//! inversement.
//!
//! # Pourquoi une valeur vide n'est pas un oubli
//!
//! Dolibarr n'active ni CSP ni HSTS par defaut, et c'est deliberе : une CSP
//! trop stricte casse le premier module qui charge une police distante, et un
//! HSTS pose sur un site encore en HTTP rend le site inaccessible pour des
//! mois. Ces deux reglages se saisissent, ils ne se devinent pas.
//!
//! `Referrer-Policy` fait exception : Dolibarr le pose a `same-origin` sans
//! rien demander, parce que la valeur par defaut des navigateurs
//! (`strict-origin-when-cross-origin`) envoie quand meme le domaine du client
//! a chaque lien sortant. Ici aussi.

use std::error::Error;

use mysql::Pool;

/// La politique de securite, telle qu'elle sera envoyee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `Content-Security-Policy`. Vide : aucun en-tete.
    pub csp: String,
    /// `Content-Security-Policy-Report-Only` : signale sans bloquer.
    pub csp_rapport: String,
    /// `Strict-Transport-Security`. Vide : aucun en-tete.
    pub hsts: String,
    /// `Permissions-Policy` : ce que la page a le droit d'utiliser.
    pub permissions: String,
    /// `Referrer-Policy`. `same-origin` par defaut.
    pub referrer: String,
    /// Autoriser les appels d'un autre site (API), avec `Vary: Origin`.
    pub cors: bool,
}

pub const CLE_CSP: &str = "MAIN_SECURITY_FORCECSP";
pub const CLE_CSP_RAPPORT: &str = "MAIN_SECURITY_FORCECSPRO";
pub const CLE_HSTS: &str = "MAIN_SECURITY_FORCESTS";
pub const CLE_PERMISSIONS: &str = "MAIN_SECURITY_FORCEPP";
pub const CLE_REFERRER: &str = "MAIN_SECURITY_FORCERP";
pub const CLE_CORS: &str = "MAIN_SECURITY_FORCE_ACCESS_CONTROL_ALLOW_ORIGIN";

impl Default for Config {
    fn default() -> Self {
        Self {
            csp: String::new(),
            csp_rapport: String::new(),
            hsts: String::new(),
            permissions: String::new(),
            // Le defaut de Dolibarr, et le seul en-tete qu'il pose sans qu'on
            // le lui demande.
            referrer: "same-origin".to_string(),
            cors: false,
        }
    }
}

impl Config {
    /// Vrai si au moins un en-tete sortira au-dela de `nosniff`.
    pub fn rien_de_configure(&self) -> bool {
        self.csp.trim().is_empty()
            && self.csp_rapport.trim().is_empty()
            && self.hsts.trim().is_empty()
            && self.permissions.trim().is_empty()
            && self.referrer.trim().is_empty()
    }
}

/// Lit la configuration dans `llx_const`.
pub fn config(pool: &Pool, entity: i64) -> Result<Config, Box<dyn Error>> {
    let lire = |cle: &str| -> String {
        crate::constantes::lire(pool, entity, cle)
            .ok()
            .flatten()
            .unwrap_or_default()
    };
    let defaut = Config::default();
    let referrer = lire(CLE_REFERRER);

    Ok(Config {
        csp: lire(CLE_CSP).trim().to_string(),
        csp_rapport: lire(CLE_CSP_RAPPORT).trim().to_string(),
        hsts: lire(CLE_HSTS).trim().to_string(),
        permissions: lire(CLE_PERMISSIONS).trim().to_string(),
        // Une constante absente rend la chaine vide : on garde alors le defaut
        // plutot que de desactiver l'en-tete sans que personne ne l'ait decide.
        referrer: if referrer.trim().is_empty() {
            defaut.referrer
        } else {
            referrer.trim().to_string()
        },
        cors: lire(CLE_CORS).trim() == "1",
    })
}

/// Ecrit la configuration.
pub fn enregistrer(pool: &Pool, entity: i64, config: &Config) -> Result<(), Box<dyn Error>> {
    // Une valeur d'en-tete ne contient ni retour a la ligne ni caractere de
    // controle : c'est par la qu'on injecte un second en-tete.
    for (nom, valeur) in [
        (CLE_CSP, &config.csp),
        (CLE_CSP_RAPPORT, &config.csp_rapport),
        (CLE_HSTS, &config.hsts),
        (CLE_PERMISSIONS, &config.permissions),
        (CLE_REFERRER, &config.referrer),
    ] {
        verifier_valeur(nom, valeur)?;
    }

    crate::constantes::ecrire(pool, entity, CLE_CSP, config.csp.trim())?;
    crate::constantes::ecrire(pool, entity, CLE_CSP_RAPPORT, config.csp_rapport.trim())?;
    crate::constantes::ecrire(pool, entity, CLE_HSTS, config.hsts.trim())?;
    crate::constantes::ecrire(pool, entity, CLE_PERMISSIONS, config.permissions.trim())?;
    crate::constantes::ecrire(pool, entity, CLE_REFERRER, config.referrer.trim())?;
    crate::constantes::ecrire(pool, entity, CLE_CORS, if config.cors { "1" } else { "" })?;
    Ok(())
}

/// Refuse une valeur qui ne peut pas voyager dans un en-tete.
///
/// Un retour a la ligne dans `Referrer-Policy` suffit a ajouter un
/// `Set-Cookie` a la reponse. C'est la faille la plus banale de ce fichier, et
/// elle s'ecrit en une phrase.
pub fn verifier_valeur(nom: &str, valeur: &str) -> Result<(), Box<dyn Error>> {
    if valeur
        .chars()
        .any(|c| c == '\r' || c == '\n' || (c.is_control() && c != '\t'))
    {
        return Err(format!("{nom} : un en-tete ne peut pas contenir de retour a la ligne").into());
    }
    Ok(())
}

/// Les en-tetes a poser, dans l'ordre.
///
/// `nosniff` est toujours la : il empeche le navigateur de deviner un type de
/// contenu, ce qui transforme un fichier depose par un client en script
/// executable. Il ne casse rien, contrairement a une CSP.
pub fn entetes(config: &Config, base: &str) -> Vec<(String, String)> {
    let mut out = vec![("X-Content-Type-Options".to_string(), "nosniff".to_string())];

    let mut poser = |nom: &str, valeur: &str| {
        let v = valeur.trim();
        if !v.is_empty() && verifier_valeur(nom, v).is_ok() {
            out.push((nom.to_string(), v.to_string()));
        }
    };
    poser("Content-Security-Policy", &config.csp);
    poser("Content-Security-Policy-Report-Only", &config.csp_rapport);
    poser("Strict-Transport-Security", &config.hsts);
    poser("Permissions-Policy", &config.permissions);
    poser("Referrer-Policy", &config.referrer);

    if config.cors {
        out.push(("Access-Control-Allow-Origin".to_string(), base.to_string()));
        out.push(("Vary".to_string(), "Origin".to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nosniff_est_toujours_la() {
        let e = entetes(&Config::default(), "https://exemple.fr");
        assert!(e
            .iter()
            .any(|(n, v)| n == "X-Content-Type-Options" && v == "nosniff"));
    }

    #[test]
    fn une_valeur_vide_ne_produit_pas_d_en_tete() {
        let e = entetes(&Config::default(), "https://exemple.fr");
        assert!(!e.iter().any(|(n, _)| n == "Content-Security-Policy"));
        assert!(!e.iter().any(|(n, _)| n == "Strict-Transport-Security"));
    }

    #[test]
    fn le_referrer_par_defaut_est_same_origin() {
        let e = entetes(&Config::default(), "https://exemple.fr");
        assert!(e
            .iter()
            .any(|(n, v)| n == "Referrer-Policy" && v == "same-origin"));
    }

    #[test]
    fn un_retour_a_la_ligne_est_refuse() {
        // La faille la plus banale : un en-tete qui en cache un autre.
        assert!(verifier_valeur("Referrer-Policy", "same-origin\r\nSet-Cookie: a=b").is_err());
        assert!(verifier_valeur("Referrer-Policy", "same-origin\n").is_err());
        // Un espace et des points-virgules sont normaux dans une CSP.
        assert!(verifier_valeur("CSP", "default-src 'self'; img-src * data:").is_ok());
    }

    #[test]
    fn une_configuration_avec_csp_sort_les_bons_en_tetes() {
        let c = Config {
            csp: "default-src 'self'".to_string(),
            hsts: "max-age=31536000".to_string(),
            cors: true,
            ..Config::default()
        };
        let e = entetes(&c, "https://exemple.fr");
        assert!(e
            .iter()
            .any(|(n, v)| n == "Content-Security-Policy" && v == "default-src 'self'"));
        assert!(e
            .iter()
            .any(|(n, v)| n == "Strict-Transport-Security" && v == "max-age=31536000"));
        // CORS n'est jamais wildcard : on renvoie l'adresse de l'instance.
        assert!(e
            .iter()
            .any(|(n, v)| n == "Access-Control-Allow-Origin" && v == "https://exemple.fr"));
        assert!(e.iter().any(|(n, v)| n == "Vary" && v == "Origin"));
    }

    #[test]
    fn une_configuration_vierge_est_reconnue() {
        assert!(
            !Config::default().rien_de_configure(),
            "referrer reste actif"
        );
        let vide = Config {
            referrer: String::new(),
            ..Config::default()
        };
        assert!(vide.rien_de_configure());
    }
}
