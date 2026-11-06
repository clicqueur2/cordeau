"""Balaie toutes les routes de l'interface contre la copie de production.

But : trouver les ecrans qui cassent sur des donnees reelles, pas sur des
donnees de test. On lit les routes declarees, on remplace les `:id` par de vrais
identifiants tires de la base, et on note le code de retour de chacune.
"""

import io
import re
import subprocess

R = r"Z:\11_PROJETS_DIVERS\dolibarr-rs"
BASE = "http://127.0.0.1:8090"
COOKIES = r"C:\Users\Maxime\AppData\Local\Temp\opencode\ck_balayage.txt"
MYSQL = r"C:\Users\Maxime\.mariadb\server\bin\mysql.exe"
SORTIE = r"Z:\11_PROJETS_DIVERS\dolibarr-rs\ECRANS-EN-ERREUR.md"


def sql(requete):
    r = subprocess.run(
        [MYSQL, "-h", "127.0.0.1", "-P", "3307", "-u", "root", "-proot",
         "--default-character-set=utf8mb4", "-N", "-B", "-D", "dolibarr_prod",
         "-e", requete],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    v = r.stdout.strip()
    return v.splitlines()[0] if v else "1"


# Un identifiant reel par collection, pour que la route ne teste pas un 404
# volontaire.
IDS = {
    "tiers": sql("SELECT rowid FROM llx_societe WHERE entity=7 ORDER BY rowid DESC LIMIT 1"),
    "produits": sql("SELECT rowid FROM llx_product WHERE entity=7 ORDER BY rowid DESC LIMIT 1"),
    "factures": sql("SELECT rowid FROM llx_facture WHERE entity=7 ORDER BY rowid DESC LIMIT 1"),
    "devis": sql("SELECT rowid FROM llx_propal WHERE entity=7 ORDER BY rowid DESC LIMIT 1"),
    "commandes": sql("SELECT rowid FROM llx_commande WHERE entity=7 ORDER BY rowid DESC LIMIT 1"),
    "chantiers": sql("SELECT rowid FROM llx_projet WHERE entity=7 ORDER BY rowid DESC LIMIT 1"),
    "tickets": sql("SELECT rowid FROM llx_ticket ORDER BY rowid DESC LIMIT 1"),
    "contrats": sql("SELECT rowid FROM llx_contrat ORDER BY rowid DESC LIMIT 1"),
    "banques": sql("SELECT rowid FROM llx_bank_account WHERE entity=7 ORDER BY rowid DESC LIMIT 1"),
    "projets": sql("SELECT rowid FROM llx_projet WHERE entity=7 ORDER BY rowid DESC LIMIT 1"),
    "utilisateurs": sql("SELECT rowid FROM llx_user WHERE entity=7 ORDER BY rowid DESC LIMIT 1"),
    "contacts": sql("SELECT rowid FROM llx_socpeople ORDER BY rowid DESC LIMIT 1"),
}

source = io.open(rf"{R}\crates\web\src\routes.rs", encoding="utf-8").read()
routes = sorted(set(re.findall(r'"(/[a-z0-9\-/:._]*)"', source)))
routes = [r for r in routes if ":id" not in r or True]

# On ne garde que des GET : les POST ont des effets.
avec_id = [r for r in routes if ":id" in r]
sans_id = [r for r in routes if ":id" not in r]

cibles = list(sans_id)
for r in avec_id:
    prefixe = r.strip("/").split("/")[0]
    remplace = r.replace(":id", IDS.get(prefixe, "1"))
    remplace = remplace.replace(":code", "Pays").replace(":cle", "MAIN_INFO_SOCIETE_NOM")
    remplace = remplace.replace(":nom", "llx_societe").replace(":compte", "411000")
    cibles.append(remplace)

resultats = []
for chemin in cibles:
    p = subprocess.run(
        ["curl.exe", "-s", "-o", "NUL", "-w", "%{http_code}", "-b", COOKIES,
         "--max-time", "45", BASE + chemin],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    code = p.stdout.strip()
    resultats.append((chemin, code))

casses = [x for x in resultats if x[1] not in ("200", "303", "302", "405", "400")]
print(f"{len(resultats)} routes essayees, {len(casses)} en defaut")
for c, code in casses:
    print(f"  {code} {c}")

lignes = [
    "# Écrans en erreur — balayage sur la copie de production",
    "",
    f"{len(resultats)} routes GET essayées, **{len(casses)} en défaut**.",
    "",
]
if casses:
    lignes += ["| Code | Route |", "|---|---|"]
    lignes += [f"| {code} | `{c}` |" for c, code in casses]
else:
    lignes.append("Aucune.")
lignes += ["", "## Routes essayées", ""]
lignes += [f"- `{c}` → {code}" for c, code in resultats]
io.open(SORTIE, "w", encoding="utf-8", newline="\n").write("\n".join(lignes) + "\n")
print("rapport : ECRANS-EN-ERREUR.md")
