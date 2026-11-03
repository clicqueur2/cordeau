//! Gabarit compact : un document dense, une page si possible.
//!
//! Meme contrat que `document.typ` : `pdf::build_source` injecte juste avant ce
//! fichier le dictionnaire `data` (voir `DocumentView` dans `src/lib.rs`). Ce
//! qui change ici, c'est la mise en page, pas la donnee :
//! - marges 8 mm / 10 mm en haut / 24 mm en bas, corps 8 pt, interligne serre ;
//! - en-tete sur une seule bande : vendeur a gauche, titre a droite, filet ;
//! - fiche client et references sur deux colonnes resserrees, sans bloc gris ;
//! - tableau a six colonnes (l'unite est accolee a la quantite, pas de colonne
//!   dediee) ;
//! - totaux en lignes simples, le TTC en gras, sans cartouche ;
//! - pied de page : RIB, mentions legales, pagination.
//!
//! Les CGV et la note libre ne sont pas prevues au contrat : `Modele::Compact`
//! les retire avant composition. Les blocs restent ecrits ici pour qu'un appel
//! direct au gabarit ne perde jamais silencieusement une donnee.

// --- Palette et aides -------------------------------------------------------

#let ink = rgb("#1f2933")
#let muted = rgb("#5b6672")
#let accent = rgb("#2f5d8a")
#let rule = rgb("#d6dce3")

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
  margin: (x: 8mm, top: 10mm, bottom: 24mm),
  footer: context [
    #set text(size: 5.8pt, fill: muted)
    #set par(leading: 0.4em)
    #line(length: 100%, stroke: 0.4pt + rule)
    #v(2pt)
    #if has(data.bank) [
      Règlement par virement : #data.bank
      #v(1pt)
    ]
    #for m in data.mentions [
      #m \
    ]
    #v(1.5pt)
    #grid(
      columns: (1fr, auto),
      align: (left, right),
      [#data.seller.name],
      [
        #data.title #if has(data.reference) [· #data.reference] · Page #counter(page).display() / #(counter(page).final().first())
      ],
    )
  ],
)

#set text(font: "DejaVu Sans", size: 8pt, lang: "fr", fill: ink)
#set par(leading: 0.35em, spacing: 0.55em)

// --- En-tete : vendeur a gauche, titre a droite, un seul filet --------------

#grid(
  columns: (1fr, auto),
  column-gutter: 6mm,
  align: (left + top, right + top),
  [
    #text(size: 10.5pt, weight: "bold", fill: accent)[#data.seller.name]
    #v(1pt)
    #set text(size: 6.8pt, fill: muted)
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
      N° TVA : #data.seller.tva_intra
    ]
  ],
  [
    #text(size: 13pt, weight: "bold", fill: accent)[#data.title]
    #v(1pt)
    #set text(size: 7pt, fill: muted)
    #if has(data.reference) [
      Réf. : #text(fill: ink, weight: "medium")[#data.reference]
    ]
  ],
)

#v(3mm)
#line(length: 100%, stroke: 0.7pt + ink)
#v(3mm)

// --- Client et references, cote a cote, sans cartouche ----------------------

#grid(
  columns: (1fr, 62mm),
  column-gutter: 6mm,
  align: (left + top, left + top),
  [
    #set text(size: 6.2pt, weight: "bold", fill: muted)
    CLIENT
    #v(1pt)
    #set text(size: 8pt, fill: ink)
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
  ],
  [
    #{
      let cells = ()
      if has(data.date) {
        cells += ([Date], data.date)
      }
      if has(data.due_date) {
        cells += ([Échéance], data.due_date)
      }
      if has(data.valid_until) {
        cells += ([Validité], data.valid_until)
      }
      // Mentions obligatoires : date de la prestation et bon de commande.
      if has(data.service_date) {
        cells += ([Prestation], data.service_date)
      }
      if has(data.customer_ref) {
        cells += ([Bon de commande], data.customer_ref)
      }
      // Conditions de reglement : mention obligatoire sur toute facture.
      if has(data.payment_terms) {
        cells += ([Règlement], data.payment_terms)
      }
      if has(data.deposit) {
        cells += ([Acompte], data.deposit)
      }
      grid(
        columns: (auto, 1fr),
        column-gutter: 6pt,
        row-gutter: 1.5pt,
        align: (left, right),
        ..cells,
      )
    }
  ],
)

