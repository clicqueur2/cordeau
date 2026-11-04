# Extraction : fiche contrat Dolibarr

Première fiche d'un module **absent de notre noyau** (pas de `crates/db/src/contrat.rs`,
pas de table dans notre DDL). Source : le vrai `htdocs/contrat/card.php` de ta
production, Dolibarr 24.0.1.

```
htdocs/contrat/card.php   111 481 o   2 625 lignes   MD5 61A10C75B9F29908F3031546A28C2D8E
```

> Note de méthode, à dire avant les chiffres : le fichier annoncé faisait
> **2 278 lignes**, il en fait **2 625** (LF, pas de CR). Et **la fin est coupée** :
> le fichier s'arrête au milieu d'un bloc `<script>` de fin de page (une dizaine
> de lignes cosmétiques manquent, le `</script>` n'est jamais fermé). Rien de
> fonctionnel ne manque — `llxFooter()` et `$db->close()` sont bien appelés
> (l. 2562-2564) **avant** la coupure, et toutes les actions sont présentes.
> Les deux anomalies (compte et coupure) viennent du téléchargement, pas du code.

## Ce que la fiche contient réellement

| Mesure | Valeur |
|---|---|
| **Actions distinctes** | **45** |
| Champs de formulaire distincts (`GETPOST*`) | **113** |
| Appels de lecture d'entrée | **267** (119 `GETPOST`, 138 `GETPOSTINT`, 9 `GETPOSTISSET`, 1 `GETPOSTISARRAY`) |
| `<input name="…">` distincts | 32 |
| Statuts du contrat | 3 (`0`, `1`, `2`) |
| Statuts de service (ligne) | 3 manipulés ici (`0`, `4`, `5`) |
| Statut de signature | 1 colonne (`signed_status`), 2 valeurs nommées |
| Droits vérifiés dans ce fichier | **11** |
| Fichiers d'actions inclus (actions hors fichier) | 6 |

## Les 45 actions, classées

**Cycle de vie du contrat (19)**
`add`, `create`, `view`, `valid`, `confirm_valid`, `reopen`, `close`,
`confirm_close`, `activate`, `confirm_activate`, `delete`, `confirm_delete`,
`clone`, `confirm_clone`, `sign`, `confirm_sign`, `unsign`, `confirm_unsign`,
`presend`

**Services du contrat — les lignes (13)**
`addline`, `updateline`, `editline`, `deleteline`, `confirm_deleteline`,
`activateline`, `confirm_active`, `unactivateline`, `confirm_closeline`,
`move`, `confirm_move`, `up`, `down`

**Édition de la pièce (7)**
`setref`, `setref_customer`, `setref_supplier`, `setdate_contrat`,
`update_extras`, `classify`, `classin`

**Affichage et filtres (2)**
`showclosedlines`, `hideclosedlines`

**Contacts liés (3)**
`addcontact`, `deletecontact`, `swapstatut`

**Action déclarée mais sans traitement (1)**
`setremise`

> Deux actions sur 45 sont **mortes**, mesuré et non supposé :
> `setremise` est posée en `<input name="action" value="setremise">` (l. 1503)
> et **aucune branche du fichier ne la traite** ; `hideclosedlines` est émise en
> URL (l. 2469) alors que seule `showclosedlines` a un test (l. 2468).
> Le formulaire « remise » du contrat, en brouillon, ne fait donc rien.

> Et 45 est un **plancher** : six `include` de fichiers d'actions du noyau
> ajoutent leurs propres actions sans qu'on les compte ici —
> `actions_setnotes`, `actions_dellink`, `actions_lineupdown` (d'où `up`/`down`),
> `actions_printing`, `actions_builddoc`, `actions_sendmails` (l. 184-188,
> 1072-1083). Une réimplémentation doit les compter aussi.

## Les 11 droits vérifiés dans ce fichier

```
contrat/creer          contrat/lire          contrat/activer
contrat/desactiver     contrat/supprimer
contract/creer                     ← module au SINGULIER (2 occurrences)
produit/ignore_price_min_advance
propale/creer          commande/creer        facture/creer
fournisseur/facture/creer
```

Le module s'appelle `contrat` (5 droits). Les deux actions de **signature**
(`confirm_sign` l. 208, `confirm_unsign` l. 216) testent `hasRight('contract', 'creer')`
— **`contract` au singulier, un module de droits qui n'existe pas**. Un
utilisateur qui n'a que `contrat/creer` se voit donc refuser la signature, sans
message explicite : l'action ne rentre dans aucune branche et la page se
réaffiche. C'est un vrai défaut du fichier, pas une lecture hâtive ; à confirmer
par un test sur ton instance avant d'en faire une règle.

Deux droits calculés, pas lus : `$permissiontodelete` (brouillon **et**
`creer`, **ou** `supprimer`) et `$permissiontodisable` (marqué `TODO use same
than $permissiontoactivate` en commentaire, l. 134).

## Les statuts, et où ils sont écrits

Le contrat porte **trois** statuts. La seule définition présente dans le fichier
est un commentaire (l. 1878) : `status contract 0=draft,1=validated,2=closed`.

| Statut contrat | Sens | Écrit par |
|---|---|---|
| `0` (`STATUS_DRAFT`) | brouillon, `ref` = `PROV…` | `create()` |
| `1` (`STATUS_VALIDATED`) | validé, `ref` définitive | `validate()` (l. 900) |
| `2` | clos | **jamais par ce fichier** |

`card.php` ne pose **jamais** `statut` lui-même. L'action `close` appelle
`closeAll($user)` (l. 932), qui ferme les **services** ; ce que fait `closeAll()`
à l'intérieur de la classe n'est pas visible ici. Conséquence mesurable : les 2
occurrences de `STATUS_CLOSED` dans le fichier sont `ContratLigne::STATUS_CLOSED`
(l. 1800, 2071) — le statut de **ligne** — et `Contrat::STATUS_CLOSED` n'y
apparaît pas une seule fois. Les colonnes `llx_contrat.fin_validite` et
`llx_contrat.date_cloture` existent au schéma mais ne sont **jamais lues ni
écrites par `card.php`** (0 occurrence des deux noms).

Les statuts de ligne réellement manipulés ici sont **trois** : `0`, `4`, `5`
(toutes les comparaisons du fichier portent sur ces valeurs : l. 1918, 1930,
2134, 2278-2279, et les totaux par statut l. 1608-1611).

| Statut ligne | Sens | Écrit par |
|---|---|---|
| `0` | en attente / inactif (`nbofserviceswait`) | `addline` |
| `4` | **actif / en service** (`nbofservicesopened`) | `active_line()`, `activateAll()` |
| `4` **et** `date_fin_validite` passée | expiré (`nbofservicesexpired`) — même statut, un simple drapeau à l'affichage | rien, c'est calculé |
| `5` (`ContratLigne::STATUS_CLOSED`) | clos (`nbofservicesclosed`) | `close_line()`, `closeAll()` |

Il y a enfin un statut **orthogonal**, la signature : `signed_status`, piloté par
`setSignedStatus($user, …, 'CONTRACT_MODIFY')`, avec deux valeurs nommées
utilisées (`STATUS_NO_SIGNATURE`, `STATUS_SIGNED_ALL`) et un bouton masqué
derrière la constante `CONTRACT_SHOW_SIGNATURE_STATUS_WITH_SERVICE_STATUS`.
Il ne change **rien** au statut du contrat ni à la facturation.

## Le cycle de vie : quelle transition fait quoi

| Action | Fonction appelée | Effet mesuré | Droit |
|---|---|---|---|
| `add`/`create` | `$object->create($user)` | contrat `0`, `ref` = `PROV` | `contrat/creer` |
| `confirm_valid` | `validate($user)` | `0 → 1`, `ref` renumérotée, PDF auto-généré (sauf `MAIN_DISABLE_PDF_AUTOUPDATE`) | `contrat/creer` |
| `reopen` | `reopen($user)` | `1 → 0`, on réédite | `contrat/creer` |
| `confirm_activate` | `activateAll($user, d_start, 0, comment, d_end)` | **tous** les services `→ 4` | `contrat/activer` |
| `confirm_active` | `active_line($user, ligne, start, end, comment)` | **un** service `→ 4` | `contrat/activer` |
| `confirm_closeline` | `close_line($user, ligne, date_end, comment)` | **un** service `→ 5`, `date_end_real` plafonnée à maintenant | `contrat/desactiver` |
| `confirm_close` | `closeAll($user)` | **tous** les services `→ 5` | `contrat/desactiver` |
| `confirm_sign` | `setSignedStatus($user, signed_status, 0, 'CONTRACT_MODIFY')` | signature seulement | `contract/creer` (cassé) |
| `confirm_delete` | `delete($user)` | suppression | brouillon+`creer` ou `supprimer` |

Piège de nommage, mesuré : l'activation **du contrat** est `confirm_activate`
(avec un `a` de plus), celle **d'une ligne** est `confirm_active`. Les deux
existent, à une lettre près, et n'ont pas les mêmes champs de formulaire :

- contrat : `d_startmonth/d_startday/d_startyear`, `d_endmonth/…`, `comment` ;
- ligne : `startmonth/startday/startyear`, `endmonth/…`, `comment`, `ligne`.

La même ligne `confirm_closeline` reçoit `endmonth/endday/endyear` + `ligne`.
`comment` est `GETPOST(…,'alpha')` côté activation et `urldecode(GETPOST('comment'))`
côté clôture — asymétrie réelle (l. 200 et 234).

Deux boutons seulement, avec leur condition exacte (l. 2451 et 2458) :
« Activer » exige `nbofservicesclosed > 0 || nbofserviceswait > 0` ;
« Clôturer » exige `nbofservicesclosed < nbofservices`.

## Les services : les colonnes exactes

`card.php` ne nomme jamais les colonnes de la ligne : il écrit une requête de
jointure qui les alias sur les noms de l'objet (l. 1741-1742). C'est le seul
endroit du fichier où le mapping est explicite :

```sql
cd.date_ouverture_prevue as date_start,      cd.date_ouverture  as date_start_real,
cd.date_fin_validite     as date_end,        cd.date_cloture    as date_end_real,
```

Donc, dans `llx_contratdet` (schéma 24.0.1, `sql-ready/1_tables.sql` l. 2106-2154) :

| Usage du code | Colonne réelle | Écrite quand |
|---|---|---|
| `date_start` (début prévu) | **`date_ouverture_prevue`** | `addline` / `updateline` |
| `date_end` (fin prévue) | **`date_fin_validite`** | `addline` / `updateline` |
| `date_start_real` (début réel) | **`date_ouverture`** | activation |
| `date_end_real` (fin réelle) | **`date_cloture`** | clôture |
| statut du service | `statut` | 0 / 4 / 5 |
| commentaire d'activation | `commentaire` | `active_line`, `close_line` |
| qui a ouvert / clos | `fk_user_ouverture`, `fk_user_cloture` | automatique |

Deux points de contrôle sont posés à l'ajout et à la modification de ligne :
`date_start > date_end` est refusé (l. 545 et 775). Il n'y a **aucune autre
règle** : ni durée minimale, ni cohérence avec la date du contrat.

La date de fin prévue est **pré-remplie par le produit** : si le service a un
`fk_product` et que le produit a `duration_value` + `duration_unit`,
l'échéance proposée est `dol_time_plus_duree(time(), duration_value, duration_unit)`
(l. 2212-2213, 2266-2269). C'est le seul mécanisme « durée » du module, et il
passe par la fiche produit, pas par le contrat.

Deux seuils d'alerte viennent de la configuration du module, pas de la base :
`$conf->contrat->services->inactifs->warning_delay` (service prévu commencé et
toujours inactif) et `$conf->contrat->services->expires->warning_delay` (service
actif dont la fin prévue est passée). 10 occurrences de `warning_delay` dans le
fichier.

## Ce qui déclenche une facture de contrat

C'est le point le plus important de la fiche, et la réponse est nette.

**`card.php` ne crée jamais de facture.** Aucune requête, aucune écriture sur
`llx_facture`, `llx_facture_rec` ou `llx_element_element`. Ce que la fiche fait,
c'est **un lien** vers la fiche facture, dans le menu « Créer » (l. 2422-2430) :

```
/compta/facture/card.php?action=create&origin=<element>&originid=<id>&socid=<socid>
```

avec trois conditions cumulatives, toutes mesurées :

1. `$object->status > 0` — un contrat **brouillon ne peut pas être facturé** ;
2. `$soc->client > 0` — le tiers doit être un client ;
3. `$user->hasRight('facture', 'creer')`.

Le même menu propose le devis (`/comm/propal/card.php?…&renewal=true`, l. 2405),
la commande (`/commande/card.php?…`) et la facture fournisseur. Le contrat est
donc une **source de lignes**, pas un moteur de facturation : c'est la fiche
facture qui va chercher les lignes du contrat par `originid`.

**`llx_facture_rec` n'a aucune colonne `fk_contrat`.** Vérifié colonne par
colonne sur le schéma 24.0.1 (l. 3205-3255) : `titre`, `fk_soc`, `frequency`,
`unit_frequency`, `date_when`, `date_last_gen`, `nb_gen_done/max`,
`rule_for_lines_dates`, `auto_validate`… et pas de contrat. Il n'existe donc
**aucun chemin de facturation récurrente d'un contrat** dans cette version :
un contrat ne se facture pas tout seul, quelqu'un clique.

`fk_contrat` existe pourtant, ailleurs (3 occurrences dans le schéma) :
`llx_fichinter.fk_contrat` (l. 3383) et `llx_fichinter_rec.fk_contrat`
(l. 3425). **Un contrat produit des interventions récurrentes, pas des factures
récurrentes.** C'est une distinction de conception, pas un oubli.

Le lien inverse existe : on peut **créer un contrat depuis un modèle de facture
récurrente** (`origin=facturerec` ou `origin=facture_rec`, l. 336-341 et
1208-1212, classe `FactureRec`). Les lignes de type service du modèle sont
recopiées dans le contrat ; les autres sont ignorées, sauf si la constante
`CONTRACT_SUPPORT_PRODUCTS` est active (l. 384).

## Le renouvellement : il n'est pas stocké

Question posée, réponse mesurée, et elle est radicale : **le schéma 24.0.1 ne
contient aucun champ de reconduction ni de préavis.** Recherche sur
`sql-ready/1_tables.sql` (6 515 lignes, toutes les tables du schéma) :
`reconduc`, `preavis`, `tacite` → **0 occurrence**, toutes tables confondues.

Ce qui existe à la place :

| Mécanisme | Où | Ce qu'il fait |
|---|---|---|
| `llx_contrat.fin_validite` | schéma l. 2073 | colonne présente, **jamais touchée** par `card.php` |
| `llx_contrat.date_cloture` | schéma l. 2074 | idem, **jamais touchée** |
| `llx_contratdet.date_fin_validite` | schéma l. 2119 | la vraie échéance, par service |
| `duration_value`/`duration_unit` du **produit** | fiche produit | pré-remplit l'échéance à l'activation |
| Devis de renouvellement | `/comm/propal/card.php?…&renewal=true` | on refait un devis, à la main |

Autrement dit : dans Dolibarr, **renouveler un contrat = refaire un devis**
(marqué `renewal`), pas appliquer une règle de tacite reconduction. Il n'y a ni
préavis à paramétrer, ni date de renouvellement à calculer, ni notification à
déclencher. Le seul garde-fou automatique est l'alerte visuelle sur un service
actif dont la `date_fin_validite` est dépassée, avec un délai réglé dans la
configuration du module.

Pour un maçon qui veut « contrat d'entretien annuel renouvelé tacitement avec
préavis de 2 mois », **rien de tout ça n'existe** : c'est à nous de le porter, et
il faudra une table (ou des extrafields) puisque le schéma Dolibarr n'a pas la
place.

## L'écart avec notre noyau

| | Dolibarr | Noyau Rust |
|---|---|---|
| Actions sur la fiche | **45** | **0** |
| Champs de saisie | 113 | 0 |
| Écritures en base | 1 `SELECT` dans le fichier (les lignes, l. 1739) et **19 méthodes d'écriture** distinctes appelées sur les objets | **1 `SELECT`**, aucune écriture |
| Statuts du contrat | 3 | 3, mais mal nommés (voir ci-dessous) |
| Statuts de service | 0 / 4 / 5 | absents |
| Droits | 11 | 0 |
| Routes | `card.php`, `list.php`, `agenda.php`, `messaging.php` + les documents | 1 (`/contrats`) |

Ce que le noyau possède réellement :

- `crates/db/src/divers.rs` — struct `Contrat` à **6 champs** (`rowid`, `ref_`,
  `tiers`, `date`, `statut`, `total_ttc`) et **une** fonction `contrats()`, un
  `SELECT` en lecture seule, `LIMIT` paramétré (l. 25-100) ;
- `crates/web/src/pages.rs` — page `contrats`, liste de 200 lignes maximum,
  en dur (l. 425-454) ;
- `crates/db/src/recurrence.rs` — le commentaire qui dit juste (l. 27-31) :
  `llx_contrat` n'a aucune colonne de récurrence et « un contrat ne se facture
  pas tout seul : l'utilisateur déclenche ». C'est exactement ce que la fiche
  confirme, et c'est déjà écrit chez nous.

L'écart se chiffre en une ligne : **0 action sur 45, soit 0 %**. Ce n'est pas
une critique, c'est un état des lieux — c'était le seul module dont on ne savait
rien.

Un défaut à corriger chez nous, indépendamment de Dolibarr : la page mappe
`1 → "actif"`, `2 → "clos"`, `_ → "brouillon"` (l. 445). Or en Dolibarr `1` est
**validé**, pas actif : l'activité est portée par les **services** (statut 4),
pas par le contrat. Un contrat validé dont tous les services sont clos
s'afficherait chez nous « actif ». Le libellé est faux même si le code du statut
est bon.

## Ce que cette extraction apprend, au-delà du chiffre

1. **La facturation d'un contrat n'est pas un événement du contrat.** C'est un
   lien sortant vers la fiche facture, gardé par `statut > 0` et par le droit
   `facture/creer`. Si on veut un jour facturer depuis un contrat, il ne faut
   pas chercher une mécanique cachée : il n'y en a pas. Et `llx_facture_rec` ne
   connaît pas les contrats — ne pas chercher à les y brancher, la colonne
   n'existe pas.
2. **Le renouvellement est un devis, pas une règle.** Zéro champ de
   reconduction, zéro préavis dans tout le schéma. `fin_validite` et
   `date_cloture` sont des colonnes mortes sur le contrat. C'est un manque du
   modèle, pas une option à retrouver ailleurs.
3. **Le module est plus petit qu'il n'y paraît, mais les pièges de nommage sont
   réels** : `contract` vs `contrat` (signature cassée), `confirm_activate` vs
   `confirm_active` (contrat vs ligne), `d_startmonth` vs `startmonth`, deux
   actions mortes (`setremise`, `hideclosedlines`), et deux constantes mal
   orthographiées (`COINTRACT_ASK_CONTRACT_REF_FOR_CUSTOMER_AT_CREATION`,
   `…FOR_SUPPLIER…`, l. 1277 et 1282) qui ne s'appliqueront jamais. Une
   réimplémentation qui copie les noms sans les corriger copie les bugs.

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
