# Ce que font Monday, Notion, Linear, Airtable, Stripe — et ce qu'on en prend

> Écrit le 9 novembre 2026. **Méthode** : analyse de la langue visuelle de ces
> outils, croisée avec les palettes documentées dans le jeu de données local
> `ui-ux-pro-max` (domaines `color` et `style`). Ce n'est pas un relevé pixel par
> pixel de leurs sites : je ne prétends pas avoir mesuré leurs CSS. Ce sont les
> principes qu'ils partagent, et ce qu'ils produisent chez l'utilisateur.

---

## 1. Ce qu'ils ont en commun

| Principe | Chez eux | Pourquoi ça marche |
|---|---|---|
| **Un accent vif, un seul** | Monday : orange/rose. Notion : noir + un bleu. Linear : indigo. Airtable : jaune. | L'œil trouve l'action sans lire. Deux accents = aucun accent. |
| **Fond teinté, pas blanc** | `#F5F3FF`, `#F7F8FA` | Le blanc pur fatigue sur un écran dense ; une teinte de 2 % sépare les surfaces sans les border. |
| **Cartes à 12-16 px de rayon, ombre basse** | partout | Le coin arrondi dit « objet », l'ombre dit « posé ». Deux signaux, pas un cadre. |
| **Statuts en pastilles colorées** | Monday surtout | La couleur porte l'information avant le mot : « payée » se voit avant de se lire. |
| **Typographie sans-serif, forte en titres** | Linear, Stripe | Le contraste de graisse porte la hiérarchie ; les filets deviennent inutiles. |
| **Densité moyenne, beaucoup de blanc entre les blocs** | tous | L'air entre les groupes remplace les séparateurs. |
| **États visibles : survol, focus, sélection** | tous | On sait toujours où on est. |

## 2. Ce qu'ils font et qu'on refuse

| Ce qu'ils font | Pourquoi on ne le fait pas |
|---|---|
| Dégradés partout (Monday, Stripe) | Joli sur une page marketing, illisible sur 200 lignes de factures. |
| Illustrations et emojis décoratifs | Notre densité ne le permet pas ; et un emoji change selon la police. |
| Animations à l'entrée, parallaxe | Une page de gestion se consulte cent fois par jour. Le mouvement doit être instantané ou absent. |
| Dépendance à JavaScript pour l'essentiel | Cordeau rend tout côté serveur : c'est ce qui le rend rapide et imprimable. |
| Sidebars à trois niveaux | On a déjà module + entrées ; un troisième niveau se paie en clics. |

## 3. Ce qu'on en a tiré, concrètement

Le style **« Collaboratif »** ajouté le 9 novembre reprend la palette d'un outil
de travail d'équipe (`#6366F1` indigo, fond `#F5F3FF`), le vert de réussite
(`#059669`), les rayons de 12-16 px et les pastilles colorées — sans les dégradés
ni les animations.

| Jeton | Valeur | D'où ça vient |
|---|---|---|
| accent | `#6366F1` | indigo « remote work » documenté |
| fond | `#F5F3FF` | lavande très clair, pas blanc |
| surfaces | `#FFFFFF` | cartes posées sur la lavande |
| succès | `#059669` | vert franc, lisible sur blanc |
| rayons | 10 / 14 px | entre Linear (net) et Monday (rond) |

## 4. Les cinq styles, et à quoi ils servent

| Style | Registre | Pour qui |
|---|---|---|
| **Système** | macOS : gris, bleu Apple, verre dépoli | travail quotidien, sobre |
| **Collaboratif** | indigo, lavande, pastilles vives | travail d'équipe, lecture rapide des statuts |
| **Futuriste** | bleu-nuit, cyan | démonstration, écran partagé |
| **Minuit** | sombre neutre, or sourd | tard le soir, fatigue visuelle |
| **Sable** | clair chaud, terre cuite | impression, présentation client |
| **Classique** | papier gris, acier | qui vient de Dolibarr et ne veut pas être dépaysé |

## 5. La règle qui tient tout

**Un style ne change que des jetons.** Aucune page n'est dupliquée, aucun écran
n'est réécrit. Si un écran écrit une couleur en dur, il échappe au style — c'est
pourquoi les 109 styles en ligne restants sont un vrai chantier et pas une
coquetterie.
