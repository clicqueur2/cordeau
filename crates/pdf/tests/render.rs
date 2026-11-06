//! Tests de rendu : validite du PDF, pagination, determinisme.

use pdf::{render, render_document, DocKind, DocumentView, LineView, PartyView};

fn seller() -> PartyView {
    PartyView {
        legal_line: None,
        name: "Pierre Angulaire".into(),
        address: Some("1 rue de l'Exemple".into()),
        zip: Some("69450".into()),
        town: Some("Saint-Cyr-au-Mont-d'Or".into()),
        siret: Some("123 456 789 00016".into()),
        siren: None,
        ape: None,
        tva_intra: Some("FR12345678900".into()),
    }
}

fn customer() -> PartyView {
    PartyView {
        legal_line: None,
        name: "M. et Mme Dupont".into(),
        address: Some("12 rue des Lilas".into()),
        zip: Some("69370".into()),
        town: Some("Saint-Didier-au-Mont-d'Or".into()),
        siret: None,
        siren: None,
        ape: None,
        tva_intra: None,
    }
}

/// Document de `rows` lignes, volontairement representatif (accents, remise,
/// description sur certaines lignes).
fn sample(rows: usize) -> DocumentView {
    let lines = (1..=rows)
        .map(|i| LineView {
            kind: Default::default(),
            label: format!("Poste {i} — reprise de maçonnerie en pierre de taille"),
            description: if i % 3 == 0 {
                Some("Fourniture et pose, y compris évacuation des gravois.".into())
            } else {
                None
            },
            qty: "2,00".into(),
            unit: "forfait".into(),
            unit_price_ht: "125,50 €".into(),
            discount_percent: "0,00 %".into(),
            vat_rate: "10,00 %".into(),
            total_ht: "251,00 €".into(),
        })
        .collect();

    DocumentView {
        kind: DocKind::Quote,
        title_override: None,
        reference: "PROV612".into(),
        date: "16/10/2026".into(),
        due_date: None,
        valid_until: Some("16/11/2026".into()),
        customer_ref: None,
        service_date: None,
        bank: None,
        cgv: Some("Les marchandises voyagent aux risques et perils du destinataire.".into()),
        payment_terms: Some("30 jours fin de mois".into()),
        deposit: None,
        seller: seller(),
        customer: customer(),
        lines,
        total_ht: "15 856,63 €".into(),
        total_vat: "1 585,66 €".into(),
        total_ttc: "17 442,29 €".into(),
        vat_breakdown: vec![("10,00 %".into(), "1 585,66 €".into())],
        mentions: vec![
            "Devis valable un mois à compter de sa date d'émission.".into(),
            "TVA acquittée sur les encaissements.".into(),
        ],
        notes: Some("Acompte de 30 % à la commande.\nSolde à la réception des travaux.".into()),
        margins: Default::default(),
    }
}

fn assert_valid_pdf(bytes: &[u8]) {
    assert!(
        bytes.starts_with(b"%PDF-"),
        "le fichier ne commence pas par %PDF-"
    );
    let tail = String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(32)..]);
    assert!(
        tail.contains("%%EOF"),
        "le fichier ne se termine pas par %%EOF"
    );
}

#[test]
fn devis_de_trois_lignes_produit_un_pdf_valide() {
    let rendered = render_document(&sample(3)).expect("le devis doit se composer");

    assert_valid_pdf(&rendered.bytes);
    assert!(
        rendered.bytes.len() > 2048,
        "PDF trop petit : {} octets",
        rendered.bytes.len()
    );
    assert_eq!(
        rendered.pages, 1,
        "un devis de 3 lignes doit tenir sur une page"
    );
}

#[test]
fn soixante_lignes_produit_plusieurs_pages() {
    let rendered = render_document(&sample(60)).expect("le document doit se composer");

    assert_valid_pdf(&rendered.bytes);
    assert!(
        rendered.pages >= 2,
        "60 lignes doivent deborder sur plusieurs pages"
    );
    assert!(
        rendered.bytes.len() > 4096,
        "PDF trop petit : {} octets",
        rendered.bytes.len()
    );
}

#[test]
fn rendu_deterministe() {
    let doc = sample(12);

    let first = render(&doc).expect("premier rendu");
    let second = render(&doc).expect("second rendu");

    assert_valid_pdf(&first);
    assert_valid_pdf(&second);

    let delta = first.len().abs_diff(second.len());
    assert!(
        delta <= 64,
        "tailles de PDF instables : {delta} octets d'ecart"
    );

    let pages_a = render_document(&doc).expect("rendu A").pages;
    let pages_b = render_document(&doc).expect("rendu B").pages;
    assert_eq!(pages_a, pages_b);
}

#[test]
fn document_minimal_sans_ligne_ni_mention() {
    let mut doc = sample(0);
    doc.lines.clear();
    doc.vat_breakdown.clear();
    doc.mentions.clear();
    doc.notes = None;
    doc.customer = PartyView {
        legal_line: None,
        name: "Client sans adresse".into(),
        ..PartyView::default()
    };

    let rendered = render_document(&doc).expect("un document vide doit rester composable");

    assert_valid_pdf(&rendered.bytes);
    assert_eq!(rendered.pages, 1);
}

#[test]
fn les_quatre_natures_de_document_se_composent() {
    for kind in [
        DocKind::Quote,
        DocKind::Order,
        DocKind::Invoice,
        DocKind::CreditNote,
    ] {
        let mut doc = sample(2);
        doc.kind = kind;
        doc.due_date = Some("30/11/2026".into());
        let bytes = render(&doc).expect("chaque nature doit se composer");
        assert_valid_pdf(&bytes);
    }
}

/// Le gabarit ne doit imprimer le RIB et les CGV qu'une seule fois : une
/// facture qui repete ses coordonnees bancaires est illisible (defaut constate
/// sur le modele de production, 3 RIB identiques).
#[test]
fn le_rib_et_les_cgv_s_impriment_une_seule_fois() {
    let gabarit = include_str!("../templates/document.typ");
    assert_eq!(
        gabarit.matches("data.bank").count(),
        2,
        "le RIB : un test de presence et une impression, rien de plus"
    );
    assert_eq!(
        gabarit.matches("data.cgv_lines").count(),
        2,
        "les CGV : un test de presence et une boucle, rien de plus"
    );
}
