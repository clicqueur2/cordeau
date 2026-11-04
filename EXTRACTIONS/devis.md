# Extraction : fiche devis Dolibarr

Deuxième fiche, même méthode que `facture.md`. Source : le vrai
`htdocs/comm/propal/card.php` de la production (Dolibarr 24.0.1), téléchargé une
fois puis analysé hors ligne.

```
htdocs/comm/propal/card.php   170 154 octets   3 871 lignes
```

> Note de méthode : le brief annonçait 3 427 lignes, le fichier en fait **3 871**
> (mesuré : 3 871 retours à la ligne, aucun CRLF). Tous les chiffres ci-dessous
> sont recomptés sur ce fichier, pas repris du brief.

> Note de mesure : les actions sont comptées par les comparaisons sur
> `$action` (`==`, `===`, `!=`, `!==`, guillemets simples et doubles) ;
> les champs par tous les appels `GETPOST*` (`GETPOST`, `GETPOSTINT`,
> `GETPOSTFLOAT`, `GETPOSTISSET`, `GETPOSTISARRAY`) plus les accès directs
> `$_POST`/`$_GET` ; les droits par les appels `hasRight(...)`.

## Ce que la fiche contient réellement

| Mesure | Valeur |
|---|---|
| **Actions distinctes** | **65** (pour 93 comparaisons sur `$action`) |
| dont branches d'écriture traitées dans le fichier | 61 |
| dont états d'affichage (`editline`, `classify`, `selectlines`) | 3 |
| dont action déléguée à un include (`presend`) | 1 |
| Champs de formulaire distincts | **152** bruts, **116** logiques |
| Appels `GETPOST*` | 360 |
| Statuts référencés | **6** |
| Droits vérifiés dans ce fichier | **15** (18 appels) |

Les 152 champs bruts comptent séparément les composants de date
(`date_startday`, `date_startmonth`…) : après fusion en 10 groupes, il reste
**116 champs logiques**. C'est le chiffre à comparer au nôtre.

## Les 65 actions, classées

| Famille | Nombre |
|---|---|
| Édition de l'en-tête (champ par champ) | **26** |
| Validation, signature, cycle de vie | **10** |
| Lignes du devis | **8** |
| Sections et sous-totaux | **8** |
| Création et copie | **4** |
| Contacts liés | **3** |
| Champs libres et étiquettes | **3** |
| Suppression | **2** |
| Envoi et PDF | **1** |
| **Total** | **65** |

**Édition de l'en-tête (26)**
`setdate`, `editdate`, `setecheance`, `editecheance`, `setdate_livraison`,
`setref_client`, `setconditions`, `editconditions`, `setmode`, `editmode`,
`setbankaccount`, `editbankaccount`, `setavailability`, `editavailability`,
`setdemandreason`, `editdemandreason`, `setshippingmethod`,
`editshippingmethod`, `setwarehouse`, `editwarehouse`, `set_incoterms`,
`editincoterm`, `setmulticurrencycode`, `setmulticurrencyrate`, `classin`,
`classify`

Dix paires `set*`/`edit*` (dont une mal nommée : `set_incoterms` /
`editincoterm`) plus six actions isolées (`setdate_livraison`, `setref_client`,
`setmulticurrencycode`, `setmulticurrencyrate`, `classin`, `classify`) : la fiche
est un **formulaire par champ**, pas un formulaire global. `classify` n'écrit
rien — c'est l'état d'affichage du sélecteur de projet, `classin` fait
l'écriture.

**Validation, signature, cycle de vie (10)**
`validate`, `confirm_validate`, `modif` (retour au brouillon), `closeas`,
`confirm_closeas`, `reopen`, `confirm_reopen`, `cancel`, `confirm_cancel`,
`classifybilled`

**Lignes du devis (8)**
`addline`, `updateline`, `editline`, `ask_deleteline`, `confirm_deleteline`,
`setabsolutediscount`, `selectlines`, `import_lines_from_object`

**Sections et sous-totaux (8)**
`add_title_line`, `confirm_addtitleline`, `updatetitleline`,
`add_subtotal_line`, `confirm_addsubtotalline`, `updatesubtotalline`,
`ask_subtotal_deleteline`, `confirm_delete_subtotalline`

**Création et copie (4)**
`add`, `create`, `clone`, `confirm_clone`

**Contacts liés (3)**
`addcontact`, `deletecontact`, `swapstatut` (bascule le statut d'un contact,
pas celui du devis)

