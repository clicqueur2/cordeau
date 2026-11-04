# Extraction : fiche produit Dolibarr

Deuxième fiche extraite **une par une**. Source : le vrai
`htdocs/product/card.php` de ta production, téléchargé par FTP.

```
htdocs/product/card.php   146 702 octets   3 360 lignes
```

> Note de méthode : le brief annonçait 2 964 lignes ; le fichier sur disque en
> fait **3 360** (comptage des `LF`, aucun `CRLF`, terminé par un saut de ligne).
> C'est la mesure qui fait foi ici — le fichier a peut-être été retéléchargé
> depuis. Le chemin, lui, est bien `htdocs/product/card.php` (pas de
> `htdocs/produit/`).

## Ce que la fiche contient réellement

| Mesure | Valeur |
|---|---|
| **Actions distinctes dans le fichier** | **18** |
| Actions déléguées à des `include` | 4 (`builddoc`, impression ×2, envoi de mail) |
| Champs de formulaire lus | **77** distincts, sur **356** appels (`352` `GETPOST*` + `4` `GETPOSTFLOAT`) |
| — dont `GETPOSTISSET` / `GETPOSTINT` | 73 / 63 |
| — dont noms **calculés** (non littéraux) | 7 (dont `price_<n>` et `multiprices_base_type_<n>`) |
| Types de produit | 2 (`TYPE_PRODUCT` = 0, `TYPE_SERVICE` = 1) |
| Droits distincts vérifiés | **13**, sur 19 appels `hasRight()` |
| Indicateurs globaux lus (`getDolGlobal*`) | 50 distincts, sur 149 appels |
| Modules testés (`isModEnabled`) | 13 |
| Hooks exécutés | 10 |
| Transactions (`$db->begin` / `commit`) | 2 / 2 |
| Occurrences de devise (`multicurrency`, `currency`) | **0** |

## Les 18 actions, classées

Le fichier ne compare `$action` qu'à 18 valeurs. Dix écrivent en base, huit ne
font qu'ouvrir un formulaire.

**Écritures (10)**

| Action | Effet mesuré |
|---|---|
| `add` | `Product::create()` + `setCategories()` |
| `update` | `Product::update()` + `setCategories()` |
| `setfk_product_type` | `setValueFrom('fk_product_type')` |
| `setfk_barcode_type` | `setValueFrom('fk_barcode_type')` |
| `setbarcode` | `check_barcode()` puis `setValueFrom('barcode')` |
| `update_extras` | `updateExtraField()` |
| `confirm_delete` | `Product::delete()` |
| `confirm_clone` | `create()` puis `clone_associations` / `cloneCategories` / `clone_price` / BOM |
| `confirm_merge` | `update()` + `replaceProduct()` sur **14** classes + `delete()` de l'origine |
| `addin` | `addline()` sur une propale, une commande **ou** une facture |

Les 3 `setValueFrom` sont les seuls accès directs à une colonne hors
`create`/`update` : `fk_product_type`, `fk_barcode_type`, `barcode`. Aucun ne
touche un prix.

**Modes d'affichage (8)** — n'écrivent rien :
`create`, `edit`, `view` (défaut, ligne 119), `delete`, `clone`, `merge`,
`editbarcode`, `editbarcodetype`.

**Ce qui n'est pas compté** : `builddoc`, l'impression et l'envoi de mail
viennent de 4 `include` (`actions_builddoc`, `actions_printing` ×2,
`actions_sendmails`). Leurs actions existent mais ne sont pas dans ce fichier —
je ne les invente pas.

## Les 77 champs, par famille

| Famille | Nb | Contenu |
|---|---|---|
| Identité | **37** | `ref`, `label`, `desc`, `note_private`, `url`, `customcode`, `country_id`, `state_id`, `type`, `fk_product_type`, `finished`, `categories`, `statut`, `statut_buy`, `barcode`, `fk_barcode_type`, `barcode_type_code`, `modelselected`, `canvas`, dimensions (7 champs + 7 unités), `net_measure(_units)`, `duration_value`, `duration_unit`, `mandatoryperiod`, `fk_default_bom`, `fk_default_workstation` |
| Stock et lots | **9** | `fk_default_warehouse`, `seuil_stock_alerte`, `desiredstock`, `stockable_product`, `status_batch`, `batch_mask`, `sell_or_eat_by_mandatory`, `lifetime`, `qc_frequency` |
| Technique (routage) | **8** | `action`, `cancel`, `confirm`, `backtopage`, `backtopageforcancel`, `id`, `socid`, `attribute` |
| Copie | **7** | `clone_ref`, `clone_content`, `clone_categories`, `clone_composition`, `clone_prices`, `clone_defbom`, `clone_otherboms` |
| Comptabilité | **6** | `accountancy_code_sell` / `_intra` / `_export`, `accountancy_code_buy` / `_intra` / `_export` |
| Documents liés | **5** | `propalid`, `commandeid`, `factureid`, `qty`, `remise_percent` |
| **Prix** | **4** | `price`, `price_min`, `price_base_type`, `tva_tx` |
| Fusion | **1** | `product_origin` |

