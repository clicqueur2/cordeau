//! Factur-X : le XML CII que la réforme de 2026 rend obligatoire.
//!
//! Depuis le 1er septembre 2026, toute entreprise assujettie à la TVA établie en
//! France doit pouvoir **recevoir** des factures électroniques, et les émettre
//! progressivement (grandes entreprises et ETI dès 2026, toutes en 2027). Le
//! format d'usage est **Factur-X** : un PDF avec, à l'intérieur, un fichier XML
//! au format **CII** (Cross Industry Invoice) conforme à la norme EN 16931.
//!
//! Ce crate produit le **XML**. Il ne produit pas encore le PDF/A-3 qui
//! l'encapsule : attacher un fichier à un PDF existant demande une
//! bibliothèque PDF (flux `/EmbeddedFile`, arbre `/Names`, `/AF`) et une passe
//! de conformité PDF/A-3. C'est le palier suivant, et il est nommé.
//!
//! # Ce que le XML contient
//!
//! Profil **EN 16931** (le plus complet des trois : MINIMUM, BASIC, EN 16931) :
//! vendeur et acheteur identifiés, numéro et date, lignes avec quantité et prix,
//! ventilation de TVA par taux, totaux HT/TVA/TTC, échéance.
//!
//! # Ce que ce module ne fait pas
//!
//! - pas de validation contre le schéma XSD officiel (il faudrait embarquer le
//!   schéma) ; les tests vérifient la structure et les sommes, pas la conformité
//!   formelle ;
//! - pas de PDF/A-3, donc pas de Factur-X complet au sens strict ;
//! - pas de plateforme agréée : c'est un partenariat, pas du code.

use std::fmt::Write as _;

use rust_decimal::Decimal;

/// Profil Factur-X.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profil {
    /// Minimum légal.
    Minimum,
    /// Le plus courant pour une PME.
    Basic,
    /// Complet, avec ventilation de TVA détaillée.
    En16931,
}

impl Profil {
    pub fn code(self) -> &'static str {
        match self {
            Profil::Minimum => "MINIMUM",
            Profil::Basic => "BASIC",
            Profil::En16931 => "EN 16931",
        }
    }
}

/// Une partie (vendeur ou acheteur).
#[derive(Debug, Clone, Default)]
pub struct Partie {
    pub nom: String,
    pub siret: Option<String>,
    pub tva: Option<String>,
    pub rue: Option<String>,
    pub code_postal: Option<String>,
    pub ville: Option<String>,
    pub pays: Option<String>,
}

/// Une ligne de facture.
#[derive(Debug, Clone)]
pub struct Ligne {
    pub designation: String,
    pub quantite: Decimal,
    pub unite: Option<String>,
    pub prix_unitaire_ht: Decimal,
    pub taux_tva: Decimal,
    pub montant_ht: Decimal,
    /// Motif d'exoneration, quand la ligne n'est pas soumise au taux normal.
    pub exoneration: Option<Exoneration>,
}

/// Un motif d'exoneration de TVA, tel que Factur-X le porte.
///
/// Deux elements de la norme : le **code** (BT-120, une valeur de la liste
/// VATEX de la Commission) et le **texte** libre (BT-121). Dolibarr les tient
/// dans `llx_c_tva.einvoice_vatex`, colonne ajoutee en 23.0 pour preparer la
/// reforme francaise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exoneration {
    /// Code VATEX, par exemple `VATEX-EU-132`.
    pub code: String,
    /// Texte libre, quand le code ne suffit pas.
    pub motif: Option<String>,
}

impl Exoneration {
    /// La categorie de TVA (BT-118) que ce code impose.
    ///
    /// C'est la categorie qui decide de tout : une exoneration declaree en
    /// categorie « S » (taux normal) est simplement fausse, et une plateforme
    /// la refuse. Un code qu'on ne reconnait pas tombe en « E » (exonere),
    /// qui est la categorie la plus prudente — et qui exige un motif.
    pub fn categorie(&self) -> &'static str {
        let c = self.code.to_uppercase();
        if c.contains("AUTOLIQ") || c.contains("-AE") {
            "AE"
        } else if c.contains("-IC") || c.contains("-K") {
            "K"
        } else if c.contains("-G") {
            "G"
        } else if c.contains("-O") {
            "O"
        } else {
            "E"
        }
    }
}

/// Une ventilation de TVA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentilationTva {
    pub taux: Decimal,
    /// Le motif d'exoneration de cette ventilation, s'il y en a un.
    pub exoneration: Option<Exoneration>,
    pub base_ht: Decimal,
    pub montant: Decimal,
}

/// La facture à convertir.
#[derive(Debug, Clone)]
pub struct Facture {
    pub numero: String,
    /// Date d'émission, `AAAA-MM-JJ`.
    pub date: String,
    /// Échéance, `AAAA-MM-JJ`.
    pub echeance: Option<String>,
    pub vendeur: Partie,
    pub acheteur: Partie,
    pub lignes: Vec<Ligne>,
    pub total_ht: Decimal,
    pub total_tva: Decimal,
    pub total_ttc: Decimal,
    pub devise: String,
    /// Code moyen de paiement EN 16931 (BT-81) : 30 = virement, 10 = especes,
    /// 58 = SEPA. Facultatif, mais attendu par la plupart des plateformes.
    pub moyen_paiement: Option<String>,
    /// IBAN du compte a crediter (BT-84).
    pub iban: Option<String>,
    /// Processus metier (BT-23). « A1 » = commande, valeur d'usage en France.
    pub processus: Option<String>,
}

/// Ce qui empêche de produire un XML valable.
#[derive(Debug, PartialEq, Eq)]
pub enum Erreur {
    NumeroVide,
    DateInvalide(String),
    VendeurSansNom,
    AcheteurSansNom,
    AucuneLigne,
    /// Les totaux annoncés ne correspondent pas aux lignes.
    TotauxIncoherents(String),
    /// Le PDF n a pas pu etre lu ou reecrit.
    Pdf(String),
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::NumeroVide => write!(f, "une facture sans numéro n'a pas d'identité"),
            Erreur::DateInvalide(d) => write!(f, "date attendue AAAA-MM-JJ : {d}"),
            Erreur::VendeurSansNom => write!(f, "le vendeur doit être nommé"),
            Erreur::AcheteurSansNom => write!(f, "l'acheteur doit être nommé"),
            Erreur::AucuneLigne => write!(f, "une facture sans ligne n'a rien à déclarer"),
            Erreur::TotauxIncoherents(m) => write!(f, "totaux incohérents : {m}"),
            Erreur::Pdf(m) => write!(f, "PDF : {m}"),
        }
    }
}

