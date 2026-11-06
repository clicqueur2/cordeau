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
//! aucun PDF n'est ecrit a la main. La mise en page vit dans des gabarits
//! versionnes de `templates/`, embarques dans le binaire par `include_str!` et
//! parametres par un dictionnaire Typst `data` genere depuis [`DocumentView`].
//! Chaque [`Modele`] designe son gabarit : `document.typ` pour le standard et
//! l'epure, `document_compact.typ` pour le compact.
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
//!         name: "M. et Mme Dupont".into(),
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
//!     margins: pdf::Marges::default(),
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

/// Gabarit Typst complet, embarque dans le binaire. Sert au standard et a
/// l'epure : ces deux modeles ne different que par les annexes, pas par la
/// mise en page.
const TEMPLATE_STANDARD: &str = include_str!("../templates/document.typ");

/// Gabarit Typst condense, embarque dans le binaire.
const TEMPLATE_COMPACT: &str = include_str!("../templates/document_compact.typ");

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
/// Marges de page, en millimetres.
///
/// Dolibarr les range dans `MAIN_PDF_MARGIN_LEFT`, `_RIGHT`, `_TOP`, `_BOTTOM`.
/// Le gabarit n'en connait que trois : la largeur, le haut et le bas. La gauche
/// et la droite sont donc ramenees a une seule valeur — la plus grande des
/// deux, parce qu'une marge trop petite se voit alors qu'une marge trop grande
/// ne fait que reduire un tableau.
///
/// Les valeurs par defaut sont celles du coeur Dolibarr, et non zero : un
/// document sans marge n'est pas imprimable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marges {
    pub x: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Default for Marges {
    fn default() -> Self {
        Self {
            x: 10.0,
            top: 10.0,
            bottom: 20.0,
        }
    }
}

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
    /// Marges de page, lues dans les constantes Dolibarr.
    pub margins: Marges,
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

/// Modele de mise en page d'un document.
///
/// Dolibarr en propose plusieurs par objet (`doctemplate`). Trois sont portes :
/// le standard, complet ; l'epure, qui retire les annexes longues en gardant la
/// mise en page du standard ; le compact, qui change de gabarit pour tenir sur
/// une page. Les **mentions legales restent dans les trois** : ce n'est pas une
/// option commerciale, c'est le droit francais.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modele {
    /// Document complet : conditions generales de vente et note libre.
    Standard,
    /// Document court : ni CGV ni note libre, mise en page du standard.
    Epure,
    /// Document dense : ni CGV ni note libre, gabarit condense (`document_compact.typ`).
    Compact,
}

/// Les modeles disponibles, dans l'ordre de presentation.
pub const MODELES: &[Modele] = &[Modele::Standard, Modele::Epure, Modele::Compact];

impl Modele {
    /// Identifiant stable, celui qu'on ecrit dans une constante.
    pub fn code(self) -> &'static str {
        match self {
            Modele::Standard => "standard",
            Modele::Epure => "epure",
            Modele::Compact => "compact",
        }
    }

    /// Libelle lisible.
    pub fn label(self) -> &'static str {
        match self {
            Modele::Standard => "Standard (avec CGV)",
            Modele::Epure => "Epure (sans CGV)",
            Modele::Compact => "Compact (une page, sans CGV)",
        }
    }

    /// Retrouve un modele par son code. Une valeur inconnue rend `None` :
    /// l'appelant retombe alors sur le standard.
    pub fn depuis_code(code: &str) -> Option<Self> {
        MODELES
            .iter()
            .copied()
            .find(|m| m.code().eq_ignore_ascii_case(code.trim()))
    }

    /// Comme [`Modele::depuis_code`], mais retombe sur le standard au lieu de
    /// `None`. C'est le comportement attendu d'une constante de configuration :
    /// une valeur inconnue ne doit jamais empecher d'editer un document.
    pub fn depuis_code_ou_standard(code: &str) -> Self {
        Self::depuis_code(code).unwrap_or(Modele::Standard)
    }

    /// Gabarit Typst associe au modele.
    ///
    /// L'epure partage le gabarit du standard : il n'en retire que les annexes,
    /// le texte et la pagination restent identiques.
    pub(crate) fn template(self) -> &'static str {
        match self {
            Modele::Standard | Modele::Epure => TEMPLATE_STANDARD,
            Modele::Compact => TEMPLATE_COMPACT,
        }
    }

    /// Ce que le modele change dans le document avant composition.
    ///
    /// On ne touche ni aux lignes, ni aux totaux, ni aux mentions legales :
    /// seule l'annexe commerciale disparait. La mise en page, elle, est choisie
    /// par [`Modele::template`].
    pub fn preparer(self, doc: &mut DocumentView) {
        if matches!(self, Modele::Epure | Modele::Compact) {
            doc.cgv = None;
            doc.notes = None;
        }
    }
}