// --- Tableau des lignes -----------------------------------------------------

// Six cellules pour une ligne : designation, quantite + unite, PU HT, remise,
// TVA, total HT. La designation est insecable : une ligne ne se coupe pas.
#let line-cells(l) = (
  block(breakable: false)[
    #set par(leading: 0.4em)
    #l.label
    #if has(l.description) [
      #linebreak()
      #text(size: 6.8pt, fill: muted)[#l.description]
    ]
  ],
  [
    #l.qty #if has(l.unit) [#text(size: 6.4pt, fill: muted)[#l.unit]]
  ],
  l.unit_price_ht,
  l.discount_percent,
  l.vat_rate,
  l.total_ht,
)

#v(3mm)

#{
  let cells = ()
  for l in data.lines {
    if l.kind == "section" {
      cells += (table.cell(
        colspan: 6,
        fill: luma(238),
        inset: (x: 3pt, y: 4pt),
        stroke: none,
        text(weight: "bold", size: 8.5pt)[#l.label],
      ),)
    } else if l.kind == "subtotal" {
      cells += (
        table.cell(colspan: 5, align: right, stroke: none,
          inset: (x: 3pt, y: 3pt), text(weight: "bold")[#l.label]),
        table.cell(align: right, stroke: none,
          inset: (x: 3pt, y: 3pt), text(weight: "bold")[#l.total_ht]),
      )
    } else {
      cells += line-cells(l)
    }
  }

  table(
    // Largeurs fixes comptees pour que « 10,00 % » tienne sur une ligne :
    // une colonne TVA trop etroite coupe le pourcentage en deux.
    columns: (1fr, 21mm, 19mm, 14mm, 15mm, 24mm),
    align: (left, right, right, right, right, right),
    inset: (x: 3pt, y: 3pt),
    stroke: (x, y) => (
      top: if y == 0 { 0.7pt + ink } else { none },
      bottom: if y == 0 { 0.7pt + ink } else { 0.25pt + rule },
    ),
    table.header(
      [Désignation],
      [Qté],
      [P.U. HT],
      [Rem.],
      [TVA],
      [Total HT],
    ),
    ..cells,
  )
}

// --- Totaux (insecables) ----------------------------------------------------

#v(3mm)

#grid(
  columns: (1fr, 70mm),
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
        cells += ([*Total TTC*], [#text(size: 10.5pt, weight: "bold")[#data.total_ttc]])
        grid(
          columns: (1fr, auto),
          column-gutter: 6pt,
          row-gutter: 2pt,
          align: (left, right),
          ..cells,
        )
      }
      #v(1.5pt)
      #text(size: 6pt, fill: muted)[Montants exprimés en euros.]
    ]
  ],
)

// --- Notes (vides avec `Modele::Compact`) -----------------------------------

#if data.notes_lines.len() > 0 [
  #v(3mm)
  #block(width: 100%)[
    #set text(size: 6.2pt, weight: "bold", fill: muted)
    NOTES
    #v(1pt)
    #set text(size: 7pt)
    #for l in data.notes_lines [
      #l \
    ]
  ]
]

// --- CGV (vides avec `Modele::Compact`) -------------------------------------

#if data.cgv_lines.len() > 0 [
  #v(3mm)
  #block(width: 100%, breakable: true)[
    #set text(size: 6.2pt, weight: "bold", fill: muted)
    CONDITIONS GÉNÉRALES DE VENTE
    #v(1pt)
    #set text(size: 5.8pt, fill: muted)
    #set par(leading: 0.4em)
    #for l in data.cgv_lines [
      #l \
    ]
  ]
]
