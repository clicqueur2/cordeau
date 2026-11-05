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
            e("campagnes", "Campagnes d'e-mailing", "/campagnes"),
            e("mrp", "Besoins et ordres proposés", "/mrp"),
            e("stock", "Stock", "/stock"),
            e("caisse", "Caisse", "/caisse"),
            e("categories", "Catégories", "/liste/Categorie"),
            e("lots", "Lots et séries", "/liste/ProductBatch"),
            e("nouveau-lot", "Nouveau lot", "/liste/ProductBatch/nouveau"),
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
            e("receptions", "Réceptions", "/receptions"),
            e("expeditions", "Expéditions", "/expeditions"),
            e("interventions", "Interventions", "/interventions"),
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
            e("recurrences", "Récurrences", "/recurrences"),
            e("devises", "Devises", "/devises"),
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
            e("sepa", "Virements SEPA", "/achats/sepa"),
            e(
                "rapprochement",
                "Rapprochement bancaire",
                "/banques#rapprochement",
            ),
            e(
                "rapprochement-bancaire",
                "Pointer un encaissement",
                "/banques/rapprochement",
            ),
            e("virements", "Virements", "/liste/PaymentByBankTransfer"),
            e("prelevements", "Prélèvements", "/liste/Prelevement"),
            e("prets", "Prêts", "/liste/Loan"),
            e("nouveau-pret", "Nouveau prêt", "/liste/Loan/nouveau"),
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
            e("lettrage", "Lettrage", "/comptabilite/lettrage"),
            e("statistiques", "Statistiques", "/statistiques"),
            e("echeancier", "Échéancier", "/echeancier"),
            e("fec", "Comptabilisation et FEC", "/comptabilite/fec"),
            e("cloture", "Clôture d'exercice", "/comptabilite/cloture"),
            e(
                "analytique",
                "Comptabilité analytique",
                "/comptabilite/analytique",
            ),
            e("ecritures", "Écritures comptables", "/liste/Accounting"),
            e("immobilisations", "Immobilisations", "/liste/Asset"),
            e(
                "nouvelle-immobilisation",
                "Nouvelle immobilisation",
                "/liste/Asset/nouveau",
            ),
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
            e("frais", "Notes de frais", "/notes-frais"),
            e("conges", "Congés", "/conges"),
            e("prets", "Prêts", "/prets"),
            e("conges", "Congés", "/liste/Holiday"),
            e("salaires", "Salaires", "/liste/Salaries"),
            e("ressources", "Ressources", "/liste/Resource"),
            e(
                "nouvelle-ressource",
                "Nouvelle ressource",
                "/liste/Resource/nouveau",
            ),
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
            e("nouveau-taux", "Nouveau taux", "/liste/Tax/nouveau"),
            e("base", "Base de données", "/parametres/base"),
            e("constantes", "Constantes", "/parametres/constantes"),
            e("modeles-pdf", "Modèles de PDF", "/parametres/modeles-pdf"),
            e(
                "planification",
                "Planification",
                "/parametres/planification",
            ),
            e("tables", "Tables de la base", "/parametres/tables"),
            e(
                "evenements",
                "Journal des événements",
                "/parametres/evenements",
            ),
            e("extrafields", "Champs libres", "/parametres/extrafields"),
            e("categories", "Catégories", "/categories"),
            e(
                "modeles-emails",
                "Modèles d'e-mail",
                "/parametres/modeles-emails",
            ),
            e("greffons", "Greffons", "/parametres/greffons"),
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

/// Modules ajoutes par les fichiers declaratifs.
///
/// Chacun est construit une fois puis « fuit » : le registre ne rend que des
/// references de duree de vie `'static`, comme les modules du noyau.
static DECLARES: std::sync::RwLock<Vec<&'static Module>> = std::sync::RwLock::new(Vec::new());

/// Ajoute un module declaratif a la barre.
pub fn enregistrer(module: Module) {
    let statique: &'static Module = Box::leak(Box::new(module));
    let mut v = DECLARES.write().unwrap_or_else(|e| e.into_inner());
    if !v.iter().any(|m| m.cle == statique.cle) {
        v.push(statique);
    }
}

