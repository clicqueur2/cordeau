//! Mentions obligatoires des documents commerciaux francais.
//!
//! Sources : code de commerce art. L441-9 et R441-3, annexe a l'article
//! D441-5 (delais de paiement, penalites, indemnite de 40 EUR), CGI art. 242
//! nonies A (facturation), art. 293 B (franchise en base), art. 283-2 nonies
//! (autoliquidation).
//!
//! Ce crate ne fait aucune mise en page : il dit **quoi ecrire**, le crate
//! `pdf` dit **ou**.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    Devis,
    Commande,
    Facture,
    Avoir,
}

impl DocType {
    pub fn libelle(&self) -> &'static str {
        match self {
            DocType::Devis => "Devis",
            DocType::Commande => "Bon de commande",
            DocType::Facture => "Facture",
            DocType::Avoir => "Avoir",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RegimeTva {
    /// TVA collectee normalement.
    #[default]
    Assujetti,
    /// Franchise en base : mention art. 293 B du CGI, aucune TVA facturee.
    FranchiseEnBase,
}

#[derive(Debug, Clone, Default)]
pub struct Societe {
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
    pub regime_tva: RegimeTva,
    /// Assurance professionnelle : due par tout artisan, sur devis comme sur
    /// facture (code de l'artisanat, art. L132-1 et R132-1).
    pub assurance: Option<Assurance>,
    /// Mention d'assurance en texte libre, telle que Dolibarr la stocke
    /// (`MAIN_INFO_PROFID5`). Prime sur `assurance` : c'est la donnee reelle de
    /// l'entreprise, pas une reconstruction.
    pub assurance_texte: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Assurance {
    pub assureur: String,
    pub police: String,
    pub couverture: String,
}

#[derive(Debug, Clone, Default)]
pub struct Client {
    pub nom: String,
    pub siret: Option<String>,
    pub tva_intra: Option<String>,
    pub est_professionnel: bool,
    pub pays: Option<String>,
}

impl Client {
    /// Client etabli hors de France dans l'Union europeenne : autoliquidation.
    fn est_ue_hors_france(&self) -> bool {
        match self.pays.as_deref() {
            None => false,
            Some(p) => {
                let p = p.trim().to_ascii_uppercase();
                !p.is_empty() && p != "FR" && p != "FRANCE" && !p.starts_with("FR:")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Conditions {
    /// Delai de paiement en jours a compter de la date de facture.
    pub delai_jours: u32,
    /// Taux des penalites de retard, tel qu'il doit apparaitre sur le document.
    pub penalites: String,
    /// Escompte pour paiement anticipe. `None` = aucun.
    pub escompte: Option<String>,
    /// Validite d'un devis, en jours.
    pub validite_jours: Option<u32>,
    /// Facture d'origine, pour un avoir.
    pub origine: Option<String>,
    /// TVA exigible sur les encaissements (option du redevable).
    pub tva_sur_encaissements: bool,
    /// Travaux de renovation eligibles au taux reduit : l'attestation du
    /// preneur (art. 279-0 bis du CGI) doit figurer sur la facture.
    pub tva_taux_reduit: bool,
}

impl Default for Conditions {
    fn default() -> Self {
        Self {
            delai_jours: 30,
            penalites: "trois fois le taux d'interet legal en vigueur".to_string(),
            escompte: None,
            validite_jours: Some(30),
            origine: None,
            tva_sur_encaissements: false,
            tva_taux_reduit: false,
        }
    }
}

/// Ligne d'identification legale sous le nom du vendeur :
/// « SAS au capital de 10 000 EUR - RCS Lyon 123 456 789 ».
pub fn entete_legal(societe: &Societe) -> Option<String> {
    let mut morceaux: Vec<String> = Vec::new();

    if let Some(f) = societe
        .forme_juridique
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        let mut t = f.trim().to_string();
        if let Some(c) = societe.capital.as_deref().filter(|s| !s.trim().is_empty()) {
            t.push_str(&format!(" au capital de {}", c.trim()));
        }
        morceaux.push(t);
    } else if let Some(c) = societe.capital.as_deref().filter(|s| !s.trim().is_empty()) {
        morceaux.push(format!("Capital de {}", c.trim()));
    }

    if let Some(r) = societe.rcs.as_deref().filter(|s| !s.trim().is_empty()) {
        morceaux.push(r.trim().to_string());
    }

    if morceaux.is_empty() {
        None
    } else {
        Some(morceaux.join(" - "))
    }
}

/// Champs legaux absents, bloquants pour emettre un document.
///
/// Ne restent ici que ce qui empeche vraiment d'emettre : l'identite du
/// vendeur — c'est sa propre fiche, il la complete avant d'imprimer — et le nom
/// du client. Les identifiants du client ne bloquent pas : un particulier n'a
/// ni SIRET ni numero de TVA, et refuser de facturer un particulier reviendrait
/// a refuser l'essentiel de l'artisanat. Ils sont signales par
/// `mentions_client_manquantes`, qui n'empeche rien.
pub fn champs_manquants(societe: &Societe, client: &Client) -> Vec<&'static str> {
    let mut manquants = Vec::new();
    let vide = |v: &Option<String>| v.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true);

    if societe.nom.trim().is_empty() {
        manquants.push("denomination du vendeur");
    }
    if vide(&societe.adresse) {
        manquants.push("adresse du vendeur");
    }
    if vide(&societe.code_postal) {
        manquants.push("code postal du vendeur");
    }
    if vide(&societe.ville) {
        manquants.push("ville du vendeur");
    }
    if vide(&societe.siret) && vide(&societe.siren) {
        manquants.push("SIRET du vendeur");
    }

    if client.nom.trim().is_empty() {
        manquants.push("nom du client");
    }

    manquants
}

/// Mentions qui manquent au client, sans empecher d'emettre.
///
/// SIRET et numero de TVA intracommunautaire ne sont dus qu'entre
/// professionnels (art. 242 nonies A du CGI) ; un particulier n'en a pas, et
/// une entreprise en franchise de TVA non plus. Les signaler laisse le choix de
/// completer la fiche tiers ; les exiger bloquerait la facture.
pub fn mentions_client_manquantes(societe: &Societe, client: &Client) -> Vec<&'static str> {
    let mut manquants = Vec::new();
    if !client.est_professionnel {
        return manquants;
    }
    let vide = |v: &Option<String>| v.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true);

    if vide(&client.siret) {
        manquants.push("SIRET du client");
    }
    if societe.regime_tva == RegimeTva::Assujetti
        && vide(&client.tva_intra)
        && !client.est_ue_hors_france()
    {
        manquants.push("numero de TVA intracommunautaire du client");
    }

    manquants
}

/// Mentions a imprimer en pied de document, dans l'ordre d'apparition.
pub fn mentions(
    doc: DocType,
    societe: &Societe,
    client: &Client,
    conditions: &Conditions,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // Regime de TVA : la mention doit figurer sur tout document.
    if societe.regime_tva == RegimeTva::FranchiseEnBase {
        out.push("TVA non applicable, art. 293 B du CGI.".to_string());
    } else if client.est_ue_hors_france() && client.est_professionnel {
        out.push("Autoliquidation : TVA due par le preneur, art. 283-2 nonies du CGI.".to_string());
    }

    // Assurance professionnelle : due par tout artisan, sur devis comme sur
    // facture (code de l'artisanat, art. L132-1 et R132-1).
    if conditions.tva_sur_encaissements {
        out.push("TVA exigible sur les encaissements.".to_string());
    }

    // Taux reduit de 10 % (travaux de renovation d'un logement acheve depuis
    // plus de deux ans) : l'attestation du preneur est obligatoire.
    if conditions.tva_taux_reduit && doc != DocType::Devis {
        out.push(format!(
            "Je soussigné {} certifie que les travaux faisant l'objet de la présente facture \
             sont éligibles au taux de TVA appliqué, conformément à l'article 279-0 bis du CGI \
             et aux dispositions légales en vigueur.",
            client.nom.trim()
        ));
    }

    // L'assurance en texte libre vient de la fiche Dolibarr : c'est la donnee
    // que l'entreprise a saisie, et la mention est obligatoire pour un artisan.
    if let Some(t) = societe
        .assurance_texte
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        out.push(format!(
            "Assurance responsabilité civile et décennale : {t}"
        ));
    } else if let Some(a) = societe
        .assurance
        .as_ref()
        .filter(|a| !a.assureur.trim().is_empty())
    {
        let couverture = if a.couverture.trim().is_empty() {
            "France"
        } else {
            a.couverture.trim()
        };
        out.push(format!(
            "Assurance décennale : {} - police n° {} - couverture géographique : {}.",
            a.assureur.trim(),
            a.police.trim(),
            couverture
        ));
    }

    match doc {
        DocType::Devis => {
            if let Some(j) = conditions.validite_jours {
                out.push(format!(
                    "Devis valable {j} jours a compter de sa date d'emission."
                ));
            }
            out.push(
                "Prix exprimes en euros hors taxes ; TVA au taux en vigueur a la date de facturation."
                    .to_string(),
            );
            out.push(
                "Les travaux supplementaires non prevus au present devis feront l'objet d'un avenant chiffre avant execution."
                    .to_string(),
            );
        }
        DocType::Commande => {
            out.push(format!(
                "Reglement a {} jours a compter de la date de facture.",
                conditions.delai_jours
            ));
        }
        DocType::Facture => {
            out.push(mention_penalites(&conditions.penalites));
            out.push("Indemnite forfaitaire pour frais de recouvrement : 40 EUR.".to_string());
            match conditions
                .escompte
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                Some(e) => out.push(format!("Escompte pour paiement anticipe : {e}.")),
                None => out.push("Pas d'escompte pour paiement anticipe.".to_string()),
            }
        }
        DocType::Avoir => {
            match conditions
                .origine
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                Some(r) => out.push(format!("Avoir sur la facture n° {r}.")),
                None => out.push(
                    "Avoir : la facture d'origine doit etre referencee (a completer).".to_string(),
                ),
            }
            if societe.regime_tva == RegimeTva::Assujetti {
                out.push("TVA regularisee sur l'avoir.".to_string());
            }
        }
    }

    if let Some(l) = entete_legal(societe) {
        out.push(l);
    }
    if let Some(e) = societe.email.as_deref().filter(|s| !s.trim().is_empty()) {
        out.push(format!("Contact : {e}"));
    }

    out
}

