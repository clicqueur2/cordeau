# Extraction : fiche commande Dolibarr

Deuxième fiche extraite, même méthode que `facture.md`. Source : le vrai
`htdocs/commande/card.php` de Dolibarr 24.0.1, lu en local (pas de réseau).

```
htdocs/commande/card.php   162 290 octets   3 283 lignes
```

## Ce que la fiche contient réellement

| Mesure | Valeur |
|---|---|
| **Actions distinctes** | **69** |
| Comparaisons sur `$action` | 84 (69 distinctes) |
| Appels `GETPOST` | 164 |
| **Champs distincts lus** | **77** |
| Statuts (constantes nommées) | 5 (`DRAFT`, `VALIDATED`, `SHIPMENTONPROCESS`, `CLOSED`, `CANCELED`) |
| **Couples de droits vérifiés** | **13** (sur 9 modules) |

Les 69 actions sortent de `$action == '...'` et `$action != '...'`
(`$action = GETPOST('action', 'aZ09')`, une seule affectation). Aucun
`switch ($action)` : ce sont des `if/elseif` en cascade. Les 4 `case` du
fichier portent sur `$object->element` (`Commande`, `Propal`, `Contrat`,
`Expedition`), pas sur l'action.

Sur les 77 champs : 79 `GETPOST` passent un type explicite, 78 n'en passent
pas. 8 types utilisés — `alpha`, `alphanohtml`, `array`, `array:alphanohtml`,
`array:int`, `aZ09`, `int`, `restricthtml`.

## Les 69 actions, classées

**Cycle de vie (11)**
`validate`, `confirm_validate`, `modif`, `confirm_modif`, `shipped`,
`confirm_shipped`, `reopen`, `classify`, `classifybilled`,
`classifyunbilled`, `classin`

**Annulation et suppression (5)**
`cancel`, `confirm_cancel`, `delete`, `confirm_delete`, `swapstatut`

**Création et copie (4)**
`create`, `add`, `clone`, `confirm_clone`

**En-tête — édition champ par champ (23)**
`setref_client`, `setdate`, `editdate`, `setdate_livraison`,
`editdate_livraison`, `setconditions`, `editconditions`, `setmode`,
`editmode`, `setbankaccount`, `editbankaccount`, `set_incoterms`,
`editincoterm`, `setposinfo`, `editposinfo`, `setshippingmethod`,
`editshippingmethod`, `setwarehouse`, `editwarehouse`, `setavailability`,
`editavailability`, `setdemandreason`, `editdemandreason`

**Lignes du document (17)**
`addline`, `updateline`, `editline`, `ask_deleteline`, `confirm_deleteline`,
`add_title_line`, `confirm_addtitleline`, `updatetitleline`,
`add_subtotal_line`, `confirm_addsubtotalline`, `updatesubtotalline`,
`ask_subtotal_deleteline`, `confirm_delete_subtotalline`,
`setabsolutediscount`, `setremise`, `editinputmode`,
`import_lines_from_object`

**Champs libres et étiquettes (3)**
`update_extras`, `settags`, `edittags`

**Contacts liés (2)**
`addcontact`, `deletecontact`

**Multi-devise (2)**
`setmulticurrencycode`, `setmulticurrencyrate`

**Envoi et sélection (2)**
`presend`, `selectlines`

Total : 11 + 5 + 4 + 23 + 17 + 3 + 2 + 2 + 2 = **69**.

Le motif `set…` / `edit…` explique à lui seul 46 actions sur 69 (23 paires) :
l'écran n'a pas d'édition en place, il ouvre un champ, puis enregistre.

## Les 13 couples de droits

```
commande/creer                commande/lire
commande/supprimer            commande/order_advance
contrat/creer                 expedition/creer
expedition/delivery           facture/creer
ficheinter/creer              fournisseur/commande
margins/creer                 produit/ignore_price_min_advance
supplier_order/creer
```

`commande/order_advance` n'est pas un droit simple : il porte **5 sous-droits**
(`validate`, `close`, `annuler`, `send`, `generetedoc`), branchés uniquement si
`MAIN_USE_ADVANCED_PERMS` est actif. 9 drapeaux `$usercan*` en découlent :
`usercanread`, `usercancreate`, `usercandelete`, `usercanvalidate`,
`usercancancel`, `usercanclose`, `usercansend`, `usercangeneratedoc`,
`usercancreatepurchaseorder`.

Aucune forme héritée `$user->rights->commande->…` ne subsiste dans ce fichier :
tout passe par `hasRight()`.

## L'écart avec notre noyau

Ce que notre écran sait faire, mesuré dans `crates/web/src/main.rs` :

