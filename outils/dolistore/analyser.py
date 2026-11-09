"""Reclasse le catalogue : le titre d'abord, la description seulement ensuite.

Le premier essai mettait 306 modules dans « facturation » parce que le mot
« order » traine dans presque toutes les descriptions. Un titre dit ce qu'est le
module ; une description dit ce qu'il fait en plus.
"""

import io
import json
import re
from collections import defaultdict

P = r"Z:\11_PROJETS_DIVERS\dolibarr-rs\outils\dolistore\dolistore.json"
S = r"Z:\11_PROJETS_DIVERS\dolibarr-rs\outils\dolistore\DOLISTORE.md"

liste = json.load(io.open(P, encoding="utf-8"))

# Ordre du plus precis au plus general : « caisse » avant « facturation ».
FAMILLES = [
    (
        "Comptabilité / fiscalité",
        ["comptab", "accounting", "fec", "tva", "vat ", "bilan", "lettrage", "fiscal", "immobilisation", "amortissement"],
    ),
    ("Banque, prélèvements, trésorerie", ["banque", "bank", "sepa", "prélèvement", "prelevement", "relevé bancaire", "rapprochement", "trésorerie", "treasury"]),
    ("RH : paie, congés, notes de frais", ["paie", "salaire", "salary", "payroll", "congé", "holiday", "absence", "note de frais", "expense report", "hrm", "recrutement", "recruit", "contrat de travail", "pointage", "badgeuse"]),
    ("Caisse, point de vente", ["caisse", "pos", "point of sale", "ticket de caisse", "restaurant"]),
    ("Projets, chantiers, temps", ["projet", "project", "chantier", "tâche", "task", "timesheet", "planning", "gantt", "intervention", "feuille de temps"]),
    ("Stock, produits, production", ["stock", "produit", "product", "article", "production", "mrp", "bom", "nomenclature", "lot", "batch", "inventaire", "warehouse", "entrepôt", "étiquette"]),
    ("Documents, GED, signature", ["ged", "ecm", "signature", "archiv", "scan", "document", "pdf", "bibliothèque"]),
    ("Portail, e-commerce, extranet", ["portail", "portal", "e-commerce", "ecommerce", "boutique", "shop", "extranet", "espace client", "client area"]),
    ("Transport, livraison, logistique", ["transport", "livraison", "delivery", "expédition", "shipment", "colis", "tracking", "logistique", "tournée"]),
    ("Reporting, tableaux de bord, BI", ["dashboard", "tableau de bord", "report", "rapport", "statistique", "kpi", "analyse"]),
    ("CRM, prospects, marketing", ["crm", "lead", "prospect", "marketing", "emailing", "mailing", "newsletter", "sondage", "survey", "campagne", "pipeline"]),
    ("Immobilier, hôtellerie, métiers", ["immobilier", "hôtel", "hotel", "pms", "location", "rental", "syndic", "copropriété", "santé", "pharmacie", "cabinet", "avocat", "notaire", "formation", "école", "association", "garage", "salon"]),
    ("Qualité, conformité, sécurité", ["qualité", "quality", "iso ", "conformité", "rgpd", "gdpr", "audit", "haccp", "sécurité", "hygiène"]),
    ("IA, automatisation", ["ia", "ai", "intelligence artificielle", "chatgpt", "openai", "automat", "workflow", "no-code", "ocr"]),
    ("Interface, administration, technique", ["thème", "theme", "skin", "menu", "ergonom", "interface", "admin", "utilisateur", "user", "droit", "permission", "sauvegarde", "backup", "migration", "api", "ldap", "oauth", "sso", "smtp", "sms", "mail", "notification", "impression"]),
    ("Facturation, devis, achats", ["factur", "invoice", "devis", "quote", "propal", "avoir", "credit note", "acompte", "deposit", "commande", "order", "achat", "purchase", "fournisseur", "supplier", "remise", "discount", "prix", "price", "tarif"]),
]

COUVERT = {
    "Comptabilité / fiscalité": "oui",
    "Facturation, devis, achats": "oui",
    "Banque, prélèvements, trésorerie": "partiel",
    "CRM, prospects, marketing": "partiel",
    "RH : paie, congés, notes de frais": "partiel",
    "Stock, produits, production": "partiel",
    "Projets, chantiers, temps": "oui",
    "Documents, GED, signature": "partiel",
    "Portail, e-commerce, extranet": "non",
    "Caisse, point de vente": "oui",
    "Transport, livraison, logistique": "partiel",
    "Reporting, tableaux de bord, BI": "oui",
    "Immobilier, hôtellerie, métiers": "non",
    "Qualité, conformité, sécurité": "non",
    "IA, automatisation": "non",
    "Interface, administration, technique": "partiel",
}


