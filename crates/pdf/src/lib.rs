//! Rendu PDF des documents commerciaux (devis, commande, facture, avoir).
//!
//! Crate volontairement isolee de `db` et de `money` : elle ne connait que les
//! structures de vue definies ici — des chaines **deja formatees** — ce qui
//! permet de la tester sans base de donnees ni arithmetique monetaire.
//!
//! # Approche
//!
//! La composition est faite par [Typst](https://typst.app) en memoire
//! (`typst::compile` + `typst_pdf::pdf`) : aucun binaire externe n'est appele,
//! aucun PDF n'est ecrit a la main. La mise en page vit dans un gabarit
//! versionne, `templates/document.typ`, embarque dans le binaire par
//! `include_str!` et parametre par un dictionnaire Typst `data` genere depuis
//! [`DocumentView`].
//!
//! La police (DejaVu Sans, regular + gras) est embarquee dans le binaire et
//! donc **incluse dans le PDF** : le fichier produit est autosuffisant.
//!
//! # Exemple
//!
//! ```
//! use pdf::{DocKind, DocumentView, LineView, PartyView, render};
//!
//! let doc = DocumentView {
//!     kind: DocKind::Quote,
//!     title_override: None,
//!     reference: "PROV612".into(),
//!     date: "16/10/2026".into(),
//!     due_date: None,
//!     valid_until: Some("16/11/2026".into()),
//!     customer_ref: Some("BC-2026-114".into()),
//!     service_date: Some("12/10/2026".into()),
//!     bank: Some("IBAN FR76 1234 5678 9012 3456 7890 123".into()),
//!     payment_terms: Some("30 jours fin de mois".into()),
//!     deposit: Some("30 % — 1 526,14 €".into()),
//!     seller: PartyView {
//!         legal_line: None,
//!         name: "Pierre Angulaire".into(),
//!         address: Some("12 rue des Peupliers".into()),
//!         zip: Some("69450".into()),
//!         town: Some("Lyon".into()),
//!         siret: Some("123 456 789 00016".into()),
//!         tva_intra: Some("FR12345678900".into()),
//!     },
//!     customer: PartyView {
//!         legal_line: None,
//!         name: "M. et Mme Martin".into(),
//!         address: None,
//!         zip: None,
//!         town: None,
//!         siret: None,
//!         tva_intra: None,
//!     },
//!     lines: vec![LineView {
//!         kind: pdf::LineKind::Normal,
//!         label: "Reprise de contour de piscine".into(),
//!         description: None,
//!         qty: "1".into(),
//!         unit: "forfait".into(),
//!         unit_price_ht: "4 624,65 EUR".into(),
//!         discount_percent: "0 %".into(),
//!         vat_rate: "10 %".into(),
//!         total_ht: "4 624,65 EUR".into(),
//!     }],
//!     total_ht: "4 624,65 EUR".into(),
//!     total_vat: "462,47 EUR".into(),
//!     total_ttc: "5 087,12 EUR".into(),
//!     vat_breakdown: vec![("10 %".into(), "462,47 EUR".into())],
//!     mentions: vec!["Devis valable 1 mois.".into()],
//!     cgv: None,
//!     notes: None,
//! };
//!
//! let bytes = render(&doc).expect("rendu PDF");
//! assert!(bytes.starts_with(b"%PDF-"));
//! ```

#![forbid(unsafe_code)]

use std::fmt;
use std::sync::OnceLock;

use typst::diag::{EcoVec, FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

/// Gabarit Typst versionne, embarque dans le binaire.
const TEMPLATE: &str = include_str!("../templates/document.typ");

/// Police du texte courant (regular). Licence DejaVu (redistribution libre).
const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");

/// Police du texte courant (gras).
const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");

/// Nom du fichier virtuel presente a Typst (apparait dans les messages
/// d'erreur de compilation).
const MAIN_FILE: &str = "document.typ";

// ---------------------------------------------------------------------------
// Modele de vue
// ---------------------------------------------------------------------------

/// Nature du document commercial rendu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocKind {
    /// Devis (`propal`).
    Quote,
    /// Commande client (`commande`).
    Order,
    /// Facture (`facture`).
    Invoice,
    /// Avoir / note de credit (`facture rec.`).
    CreditNote,
}

impl DocKind {
    /// Libelle francais affiche en tete du document.
    pub fn label(self) -> &'static str {
        match self {
            Self::Quote => "Devis",
            Self::Order => "Commande",
            Self::Invoice => "Facture",
            Self::CreditNote => "Avoir",
        }
    }
}

