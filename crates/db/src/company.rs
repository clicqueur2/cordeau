//! Identite de la societe emettrice, lue dans `llx_const`.
//!
//! Dolibarr stocke ces informations dans les constantes `MAIN_INFO_*`
//! (voir `htdocs/admin/company.php`). On lit et on ecrit exactement les memes
//! cles, pour qu'une fiche societe saisie dans Dolibarr soit reprise telle
//! quelle par le noyau Rust, et inversement.
//!
//! Deux constantes propres au noyau, prefixees `DOLIBARR_RS_`, completent ce que
//! Dolibarr ne stocke pas globalement (regime de TVA, delai de paiement).

use mysql::prelude::*;
use mysql::{params, Pool};
use std::error::Error;

pub const CLE_NOM: &str = "MAIN_INFO_SOCIETE_NOM";
pub const CLE_FORME: &str = "MAIN_INFO_SOCIETE_FORME_JURIDIQUE";
pub const CLE_ADRESSE: &str = "MAIN_INFO_SOCIETE_ADDRESS";
pub const CLE_CP: &str = "MAIN_INFO_SOCIETE_ZIP";
pub const CLE_VILLE: &str = "MAIN_INFO_SOCIETE_TOWN";
pub const CLE_PAYS: &str = "MAIN_INFO_SOCIETE_COUNTRY";
pub const CLE_SIRET: &str = "MAIN_INFO_SIRET";
pub const CLE_SIREN: &str = "MAIN_INFO_SIREN";
pub const CLE_TVA_INTRA: &str = "MAIN_INFO_TVAINTRA";
pub const CLE_CAPITAL: &str = "MAIN_INFO_CAPITAL";
pub const CLE_RCS: &str = "MAIN_INFO_RCS";
pub const CLE_MAIL: &str = "MAIN_INFO_SOCIETE_MAIL";
pub const CLE_TEL: &str = "MAIN_INFO_SOCIETE_TEL";
pub const CLE_FRANCHISE: &str = "DOLIBARR_RS_FRANCHISE_EN_BASE";
pub const CLE_DELAI: &str = "DOLIBARR_RS_DELAI_PAIEMENT_JOURS";
pub const CLE_IBAN: &str = "DOLIBARR_RS_IBAN";
pub const CLE_BIC: &str = "DOLIBARR_RS_BIC";
pub const CLE_ASSURANCE_ASSUREUR: &str = "DOLIBARR_RS_ASSURANCE_ASSUREUR";
pub const CLE_ASSURANCE_POLICE: &str = "DOLIBARR_RS_ASSURANCE_POLICE";
pub const CLE_ASSURANCE_COUVERTURE: &str = "DOLIBARR_RS_ASSURANCE_COUVERTURE";
/// Texte des conditions generales de vente imprime sur les documents.
pub const CLE_CGV: &str = "MAIN_PDF_TEXT_CGV";
/// TVA exigible sur les encaissements (option du redevable).
pub const CLE_TVA_ENCAISSEMENTS: &str = "DOLIBARR_RS_TVA_ENCAISSEMENTS";
/// Travaux de renovation au taux reduit : l'attestation du preneur est imprimee.
pub const CLE_TVA_TAUX_REDUIT: &str = "DOLIBARR_RS_TVA_TAUX_REDUIT";

#[derive(Debug, Clone, Default)]
pub struct Company {
    pub nom: String,
    pub forme_juridique: Option<String>,
    pub adresse: Option<String>,
    pub code_postal: Option<String>,
    pub ville: Option<String>,
    pub pays: Option<String>,
    pub siret: Option<String>,
    pub siren: Option<String>,
    pub tva_intra: Option<String>,
    pub capital: Option<String>,
    pub rcs: Option<String>,
    pub email: Option<String>,
    pub telephone: Option<String>,
    /// Franchise en base de TVA (art. 293 B du CGI).
    pub franchise_en_base: bool,
    pub delai_paiement_jours: u32,
    /// Assurance professionnelle (artisan) : assureur, n° de police, couverture.
    pub assurance_assureur: Option<String>,
    pub assurance_police: Option<String>,
    pub assurance_couverture: Option<String>,
    /// Coordonnées bancaires, imprimées sur les factures.
    pub iban: Option<String>,
    pub bic: Option<String>,
    /// Conditions générales de vente, imprimées en pied de document.
    pub cgv: Option<String>,
    /// TVA exigible sur les encaissements : la mention figure sur les documents.
    pub tva_sur_encaissements: bool,
    /// Travaux au taux reduit de 10 % : attestation du preneur sur la facture.
    pub tva_taux_reduit: bool,
}

impl Company {
    /// Pays normalise : Dolibarr stocke `FR` ou `id:FR`.
    pub fn code_pays(&self) -> Option<String> {
        self.pays.as_deref().map(|p| {
            p.rsplit(':')
                .next()
                .unwrap_or(p)
                .trim()
                .to_ascii_uppercase()
        })
    }
}

