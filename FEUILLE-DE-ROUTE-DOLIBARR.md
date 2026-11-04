# Feuille de route Dolibarr — ce qu'elle change pour nous

Relevé du **4 novembre 2026**. Les dates viennent de l'API GitHub (publications
réelles), les contenus du `ChangeLog` de la branche `develop` (13 908 lignes) et
du wiki officiel. Rien n'est estimé.

## 1. Le calendrier, tel qu'il est tenu

La règle est écrite dans le wiki et n'a pas bougé depuis 2011 : **deux versions
majeures par an**, janvier et juin, avec un gel (début de beta) deux mois avant.

| Version | Publication | Nature |
|---|---|---|
| 22.0.3 | 25/11/2025 | maintenance |
| 22.0.4 | 24/12/2025 | maintenance |
| 23.0.0 | 05/03/2026 | **majeure** — « préparer la certification française, loi de finances, e-invoicing » |
| 23.0.1 → 23.0.3 | 04/04 → 17/05/2026 | maintenance |
| 22.0.5 | 26/05/2026 | maintenance |
| 24.0.0 | 20/08/2026 | **majeure** |
| 24.0.1 | 07/09/2026 | maintenance — **notre base** |
| 24.0.2, 24.0.3 | en cours (`develop`) | maintenance |
| **25.0.0** | **gel novembre 2026, sortie janvier 2027** | majeure — **en beta maintenant** |

Autrement dit : nous avons ancré le noyau sur une version sortie il y a deux
mois, et **la suivante est en train de geler pendant que nous parlons**.

## 2. Ce que chaque version apporte

### 23.0 — 174 nouveautés, focus avoué sur la certification française

Le wiki dit texto : « Code change to prepare french certification Loi finance,
e-invoicing and more. » Traduction en chantiers :

- **Comptabilité** : axe analytique (structure SQL), rapprochement sur compte
  général, TVA par mode de paiement, remises comptabilisées, mode d'export
  ISTEA, vérification de la centralisation.
- **E-invoicing** : colonne `einvoice_vatex` ajoutée au dictionnaire TVA, page
  d'aide pour trouver les modules e-invoice, possibilité d'éditer le code
  d'exonération de TVA.
- **Sécurité** : page de réglage des en-têtes HTTP, événement de sécurité à
  l'installation d'un module.
- **Extrafields** : option « vider à la duplication », lien d'un champ à un
  module.

### 24.0 — 180 nouveautés, et c'est notre base

- **Extrafields : les 79 pages d'administration presque identiques sont
  remplacées par une seule** (`htdocs/admin/extrafields.php`), à liste blanche.
  C'est un aveu : Dolibarr lui-même trouvait ce maquis ingérable.
- **Serveur MCP et assistant IA, expérimentaux** — nous avons déjà le crate
  `mcp`.
- **Comptabilité** : choix entre comptabilité **d'engagement et de trésorerie**,
  **bilan comparatif avec instantané de clôture**, modèles de transactions,
  rapports multiples.
- Transfert de stock stabilisé, numéro EUID, restriction IP par plage CIDR,
  changement de mot de passe forcé.

### 25.0 — la version non stable, et elle est instructive

À ce jour, la section 25.0 du `ChangeLog` ne contient **aucune nouveauté** :
uniquement une liste d'avertissements de rupture. C'est donc une version
d'**hygiène technique**, pas de fonctionnalités.

Ce qu'elle casse :

- hooks du portail web renommés en `webPortalX…` ;
- `BILL_SUPPLIER_CANCELED` → `BILL_SUPPLIER_CANCEL` ;
- `img_object()` n'ajoute plus le préfixe `object_` ;
- suppression de la bibliothèque MathPHP, du gestionnaire de menus
  « smartphone », de la colonne `egroupware_id` ;
- **et le renommage de huit colonnes de date en `date_creation`** :
  `llx_expensereport`, `llx_multicurrency`, `llx_holiday` (`date_create`),
  `llx_element_contact` (`datecreate`), `llx_ecm_directories` et
  `llx_ecm_files` (`date_c`), `llx_mailing` et `llx_mailing_unsubscribe`
  (`date_creat`).

Vérifié dans **notre** base, pas dans la leur :

```
llx_multicurrency            -> date_create
llx_holiday                  -> date_create
llx_ecm_directories          -> date_c
llx_ecm_files                -> date_c
llx_mailing                  -> date_creat
llx_mailing_unsubscribe      -> date_creat
```

