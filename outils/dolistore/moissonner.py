"""Le catalogue Dolistore, prix compris cette fois.

Le prix est dans `<span class="price product-price"> 395.00 € <span class="small">`
— un `span` imbrique fait echouer un `.*?` trop gourmand. On lit le premier
nombre, point.
"""

import io
import json
import os
import re
import time

from scrapling import Fetcher

BASE = "https://www.dolistore.com/index.php?cat=68&l=en"
SORTIE = r"Z:\11_PROJETS_DIVERS\dolibarr-rs\outils\dolistore"
os.makedirs(SORTIE, exist_ok=True)


def propre(s):
    return re.sub(r"\s+", " ", re.sub(r"<[^>]+>", " ", s or "")).strip()


def nombre(s):
    m = re.search(r"(\d[\d\s.,]*)", s or "")
    if not m:
        return None
    brut = m.group(1).replace(" ", "").replace(",", ".")
    try:
        return float(brut)
    except ValueError:
        return None


def cartes(html):
    morceaux = html.split('<div class="product-container">')[1:]
    trouves = []
    for bloc in morceaux:
        m = re.search(r"product\.php\?id=(\d+)", bloc)
        if not m:
            continue
        titre = re.search(r'class="product-name truncate2"[^>]*title="([^"]*)"', bloc)
        nom = re.search(r'class="product-name truncate2"[^>]*>([\s\S]*?)</a>', bloc)
        desc = re.search(r'class="product-desc truncate2">([\s\S]*?)</p>', bloc)
        prix = re.search(r'class="price product-price"[^>]*>([\s\S]*?)<span class="small"', bloc)
        version = re.search(r'class="version-label">([\s\S]*?)</span>', bloc)
        brut = propre(prix.group(1)) if prix else ""
        trouves.append(
            {
                "id": int(m.group(1)),
                "url": f"https://www.dolistore.com/product.php?id={m.group(1)}&l=en",
                "titre": propre(nom.group(1)) if nom else (titre.group(1) if titre else ""),
                "resume": propre(titre.group(1)) if titre else "",
                "description": propre(desc.group(1)) if desc else "",
                "prix_texte": brut,
                "prix": nombre(brut),
                "gratuit": bool(brut) and ("free" in brut.lower() or nombre(brut) == 0),
                "dolibarr": propre(version.group(1)) if version else "",
            }
        )
    return trouves


catalogue = {}
page = 1
while page <= 80:
    p = Fetcher.get(f"{BASE}&p={page}", timeout=45)
    h = p.html_content if hasattr(p, "html_content") else str(p)
    lots = cartes(h)
    nouveaux = 0
    for c in lots:
        if c["id"] not in catalogue:
            catalogue[c["id"]] = c
            nouveaux += 1
    if nouveaux == 0:
        break
    page += 1
    time.sleep(0.3)

liste = sorted(catalogue.values(), key=lambda x: x["titre"].lower())
io.open(rf"{SORTIE}\dolistore.json", "w", encoding="utf-8", newline="\n").write(
    json.dumps(liste, ensure_ascii=False, indent=2)
)

avec_prix = [m for m in liste if m["prix"] is not None]
gratuits = [m for m in liste if m["gratuit"]]
print(f"{len(liste)} modules, {len(avec_prix)} avec un prix, {len(gratuits)} gratuits")
if avec_prix:
    prix = sorted(m["prix"] for m in avec_prix)
    print(f"prix median {prix[len(prix)//2]:.0f} €, max {prix[-1]:.0f} €")
