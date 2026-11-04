# Extraction : fiche facture fournisseur Dolibarr

Deuxième fiche extraite **une par une**, même méthode que `facture.md`. Source :
le vrai `htdocs/fourn/facture/card.php` de ta production, téléchargé une fois en
local, lu hors ligne.

```
htdocs/fourn/facture/card.php   191 Ko   3 986 lignes
```

> Note de chemin : la facture **client** est dans `htdocs/compta/facture/`, la
> facture **fournisseur** est dans `htdocs/fourn/facture/`. Deux modules, deux
> arborescences, aucun code partagé à part `core/actions_*.inc.php`.

## Ce que la fiche contient réellement

| Mesure | Valeur |
|---|---|
| **Actions distinctes** | **58** |
| Champs de formulaire distincts | **119** (60 `GETPOST` + 73 `GETPOSTINT` + 1 `GETPOSTFLOAT`, 15 en double) |
| Statuts | 4 (`DRAFT`, `VALIDATED`, `CLOSED`, `ABANDONED`) |
| Droits vérifiés dans ce fichier | **10 chaînes** sur **8** sites d'appel |

Trois précisions de comptage, pour que le chiffre soit vérifiable :

- **58 actions** = 53 comparaisons `$action == '…'` + 1 en guillemets doubles
  (`setabsolutediscount`) + 4 qui n'apparaissent que dans un garde
  `$action != '…'` (`editline`, `editdate_lim_reglement`, `editincoterm`,
  `presend` — ce sont des états d'affichage de formulaire, donc de vraies
  actions).
- **6 includes partagés** ajoutent des actions qui ne sont pas écrites ici :
  `actions_setnotes.inc.php`, `actions_dellink.inc.php`,
  `actions_lineupdown.inc.php`, `actions_printing.inc.php`,
  `actions_sendmails.inc.php`, `actions_builddoc.inc.php`. Elles existent sur
  cette page mais vivent dans un autre fichier : **non comptées** dans les 58.
- **1 opération n'est pas une action** : le rattachement à une commande
  fournisseur se déclenche sur `GETPOST('linkedOrder')` seul, hors de tout
  `$action` (ligne 202). C'est le seul endroit du fichier où une écriture part
  sans passer par le routeur d'actions.

## Les 58 actions, classées

**Validation et cycle de vie (6)**
`valid`, `confirm_valid`, `paid`, `confirm_paid`, `confirm_paid_partially`,
`reopen`

**Annulation et suppression (5)**
`canceled`, `confirm_canceled`, `delete`, `confirm_delete`, `swapstatut`

**Édition de l'en-tête de pièce (19)**
`edit`, `confirm_edit`, `setref_supplier`, `setdatef`, `setlabel`,
`setconditions`, `editconditions`, `setmode`, `editmode`, `setbankaccount`,
`editbankaccount`, `setdate_lim_reglement`, `editdate_lim_reglement`,
`set_incoterms`, `editincoterm`, `settransportmode`, `edittransportmode`,
`setvatreversecharge`, `editvatreversecharge`

**Lignes du document (9)**
`addline`, `updateline`, `editline`, `ask_deleteline`, `confirm_deleteline`,
`add_title_line`, `add_subtotal_line`, `setabsolutediscount`, `unlinkdiscount`

**Création, copie, conversion (6)**
`create`, `add`, `clone`, `confirm_clone`, `converttoreduc`,
`confirm_converttoreduc`

**Champs libres et étiquettes (3)**
`update_extras`, `settags`, `edittags`

**Paiements (2)**
`deletepayment`, `confirm_delete_paiement`

**Rattachement au projet (2)**
`classify`, `classin`

**Contacts liés (2)**
`addcontact`, `deletecontact`

**Multi-devise (2)**
`setmulticurrencycode`, `setmulticurrencyrate`

**Recalcul (1)**
`calculate`

**Envoi (1)**
`presend`

