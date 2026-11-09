# La roadmap de Dolibarr — ce qu'elle dit, et ce que ça nous impose

> **Source** : `wiki.dolibarr.org/index.php?title=Category:RoadMap` (MediaWiki),
> page lue le **9 novembre 2026**, révision `oldid=65564`, dernière édition
> **4 mars 2026 à 23 h 58**. Licence CC BY-SA.
>
> Rien n'est paraphrasé sans le dire : les règles ci-dessous sont celles de la
> page, les conséquences pour Cordeau sont signalées comme telles.

---

## 1. La cadence de sortie

| Élément | Règle officielle |
|---|---|
| Versions majeures | **2 par an**, cadence fixée depuis 2011 et jamais changée |
| Versions de maintenance | **N**, selon les anomalies signalées et leur gravité |
| Majeure | **janvier** (version A.0.0) |
| Gel + beta | **avril** (branche A+1.0 créée, beta de A+1.0.0) |
| Majeure | **juin** (version A+1.0) |
| Gel + beta | **novembre** (branche A+2.0) |
| Maintenance | à tout moment, sur les branches stables `*.*` |

**Conséquence pour Cordeau** : une base figée sur 24.0.1 le reste tant qu'on ne
suit pas. Mais la prochaine majeure tombe en **janvier 2027** — et notre règle
« schéma du cœur identique à l'octet » devra être revérifiée à ce moment-là, pas
avant. Ce n'est pas une urgence, c'est une échéance.

## 2. Ce que le gel autorise, et ce qu'il interdit

Pendant un gel (beta), la page autorise explicitement : corrections de bugs,
optimisations de performance, traductions, données de référence (`llx_c_*`),
finition de travaux commencés, **changements de thème ou d'apparence**, appels à
`dol_syslog()`, ajout d'une colonne si un bug ou une lenteur l'exige.

Et interdit : changement d'architecture, normalisation de code, ajout ou retrait
de bibliothèque externe, **ajout ou retrait de table, retrait de colonne**, et
toute nouvelle fonctionnalité.

**Conséquence pour Cordeau** : notre `REGLE-SCHEMA.md` (cœur à l'octet, tables de
module sous `llx_<module>_`) est **exactement** dans l'esprit de ces règles. On
peut donc écrire dans une base Dolibarr sans la rendre incompatible — à condition
de ne jamais toucher aux tables du cœur, ce que la règle impose déjà.

## 3. La maintenance : « toutes les versions sont LTS »

La page est nette : les correctifs remontent jusqu'à N-4, parfois N-5, et
n'importe quelle version s'améliore si quelqu'un propose le correctif. Dolibarr
garantit aussi qu'on peut **mettre à jour depuis n'importe quelle version**, même
vingt majeures en arrière, en quelques secondes et sans perte de données — sauf
modifications du cœur ou modules externes incompatibles.

**Conséquence pour Cordeau** : viser 24.0.1 est un choix durable (≈ deux ans de
correctifs), et la promesse de mise à jour de Dolibarr est ce qui rend notre
approche « même base, deux binaires » tenable.

## 4. Ce que la page ne dit pas

La liste des pages de version s'arrête à **« Roadmap and Release 23.0.0 »**,
alors que la base de production tourne en **24.0.1**. La page de la 24 n'est donc
pas dans cette catégorie — je le signale comme une **observation**, pas comme une
conclusion : elle peut exister ailleurs sur le wiki, ou ne pas avoir été créée.

C'est exactement le genre d'écart qu'il faut noter au lieu de le combler par une
supposition.

## 5. Ce que la roadmap ne dit pas, et que le forum dit

La roadmap décrit **comment** Dolibarr sort ses versions. Elle ne dit rien des
douleurs des utilisateurs. C'est l'autre fichier qui s'en charge :
`DOLIBARR-FORUM-PROBLEMES.md` (9 Ko), tiré de l'API Discourse le 2 novembre 2026
— **6 catégories, 7 966 sujets, ~32 000 messages**, titres réels, méthode de
collecte écrite dans le fichier, données brutes dans `Temp/opencode/forum/`
(`categories.json` 88 Ko, `top-all.json` 323 Ko).

Les deux se lisent ensemble : la roadmap donne le **rythme** que Cordeau doit
suivre, le forum donne ce que les gens **attendent** qu'on règle (comptabilité
complète, traductions propres).

---

## Méthode, pour être vérifiable

```bash
# la page de catégorie (celle qui porte les règles)
curl -s "https://wiki.dolibarr.org/index.php?title=Category:RoadMap"

# la page d'aiguillage
curl -s "https://wiki.dolibarr.org/index.php?title=Roadmap"
```

Les deux ont été lues le 9 novembre 2026. La page `Roadmap` redirige vers
`Category:RoadMap` — elle ne porte aucune règle propre.