/// Compose le document en PDF et renvoie les octets du fichier.
///
/// Le rendu est deterministe : aucune date systeme, aucun identifiant aleatoire
/// n'est injecte dans le flux produit.
pub fn render(doc: &DocumentView) -> Result<Vec<u8>, PdfError> {
    Ok(render_document(doc)?.bytes)
}

/// Comme [`render_document`], avec un modele de mise en page.
///
/// Le document recu n'est pas modifie : le modele s'applique sur une copie,
/// pour qu'un appelant puisse enchainer deux rendus avec deux modeles.
pub fn render_avec_modele(doc: &DocumentView, modele: Modele) -> Result<RenderedPdf, PdfError> {
    let mut copie = doc.clone();
    modele.preparer(&mut copie);
    let document = compose_avec_modele(&copie, modele)?;
    exporter(&document)
}

/// Comme [`render`], mais renvoie aussi le nombre de pages composees.
///
/// Rendu avec le modele [`Modele::Standard`] : pour un autre modele, passer par
/// [`render_avec_modele`].
pub fn render_document(doc: &DocumentView) -> Result<RenderedPdf, PdfError> {
    exporter(&compose(doc)?)
}

fn exporter(document: &PagedDocument) -> Result<RenderedPdf, PdfError> {
    let bytes = typst_pdf::pdf(document, &typst_pdf::PdfOptions::default())
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
    compose_avec_modele(doc, Modele::Standard)
}

/// Comme [`compose`], avec un modele : c'est le gabarit du modele qui sert.
#[doc(hidden)]
pub fn compose_avec_modele(doc: &DocumentView, modele: Modele) -> Result<PagedDocument, PdfError> {
    let world = DocWorld::new(build_source(doc, modele));
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

/// Concatene le dictionnaire `data` (syntaxe Typst) et le gabarit du modele.
pub(crate) fn build_source(doc: &DocumentView, modele: Modele) -> String {
    let template = modele.template();
    let mut source = String::with_capacity(template.len() + 8192);
    source.push_str("#let data = ");
    source.push_str(&document_dict(doc));
    source.push('\n');
    source.push_str(template);
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
        // Les marges viennent des constantes Dolibarr, en millimetres. Le
        // gabarit les multiplie par `1mm` : on reste dans l'unite naturelle du
        // reglage, celle que l'administrateur saisit.
        (
            "margins",
            dict(&[
                ("x", typst_num(doc.margins.x)),
                ("top", typst_num(doc.margins.top)),
                ("bottom", typst_num(doc.margins.bottom)),
            ]),
        ),
    ])
}

