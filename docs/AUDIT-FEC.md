# Audit du FEC produit par le noyau

Contrôle mené selon la grille **Axe 1 — examen du FEC (art. L. 47 A-I du LPF)**
du skill `controleur-fiscal`, sur la base de développement (10 écritures,
exercice 2026).

> Ce n'est pas un contrôle fiscal : c'est la grille d'un vérificateur appliquée à
> ce que **notre** logiciel produit. Le but est de trouver nos défauts avant
> qu'un inspecteur ne les trouve.

## Les huit contrôles obligatoires

| # | Contrôle | Résultat |
|---|---|---|
| 1 | 18 colonnes, séparateur admis | **conforme** — en-tête à 18 colonnes, tabulation |
| 2 | **Équilibre global** : total débits = total crédits | **conforme** — 9 419,20 = 9 419,20 |
| 3 | **Équilibre par écriture** (chaque `EcritureNum` balancé) | **NON CONFORME AVANT CORRECTION** |
| 4 | Numérotation séquentielle sans trou | conforme après correction |
| 5 | Dates dans la période de l'exercice | conforme — du 10/09/2026 au 02/11/2026 |
| 6 | Absence de montants négatifs | conforme |
| 7 | `PieceRef` renseignée | conforme — référence de la pièce reprise |
| 8 | Cohérence `CompteNum` / racines du PCG | **partiel** — voir chef n° 2 |

## Chef de redressement n° 1 — FEC désagrégé

**Impôt concerné** : IS et TVA (comptabilité non probante)
**Base légale** : art. L. 47 A-I du LPF ; art. L. 192 du LPF
**Nature** : chaque ligne portait son propre `EcritureNum`

### Fait constaté

Le fichier produit numérotait **ligne à ligne** : dans la base, les 10 lignes
forment des écritures de 3 lignes, mais l'export leur donnait 10 numéros
distincts. Lu par un vérificateur, cela donne **10 écritures d'une seule ligne,
toutes déséquilibrées**.

Mesure avant correction :

```
ref (identifiant d'écriture en base) : NULL sur les 10 lignes
contrôle « équilibre par écriture »   : impossible à satisfaire
total global débit / crédit           : 9 419,20 / 9 419,20   (équilibré)
```

### Fondement juridique

Un FEC dont les écritures ne s'équilibrent pas est réputé **non probant** :
la charge de la preuve bascule du côté de l'entreprise (art. L. 192 LPF). Le
vérificateur peut alors reconstituer le résultat et écarter la comptabilité.

### Niveau de risque

🔴 **Élevé** — c'est le contrôle que tout logiciel de comptabilité doit passer.

### Recommandation — appliquée

`db::fec::export` regroupe désormais les lignes par
`(journal, référence de pièce, date)` : **toutes les lignes d'une écriture
portent le même `EcritureNum`**. La numérotation reste séquentielle par journal,
sans trou.

Reste à faire : que **tous les écrivains** renseignent la colonne `ref`
(aujourd'hui `NULL` sur les 10 lignes, elles viennent de `fec::generer` et non
de `persist_ecritures`). L'export s'en passe, mais la base perd l'identité de
l'écriture.

## Chef de redressement n° 2 — plan comptable incomplet

**Impôt concerné** : IS
**Base légale** : art. 410-1 du PCG (plan de comptes) ; art. L. 123-12 C. com.
**Nature** : nous produisons six comptes, pas un plan comptable

### Fait constaté

Notre noyau n'utilise que `411000`, `401000`, `601000`, `706000`, `445660`,
`445710` (et les comptes de banque). Un contrôleur demandera la **balance
générale** et constatera que la plupart des racines du PCG sont absentes :
classe 2 (immobilisations), classe 3 (stocks), classe 4 hors clients et
fournisseurs (TVA à décaisser, personnel), classe 5 hors banque (caisse).

### Niveau de risque

🟡 **Moyen** — ce n'est pas faux, c'est **incomplet** : les opérations que nous
ne comptabilisons pas (paie, immobilisations, stocks) n'existent simplement pas
dans notre grand livre.

### Recommandation

Ne pas prétendre à une comptabilité générale. Le noyau produit un **journal des
ventes et des achats** ; la comptabilité générale (immobilisations, paie,
stocks, TVA à décaisser) reste dans le logiciel comptable. C'est ce que
`COMPARAISON-EXHAUSTIVE.md` dit déjà, et le document de conformité doit le
répéter.

## Chef de redressement n° 3 — aucun lettrage

**Base légale** : art. 410-1 PCG ; doctrine sur la justification des comptes
**Nature** : les comptes 411 et 401 ne sont jamais rapprochés des règlements

### Fait constaté

`EcritureLet` et `DateLet` sont vides sur toutes nos lignes. Un règlement
n'éteint pas la créance dans le grand livre : il faudrait une écriture de
lettrage, ou au minimum que le règlement soit comptabilisé au 512 avec
contrepartie 411.

### Niveau de risque

🟡 **Moyen** — le solde du 411 devient **inexpliquable** : un vérificateur ne
peut pas dire quelle facture reste due.

### Recommandation

Le jour où l'on veut « une comptabilité », il faut trois choses de plus :
lettrage automatique (la brique `comptabilite::Lettrage` existe déjà, elle
n'est pas branchée), comptabilisation des règlements, et à-nouveaux en début
d'exercice.

## Synthèse

| # | Nature | Impôt | Risque | État |
|---|---|---|---|---|
| 1 | FEC désagrégé (un numéro par ligne) | IS/TVA | 🔴 | **corrigé** |
| 2 | Plan comptable incomplet | IS | 🟡 | assumé, à documenter |
| 3 | Aucun lettrage, pas d'à-nouveaux | IS | 🟡 | chantier |

### Opinion

> La comptabilité produite par le noyau est **régulière dans sa forme** depuis
> la correction du numéro d'écriture : le FEC s'équilibre globalement et par
> écriture, les dates sont dans l'exercice, les références de pièce sont
> renseignées.
>
> Elle n'est **pas une comptabilité générale** : six comptes, aucun lettrage,
> aucun à-nouveau, pas de paie, pas d'immobilisation, pas de stock. Un vérificateur
> la lirait comme un **journal de ventes et d'achats**, pas comme une
> comptabilité d'entreprise.

C'est l'écart que le document de conformité doit énoncer tel quel, sans le
maquiller en « comptabilité tenue ».
