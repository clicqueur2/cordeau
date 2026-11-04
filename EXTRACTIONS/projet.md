# Extraction : fiche chantier (projet) Dolibarr

Cinquième fiche, même méthode que `facture.md`. Source : le vrai
`htdocs/projet/card.php` de ta production, téléchargé par FTP.

```
htdocs/projet/card.php   82 367 octets   1 881 lignes   (LF, 0 CRLF)
```

> Note de méthode : le brief annonçait 1 639 lignes. Le fichier réel en compte
> **1 881** — mesuré au comptage des `\n`, pas à l'estimation. Tous les chiffres
> ci-dessous sortent du même fichier.

## Ce que la fiche contient réellement

| Mesure | Valeur |
|---|---|
| **Actions distinctes** | **21** |
| Champs de formulaire lus (`GETPOST*`) | **62** noms distincts (170 appels), dont 44 fonctionnels |
| Statuts | 3 (`STATUS_DRAFT`, `STATUS_VALIDATED`, `STATUS_CLOSED`) |
| Droits vérifiés | **14** distincts (28 appels `hasRight`) |
| Constantes globales qui pilotent l'affichage | 18 |
| Hooks | 6 |
| Boutons d'action | 19 `dolGetButtonAction` + 5 formulaires de confirmation |

## Les 21 actions, classées

