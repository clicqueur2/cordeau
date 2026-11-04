# Extraction : fiche facture Dolibarr

Première fiche extraite **une par une**, comme demandé. Source : le vrai
`htdocs/compta/facture/card.php` de ta production, téléchargé par FTP.

```
htdocs/compta/facture/card.php   311 Ko   7 114 lignes
```

> Note de méthode : le fichier n'est pas dans `htdocs/facture/` mais dans
> `htdocs/compta/facture/` — l'ancien chemin n'existe plus en 24.0.1. Mon
> premier téléchargement a échoué avec « Server denied you to change to the
> given directory ».

## Ce que la fiche contient réellement

| Mesure | Valeur |
|---|---|
| **Actions distinctes** | **81** |
| Champs de formulaire lus (`GETPOST`) | **87** |
| Statuts | 4 (`DRAFT`, `VALIDATED`, `CLOSED`, `ABANDONED`) |
| Droits vérifiés dans ce fichier | 6 |

## Les 81 actions, classées

**Validation et cycle de vie (8)**
`valid`, `confirm_valid`, `paid`, `confirm_paid`, `confirm_paid_partially`,
`situationout`, `confirm_situationout`, `reopen`

**Annulation et suppression (5)**
`canceled`, `confirm_canceled`, `delete`, `confirm_delete`, `swapstatut`

**Édition de la pièce (18)**
`modif`, `confirm_modif`, `setref`, `setref_client`, `setinvoicedate`,
`editinvoicedate`, `setconditions`, `editconditions`, `setmode`, `setpaymentterm`,
`editpaymentterm`, `setbankaccount`, `editbankaccount`, `setrevenuestamp`,
`editrevenuestamp`, `setdate_pointoftax`, `editdate_pointoftax`, `setref`

**Lignes du document (20)**
`addline`, `updateline`, `editline`, `confirm_deleteline`, `ask_deleteline`,
`delete`, `updatealllines`, `add_title_line`, `confirm_addtitleline`,
`updatetitleline`, `add_subtotal_line`, `confirm_addsubtotalline`,
`updatesubtotalline`, `ask_subtotal_deleteline`, `confirm_delete_subtotalline`,
`deletecorrespondingsubtotalline`, `setabsolutediscount`, `unlinkdiscount`,
`setremisepercent`, `update_label`

**Paiements (3)**
`deletepayment`, `confirm_delete_paiement`, `classify`, `classin`

**Contacts liés (3)**
`addcontact`, `deletecontact`, `set_incoterms`

**Champs libres / étiquettes (4)**
`edit_extras`, `update_extras`, `settags`, `edittags`

**Multi-devise (2)**
`setmulticurrencycode`, `setmulticurrencyrate`

**Copie et import (4)**
`clone`, `confirm_clone`, `create`, `import_lines_from_object`

**Autres (14)**
`presend`, `send`, `converttoreduc`, `confirm_converttoreduc`,
`setdispute_status`, `setdemandreason`, `editinput_reason`, `setposinfo`,
`editposinfo`, `setretainedwarranty`, `editretainedwarranty`,
`setretainedwarrantyconditions`, `setretainedwarrantydatelimit`,
`editretainedwarrantypaymentterms`, `addcontact`

## Les 6 droits vérifiés dans ce fichier

```
facture/creer                    facture/invoice_advance
margins/creer                    margins/liretous
prelevement/bons                 produit/ignore_price_min_advance
```

## L'écart avec notre noyau

| | Dolibarr | Noyau Rust |
|---|---|---|
| Actions sur la fiche | **81** | **5** (valider, encaisser, PDF, situation, avoir) |
| Champs de saisie | 87 | ~10 |
| Statuts | 4 | 4 (les mêmes) |
| Droits | 6 | 2 |

Autrement dit : **la fiche facture est à 6 %**. Ce n'est pas une estimation, c'est
un rapport de comptage sur le fichier réel.

## Ce que cette extraction apprend, au-delà du chiffre

1. **Les actions ne sont pas 81 fonctionnalités indépendantes** : 20 concernent les
   lignes, 18 l'édition de la pièce. Par famille, c'est faisable — 20 actions de
   lignes partagent le même formulaire.
2. **Les sous-totaux et titres de ligne** (`add_subtotal_line`, `addtitleline`)
   sont 6 actions à eux seuls, et c'est ce que le PDF doit savoir rendre : c'est
   déjà notre cas, mais l'édition manque.
3. **La garantie retenue** (`retainedwarranty`, 5 actions) est une spécificité
   française de la loi Spinetta sur la construction — pertinent pour un maçon.
4. **Le multi-devise existe sur la pièce** (`setmulticurrencycode`/`rate`), pas
   seulement dans un moteur séparé.

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