fn mention_penalites(taux: &str) -> String {
    format!(
        "Penalites de retard de paiement : {taux}. Tout retard entraine de plein droit \
         l'exigibilite des penalites, sans mise en demeure prealable."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn societe() -> Societe {
        Societe {
            nom: "Pierre Angulaire".into(),
            forme_juridique: Some("SAS".into()),
            capital: Some("10 000 EUR".into()),
            rcs: Some("RCS Lyon 123 456 789".into()),
            adresse: Some("1 rue de l'Exemple".into()),
            code_postal: Some("69005".into()),
            ville: Some("Lyon".into()),
            siret: Some("12345678900012".into()),
            tva_intra: Some("FR12345678900".into()),
            email: Some("contact@exemple.fr".into()),
            ..Default::default()
        }
    }

    fn client_pro() -> Client {
        Client {
            nom: "M. et Mme Dupont".into(),
            siret: Some("98765432100019".into()),
            tva_intra: Some("FR98765432100".into()),
            est_professionnel: true,
            pays: Some("FR".into()),
        }
    }

    #[test]
    fn entete_legale_composee() {
        let e = entete_legal(&societe()).expect("entete");
        assert!(e.contains("SAS au capital de 10 000 EUR"));
        assert!(e.contains("RCS Lyon"));
    }

    #[test]
    fn entete_absente_si_rien() {
        let s = Societe {
            nom: "X".into(),
            ..Default::default()
        };
        assert!(entete_legal(&s).is_none());
    }

    #[test]
    fn facture_porte_les_mentions_de_retard() {
        let m = mentions(
            DocType::Facture,
            &societe(),
            &client_pro(),
            &Conditions::default(),
        );
        let tout = m.join(" ");
        assert!(tout.contains("Penalites de retard"));
        assert!(tout.contains("40 EUR"));
        assert!(tout.contains("Pas d'escompte"));
        assert!(tout.contains("RCS Lyon"));
        assert!(!tout.contains("293 B"), "pas de franchise ici");
    }

    #[test]
    fn franchise_en_base_mentionne_293_b() {
        let mut s = societe();
        s.regime_tva = RegimeTva::FranchiseEnBase;
        let m = mentions(DocType::Facture, &s, &client_pro(), &Conditions::default());
        assert!(m.iter().any(|l| l.contains("293 B du CGI")));
    }

    #[test]
    fn client_ue_pro_declenche_autoliquidation() {
        let mut c = client_pro();
        c.pays = Some("DE".into());
        let m = mentions(DocType::Facture, &societe(), &c, &Conditions::default());
        assert!(
            m.iter().any(|l| l.contains("283-2 nonies")),
            "autoliquidation attendue : {m:?}"
        );
    }

    #[test]
    fn client_francais_pas_d_autoliquidation() {
        let m = mentions(
            DocType::Facture,
            &societe(),
            &client_pro(),
            &Conditions::default(),
        );
        assert!(!m.iter().any(|l| l.contains("283-2 nonies")));
    }

    #[test]
    fn devis_porte_la_validite_pas_les_penalites() {
        let m = mentions(
            DocType::Devis,
            &societe(),
            &client_pro(),
            &Conditions::default(),
        );
        let tout = m.join(" ");
        assert!(tout.contains("valable 30 jours"));
        assert!(!tout.contains("Penalites de retard"));
        assert!(!tout.contains("40 EUR"));
    }

    #[test]
    fn avoir_sans_origine_est_signale() {
        let m = mentions(
            DocType::Avoir,
            &societe(),
            &client_pro(),
            &Conditions::default(),
        );
        assert!(m.iter().any(|l| l.contains("a completer")));

        let c = Conditions {
            origine: Some("FA2611-0001".into()),
            ..Default::default()
        };
        let m = mentions(DocType::Avoir, &societe(), &client_pro(), &c);
        assert!(m.iter().any(|l| l.contains("FA2611-0001")));
    }

    #[test]
    fn detecte_les_champs_legaux_manquants() {
        let vide = Societe::default();
        let client = Client {
            nom: String::new(),
            ..Default::default()
        };
        let m = champs_manquants(&vide, &client);
        assert!(m.contains(&"denomination du vendeur"));
        assert!(m.contains(&"SIRET du vendeur"));
        assert!(m.contains(&"nom du client"));
    }

    #[test]
    fn une_societe_complete_ne_manque_de_rien() {
        let m = champs_manquants(&societe(), &client_pro());
        assert!(m.is_empty(), "champs manquants inattendus : {m:?}");
    }

    /// Un professionnel sans SIRET n'empeche pas d'emettre : sa fiche est
    /// incomplete, la facture reste valable. C'est un avertissement, pas un
    /// refus.
    #[test]
    fn un_client_sans_siret_n_empeche_pas_la_facture() {
        let mut c = client_pro();
        c.siret = None;
        assert!(champs_manquants(&societe(), &c).is_empty());
        assert_eq!(
            mentions_client_manquantes(&societe(), &c),
            vec!["SIRET du client"]
        );
    }

    /// Le cas de la production : Dolibarr stocke la chaine vide, pas `NULL`.
    /// Un tiers dont le SIRET vaut `""` n'est pas un professionnel — sinon
    /// toutes les factures de particuliers seraient refusees.
    #[test]
    fn un_siret_en_chaine_vide_ne_fait_pas_un_professionnel() {
        let mut c = client_pro();
        c.siret = Some(String::new());
        c.tva_intra = Some("  ".to_string());
        c.est_professionnel = false;
        assert!(champs_manquants(&societe(), &c).is_empty());
        assert!(mentions_client_manquantes(&societe(), &c).is_empty());
    }
}