impl std::error::Error for Erreur {}

fn date_valide(d: &str) -> bool {
    d.len() == 10
        && d.chars().enumerate().all(|(i, c)| match i {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        })
}

fn echappe(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            autre => out.push(autre),
        }
    }
    out
}

/// Montant à deux décimales, point décimal (le format d'échange, pas le français).
fn m(v: Decimal) -> String {
    let a = v.round_dp(2);
    format!("{a:.2}")
}

/// Ventile la TVA par taux, à partir des lignes.
pub fn ventiler(lignes: &[Ligne]) -> Vec<VentilationTva> {
    let mut par_taux: std::collections::BTreeMap<String, VentilationTva> =
        std::collections::BTreeMap::new();
    for l in lignes {
        // La cle porte le taux **et** le motif : deux exonerations differentes
        // au meme taux sont deux lignes de ventilation, pas une.
        let cle = format!(
            "{}|{}",
            l.taux_tva.normalize(),
            l.exoneration
                .as_ref()
                .map(|e| e.code.clone())
                .unwrap_or_default()
        );
        let e = par_taux.entry(cle).or_insert(VentilationTva {
            taux: l.taux_tva,
            exoneration: l.exoneration.clone(),
            base_ht: Decimal::ZERO,
            montant: Decimal::ZERO,
        });
        e.base_ht += l.montant_ht;
        e.montant += (l.montant_ht * l.taux_tva / Decimal::from(100)).round_dp(2);
    }
    par_taux.into_values().collect()
}

/// Les deux elements d'exoneration (BT-120, BT-121), ou rien.
///
/// `ExemptionReasonCode` et `ExemptionReason` vivent dans
/// `ram:ApplicableTradeTax`, apres la categorie. Omettre le motif quand il y a
/// une exoneration fait rejeter la facture.
fn bloc_exoneration(e: Option<&Exoneration>) -> String {
    let Some(e) = e else {
        return String::new();
    };
    let mut s = String::new();
    if !e.code.trim().is_empty() {
        let _ = write!(
            s,
            "<ram:ExemptionReasonCode>{}</ram:ExemptionReasonCode>",
            echappe(&e.code)
        );
    }
    if let Some(motif) = &e.motif {
        if !motif.trim().is_empty() {
            let _ = write!(
                s,
                "<ram:ExemptionReason>{}</ram:ExemptionReason>",
                echappe(motif)
            );
        }
    }
    s
}

