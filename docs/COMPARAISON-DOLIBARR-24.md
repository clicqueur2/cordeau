# Noyau Rust contre Dolibarr 24.0.1 — la comparaison

Mesures du **5 novembre 2026**. Les chiffres du noyau sortent du dépôt, ceux de
Dolibarr de son tag 24.0.1 et de son API de publication. Rien n'est estimé.

Ce document remplace `COMPARAISON-EXHAUSTIVE.md` (3 novembre), dont les chiffres
ont vieilli.

## La réponse courte

**Ce n'est pas un équivalent de Dolibarr : c'est le même socle, avec beaucoup
moins d'écrans.** Même base de données, à l'octet. Même arithmétique, vérifiée au
centime. Mais 155 écrans contre 789, et aucun module tiers.

**Ce qu'on gagne** : un binaire unique, pas de PHP, dix fois moins de mémoire, une
interface deux à trois fois plus rapide, un mode lecture seule, et trois moteurs
que Dolibarr laisse à des modules payants — facturation électronique, comptabilité
analytique, besoins nets.

**Ce qu'on perd** : tout l'écosystème. Dolibarr a vingt ans de modules, trente
langues et une communauté ; ce noyau a quatre semaines, deux langues et personne.

## 1. Le volume, mesuré

| | Dolibarr 24.0.1 | Noyau Rust | Rapport |
|---|---|---|---|
| Fichiers source | 15 640 | **148** `.rs` | 1 / 106 |
| Lignes de code | **1 630 232** de PHP | **54 592** de Rust | **3,3 %** |
| Fichiers dans `htdocs` | 4 469 | — | — |
| Crates (paquets) | — | **24** | — |
| Écrans (routes HTML) | 789 | **155** | **20 %** |
| Tables du schéma | 410 | **410** | 100 % |
| Tests | ~4 000 | **545** | 14 % |
| Langues | 30+ | **2** (fr, en) | 6 % |
| Modules tiers | des centaines | **0** | — |
| Commits | 20 ans | **116** | — |

Trois lignes à lire ensemble : **3 % du code, 20 % des écrans, 100 % du schéma.**
C'est le seul chiffre qui compte — et c'est le plus difficile : la base est
interchangeable dans les deux sens.

## 2. Ce qui est identique, et vérifiable

| Élément | Preuve |
|---|---|
| **Schéma SQL** | 410 tables, empreinte `7293039a…` — `schema verify` répond `compatible` |
| **Constantes `MAIN_MODULE_*`** | mêmes noms, semées en base pour 94 modules |
| **Droits** | mêmes identifiants Dolibarr (11 facture, 21 devis, 81 commande, 95 compta…) |
| **Menus** | lus dans `llx_menu` quand la table est peuplée ; 12 menus de tête |
| **Arithmétique des prix** | identique au centime, `calcul_price_total()` PHP gardé comme oracle de test |
| **Mots de passe** | bcrypt `$2y$` — un compte créé ici fonctionne dans Dolibarr, et l'inverse |
| **Multi-entité** | portée par la session, vérifiée sur les 410 tables |

C'est ce qui autorise la phrase la plus utile de ce document : **on peut ouvrir
la même base avec Dolibarr, ou avec ce noyau, sans migration.**

## 3. Les gains, dans l'ordre où on les sent

**1. Un binaire, pas une pile.** Pas de PHP, pas d'extensions, pas d'OPcache à
purger, pas de `.htaccess` à comprendre. Un exécutable et MariaDB. L'installation
tient en un script et une page.

**2. La mémoire et la latence.** Un seul processus, ~10 Mo, réponses mesurées
entre **2 et 9 ms** sur la base de test — là où Dolibarr démarre un processus PHP
par requête.

**3. Le mode lecture seule.** `DOLIBARR_LECTURE_SEULE=1` refuse toute méthode
autre que `GET`. Dolibarr n'a pas l'équivalent : une fois connecté, on écrit.

**4. Les 410 tables sont lisibles à l'écran.** Un explorateur générique : aucune
donnée de Dolibarr n'est invisible, même quand son écran métier n'est pas écrit.
C'est le filet de sécurité du portage.

**5. Trois moteurs que Dolibarr vend à part.** Facturé-X (XML CII + PDF porteur,
règles EN 16931 vérifiées), comptabilité analytique (marge par chantier),
besoins nets avec ordres proposés. Chez Dolibarr, le premier est un module
communautaire, les deux autres des modules du Dolistore.

**6. La comptabilité tenue.** Écritures VE/AC/BQ/AN persistées, FEC exportable,
clôture d'exercice soldée au compte 12 avec instantané, lettrage FIFO écrit en
base (`lettering_code`). Beaucoup d'installations Dolibarr n'ont jamais ce
dernier point.

