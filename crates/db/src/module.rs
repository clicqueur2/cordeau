//! Modules / applications, comme Dolibarr : l'activation est une constante
//! `MAIN_MODULE_<CODE>` dans `llx_const`, pas une table maison.
//!
//! Le catalogue reprend les modules du cœur 24.0.1 (numero, nom technique,
//! famille). `porte` indique qu'un ecran existe dans dolibarr-rs ; les autres
//! sont listes et activables, mais leur ecran reste a ecrire.

use std::collections::BTreeSet;
use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Un module du cœur Dolibarr.
pub struct Module {
    /// Numero historique Dolibarr (`$this->numero`), qui fixe l'ordre d'affichage.
    pub numero: i32,
    /// Nom technique : `Facture`, `Banque`, `Projet`...
    pub code: &'static str,
    /// Libelle francais affiche.
    pub nom: &'static str,
    /// Famille Dolibarr : crm, financial, products, projects, srm, hr, interface...
    pub famille: &'static str,
    /// Ecran disponible dans dolibarr-rs.
    pub porte: bool,
}

const fn m(
    numero: i32,
    code: &'static str,
    nom: &'static str,
    famille: &'static str,
    porte: bool,
) -> Module {
    Module {
        numero,
        code,
        nom,
        famille,
        porte,
    }
}

/// Catalogue des modules du cœur, dans l'ordre de Dolibarr.
pub const CATALOGUE: &[Module] = &[
    m(0, "User", "Utilisateurs et groupes", "hr", true),
    m(1, "Societe", "Tiers", "crm", true),
    m(
        10,
        "Comptabilite",
        "Comptabilité (partie double)",
        "financial",
        false,
    ),
    m(20, "Propale", "Propositions commerciales", "crm", true),
    m(22, "Mailing", "Emailings", "interface", false),
    m(25, "Commande", "Commandes clients", "crm", false),
    m(30, "Facture", "Factures et avoirs", "financial", true),
    m(39, "SupplierOrder", "Commandes fournisseurs", "srm", false),
    m(40, "Fournisseur", "Fournisseurs", "srm", true),
    m(41, "SupplierInvoice", "Factures fournisseurs", "srm", true),
    m(42, "Syslog", "Journal système", "base", false),
    m(43, "DebugBar", "DebugBar", "base", false),
    m(50, "Product", "Produits", "products", true),
    m(52, "Stock", "Stocks", "products", false),
    m(53, "Service", "Services", "products", true),
    m(54, "Contrat", "Contrats", "crm", false),
    m(55, "Barcode", "Codes-barres", "technic", false),
    m(
        56,
        "PaymentByBankTransfer",
        "Virements bancaires",
        "financial",
        false,
    ),
    m(57, "Prelevement", "Prélèvements", "financial", false),
    m(58, "ClickToDial", "Clic pour appeler", "interface", false),
    m(60, "Label", "Étiquettes", "technic", false),
    m(70, "Ficheinter", "Fiches d'intervention", "crm", false),
    m(80, "Expedition", "Expéditions", "crm", false),
    m(85, "Banque", "Banques et rapprochement", "financial", true),
    m(105, "MailmanSpip", "Mailman / SPIP", "interface", false),
    m(200, "Ldap", "Annuaire LDAP", "interface", false),
    m(240, "Export", "Export de données", "technic", true),
    m(250, "Import", "Import de données", "technic", true),
    m(310, "Adherent", "Adhérents", "hr", false),
    m(320, "ExternalRss", "Flux RSS externes", "technic", false),
    m(330, "Bookmark", "Signets", "technic", false),
    m(400, "Projet", "Projets et chantiers", "projects", true),
    m(500, "Tax", "Taxes et TVA", "financial", false),
    m(510, "Salaries", "Salaires", "financial", false),
    m(520, "Loan", "Prêts", "financial", false),
    m(600, "Notification", "Notifications", "interface", false),
    m(610, "Variants", "Déclinaisons produit", "products", false),
    m(650, "Bom", "Nomenclatures", "products", false),
    m(660, "Mrp", "Calcul des besoins (MRP)", "products", false),
    m(690, "Workstation", "Postes de charge", "products", false),
    m(700, "Don", "Dons", "financial", false),
    m(701, "StockTransfer", "StockTransfer", "products", false),
    m(750, "Recruitment", "Recrutement", "hr", false),
    m(770, "ExpenseReport", "Notes de frais", "hr", false),
    m(
        1120,
        "SupplierProposal",
        "Demandes de prix fournisseurs",
        "srm",
        false,
    ),
    m(
        1520,
        "DocumentGeneration",
        "Génération de documents",
        "technic",
        false,
    ),
    m(1780, "Categorie", "Catégories", "technic", false),
    m(2000, "Fckeditor", "Éditeur HTML", "technic", false),
    m(2200, "DynamicPrices", "Prix dynamiques", "products", false),
    m(2300, "Cron", "Tâches planifiées", "base", true),
    m(2400, "Agenda", "Agenda et événements", "projects", true),
    m(2430, "BookCal", "Agenda de réservation", "portal", false),
    m(
        2450,
        "EventOrganization",
        "Organisation d'événements",
        "projects",
        false,
    ),
    m(2500, "ECM", "Documents (GED)", "ecm", true),
    m(
        2600,
        "WebServices",
        "Services web (SOAP)",
        "interface",
        false,
    ),
    m(2610, "Api", "API et services web", "interface", false),
    m(2700, "Gravatar", "Gravatar", "interface", false),
    m(2800, "FTP", "Dépôt FTP", "interface", false),
    m(
        2900,
        "GeoIPMaxmind",
        "Géolocalisation IP",
        "interface",
        false,
    ),
    m(3200, "BlockedLog", "Journal inaltérable", "base", false),
    m(
        3300,
        "ModuleBuilder",
        "Générateur de modules",
        "technic",
        false,
    ),
    m(
        3400,
        "SocialNetworks",
        "Réseaux sociaux",
        "interface",
        false,
    ),
    m(4000, "HRM", "Ressources humaines", "hr", false),
    m(
        4100,
        "DataPolicy",
        "Politique de données (RGPD)",
        "technic",
        false,
    ),
    m(4560, "Ai", "IA (assistants)", "technic", false),
    m(6000, "Workflow", "Workflows", "technic", false),
    m(10000, "Website", "Sites web", "portal", false),
    m(11000, "WebPortal", "Portail externe", "portal", false),
    m(20000, "Holiday", "Congés", "hr", false),
    m(30000, "Collab", "Collab", "ecm", false),
    m(39000, "ProductBatch", "Lots et séries", "products", false),
    m(40000, "MultiCurrency", "Multi-devises", "technic", false),
    m(50150, "TakePos", "Point de vente", "portal", false),
    m(50200, "Paypal", "Paypal", "interface", false),
    m(50300, "Stripe", "Stripe", "interface", false),
    m(50310, "Dav", "CalDAV / CardDAV", "interface", false),
    m(
        50320,
        "EmailCollector",
        "Collecteur d'e-mails",
        "interface",
        false,
    ),
    m(50330, "Zapier", "Zapier", "interface", false),
    m(
        50400,
        "Accounting",
        "Comptabilité avancée",
        "financial",
        false,
    ),
    m(51000, "Asset", "Immobilisations", "financial", false),
    m(55000, "OpenSurvey", "Sondages", "portal", false),
    m(56000, "Ticket", "Tickets support", "crm", false),
    m(
        57000,
        "KnowledgeManagement",
        "Base de connaissances",
        "crm",
        false,
    ),
    m(58000, "Partnership", "Partenariats", "crm", false),
    m(59000, "Margin", "Marges", "financial", false),
    m(62000, "Incoterm", "Incoterms", "srm", false),
    m(63000, "Resource", "Ressources", "projects", false),
    m(64000, "Printing", "Impression directe", "interface", false),
    m(66000, "Oauth", "OAuth", "interface", false),
    m(
        67000,
        "ReceiptPrinter",
        "Imprimante de tickets",
        "interface",
        false,
    ),
    m(
        68000,
        "Intracommreport",
        "DEB / Intrastat",
        "financial",
        false,
    ),
    m(68305, "Webhook", "Webhooks", "interface", false),
    m(94160, "Reception", "Réceptions", "srm", false),
    m(141916, "QuickMemo", "Mémo rapide", "technic", false),
];

