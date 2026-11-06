//! Apercu PNG du gabarit, page par page (controle visuel sans lecteur PDF).
//!
//! ```text
//! cargo run -p pdf --example apercu -- apercu 12
//! ```
//! Ecrit `apercu-1.png`, `apercu-2.png`, ... dans le dossier courant.

use pdf::{render_document, DocKind, DocumentView, LineView, PartyView};
use typst::utils::Scalar;

fn main() {
    let mut args = std::env::args().skip(1);
    let prefix = args.next().unwrap_or_else(|| "apercu".to_string());
    let rows: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(6);

    match render_document(&example(rows)) {
        Ok(rendered) => {
            println!(
                "{} page(s), {} octets",
                rendered.pages,
                rendered.bytes.len()
            );
            for (index, png) in pages_as_png(rows).enumerate() {
                let path = format!("{prefix}-{}.png", index + 1);
                match std::fs::write(&path, png) {
                    Ok(()) => println!("{path}"),
                    Err(err) => eprintln!("ecriture impossible : {err}"),
                }
            }
        }
        Err(err) => eprintln!("rendu impossible : {err}"),
    }
}

fn pages_as_png(rows: usize) -> impl Iterator<Item = Vec<u8>> {
    let document = pdf::compose(&example(rows)).expect("composition");
    let options = typst_render::RenderOptions {
        pixel_per_pt: Scalar::new(1.4),
        render_bleed: false,
    };
    let pixmaps: Vec<Vec<u8>> = document
        .pages()
        .iter()
        .filter_map(|page| typst_render::render(page, &options).encode_png().ok())
        .collect();
    pixmaps.into_iter()
}

fn example(rows: usize) -> DocumentView {
    let lines = (1..=rows)
        .map(|i| LineView {
            kind: Default::default(),
            label: format!("Poste {i} - reprise de maconnerie en pierre de taille"),
            description: if i % 2 == 0 {
                Some("Fourniture et pose, y compris evacuation des gravois.".to_string())
            } else {
                None
            },
            qty: "2,00".to_string(),
            unit: "m²".to_string(),
            unit_price_ht: "125,50 \u{20ac}".to_string(),
            discount_percent: "0,00 %".to_string(),
            vat_rate: "10,00 %".to_string(),
            total_ht: "251,00 \u{20ac}".to_string(),
        })
        .collect();

    DocumentView {
        kind: DocKind::Quote,
        title_override: None,
        reference: "PROV612".to_string(),
        date: "16/10/2026".to_string(),
        due_date: None,
        valid_until: Some("16/11/2026".to_string()),
        customer_ref: None,
        service_date: None,
        bank: None,
        cgv: Some("Les marchandises voyagent aux risques et perils du destinataire.".into()),
        payment_terms: Some("30 jours fin de mois".into()),
        deposit: None,
        seller: PartyView {
            legal_line: None,
            name: "Pierre Angulaire".to_string(),
            address: Some("1 rue de l'Exemple".to_string()),
            zip: Some("69450".to_string()),
            town: Some("Lyon".to_string()),
            siret: Some("123 456 789 00016".to_string()),
            tva_intra: Some("FR12345678900".to_string()),
        },
        customer: PartyView {
            legal_line: None,
            name: "M. et Mme Dupont".to_string(),
            address: Some("12 rue des Lilas".to_string()),
            zip: Some("69370".to_string()),
            town: Some("Saint-Didier-au-Mont-d'Or".to_string()),
            siret: None,
            tva_intra: None,
        },
        lines,
        total_ht: "1 506,00 \u{20ac}".to_string(),
        total_vat: "150,60 \u{20ac}".to_string(),
        total_ttc: "1 656,60 \u{20ac}".to_string(),
        vat_breakdown: vec![("10,00 %".to_string(), "150,60 \u{20ac}".to_string())],
        mentions: vec![
            "Devis valable un mois a compter de sa date d'emission.".to_string(),
            "TVA acquittee sur les encaissements.".to_string(),
            "Travaux realises selon les normes NF DTU en vigueur.".to_string(),
        ],
        notes: Some(
            "Acompte de 30 % a la commande.\nSolde a la reception des travaux.".to_string(),
        ),
        margins: Default::default(),
    }
}