**7. L'extension sans recompiler.** Un module = un fichier JSON déposé, ou un
greffon en ABI C. Dolibarr exige du PHP, un descripteur et une activation.

**8. Le code est lisible d'un bloc.** 54 000 lignes contre 1,63 million. Un
développeur peut faire le tour du noyau en une semaine ; personne ne fait le tour
de Dolibarr.

## 4. Les pertes, sans enrobage

**1. Aucun module PHP tiers ne tourne.** C'est une décision, pas un oubli : on
n'exécute pas de PHP. Conséquence directe : tout ce que la communauté a écrit
pendant vingt ans est hors d'atteinte.

**2. Deux langues, pas trente.** 669 clés anglaises écrites à la main. Une
troisième langue demande un dossier de traductions, pas du code — mais elle
demande quelqu'un qui la parle.

**3. La moitié des écrans, et une profondeur inégale.** Les listes ont des filtres
depuis cette semaine ; les fiches n'ont pas encore les modèles d'e-mail, les
actions de masse partout, l'affectation manuelle d'un règlement venu de la banque.

**4. Pas de portail public, pas de constructeur de site, pas de TakePOS tactile,
pas de LDAP/OAuth, pas de notifications.** Des pans entiers, laissés de côté en
connaissance de cause.

**5. Aucune communauté, aucun support.** Un bug ici n'a pas de forum où trouver la
réponse. C'est le coût réel d'un fork, et il ne se mesure pas en lignes de code.

**6. Aucun thème.** Un seul, en CSS écrit à la main.

## 5. Fonction par fonction

| Domaine | Dolibarr 24 | Noyau Rust |
|---|---|---|
| Tiers, contacts | complet | complet |
| Produits, stocks | complet | complet, mouvements écrits |
| Devis, commandes, factures | complet | complet + situations, récurrences, envoi e-mail |
| Avoirs, règlements | complet | complet |
| Comptabilité | modules, écritures, FEC | **écritures, FEC, clôture, analytique, lettrage** |
| Facturation électronique | **module externe** | **cœur : EN 16931, PDF, statuts, catégorie O** |
| MRP | module complet | besoins nets et ordres proposés (pas d'ordres de fabrication) |
| Paie | modules externes | moteur d'application d'un barème — aucun taux fourni |
| Point de vente | TakePOS | caisse + ticket + journal scellé, sans écran tactile |
| Projets, tâches | complet | complet |
| CRM, prospects | complet | partiel |
| Site web, portail, LDAP, OAuth | oui | non |
| Modules tiers | des centaines | **aucun** |
| Langues | 30+ | 2 |
| Mémoire par requête | processus PHP | ~10 Mo, un processus |
| Latence | non mesurée ici | 2 à 9 ms |

## 6. Pour qui, et pourquoi

**Pour un artisan ou une TPE qui facture du chantier** : le noyau fait le travail,
et mieux sur trois points — la facture électronique est native, la marge par
chantier existe, le PDF porte les bonnes mentions. Il manque le confort
(profondeur d'écran) et la communauté.

**Pour une entreprise qui a besoin d'un module du Dolistore** : Dolibarr, sans
discussion. Aucun module tiers ne tournera jamais ici.

**Pour un éditeur** : le noyau est une base saine — schéma intact, arithmétique
exacte, tests qui échouent quand quelque chose casse. C'est un point de départ
pour un produit, pas un produit.

## 7. Ce que la comparaison ne dit pas

Un écran présent n'est pas un écran complet. Le décompte des routes mesure une
surface, pas une profondeur. Une fiche facture Dolibarr porte en plus les actions
de masse, les modèles d'e-mail, le journal, les extrafields et le lettrage ; la
nôtre porte lignes, TVA, PDF, validation, avoir, règlement, GED, historique,
envoi e-mail, récurrence. C'est proche, ce n'est pas égal.

Et la durée : Dolibarr a vingt ans de corrections derrière lui. Ce noyau a quatre
semaines. Aucun test ne remplace vingt ans.

## 8. Sources

- Dépôt local, mesures du 5 novembre 2026 : `crates/`, `langs/`, `git`.
- `github.com/Dolibarr/dolibarr`, tag `24.0.1` : volumes, écrans, API.
- `github.com/Dolibarr/dolibarr-community-modules/tree/main/einvoicing` :
  le module de facturation électronique, version 1.4.0.
- `impots.gouv.fr/specifications-externes-b2b` : normes AFNOR XP Z12-012,
  013 et 014.
- `AUDIT-ECART-DOLIBARR.md` : l'audit détaillé, chantier par chantier.
