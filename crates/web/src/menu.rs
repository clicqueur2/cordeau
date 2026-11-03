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
            e("contacts", "Contacts/Adresses", "/contacts"),
            e("adherents", "Adhérents", "/liste/Adherent"),
            e("dons", "Dons", "/liste/Don"),
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
            e("categories", "Catégories", "/liste/Categorie"),
            e("lots", "Lots et séries", "/liste/ProductBatch"),
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
            e(
                "commandes-fournisseurs",
                "Commandes fournisseurs",
                "/commandes-fournisseurs",
            ),
            e("contrats", "Contrats", "/contrats"),
            e("receptions", "Réceptions", "/liste/Reception"),
            e("expeditions", "Expéditions", "/expeditions"),
            e(
                "interventions",
                "Fiches d'intervention",
                "/liste/Ficheinter",
            ),
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
        cle: "tickets",
        code: "Ticket",
        libelle: "Tickets",
        entrees: &[e("liste", "Tickets", "/tickets")],
    },
    Module {
        cle: "banques",
        code: "Banque",
        libelle: "Banques",
        entrees: &[
            e("comptes", "Liste des comptes", "/banques"),
            e(
                "rapprochement",
                "Rapprochement bancaire",
                "/banques#rapprochement",
            ),
            e("virements", "Virements", "/liste/PaymentByBankTransfer"),
            e("prelevements", "Prélèvements", "/liste/Prelevement"),
            e("prets", "Prêts", "/liste/Loan"),
        ],
    },
    Module {
        cle: "comptabilite",
        code: "Comptabilite",
        libelle: "Comptabilité",
        entrees: &[
            e("journaux", "Synthèse et TVA", "/comptabilite"),
            e(
                "journal-ventes",
                "Journal des ventes",
                "/comptabilite/journal?sens=ventes",
            ),
            e(
                "journal-achats",
                "Journal des achats",
                "/comptabilite/journal?sens=achats",
            ),
            e("grand-livre", "Grand livre", "/comptabilite/grand-livre"),
            e("fec", "Comptabilisation et FEC", "/comptabilite/fec"),
            e("ecritures", "Écritures comptables", "/liste/Accounting"),
            e("immobilisations", "Immobilisations", "/liste/Asset"),
        ],
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
        entrees: &[
            e("evenements", "Événements", "/agenda"),
            e("nouvel-evenement", "Nouvel événement", "/agenda?form=1"),
        ],
    },
    Module {
        cle: "communication",
        code: "Mailing",
        libelle: "Communication",
        entrees: &[e("emailings", "Emailings", "/liste/Mailing")],
    },
    Module {
        cle: "rh",
        code: "HRM",
        libelle: "RH",
        entrees: &[
            e("frais", "Notes de frais", "/liste/ExpenseReport"),
            e("conges", "Congés", "/liste/Holiday"),
            e("salaires", "Salaires", "/liste/Salaries"),
            e("ressources", "Ressources", "/liste/Resource"),
        ],
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
            e("bookmarks", "Signets", "/liste/Bookmark"),
            e("taxes", "Taux de TVA", "/liste/Tax"),
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
#[cfg(test)]
mod tests {
    use super::*;

    /// Tout lien « /liste/<Code> » du menu doit exister dans le registre, et
    /// tout ecran doit avoir un droit : un lien mort ou un ecran ouvert a tous
    /// sont les deux facons de casser la navigation sans s'en apercevoir.
    #[test]
    fn les_listes_du_menu_existent_et_sont_protegees() {
        let mut vues = 0;
        for m in MODULES {
            for e in m.entrees {
                assert!(e.lien.starts_with('/'), "lien invalide : {}", e.lien);
                if let Some(code) = e.lien.strip_prefix("/liste/") {
                    let code = code.split(['?', '#']).next().unwrap_or(code);
                    assert!(
                        crate::pages::liste_simple(code).is_some(),
                        "liste inconnue dans le menu : {code}"
                    );
                    assert!(
                        !db::droit::droits_du_chemin(e.lien).is_empty(),
                        "aucun droit exige pour {code} : l'ecran serait ouvert a tout compte"
                    );
                    vues += 1;
                }
            }
        }
        assert!(vues >= 18, "seulement {vues} listes dans le menu");
    }
}