/// Vrai quand la barre vient de `llx_menu` et non de nos constantes.
///
/// Dolibarr ne code pas sa barre dans le PHP : il la lit dans `llx_menu`. Quand
/// cette table est peuplee, c'est **elle** qui fait foi — sinon on afficherait
/// notre disposition a cote de la sienne, et plus rien ne serait « au meme
/// endroit ». Tant qu'elle est vide, nos menus restent en repli.
static DEPUIS_BASE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Remplace la barre du noyau par celle lue en base.
pub fn basculer_sur_base() {
    DEPUIS_BASE.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// La barre vient-elle de la base ?
pub fn depuis_base() -> bool {
    DEPUIS_BASE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Entites deja chargees : on ne relit `llx_menu` qu'une fois par entite.
static CHARGEES: std::sync::Mutex<Vec<i64>> = std::sync::Mutex::new(Vec::new());

/// Charge la barre depuis `llx_menu` pour une entite, une seule fois.
///
/// Dolibarr ne code pas sa barre dans le PHP : il la lit dans cette table.
/// Quand elle est peuplee, c'est elle qui fait foi. L'entite n'est connue
/// qu'a la premiere requete d'une session — d'ou ce chargement paresseux.
///
/// Rend le nombre de menus de tete lus, 0 si la table est vide.
pub fn charger_depuis_base(pool: &db::Pool, entity: i64) -> usize {
    {
        let deja = CHARGEES.lock().unwrap_or_else(|e| e.into_inner());
        if deja.contains(&entity) {
            return 0;
        }
    }

    let entrees = match db::menu::lire(pool, entity) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("lecture de llx_menu impossible (entite {entity}) : {e}");
            return 0;
        }
    };
    if entrees.is_empty() {
        return 0;
    }

    let arbre = db::menu::arbre(&entrees);
    let n = arbre.len();
    {
        let mut v = DECLARES.write().unwrap_or_else(|e| e.into_inner());
        v.clear();
        for (tete, enfants) in arbre {
            let mut liste: Vec<Entree> = Vec::new();
            for e in enfants {
                liste.push(Entree {
                    cle: fuite(&e.titre),
                    libelle: fuite(&e.titre),
                    lien: if e.url.is_empty() { "/" } else { fuite(&e.url) },
                });
            }
            v.push(Box::leak(Box::new(Module {
                cle: fuite(&tete.titre),
                libelle: fuite(&tete.titre),
                code: fuite(&tete.mainmenu),
                entrees: Box::leak(liste.into_boxed_slice()),
            })));
        }
    }
    basculer_sur_base();
    CHARGEES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(entity);
    n
}

/// Rend une chaine vivante aussi longtemps que le serveur : la barre est
/// construite en `&'static str`, comme les modules du noyau.
fn fuite(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// Tous les modules : ceux du noyau, puis ceux des fichiers.
pub fn modules() -> Vec<&'static Module> {
    let declares = DECLARES.read().unwrap_or_else(|e| e.into_inner());
    MODULES.iter().chain(declares.iter().copied()).collect()
}

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
    modules()
        .into_iter()
        .find(|m| m.cle == cle)
        .unwrap_or(&MODULES[0])
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
        for m in modules() {
            for e in m.entrees {
                assert!(e.lien.starts_with('/'), "lien invalide : {}", e.lien);
                if let Some(reste) = e.lien.strip_prefix("/liste/") {
                    // « /liste/Loan/nouveau » : la liste est le premier segment.
                    let code = reste
                        .split(['?', '#'])
                        .next()
                        .unwrap_or(reste)
                        .split('/')
                        .next()
                        .unwrap_or(reste);
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

    /// Chaque formulaire de saisie doit viser une liste existante, une table
    /// reelle et des colonnes nommees comme du SQL. Un registre incoherent
    /// donnerait une page morte ou une erreur MySQL a l'enregistrement.
    #[test]
    fn les_formulaires_de_saisie_sont_coherents() {
        let identifiant =
            |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        for s in crate::pages::SAISIES {
            assert!(
                crate::pages::liste_simple(s.code).is_some(),
                "formulaire sans liste : {}",
                s.code
            );
            assert!(
                !db::droit::droits_du_chemin(&format!("/liste/{}/nouveau", s.code)).is_empty(),
                "formulaire sans droit : {}",
                s.code
            );
            assert!(identifiant(s.table), "table invalide : {}", s.table);
            assert!(!s.champs.is_empty(), "formulaire vide : {}", s.code);
            for ch in s.champs {
                assert!(identifiant(ch.nom), "colonne invalide : {}", ch.nom);
            }
        }
    }

    /// Un nombre se saisit en francais : « 1 500,50 » doit arriver en base
    /// comme 1500.50, pas comme zero ni comme une chaine refusee.
    #[test]
    fn les_nombres_se_lisent_en_francais() {
        use crate::pages::Sorte;
        assert_eq!(
            Sorte::Nombre.normaliser("1 500,50").as_deref(),
            Some("1500.50")
        );
        assert_eq!(Sorte::Nombre.normaliser(" 12,5 ").as_deref(), Some("12.5"));
        assert_eq!(Sorte::Entier.normaliser("12").as_deref(), Some("12"));
        assert_eq!(Sorte::Nombre.normaliser("   "), None);
        assert_eq!(
            Sorte::Date.normaliser("2026-01-31").as_deref(),
            Some("2026-01-31")
        );
    }
}
