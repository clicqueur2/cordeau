# Feuille de route — Cordeau

> Décidée le **6 novembre 2026**. Ce fichier est la référence : il dit dans quel
> ordre les chantiers se font, et pourquoi celui-là avant celui-là.

## Où on en est, mesuré

| | |
|---|---|
| Modules Dolibarr portés (couche base) | **36 / 73** |
| Routes | 239 |
| Fichiers d'écran | 60 |
| Modules `db` | 84 |
| Tests | **888**, 0 échec |
| `main.rs` / `pages.rs` | 242 / 162 lignes (contre 5 196 / 2 921) |

Le corps general tient debout : tiers, produits, devis, commandes, factures,
achats, stock, comptabilite, banque, projets, RH, et une vingtaine de modules
secondaires. Ce qui manque est liste plus bas, avec un ordre.

## Les quatre chantiers, dans l'ordre

### 1. Finir le corps general

**Pourquoi d'abord.** Un preset metier n'est credible que s'il active des
modules qui existent. Livrer un preset « BTP » dont la moitie des cases ne fait
rien serait pire que pas de preset du tout.

Portes le 06/11 :

| Module | Ce qu'il apporte | Ce que la mesure a corrige |
|---|---|---|
| `variants` | declinaisons d'un produit | la table de combinaison PORTE une entity, contrairement aux deux tables de liaison |
| `workstation` | postes de charge | pas de brouillon chez Dolibarr : `STATUS_DISABLED = 0` est l'etat de repos |
| `salaries` | fiches de paie | aucune numerotation : la reference EST le rowid |
| `recruitment` | offres et candidatures | le statut « en cours d'entretien » n'existe pas (commente dans la source) |
| `eventorganization` | inscriptions | la table n'a pas d'entity ; le cloisonnement vient du projet |
| `bookcal` | reservations | `day_of_week` n'existe pas, et les heures sont LOCALES |
| `printing` | journal des impressions | la table n'a pas d'entity : une impression est un fait de l'installation |
| `barcode` | codes-barres | Code 39 et Code 128 passent par un binaire externe chez Dolibarr |

Ne seront **pas** portes : `asterisk`, `dav`, `ftp`, `webservices`, `webhook`,
`zapier`, `paypal`, `stripe`, `ai`, `datapolicy`, `debugbar`, `modulebuilder`,
`langs`, `conf`. Ce sont des connecteurs : sans leur homologue externe, le code
serait mort.

### 2. Presets de base par metier

Une option, a la creation de la base **et** depuis Parametres > Modules, qui
installe un socle coherent :

```
Base nue          le minimum : tiers, produits, facturation
BTP / maconnerie  + chantiers, nomenclatures, marges, situations, retenues
Logistique        + stock, expeditions, receptions, transferts, inventaires
Commerce          + variantes, commandes, caisse, mailing
Bureau d'etudes   + projets, ressources, temps passe, notes de frais
Association       + adherents, dons, partenariats, evenements
```