/// Identite d'une partie (vendeur ou client).
///
/// Tous les champs autres que `name` sont optionnels : un particulier n'a ni
/// SIRET ni numero de TVA, et l'adresse peut etre inconnue.
#[derive(Debug, Clone, Default)]
pub struct PartyView {
    pub name: String,
    pub address: Option<String>,
    pub zip: Option<String>,
    pub town: Option<String>,
    pub siret: Option<String>,
    pub tva_intra: Option<String>,
    /// Identification legale affichee sous le nom : forme juridique, capital, RCS.
    pub legal_line: Option<String>,
}

/// Nature d'une ligne dans le tableau.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineKind {
    #[default]
    Normal,
    /// Titre de section : bande pleine largeur, sans montant.
    Section,
    /// Sous-total de section : libelle a gauche, montant aligne a droite.
    Subtotal,
}

/// Une ligne de document, integralement pre-formatee par l'appelant.
#[derive(Debug, Clone, Default)]
pub struct LineView {
    /// Nature de la ligne. `Normal` pour une prestation.
    pub kind: LineKind,
    pub label: String,
    pub description: Option<String>,
    /// Quantite deja formatee (ex. `"2,00"`).
    pub qty: String,
    /// Unite de la ligne (ex. `"m²"`, `"forfait"`), deja lisible.
    pub unit: String,
    /// Prix unitaire hors taxes deja formate, symbole monetaire inclus.
    pub unit_price_ht: String,
    /// Remise en pourcentage deja formatee (ex. `"5,00 %"`).
    pub discount_percent: String,
    /// Taux de TVA deja formate (ex. `"20,00 %"`).
    pub vat_rate: String,
    /// Total hors taxes de la ligne, deja formate.
    pub total_ht: String,
}

/// Vue complete d'un document commercial, prete a etre composee.
///
/// Aucun champ n'est calcule ici : les montants arrivent en chaines, ce qui
/// garantit que l'arithmetique de `money` reste la seule source de verite.
#[derive(Debug, Clone)]
pub struct DocumentView {
    pub kind: DocKind,
    /// Remplace le libelle du type quand le document est particulier
    /// (« Facture de situation n° 2 »).
    pub title_override: Option<String>,
    pub reference: String,
    pub date: String,
    pub due_date: Option<String>,
    pub valid_until: Option<String>,
    /// Bon de commande du client, s'il a été établi.
    pub customer_ref: Option<String>,
    /// Date de la vente ou de la prestation.
    pub service_date: Option<String>,
    /// Coordonnées bancaires : IBAN et BIC, sur une ligne.
    pub bank: Option<String>,
    /// Condition de règlement, telle qu'elle doit apparaître au client
    /// (ex. « 30 jours fin de mois »).
    pub payment_terms: Option<String>,
    /// Acompte demandé à la commande, déjà formaté (ex. « 30,00 % — 1 234,56 € TTC »).
    pub deposit: Option<String>,
    pub seller: PartyView,
    pub customer: PartyView,
    pub lines: Vec<LineView>,
    pub total_ht: String,
    pub total_vat: String,
    pub total_ttc: String,
    /// Ventilation de la TVA : `(taux deja formate, montant deja formate)`.
    pub vat_breakdown: Vec<(String, String)>,
    /// Mentions legales, placees en pied de page (une par ligne).
    pub mentions: Vec<String>,
    /// Conditions generales de vente, imprimees en fin de document.
    pub cgv: Option<String>,
    /// Note libre affichee sous les totaux (les `\n` sont conserves).
    pub notes: Option<String>,
}

/// Erreur de rendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfError {
    /// Le gabarit n'a pas pu etre compose (erreur Typst, avec messages).
    Compile(Vec<String>),
    /// La composition a reussi mais l'export PDF a echoue.
    Export(Vec<String>),
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(messages) => {
                write!(f, "echec de composition Typst : {}", messages.join(" | "))
            }
            Self::Export(messages) => {
                write!(f, "echec de l'export PDF : {}", messages.join(" | "))
            }
        }
    }
}

impl std::error::Error for PdfError {}

/// Resultat d'un rendu : les octets du PDF et le nombre de pages produites.
///
/// Expose en plus de [`render`] parce que la pagination est une information
/// utile a l'appelant (et verifiable par les tests sans re-analyser le PDF).
#[derive(Debug, Clone)]
pub struct RenderedPdf {
    /// Contenu binaire du fichier PDF.
    pub bytes: Vec<u8>,
    /// Nombre de pages effectivement composees.
    pub pages: usize,
}