fn non_vide(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn get(pool: &Pool, entity: i64) -> Result<Company, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<(String, String)> = conn.exec(
        "SELECT name, value FROM llx_const \
         WHERE (entity IN (0, :entity) AND name LIKE 'MAIN\\_INFO\\_%') OR \
               (entity IN (0, :entity) AND name LIKE 'DOLIBARR\\_RS\\_%') OR \
               (entity IN (0, :entity) AND name = 'MAIN_PDF_TEXT_CGV')",
        params! { "entity" => entity },
    )?;

    let mut map = std::collections::BTreeMap::new();
    for (k, v) in lignes {
        map.insert(k, v);
    }
    let lire = |cle: &str| non_vide(map.get(cle).cloned());
    let lire_brut = |cle: &str| map.get(cle).cloned();

    Ok(Company {
        nom: lire(CLE_NOM).unwrap_or_default(),
        forme_juridique: lire(CLE_FORME),
        adresse: lire(CLE_ADRESSE),
        code_postal: lire(CLE_CP),
        ville: lire(CLE_VILLE),
        pays: lire(CLE_PAYS),
        siret: lire(CLE_SIRET),
        siren: lire(CLE_SIREN),
        tva_intra: lire(CLE_TVA_INTRA),
        capital: lire(CLE_CAPITAL),
        rcs: lire(CLE_RCS),
        email: lire(CLE_MAIL),
        telephone: lire(CLE_TEL),
        franchise_en_base: lire_brut(CLE_FRANCHISE).as_deref() == Some("1"),
        delai_paiement_jours: lire_brut(CLE_DELAI)
            .and_then(|v| v.parse().ok())
            .unwrap_or(30),
        assurance_assureur: lire(CLE_ASSURANCE_ASSUREUR),
        assurance_police: lire(CLE_ASSURANCE_POLICE),
        assurance_couverture: lire(CLE_ASSURANCE_COUVERTURE),
        iban: lire(CLE_IBAN),
        bic: lire(CLE_BIC),
        cgv: lire(CLE_CGV),
        tva_sur_encaissements: lire_brut(CLE_TVA_ENCAISSEMENTS).as_deref() == Some("1"),
        tva_taux_reduit: lire_brut(CLE_TVA_TAUX_REDUIT).as_deref() == Some("1"),
    })
}

/// Lit une constante globale (`entity = 0`), si elle existe.
pub fn get_const(pool: &Pool, cle: &str) -> Result<Option<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    Ok(conn.exec_first(
        "SELECT value FROM llx_const WHERE name = :name AND entity = 0 LIMIT 1",
        params! { "name" => cle },
    )?)
}

/// Ecrit une constante globale (entity 0), comme le fait l'administration Dolibarr.'
pub fn set_const(pool: &Pool, cle: &str, valeur: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_const (name, entity, value, type, visible) \
         VALUES (:name, 0, :value, 'string', 0) \
         ON DUPLICATE KEY UPDATE value = VALUES(value)",
        params! { "name" => cle, "value" => valeur },
    )?;
    Ok(())
}

pub fn save(pool: &Pool, societe: &Company) -> Result<(), Box<dyn Error>> {
    let mut paires: Vec<(&str, String)> = vec![(CLE_NOM, societe.nom.clone())];
    let mut ajouter = |cle: &'static str, valeur: &Option<String>| {
        // `Some("")` efface explicitement le champ ; `None` ne le touche pas.
        if let Some(v) = valeur.as_deref() {
            paires.push((cle, v.trim().to_string()));
        }
    };
    ajouter(CLE_FORME, &societe.forme_juridique);
    ajouter(CLE_ADRESSE, &societe.adresse);
    ajouter(CLE_CP, &societe.code_postal);
    ajouter(CLE_VILLE, &societe.ville);
    ajouter(CLE_PAYS, &societe.pays);
    ajouter(CLE_SIRET, &societe.siret);
    ajouter(CLE_SIREN, &societe.siren);
    ajouter(CLE_TVA_INTRA, &societe.tva_intra);
    ajouter(CLE_CAPITAL, &societe.capital);
    ajouter(CLE_RCS, &societe.rcs);
    ajouter(CLE_MAIL, &societe.email);
    ajouter(CLE_TEL, &societe.telephone);
    ajouter(CLE_ASSURANCE_ASSUREUR, &societe.assurance_assureur);
    ajouter(CLE_ASSURANCE_POLICE, &societe.assurance_police);
    ajouter(CLE_ASSURANCE_COUVERTURE, &societe.assurance_couverture);
    ajouter(CLE_IBAN, &societe.iban);
    ajouter(CLE_BIC, &societe.bic);
    ajouter(CLE_CGV, &societe.cgv);
    paires.push((
        CLE_TVA_ENCAISSEMENTS,
        if societe.tva_sur_encaissements {
            "1"
        } else {
            "0"
        }
        .to_string(),
    ));
    paires.push((
        CLE_TVA_TAUX_REDUIT,
        if societe.tva_taux_reduit { "1" } else { "0" }.to_string(),
    ));

    for (cle, valeur) in paires {
        set_const(pool, cle, &valeur)?;
    }
    set_const(
        pool,
        CLE_FRANCHISE,
        if societe.franchise_en_base { "1" } else { "0" },
    )?;
    set_const(pool, CLE_DELAI, &societe.delai_paiement_jours.to_string())?;
    Ok(())
}
