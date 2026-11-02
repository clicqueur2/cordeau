//! Arborescence de navigation, sur le decoupage de Dolibarr : modules en haut,
//! entrees du module a gauche.
//!
//! Les entrees non encore portees restent visibles, marquees « a venir » :
//! cacher un ecran donne l'impression que la fonction n'existe pas, alors
//! qu'elle existe dans Dolibarr et qu'elle doit arriver ici.

/// Une entree de menu.
pub struct Entree {
    /// Cle d'onglet utilisee par `ui::page` pour marquer l'entree active.
    pub cle: &'static str,
    pub libelle: &'static str,
    /// Ecran correspondant. Il n'y a plus d'entree sans ecran.
    pub lien: &'static str,
}

/// Un module (barre du haut) et ses entrees (colonne de gauche).
pub struct Module {
    pub cle: &'static str,
    pub libelle: &'static str,
    /// Code du module Dolibarr qui commande son affichage (`MainModule`).
    pub code: &'static str,
    pub entrees: &'static [Entree],
}

const fn e(cle: &'static str, libelle: &'static str, lien: &'static str) -> Entree {
    Entree { cle, libelle, lien }
}

pub const MODULES: &[Module] = &[
    Module {
        cle: "accueil",
        code: "User",
        libelle: "Accueil",
        entrees: &[
            e("accueil", "Mon tableau de bord", "/"),
            e("recherche", "Recherche", "/recherche"),
        ],
    },
    Module {
        cle: "tiers",
        code: "Societe",
        libelle: "Tiers",
        entrees: &[
            e("nouveau", "Nouveau tiers", "/tiers/nouveau"),
            e("liste", "Liste", "/tiers"),
            e("clients", "Clients", "/tiers?type=client"),
            e("prospects", "Prospects", "/tiers?type=prospect"),
            e("fournisseurs", "Fournisseurs", "/tiers?type=fournisseur"),
        ],
    },
    Module {
        cle: "produits",
        code: "Product",
        libelle: "Produits / Services",
        entrees: &[
            e("nouveau", "Nouveau produit", "/produits/nouveau"),
            e("liste", "Catalogue", "/produits"),
            e("stock", "Stock", "/stock"),
        ],
    },
    Module {
        cle: "chantiers",
        code: "Projet",
        libelle: "Chantiers",
        entrees: &[e("liste", "Chantiers et tâches", "/chantiers")],
    },
    Module {
        cle: "devis",
        code: "Propale",
        libelle: "Commerce",
        entrees: &[
            e("nouveau", "Nouvelle proposition", "/devis/nouveau"),
            e("liste", "Liste des propositions", "/devis"),
            e("commandes", "Commandes", "/commandes"),
            e("contrats", "Contrats", "/contrats"),
            e("expeditions", "Expéditions", "/expeditions"),
        ],
    },
    Module {
        cle: "achats",
        code: "Fournisseur",
        libelle: "Achats",
        entrees: &[e("fournisseurs", "Fournisseurs et factures", "/achats")],
    },
    Module {
        cle: "factures",
        code: "Facture",
        libelle: "Facturation",
        entrees: &[
            e("nouveau", "Facture directe", "/factures/nouveau"),
            e("liste", "Liste des factures", "/factures"),
            e("relances", "Relances et impayés", "/relances"),
            e("paiements", "Saisie des règlements", "/reglements"),
            e("avoirs", "Avoirs", "/avoirs"),
        ],
    },
    Module {
        cle: "banques",
        code: "Banque",
        libelle: "Banques",
        entrees: &[
            e("comptes", "Comptes et écritures", "/banques"),
            e("rapprochement", "Rapprochement bancaire", "/banques"),
        ],
    },
    Module {
        cle: "comptabilite",
        code: "Comptabilite",
        libelle: "Comptabilité",
        entrees: &[e("journaux", "Journaux et TVA", "/comptabilite")],
    },
    Module {
        cle: "documents",
        code: "ECM",
        libelle: "Documents",
        entrees: &[e("ged", "Documents archivés", "/documents")],
    },
    Module {
        cle: "agenda",
        code: "Agenda",
        libelle: "Agenda",
        entrees: &[e("evenements", "Événements", "/agenda")],
    },
    Module {
        cle: "outils",
        code: "User",
        libelle: "Outils",
        entrees: &[
            e("societe", "Société ou organisation", "/parametres"),
            e(
                "utilisateurs",
                "Utilisateurs et groupes",
                "/parametres/utilisateurs",
            ),
            e(
                "dictionnaires",
                "Dictionnaires",
                "/parametres/dictionnaires",
            ),
            e("import", "Import / Export", "/parametres/import"),
            e("sauvegarde", "Sauvegarde", "/parametres/sauvegarde"),
            e("base", "Base de données", "/parametres/base"),
            e("modules", "Modules / Applications", "/parametres/modules"),
            e("affichage", "Affichage", "/parametres/reglage/affichage"),
            e("menus", "Menus", "/parametres/reglage/menus"),
            e("traduction", "Traduction", "/parametres/reglage/traduction"),
            e(
                "valeurs",
                "Valeurs, filtres et tris",
                "/parametres/reglage/valeurs",
            ),
            e("widgets", "Widgets", "/parametres/reglage/widgets"),
            e("alertes", "Alertes", "/parametres/reglage/alertes"),
            e("securite", "Sécurité", "/parametres/reglage/securite"),
            e(
                "limites",
                "Limites et précision",
                "/parametres/reglage/limites",
            ),
            e("pdf", "Modèles de PDF", "/parametres/reglage/pdf"),
            e(
                "emails",
                "E-mails et notifications",
                "/parametres/reglage/emails",
            ),
            e("sms", "SMS", "/parametres/reglage/sms"),
            e("journal", "Journal des erreurs", "/parametres/journal"),
            e("divers", "Divers", "/parametres/reglage/divers"),
        ],
    },
];

/// Module contenant la section demandee.
///
/// Plusieurs ecrans partagent un module : les relances vivent dans
/// Facturation, les parametres dans Outils.
pub fn module_de(section: &str) -> &'static Module {
    let cle = match section {
        "relances" => "factures",
        "parametres" => "outils",
        autre => autre,
    };
    MODULES.iter().find(|m| m.cle == cle).unwrap_or(&MODULES[0])
}