/// Produit le XML CII.
pub fn xml(facture: &Facture, profil: Profil) -> Result<String, Erreur> {
    if facture.numero.trim().is_empty() {
        return Err(Erreur::NumeroVide);
    }
    if !date_valide(&facture.date) {
        return Err(Erreur::DateInvalide(facture.date.clone()));
    }
    if let Some(e) = &facture.echeance {
        if !date_valide(e) {
            return Err(Erreur::DateInvalide(e.clone()));
        }
    }
    if facture.vendeur.nom.trim().is_empty() {
        return Err(Erreur::VendeurSansNom);
    }
    if facture.acheteur.nom.trim().is_empty() {
        return Err(Erreur::AcheteurSansNom);
    }
    if facture.lignes.is_empty() {
        return Err(Erreur::AucuneLigne);
    }

    // Contrôle qui évite d'envoyer une facture fausse : la somme des lignes
    // doit égaler le HT annoncé, au centime.
    let somme_ht: Decimal = facture.lignes.iter().map(|l| l.montant_ht).sum();
    if somme_ht != facture.total_ht {
        return Err(Erreur::TotauxIncoherents(format!(
            "somme des lignes {} contre total HT {}",
            m(somme_ht),
            m(facture.total_ht)
        )));
    }

    let ventilations = ventiler(&facture.lignes);
    let somme_tva: Decimal = ventilations.iter().map(|v| v.montant).sum();
    if somme_tva != facture.total_tva {
        return Err(Erreur::TotauxIncoherents(format!(
            "TVA ventilée {} contre total TVA {}",
            m(somme_tva),
            m(facture.total_tva)
        )));
    }

    let devise = if facture.devise.trim().is_empty() {
        "EUR"
    } else {
        &facture.devise
    };

    let mut x = String::new();
    let _ = write!(
        x,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<rsm:CrossIndustryInvoice ",
            "xmlns:rsm=\"urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100\" ",
            "xmlns:qdt=\"urn:un:unece:uncefact:data:standard:QualifiedDataType:100\" ",
            "xmlns:ram=\"urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100\" ",
            "xmlns:udt=\"urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100\">\n",
            "  <rsm:ExchangedDocumentContext>\n",
            "    <ram:GuidelineSpecifiedDocumentContextParameter>\n",
            "      <ram:ID>urn:cen.eu:en16931:2017</ram:ID>\n",
            "    </ram:GuidelineSpecifiedDocumentContextParameter>\n",
            "    <ram:BusinessProcessSpecifiedDocumentContextParameter>\n",
            "      <ram:ID>{processus}</ram:ID>\n",
            "    </ram:BusinessProcessSpecifiedDocumentContextParameter>\n",
            "  </rsm:ExchangedDocumentContext>\n",
        ),
        processus = echappe(facture.processus.as_deref().unwrap_or("A1")),
    );
    let _ = write!(
        x,
        concat!(
            "  <rsm:ExchangedDocument>\n",
            "    <ram:ID>{numero}</ram:ID>\n",
            "    <ram:TypeCode>380</ram:TypeCode>\n",
            "    <ram:IssueDateTime><udt:DateTimeString format=\"102\">{date102}</udt:DateTimeString></ram:IssueDateTime>\n",
            "  </rsm:ExchangedDocument>\n",
        ),
        numero = echappe(&facture.numero),
        date102 = facture.date.replace('-', ""),
    );

    let _ = writeln!(x, "  <rsm:SupplyChainTradeTransaction>");

    // Lignes.
    for (i, l) in facture.lignes.iter().enumerate() {
        let _ = write!(
            x,
            concat!(
                "    <ram:IncludedSupplyChainTradeLineItem>\n",
                "      <ram:AssociatedDocumentLineDocument><ram:LineID>{n}</ram:LineID></ram:AssociatedDocumentLineDocument>\n",
                "      <ram:SpecifiedTradeProduct><ram:Name>{designation}</ram:Name></ram:SpecifiedTradeProduct>\n",
                "      <ram:SpecifiedLineTradeAgreement>\n",
                "        <ram:NetPriceProductTradePrice><ram:ChargeAmount>{pu}</ram:ChargeAmount></ram:NetPriceProductTradePrice>\n",
                "      </ram:SpecifiedLineTradeAgreement>\n",
                "      <ram:SpecifiedLineTradeDelivery><ram:BilledQuantity unitCode=\"{unite}\">{qte}</ram:BilledQuantity></ram:SpecifiedLineTradeDelivery>\n",
                "      <ram:SpecifiedLineTradeSettlement>\n",
                "        <ram:ApplicableTradeTax><ram:TypeCode>VAT</ram:TypeCode><ram:CategoryCode>{categorie}</ram:CategoryCode>",
                "{exoneration}",
                "<ram:RateApplicablePercent>{taux}</ram:RateApplicablePercent></ram:ApplicableTradeTax>\n",
                "        <ram:SpecifiedTradeSettlementLineMonetarySummation>",
                "<ram:LineTotalAmount>{ht}</ram:LineTotalAmount>",
                "</ram:SpecifiedTradeSettlementLineMonetarySummation>\n",
                "      </ram:SpecifiedLineTradeSettlement>\n",
                "    </ram:IncludedSupplyChainTradeLineItem>\n",
            ),
            n = i + 1,
            designation = echappe(&l.designation),
            pu = m(l.prix_unitaire_ht),
            unite = echappe(l.unite.as_deref().unwrap_or("C62")),
            qte = l.quantite.normalize(),
            taux = m(l.taux_tva),
            ht = m(l.montant_ht),
            categorie = l.exoneration.as_ref().map(|e| e.categorie()).unwrap_or("S"),
            exoneration = bloc_exoneration(l.exoneration.as_ref()),
        );
    }

    // Vendeur et acheteur.
    // Une facture entierement hors champ : la categorie O impose de retirer
    // les identifiants de TVA et de ne pas melanger les regimes.
    let hors_champ = ventilations
        .iter()
        .any(|v| v.exoneration.as_ref().map(|e| e.categorie()) == Some("O"));

    let _ = write!(
        x,
        concat!(
            "    <ram:ApplicableHeaderTradeAgreement>\n",
            "      <ram:SellerTradeParty>\n",
            "        <ram:Name>{vendeur}</ram:Name>\n",
            "        {vendeur_siret}",
            "        {vendeur_tva}",
            "        {vendeur_adresse}",
            "      </ram:SellerTradeParty>\n",
            "      <ram:BuyerTradeParty>\n",
            "        <ram:Name>{acheteur}</ram:Name>\n",
            "        {acheteur_siret}",
            "        {acheteur_adresse}",
            "      </ram:BuyerTradeParty>\n",
            "    </ram:ApplicableHeaderTradeAgreement>\n",
        ),
        vendeur = echappe(&facture.vendeur.nom),
        vendeur_siret = bloc_siret(&facture.vendeur, "SpecifiedLegalOrganization"),
        // Une operation hors champ de la TVA (categorie O) ne porte **aucun**
        // identifiant de TVA : ni celui du vendeur, ni celui de l'acheteur. Le
        // SIREN reste, c'est lui qui identifie la partie.
        vendeur_tva = if hors_champ {
            String::new()
        } else {
            bloc_tva(&facture.vendeur)
        },
        vendeur_adresse = bloc_adresse(&facture.vendeur),
        acheteur = echappe(&facture.acheteur.nom),
        acheteur_siret = bloc_siret(&facture.acheteur, "SpecifiedLegalOrganization"),
        acheteur_adresse = bloc_adresse(&facture.acheteur),
    );

    let _ = write!(
        x,
        concat!(
            "    <ram:ApplicableHeaderTradeDelivery/>\n",
            "    <ram:ApplicableHeaderTradeSettlement>\n",
            "      <ram:InvoiceCurrencyCode>{devise}</ram:InvoiceCurrencyCode>\n",
            "      {ventilations}",
            "      <ram:SpecifiedTradeSettlementHeaderMonetarySummation>\n",
            "        <ram:LineTotalAmount>{ht}</ram:LineTotalAmount>\n",
            "        <ram:TaxBasisTotalAmount>{ht}</ram:TaxBasisTotalAmount>\n",
            "        <ram:TaxTotalAmount currencyID=\"{devise}\">{tva}</ram:TaxTotalAmount>\n",
            "        <ram:GrandTotalAmount>{ttc}</ram:GrandTotalAmount>\n",
            "        <ram:DuePayableAmount>{ttc}</ram:DuePayableAmount>\n",
            "      </ram:SpecifiedTradeSettlementHeaderMonetarySummation>\n",
            "      {echeance}",
            "      {paiement}",
            "    </ram:ApplicableHeaderTradeSettlement>\n",
            "  </rsm:SupplyChainTradeTransaction>\n",
            "</rsm:CrossIndustryInvoice>\n",
        ),
        devise = echappe(devise),
        ventilations = ventilations
            .iter()
            .map(|v| format!(
                concat!(
                    "      <ram:ApplicableTradeTax>\n",
                    "        <ram:CalculatedAmount>{}</ram:CalculatedAmount>\n",
                    "        <ram:TypeCode>VAT</ram:TypeCode>\n",
                    "        <ram:BasisAmount>{}</ram:BasisAmount>\n",
                    "        <ram:CategoryCode>{}</ram:CategoryCode>\n",
                    "        {}\n",
                    "        <ram:RateApplicablePercent>{}</ram:RateApplicablePercent>\n",
                    "      </ram:ApplicableTradeTax>\n",
                ),
                m(v.montant),
                m(v.base_ht),
                v.exoneration.as_ref().map(|e| e.categorie()).unwrap_or("S"),
                bloc_exoneration(v.exoneration.as_ref()),
                m(v.taux)
            ))
            .collect::<String>(),
        ht = m(facture.total_ht),
        tva = m(facture.total_tva),
        ttc = m(facture.total_ttc),
        paiement = {
            let mut s = String::new();
            if let Some(code) = &facture.moyen_paiement {
                s.push_str("      <ram:SpecifiedTradeSettlementPaymentMeans>");
                let _ = write!(s, "<ram:TypeCode>{}</ram:TypeCode>", echappe(code));
                if let Some(iban) = &facture.iban {
                    let _ = write!(
                        s,
                        "<ram:PayeePartyCreditorFinancialAccount><ram:IBANID>{}</ram:IBANID>\
                         </ram:PayeePartyCreditorFinancialAccount>",
                        echappe(iban)
                    );
                }
                let _ = writeln!(s, "</ram:SpecifiedTradeSettlementPaymentMeans>");
            }
            s
        },
        echeance = match &facture.echeance {
            Some(e) => format!(
                "      <ram:SpecifiedTradePaymentTerms><ram:DueDateDateTime>\
                 <udt:DateTimeString format=\"102\">{}</udt:DateTimeString>\
                 </ram:DueDateDateTime></ram:SpecifiedTradePaymentTerms>\n",
                e.replace('-', "")
            ),
            None => String::new(),
        },
    );

    // Le profil figure dans le nom du fichier et dans le PDF ; le XML garde la
    // meme racine. On le note pourtant en commentaire : une facture MINIMUM
    // n'a pas la meme exigence qu'une EN 16931.
    let _ = writeln!(x, "<!-- profil {} -->", profil.code());
    Ok(x)
}

