//! Presets de base : un socle coherent pour un metier.
//!
//! Dolibarr s'installe nu : quatre-vingt-quatorze modules, tous eteints sauf
//! trois. Celui qui monte un fonds de commerce passe une heure a cocher des
//! cases dont il ne sait pas toujours ce qu'elles font.
//!
//! Un preset repond a ca : une **liste de modules a activer**, nommee par un
//! metier. Rien de plus.
//!
//! # Trois regles
//!
//! 1. **Un preset active, il ne desactive jamais.** Cocher « BTP » sur une base
//!    ou la paie est deja en service ne doit pas eteindre la paie : le preset
//!    ajoute un socle, il ne reprend pas ce qu'on a fait.
//!
//! 2. **Un preset n'ecrit aucune donnee metier.** Pas de client d'exemple, pas
//!    de produit fictif, pas de tarif. Une base pre-remplie de faux est une base
//!    qu'on ne nettoie jamais, et un devis parti avec un faux client ne se
//!    rattrape pas.
//!
//! 3. **Un preset dit ce qu'il ignore.** Un module demande mais inconnu du
//!    catalogue n'est pas active en silence : il ressort dans le rapport, avec
//!    son nom. Une case qui ne fait rien sans le dire est pire qu'une case
//!    absente.
//!
//! Le schema reste identique a l'octet : c'est la **selection** qui change, et
//! elle se defait module par module.

use std::error::Error;

use mysql::Pool;

use crate::module;

/// Un socle de modules, nomme par un metier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    /// Code court, stable : il sert dans les URL et les tests.
    pub code: &'static str,
    pub libelle: &'static str,
    /// A qui ce socle s'adresse, en une phrase.
    pub description: &'static str,
    /// Les codes de modules a activer, **tels que Dolibarr les nomme**.
    pub modules: &'static [&'static str],
}

/// Les presets proposes.
///
/// Les codes sont ceux des constantes `MAIN_MODULE_*` : `SOCIETE`, `PROPALE`
/// (et non `PROPAL`), `FACTURE`, `COMMANDE`, `FOURNISSEUR`, `STOCK`, `PROJET`,
/// `BANQUE`, `COMPTABILITE`, `TICKET`, `CONTRAT`, `EXPEDITION`, `ECM`,
/// `AGENDA`, `USER`, `IMPORT`, `EXPORT`, `CRON`.
///
/// Un code absent du catalogue n'est pas une faute de frappe toleree : il
/// ressort dans le rapport. C'est ce qui permet d'ecrire un preset pour un
/// module pas encore porte sans casser l'application — et de le voir.
pub const PRESETS: &[Preset] = &[
    Preset {
        code: "nue",
        libelle: "Base nue",
        description: "Le minimum vital : des tiers, des produits, des devis, des factures.",
        modules: &["SOCIETE", "PRODUCT", "PROPALE", "FACTURE", "USER", "AGENDA"],
    },
    Preset {
        code: "btp",
        libelle: "BTP et maconnerie",
        description: "Chantiers, achats, stock de chantier, situations et suivi du temps.",
        modules: &[
            "SOCIETE",
            "PRODUCT",
            "PROPALE",
            "COMMANDE",
            "FACTURE",
            "FOURNISSEUR",
            "SUPPLIERINVOICE",
            "STOCK",
            "PROJET",
            "EXPEDITION",
            "BANQUE",
            "COMPTABILITE",
            "AGENDA",
            "ECM",
            "USER",
        ],
    },
    Preset {
        code: "logistique",
        libelle: "Logistique et negoce",
        description: "Stock, expeditions, fournisseurs et facturation d'achat.",
        modules: &[
            "SOCIETE",
            "PRODUCT",
            "PROPALE",
            "COMMANDE",
            "FACTURE",
            "FOURNISSEUR",
            "SUPPLIERINVOICE",
            "STOCK",
            "EXPEDITION",
            "BANQUE",
            "COMPTABILITE",
            "USER",
        ],
    },
    Preset {
        code: "commerce",
        libelle: "Commerce et detail",
        description: "Vente au comptoir, clients, encaissement et suivi.",
        modules: &[
            "SOCIETE",
            "PRODUCT",
            "PROPALE",
            "FACTURE",
            "BANQUE",
            "COMPTABILITE",
            "TICKET",
            "USER",
            "AGENDA",
        ],
    },
    Preset {
        code: "bureau-etudes",
        libelle: "Bureau d'etudes",
        description: "Projets, devis, facturation au temps passe et documents.",
        modules: &[
            "SOCIETE",
            "PROPALE",
            "FACTURE",
            "PROJET",
            "CONTRAT",
            "AGENDA",
            "ECM",
            "USER",
            "BANQUE",
            "COMPTABILITE",
        ],
    },
    Preset {
        code: "association",
        libelle: "Association",
        description: "Adherents, dons, evenements et comptabilite simple.",
        modules: &[
            "SOCIETE",
            "FACTURE",
            "BANQUE",
            "COMPTABILITE",
            "AGENDA",
            "ECM",
            "USER",
            "PROJET",
        ],
    },
];