/// Un nombre en syntaxe Typst, sans expose ni `f32` parasite.
///
/// Typst lit un litteral numerique ; `10` et `10.0` sont equivalents, mais
/// `10.0000001` ferait naitre des marges bizarres. On ecrit donc au plus une
/// decimale, et un entier quand c'en est un.
fn typst_num(valeur: f32) -> String {
    let v = (valeur * 10.0).round() / 10.0;
    if (v - v.round()).abs() < f32::EPSILON {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
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

#[cfg(test)]
mod tests_modeles {
    use super::{Modele, MODELES};

    #[test]
    fn les_codes_de_modele_font_l_aller_retour() {
        assert_eq!(MODELES.len(), 3);
        for m in MODELES {
            assert_eq!(Modele::depuis_code(m.code()), Some(*m));
        }
        // Casse et espaces tolerés : la valeur vient d'une constante saisie
        // a la main.
        assert_eq!(Modele::depuis_code(" EPURE "), Some(Modele::Epure));
        assert_eq!(Modele::depuis_code("standard"), Some(Modele::Standard));
        assert_eq!(Modele::depuis_code("COMPACT"), Some(Modele::Compact));
        // Inconnu : l'appelant retombe sur le standard.
        assert_eq!(Modele::depuis_code("modele_qui_n_existe_pas"), None);
        assert_eq!(Modele::depuis_code(""), None);
    }

    #[test]
    fn les_libelles_sont_distincts() {
        for (i, a) in MODELES.iter().enumerate() {
            for b in &MODELES[i + 1..] {
                assert_ne!(a.label(), b.label(), "libelles identiques : {}", a.code());
            }
        }
    }

    /// Une constante de configuration inconnue ne doit jamais empecher
    /// d'editer un document : on retombe sur le standard.
    #[test]
    fn une_constante_inconnue_retombe_sur_le_standard() {
        assert_eq!(
            Modele::depuis_code_ou_standard("modele_qui_n_existe_pas"),
            Modele::Standard
        );
        assert_eq!(Modele::depuis_code_ou_standard(""), Modele::Standard);
        assert_eq!(Modele::depuis_code_ou_standard("  "), Modele::Standard);
        // Les codes connus restent respectes.
        assert_eq!(Modele::depuis_code_ou_standard("compact"), Modele::Compact);
        assert_eq!(Modele::depuis_code_ou_standard("epure"), Modele::Epure);
    }

    /// Le compact et l'epure retirent les annexes ; le standard les garde.
    #[test]
    fn preparer_retire_les_annexes_sauf_en_standard() {
        for modele in MODELES {
            let mut doc = super::tests_gabarits::document_de_test();
            modele.preparer(&mut doc);
            let annexes_attendues = *modele == Modele::Standard;
            assert_eq!(
                doc.cgv.is_some(),
                annexes_attendues,
                "CGV inattendues pour {}",
                modele.code()
            );
            assert_eq!(
                doc.notes.is_some(),
                annexes_attendues,
                "note libre inattendue pour {}",
                modele.code()
            );
            // Les mentions legales, elles, ne bougent jamais.
            assert!(!doc.mentions.is_empty());
            assert!(!doc.lines.is_empty());
        }
    }

    /// Les gabarits sont bien distincts : le compact ne rejoue pas le standard.
    #[test]
    fn le_compact_a_son_propre_gabarit() {
        assert_ne!(
            Modele::Compact.template(),
            Modele::Standard.template(),
            "le compact doit avoir son propre gabarit"
        );
        assert_eq!(Modele::Epure.template(), Modele::Standard.template());
        assert!(Modele::Compact.template().contains("margin: (x: 8mm"));
    }
}

#[cfg(test)]
mod tests_gabarits {
    use super::{
        build_source, compose_avec_modele, render_avec_modele, DocKind, DocumentView, LineKind,
        LineView, Marges, Modele, PartyView,
    };

    /// Les marges reglees arrivent vraiment dans le gabarit.
    ///
    /// Le defaut qu'on corrige : les marges etaient ecrites en dur dans le
    /// gabarit, et l'ecran de reglage enregistrait des valeurs que rien ne
    /// lisait. Un reglage qui ne change rien est pire que pas de reglage.
    #[test]
    fn les_marges_arrivent_dans_le_gabarit() {
        let mut doc = document_de_test();
        doc.margins = Marges {
            x: 30.0,
            top: 22.0,
            bottom: 40.0,
        };
        let source = build_source(&doc, Modele::Standard);
        assert!(
            source.contains("x: 30"),
            "la marge laterale doit etre dans le dictionnaire"
        );
        assert!(
            source.contains("top: 22"),
            "la marge haute doit etre dans le dictionnaire"
        );
        assert!(
            source.contains("bottom: 40"),
            "la marge basse doit etre dans le dictionnaire"
        );
        // Et le gabarit la consomme, au lieu de la valeur en dur d'avant.
        assert!(
            source.contains("data.margins.x * 1mm"),
            "le gabarit doit lire la marge"
        );
    }

    /// Document de reference pour les tests de gabarit : deux lignes, une
    /// section, un sous-total, une TVA a deux taux, des mentions, des CGV et
    /// une note libre.
    pub(super) fn document_de_test() -> DocumentView {
        DocumentView {
            kind: DocKind::Quote,
            title_override: None,
            reference: "PROV612".into(),
            date: "16/10/2026".into(),
            due_date: Some("16/11/2026".into()),
            valid_until: Some("16/11/2026".into()),
            customer_ref: Some("BC-2026-114".into()),
            service_date: Some("12/10/2026".into()),
            bank: Some("IBAN FR76 1680 7003 4838 4868 9521 643".into()),
            payment_terms: Some("30 jours fin de mois".into()),
            deposit: Some("30 % — 1 526,14 €".into()),
            seller: PartyView {
                legal_line: Some("SAS au capital de 10 000 € — RCS Lyon".into()),
                name: "Pierre Angulaire".into(),
                address: Some("12 rue des Peupliers".into()),
                zip: Some("69450".into()),
                town: Some("Lyon".into()),
                siret: Some("123 456 789 00016".into()),
                tva_intra: Some("FR12345678900".into()),
            },
            customer: PartyView {
                legal_line: None,
                name: "M. et Mme Dupont".into(),
                address: Some("12 rue des Lilas".into()),
                zip: Some("69370".into()),
                town: Some("Saint-Didier-au-Mont-d'Or".into()),
                siret: None,
                tva_intra: None,
            },
            lines: vec![
                LineView {
                    kind: LineKind::Section,
                    label: "Lot 1 — Gros oeuvre".into(),
                    ..LineView::default()
                },
                LineView {
                    kind: LineKind::Normal,
                    label: "Reprise de contour de piscine".into(),
                    description: Some("Y compris évacuation des gravois.".into()),
                    qty: "1".into(),
                    unit: "forfait".into(),
                    unit_price_ht: "4 624,65 €".into(),
                    discount_percent: "0,00 %".into(),
                    vat_rate: "10,00 %".into(),
                    total_ht: "4 624,65 €".into(),
                },
                LineView {
                    kind: LineKind::Subtotal,
                    label: "Sous-total Lot 1".into(),
                    total_ht: "4 624,65 €".into(),
                    ..LineView::default()
                },
            ],
            total_ht: "4 624,65 €".into(),
            total_vat: "462,47 €".into(),
            total_ttc: "5 087,12 €".into(),
            vat_breakdown: vec![
                ("10,00 %".into(), "462,47 €".into()),
                ("20,00 %".into(), "0,00 €".into()),
            ],
            mentions: vec![
                "Devis valable un mois à compter de sa date d'émission.".into(),
                "TVA acquittée sur les encaissements.".into(),
            ],
            cgv: Some(
                "Les marchandises voyagent aux risques et périls du destinataire.\n\
                 Paiement par virement à réception de facture."
                    .into(),
            ),
            notes: Some("Acompte de 30 % à la commande.\nSolde à la réception.".into()),
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
    fn standard_et_compact_produisent_un_pdf_valide() {
        let doc = document_de_test();
        for modele in [Modele::Standard, Modele::Compact] {
            let rendu = render_avec_modele(&doc, modele)
                .unwrap_or_else(|e| panic!("rendu impossible pour {} : {e}", modele.code()));
            assert_valid_pdf(&rendu.bytes);
            assert!(
                rendu.bytes.len() > 2048,
                "PDF trop petit pour {} : {} octets",
                modele.code(),
                rendu.bytes.len()
            );
            assert!(rendu.pages >= 1);
        }
    }

    /// Le compact doit tenir dans au plus autant de pages que le standard.
    #[test]
    fn le_compact_ne_depasse_pas_le_standard_en_pages() {
        let mut doc = document_de_test();
        // Document long : les deux modeles paginent, et l'ecart se mesure.
        let modele_ligne = doc.lines[1].clone();
        for i in 0..45 {
            let mut ligne = modele_ligne.clone();
            ligne.kind = LineKind::Normal;
            ligne.label = format!("Poste {} — reprise de maçonnerie", i + 2);
            doc.lines.push(ligne);
        }

        let standard = render_avec_modele(&doc, Modele::Standard).expect("rendu standard");
        let compact = render_avec_modele(&doc, Modele::Compact).expect("rendu compact");

        assert_valid_pdf(&standard.bytes);
        assert_valid_pdf(&compact.bytes);
        assert!(
            standard.pages >= 2,
            "le document de test doit paginer en standard : {} page(s)",
            standard.pages
        );
        assert!(
            compact.pages <= standard.pages,
            "le compact pagine plus que le standard ({} > {})",
            compact.pages,
            standard.pages
        );
    }

    /// Le compact ne doit pas perdre les mentions legales ni le RIB.
    #[test]
    fn le_compact_conserve_les_mentions_legales() {
        let gabarit = Modele::Compact.template();
        assert_eq!(
            gabarit.matches("data.mentions").count(),
            1,
            "les mentions legales doivent etre imprimees une fois"
        );
        assert_eq!(
            gabarit.matches("data.bank").count(),
            2,
            "le RIB : un test de presence et une impression, rien de plus"
        );
    }

    /// Les deux gabarits sont reellement differents : ce n'est pas un filtre.
    #[test]
    fn les_deux_gabarits_different_par_la_mise_en_page() {
        let doc = document_de_test();
        let standard = compose_avec_modele(&doc, Modele::Standard).expect("source standard");
        let compact = compose_avec_modele(&doc, Modele::Compact).expect("source compact");
        assert_eq!(standard.pages().len(), 1);
        assert_eq!(compact.pages().len(), 1);

        let source_standard = super::build_source(&doc, Modele::Standard);
        let source_compact = super::build_source(&doc, Modele::Compact);
        assert_ne!(source_standard, source_compact);
        // Le meme dictionnaire `data` alimente les deux gabarits.
        let prefixe = "#let data = ";
        let fin_standard = source_standard
            .find('\n')
            .map(|i| &source_standard[prefixe.len()..i])
            .unwrap_or_default();
        let fin_compact = source_compact
            .find('\n')
            .map(|i| &source_compact[prefixe.len()..i])
            .unwrap_or_default();
        assert_eq!(fin_standard, fin_compact);
    }
}