// ---------------------------------------------------------------------------
// API publique
// ---------------------------------------------------------------------------

/// Compose le document en PDF et renvoie les octets du fichier.
///
/// Le rendu est deterministe : aucune date systeme, aucun identifiant aleatoire
/// n'est injecte dans le flux produit.
pub fn render(doc: &DocumentView) -> Result<Vec<u8>, PdfError> {
    Ok(render_document(doc)?.bytes)
}

/// Comme [`render`], mais renvoie aussi le nombre de pages composees.
pub fn render_document(doc: &DocumentView) -> Result<RenderedPdf, PdfError> {
    let document = compose(doc)?;
    let bytes = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|errors| PdfError::Export(messages(&errors)))?;
    Ok(RenderedPdf {
        bytes,
        pages: document.pages().len(),
    })
}

/// Compose seulement (sans exporter) : renvoie le document Typst pagine.
///
/// Destine aux outils internes (apercu PNG/SVG, comptage de pages) : ce point
/// d'entree expose volontairement un type Typst, il n'est pas documente.
#[doc(hidden)]
pub fn compose(doc: &DocumentView) -> Result<PagedDocument, PdfError> {
    let world = DocWorld::new(build_source(doc));
    let warned = typst::compile::<PagedDocument>(&world);
    warned
        .output
        .map_err(|errors| PdfError::Compile(messages(&errors)))
}

fn messages(errors: &EcoVec<SourceDiagnostic>) -> Vec<String> {
    errors.iter().map(|e| e.message.to_string()).collect()
}

// ---------------------------------------------------------------------------
// Monde Typst (polices + fichier source en memoire)
// ---------------------------------------------------------------------------

/// Bibliotheque standard Typst, construite une seule fois par processus.
fn library() -> &'static LazyHash<Library> {
    static LIBRARY: OnceLock<LazyHash<Library>> = OnceLock::new();
    LIBRARY.get_or_init(|| LazyHash::new(Library::default()))
}

/// Polices disponibles pour le compilateur.
struct Fonts {
    book: LazyHash<FontBook>,
    faces: Vec<Font>,
}

fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut book = FontBook::new();
        let mut faces = Vec::new();
        for data in [FONT_REGULAR, FONT_BOLD] {
            for face in Font::iter(Bytes::new(data)) {
                book.push(face.info().clone());
                faces.push(face);
            }
        }
        Fonts {
            book: LazyHash::new(book),
            faces,
        }
    })
}

/// Monde Typst minimal : un fichier source, deux polices, aucune horloge.
struct DocWorld {
    main: FileId,
    source: Source,
}

impl DocWorld {
    fn new(text: String) -> Self {
        let vpath = VirtualPath::new(MAIN_FILE).expect("chemin virtuel valide");
        let main = RootedPath::new(VirtualRoot::Project, vpath).intern();
        Self {
            main,
            source: Source::new(main, text),
        }
    }
}

