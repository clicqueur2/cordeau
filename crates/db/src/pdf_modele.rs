//! Modeles de PDF : lequel chaque type de document utilise.
//!
//! Dolibarr garde ce choix dans des constantes, une par type de piece :
//!
//! | Objet | Constante Dolibarr |
//! |---|---|
//! | Facture | `FACTURE_ADDON_PDF` |
//! | Commande | `COMMANDE_ADDON_PDF` |
//! | Devis | `PROPALE_ADDON_PDF` |
//!
//! Les noms sont ceux du cceur, releves dans `htdocs/admin/pdf.php` — y compris
//! `PROPALE` avec son `E`, qui est l'orthographe historique. Les reprendre tels
//! quels a un interet concret : **Dolibarr lit les memes constantes**, donc un
//! choix fait ici vaut aussi pour lui, et l'inverse.
//!
//! Notre ancienne constante unique `DOLIBARR_RS_PDF_MODELE` reste lue en dernier
//! recours : elle n'est plus le reglage normal, mais une installation faite avant
//! ce jour ne doit pas changer de tete de document sans qu'on l'ait demande.

use std::error::Error;

use mysql::Pool;

/// Un type de piece qui porte un modele de PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Objet {
    Devis,
    Commande,
    Facture,
}

impl Objet {
    pub const TOUS: [Objet; 3] = [Objet::Devis, Objet::Commande, Objet::Facture];

    pub fn code(self) -> &'static str {
        match self {
            Objet::Devis => "devis",
            Objet::Commande => "commande",
            Objet::Facture => "facture",
        }
    }

    pub fn libelle(self) -> &'static str {
        match self {
            Objet::Devis => "Devis",
            Objet::Commande => "Commande",
            Objet::Facture => "Facture",
        }
    }

    /// La constante Dolibarr, relevee dans `admin/pdf.php`.
    pub fn constante(self) -> &'static str {
        match self {
            Objet::Devis => "PROPALE_ADDON_PDF",
            Objet::Commande => "COMMANDE_ADDON_PDF",
            Objet::Facture => "FACTURE_ADDON_PDF",
        }
    }
}

/// Les modeles que le noyau sait produire.
///
/// Le code est celui que porte la constante ; le libelle dit ce que le modele
/// change a la lecture, pas son nom de fichier.
pub const MODELES: &[(&str, &str)] = &[
    (
        "standard",
        "Standard — document complet, CGV et notes comprises",
    ),
    (
        "epure",
        "Épuré — même mise en page, sans les annexes longues",
    ),
    ("compact", "Compact — autre gabarit, tient sur deux pages"),
];

/// Le modele retenu pour un objet, ou `None` si rien n'est regle.
///
/// Ordre de lecture : la constante de l'objet, puis l'ancienne constante
/// generale, puis rien — l'appelant prendra le standard.
pub fn modele(pool: &Pool, entity: i64, objet: Objet) -> Result<Option<String>, Box<dyn Error>> {
    if let Some(v) = crate::constantes::lire(pool, entity, objet.constante())? {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Ok(Some(v));
        }
    }
    if let Some(v) = crate::constantes::lire(pool, entity, "DOLIBARR_RS_PDF_MODELE")? {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Ok(Some(v));
        }
    }
    Ok(None)
}

/// Fixe le modele d'un objet. Un code vide revient au defaut.
pub fn choisir(pool: &Pool, entity: i64, objet: Objet, code: &str) -> Result<(), Box<dyn Error>> {
    let code = code.trim();
    if !code.is_empty() && !MODELES.iter().any(|(c, _)| *c == code) {
        return Err(format!("modèle de PDF inconnu : « {code} »").into());
    }
    crate::constantes::ecrire(pool, entity, objet.constante(), code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_constantes_sont_celles_du_coeur() {
        // Relevees dans htdocs/admin/pdf.php de Dolibarr 24.0.1.
        assert_eq!(Objet::Facture.constante(), "FACTURE_ADDON_PDF");
        assert_eq!(Objet::Commande.constante(), "COMMANDE_ADDON_PDF");
        // « PROPALE » avec un E : c'est l'orthographe de Dolibarr, pas une faute.
        assert_eq!(Objet::Devis.constante(), "PROPALE_ADDON_PDF");
    }

    #[test]
    fn chaque_objet_a_un_code_et_un_libelle() {
        for o in Objet::TOUS {
            assert!(!o.code().is_empty());
            assert!(!o.libelle().is_empty());
            assert!(o.constante().ends_with("_ADDON_PDF"));
        }
    }

    #[test]
    fn les_trois_modeles_ont_un_code_et_un_libelle() {
        assert_eq!(MODELES.len(), 3);
        for (code, libelle) in MODELES {
            assert!(!code.trim().is_empty());
            assert!(!libelle.trim().is_empty());
        }
    }

    #[test]
    fn les_codes_sont_uniques() {
        let mut vus = std::collections::BTreeSet::new();
        for (code, _) in MODELES {
            assert!(vus.insert(*code), "code en double : {code}");
        }
    }
}