**Cycle de vie (7)** — 5 transitions à double action (un écran de confirmation
puis l'écriture), 1 à action unique :

```
validate  → confirm_validate        close → confirm_close
reopen    → confirm_reopen          delete → confirm_delete
clone     → confirm_clone           confirm_setdraft   (seul, sans écran)
```

**Édition de la pièce (4)**
`add` (création), `update`, `edit` (affichage du formulaire), `update_extras`

**Création (3)**
`create` (affichage seul), `clone`, `confirm_clone`

**Documents (2)**
`builddoc`, `remove_file`

**Affaires / opportunités (2)**
`set_opp_status`, `edit_opp_status`

**Email (1)**
`presend`

> Trois `include` ajoutent leurs propres actions, non comptées ci-dessus :
> `actions_dellink.inc.php`, `actions_printing.inc.php`,
> `actions_sendmails.inc.php`. Le vrai total de la page est donc supérieur à 21.

## Les 62 champs, par famille

| Famille | Nb | Champs |
|---|---|---|
| Identité de la pièce | 11 | `id`, `ref`, `title`, `description`, `status`, `socid`, `fk_project`, `public`, `categories`, `typeofcontact`, `attribute` |
| Dates du projet | 9 | `projectstart` + ses 3 sous-champs, `projectend` + ses 3, `reportdate` |
| Dates d'événement | 14 | `date_start_event` et `date_end_event`, chacune + 6 sous-champs (h/min/s/j/m/a) |
| Affaires | 4 | `opp_status`, `opp_percent`, `opp_amount`, `closeproject` |
| Budget | 1 | `budget_amount` |
| Usages (cases à cocher) | 4 | `usage_opportunity`, `usage_task`, `usage_bill_time`, `usage_organize_event` |
| Événement | 1 | `location` |
| Clonage | 7 | `clone_contacts`, `clone_tasks`, `clone_project_files`, `clone_task_files`, `clone_notes`, `move_date`, `comefromclone` |
| Navigation / technique | 11 | `action`, `cancel`, `confirm`, `backtopage`, `backtopageforcancel`, `mode`, `model`, `modelselected`, `lang_id`, `objcanvas`, `file` |

Détail mesuré : `mode` n'est lu qu'une fois (`$mine`, ligne 81) et **jamais
utilisé** — variable morte. Un seul `$_POST` direct dans tout le fichier
(`unset($_POST["ref"])`, ligne 299).

## Les 14 droits

**Sur le projet (4)** — c'est le cœur :

```
projet/lire            projet/creer          projet/supprimer       projet/all/lire
```

**Sur les autres modules (10)** — uniquement pour peupler le menu « Créer » :

```
propal/creer   commande/creer   facture/creer   supplier_proposal/creer
fournisseur/commande/creer   fournisseur/facture/creer
fichinter/creer   contrat/creer   expensereport/creer   don/creer
```

**Trois contrôles qui ne passent pas par `hasRight`** — et ce sont eux qui
décident vraiment :

```
ligne   107   restrictedArea($user, 'projet', $object->id, 'projet&project')
lignes 1027   $userAccess = $object->restrictedProjectArea($user, 'read')
       1028   $userWrite  = $object->restrictedProjectArea($user, 'write')
       1029   $userDelete = $object->restrictedProjectArea($user, 'delete')
ligne  1420   $object->getProjectsAuthorizedForUser($user, 0, 0)
```

## Les quatre points demandés

### 1. Confidentialité : `public` n'est pas un droit

Ce que le fichier prouve, ligne par ligne :

- `public` est **0 ou 1**, libellés `PrivateProject` / `SharedProject` ; deux
  constantes peuvent supprimer chaque option (`PROJECT_DISABLE_PRIVATE_PROJECT`,
  `PROJECT_DISABLE_PUBLIC_PROJECT`) et, si les deux sont posées, un `input
  hidden` **fige** la valeur (lignes 941-960, 1356-1375).
- La valeur est écrite telle quelle à la création et à la modification
  (`$object->public = GETPOSTINT('public') ? 1 : 0`, lignes 229 et 330).
- **Aucune décision de droit ne lit jamais `public`.** Il est affiché
  (lignes 1509-1517) et rien d'autre. Le filtrage par visibilité est délégué à
  `restrictedProjectArea()`.
- `projet/all/lire` n'apparaît **qu'une fois**, ligne 1419, et il ne débloque
  aucun bouton : il élargit seulement la liste des projets précédent/suivant.
- Le refus est un **bouton grisé** avec l'infobulle `NotOwnerOfProject`
  (7 occurrences : lignes 1700, 1710, 1720, 1730, 1739, 1774, 1783), pas un 403.
- Le seul vrai 403 est le `restrictedArea()` de la ligne 107, en tête de page.
- **Exception mesurée** : supprimer un projet **en brouillon** ne demande que
  `projet/creer`, pas `projet/supprimer` (lignes 1779-1780).
- Génération de document : `projet/lire` **et** `$userAccess > 0` (1840).
  Suppression d'un fichier : `projet/creer` **et** `$userWrite > 0` (1841).
- Doctrine écrite en commentaire, ligne 106 : pour un utilisateur externe,
  « no check is done on company because readability is managed by **public
  status of project and assignment** ». C'est exactement l'articulation
  41 / 141 : le droit 41 ouvre le module, le 141 contourne le filtre ligne à
  ligne, et la règle « privé + affectation » vit dans `project.class.php`, **pas
  dans ce fichier**. Ce fichier ne la démontre pas, il la présuppose.

### 2. Les tâches ne sont pas éditées ici

Aucune action de tâche dans les 21. La fiche ne fait que trois choses avec
elles :

- **compter** pour avertir avant suppression (`getTasksArray()` +
  `ThisWillAlsoRemoveTasks`, lignes 1052-1056) ;
- **cloner** (`clone_tasks` coché par défaut, `clone_task_files` décoché,
  lignes 1068 et 1072) ;
- **décaler** : si la date de début change et que `reportdate` est coché,
  `shiftTaskDate()` décale tout le planning (lignes 401-407). La case n'est
  affichée que si `usage_task` **et** `$object->lines` sont non vides
  (lignes 1315-1322) — ce qui prouve au passage que `$object->lines`, pour un
  projet, **ce sont les tâches** (`getLinesArray`).

L'édition se fait dans `projet/tasks.php` et `projet/task.php`, hors de ce
fichier.

### 3. Lien projet → éléments : une colonne, pas une table pivot

`card.php` ne lit **jamais** `llx_element_element`. Le mécanisme est inverse :
le bouton « Créer » construit **10 URLs** de création, chacune passant
`projectid=<id>` au module concerné (lignes 1750-1761) :

```
10 propal      20 commande    30 facture     40 supplier_proposal
50 fourn commande  60 fourn facture  70 fichinter
80 contrat     90 expensereport   100 don
```

Chaque objet stocke donc le lien **de son côté** (`llx_<objet>.fk_projet`) ;
la fiche projet n'a aucune liste d'éléments liés. Deux conditions par bouton :
le module activé, et le droit `creer` du module. Les boutons client
(10/20/30) ne s'affichent que si le tiers est client ou prospect ; les boutons
fournisseur (40/50/60) que si `is_supplier_only`.

### 4. Contacts : un seul contact écrit, jamais depuis la fiche

- Le **seul** `add_contact` du fichier est à la création : l'utilisateur
  courant devient contact interne du projet avec le rôle choisi dans
  `typeofcontact` (défaut `PROJECTLEADER`, liste
  `liste_type_contact('internal','position',1)`, lignes 255-257 et 963-969).
- Ensuite, plus rien : ni `addcontact`, ni `deletecontact` — contrairement à
  `facture/card.php` qui les porte. L'ajout/retrait des autres contacts et le
  rôle par contact se font dans `projet/contact.php`.
- **Incohérence mesurée** : si le rôle `PROJECTLEADER` n'existe plus,
  `add_contact` renvoie `-3` ; le commentaire dit « don't prevent creation »,
  mais le code fait `$error++` puis `rollback()` (lignes 259-267, 285-301).
  La création est donc bel et bien annulée.

## L'écart avec notre noyau

| | Dolibarr `projet/card.php` | Noyau Rust |
|---|---|---|
| Actions | **21** | **3** (créer, modifier, ajouter une tâche) |
| Champs de saisie | 62 noms | 7 (`title`, `soc`, `debut`, `fin`, `budget`, `description`, `label`) |
| Statuts | 3 | 2 (`0` ouvert, `2` clos) |
| Droits | 14, dont 4 sur projet + 3 contrôles ligne à ligne | 2 au niveau route (41, 141), **0 au niveau ligne** |
| Colonnes de `llx_projet` écrites | 21 (+ catégories et extrafields en tables pivots) | 13 |
| Constantes de réglage | 18 | 0 |
| Hooks | 6 | 0 |

Soit **3 actions sur 21 : la fiche chantier est à 14 %**. Ce n'est pas une
estimation, c'est un rapport de comptage sur le fichier réel.

Le détail des colonnes, parce que c'est là que se joue la compatibilité :

- **écrites par les deux** (9) : `ref`, `fk_soc`, `title`, `description`,
  `datec`, `dateo`, `datee`, `fk_statut`, `budget_amount` ;
- **écrites par Dolibarr, jamais par nous** (12) : `fk_project` (sous-projet),
  `public`, `opp_amount`, `fk_opp_status`, `opp_percent`, `usage_opportunity`,
  `usage_task`, `usage_bill_time`, `usage_organize_event`, `date_start_event`,
  `date_end_event`, `location`.

Deux points de compatibilité à trancher, pas des détails :

1. **`fk_statut`** : nous n'écrivons que `0` ou `2` et notre commentaire dit
   « 0 ouvert ». La fiche Dolibarr distingue trois états dont un brouillon
   (`STATUS_DRAFT`). Les valeurs numériques ne sont pas dans ce fichier (elles
   sont dans `project.class.php`) : si `STATUS_DRAFT = 0`, **tout chantier créé
   par le noyau naît en brouillon côté Dolibarr** — donc invisible pour un
   utilisateur qui n'a pas le droit de lire les brouillons, et non modifiable
   comme un projet validé. À vérifier avant de s'en servir en production.
2. **Aucun `fk_projet` ailleurs chez nous** : `grep fk_projet crates/*/src/*.rs`
   ne renvoie que `db/src/projet.rs`. Autrement dit nos devis, commandes et
   factures **ne sont rattachables à aucun chantier**. Dolibarr le fait par une
   colonne sur dix familles d'objets, pas par une table pivot.

## Les trois découvertes

1. **La confidentialité ne se règle pas dans la fiche.** `public` y est écrit,
   affiché, et jamais consulté pour autoriser quoi que ce soit. Tout passe par
   `restrictedProjectArea()` — trois appels `read`/`write`/`delete` dont le
   retour est comparé à `> 0`. Conséquence pratique pour nous : un droit de
   route (41/141) ne suffira jamais à reproduire le comportement Dolibarr ; il
   faut la règle de ligne, et cette règle est dans un fichier que nous n'avons
   pas encore lu. Le 141 ne débloque aucun bouton : il ne sert qu'à la
   navigation précédent/suivant. C'est un droit « voir tout », pas un droit
   « agir sur tout ».

2. **Le projet Dolibarr n'est pas un hub, c'est une étiquette.** Pas de table
   pivot, pas de liste d'éléments dans la fiche : dix URLs de création qui
   passent `projectid`, et chaque module garde le lien chez lui. C'est plus
   fragile (rien ne garantit l'intégrité) mais ça explique pourquoi un projet
   reste lisible même si un module est désactivé — et ça veut dire que porter
   le module « chantier » sans porter `fk_projet` sur devis/factures ne donne
   qu'un objet décoratif. Notre noyau est exactement dans ce cas aujourd'hui.

3. **Le décalage de planning est une action cachée, et c'est la seule
   automatisation réelle de la fiche.** Modifier la date de début peut décaler
   toutes les tâches (`shiftTaskDate`), via une case `reportdate` qui n'apparaît
   que si le projet a des tâches. C'est ce qu'un maçon attend d'un chantier qui
   glisse — et c'est ce que notre noyau ne fait pas : il change la date du
   projet et laisse les tâches sur leurs dates d'origine.

## La méthode, pour les fiches suivantes

```
1. télécharger le card.php du module          (FTP, lecture seule)
2. extraire : actions, champs, droits, statuts
3. classer les actions par famille
4. compter l'écart avec le noyau
5. décider : porter, ou assumer le manque
```

Une fiche = un fichier `EXTRACTIONS/<module>.md`. Le décompte est vérifiable, la
décision est explicite, et rien n'est estimé.