fn bloc_siret(p: &Partie, balise: &str) -> String {
    match &p.siret {
        Some(s) if !s.trim().is_empty() => format!(
            "<ram:{balise}><ram:ID schemeID=\"0009\">{}</ram:ID></ram:{balise}>\n        ",
            echappe(s)
        ),
        _ => String::new(),
    }
}

fn bloc_tva(p: &Partie) -> String {
    match &p.tva {
        Some(t) if !t.trim().is_empty() => format!(
            "<ram:SpecifiedTaxRegistration><ram:ID schemeID=\"VA\">{}</ram:ID>\
             </ram:SpecifiedTaxRegistration>\n        ",
            echappe(t)
        ),
        _ => String::new(),
    }
}

fn bloc_adresse(p: &Partie) -> String {
    let mut s = String::from("<ram:PostalTradeAddress>\n          ");
    if let Some(cp) = &p.code_postal {
        let _ = write!(s, "<ram:PostcodeCode>{}</ram:PostcodeCode>", echappe(cp));
    }
    if let Some(rue) = &p.rue {
        let _ = write!(s, "<ram:LineOne>{}</ram:LineOne>", echappe(rue));
    }
    if let Some(ville) = &p.ville {
        let _ = write!(s, "<ram:CityName>{}</ram:CityName>", echappe(ville));
    }
    let pays = p.pays.clone().unwrap_or_else(|| "FR".to_string());
    let _ = write!(s, "<ram:CountryID>{}</ram:CountryID>", echappe(&pays));
    s.push_str("</ram:PostalTradeAddress>\n        ");
    s
}

// ---------------------------------------------------------------------------
// Encapsulation : coller le XML dans le PDF.
//
// Un Factur-X, c'est un PDF qui **contient** ce XML comme fichier joint. On
// utilise `lopdf` pour cela : ajout d'un flux `/EmbeddedFile`, d'un `/Filespec`,
// d'un arbre `/Names/EmbeddedFiles` et de l'entree `/AF` au catalogue.
//
// Ce que cela donne : un PDF lisible par Acrobat, dont on peut extraire le XML,
// et que les plateformes savent lire. Ce que cela ne donne pas : la conformite
// **PDF/A-3** (polices normalisees, sortie d'intention, metadonnees XMP). Un
// validateur PDF/A le refuserait encore.
// ---------------------------------------------------------------------------

use lopdf::{dictionary, Document, Object, Stream};

/// Nom du fichier joint attendu par Factur-X.
pub const NOM_FICHIER: &str = "factur-x.xml";

/// Insere le XML dans un PDF existant et rend le nouveau PDF.
///
/// Le XML est joint sous `factur-x.xml`, declare en `/AFRelationship
/// /Alternative` : c'est la convention Factur-X (le PDF est l'image, le XML est
/// la piece de reference).
pub fn attacher(pdf: &[u8], xml: &str) -> Result<Vec<u8>, Erreur> {
    let mut doc =
        Document::load_mem(pdf).map_err(|e| Erreur::Pdf(format!("PDF illisible : {e}")))?;

    // 1. Le flux du fichier joint.
    let flux = Stream::new(
        dictionary! {
            "Type" => "EmbeddedFile",
            "Subtype" => Object::Name(b"text/xml".to_vec()),
            "Params" => dictionary! {
                "Size" => xml.len() as i64,
            },
        },
        xml.as_bytes().to_vec(),
    );
    let id_flux = doc.add_object(flux);

    // 2. La description du fichier (/Filespec).
    let fiche = dictionary! {
        "Type" => "Filespec",
        "F" => Object::string_literal(NOM_FICHIER),
        "UF" => Object::string_literal(NOM_FICHIER),
        "Desc" => Object::string_literal("Factur-X XML"),
        "AFRelationship" => "Alternative",
        "EF" => dictionary! {
            "F" => Object::Reference(id_flux),
            "UF" => Object::Reference(id_flux),
        },
    };
    let id_fiche = doc.add_object(fiche);

    // 3. L'arbre des noms.
    let arbre = dictionary! {
        "Names" => vec![
            Object::string_literal(NOM_FICHIER),
            Object::Reference(id_fiche),
        ],
    };
    let id_arbre = doc.add_object(arbre);

    // 4. Le catalogue : /Names et /AF.
    {
        let catalogue = doc
            .catalog_mut()
            .map_err(|e| Erreur::Pdf(format!("catalogue absent : {e}")))?;
        catalogue.set(
            "Names",
            Object::Dictionary(dictionary! {
                "EmbeddedFiles" => Object::Reference(id_arbre),
            }),
        );
        catalogue.set("AF", vec![Object::Reference(id_fiche)]);
    }

    let mut sortie = Vec::new();
    doc.save_to(&mut sortie)
        .map_err(|e| Erreur::Pdf(format!("reecriture impossible : {e}")))?;
    Ok(sortie)
}

