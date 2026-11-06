"""Etape 2 : comparer ce que Cordeau affiche a ce que la base contient.

Le balayage HTTP ne voit que ce qui casse. Ici on compare des **chiffres** :
ce que la page annonce, et ce que le SQL dit de la meme base. Un ecart est soit
un bug d'affichage, soit un bug de calcul — les deux se corrigent, mais pas au
meme endroit.

L'entite est lue **dans la page**, jamais codee en dur : comparer l'entite 7 a
une page affichant l'entite 1 donnait quatre faux ecarts.

Usage : python tools/compare-prod.py   (serveur sur 8090, base `dolibarr_prod`)
Sortie : tools/ecarts-prod.md
"""

import io
import re
import subprocess

BASE = "http://127.0.0.1:8090"
COOKIES = r"C:\Users\Maxime\AppData\Local\Temp\opencode\ck_compare.txt"
MYSQL = r"C:\Users\Maxime\.mariadb\server\bin\mysql.exe"
RAPPORT = r"Z:\11_PROJETS_DIVERS\dolibarr-rs\tools\ecarts-prod.md"


def sql(requete):
    r = subprocess.run(
        [MYSQL, "-h", "127.0.0.1", "-P", "3307", "-u", "root", "-proot",
         "--default-character-set=utf8mb4", "-N", "-B", "-D", "dolibarr_prod", "-e", requete],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (r.stdout or r.stderr).strip()


def html(chemin):
    r = subprocess.run(
        ["curl.exe", "-s", "-b", COOKIES, "--max-time", "60", BASE + chemin],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return r.stdout


def texte(chemin):
    h = html(chemin)
    h = re.sub(r"(?is)<style.*?</style>", " ", h)
    h = re.sub(r"(?is)<head.*?</head>", " ", h)
    h = re.sub(r"(?is)<svg.*?</svg>", " ", h)
    h = re.sub(r"<[^>]+>", " ", h)
    return re.sub(r"[ \t\u00a0]+", " ", h)


def entite_courante():
    """L'entite du selecteur, pas une constante."""
    m = re.search(r'<option value="(\d+)" selected>', html("/factures"))
    return int(m.group(1)) if m else 7


ECARTS = []


def comparer(intitule, attendu, affiche):
    ok = attendu == affiche
    ECARTS.append({"ecran": intitule, "sql": attendu, "page": affiche, "ok": ok})
    print(f"  {'OK ' if ok else 'ECART'} {intitule:34} base={attendu} page={affiche}")


def compter(page, motif, entity, table):
    t = texte(page)
    m = re.search(motif, t)
    comparer(page, int(sql(f"SELECT COUNT(*) FROM {table} WHERE entity={entity}") or 0),
             int(m.group(1)) if m else None)


E = entite_courante()
print(f"--- entite {E} ---")
compter("/factures", r"(\d+) facture\(s\)", E, "llx_facture")
compter("/tiers", r"(\d+) fiche\(s\)", E, "llx_societe")
compter("/devis", r"(\d+) devis", E, "llx_propal")

ref = sql(f"SELECT ref FROM llx_facture WHERE entity={E} ORDER BY rowid DESC LIMIT 1")
comparer("facture la plus recente", ref, ref if ref in texte("/factures") else None)

total = sum(1 for e in ECARTS if not e["ok"])
lignes = ["# Écarts entre Cordeau et la base — copie de production", "",
          f"Entité contrôlée : **{E}** · {len(ECARTS)} contrôles, **{total} en écart**.", "",
          "| Contrôle | Base | Page | Verdict |", "|---|---|---|---|"]
lignes += [f"| {e['ecran']} | {e['sql']} | {e['page']} | {'ok' if e['ok'] else '**écart**'} |"
           for e in ECARTS]
io.open(RAPPORT, "w", encoding="utf-8", newline="\n").write("\n".join(lignes) + "\n")
print(f"{total} ecart(s) — tools/ecarts-prod.md")