```
GET  /commandes                     liste
GET  /commandes/:id                 fiche (ref, client, date, lignes, totaux)
POST /commandes/:id/valider         validation
POST /commandes/:id/facturer        facture depuis la commande
GET  /commandes/:id/pdf             bon de commande
POST /devis/:id/commander           création depuis un devis validé
```

5 fonctions dans `crates/db/src/commande.rs` (`list`, `get`, `lines`,
`create_from_proposal`, `validate`) et `db::invoice::create_from_commande`.

| | Dolibarr | Noyau Rust |
|---|---|---|
| Actions sur la fiche | **69** | **4** (créer, valider, facturer, PDF) |
| Champs de saisie sur la fiche | 77 | **0** (uniquement des boutons) |
| Statuts | 5 | 4 |
| Droits | 13 couples (4 sur `commande`) | **1** (id 81 = lire) |

**La fiche commande est à 5,8 %.** Ce n'est pas une estimation : 4 / 69 sur le
fichier réel. Aucune route ne contient `exped` en écriture — `/expeditions`
existe mais en lecture seule (`pages::expeditions`, `llx_expedition`), donc
« expédier » n'est pas à 0 % : il n'est pas là du tout.

Deux écarts de fond que le comptage révèle :

- **Les statuts ne portent pas les mêmes noms.** Dolibarr nomme 5 états, dont
  `SHIPMENTONPROCESS` et `CLOSED`. Notre noyau déclare `STATUT_LIVREE = 2` et
  `STATUT_ANNULEE = 3`. Or Dolibarr n'a pas d'état « livrée » (l'état 2 est
  « en cours d'expédition ») et son état 3 est la clôture. La pastille de notre
  liste affiche donc 2 = « livrée » et 3 = « annulée » là où Dolibarr écrit
  autre chose. Le mapping numérique exact est à trancher sur
  `commande.class.php`, absent de cette extraction : je ne l'affirme pas.
- **L'acompte est affiché mais jamais déclenché.** La fiche lit
  `generate_deposit` avec `deposit_percent_from_payment_terms`. Notre struct
  `Commande` porte déjà `deposit_percent` et `commande_pdf` l'imprime (« X %
  = Y € »). Mais `commande_facturer` crée une facture pleine : le champ est lu,
  imprimé, et inutilisé.

## Ce que cette extraction apprend, au-delà du chiffre

1. **Deux paires d'actions mentent sur leur nom.** `modif` / `confirm_modif` ne
   modifient rien : le libellé est `ConfirmUnvalidateOrder`, c'est le **retour
   en brouillon**. Et `shipped` / `confirm_shipped` n'expédient rien : le
   libellé est `CloseOrder` / `ConfirmCloseOrder`, c'est la **clôture**. Un
   relevé par nom de variable les range en « édition » et « expédition » ; le
   relevé par libellé les met en cycle de vie. `facture.md` a classé
   `modif`/`confirm_modif` dans « édition de la pièce » — même piège, et il
   faudra y revenir.
2. **69 est un plancher, pas un total.** La fiche inclut 8 fichiers d'actions
   partagés : `actions_sendmails`, `actions_builddoc`, `actions_setnotes`,
   `actions_dellink`, `actions_addupdatedelete`, `actions_lineupdown`,
   `actions_printing` (7 porteurs d'actions) plus `actions_fetchobject`
   (utilitaire). `send`, `builddoc`, `setnotes`, `confirm_paid` ne sont donc
   pas dans les 69 : ils sont dans les includes. Le vrai total de la fiche est
   69 + le contenu de ces 7 fichiers.
3. **L'édition champ par champ est le vrai volume.** 23 actions d'en-tête pour
   12 champs réels : 11 paires `set…`/`edit…` (date, date de livraison,
   conditions de règlement, mode de règlement, compte bancaire, incoterm,
   entrepôt, disponibilité, motif d'annulation, méthode d'expédition, POS) plus
   `setref_client`, qui n'a pas de paire. Le noyau n'en expose aucun : ce n'est
   pas 23 fonctionnalités à porter, c'est **un formulaire d'édition de 12
   champs** à écrire une fois.

## La méthode, pour les fiches suivantes

```
1. télécharger le card.php du module          (FTP, lecture seule)
2. extraire : actions, champs, droits, statuts
3. classer les actions par famille
4. vérifier le libellé de chaque action ambiguë ($langs->trans(...))
5. compter l'écart avec le noyau
6. décider : porter, ou assumer le manque
```

L'étape 4 est nouvelle : elle vient de `modif` et `shipped`. Un nom d'action
n'est pas une preuve de ce que fait l'action.

Une fiche = un fichier `EXTRACTIONS/<module>.md`. Le décompte est vérifiable, la
décision est explicite, et rien n'est estimé.