Somme : 6+5+19+9+6+3+2+2+2+2+1+1 = **58**.

> Correction de lecture utile : `classify` / `classin` ne sont **pas** des
> actions de paiement malgré le nom. `classin` appelle `setProject($projectid)`
> (ligne 2007) — c'est le rattachement de la facture à un projet/chantier. Je
> les avais d'abord rangées avec les paiements, à tort.

## Les 119 champs, ce qu'ils sont vraiment

Le tri brut ne dit pas grand-chose ; ce qui compte, c'est la part de mécanique :

| Famille de champs | Nombre | Exemples |
|---|---|---|
| Fragments de date (`…day`, `…month`, `…year`, `…hour`, `…min`, `…sec`) | **37** | `datefday`, `date_lim_reglementmonth`, `alldate_startyear` |
| Devise | **6** | `multicurrency_code`, `multicurrency_tx`, `multicurrency_price_ht`, `multicurrency_price_ttc`, `multicurrency_subprice`, `originmulticurrency_tx` |
| Montants et TVA | 12 | `price_ht`, `price_ttc`, `tva_tx`, `remise_percent`, `remiseforalllines`, `vat_reverse_charge`, `vatforalllines`, `amount` |
| Liens et origine | 8 | `origin`, `originid`, `linkedOrder`, `fac_replacement`, `fac_avoir`, `fac_rec`, `facid`, `projectid` |
| Reste (tiers, règlement, produit, texte, stock, divers) | 56 | `ref_supplier`, `fourn_ref`, `cond_reglement_id`, `mode_reglement_id`, `fk_account`, `idwarehouse`, `dp_desc`, `modelselected` |

Autrement dit : **sur 119 champs, 37 sont des morceaux de date** produits par les
sélecteurs Dolibarr, pas 37 informations différentes. Un formulaire moderne avec
trois `<input type="date">` couvre la même chose. La matière réelle est de
l'ordre de **80 champs**, dont 6 pour la devise seule.

## Les 4 statuts

| Constante | Valeur | Sens |
|---|---|---|
| `STATUS_DRAFT` | 0 | brouillon, modifiable, sans référence définitive |
| `STATUS_VALIDATED` | 1 | validée, engagée en comptabilité |
| `STATUS_CLOSED` | 2 | clôturée (payée ou soldée) |
| `STATUS_ABANDONED` | 3 | abandonnée |

Les 4 constantes apparaissent dans les comparaisons du fichier. À noter que
`STATUS_CLOSED` est piloté par la colonne `paye` en pratique — Dolibarr écrit
`fk_statut = 2` **et** `paye = 1`.

## Les 10 droits vérifiés dans ce fichier

```
fournisseur / facture                   / lire
fournisseur / facture                   / creer
fournisseur / facture                   / supprimer
fournisseur / supplier_invoice_advance  / validate
fournisseur / supplier_invoice_advance  / send
supplier_invoice / lire
supplier_invoice / creer
supplier_invoice / supprimer
contrat / creer
paymentbybanktransfer / create
```

Trois remarques qui comptent pour la réécriture :

1. **Chaque droit est testé en double** : la nouvelle arborescence
   (`fournisseur/facture/creer`) **ou** l'ancienne (`supplier_invoice/creer`).
   C'est une compatibilité ascendante, pas deux fonctionnalités.
2. **La validation et l'envoi sont derrière un drapeau global**
   (`MAIN_USE_ADVANCED_PERMS`). Sans le drapeau, valider ne demande que le droit
   de créer ; avec, il faut `supplier_invoice_advance/validate`. Donc « qui peut
   créer peut valider » est le comportement par défaut.
3. `usercandelete` est plus permissif que son nom : il est vrai si l'utilisateur
   a le droit de supprimer **ou** s'il peut créer **et** que
   `$object->is_erasable() == 1`. Le droit de suppression est donc contournable
   sur un brouillon.

## L'écart avec notre noyau