def contient(texte, mot):
    """Un mot de trois lettres ou moins se cherche en entier : `pos` ne doit
    pas se trouver dans « position », ni `ai` dans « mail »."""
    if len(mot) <= 4:
        return re.search(rf"\b{re.escape(mot)}\b", texte) is not None
    return mot in texte


def famille(m):
    titre = m["titre"].lower()
    for nom, mots in FAMILLES:
        if any(contient(titre, mot) for mot in mots):
            return nom
    texte = f"{m['titre']} {m['resume']} {m['description']}".lower()
    for nom, mots in FAMILLES:
        if any(contient(texte, mot) for mot in mots):
            return nom
    return "Autre"


par_famille = defaultdict(list)
for m in liste:
    par_famille[famille(m)].append(m)

prix = sorted(m["prix"] for m in liste if m["prix"])

lignes = []
lignes.append("# Dolistore : ce que l'écosystème Dolibarr a que nous n'avons pas\n")
lignes.append(
    "> Relevé le **9 novembre 2026** avec `scrapling`, sur "
    "[dolistore.com](https://www.dolistore.com), catégorie *Modules/Plugins*.\n"
)
lignes.append(
    "**Ce que Dolistore ne publie pas** : ni compteur de téléchargements, ni "
    "classement par popularité. Le seul tri offert est *date / prix / nom* "
    '(`<select id="selectProductSort">` vérifié dans le code de la page). On ne '
    "peut donc pas mesurer « les modules les plus téléchargés » — personne ne "
    "peut, à partir du site. Ce qui suit est le **catalogue entier**, c'est-à-dire "
    "tout ce qui est mesurable.\n"
)
lignes.append("## Le chiffre\n")
lignes.append(f"- **{len(liste)} modules** au catalogue")
if prix:
    lignes.append(f"- prix médian **{prix[len(prix)//2]:.0f} €**, maximum **{prix[-1]:.0f} €**")
lignes.append(f"- **{len(par_famille)} familles** fonctionnelles\n")
lignes.append(
    "> Le classement se fait sur des mots-clés, le titre d'abord, la description "
    "ensuite. Il est **approximatif** : un module peut tomber dans la famille "
    "voisine. Le fichier `dolistore.json` porte le titre et la description de "
    "chacun, donc le classement se refait en une requête si besoin.\n"
)

lignes.append("## Famille par famille\n")
lignes.append("| Famille | Modules | Prix médian | Cordeau |")
lignes.append("|---|---:|---:|---|")
for nom, _ in FAMILLES:
    lots = par_famille.get(nom, [])
    if not lots:
        continue
    p = sorted(m["prix"] for m in lots if m["prix"])
    median = f"{p[len(p)//2]:.0f} €" if p else "—"
    lignes.append(f"| {nom} | {len(lots)} | {median} | **{COUVERT.get(nom, '?')}** |")
autres = par_famille.get("Autre", [])
if autres:
    p = sorted(m["prix"] for m in autres if m["prix"])
    median = f"{p[len(p)//2]:.0f} €" if p else "—"
    lignes.append(f"| Autre | {len(autres)} | {median} | — |")

lignes.append("\n## Ce que nous n'avons pas, famille par famille\n")
lignes.append("Les modules sont cités du moins cher au plus cher : c'est le prix d'entrée du marché.\n")
for nom, _ in FAMILLES:
    lots = par_famille.get(nom, [])
    if not lots or COUVERT.get(nom) == "oui":
        continue
    lignes.append(f"### {nom} — {len(lots)} modules, Cordeau : {COUVERT.get(nom)}\n")
    for m in sorted(lots, key=lambda x: (x["prix"] if x["prix"] is not None else 9e9))[:10]:
        prix_txt = f"{m['prix']:.0f} €" if m["prix"] else "—"
        lignes.append(f"- **{m['titre']}** — {prix_txt} · {m['dolibarr'] or 'version ?'}")
    lignes.append("")

lignes.append("## Nos propres trous, relevés dans le code\n")
lignes.append("| Module | Ce qui manque |")
lignes.append("|---|---|")
for cle, quoi in (
    ("contrat", "contrats : lecture seule, ni création ni validation"),
    ("salaire", "bulletins de paie : lecture seule"),
    ("variante", "déclinaisons produit (taille, couleur) : pas d'écran"),
    ("recrutement", "offres et candidatures : pas d'écran"),
    ("reservation", "réservations : pas d'écran"),
    ("ressource", "ressources et affectations : pas d'écran"),
    ("caisse_journal", "journal de caisse : pas d'écran"),
    ("evenement", "validation d'un événement d'agenda : pas d'écran"),
):
    lignes.append(f"| `{cle}` | {quoi} |")

io.open(S, "w", encoding="utf-8", newline="\n").write("\n".join(lignes) + "\n")
print("\n".join(lignes))
