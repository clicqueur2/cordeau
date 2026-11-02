//! Contrat d'extension : ce qu'un module Rust declare pour exister.
//!
//! Un module n'est plus « du code quelque part » : il se declare ici, avec son
//! code Dolibarr, sa version, son ecran, ses entrees de menu et ses constantes
//! d'options. L'activation reste celle de Dolibarr (`MAIN_MODULE_<CODE>` dans
//! `llx_const`), donc la base reste lisible par Dolibarr PHP.
//!
//! Ajouter un module = ajouter une entree dans `REGISTRE`, implementer ses
//! routes, et eventuellement declarer ses options. Rien d'autre n'est a
//! toucher : la page d'options et la page Modules se construisent a partir d'ici.

/// Nature d'une option de module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Genre {
    Texte,
    Nombre,
    OuiNon,
}

/// Une option de module : une constante Dolibarr, son libelle, sa valeur par
/// defaut. C'est exactement ce que Dolibarr affiche sur la page du module.
pub struct OptionModule {
    pub cle: &'static str,
    pub libelle: &'static str,
    pub genre: Genre,
    pub defaut: &'static str,
    /// Aide affichee sous le champ.
    pub aide: &'static str,
}

/// Une entree de menu fournie par le module.
pub struct EntreeModule {
    pub libelle: &'static str,
    pub route: &'static str,
}

/// Declaration d'un module du noyau.
pub struct ModuleRust {
    /// Code Dolibarr : c'est lui qui donne `MAIN_MODULE_<CODE>`.
    pub code: &'static str,
    pub nom: &'static str,
    pub version: &'static str,
    /// Ecran principal, s'il y en a un.
    pub ecran: Option<&'static str>,
    pub entrees: &'static [EntreeModule],
    pub options: &'static [OptionModule],
}

const fn entree(libelle: &'static str, route: &'static str) -> EntreeModule {
    EntreeModule { libelle, route }
}

const fn option(
    cle: &'static str,
    libelle: &'static str,
    genre: Genre,
    defaut: &'static str,
    aide: &'static str,
) -> OptionModule {
    OptionModule {
        cle,
        libelle,
        genre,
        defaut,
        aide,
    }
}