Ce que notre écran d'achat sait faire, mesuré dans `crates/web/src/main.rs`
(routes), `crates/db/src/achat.rs` et `crates/db/src/achat_ecriture.rs`.

| | Dolibarr | Noyau Rust |
|---|---|---|
| Actions sur la fiche | **58** | **4** (créer facture, valider facture, créer fournisseur, modifier fournisseur) |
| Champs de saisie | 119 (≈80 hors dates) | **8** logiques (`soc`, `ref_supplier`, `datef`, `note`, et 4 par ligne), 24 noms de formulaire (5 lignes fixes × 4) |
| Statuts | 4 | 4 constantes déclarées, **2 utilisées** |
| Droits | 10 chaînes | **2** (9006 lecture ← 1181, 9015 écriture) |
| Devise | 6 champs + 2 actions | **0** |
| TVA non déductible (`tva_npr`) | gérée | **0** |
| Acompte fournisseur | `TYPE_DEPOSIT`, 20 occurrences | **0** |
| Avoir (`TYPE_CREDIT_NOTE`) | 28 occurrences | **0** côté achat |
| Lien réception / commande | 3 mécanismes | **0** |

Chiffre de couverture : **4 / 58 = 6,9 %**.

Ce que le noyau fait **mieux** que Dolibarr, et qu'il ne faut pas perdre :

- la **référence définitive dès la création** (`FF` + AAMM + `-` + 4 chiffres,
  cherchée sur toutes les périodes comme `mod_facture_fournisseur_cactus`), là
  où Dolibarr crée `(PROV<id>)` puis renumérote ;
- la **numérotation sérialisée** par `GET_LOCK` + réessai sur collision 1062 et
  1213/1205 — deux processus ne peuvent pas tirer le même numéro ;
- le **refus explicite** du doublon `(ref_supplier, fk_soc, entity)`, avec un
  message lisible au lieu d'une erreur 1062 brute ;
- les totaux calculés par `money::line_totals` en `Decimal`, arrondis une seule
  fois à l'en-tête : pas de `double` PHP dans la chaîne.

Ce qu'il ne fait pas du tout : **payer** une facture fournisseur. Il n'existe
aucune fonction d'écriture dans `llx_paiementfourn` — seulement de la lecture
(`divers.rs`, `sepa.rs`). Une facture validée reste donc à `fk_statut = 1`
indéfiniment.

## Le point devise : ce que Dolibarr fait vraiment

Le fichier ne calcule pas d'**écart de conversion**. Il **fige un taux** sur la
pièce et convertit au moment de l'affichage :

```
multicurrency_tx          le taux, stocké sur la facture
setmulticurrencycode      change la devise       (action)
setmulticurrencyrate      change le taux         (action)
```

Deux choses mesurables :

1. **Le taux est un multiplicateur, pas un diviseur.** Ligne 731, pour convertir
   un montant société vers la devise :
   `multicurrency_total_ht = amount_ttc × multicurrency_tx`. Donc
   `montant_devise = montant_societe × tx`.