Hors décompte, parce que construits dynamiquement : `price_2` … `price_N` et
`multiprices_base_type_2` … `_N` (boucle ligne 720, bornée par
`PRODUIT_MULTIPRICES_LIMIT`).

## Les types de produit : ce que le code teste

`TYPE_PRODUCT` = **0**, `TYPE_SERVICE` = **1** — attesté dans le fichier
lui-même, qui écrit les deux formes côte à côte (`$type == 0` ligne 545,
`Product::TYPE_PRODUCT` ligne 216). `TYPE_SERVICE_OR_PRODUCT` : **0 occurrence**.

Le type n'est pas décoratif, il commande **7** comportements :

1. **Deux modules de droits séparés** : `restrictedArea($user, 'produit')` ou
   `'service'` (lignes 205-210), et un produit n'ouvre pas un service.
2. **Les 3 permissions de la page** sont recalculées à partir du type
   (lignes 216-218) : `(produit|lire)` **ou** `(service|lire)`, idem créer et
   supprimer. Un utilisateur peut donc créer des services sans voir les produits.
3. **`stockable_product`** (ligne 545) : un produit l'est toujours ; un service
   seulement si `STOCK_SUPPORTS_SERVICES` est actif.
4. **Champs réservés au service** : `duration_value`/`duration_unit`,
   `mandatoryperiod`, `fk_default_workstation` (module `workstation`).
5. **Champs réservés au produit** : `finished`, `fk_default_bom` (nomenclature),
   les codes douanes, les lots.
6. **Défauts comptables différents** : `ACCOUNTING_PRODUCT_*` contre
   `ACCOUNTING_SERVICE_*` (6 champs, lignes 1896-1960).
7. **Le libellé de la référence change** : `ProductRef` contre `ServiceRef`
   (ligne 160) — le message d'erreur n'est pas le même.

Le test de type apparaît **17 fois sur 16 lignes** : 13 `isService()` (dont 6
niés) et 4 `isProduct()`. Les variantes (module `variants`) ne servent qu'à
afficher le produit parent (ligne 2897, qui porte les deux tests) — **aucune
action** de variante ici.

## Les 13 droits, et les droits fins

```
produit/lire         produit/creer        produit/supprimer
service/lire         service/creer        service/supprimer
societe/creer                              (fusion)
propal/creer   ×2    commande/creer ×2    facture/creer ×2
bom/write      ×2                          (clone de nomenclature)
barcode/lire_advance ×3   barcode/creer_advance ×1
```

Les deux derniers sont les **droits fins** : ils ne s'activent que si
`MAIN_USE_ADVANCED_PERMS` est positionné (ligne 235). Sans lui, la création de
code-barres est ouverte à quiconque a `produit/creer`.

Détail à retenir : la fusion (`confirm_merge`) est gardée par
**`societe/creer`** — un droit de tiers, pas un droit produit (ligne 269).
C'est un reste de copier-coller des fiches société, et ça se voit.

## Les 8 familles demandées, et ce qu'elles contiennent ici

| Famille | Actions | Verdict |
|---|---|---|
| Identité | **17** | tout est là |
| Documents liés | **1** (`addin`) | + 2 via `include` (PDF, e-mail) |
| Prix et marges | **0** | les prix ne s'écrivent que dans `add` |
| Stock et entrepôt | **0** | 4 champs, aucune action |
| Lots et séries | **0** | 5 champs, aucune action |
| Variantes et combinaisons | **0** | un affichage, rien d'autre |
| Traductions | **0** | ni champ, ni action |
| Autres | **0** | — |

C'est le résultat le plus net de cette extraction : **17 des 18 actions portent
sur l'identité du produit**. La fiche produit n'est pas une fiche « tout-en-un »
comme la facture (81 actions dont 20 sur les lignes) : c'est un formulaire
d'identité avec trois appendices (code-barres, copie, fusion). Les prix, le
stock, les lots et les variantes vivent dans d'autres écrans, et le fichier le
dit lui-même — il n'y a simplement pas de code pour ça ici.

