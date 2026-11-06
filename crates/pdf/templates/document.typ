//! Gabarit de document commercial : devis, commande, facture, avoir.
//!
//! Ce fichier n'est PAS autonome : `pdf::build_source` injecte juste avant lui
//! le dictionnaire `data` (voir `DocumentView` dans `src/lib.rs`), puis
//! compose l'ensemble en memoire avec Typst.
//!
//! Contraintes de mise en page tenues ici :
//! - A4 portrait, marges 20 mm (haut 18 mm pour degager le pied) ;
//! - police embarquee dans le binaire (DejaVu Sans), donc dans le PDF ;
//! - tableau des lignes repetant son en-tete a chaque page ;
//! - bloc totaux insecable (`breakable: false`) : il n'est jamais coupe ;
//! - pied de page : mentions legales + `Page n / N`.

// --- Palette et aides -------------------------------------------------------

#let ink = rgb("#1f2933")
#let muted = rgb("#5b6672")
#let accent = rgb("#2f5d8a")
#let rule = rgb("#d6dce3")
#let tint = rgb("#eef2f7")

// `true` si la valeur est renseignee (ni `none`, ni chaine vide).
#let has(v) = v != none and v != ""

// --- Metadonnees du PDF -----------------------------------------------------

#set document(
  title: if has(data.reference) { data.title + " " + data.reference } else { data.title },
  author: data.seller.name,
)

// --- Page, texte, pied ------------------------------------------------------

#set page(
  paper: "a4",
  // Marges lues dans les constantes Dolibarr (MAIN_PDF_MARGIN_*). Le defaut du
// gabarit est celui du coeur : 10 mm sur les cotes, 10 en haut, 20 en bas.
margin: (
    x: data.margins.x * 1mm,
    top: data.margins.top * 1mm,
    bottom: data.margins.bottom * 1mm,
  ),
  footer: context [
    #set text(size: 6.5pt, fill: muted)
    #set par(leading: 0.5em)
    #line(length: 100%, stroke: 0.4pt + rule)
    #v(2.5pt)
    #if has(data.bank) [
    #text(weight: "medium")[#data.bank]
    #v(1.5pt)
  ]
  #for m in data.mentions [
      #m \
    ]
    #v(1.5pt)
    #grid(
      columns: (1fr, auto),
      align: (left, right),
      [#data.seller.name],
      [Page #counter(page).display() / #(counter(page).final().first())],
    )
  ],
)

#set text(font: "DejaVu Sans", size: 9pt, lang: "fr", fill: ink)
#set par(leading: 0.6em, spacing: 1.1em)

// --- En-tete : vendeur a gauche, client a droite ----------------------------

#grid(
  columns: (1fr, 74mm),
  column-gutter: 8mm,
  align: (left + top, left + top),
  [
    #text(size: 13pt, weight: "bold", fill: accent)[#data.seller.name]
    #v(3pt)
    #set text(size: 8pt, fill: muted)
    #if has(data.seller.legal_line) [
      #data.seller.legal_line \
    ]
    #for l in data.seller.address_lines [
      #l \
    ]
    #if has(data.seller.siret) [
      SIRET : #data.seller.siret \
    ]
    #if has(data.seller.tva_intra) [
      N° TVA intracommunautaire : #data.seller.tva_intra \
    ]
  ],
  [
    #block(
      width: 100%,
      fill: tint,
      inset: (x: 8pt, y: 7pt),
      radius: 2pt,
    )[
      #set text(size: 7pt, fill: muted, weight: "bold")
      CLIENT
      #v(2.5pt)
      #set text(size: 8.5pt, fill: ink, weight: "regular")
      #text(weight: "bold")[#data.customer.name] \
      #for l in data.customer.address_lines [
        #l \
      ]
      #if has(data.customer.siret) [
        SIRET : #data.customer.siret \
      ]
      #if has(data.customer.tva_intra) [
        N° TVA : #data.customer.tva_intra \
      ]
    ]
  ],
)

// --- Bandeau document : type, reference, date, echeance, validite ----------

#v(6mm)