**Livre** : `db::preset` (6 socles : nue, BTP, logistique, commerce, bureau
d'etudes, association) et l'ecran `/presets`, qui montre pour chaque socle quels
modules sont deja allumes. Trois regles tenues par le code et par les tests :
un preset **active sans jamais eteindre**, n'ecrit **aucune donnee metier**, et
**dit ce qu'il ignore** (un module absent du catalogue ressort au rapport).

Reste a faire : les constantes par metier (conditions de reglement, unites,
mentions de pied de page), et l'application du preset depuis la page
d'installation.

Chaque preset est une **liste de modules actives** et de **constantes posees**,
pas une base differente. Le schema reste identique a l'octet : c'est la
selection qui change, et elle se defait.

Contrainte : un preset ne doit jamais ecrire de donnees metier (pas de tiers,
pas de produit). Une base pre-remplie de faux clients est une base qu'on ne
nettoie jamais.

### 3. Passe d'interface (skill `ui-ux-pro-max`)

**Seulement apres 1 et 2.** Refaire une interface sur un perimetre qui bouge
encore, c'est deux fois le travail.

Objectifs : systeme de jetons (couleurs, espacements, typographie), composants
coherents (tableaux, formulaires, filtres, pastilles), etats vides utiles,
accessibilite (contrastes, navigation clavier, cibles tactiles), mode sombre,
et densite adaptee a un ecran de bureau de comptable.

L'interface actuelle est sobre et lisible, mais elle a ete ecrite ecran par
ecran : les jetons et les composants sont a unifier, pas a inventer.

### 4. Multi-company par defaut

Etat au 06/11 :

| Point | Etat |
|---|---|
| Choix de l'entite | **fait** — la session demarre sur l'entite du compte, et `/entite` permet d'en changer |
| Bandeau | **fait** — le selecteur ne s'affiche que s'il y a plusieurs societes, et porte la mention « listes filtrees » |
| Listes filtrees | **fait** — la lecture par identifiant est cloisonnee comme la liste (teste) |
| Garde-fou | **fait** — facturer un client d'une autre societe est refuse, avec le motif, et rien n'est ecrit (teste) |
| Export par entite | **fait** — les CSV (tiers, produits, ventes, achats, TVA, grand livre) prennent l'entite de la session |
| Sauvegarde par entite | **non, et c'est voulu** |

Sur la sauvegarde : un `mysqldump` est **par nature** global — c'est la base
entiere, pas une societe. Filtrer un dump par entite produirait une sauvegarde
qui ne se restaure pas seule (les cles etrangeres pointeraient vers des lignes
absentes). Qui veut sortir les donnees d'**une** societe prend l'export CSV, qui
existe precisement pour ca. Une sauvegarde incomplete qui a l'air complete est
plus dangereuse qu'une sauvegarde annoncee globale.

## Ce qui ne change jamais

Ces trois regles valent pour tous les chantiers ci-dessus.

1. **Les 410 tables du cceur restent intactes.** Empreinte
   `7293039ac8f4426f3a4a102552ca681228c2e2ffaec7bac10c9e6cc2cae68b2f`. Une
   base qui la respecte s'ouvre dans Dolibarr, et l'inverse aussi. Un module
   peut creer ses tables sous `llx_<module>_` (voir `REGLE-SCHEMA.md`).
2. **Rien n'est declare termine sans que les tests tournent.** `verifier.ps1`
   doit dire « TOUT EST VERT », pas « ca devrait marcher ».
3. **Un manque s'ecrit.** Un module qui ne fait pas une chose le dit dans sa
   doc et sur son ecran. C'est la regle qui a produit les meilleurs choix du
   projet : les receptions ne bougent pas le stock, `marquer_envoye` n'envoie
   rien, `cout_matiere` echoue plutot que de rendre un total partiel.

## Defauts de fond encore ouverts

| Defaut | Etat |
|---|---|
| Acompte non deduit de la facture finale | **corrige** le 06/11 (840 EUR au lieu de 1 200, test a l'appui) |
| `fk_projet` jamais renseigne | **corrige** le 06/11 (propage devis → commande → facture) |
| Marges du gabarit PDF non lues | **corrige** le 06/11 (lues dans `MAIN_PDF_MARGIN_*`) |
| Validation sans PDF, en silence | **corrige** le 06/11 (l'echec se voit et part au journal) |

Aucun defaut de fond ouvert.

## Etat des chantiers

| Chantier | Etat |
|---|---|
| 1. Finir le corps general | **termine le 06/11** — 46 modules, la liste des 8 restants est close |
| 2. Presets de base par metier | **premiere version livree** le 06/11 — restent les constantes par metier |
| 3. Passe d'interface (`ui-ux-pro-max`) | pas commence — le perimetre est desormais stable |
| 4. Multi-company par defaut | **en cours** — cloisonnement prouve, selecteur explicite |

Ce qui a ete porte pour clore le chantier 1 : variantes, postes de charge,
codes-barres, salaires, recrutement, evenements, reservations, impressions.