/// Les modules Rust du noyau, dans l'ordre de Dolibarr.
pub const REGISTRE: &[ModuleRust] = &[
    ModuleRust {
        code: "Societe",
        nom: "Tiers",
        version: "24.0.1",
        ecran: Some("/tiers"),
        entrees: &[
            entree("Liste des tiers", "/tiers"),
            entree("Nouveau tiers", "/tiers/nouveau"),
            entree("Prospects", "/tiers?type=prospect"),
            entree("Fournisseurs", "/tiers?type=fournisseur"),
        ],
        options: &[
            option(
                "DOLIBARR_RS_CODE_CLIENT_PREFIXE",
                "Préfixe des codes client",
                Genre::Texte,
                "CU",
                "Deux lettres, suivies de AAMM et d'un compteur.",
            ),
            option(
                "DOLIBARR_RS_TIERS_PAR_PAGE",
                "Tiers par page",
                Genre::Nombre,
                "50",
                "Taille des listes.",
            ),
        ],
    },
    ModuleRust {
        code: "Product",
        nom: "Produits et services",
        version: "24.0.1",
        ecran: Some("/produits"),
        entrees: &[
            entree("Catalogue", "/produits"),
            entree("Nouveau produit", "/produits/nouveau"),
        ],
        options: &[option(
            "DOLIBARR_RS_TVA_DEFAUT",
            "Taux de TVA par défaut",
            Genre::Texte,
            "20",
            "Appliqué aux nouvelles lignes de produit.",
        )],
    },
    ModuleRust {
        code: "Stock",
        nom: "Stocks",
        version: "24.0.1",
        ecran: Some("/stock"),
        entrees: &[entree("Stock", "/stock")],
        options: &[option(
            "DOLIBARR_RS_STOCK_NEGATIF",
            "Autoriser un stock négatif",
            Genre::OuiNon,
            "1",
            "Dolibarr l'autorise : c'est un signal d'erreur de saisie, pas un blocage.",
        )],
    },
    ModuleRust {
        code: "Propale",
        nom: "Propositions commerciales",
        version: "24.0.1",
        ecran: Some("/devis"),
        entrees: &[
            entree("Liste des propositions", "/devis"),
            entree("Nouvelle proposition", "/devis/nouveau"),
            entree("Contrats", "/contrats"),
            entree("Expéditions", "/expeditions"),
        ],
        options: &[
            option(
                "DOLIBARR_RS_DEVIS_VALIDITE_JOURS",
                "Validité par défaut (jours)",
                Genre::Nombre,
                "30",
                "Pré-remplit la date de fin de validité.",
            ),
            option(
                "DOLIBARR_RS_DEVIS_ACOMPTE_DEFAUT",
                "Acompte par défaut (%)",
                Genre::Nombre,
                "0",
                "0 = aucun acompte demandé.",
            ),
        ],
    },
    ModuleRust {
        code: "Commande",
        nom: "Commandes clients",
        version: "24.0.1",
        ecran: Some("/commandes"),
        entrees: &[entree("Commandes", "/commandes")],
        options: &[],
    },
    ModuleRust {
        code: "Facture",
        nom: "Factures et avoirs",
        version: "24.0.1",
        ecran: Some("/factures"),
        entrees: &[
            entree("Liste des factures", "/factures"),
            entree("Facture directe", "/factures/nouveau"),
            entree("Avoirs", "/avoirs"),
            entree("Règlements", "/reglements"),
            entree("Relances", "/relances"),
        ],
        options: &[
            option(
                "DOLIBARR_RS_FACTURE_DELAI_JOURS",
                "Délai de paiement par défaut (jours)",
                Genre::Nombre,
                "30",
                "Utilisé quand aucune condition de règlement n'est choisie.",
            ),
            option(
                "DOLIBARR_RS_PDF_MARGE_HAUT",
                "Marge haute du PDF (mm)",
                Genre::Nombre,
                "14",
                "Le gabarit lit cette valeur à chaque rendu.",
            ),
        ],
    },
    ModuleRust {
        code: "Fournisseur",
        nom: "Fournisseurs et achats",
        version: "24.0.1",
        ecran: Some("/achats"),
        entrees: &[
            entree("Fournisseurs", "/achats"),
            entree("Saisir une facture", "/achats/facture"),
        ],
        options: &[],
    },
    ModuleRust {
        code: "Banque",
        nom: "Banques",
        version: "24.0.1",
        ecran: Some("/banques"),
        entrees: &[entree("Comptes et écritures", "/banques")],
        options: &[],
    },
    ModuleRust {
        code: "Comptabilite",
        nom: "Comptabilité",
        version: "24.0.1",
        ecran: Some("/comptabilite"),
        entrees: &[
            entree("Synthèse et TVA", "/comptabilite"),
            entree("Grand livre", "/comptabilite/grand-livre"),
            entree("Comptabilisation et FEC", "/comptabilite/fec"),
        ],
        options: &[option(
            "DOLIBARR_RS_JOURNAL_VENTES",
            "Code journal des ventes",
            Genre::Texte,
            "VE",
            "Utilisé par le grand livre et le FEC.",
        )],
    },
    ModuleRust {
        code: "Projet",
        nom: "Projets et chantiers",
        version: "24.0.1",
        ecran: Some("/chantiers"),
        entrees: &[entree("Chantiers", "/chantiers")],
        options: &[],
    },
    ModuleRust {
        code: "Agenda",
        nom: "Agenda",
        version: "24.0.1",
        ecran: Some("/agenda"),
        entrees: &[entree("Événements", "/agenda")],
        options: &[],
    },
    ModuleRust {
        code: "ECM",
        nom: "Documents (GED)",
        version: "24.0.1",
        ecran: Some("/documents"),
        entrees: &[entree("Documents archivés", "/documents")],
        options: &[],
    },
    ModuleRust {
        code: "User",
        nom: "Utilisateurs et groupes",
        version: "24.0.1",
        ecran: Some("/parametres/utilisateurs"),
        entrees: &[
            entree("Utilisateurs", "/parametres/utilisateurs"),
            entree("Groupes", "/parametres/groupes"),
        ],
        options: &[option(
            "DOLIBARR_RS_PASSWORD_MIN",
            "Longueur minimale du mot de passe",
            Genre::Nombre,
            "10",
            "Refus en dessous.",
        )],
    },
    ModuleRust {
        code: "Import",
        nom: "Import / Export",
        version: "24.0.1",
        ecran: Some("/parametres/import"),
        entrees: &[entree("Import / Export", "/parametres/import")],
        options: &[],
    },
];

/// Le module Rust qui porte ce code Dolibarr.
pub fn par_code(code: &str) -> Option<&'static ModuleRust> {
    REGISTRE.iter().find(|m| m.code.eq_ignore_ascii_case(code))
}