#block(width: 100%, fill: tint, inset: (x: 9pt, y: 7pt), radius: 2pt)[
  #grid(
    columns: (1fr, auto),
    align: (left, right),
    [
      #text(size: 15pt, weight: "bold", fill: accent)[#data.title]
      #if has(data.reference) [
        #v(1pt)
        #text(size: 8pt, fill: muted)[Réf. :] #text(weight: "medium")[#data.reference]
      ]
    ],
    [
      #{
        let cells = ()
        if has(data.date) {
          cells += ([Date facturation :], data.date)
        }
        if has(data.due_date) {
          cells += ([Date échéance :], data.due_date)
        }
        if has(data.valid_until) {
          cells += ([Valable jusqu'au], data.valid_until)
        }
        // Mentions obligatoires : date de la prestation et bon de commande.
        if has(data.service_date) {
          cells += ([Prestation du], data.service_date)
        }
        if has(data.customer_ref) {
          cells += ([Bon de commande], data.customer_ref)
        }
        // Conditions de reglement : mention obligatoire sur toute facture.
        if has(data.payment_terms) {
          cells += ([Conditions de règlement], data.payment_terms)
        }
        if has(data.deposit) {
          cells += ([Acompte à la commande], data.deposit)
        }
        grid(
          columns: (auto, auto),
          column-gutter: 10pt,
          row-gutter: 2.5pt,
          align: (right, right),
          ..cells,
        )
      }
    ],
  )
]

// --- Tableau des lignes -----------------------------------------------------

// Six cellules pour une ligne : designation, quantite, PU HT, remise, TVA,
// total HT. Les montants restent alignes a droite par le `table` ci-dessous.
// La designation est insecable : une ligne ne peut pas etre coupee par un
// saut de page (sinon sa description se retrouve orpheline en haut de page).
#let line-cells(l) = (
  block(breakable: false)[
    #set par(leading: 0.45em)
    #l.label
    #if has(l.description) [
      #linebreak()
      #text(size: 7.5pt, fill: muted)[#l.description]
    ]
  ],
  l.qty,
  l.unit,
  l.unit_price_ht,
  l.discount_percent,
  l.vat_rate,
  l.total_ht,
)

#v(5mm)

#{
  let cells = ()
  for l in data.lines {
    if l.kind == "section" {
      cells += (table.cell(
        colspan: 7,
        fill: luma(238),
        inset: (x: 4pt, y: 6pt),
        stroke: none,
        text(weight: "bold", size: 9.5pt)[#l.label],
      ),)
    } else if l.kind == "subtotal" {
      cells += (
        table.cell(colspan: 6, align: right, stroke: none,
          inset: (x: 4pt, y: 4pt), text(weight: "bold")[#l.label]),
        table.cell(align: right, stroke: none,
          inset: (x: 4pt, y: 4pt), text(weight: "bold")[#l.total_ht]),
      )
    } else {
      cells += line-cells(l)
    }
  }

  table(
    columns: (1fr, 15mm, 18mm, 23mm, 16mm, 18mm, 25mm),
    align: (left, right, right, right, right, right),
    inset: (x: 4pt, y: 4.5pt),
    stroke: (x, y) => (
      top: if y == 0 { 0.8pt + ink } else { none },
      bottom: if y == 0 { 0.8pt + ink } else { 0.3pt + rule },
    ),
    table.header(
      [Désignation],
      [Qté],
      [Unité],
      [P.U. HT],
      [Rem.],
      [TVA],
      [Total HT],
    ),
    ..cells,
  )
}

// --- Totaux (insecables : jamais coupes par un saut de page) ----------------

#v(4mm)

#grid(
  columns: (1fr, 76mm),
  [],
  [
    #block(width: 100%, breakable: false)[
      #{
        let cells = ()
        cells += ([Total HT], data.total_ht)
        for v in data.vat_breakdown {
          cells += ([TVA #v.rate], v.amount)
        }
        if data.vat_breakdown.len() == 0 {
          cells += ([TVA], data.total_vat)
        }
        grid(
          columns: (1fr, auto),
          column-gutter: 8pt,
          row-gutter: 3.5pt,
          align: (left, right),
          ..cells,
        )
      }
      #v(4pt)
      #block(width: 100%, fill: tint, inset: (x: 7pt, y: 5pt), radius: 2pt)[
        #grid(
          columns: (1fr, auto),
          align: (left, right),
          [*Total TTC*],
          [#text(size: 11.5pt, weight: "bold")[#data.total_ttc]],
        )
      ]
      #v(2pt)
      #text(size: 6.8pt, fill: muted)[Montants exprimés en euros.]
    ]
  ],
)

// --- Notes ------------------------------------------------------------------

#if data.notes_lines.len() > 0 [
  #v(6mm)
  #block(width: 100%)[
    #text(size: 7pt, weight: "bold", fill: muted)[NOTES]
    #v(2pt)
    #set text(size: 8.5pt)
    #for l in data.notes_lines [
      #l \
    ]
  ]
]

// --- Conditions generales de vente ------------------------------------------

#if data.cgv_lines.len() > 0 [
  #v(6mm)
  #block(width: 100%, breakable: true)[
    #text(size: 7pt, weight: "bold", fill: muted)[CONDITIONS GÉNÉRALES DE VENTE]
    #v(2pt)
    #set text(size: 6.8pt, fill: muted)
    #set par(leading: 0.5em)
    #for l in data.cgv_lines [
      #l \
    ]
  ]
]