/// Extrait le XML d'un PDF (pour verifier ce qu'on a produit).
pub fn extraire(pdf: &[u8]) -> Result<String, Erreur> {
    let doc = Document::load_mem(pdf).map_err(|e| Erreur::Pdf(format!("PDF illisible : {e}")))?;
    for objet in doc.objects.values() {
        if let Object::Stream(flux) = objet {
            let type_ = flux
                .dict
                .get(b"Type")
                .and_then(|o| o.as_name())
                .map(|n| n.to_vec())
                .unwrap_or_default();
            if type_ == b"EmbeddedFile" {
                return Ok(String::from_utf8_lossy(&flux.content).to_string());
            }
        }
    }
    Err(Erreur::Pdf("aucun fichier joint dans ce PDF".into()))
}

// ---------------------------------------------------------------------------
// Verification au regard de la norme.
//
// On ne valide pas le XSD (il faudrait embarquer le schema) : on verifie la
// **presence des elements obligatoires** de la norme EN 16931, en citant leur
// numero (BT-xx / BG-xx). Un element manquant est un motif de rejet par une
// plateforme agreee, donc on le nomme.
// ---------------------------------------------------------------------------

/// Un element obligatoire absent, avec son numero de la norme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manquement {
    /// Numero de la norme, par exemple « BT-5 ».
    pub reference: &'static str,
    /// Ce que la norme demande.
    pub libelle: &'static str,
}

/// Les elements obligatoires d'une facture **EN 16931**, avec la balise qui les
/// porte dans le CII. La liste suit la norme, pas notre implementation.
const OBLIGATOIRES: &[(&str, &str, &str)] = &[
    // En-tete du document.
    ("BT-1", "Numero de facture", "<ram:ID>"),
    ("BT-2", "Date d'emission", "<udt:DateTimeString"),
    (
        "BT-3",
        "Type de document (380 = facture)",
        "<ram:TypeCode>380</ram:TypeCode>",
    ),
    ("BT-5", "Devise de la facture", "<ram:InvoiceCurrencyCode>"),
    (
        "BT-23",
        "Processus metier",
        "<ram:BusinessProcessSpecifiedDocumentContextParameter>",
    ),
    // Vendeur (BG-4).
    ("BT-27", "Nom du vendeur", "<ram:SellerTradeParty>"),
    (
        "BT-30",
        "Identifiant legal du vendeur (SIREN/SIRET)",
        "schemeID=\"0009\"",
    ),
    ("BT-31", "Numero de TVA du vendeur", "schemeID=\"VA\""),
    ("BG-5", "Adresse du vendeur", "<ram:PostalTradeAddress>"),
    // Acheteur (BG-7).
    ("BT-44", "Nom de l'acheteur", "<ram:BuyerTradeParty>"),
    ("BG-8", "Adresse de l'acheteur", "<ram:PostalTradeAddress>"),
    // Totaux (BG-22).
    ("BT-106", "Somme des lignes HT", "<ram:LineTotalAmount>"),
    ("BT-109", "Total HT", "<ram:TaxBasisTotalAmount>"),
    ("BT-110", "Total TVA", "<ram:TaxTotalAmount"),
    ("BT-112", "Total TTC", "<ram:GrandTotalAmount>"),
    ("BT-115", "Montant du", "<ram:DuePayableAmount>"),
    // Ventilation de TVA (BG-23).
    ("BT-116", "Base HT par taux", "<ram:BasisAmount>"),
    (
        "BT-117",
        "Montant de TVA par taux",
        "<ram:CalculatedAmount>",
    ),
    (
        "BT-118",
        "Code de categorie de TVA",
        // Une facture entierement exoneree n'a pas de « S » : exiger la lettre
        // ferait refuser une facture conforme.
        "<ram:CategoryCode>",
    ),
    ("BT-119", "Taux de TVA", "<ram:RateApplicablePercent>"),
    // Lignes (BG-25).
    ("BT-126", "Identifiant de ligne", "<ram:LineID>"),
    ("BT-129", "Quantite", "<ram:BilledQuantity"),
    ("BT-130", "Unite", "unitCode="),
    ("BT-131", "Montant de la ligne", "<ram:LineTotalAmount>"),
    ("BT-146", "Prix unitaire", "<ram:ChargeAmount>"),
];

/// Verifie la presence des elements obligatoires. Rend la liste des manquants,
/// vide si tout est la.
pub fn verifier_norme(xml: &str) -> Vec<Manquement> {
    let mut manquants = Vec::new();
    // Sur une operation hors champ (categorie O), l'identifiant de TVA du
    // vendeur (BT-31) est **interdit**, pas seulement facultatif : l'exiger
    // ferait refuser une facture conforme.
    let hors_champ = xml.contains("<ram:CategoryCode>O</ram:CategoryCode>");

    for (reference, libelle, balise) in OBLIGATOIRES {
        if hors_champ && *reference == "BT-31" {
            continue;
        }
        if !xml.contains(balise) {
            manquants.push(Manquement { reference, libelle });
        }
    }
    manquants
}

// ---------------------------------------------------------------------------
// Les regles metier EN 16931 (famille BR-*).
//
// La presence des elements ne suffit pas : les plateformes appliquent aussi
// les regles de coherence. Ce sont elles qui font refuser une facture
// « complete mais fausse ». On implemente les regles arithmetiques et de
// nomenclature qui comptent, en les citant par leur numero.
// ---------------------------------------------------------------------------

/// Une regle metier violee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub reference: &'static str,
    pub libelle: &'static str,
}