## L'écart avec notre noyau

Notre écran produit, c'est 7 routes (`crates/web/src/main.rs` lignes 164-169 et
343-346, plus import/export lignes 330 et 337) et `db::product` (create, update,
get, list, set_hidden, delete).

| | Dolibarr `card.php` | Noyau Rust | Rapport |
|---|---|---|---|
| Actions distinctes | **18** (+4 includes) | **6** (créer, modifier, consulter, lister, importer, exporter) | **33 %** |
| Champs de saisie | **77** | **8** (`ref`, `label`, `prix`, `tva`, `unite`, `nature`, `description`, `vente`) | **10 %** |
| Familles avec du contenu | **2 / 8** (identité, documents liés) | **3 / 8** (identité, prix, stock) | — |
| Droits distincts | **13** | **1** (`/produits` → `[9001, 31]`) | **8 %** |
| Statuts | **3** (`statut`, `statut_buy`, `status_batch`) | **1** (`tosell`, via « En vente » oui/non) | — |
| Types | 2, avec 7 comportements dérivés | 2, avec 1 comportement (`est_service()`) | — |
| Prix saisissables | 4 + N niveaux de multi-prix | **2** (`prix` HT, `tva`) | — |

Le « 3 / 8 » n'est pas une victoire : il vient de ce que notre noyau n'a qu'un
écran là où Dolibarr en a quatre (fiche, prix, stock, lots). La colonne Dolibarr
compte **un seul fichier**. Sur ce fichier, 6 des 8 familles sont vides.

Trois écarts qui méritent d'être nommés plutôt que comptés :

1. **Le droit d'ouvrir les produits, chez nous, c'est le droit des tiers.**
   `droits_du_chemin("/produits")` renvoie `[9001, 31]` : le premier identifiant
   est la définition **« Tiers »** (`crates/db/src/droit.rs:196`). Un
   utilisateur à qui on donne « Tiers » ouvre les produits. Les 6 droits
   `produit/*` et `service/*` de Dolibarr ne sont pas distingués : le droit du
   coeur `31` n'est qu'une alternative, c'est `9001` (Tiers) qui ouvre en
   pratique.
2. **Le stock, chez nous, existe — et pas dans ce fichier.** `pages.rs:728-875`
   fait des niveaux par entrepôt, des mouvements et la création d'entrepôt
   (7 champs, 2 écritures). `card.php` n'a **aucune** action de stock : il
   affiche le seuil d'alerte et le stock désiré, et délègue le reste. Sur ce
   point précis notre écran en fait plus que la fiche qu'on imite.
3. **On ne peut pas changer la nature d'un produit chez nous.** Dolibarr a une
   action dédiée (`setfk_product_type`), que l'indicateur global
   `PRODUCT_DENY_CHANGE_PRODUCT_TYPE` masque si on veut la verrouiller
   (ligne 2626) ; notre écran ne propose `nature` qu'à la création
   (`main.rs:3084-3089`), jamais à la modification.

## Trois découvertes sur les prix

### 1. Le prix ne s'écrit qu'à la création — `update` ne le touche jamais

Mesuré : les 4 champs de prix sont lus aux lignes **558-571**, dans le bloc
`$action == 'add'`. Le bloc `update` (lignes 790-963) ne lit **aucun** des
quatre : zéro occurrence de `price`, `price_min`, `price_base_type` ou `tva_tx`
entre 790 et 963. Et le fichier assume le procédé pour le stock, avec ce
commentaire (ligne 823) : « removed from update view so GETPOST always empty ».

Notre noyau fait exactement l'inverse : `POST /produits/:id/modifier` écrit
`price` et `tva_tx` (`main.rs:3024-3039`, `db::product::update`). Ce n'est pas
un défaut — notre écran est la seule page, la référence en a une autre. Mais il
faut le savoir : **corriger un prix chez Dolibarr n'est pas une opération de la
fiche produit**, et une ligne déjà émise ne bouge plus.

### 2. Deux colonnes pour un seul champ, et deux arrondis chez nous

`price_base_type` (HT ou TTC, défaut global `PRODUCT_PRICE_BASE_TYPE`) décide
*laquelle* des deux colonnes est renseignée :

```php
if ($object->price_base_type == 'TTC') {
    $object->price_ttc = GETPOSTFLOAT('price');   // price laissé à dériver
} else {
    $object->price     = GETPOSTFLOAT('price');   // price_ttc laissé à dériver
}
```

