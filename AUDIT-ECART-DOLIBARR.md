# Audit d'écart : noyau Rust contre Dolibarr 24.0.1

Mesures du **4 novembre 2026**, sur le dépôt et sur les sources officielles. Rien
n'est estimé : les chiffres Rust sortent du dépôt, ceux de Dolibarr de son tag
24.0.1 et de l'API GitHub.

Ce document remplace les chiffres de `COMPARAISON-EXHAUSTIVE.md`, qui datent du
3 novembre, quand le noyau avait 32 790 lignes et 269 tests.

## 1. Le volume, aujourd'hui

| | Dolibarr 24.0.1 | Noyau Rust | Rapport |
|---|---|---|---|
| Fichiers source | 15 640 | **135** `.rs` | 1 / 116 |
| Lignes de code | **1 630 232** de PHP | **49 972** de Rust | **3,1 %** |
| Fichiers PHP dans `htdocs` | 4 469 | — | — |
| Crates | — | **24** | — |
| Écrans (routes HTML) | 789 | **142** | **18 %** |
| Tables lues | 410 | **410** | 100 % |
| Tests | ~4 000 | **468** | 12 % |
| Commits | — | **103** | — |

Trois lignes à lire ensemble : **3 % du code, 18 % des écrans, 100 % du
schéma**. C'est la seule chose qui n'a pas bougé depuis le début, et c'est la
plus difficile : la base est interchangeable dans les deux sens.

## 2. La découverte de cet audit : les modules ajoutent des tables

Jusqu'ici, la règle était « aucune table ajoutée, schéma à l'octet ». Elle est
**à nous**, pas à Dolibarr.

Le module officiel de facturation électronique
(`Dolibarr/dolibarr-community-modules/einvoicing`, version 1.4.0, GPLv3) crée ses
propres tables avec ses scripts `sql/` :

```
llx_einvoicing_lifecycle_msg   historique des statuts reçus
llx_einvoicing_extlinks        lien facture <-> objet chez la plateforme
llx_einvoicing_document        le document déposé, sa règle de traitement
```

Conséquence : **un module Dolibarr n'est pas contraint par le schéma du cœur.**
Notre règle nous a fait ranger le journal de caisse et le scellé hors base
(fichier + `llx_const`). C'était prudent, mais ce n'était pas obligatoire — et
ça nous a coûté la possibilité de faire des jointures SQL sur ces données.

**Décision à prendre** (pas tranchée ici) : autoriser des tables préfixées
`llx_<module>_*` pour les objets qui n'existent pas dans le schéma du cœur, ou
rester hors base. Dolibarr a tranché pour la première option.

## 3. La facturation électronique : nous sommes en avance sur un point, en retard sur trois

Le module officiel lit le code d'exonération dans **la colonne du dictionnaire
TVA à partir de Dolibarr 24** — exactement ce que `db::dict::exoneration_tva`
fait depuis ce matin. Sur ce point, nous sommes alignés, et même en avance sur
le cœur, qui ne sait rien faire de cette colonne.

Ce que le module fait et que nous ne faisons pas :

| Sujet | Le module officiel | Nous |
|---|---|---|
| **14 statuts de cycle de vie** | modélisés, dont **4 obligatoires** : 200 déposée, 210 refusée, 212 encaissée, 213 rejetée | nos statuts sont des chaînes opaques |
| **Catégorie O** (hors champ) | retire les identifiants de TVA (BT-31/48/63), garde le SIREN (BT-32), refuse de mélanger O avec exonéré ou taxé sur une même facture | non traité |
| **Fournisseur d'accès** | abstraction multi-plateforme, OAuth SuperPDP documenté, `ADD-A-PDP-PROVIDER.md` | client XP Z12-013 générique, sans compte |
| **Multisociété** | déplacement d'une facture d'une entité à l'autre | non |