/// Ce qu'un preset a fait.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rapport {
    /// Modules effectivement actives par cet appel.
    pub actives: Vec<String>,
    /// Modules deja actifs : rien a faire, et ce n'est pas une erreur.
    pub deja_actifs: Vec<String>,
    /// Codes demandes mais inconnus du catalogue.
    pub inconnus: Vec<String>,
}

impl Rapport {
    /// Vrai si le preset n'a rien change.
    pub fn sans_effet(&self) -> bool {
        self.actives.is_empty()
    }
}

/// Le preset portant ce code, ou `None`.
pub fn preset(code: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.code == code)
}

/// Applique un preset : active les modules qu'il demande, sans jamais
/// desactiver quoi que ce soit.
///
/// # Erreurs
///
/// Code de preset inconnu. Un **module** inconnu n'est pas une erreur : il
/// ressort dans `inconnus`, parce qu'un preset peut citer un module pas encore
/// porte et que refuser tout le socle pour un seul nom serait disproportionne.
pub fn appliquer(pool: &Pool, entity: i64, code: &str) -> Result<Rapport, Box<dyn Error>> {
    let choix = preset(code).ok_or_else(|| format!("preset inconnu : {code}"))?;
    let actifs = module::actifs(pool, entity)?;
    let mut rapport = Rapport::default();

    for demande in choix.modules {
        // Le catalogue compare sans tenir compte de la casse ; `definir_actif`
        // refuserait un code absent, on le teste donc avant d'essayer.
        let connu = module::CATALOGUE
            .iter()
            .any(|m| m.code.eq_ignore_ascii_case(demande));
        if !connu {
            rapport.inconnus.push((*demande).to_string());
            continue;
        }
        if actifs.contains(*demande) {
            rapport.deja_actifs.push((*demande).to_string());
            continue;
        }
        module::definir_actif(pool, entity, demande, true)?;
        rapport.actives.push((*demande).to_string());
    }

    Ok(rapport)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_preset_est_decrit() {
        assert!(!PRESETS.is_empty());
        for p in PRESETS {
            assert!(!p.libelle.trim().is_empty(), "{}", p.code);
            assert!(!p.description.trim().is_empty(), "{}", p.code);
            assert!(
                !p.modules.is_empty(),
                "{} : un socle vide n'est pas un socle",
                p.code
            );
        }
    }

    #[test]
    fn les_codes_sont_uniques() {
        let mut vus = std::collections::BTreeSet::new();
        for p in PRESETS {
            assert!(vus.insert(p.code), "code en double : {}", p.code);
        }
    }

    #[test]
    fn un_preset_se_retrouve_par_son_code() {
        assert_eq!(preset("btp").map(|p| p.libelle), Some("BTP et maconnerie"));
        assert_eq!(preset("licorne"), None);
    }

    /// Le vrai socle commun : sans societe, sans compte et sans facture, un
    /// preset ne produit rien d'utilisable.
    ///
    /// On n'exige pas « base nue » partout : une association n'a pas besoin de
    /// produits, un bureau d'etudes n'en vend pas. Exiger le meme socle pour
    /// tous obligerait a activer des modules que personne n'ouvrira.
    #[test]
    fn le_socle_commun_est_dans_tous_les_presets() {
        for p in PRESETS {
            for m in ["SOCIETE", "USER", "FACTURE"] {
                assert!(
                    p.modules.contains(&m),
                    "{} n'active pas {m}, sans lequel la base ne sert a rien",
                    p.code
                );
            }
        }
    }

    /// Aucun preset ne cite deux fois le meme module : la liste se lit comme
    /// une intention, pas comme un compteur.
    #[test]
    fn aucun_module_n_est_cite_deux_fois() {
        for p in PRESETS {
            let mut vus = std::collections::BTreeSet::new();
            for m in p.modules {
                assert!(vus.insert(*m), "{} cite {m} deux fois", p.code);
            }
        }
    }
}