2. **`setmulticurrencyrate` prend deux arguments** (ligne 484) : le taux, et un
   `calculation_mode` (`GETPOSTINT`). Dolibarr peut donc recalculer soit depuis
   le taux, soit depuis un montant saisi en devise. Je n'ai pas le corps de
   `setMulticurrencyRate` sous les yeux : **la sémantique exacte des valeurs de
   `calculation_mode` est incertaine** (0 = depuis le taux, 1 = depuis le
   montant, ou l'inverse — non vérifié ici).

Conséquence pour nous : il n'y a **rien à porter** côté « écart de change » dans
cette fiche. La perte/gain de change se constate au paiement, dans un autre
fichier. Ce qu'il faut porter, c'est **le taux figé sur la pièce** — sinon un
taux qui bouge entre la saisie et le règlement change le montant dû.

## Le point TVA : l'ordre des montants

C'est le point le plus structurant du fichier, et il n'est pas symétrique.

**Règle générale — le HT est la base, le TTC est dérivé :**

```
ligne 1928-1934
  si price_ht fourni  -> pu_ht = price2num(price_ht)
  sinon               -> pu_ht = pu_ttc / (1 + tva_tx/100)
  price_base_type = 'HT'   (toujours)
```

Donc à la saisie d'une ligne, Dolibarr convertit systématiquement en HT et
stocke HT. La TVA puis le TTC sont recalculés à partir de là. Ordre :
**HT → TVA → TTC**, jamais l'inverse.

**Exception — l'acompte part du TTC et redescend :**

```
ligne 1255
  $amountdeposit[$tva] += $am / (1 + (float) $tva / 100); // Convert into HT
```

et ligne 1929-1932 pour une ligne TTC, même division.

**La TVA non déductible tient dans un astérisque.** Ligne 1911 :

```
$tva_npr = (preg_match('/\*/', $tva_tx) ? 1 : 0);
$tva_tx  = str_replace('*', '', $tva_tx);
```

Un taux saisi `20*` signifie « TVA non perçue / non récupérable » : le `*` est
retiré du taux et `tva_npr = 1` est écrit sur la ligne. Ce drapeau est ensuite
passé à `get_localtax($tva_tx, 1, …, $tva_npr)` (lignes 1844-1845), c'est-à-dire
qu'il **change le calcul des taxes locales**. Sur une facture fournisseur, c'est
exactement le cas de l'autoliquidation en sous-traitance BTP : la TVA figure sur
la facture mais elle n'est pas déductible au même titre.

**Et il y a un second mécanisme, au niveau de la pièce** :
`vat_reverse_charge` (autoliquidation), piloté par `setvatreversecharge` /
`editvatreversecharge` et lu en `GETPOST('vat_reverse_charge') == 'on'`
(lignes 877, 951, 1142). Deux drapeaux pour deux choses voisines :
`tva_npr` est **par ligne**, `vat_reverse_charge` est **par facture**.

## Le point rapprochement : réception et commande

Dolibarr ne lie pas « la facture à la commande » d'une seule façon. Il y a
**trois** chemins, et ils ne sont pas équivalents :

1. **Rattachement direct à une commande fournisseur**, ligne 202 :
   ```php
   if (GETPOST('linkedOrder') && empty($cancel) && $id > 0 && $permissiontoadd) {
       $result = $object->add_object_linked('order_supplier', GETPOSTINT('linkedOrder'));
   }
   ```
   C'est un lien `order_supplier` posé **après coup**, sur une facture
   existante. Ce n'est pas une action routée.

2. **Création depuis une origine** (`origin` + `originid`, lignes 1158-1214) :
   la facture naît d'un autre objet, et `origin_type` / `origin_id` sont posés
   avant `create()`. Le lien est alors
   `linkedObjectsIds[origin_type][-1] = origin_id` (ligne 1200).

3. **Le cas réception, qui lie deux objets d'un coup** (lignes 1203-1212) :
   ```php
   // Add also link with order if object is reception
   if ($object->origin_type == 'reception') {
       $objectsrc->fetchObjectLinked();
       if (count($objectsrc->linkedObjectsIds['order_supplier']) > 0) {
           foreach ($objectsrc->linkedObjectsIds['order_supplier'] as $key => $value) {
               $object->linkedObjectsIds['order_supplier'][-1] = $value;
           }
       }
   }
   ```
   **C'est la découverte de ce fichier.** Facturer une **réception** ne lie pas
   seulement la facture à la réception : Dolibarr remonte la réception vers sa
   commande d'origine (`fetchObjectLinked`) et **ajoute aussi le lien vers la
   commande fournisseur**. Une facture de réception est donc liée à deux
   objets. Si notre écran ne conserve que `origin_id`, il perd la commande — et
   avec elle la possibilité de contrôler l'écart facture/commande.

Bonus du même mécanisme, lignes 2287-2307 : quand l'origine est une réception
et que les conditions de règlement, le mode de règlement, le compte bancaire ou
le mode de transport sont vides sur la facture, Dolibarr remonte à la
**commande** (`$objectsrc->fetch_origin()`) pour les hériter. La réception ne
porte pas ces champs ; la commande si.

## Le point acompte : oui, il est géré ici

`TYPE_DEPOSIT` apparaît **20 fois** dans le fichier, et l'acompte fournisseur
n'est pas un objet séparé : c'est une **facture fournisseur** avec
`GETPOSTINT('type') == FactureFournisseur::TYPE_DEPOSIT`, à une ligne.

Saisie (ligne 1227-1290) : deux modes, `typedeposit` valant `amount` (montant
fixe) ou `variable` (pourcentage).

**Et là il y a une asymétrie qui compte** : le calcul du pourcentage ne porte pas
sur la même base selon un réglage global.

- `MAIN_DEPOSIT_MULTI_TVA` **actif** (ligne 1238) :
  `$amount = $srcobject->total_ttc * (valuedeposit / 100)` → **pourcentage du
  TTC**, éclaté par taux de TVA, puis chaque part redescendue en HT par
  division (ligne 1255).
- `MAIN_DEPOSIT_MULTI_TVA` **inactif** (lignes 1274-1276) :
  `$amountdeposit[$tva_tx] += ($lines[$i]->total_ht * $valuedeposit) / 100`
  → **pourcentage du HT**, une ligne par taux, sans conversion.
  Le code porte d'ailleurs son propre doute, ligne 1274 :
  `// Fixme : is it not for the customer ? Shouldn't we take total_ttc ?`

Un acompte de 30 % ne vaut donc pas la même chose selon le réglage : 30 % du HT
ou 30 % du TTC. Sur 1 000 HT à 20 %, l'écart est de 60 € — et sur un devis de
60 000 € à 30 %, de 3 600 €.

## Ce que cette extraction apprend, au-delà du chiffre

1. **La TVA non déductible s'encode dans la chaîne du taux** (`20*`, `tva_npr`),
   pas dans une colonne dédiée au niveau de la ligne — et un second drapeau
   `vat_reverse_charge` existe au niveau de la pièce. Deux mécanismes voisins,
   deux portées différentes. Pour un maçon qui sous-traite, c'est le cas
   courant, et notre noyau n'a aujourd'hui **aucun** des deux.

2. **Le taux de change est figé sur la pièce, et il multiplie.** Il n'y a pas
   d'écart de conversion à porter ici : ce que notre noyau doit reproduire, c'est
   `montant_devise = montant_societe × tx` stocké avec la facture, pas un moteur
   de change.

3. **Facturer une réception lie deux objets, pas un.** Dolibarr remonte de la
   réception vers sa commande fournisseur (lignes 1203-1212) et en hérite en
   plus les conditions de règlement. C'est le rapprochement à trois pièces
   (commande → réception → facture) qui rend le contrôle des achats possible ;
   c'est exactement ce que notre `factures_fournisseur()` ne sait pas afficher.

## La méthode, pour les fiches suivantes

```
1. télécharger le card.php du module          (FTP, lecture seule)
2. extraire : actions, champs, droits, statuts
3. classer les actions par famille
4. compter l'écart avec le noyau
5. décider : porter, ou assumer le manque
```

Deux réflexes ajoutés après cette fiche :

- **compter les champs par famille, pas en vrac** : 119 champs dont 37 fragments
  de date, ce n'est pas 119 informations ;
- **lire les `!=`, pas seulement les `==`** : quatre actions (`editline`,
  `editincoterm`, `editdate_lim_reglement`, `presend`) n'existent que sous cette
  forme, et un comptage naïf les rate.

Une fiche = un fichier `EXTRACTIONS/<module>.md`. Le décompte est vérifiable, la
décision est explicite, et rien n'est estimé.