/// Constante Dolibarr qui porte l'activation : `MAIN_MODULE_FACTURE`.
pub fn cle_constante(module: &Module) -> String {
    format!("MAIN_MODULE_{}", module.code.to_uppercase())
}

/// Codes des modules actives pour l'entite.
pub fn actifs(pool: &Pool, entity: i64) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<(String, String)> = conn.exec(
        "SELECT name, value FROM llx_const \
         WHERE entity = :entity AND name LIKE 'MAIN_MODULE_%'",
        params! { "entity" => entity },
    )?;
    Ok(lignes
        .into_iter()
        .filter(|(_, v)| v == "1" || v.eq_ignore_ascii_case("true"))
        .map(|(n, _)| n.trim_start_matches("MAIN_MODULE_").to_string())
        .collect())
}

/// Active ou desactive un module, en ecrivant la meme constante que Dolibarr.
pub fn definir_actif(
    pool: &Pool,
    entity: i64,
    code: &str,
    actif: bool,
) -> Result<(), Box<dyn Error>> {
    let module = CATALOGUE
        .iter()
        .find(|m| m.code.eq_ignore_ascii_case(code))
        .ok_or_else(|| format!("module inconnu : {code}"))?;
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_const (name, entity, value, type, visible) \
         VALUES (:name, :entity, :value, 'chaine', 1) \
         ON DUPLICATE KEY UPDATE value = VALUES(value)",
        params! {
            "name" => cle_constante(module),
            "entity" => entity,
            "value" => if actif { "1" } else { "0" },
        },
    )?;
    Ok(())
}

/// Active d'emblee les modules dont un ecran existe : une installation neuve
/// doit etre utilisable sans passer par la page de configuration.
pub fn activer_modules_portes(pool: &Pool, entity: i64) -> Result<usize, Box<dyn Error>> {
    let mut n = 0;
    for module in CATALOGUE.iter().filter(|m| m.porte) {
        definir_actif(pool, entity, module.code, true)?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cle_suit_la_convention_dolibarr() {
        let facture = CATALOGUE.iter().find(|m| m.code == "Facture").unwrap();
        assert_eq!(cle_constante(facture), "MAIN_MODULE_FACTURE");
    }

    #[test]
    fn les_numeros_sont_uniques_et_ordonnes() {
        let mut vus = BTreeSet::new();
        for m in CATALOGUE {
            assert!(vus.insert(m.numero), "numero duplique : {}", m.numero);
        }
        assert!(
            CATALOGUE.windows(2).all(|w| w[0].numero < w[1].numero),
            "le catalogue doit suivre l'ordre des numeros Dolibarr"
        );
    }
}
