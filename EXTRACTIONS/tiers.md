# Extraction : fiche tiers Dolibarr

Nouvelle fiche extraite, sur le même format que `facture.md` et `commande.md`.
Source : le vrai `htdocs/societe/card.php` de la production, téléchargé par FTP.

```
htdocs/societe/card.php   164 406 octets   3 699 lignes
```

> Note de méthode : le décompte de lignes annoncé dans la commande était de
> 3 235 ; la mesure réelle du fichier est 3 699 lignes / 164 406 octets. C'est
> le chiffre mesuré qui est retenu ci-dessous.

## Ce que la fiche contient réellement

| Mesure | Valeur |
|---|---|
| **Actions distinctes** | **27** |
| Champs de formulaire lus (`GETPOST`) | **87** distincts, **322** appels |
| Droits distincts vérifiés | **9** (44 appels `hasRight`, 7 modules) |
| États du type de tiers | `client` 0/1/2/3 × `fournisseur` 0/1 = **8** combinaisons |
| Modules testés (`isModEnabled`) | 16 |
| Constantes `getDolGlobal*` | **44** distinctes, **112** appels |
| Fichiers uploadés | 1 (`$_FILES['photo']`) |
| Hook exposés | 6 |
| Templates inclus | 5 |

## Les 27 actions, classées

**Cycle de vie et pièce (11)**
`view`, `create`, `add`, `edit`, `update`, `delete`, `confirm_delete`,
`merge`, `confirm_merge`, `clone`, `confirm_clone`

**Identité et rattachement (4)**
`set_thirdpartytype`, `editthirdpartytype`, `set_thirdparty` (maison mère),
`editparentcompany`

**Commercial (1)**
`set_salesrepresentatives`

**Fiscalité et localisation (6)**
`set_localtax1`, `set_localtax2`, `set_incoterms`, `editincoterm`,
`editRE` (régime Espagne), `editIRPF` (régime Portugal)

**Logistique (2)**
`setwarehouse`, `editwarehouse`

**Champs libres (2)**
`update_extras`, `edit_extras`

**Diffusion (1)**
`presend`

## Familles demandées vs familles réellement présentes

| Famille | Actions dans `card.php` |
|---|---|
| Identité | 4 |
| Statuts / cycle de vie | 11 |
| Commercial | 1 |
| **Adresses** | **0** |
| **Contacts** | **0** |
| **Banque / RIB** | **0** |
| **Encours et risque** | **0** |
| **Documents liés** | **0** |
| Autres (fiscalité 6, logistique 2, extras 2, diffusion 1) | 11 |

Cinq des neuf familles sont **vides**, et ce n'est pas un oubli de lecture : le
fichier ne contient aucune occurrence de `encours`, `risque` ni `credit`. L'adresse est saisie dans `add`/`update` mais n'a **aucune action
dédiée** ; les contacts vivent dans `contact.php` ; le RIB n'est pas porté par
le tiers, seulement par un pointeur `fk_account` vers un compte bancaire ; le
nombre de documents liés est rendu par le tableau d'onglets
(`societe_prepare_head`), pas par une action.

## Les 87 champs, classés

| Famille de champs | Nb | Exemples |
|---|---|---|
| Identité | 22 | `name`, `name_alias`, `firstname`, `civility_id`, `typent_id`, `effectif_id`, `forme_juridique_code`, `entity` |
| Fiscalité | 10 | `assujtva_value`, `tva_intra`, `vat_reverse_charge`, `localtax1assuj_value`, `lt1`, `lt2` |
| Adresses | 7 | `address`, `zipcode`, `town`, `country_id`, `state_id` |
| Codes | 7 | `code_client`, `customer_code`, `code_fournisseur`, `supplier_code`, `prefix_comm`, `getcustomercode`, `getsuppliercode` |
| Technique | 7 | `action`, `confirm`, `cancel`, `backtopage`, `modelselected`, `deletephoto` |
| Identifiants professionnels | 6 | `idprof1` … `idprof6` |
| Nature du tiers | 5 | `client`, `customer`, `prospect`, `fournisseur`, `supplier` |
| Commercial | 5 | `commercial`, `commercial_id`, `cond_reglement_id`, `mode_reglement_id` |
| Coordonnées | 5 | `email`, `phone`, `phone_mobile`, `fax`, `url` |
| Catégories | 3 | `custcats`, `suppcats`, `contcats` |
| Contacts | 2 | `no_email`, `contact_no_email` |
| Web services | 2 | `webservices_url`, `webservices_key` |
| Banque | 1 | `fk_account` |
| Logistique | 1 | `fk_warehouse` |
| Champs libres | 1 | `attribute` |
| Non classés | 3 | `capital`, `status`, `lcoation_incoterms` |

**Cinq noms de champs différents pour deux colonnes** : `customer` (1) et
`prospect` (2) sont les cases à cocher de `add`/`update` ; `supplier` (1) est la
case fournisseur ; `client` et `fournisseur` sont les champs cachés relus pour
réafficher le formulaire d'édition ; `type` (`c` / `p` / `f`) est le paramètre
d'URL de `create`. Deux colonnes en base, six paramètres HTTP.

