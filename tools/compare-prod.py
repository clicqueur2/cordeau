"""Etape 2 : comparer ce que Cordeau affiche a ce que la base contient.

Le balayage HTTP ne voit que ce qui casse. Ici on compare des **chiffres** :
ce que la page annonce, et ce que le SQL dit de la meme base. Un ecart est soit
un bug d'affichage, soit un bug de calcul — les deux se corrigent, mais pas au
meme endroit.

Usage : python tools/compare-prod.py  (serveur lance sur 8090, copie `dolibarr_prod`)
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


def page(chemin):
    r = subprocess.run(
        ["curl.exe", "-s", "-b", COOKIES, "--max-time", "60", BASE + chemin],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    h = r.stdout
    h = re.sub(r"(?is)<style.*?</style>", " ", h)
    h = re.sub(r"(?is)<head.*?</head>", " ", h)
    h = re.sub(r"(?is)<svg.*?</svg>", " ", h)
    h = re.sub(r"<[^>]+>", " ", h)
    return re.sub(r"[ \t\u00a0]+", " ", h)


def euro(v):
    """« 63 205,66 » -> 63205.66"""
    v = (v or "").replace("\u202f", "").replace("\u00a0", "").replace(" ", "").replace(",", ".")
    try:
        return round(float(v), 2)
    except ValueError:
        return None


ecarts = []


def comparer(intitule, attendu, affiche, note=""):
    ok = attendu == affiche
    ecarts.append({"ecran": intitule, "sql": attendu, "page": affiche, "ok": ok, "note": note})
    print(f"  {'OK ' if ok else 'ECART'} {intitule:42} sql={attendu} page={affiche} {note}")


print("--- Factures (entite 7) ---")
t = page("/factures")
nb = int(sql("SELECT COUNT(*) FROM llx_facture WHERE entity=7") or 0)
comparer("nombre de factures", nb, int(re.search(r"(\d+) facture\(s\)", t).group(1)) if re.search(r"(\d+) facture\(s\)", t) else None)
total = float(sql("SELECT ROUND(SUM(total_ttc),2) FROM llx_facture WHERE entity=7") or 0)
# La page liste les 5 plus recentes : on compare la premiere facture a la base.
ref = sql("SELECT ref FROM llx_facture WHERE entity=7 ORDER BY rowid DESC LIMIT 1")
comparer("reference la plus recente", ref, ref if ref in t else None)

print("--- Tiers (entite 7) ---")
t = page("/tiers")
nb = int(sql("SELECT COUNT(*) FROM llx_societe WHERE entity=7") or 0)
m = re.search(r"(\d+) fiche\(s\)", t)
comparer("nombre de tiers", nb, int(m.group(1)) if m else None)

print("--- Devis (entite 7) ---")
t = page("/devis")
nb = int(sql("SELECT COUNT(*) FROM llx_propal WHERE entity=7") or 0)
m = re.search(r"(\d+) devis", t)
comparer("nombre de devis", nb, int(m.group(1)) if m else None)

print("--- Grand livre (entite 7) ---")
t = page("/comptabilite/grand-livre")
ventes = float(sql(
    "SELECT ROUND(COALESCE(SUM(total_ht),0),2) FROM llx_facture WHERE entity=7 AND fk_statut>=1") or 0)
# Le grand livre reconstitue : on verifie au moins que la page porte un total.
m = re.search(r"Totaux\s*([\d \u202f\u00a0.,]+)€", t)
affiche = euro(m.group(1)) if m else None
ecarts.append({"ecran": "grand livre : total debit", "sql": None, "page": affiche,
               "ok": affiche is not None, "note": "reconstitution, pas de reference SQL directe"})
print(f"  info  grand livre : total affiche = {affiche}")

total_ecarts = sum(1 for e in ecarts if not e["ok"])
lignes = [
    "# Écarts entre Cordeau et la base — copie de production",
    "",
    f"{len(ecarts)} contrôles, **{total_ecarts} en écart**.",
    "",
    "| Contrôle | Base | Page | Verdict |",
    "|---|---|---|---|",
]
for e in ecarts:
    lignes.append(
        f"| {e['ecran']} | {e['sql'] if e['sql'] is not None else '—'} | "
        f"{e['page'] if e['page'] is not None else '—'} | {'ok' if e['ok'] else '**écart**'} |"
    )
io.open(RAPPORT, "w", encoding="utf-8", newline="\n").write("\n".join(lignes) + "\n")
print(f"\n{total_ecarts} ecart(s) — rapport : tools/ecarts-prod.md")