impl World for DocWorld {
    fn library(&self) -> &LazyHash<Library> {
        library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &fonts().book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(MAIN_FILE.into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main {
            Ok(Bytes::from_string(self.source.text().to_string()))
        } else {
            Err(FileError::NotFound(MAIN_FILE.into()))
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        fonts().faces.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        // Volontairement `None` : le rendu ne doit dependre d'aucune horloge,
        // sans quoi les PDF ne seraient pas reproductibles.
        None
    }
}

// ---------------------------------------------------------------------------
// Generation de la source Typst
// ---------------------------------------------------------------------------

/// Concatene le dictionnaire `data` (syntaxe Typst) et le gabarit.
pub(crate) fn build_source(doc: &DocumentView) -> String {
    let mut source = String::with_capacity(TEMPLATE.len() + 8192);
    source.push_str("#let data = ");
    source.push_str(&document_dict(doc));
    source.push('\n');
    source.push_str(TEMPLATE);
    source
}

fn document_dict(doc: &DocumentView) -> String {
    let lines: Vec<String> = doc.lines.iter().map(line_dict).collect();
    let vat: Vec<String> = doc
        .vat_breakdown
        .iter()
        .map(|(rate, amount)| dict(&[("rate", typst_str(rate)), ("amount", typst_str(amount))]))
        .collect();
    let notes: Vec<String> = doc.notes.as_deref().map(text_lines).unwrap_or_default();
    let cgv: Vec<String> = doc.cgv.as_deref().map(text_lines).unwrap_or_default();

    dict(&[
        (
            "title",
            typst_str(doc.title_override.as_deref().unwrap_or(doc.kind.label())),
        ),
        ("reference", typst_str(&doc.reference)),
        ("date", typst_str(&doc.date)),
        ("due_date", opt_str(doc.due_date.as_deref())),
        ("customer_ref", opt_str(doc.customer_ref.as_deref())),
        ("service_date", opt_str(doc.service_date.as_deref())),
        ("bank", opt_str(doc.bank.as_deref())),
        ("payment_terms", opt_str(doc.payment_terms.as_deref())),
        ("deposit", opt_str(doc.deposit.as_deref())),
        ("valid_until", opt_str(doc.valid_until.as_deref())),
        ("seller", party_dict(&doc.seller)),
        ("customer", party_dict(&doc.customer)),
        ("lines", array(&lines)),
        ("total_ht", typst_str(&doc.total_ht)),
        ("total_vat", typst_str(&doc.total_vat)),
        ("total_ttc", typst_str(&doc.total_ttc)),
        ("vat_breakdown", array(&vat)),
        ("mentions", str_array(&doc.mentions)),
        ("notes_lines", str_array(&notes)),
        ("cgv_lines", str_array(&cgv)),
    ])
}

fn party_dict(party: &PartyView) -> String {
    let mut address = party.as_ref_lines();
    if let Some(locality) = locality(party) {
        address.push(locality);
    }
    dict(&[
        ("name", typst_str(&party.name)),
        ("address_lines", str_array(&address)),
        ("siret", opt_str(party.siret.as_deref())),
        ("tva_intra", opt_str(party.tva_intra.as_deref())),
        ("legal_line", opt_str(party.legal_line.as_deref())),
    ])
}

impl PartyView {
    /// Lignes d'adresse, `address` eclatee sur les retours a la ligne.
    fn as_ref_lines(&self) -> Vec<String> {
        self.address.as_deref().map(text_lines).unwrap_or_default()
    }
}

/// `"69450" + "Saint-Cyr"` -> `"69450 Saint-Cyr"` ; `None` si les deux sont vides.
fn locality(party: &PartyView) -> Option<String> {
    let zip = party.zip.as_deref().unwrap_or("").trim();
    let town = party.town.as_deref().unwrap_or("").trim();
    match (zip.is_empty(), town.is_empty()) {
        (true, true) => None,
        (true, false) => Some(town.to_string()),
        (false, true) => Some(zip.to_string()),
        (false, false) => Some(format!("{zip} {town}")),
    }
}

fn line_dict(line: &LineView) -> String {
    dict(&[
        ("kind", typst_str(kind_name(line.kind))),
        ("label", typst_str(&line.label)),
        ("description", opt_str(line.description.as_deref())),
        ("qty", typst_str(&line.qty)),
        ("unit", typst_str(&line.unit)),
        ("unit_price_ht", typst_str(&line.unit_price_ht)),
        ("discount_percent", typst_str(&line.discount_percent)),
        ("vat_rate", typst_str(&line.vat_rate)),
        ("total_ht", typst_str(&line.total_ht)),
    ])
}

fn kind_name(kind: LineKind) -> &'static str {
    match kind {
        LineKind::Normal => "normal",
        LineKind::Section => "section",
        LineKind::Subtotal => "subtotal",
    }
}

/// Decoupe un texte libre en lignes non vides et non vides d'espaces.
fn text_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

// --- petits serialiseurs vers la syntaxe Typst -----------------------------

/// Litteral de chaine Typst, echappe (`"`, `\`, sauts de ligne, tabulations).
fn typst_str(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// `Some("x")` -> `"x"`, chaine vide ou `None` -> `none`.
fn opt_str(value: Option<&str>) -> String {
    match value {
        Some(v) if !v.trim().is_empty() => typst_str(v),
        _ => "none".to_string(),
    }
}

/// Tableau Typst de chaines brutes : chaque element est echappe.
fn str_array(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| typst_str(s)).collect();
    array(&quoted)
}

/// Tableau Typst a partir d'elements deja serialises.
/// `()` pour vide, `(a,)` a un element, `(a, b)` au-dela.
fn array(items: &[String]) -> String {
    match items {
        [] => "()".to_string(),
        [only] => format!("({only},)"),
        _ => format!("({})", items.join(", ")),
    }
}

/// Dictionnaire Typst a cles identifiantes.
fn dict(fields: &[(&str, String)]) -> String {
    if fields.is_empty() {
        return "(:)".to_string();
    }
    let body: Vec<String> = fields
        .iter()
        .map(|(key, value)| format!("{key}: {value}"))
        .collect();
    format!("({})", body.join(", "))
}