Et une correction de vocabulaire : le module parle de **BT-121** pour le code
d'exonération. La norme met le **code en BT-120** et le **texte en BT-121** ;
`facturx` suit la norme.

Les normes du triptyque, pour mémoire : **XP Z12-012** (modèle sémantique et
statuts), **XP Z12-013** (API entre logiciels et plateformes), **XP Z12-014**
(cas d'usage, annexe A normative pour l'ordre des statuts). Je n'avais cité que
la 013 dans `PLATEFORMES-AGREEES.md`.

## 4. Ce que les utilisateurs installent, et que le cœur ne fait pas

Relevé sur Dolistore et sur les pages officielles :

| Module | Ce qu'il apporte | Pour qui |
|---|---|---|
| **einvoicing** (officiel, gratuit) | la réforme 2026 | tout le monde, en France |
| **BTP / suivi de chantier** | chantiers, équipes, budgets, pointage | artisans et maçons — notre cas |
| **Milestone** | lignes de commentaire sans prix ni TVA dans les devis | devis de bâtiment |
| **Place** | lieux, bâtiments, étages, salles, coordonnées GPS | gestion immobilière |

« Abricot » : **introuvable** dans l'écosystème Dolibarr (recherche web du
4 novembre). Les cadres de développement réels sont **Dolibase** (framework de
modules) et le **Module Builder** (générateur d'écrans). Si tu pensais à l'un
des deux, dis-moi lequel : c'est ce qui conditionne la façon dont un tiers
étendra notre noyau.

Sur le forum, les deux reproches récurrents sont les mêmes depuis des années et
ne sont pas dans le code : **la lenteur sur les grosses bases** et **le manque
d'un vrai point de vente tactile**. Le second est un module (TakePOS), pas du
cœur.

## 5. Ce qui manque, classé par ce que ça coûte à l'usage

| Rang | Manque | Effet aujourd'hui |
|---|---|---|
| 1 | **Statuts de cycle de vie 200/210/212/213** | une facture déposée ne se suit pas ; un rejet ne se distingue pas d'un silence |
| 2 | **Catégorie O et refus de mélange** | une facture hors champ serait refusée par une plateforme |
| 3 | **Actions de masse** | valider 40 factures = 40 clics |
| 4 | **Comptabilité analytique** | pas de marge par chantier |
| 5 | **MRP complet** (besoins nets, ordres proposés) | seule l'explosion BOM existe |
| 6 | **Multidevises sur les documents** | taux non figé sur la pièce |
| 7 | **935 chaînes i18n en dur** | français seulement, et pas traduisible |
| 8 | **Modules PHP tiers** | aucun ne tourne — décision assumée |

Les deux premiers sont des corrections de conformité, pas des conforts : ils
tiennent en une journée et ferment un risque de rejet.

## 6. Ce que l'audit ne dit pas

Un écran présent n'est pas un écran complet. Notre fiche facture sait créer,
valider, encaisser, imprimer et faire une situation ; celle de Dolibarr porte en
plus les actions de masse, les récurrences, les modèles d'e-mail, le journal, les
extrafields et le lettrage. Le décompte des routes mesure une surface, pas une
profondeur.

## 7. Sources

- Dépôt local : mesures du 4 novembre 2026 (`crates/`, `sql-ready/`, `git`).
- `github.com/Dolibarr/dolibarr` tag `24.0.1` — volumes, écrans, API.
- `github.com/Dolibarr/dolibarr-community-modules/tree/main/einvoicing` —
  README, `doc/LIFECYCLE-STATUSES.md`, `doc/COVERAGE-AND-LIMITS.md`,
  `doc/ADD-A-PDP-PROVIDER.md` du 4 novembre 2026.
- `dolibarr.org` — page « Facturation Électronique 2026 » (module 1.0.3 puis
  1.4.0, dépôt communautaire, PDP Libre / EsaLink).
- `impots.gouv.fr/specifications-externes-b2b` — triptyque AFNOR XP Z12-012,
  013, 014.