## L'encodage réel du type de tiers — les conditions exactes

C'est le point qui casse chez nous. Voici ce que le fichier fait, sans
interprétation.

### 1. Ce sont deux colonnes indépendantes

Ce que le bloc `add` **et** `update` écrit (l.402-407), sous forme de formule :

| Colonne | Valeur écrite | Source |
|---|---|---|
| `client` | `(customer > 0 ? 1 : 0) + (prospect > 0 ? 2 : 0)` | cases à cocher `customer`, `prospect` |
| `fournisseur` | `(supplier > 0 ? 1 : 0)` | case à cocher `supplier` |

`client` est donc une **somme** de deux booléens pondérés : `0` aucun,
`1` client, `2` prospect, `3` client **et** prospect. `fournisseur` est un
booléen séparé, jamais combiné à `client`.

### 2. Les cases à cocher portent déjà la valeur du code

Le formulaire HTML (l.1520, 1524, 1530 pour la création ; 2411, 2415, 2421 pour
l'édition) déclare `name="prospect" value="2"`, `name="customer" value="1"` et
`name="supplier" value="1"`. Mais le code ne lit **pas** la valeur postée : il
teste `> 0` puis applique sa propre pondération. Poster `customer=2` donnerait
quand même `1`. Le `value=` n'est qu'un marqueur « coché ».

### 3. Le `client` posté directement n'est pas écouté

La ligne qui aurait lu un `client` fourni par le formulaire existe et est
**commentée** dans le fichier (l.1084). Le seul `GETPOSTINT('client')` actif
(l.2161) est dans le bloc `edit`, pour **réafficher** le formulaire avec les
valeurs postées ; ce n'est pas une écriture. Conclusion mesurable : **aucun
chemin ne fait confiance à un `client` venu du formulaire**.

### 4. Le chemin `create` a un second encodage, par `type`

C'est le formulaire de création (l.1052-1077). Il ne lit pas les cases à cocher :
il lit un paramètre d'URL `type`.

| `type` | Condition annexe | `client` écrit |
|---|---|---|
| absent | `THIRDPARTY_CUSTOMERPROSPECT_BY_DEFAULT` vide | `0` |
| absent | `THIRDPARTY_CUSTOMERPROSPECT_BY_DEFAULT` posée | `3` |
| `c` | `THIRDPARTY_CUSTOMERTYPE_BY_DEFAULT` = 0 (cas par défaut) | **`3`** |
| `c` | `THIRDPARTY_CUSTOMERTYPE_BY_DEFAULT` = n | `n` |
| `p` | — | `2` |
| `f` | — | `0` |

Et pour `fournisseur` : `1` si `type=f`, ou si `type` est vide **et**
`THIRDPARTY_SUPPLIER_BY_DEFAULT` est posée — dans les deux cas seulement si
`supplier_order` ou `supplier_invoice` est activé.

Enfin, si `SOCIETE_DISABLE_PROSPECTSCUSTOMERS` est active, tout `client = 3`
calculé ci-dessus est ramené à `1` (l.1071-1073).

**C'est ici que naît le bug** : `type=c` écrit `3`, pas `1`. Un tiers créé comme
« client » est donc *client et prospect* par défaut, sauf si
`THIRDPARTY_CUSTOMERTYPE_BY_DEFAULT` vaut 1. Un code qui teste `client = 1` pour
détecter un client rate tous ces clients-là.

### 5. Les conditions réellement testées dans le fichier

| Condition | Lignes | Ce qu'elle décide |
|---|---|---|
| `$object->client == 1 \|\| $object->client == 3` | 665, 3571 | fiche commerciale après création ; conversion en membre |
| `$object->client == 3` | 1071 | forçage `3 → 1` si `SOCIETE_DISABLE_PROSPECTSCUSTOMERS` |
| `$object->client > 0` | 411, 417 | application des conditions de règlement client |
| `empty($object->client)` | 539, 694 | **efface `code_client`** |
| `$object->fournisseur == 1` | 667 | redirection fiche fournisseur |
| `$object->fournisseur > 0` | 426 | conditions de règlement fournisseur |
| `empty($object->fournisseur)` | 542, 697 | efface `code_fournisseur` |
| `switch ($selected)` `case 1 / 2 / 3` | 1489-1499 | recochage des cases à l'écran |
| `$object->prospect` | 3322 | onglet catégories client |

Aucune de ces conditions n'écrit `client IN (2,3)` : c'est bien un test
d'égalité sur des constantes. `$object->prospect` (l.3322) n'est **jamais
assigné** dans ce fichier — c'est une propriété dérivée, pas une colonne.

### 6. Trois écritures de `client` seulement

| Ligne | Écriture | Contexte |
|---|---|---|
| 406 | `= $customer + $prospect` | `add` et `update` |
| 903 | `= 0` | `confirm_clone` — le clone naît sans nature |
| 1054-1072 | `= 0 / 1 / 2 / 3` | `create`, via `type` |

## Les 9 droits vérifiés

```
societe/creer          (11 appels)      fournisseur/lire         (7)
societe/lire           (2)              supplier_order/lire      (7)
societe/supprimer      (2)              supplier_invoice/lire    (7)
categorie/lire         (4)              supplier_proposal/lire   (3)
                                        user/user                (1)
```

Plus quatre choses qui ne passent pas par `hasRight` :

- `restrictedArea($user, 'societe', ...)` (l.178) — c'est **lui**, pas
  `hasRight('societe','lire')`, qui garde l'entrée de la page ;
- `$user->rights->categorie->lire` (ancienne forme, encore présente) ;
- `dol_eval()` sur les permissions d'attributs libres (l.170-173) — le droit
  d'éditer un extrafield peut être une expression stockée en base ;
- `$permissiontodelete` (l.166) : `societe/supprimer` **ou** (`societe/creer`
  **et** `status == 0`) — une fiche désactivée est supprimable par un simple
  rédacteur.

## L'écart avec notre noyau

Ce que notre écran tiers sait faire, mesuré dans `crates/web/src/main.rs`,
`pages.rs`, `db/party.rs`, `db/divers.rs` :

| | Dolibarr | Noyau Rust |
|---|---|---|
| Actions | **27** | **5** (créer, enregistrer, filtrer, exporter CSV, consulter) |
| Champs de saisie | 87 | **8** (`nom`, `adresse`, `cp`, `ville`, `email`, `tel`, `siret`, `tva_intra`) |
| Droits distincts | 9 | 3 identifiants (`9001`, `241`, `242`) |
| Nature du tiers | 3 cases indépendantes × 2 colonnes | **aucune saisie** ; filtre seulement |
| Statut (activité cessée) | sélecteur + droit de suppression | `set_statut` existe, **0 appel** |
| Lignes de code de l'écran | 3 699 | **353** (`tiers` 55, `tiers_fiche` 99, `tiers_nouveau` 96, `tiers_creer` 20, `tiers_modifier` 62, `tiers_enregistrer` 21) |

Autrement dit : **l'écran tiers est à 18,5 % des actions et 9,2 % des champs**.
Ce n'est pas une estimation, c'est un rapport de comptage.

### Ce que le noyau fait juste sur le type

`db::divers::tiers_filtres` écrit `client IN (2, 3)` pour prospect et
`client IN (1, 3)` pour client, avec `fournisseur = 1` séparé. **C'est la bonne
sémantique** — celle que la fiche Dolibarr exprime par `== 1 || == 3` et le
`switch case 3`. Rien à corriger là.

### Ce que le noyau ne sait pas faire

1. **Créer un prospect ou un fournisseur.** `tiers_creer` construit
   `NewParty::client(nom)`, qui pose `client = CLIENT_CLIENT (1)` et
   `fournisseur = 0` en dur. Le formulaire n'a aucune case de nature. Un
   prospect créé chez nous devient un client dans la base Dolibarr.
2. **Changer la nature d'un tiers existant.** `db::party::update` ne touche pas
   `client` ni `fournisseur` (l'`UPDATE` ne les liste pas) : c'est prudent, mais
   du coup le passage prospect → client est impossible depuis l'écran.
3. **Fermer une fiche.** `db::party::set_statut` est écrit et jamais appelé ;
   aucune route, aucun bouton.
4. **Séparer lecture et écriture.** La garde de `main.rs` l.481-491 accepte
   n'importe lequel des droits exigés (`.any()`), et ce pour toutes les
   méthodes : un compte qui n'a que `241` (« lire les tiers ») peut ouvrir
   `POST /tiers/nouveau` et créer. Dolibarr sépare `creer` de `lire`.
5. **Le code client.** Nous générons `CU` + `AAAMM` + 5 chiffres
   (`next_code_client`), alors que Dolibarr passe par un modèle configurable
   (`SOCIETE_CODECLIENT_ADDON`, défaut `mod_codeclient_leopard`) et efface le
   code quand `client == 0`.

## Trois découvertes

1. **Un « client » Dolibarr est un prospect par défaut.**
   `type=c` écrit `client = 3` (l.1064). Toute requête qui demande les clients
   avec `client = 1` rate les clients créés par ce chemin. Notre `IN (1,3)` est
   le seul test correct, et il est déjà en place — mais il faut le savoir avant
   d'écrire la prochaine requête.

2. **Le `code_client` est effacé, pas laissé vide par défaut.**
   À la ligne 539 : si `client` est vide, `code_client` est remis à la chaîne
   vide avant l'insertion. Un fournisseur pur n'a donc pas de code client. À
   l'inverse, un prospect (`client = 2`) en a un : le code n'est pas un
   indicateur fiable de « c'est un client ».

3. **La fiche tiers ne porte pas le risque.** Zéro occurrence de `encours`,
   `risque` ou `credit` dans 3 699 lignes. L'encours n'est pas un champ du
   tiers : il est recalculé depuis les factures. Toute idée de « plafond
   d'encours sur la fiche » demanderait une colonne qui n'existe pas dans le
   modèle Dolibarr 24.0.1.

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