## 3. Les trois leçons

**1. Dolibarr consolide, il ne se réécrit pas.** Aucune version à venir ne
menace notre travail : la feuille de route est faite de modules, d'options, de
hooks. Notre pari Rust n'est pas un rattrapage à retardement, c'est une
bifurcation assumée.

**2. Notre ancre « schéma à l'octet » doit être versionnée.** C'était la
promesse centrale : le schéma de Dolibarr 24.0.1, à l'octet. En janvier 2027,
25.0 renomme huit colonnes. La promesse devient donc « le schéma de la version
X », et il faut un script de migration à chaque majeure. C'est le seul point où
leur feuille de route nous atteint.

**3. La certification française n'est pas dans leur cœur.** Deux colonnes de
dictionnaire TVA et un renvoi vers des modules tiers. Nous avons `facturx`,
`einvoicing` et les règles EN 16931 : sur ce terrain, nous sommes **en avance sur
Dolibarr**, pas derrière.

## 4. Ce qu'on peut implémenter, par rapport valeur / effort

| # | Chantier | Pourquoi maintenant | Effort |
|---|---|---|---|
| 1 | **Extrafields** — une page pour les 410 tables | 24.0 vient d'unifier les 79 pages : le design cible est connu, et 30 modules en dépendent. Aucun écran aujourd'hui. | 1 j |
| 2 | **Journal des événements** (`llx_events`) + événements de sécurité | Axe explicite des 23.0 et 24.0. Zéro aujourd'hui. C'est aussi ce qui rend un ERP auditable. | 1/2 j |
| 3 | **Clôture d'exercice + instantané** (bilan comparatif) | 24.0 en fait une fonction ; c'est mon point bloqué depuis le début (compte 12 ou 120/129). | 1 j |
| 4 | **`einvoice_vatex` → Factur-X** (BT-120/121, motif d'exonération) | C'est exactement ce que Dolibarr 23.0 a ajouté ; la colonne est **déjà dans notre base**. Branchement court. | 2 h |
| 5 | **Comptabilité analytique** (axe) | Pilier de 23.0 et 24.0. Les tables sont dans le schéma. | 1 j |
| 6 | **Actions de masse** (valider 40 factures) | Présent sur chaque liste Dolibarr, absent chez nous. | 1/2 j |
| 7 | **Multidevises sur les documents** (taux figé sur la pièce) | Tables `llx_multicurrency` là, taux non porté sur la pièce. | 1/2 j |
| 8 | **MRP complet** (besoins nets, délais, ordres proposés) | Seule l'explosion BOM est faite. | 2 j |
| 9 | **i18n** — extraire les 935 chaînes | 15 617 clés `fr_FR` chargées et inutilisées. | 1 j |
| 10 | **Transfert de stock**, portail web | 24.0 les a stabilisés. | 1-2 j |

Les quatre premiers tiennent en trois jours et referment les trous les plus
visibles.

## 5. Ce qu'on ne fera pas

Modules PHP tiers (aucun PHP exécuté, c'est une décision), constructeur de site,
TakePOS tactile complet, LDAP/OAuth, portails publics, module builder. Ce sont
des pans entiers de Dolibarr qu'on laisse de côté en connaissance de cause, et
`COMPARAISON-EXHAUSTIVE.md` le dit déjà : c'est 98 % du code.

## 6. La décision à prendre maintenant

**Rester sur 24.0.1** (stable, sept patches de recul) et écrire la migration
quand 25.0 sortira, en janvier 2027. Ne pas suivre `develop` : c'est une branche
de rupture, on n'ancre pas un noyau sur un schéma mouvant.

En attendant, la veille se fait en une commande : comparer le `ChangeLog` de
`develop` à notre empreinte de schéma. Le jour où la liste des ruptures bouge,
on le sait.

## 7. Sources

- `api.github.com/repos/Dolibarr/dolibarr/releases` — dates de publication
- `raw.githubusercontent.com/Dolibarr/dolibarr/develop/ChangeLog` — 13 908
  lignes, sections 22.0 → 25.0
- `wiki.dolibarr.org` — page RoadMap (politique de version, gel, LTS de fait)
- `wiki.dolibarr.org/index.php/Roadmap_and_Release_23.0.0` — focus et liste
- `sql-ready/1_tables.sql` — vérification des colonnes dans notre base