La même case `price` alimente donc `price` **ou** `price_ttc` selon une liste
déroulante, et c'est `Product::create()` qui calcule l'autre. Idem pour le prix
mini (`price_min` / `price_min_ttc`). Le multi-prix suit la même logique :
`price_2` … `price_N` par **niveau de prix du tiers** (`$soc->price_level`), et
un prix par client daté (`PRODUIT_CUSTOMER_PRICES`) dont le premier intervalle
valide gagne — `break` ligne 1191, sans tri explicite.

Conséquence pour notre arithmétique : nous saisissons toujours du HT et nous
calculons le TTC, mais **pas par le même chemin**. Dans
`crates/db/src/product.rs` :

- `create` (lignes 188-191) : `(prix * (1 + taux)).round_dp(8)` — 8 décimales,
  arrondi par défaut de `rust_decimal` ;
- `update` (ligne 248) : `money::round_unit_price(...)` — **6** décimales, demi
  hors de zéro, c'est-à-dire l'équivalent de `price2num($x, 'MU')`.

Deux précisions différentes pour la même colonne, sur deux chemins. C'est le
seul **prix stocké** du noyau qui échappe à `money` : les deux autres `round_dp(`
sans stratégie explicite sont un arrondi d'affichage (`crates/web/src/ui.rs:219`)
et un arrondi de ticket (`crates/pos/src/ticket.rs:53`).

### 3. Le prix d'achat de la marge n'est pas `cost_price`

Le fichier ne lit **jamais** `llx_product.cost_price` — ni `pmp`, ni `fifo`,
ni `lifo`, ni `price_label`, ni `price_autogen`, ni `fk_price_expression`
(0 `GETPOST` pour chacun, alors que les 7 colonnes existent au schéma). Le prix
d'achat servant à la marge vient d'un appel au document :

```php
$buyprice = 0;
if (($result = $propal->defineBuyPrice($pu_ht, $remise_percent, $object->id)) < 0) {
    setEventMessages($langs->trans('FailedToGetCostPrice'), null, 'errors');
} else {
    $buyprice = $result;
}
```

**3 appels** (propale ligne 1214, commande 1254, facture 1296), un par type de
document, avec `0` en repli et un message d'erreur explicite. Le `$buyprice`
part ensuite dans `addline()` : la marge est figée **au moment de la ligne**,
pas recalculée depuis la fiche produit.

Et le prix est rejoué au taux de TVA de l'opération (lignes 1201-1209) : si le
taux du client diffère de celui du produit, le prix est reconverti
(`pu_ttc / (1 + tva/100)` ou `pu_ht * (1 + tva/100)`, arrondi 6 décimales).
Notre noyau n'a **ni `cost_price`, ni `buy_price`, ni `pmp`, ni `price_min`**
(0 occurrence dans `crates/*/src/*.rs`) : la marge n'existe nulle part, elle
n'est donc pas fausse — elle est absente.

## Deux pièges vus au passage

1. **`price()` relit un prix.** Lignes 1187-1188, dans la branche prix client :
   `$pu_ht = price($custprice_line->price);`. `price()` est le **formateur
   d'affichage** (il renvoie une chaîne avec séparateurs), pas un lecteur. Ces
   2 appels sont les seuls du fichier, et le résultat repart dans le calcul.
   À ne pas imiter : chez nous, `Decimal` d'un bout à l'autre.
2. **`PRODUIT_MULTIPRICES` et `PRODUIT_CUSTOMER_PRICES` sont exclusifs dans le
   calcul** (`elseif` ligne 1174) alors que le formulaire de création teste
   `PRODUIT_MULTIPRICES || PRODUIT_CUSTOMER_PRICES_AND_MULTIPRICES`
   (ligne 1844). Avec les deux options actives, le prix client n'est jamais
   appliqué dans `addin`.

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

## Ce que cette fiche décide pour la suite

- **À porter** : rien d'urgent. La fiche produit est à 33 % en actions mais les
  actions manquantes sont les onglets (prix, stock, lots, variantes), qui sont
  d'autres écrans — les compter ici serait malhonnête.
- **À assumer** : `price_min` et la marge (`cost_price` / `buy_price` / `pmp`)
  sont absents du noyau. Pour un maçon qui chiffre au mètre, la marge par ligne
  est pourtant l'information la plus utile — c'est un manque à combler, pas un
  écart à ignorer.
- **À corriger tout de suite** : l'arrondi du TTC à la création (8 décimales)
  qui ne passe pas par `money::round_unit_price` (6 décimales).
- **À ne pas copier** : `price()` comme lecteur, et `societe/creer` comme garde
  de la fusion.
