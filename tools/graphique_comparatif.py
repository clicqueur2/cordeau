"""Genere un graphique comparatif en SVG, sans dependance : GitHub affiche les
SVG directement dans le README, et le fichier reste lisible en texte.

Valeurs : medianes mesurees dans COMPARAISON-MESUREE.md (meme base, meme machine,
modules du coeur seulement).
"""

import io
import os

# (libelle, dolibarr_ms, cordeau_ms)
DONNEES = [
    ("Connexion", 25.0, 0.5),
    ("Liste des tiers (932 fiches)", 45.8, 4.4),
    ("Liste des produits", 32.6, 4.0),
    ("Liste des devis", 47.8, 10.0),
    ("Liste des factures (236)", 27.2, 18.6),
    ("Fiche de facture", 28.3, 17.4),
]

LARGEUR = 980
HAUTEUR = 470
MARGE_G = 250
MARGE_D = 130
MARGE_H = 96
LARGEUR_BARRES = LARGEUR - MARGE_G - MARGE_D
MAX_MS = 50.0
HAUTEUR_LIGNE = 52
EPAISSEUR = 15

GRIS = "#9aa4b2"
BLEU = "#1f6feb"
ENCRE = "#1d1d1f"
MUET = "#6b7280"
TRAIT = "#e5e7eb"


def barre(x, y, valeur, couleur):
    largeur = max(1.5, valeur / MAX_MS * LARGEUR_BARRES)
    return (
        f'<rect x="{x:.1f}" y="{y:.1f}" width="{largeur:.1f}" height="{EPAISSEUR}" '
        f'rx="3" fill="{couleur}"/>'
    )


parties = [
    f'<svg xmlns="http://www.w3.org/2000/svg" width="{LARGEUR}" height="{HAUTEUR}" '
    f'viewBox="0 0 {LARGEUR} {HAUTEUR}" font-family="-apple-system, Segoe UI, Roboto, sans-serif">',
    f'<rect width="{LARGEUR}" height="{HAUTEUR}" fill="#ffffff"/>',
    f'<text x="24" y="34" font-size="17" font-weight="600" fill="{ENCRE}">'
    "Temps de réponse, même base et même machine</text>",
    f'<text x="24" y="56" font-size="12.5" fill="{MUET}">'
    "médiane sur 12 mesures, modules du cœur seulement — Dolibarr 24.0.1 (PHP 8.3, opcache) "
    "contre Cordeau (build release)</text>",
]

# Graduations.
for ms in (0, 10, 20, 30, 40, 50):
    x = MARGE_G + ms / MAX_MS * LARGEUR_BARRES
    parties.append(
        f'<line x1="{x:.1f}" y1="{MARGE_H - 14}" x2="{x:.1f}" y2="{MARGE_H + 6 * HAUTEUR_LIGNE - 8}" '
        f'stroke="{TRAIT}" stroke-width="1"/>'
    )
    parties.append(
        f'<text x="{x:.1f}" y="{MARGE_H + 6 * HAUTEUR_LIGNE + 12}" font-size="11" '
        f'fill="{MUET}" text-anchor="middle">{ms}</text>'
    )
parties.append(
    f'<text x="{MARGE_G + LARGEUR_BARRES / 2:.0f}" y="{HAUTEUR - 16}" font-size="11" '
    f'fill="{MUET}" text-anchor="middle">millisecondes</text>'
)

for i, (libelle, dolibarr, cordeau) in enumerate(DONNEES):
    y = MARGE_H + i * HAUTEUR_LIGNE
    facteur = dolibarr / cordeau
    parties.append(
        f'<text x="{MARGE_G - 18}" y="{y + 6}" font-size="13" fill="{ENCRE}" '
        f'text-anchor="end">{libelle}</text>'
    )
    parties.append(barre(MARGE_G, y - 4, dolibarr, GRIS))
    parties.append(
        f'<text x="{MARGE_G + dolibarr / MAX_MS * LARGEUR_BARRES + 8:.1f}" y="{y + 6}" '
        f'font-size="12" fill="{MUET}">{dolibarr:.1f} ms</text>'
    )
    parties.append(barre(MARGE_G, y + EPAISSEUR, cordeau, BLEU))
    parties.append(
        f'<text x="{MARGE_G + cordeau / MAX_MS * LARGEUR_BARRES + 8:.1f}" y="{y + EPAISSEUR + 10}" '
        f'font-size="12" font-weight="600" fill="{BLEU}">{cordeau:.1f} ms</text>'
    )
    parties.append(
        f'<text x="{LARGEUR - 24}" y="{y + 12}" font-size="13" font-weight="600" '
        f'fill="{ENCRE}" text-anchor="end">×{facteur:.0f}</text>'
        if facteur >= 2
        else f'<text x="{LARGEUR - 24}" y="{y + 12}" font-size="13" fill="{MUET}" '
        f'text-anchor="end">×{facteur:.1f}</text>'
    )

# Legende.
y_legende = MARGE_H + 6 * HAUTEUR_LIGNE + 34
parties.append(f'<rect x="24" y="{y_legende}" width="14" height="10" rx="2" fill="{GRIS}"/>')
parties.append(
    f'<text x="44" y="{y_legende + 9}" font-size="12" fill="{MUET}">Dolibarr 24.0.1 (PHP)</text>'
)
parties.append(f'<rect x="204" y="{y_legende}" width="14" height="10" rx="2" fill="{BLEU}"/>')
parties.append(
    f'<text x="224" y="{y_legende + 9}" font-size="12" fill="{MUET}">Cordeau (Rust)</text>'
)
parties.append(
    f'<text x="{LARGEUR - 24}" y="{y_legende + 9}" font-size="11" fill="{MUET}" '
    f'text-anchor="end">×N = facteur — sur la fiche de facture, l\'écart est de 1,6, pas de 10</text>'
)
parties.append("</svg>")

sortie = r"Z:\11_PROJETS_DIVERS\dolibarr-rs\docs\captures\graphique-temps.svg"
io.open(sortie, "w", encoding="utf-8", newline="\n").write("\n".join(parties))
print(f"SVG ecrit : {sortie} ({os.path.getsize(sortie)} octets)")