**Champs libres et étiquettes (3)**
`settags`, `edittags`, `update_extras`

**Suppression (2)**
`delete`, `confirm_delete`

**Envoi et PDF (1)**
`presend`

> **Attention au compteur.** `presend` n'est comparé qu'avec `!=` dans ce
> fichier : l'action est traitée par `core/actions_sendmails.inc.php`, inclus à
> la ligne 1036. Le fichier inclut 6 gestionnaires partagés
> (`actions_setnotes`, `actions_dellink`, `actions_lineupdown`,
> `actions_printing`, `actions_sendmails`, `actions_builddoc`) qui ajoutent leurs
> propres actions (`builddoc`, `send`, `setnote`, `dellink`, `up`, `down`…).
> **La surface réelle de la page dépasse donc 65** ; ces actions-là ne sont pas
> comptées ici parce qu'elles ne sont pas définies dans ce fichier. Je ne donne
> pas de total « 65 + n » : je ne l'ai pas mesuré.

## Les 6 statuts référencés

`DRAFT`, `VALIDATED`, `SIGNED`, `NOTSIGNED`, `BILLED`, `CANCELED`

Le fichier ne définit pas les valeurs numériques (elles sont dans
`propal.class.php`, non téléchargé). Deux seulement sont mesurables ici :
`DRAFT` = 0 (via `fk_statut <> 0`, ligne 2742) et `NOTSIGNED` = 3
(via l'URL `?statut=3` du bouton « refuser », ligne 2990). **Les autres valeurs
ne sont pas mesurées dans ce fichier.**

`SIGNED` et `NOTSIGNED` sont les deux seules issues de `closeas` : la
signature n'est pas un booléen, c'est un statut.

## Les 15 droits vérifiés

```
propal/creer            propal/lire             propal/supprimer
propal/propal_advance/close    propal/propal_advance/reopen
propal/propal_advance/send     propal/propal_advance/validate
commande/creer          facture/creer           contrat/creer
ficheinter/creer        fournisseur/commande/creer
supplier_order/creer    margins/creer           produit/ignore_price_min_advance
```

Sept portent sur `propal` (dont 4 sous-droits de `propal_advance`), huit
portent sur d'**autres modules** — c'est la signature des conversions : pour
convertir, le fichier vérifie le droit du module de destination, pas le sien.

Les 4 sous-droits `propal_advance` ne s'appliquent que si
`MAIN_USE_ADVANCED_PERMS` est actif ; sinon `propal/creer` suffit.

## L'écart avec notre noyau

Côté Rust (`crates/web/src/main.rs`) l'écran devis, c'est 9 routes :
`GET /devis`, `GET /devis/:id`, `GET /devis/:id/pdf`, `POST /devis/:id/facturer`,
`POST /devis/:id/situation`, `POST /devis/:id/commander`,
`GET|POST /devis/nouveau`, `GET|POST /devis/:id/modifier`,
`POST /devis/:id/valider`. `crates/web/src/pages.rs` n'ajoute rien sur le devis
(contrats, expéditions et commandes fournisseurs n'y sont que des listes en
lecture, avec « devis » comme onglet actif).

Six actions écrivent : créer, enregistrer/modifier, valider, facturer,
commander, situation. Une septième produit le PDF.

| | Dolibarr | Noyau Rust | Écart |
|---|---|---|---|
| Actions sur la fiche | **65** | **7** (6 écritures + PDF) | **10,8 %** |
| Actions d'écriture | 61 | 6 | 9,8 % |
| Champs de saisie | 152 bruts / 116 logiques | 12 logiques | 10,3 % |
| Statuts | 6 | 4 libellés (0, 1 par défaut, 2, 3), vocabulaire de facture | — |
| Droits | 15 (7 sur `propal`) | **1** (`9002`/`21`, un droit de lecture) | 6,7 % |
| Lignes du devis | 8 actions + 8 sections/sous-totaux | remplacement en bloc, 10 lignes fixes | — |
| Suppression | 2 actions | aucune route (la fonction `db::proposal::delete` existe, l'écran ne l'appelle pas) | — |
| Envoi par mail | `presend` + `send` (include) | aucune route | — |

Autrement dit : **la fiche devis est à 10,8 %** — 7 actions sur 65, mesuré, pas
estimé. C'est mieux que la fiche facture (6 %) mais du même ordre.

Champs : nos deux formulaires offrent 21 champs (création) et 55 (modification,
10 lignes × 5 colonnes), pour 12 noms logiques distincts. Deux champs sont lus
par `devis_enregistrer` sans qu'aucun formulaire ne les envoie : `ref_client` et
`date_prestation`.

## Ce que cette extraction apprend, au-delà du chiffre

1. **Chez Dolibarr, un devis validé n'est pas un devis signé, et seul le signé
   se convertit.** `validate` met le statut à 1 ; cinq boutons de conversion
   (commande, commande fournisseur, facture, intervention, contrat) sont
   conditionnés à `STATUS_SIGNED`, et `classifybilled` aussi ; `closeas`
   n'apparaît qu'au statut `VALIDATED`, donc le circuit est valider *puis*
   clore. Un devis validé mais non clos ne se facture pas. Chez nous,
   `devis_fiche` affiche « Facturer ce devis » et « Créer la commande » dès que
   le devis n'a pas de situation — **sans regarder le statut** — et
   `invoice::create_from_proposal` ne vérifie ni statut ni validation (seulement
   « au moins une ligne » et « pas déjà facturé »), alors que
   `commande::create_from_proposal`, lui, refuse en dessous de `fk_statut < 1`.
   Un brouillon est donc facturable par notre écran et pas par Dolibarr. Ce
   n'est pas une opinion : c'est la lecture des deux fichiers.

2. **L'acompte à la signature existe en natif dans Dolibarr 24.** `confirm_closeas`
   peut appeler `Facture::createDepositFromOrigin(...)` dans la même action, avec
   le pourcentage porté par la condition de règlement (`deposit_percent` du
   dictionnaire `c_payment_term`), sous l'option
   `PROPOSAL_SUGGEST_DOWN_PAYMENT_INVOICE_CREATION`. C'est exactement ce que fait
   le workflow n8n « devis signé → facture d'acompte », à la main, depuis trois
   semaines. Le natif suppose d'activer une option cachée et de saisir le
   pourcentage dans le dictionnaire des conditions de règlement.

3. **La signature en ligne n'est pas une action de cette page.** Le fichier
   n'expose ni `sign` ni `signature_url` : il appelle `signature.lib.php` et
   affiche un lien (`PROPOSAL_ALLOW_ONLINESIGN`). Le circuit de signature est
   donc **hors** de la fiche — ce qui veut dire que notre découpage (lien de
   signature servi par `signature_url.php`, statut mis à jour ailleurs) n'est pas
   une hérésie : Dolibarr fait pareil.

4. **Notre modèle de droits ne sait pas distinguer lire de valider.** L'écran
   devis est gardé par un seul droit (`crates/db/src/droit.rs` : `/devis` →
   `9002` ou `21`, soit « lire les propositions commerciales »). Dolibarr en
   vérifie 15 sur la même page, dont `creer`, `supprimer` et les 4 sous-droits
   `propal_advance`. Chez nous, qui lit peut valider, facturer et faire une
   situation.

5. **Le numéro se pose à deux moments différents.** Dolibarr crée en `PROV…`
   puis renumérote en `PR…` à la validation (l'action `validate` recalcule la
   référence avant de changer le statut). Notre `db::proposal::validate` change
   le statut et rien d'autre : la référence définitive est attribuée dès la
   création. Conséquence assumée, mais à connaître : chez nous un devis supprimé
   avant validation a consommé un numéro définitif.

## Ce qui reste à vérifier

- La correspondance numérique des 6 statuts : lire `htdocs/comm/propal/class/propal.class.php`
  (non téléchargé). Deux valeurs sur six sont mesurées ici.
- Le nombre exact d'actions ajoutées par les 6 includes partagés (`builddoc`,
  `send`, `setnote`, `dellink`, `up`, `down`…) : ils ne sont pas dans le dépôt.
- Le libellé de statut affiché par notre `ui::pastille` sur un devis : elle
  applique le vocabulaire de la **facture** au devis (`statut == 2` → « payée »,
  `statut == 3` → « abandonnée »), alors qu'en 24.0.1 ces deux valeurs sont
  « signé » et « non signé ». À trancher une fois les valeurs confirmées.

## La méthode, pour les fiches suivantes

```
1. telecharger le card.php du module          (FTP, lecture seule)
2. extraire : actions, champs, droits, statuts
3. classer les actions par famille
4. compter l'ecart avec le noyau
5. decider : porter, ou assumer le manque
```

Une fiche = un fichier `EXTRACTIONS/<module>.md`. Le décompte est vérifiable, la
décision est explicite, et rien n'est estimé.