/// Verifie les regles metier sur la **facture source** (pas sur le XML :
/// reparser notre propre sortie pour verifier nos sommes serait un
/// raisonnement circulaire).
pub fn regles_metier(facture: &Facture) -> Vec<Violation> {
    let mut v = Vec::new();

    // BR-16 : au moins une ligne.
    if facture.lignes.is_empty() {
        v.push(Violation {
            reference: "BR-16",
            libelle: "une facture doit comporter au moins une ligne",
        });
    }

    // BR-CO-10 : somme des montants de ligne = BT-106.
    let somme_lignes: Decimal = facture.lignes.iter().map(|l| l.montant_ht).sum();
    if somme_lignes.round_dp(2) != facture.total_ht.round_dp(2) {
        v.push(Violation {
            reference: "BR-CO-10",
            libelle: "la somme des lignes ne vaut pas le total HT",
        });
    }

    // BR-CO-14 : total TVA = somme des TVA par categorie.
    let ventilations = ventiler(&facture.lignes);
    let somme_tva: Decimal = ventilations.iter().map(|t| t.montant).sum();
    if somme_tva.round_dp(2) != facture.total_tva.round_dp(2) {
        v.push(Violation {
            reference: "BR-CO-14",
            libelle: "le total de TVA ne vaut pas la somme des ventilations",
        });
    }

    // BR-CO-15 : TTC = HT + TVA.
    if (facture.total_ht + facture.total_tva).round_dp(2) != facture.total_ttc.round_dp(2) {
        v.push(Violation {
            reference: "BR-CO-15",
            libelle: "le total TTC ne vaut pas HT + TVA",
        });
    }

    // BR-DEC-01 et suivantes : deux decimales au plus sur les montants.
    let deux_decimales = |m: Decimal| m == m.round_dp(2);
    if !deux_decimales(facture.total_ht)
        || !deux_decimales(facture.total_tva)
        || !deux_decimales(facture.total_ttc)
    {
        v.push(Violation {
            reference: "BR-DEC",
            libelle: "un montant porte plus de deux decimales",
        });
    }

    // BR-CL-03 : la devise est un code ISO 4217 (trois lettres).
    let devise = facture.devise.trim();
    if !devise.is_empty() && !(devise.len() == 3 && devise.chars().all(|c| c.is_ascii_uppercase()))
    {
        v.push(Violation {
            reference: "BR-CL-03",
            libelle: "la devise doit etre un code ISO 4217",
        });
    }

    // BR-S-08 : dans la categorie « S », une ventilation par taux, avec un
    // taux positif. Un taux negatif n'a pas de sens.
    for ligne in &facture.lignes {
        if ligne.taux_tva < Decimal::ZERO {
            v.push(Violation {
                reference: "BR-S-08",
                libelle: "un taux de TVA ne peut pas etre negatif",
            });
            break;
        }
    }

    // Une quantite nulle n'a pas de sens dans une facture.
    if facture.lignes.iter().any(|l| l.quantite <= Decimal::ZERO) {
        v.push(Violation {
            reference: "BR-21",
            libelle: "une ligne porte une quantite nulle ou negative",
        });
    }

    // Une exoneration sans code ni motif ne dit pas pourquoi : c'est
    // exactement ce que BT-120 et BT-121 servent a eviter (BR-E-10, BR-AE-10).
    for ventilation in &ventilations {
        if let Some(e) = &ventilation.exoneration {
            let sans_code = e.code.trim().is_empty();
            let sans_motif = e
                .motif
                .as_deref()
                .map(|m| m.trim().is_empty())
                .unwrap_or(true);
            if sans_code && sans_motif {
                v.push(Violation {
                    reference: "BR-E-10",
                    libelle: "une exoneration doit porter un code ou un motif",
                });
                break;
            }
        }
    }

    // Une ligne exoneree avec un taux non nul se contredit : soit la TVA est
    // due, soit elle ne l'est pas. Ce n'est pas une regle BR numerotee, c'est
    // une coherence de la donnee ; on le dit sous notre propre etiquette.
    if facture
        .lignes
        .iter()
        .any(|l| l.exoneration.is_some() && l.taux_tva != Decimal::ZERO)
    {
        v.push(Violation {
            reference: "VATEX",
            libelle: "une ligne exoneree porte un taux de TVA non nul",
        });
    }

    // Une operation hors du champ de la TVA ne se melange pas : la norme veut
    // une facture a part. Melanger obligerait a declarer un identifiant de TVA
    // sur une piece qui ne peut pas en porter.
    let categories: std::collections::BTreeSet<&'static str> = ventilations
        .iter()
        .map(|v| v.exoneration.as_ref().map(|e| e.categorie()).unwrap_or("S"))
        .collect();
    if categories.contains("O") && categories.len() > 1 {
        v.push(Violation {
            reference: "VATEX-O",
            libelle: "une operation hors champ de la TVA ne se melange pas a une operation taxee ou exoneree",
        });
    }

    // BR-06 / BR-07 : les noms sont obligatoires.
    if facture.vendeur.nom.trim().is_empty() {
        v.push(Violation {
            reference: "BR-06",
            libelle: "le vendeur doit etre nomme",
        });
    }
    if facture.acheteur.nom.trim().is_empty() {
        v.push(Violation {
            reference: "BR-07",
            libelle: "l'acheteur doit etre nomme",
        });
    }

    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn facture() -> Facture {
        Facture {
            numero: "FA2611-0001".into(),
            date: "2026-11-04".into(),
            echeance: Some("2026-12-04".into()),
            vendeur: Partie {
                nom: "Pierre Angulaire".into(),
                siret: Some("12345678900011".into()),
                tva: Some("FR12345678900".into()),
                rue: Some("1 rue du Four".into()),
                code_postal: Some("69390".into()),
                ville: Some("Charly".into()),
                pays: None,
            },
            acheteur: Partie {
                nom: "M. Martin".into(),
                siret: None,
                tva: None,
                rue: Some("2 chemin des Vignes".into()),
                code_postal: Some("69003".into()),
                ville: Some("Lyon".into()),
                pays: None,
            },
            lignes: vec![
                Ligne {
                    designation: "Pavé granit 14x14".into(),
                    quantite: d("2"),
                    unite: Some("C62".into()),
                    prix_unitaire_ht: d("500"),
                    taux_tva: d("20"),
                    montant_ht: d("1000"),
                    exoneration: None,
                },
                Ligne {
                    designation: "Bordure T3".into(),
                    quantite: d("1"),
                    unite: Some("MTR".into()),
                    prix_unitaire_ht: d("200"),
                    taux_tva: d("10"),
                    montant_ht: d("200"),
                    exoneration: None,
                },
            ],
            total_ht: d("1200"),
            total_tva: d("220"),
            total_ttc: d("1420"),
            devise: "EUR".into(),
            moyen_paiement: Some("30".into()),
            iban: Some("FR1420041010050500013M02606".into()),
            processus: Some("A1".into()),
        }
    }

    #[test]
    fn le_xml_porte_les_mentions_obligatoires() {
        let x = xml(&facture(), Profil::En16931).unwrap();
        assert!(x.contains("<rsm:CrossIndustryInvoice"));
        assert!(x.contains("<ram:ID>FA2611-0001</ram:ID>"));
        assert!(
            x.contains("<ram:TypeCode>380</ram:TypeCode>"),
            "facture = 380"
        );
        assert!(x.contains("<udt:DateTimeString format=\"102\">20261104"));
        assert!(x.contains("<ram:Name>Pierre Angulaire</ram:Name>"));
        assert!(x.contains("<ram:ID schemeID=\"0009\">12345678900011</ram:ID>"));
        assert!(x.contains("<ram:ID schemeID=\"VA\">FR12345678900</ram:ID>"));
        assert!(x.contains("<ram:InvoiceCurrencyCode>EUR</ram:InvoiceCurrencyCode>"));
    }

    #[test]
    fn la_tva_est_ventilee_par_taux() {
        let x = xml(&facture(), Profil::En16931).unwrap();
        // 1 000 à 20 % = 200 ; 200 à 10 % = 20 ; total 220.
        assert_eq!(ventiler(&facture().lignes).len(), 2);
        assert!(x.contains("<ram:RateApplicablePercent>20.00</ram:RateApplicablePercent>"));
        assert!(x.contains("<ram:RateApplicablePercent>10.00</ram:RateApplicablePercent>"));
        assert!(x.contains("<ram:TaxTotalAmount currencyID=\"EUR\">220.00</ram:TaxTotalAmount>"));
        assert!(x.contains("<ram:GrandTotalAmount>1420.00</ram:GrandTotalAmount>"));
    }

    #[test]
    fn les_caracteres_speciaux_sont_echappes() {
        let mut f = facture();
        f.acheteur.nom = "Dupont & Fils <Travaux>".into();
        let x = xml(&f, Profil::Basic).unwrap();
        assert!(x.contains("Dupont &amp; Fils &lt;Travaux&gt;"));
        assert!(!x.contains("<Travaux>"), "le chevron doit etre echappe");
    }

    #[test]
    fn un_total_qui_ne_vaut_pas_ses_lignes_est_refuse() {
        let mut f = facture();
        f.total_ht = d("1300");
        match xml(&f, Profil::Basic) {
            Err(Erreur::TotauxIncoherents(m)) => assert!(m.contains("1200")),
            autre => panic!("refus attendu, obtenu {autre:?}"),
        }
    }

    #[test]
    fn une_tva_qui_ne_vaut_pas_sa_ventilation_est_refusee() {
        let mut f = facture();
        f.total_tva = d("250");
        assert!(matches!(
            xml(&f, Profil::Basic),
            Err(Erreur::TotauxIncoherents(_))
        ));
    }

    #[test]
    fn le_xml_ne_manque_d_aucun_element_obligatoire() {
        let x = xml(&facture(), Profil::En16931).unwrap();
        let manquants = verifier_norme(&x);
        assert!(manquants.is_empty(), "elements manquants : {:?}", manquants);
    }

    #[test]
    fn une_facture_juste_ne_viole_aucune_regle() {
        let v = regles_metier(&facture());
        assert!(v.is_empty(), "violations : {:?}", v);
    }

    #[test]
    fn une_facture_fausse_est_rejetee_par_les_regles() {
        // BR-CO-15 : TTC incoherent.
        let mut f = facture();
        f.total_ttc = d("1300");
        assert!(regles_metier(&f).iter().any(|v| v.reference == "BR-CO-15"));

        // BR-CO-10 : somme des lignes fausse.
        let mut f2 = facture();
        f2.total_ht = d("900");
        let v2 = regles_metier(&f2);
        assert!(v2.iter().any(|v| v.reference == "BR-CO-10"));
        // et le TTC devient faux aussi, par ricochet : les deux sont signalees.
        assert!(v2.iter().any(|v| v.reference == "BR-CO-15"));

        // BR-CL-03 : devise qui n'est pas un code ISO.
        let mut f3 = facture();
        f3.devise = "euros".into();
        assert!(regles_metier(&f3).iter().any(|v| v.reference == "BR-CL-03"));

        // BR-DEC : trois decimales.
        let mut f4 = facture();
        f4.total_ht = d("1000.005");
        assert!(regles_metier(&f4).iter().any(|v| v.reference == "BR-DEC"));
    }

    fn exonere(code: &str) -> Exoneration {
        Exoneration {
            code: code.into(),
            motif: None,
        }
    }

    #[test]
    fn le_code_vatex_impose_la_categorie() {
        assert_eq!(exonere("VATEX-EU-132").categorie(), "E");
        assert_eq!(exonere("VATEX-FR-FRANCHISE").categorie(), "E");
        assert_eq!(exonere("VATEX-EU-AE").categorie(), "AE");
        assert_eq!(exonere("VATEX-FR-AUTOLIQ").categorie(), "AE");
        assert_eq!(exonere("VATEX-EU-IC").categorie(), "K");
        assert_eq!(exonere("VATEX-EU-G").categorie(), "G");
        assert_eq!(exonere("VATEX-EU-O").categorie(), "O");
        // Un code inconnu tombe en « exonere », la categorie la plus prudente.
        assert_eq!(exonere("VATEX-FR-QUELQUE-CHOSE").categorie(), "E");
    }

    /// Une ligne exoneree sort en categorie E, taux 0, avec ses deux elements.
    #[test]
    fn l_exoneration_sort_dans_le_xml_et_change_la_categorie() {
        let mut f = facture();
        f.lignes = vec![Ligne {
            designation: "Travaux de renovation".into(),
            quantite: d("1"),
            unite: Some("C62".into()),
            prix_unitaire_ht: d("1000"),
            taux_tva: d("0"),
            montant_ht: d("1000"),
            exoneration: Some(Exoneration {
                code: "VATEX-FR-FRANCHISE".into(),
                motif: Some("Franchise en base de TVA, article 293 B du CGI".into()),
            }),
        }];
        f.total_ht = d("1000");
        f.total_tva = d("0");
        f.total_ttc = d("1000");

        let x = xml(&f, Profil::En16931).expect("xml");
        assert!(
            x.contains("<ram:ExemptionReasonCode>VATEX-FR-FRANCHISE</ram:ExemptionReasonCode>"),
            "{x}"
        );
        assert!(x.contains("article 293 B du CGI"), "{x}");
        assert!(x.contains("<ram:CategoryCode>E</ram:CategoryCode>"), "{x}");
        assert!(
            !x.contains("<ram:CategoryCode>S</ram:CategoryCode>"),
            "aucune ligne ne doit rester en categorie S : {x}"
        );
        // Le XML reste conforme aux elements obligatoires.
        assert!(verifier_norme(&x).is_empty(), "{:?}", verifier_norme(&x));
        assert!(regles_metier(&f).is_empty(), "{:?}", regles_metier(&f));
    }

    /// Deux motifs differents au meme taux sont deux ventilations, pas une.
    #[test]
    fn deux_motifs_au_meme_taux_ne_fusionnent_pas() {
        let mut f = facture();
        f.lignes = vec![
            Ligne {
                designation: "Exonere A".into(),
                quantite: d("1"),
                unite: None,
                prix_unitaire_ht: d("100"),
                taux_tva: d("0"),
                montant_ht: d("100"),
                exoneration: Some(exonere("VATEX-EU-132")),
            },
            Ligne {
                designation: "Exonere B".into(),
                quantite: d("1"),
                unite: None,
                prix_unitaire_ht: d("200"),
                taux_tva: d("0"),
                montant_ht: d("200"),
                exoneration: Some(exonere("VATEX-EU-IC")),
            },
        ];
        let ventilations = ventiler(&f.lignes);
        assert_eq!(ventilations.len(), 2, "deux motifs, deux ventilations");
        assert!(ventilations
            .iter()
            .any(|v| v.exoneration.as_ref().unwrap().categorie() == "E"));
        assert!(ventilations
            .iter()
            .any(|v| v.exoneration.as_ref().unwrap().categorie() == "K"));
    }

    #[test]
    fn une_exoneration_sans_code_ni_motif_est_signalee() {
        let mut f = facture();
        f.lignes[0].exoneration = Some(Exoneration {
            code: "  ".into(),
            motif: None,
        });
        f.lignes[0].taux_tva = d("0");
        let violations = regles_metier(&f);
        assert!(
            violations.iter().any(|v| v.reference == "BR-E-10"),
            "{violations:?}"
        );
    }

    #[test]
    fn une_ligne_exoneree_avec_un_taux_non_nul_est_signalee() {
        let mut f = facture();
        f.lignes[0].exoneration = Some(exonere("VATEX-EU-132"));
        // 20 % conserve : la donnee se contredit.
        let violations = regles_metier(&f);
        assert!(
            violations.iter().any(|v| v.reference == "VATEX"),
            "{violations:?}"
        );
    }

    /// Une operation hors champ de la TVA ne porte aucun identifiant de TVA,
    /// garde son SIREN, et reste conforme aux elements obligatoires.
    #[test]
    fn une_operation_hors_champ_ne_porte_aucun_identifiant_de_tva() {
        let mut f = facture();
        f.lignes = vec![Ligne {
            designation: "Livraison hors de France".into(),
            quantite: d("1"),
            unite: Some("C62".into()),
            prix_unitaire_ht: d("1000"),
            taux_tva: d("0"),
            montant_ht: d("1000"),
            exoneration: Some(exonere("VATEX-EU-O")),
        }];
        f.total_ht = d("1000");
        f.total_tva = d("0");
        f.total_ttc = d("1000");

        let x = xml(&f, Profil::En16931).expect("xml");
        assert!(x.contains("<ram:CategoryCode>O</ram:CategoryCode>"), "{x}");
        assert!(
            !x.contains(r#"schemeID="VA""#),
            "un identifiant de TVA est interdit ici : {x}"
        );
        // Le SIREN reste : c'est lui qui identifie la partie.
        assert!(x.contains(r#"schemeID="0009""#), "{x}");
        // BT-31 est ecarte du controle : il est interdit, pas manquant.
        assert!(verifier_norme(&x).is_empty(), "{:?}", verifier_norme(&x));
        assert!(regles_metier(&f).is_empty(), "{:?}", regles_metier(&f));
    }

    /// Hors champ et taxe sur la meme facture : refuse, la norme veut deux pieces.
    #[test]
    fn melanger_hors_champ_et_taxe_est_signale() {
        let mut f = facture();
        f.lignes.push(Ligne {
            designation: "Prestation hors champ".into(),
            quantite: d("1"),
            unite: None,
            prix_unitaire_ht: d("100"),
            taux_tva: d("0"),
            montant_ht: d("100"),
            exoneration: Some(exonere("VATEX-EU-O")),
        });
        let violations = regles_metier(&f);
        assert!(
            violations.iter().any(|v| v.reference == "VATEX-O"),
            "{violations:?}"
        );
    }

    #[test]
    fn un_xml_vide_manque_de_tout() {
        let manquants = verifier_norme("");
        assert_eq!(manquants.len(), OBLIGATOIRES.len());
        assert_eq!(manquants[0].reference, "BT-1");
    }

    #[test]
    fn les_mentions_manquantes_sont_refusees() {
        let mut f = facture();
        f.numero = "  ".into();
        assert_eq!(xml(&f, Profil::Basic), Err(Erreur::NumeroVide));

        let mut f2 = facture();
        f2.date = "04/11/2026".into();
        assert!(matches!(
            xml(&f2, Profil::Basic),
            Err(Erreur::DateInvalide(_))
        ));

        let mut f3 = facture();
        f3.lignes.clear();
        assert_eq!(xml(&f3, Profil::Basic), Err(Erreur::AucuneLigne));
    }
}
