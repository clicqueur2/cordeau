use std::str::FromStr;

use facturx::{attacher, extraire, xml, Facture, Ligne, Partie, Profil};
use rust_decimal::Decimal;

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
            nom: "M. Charcosset".into(),
            siret: None,
            tva: None,
            rue: Some("2 chemin des Vignes".into()),
            code_postal: Some("69003".into()),
            ville: Some("Lyon".into()),
            pays: None,
        },
        lignes: vec![Ligne {
            designation: "Pavé granit 14x14".into(),
            quantite: d("2"),
            unite: Some("C62".into()),
            prix_unitaire_ht: d("500"),
            taux_tva: d("20"),
            montant_ht: d("1000"),
            exoneration: None,
        }],
        total_ht: d("1000"),
        total_tva: d("200"),
        total_ttc: d("1200"),
        devise: "EUR".into(),
        moyen_paiement: Some("30".into()),
        iban: Some("FR7616807003483848689521643".into()),
        processus: Some("A1".into()),
    }
}

/// Un PDF minimal, produit par lopdf lui-meme : une page vide suffit a porter
/// un fichier joint.
fn pdf_minimal() -> Vec<u8> {
    use lopdf::{dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let contenu = Stream::new(dictionary! {}, b"".to_vec());
    let contenu_id = doc.add_object(contenu);
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => Object::Reference(contenu_id),
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        }),
    );
    let catalogue = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalogue));
    let mut sortie = Vec::new();
    doc.save_to(&mut sortie).unwrap();
    sortie
}

/// Le XML part dans le PDF, et on le relit.
///
/// C'est la seule chose que ce test prouve : le fichier joint existe, il
/// contient le XML, et le PDF reste lisible. La conformite PDF/A-3 n'est pas
/// verifiee — elle demande un validateur externe.
#[test]
fn le_xml_est_joint_au_pdf_et_se_relit() {
    let x = xml(&facture(), Profil::En16931).unwrap();
    let pdf = pdf_minimal();
    assert!(pdf.starts_with(b"%PDF"), "le PDF de depart est valide");

    let facturx = attacher(&pdf, &x).expect("attachement");
    assert!(facturx.starts_with(b"%PDF"), "le PDF produit reste un PDF");

    let relu = extraire(&facturx).expect("extraction");
    assert_eq!(relu, x, "le XML relu doit etre identique a l'original");
    assert!(relu.contains("FA2611-0001"));
    assert!(relu.contains("CrossIndustryInvoice"));
}

/// Un PDF sans fichier joint le dit, au lieu de rendre du vide.
#[test]
fn un_pdf_sans_fichier_joint_est_signale() {
    let pdf = pdf_minimal();
    assert!(extraire(&pdf).is_err());
}

/// Un binaire qui n'est pas un PDF est refuse proprement.
#[test]
fn un_faux_pdf_est_refuse() {
    assert!(attacher(b"pas un pdf", "<xml/>").is_err());
}
