# dolibarr-rs

> Pour contribuer : **[DEVELOPPEMENT.md](DEVELOPPEMENT.md)** — architecture, règles de code,
> recettes pour ajouter un écran ou un module, et les pièges qui coûtent une heure.
> Analyse du forum Dolibarr : C:\Users\Maxime\Documents\Default Project\DOLIBARR-FORUM-PROBLEMES.md.

Noyau Dolibarr réécrit en Rust, **sur le schéma de données officiel de Dolibarr**.
Écrans, calculs, PDF et API en Rust ; PHP ne reste que pour exécuter les modules Dolistore.

## État

| Crate | Rôle | Tests |
|---|---|---|
| `money` | arrondis et totaux de ligne compatibles `price.lib.php` | 8 |
| `schema` | parseur du schéma officiel, manifeste, préparation SQL | 10 |
| `auth` | mots de passe `llx_user` (bcrypt `$2y$`, md5 historique) | 7 |
| `db` | accès MySQL, vérification du schéma, tiers, produits, devis | 2 unités + 11 intégration |
| `pdf` | rendu PDF Typst des documents (A4, multi-page, totaux insécables) | 5 |
| `cli` | `schema …`, `party …`, `product …`, `proposal …`, `money line` | — |

## Tiers

```bash
export DOLIBARR_DSN="mysql://root:root@127.0.0.1:3307/dolibarr"

cargo run -q -p dolibarr-rs -- party add --nom "M&Mme Roure" \
  --zip 69410 --town "Saint-Cyr-au-Mont-d'Or"
# cree  rowid=17  code_client=CU2611-00003  M&Mme Roure

cargo run -q -p dolibarr-rs -- party list --limit 10
cargo run -q -p dolibarr-rs -- party show --id 17
```

Le code client suit le format de `mod_codeclient_leopard` (`CU` + `AAAMM` + `-` +
5 chiffres), attribué dans la transaction d'insertion, avec reprise en cas de
collision sur `uk_societe_code_client`.

Les tests d'intégration s'exécutent quand `DOLIBARR_DSN` est défini, et sont
ignorés proprement sinon (la CI sans base reste verte).

## Produits et services

```bash
cargo run -q -p dolibarr-rs -- product add --ref "BOR-030" \
  --label "Bordure granit gris 30 cm" --prix 125.50 --tva 20 --unite M
# cree  rowid=10  ref=BOR-030  Bordure granit gris 30 cm  prix=125.5  tva=20

cargo run -q -p dolibarr-rs -- product add --ref "MO-MAC" \
  --label "Main d oeuvre macon" --prix 48 --tva 10 --service

cargo run -q -p dolibarr-rs -- product list
```

Les prix sont lus avec `CAST(price AS CHAR)` puis convertis en `Decimal` : le
schéma les stocke en `double(24,8)`, on refuse de les faire transiter par un
flottant. `price_ttc` est calculé à l'écriture à partir de `tva_tx`.
`--unite` prend le `code` de `llx_c_units` (`M`, `KG`, `H`…).

## Devis et PDF

```bash
cargo run -q -p dolibarr-rs -- proposal add --soc 17 \
  --section "Dépose carrelage" \
  --ligne "32|28.50|10|Dépose carrelage existant et évacuation" \
  --ligne "32|42.00|10|Ragréage et préparation du support" \
  --section "Reprise étanchéité" \
  --ligne "1|1480.00|10|Reprise étanchéité sous carrelage"
# cree  rowid=62  ref=PR2611-0002  total_ht=3736  total_ttc=4109.6

cargo run -q -p dolibarr-rs -- proposal list
cargo run -q -p dolibarr-rs -- proposal show --id 62
cargo run -q -p dolibarr-rs -- proposal pdf --id 62 --out devis.pdf
# devis.pdf  1 pages  42215 octets
```

Une ligne se saisit `quantite|prix_unitaire_ht|taux_tva|libelle[|remise]`.

`--section` ouvre une section : une ligne de structure est écrite en base
(`product_type = 9`, `special_code = 104777`, montants à zéro), ce que le cœur
de Dolibarr reconnaît comme ligne spéciale (`core/lib/pdf.lib.php`). Ces lignes
**ne comptent pas** dans les totaux, et les sous-totaux de section sont calculés
au rendu, pas stockés : pas de risque de double compte.

Numérotation `PR` + `AAAMM` + `-` + 4 chiffres, sans remise à zéro mensuelle,
reprise sur collision, calculée dans la transaction (comme `mod_propale_marbre`).

Le PDF est composé par Typst depuis `crates/pdf/templates/document.typ` : le
montant de chaque cellule arrive **déjà formaté**, le crate `pdf` ne calcule rien.

## Vérifier

```powershell
.\tools\verifier.ps1            # formatage, clippy, 576 tests, schéma, chaîne métier, 12 écrans
.\tools\verifier.ps1 -Reseau    # + les tests des API de l'État (BAN, INSEE)
```

Sort en code 1 au premier échec. Utilisable en pré-commit ou en CI.

## Démarrage

```bash
# 1. schéma officiel (sparse checkout de la source amont)
cd _vendor/dolibarr-src
git sparse-checkout add htdocs/install/mysql htdocs/core/lib htdocs/install
cd ../..

# 2. base de test (MariaDB portable, port 3307)
C:/Users/Maxime/.mariadb/server/bin/mysqld.exe \
  --defaults-file=C:/Users/Maxime/.mariadb/my.ini --port=3307 --console

# 3. instructions SQL prêtes à charger
cargo run -q -p dolibarr-rs -- schema prepare \
  --src "_vendor/dolibarr-src/htdocs/install/mysql" --out "sql-ready"

# 4. chargement (ordre Dolibarr : tables, clés, dictionnaires)
mysql -h127.0.0.1 -P3307 -uroot -proot -e \
  "CREATE DATABASE dolibarr CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"
mysql -h127.0.0.1 -P3307 -uroot -proot dolibarr < sql-ready/1_tables.sql
mysql --force -h127.0.0.1 -P3307 -uroot -proot dolibarr < sql-ready/2_keys.sql
mysql --force -h127.0.0.1 -P3307 -uroot -proot dolibarr < sql-ready/3_data.sql

# 5. vérification
cargo run -q -p dolibarr-rs -- schema verify \
  --dsn mysql://root:root@127.0.0.1:3307/dolibarr --manifest schema-24.0.1.json
```

```
manifeste          24.0.1 (410 tables)
base dolibarr      410 tables, 5362 colonnes
tables absentes    0
colonnes absentes  0
types differents   0
colonnes en plus   0 (modules)
RESULTAT           compatible
```

## Schéma de référence

Dolibarr **24.0.1**, commit `b795838`.

```
749 fichiers SQL · 410 tables · 5 362 colonnes · 749 index · 235 clés étrangères
empreinte 7293039ac8f4426f3a4a102552ca681228c2e2ffaec7bac10c9e6cc2cae68b2f
```

Le manifeste est généré, jamais édité. Le test
`schema_officiel_24_0_1_est_integralement_parsable` échoue si l'amont change.

## Règles de code

1. Jamais de `SELECT *`, jamais d'`INSERT` sans liste de colonnes.
2. Aucun calcul monétaire en virgule flottante : `rust_decimal` uniquement.
3. Arrondi demi hors de zéro (comportement de `round()` en PHP).
4. Les tables et colonnes ajoutées par un module sont tolérées, jamais supprimées.
5. `llx_const` fait foi pour l'état d'activation des modules.

## Factures

```bash
cargo run -q -p dolibarr-rs -- invoice from-proposal --devis 62
# cree  rowid=73  ref=FA2611-0001  total_ht=3736  total_ttc=4109.6

cargo run -q -p dolibarr-rs -- invoice list
cargo run -q -p dolibarr-rs -- invoice show --id 73
cargo run -q -p dolibarr-rs -- invoice validate --id 73
cargo run -q -p dolibarr-rs -- invoice pdf --id 73 --out facture.pdf
```

La conversion reprend les lignes du devis, recalcule les totaux avec `money`
(elle ne fait pas confiance aux montants stockés) et écrit le lien dans
`llx_element_element` (`propal` → `facture`), pour que Dolibarr affiche la
facture depuis le devis.

Numérotation `FA` (avoir `AV`, acompte `AC`), même schéma que `mod_facture_terre`.
Elle est sérialisée par un **verrou consultatif MySQL** (`GET_LOCK`) : dix
créations simultanées donnent dix numéros distincts, sans toucher au schéma.

> Écart assumé : comme Dolibarr, le numéro est attribué **à la création**, pas à
> la validation. Un brouillon supprimé consomme donc un numéro. Pour une
> conformité stricte (séquence sans trou), il faudra déplacer l'attribution dans
> `validate()`.

## Société et conformité

Les documents sont remplis depuis la fiche société stockée dans `llx_const`,
**aux mêmes clés que Dolibarr** (`MAIN_INFO_SOCIETE_NOM`, `MAIN_INFO_SIRET`,
`MAIN_INFO_TVAINTRA`, `MAIN_INFO_CAPITAL`, `MAIN_INFO_RCS`…). Une fiche saisie
dans Dolibarr est donc reprise telle quelle, et inversement.

```bash
cargo run -q -p dolibarr-rs -- company set \
  --nom "Pierre Angulaire" --forme "SAS" --capital "10 000 EUR" \
  --rcs "RCS Lyon 123 456 789" --adresse "17 chemin du Grimpillon" \
  --cp 69410 --ville "Saint-Cyr-au-Mont-d'Or" \
  --siret 12345678900012 --tva-intra FR12345678900 \
  --email contact@maconnerie-architecture.fr --delai 30

cargo run -q -p dolibarr-rs -- company show
# conformite     complete
```

Le crate `legal` compose les mentions obligatoires (code de commerce L441-9,
CGI art. 242 nonies A, 293 B, 283-2 nonies) selon le type de document, le régime
de TVA et la nature du client :

| Situation | Mention produite |
|---|---|
| Facture | pénalités de retard, indemnité forfaitaire de 40 €, escompte |
| Devis | validité, prix HT, TVA au taux en vigueur |
| Avoir | référence de la facture d'origine, régularisation de TVA |
| Franchise en base | « TVA non applicable, art. 293 B du CGI » |
| Client pro hors France (UE) | « Autoliquidation, art. 283-2 nonies du CGI » |

Une **facture dont la société ou le client ne porte pas les champs légaux est
refusée** à la génération :

```
erreur: facture non conforme : il manque SIRET du vendeur
```

Un devis incomplet passe, avec un avertissement sur la sortie d'erreur.

- `2_keys.sql` et `3_data.sql` se chargent avec `--force` : les fichiers Dolibarr
  contiennent des doublons assumés (deux régions `7301`, index déjà déclaré en ligne).
  L'installeur PHP les ignore de la même façon.
- Les 90 fichiers `llx_accounting_account_*.sql` sont exclus, comme dans
  `install/step2.php` : les plans comptables se chargent à la sélection du pays.
- Le découpage des instructions est **aveugle aux guillemets**, comme
  `explode(';')` de Dolibarr : une apostrophe dans un commentaire (`can't`)
  ne doit pas casser le chargement.
- Le dépôt vit sur le NAS (`Z:`). Les artefacts de compilation sont redirigés
  vers le SSD local par `.cargo/config.toml`.


## Règlements et lettrage

```bash
cargo run -q -p dolibarr-rs -- invoice validate --id 73
cargo run -q -p dolibarr-rs -- payment add --facture 73 --montant 1000 \
  --mode virement --num "VIR-2026-11"
# reglement rowid=6  total TTC=4109.6  regle=1000  reste=3109.6

cargo run -q -p dolibarr-rs -- payment add --facture 73 --montant 3109.60 \
  --mode cheque --num "CHQ-1042"
# reglement rowid=7  regle=4109.6  reste=0  FACTURE SOLDEE

cargo run -q -p dolibarr-rs -- payment list --facture 73
cargo run -q -p dolibarr-rs -- invoice show --id 73
```

Modes : `virement`, `cheque`, `especes`, `carte`, `prelevement` (identifiants de
`llx_c_paiement`). Un règlement peut être partiel ; la facture passe à « payée »
(`paye = 1`, `fk_statut = 2`) quand la somme des affectations atteint le TTC.
Supprimer un règlement rouvre la facture.

**Une facture en brouillon ne peut pas être réglée** : c'est refusé avant toute
écriture, avec le message qui dit quoi faire.

Références de règlement : `PAY` + `AAAMM` + 4 chiffres. Aucune obligation légale
ne pèse sur cette numérotation, contrairement à celle des factures.

## Interface web

```bash
cargo run -q -p web
# dolibarr-web sur http://127.0.0.1:8090
```

Rendu côté serveur (maud), sans JavaScript. Les actions sont des formulaires
`POST` suivis d'une redirection, donc l'historique du navigateur fonctionne et
rien ne casse sans JS.

Navigation en **colonne de gauche** : la section courante est mise en évidence et
déploie ses actions (Nouveau tiers, Nouveau produit, Nouveau devis). Sous 900 px
la colonne passe en bandeau horizontal.

| Écran | Contenu |
|---|---|
| Tableau de bord | facturé, encaissé, reste à encaisser, montant en retard ; dernières factures et devis en attente |
| Tiers | recherche, liste, fiche avec devis et factures du client, **création** |
| Produits | catalogue, **création** avec TVA et unité prises dans les dictionnaires |
| Devis | liste, fiche avec sections et sous-totaux, **création** (client + 4 lignes), bouton « facturer », PDF |
| Factures | liste avec reste dû et statut, fiche avec règlements, validation, encaissement, PDF |
| **Paramètres** | société, dictionnaires, base de données |

## Les options de base

L'onglet **Paramètres** est découpé en trois sous-onglets :

| Sous-onglet | Contenu |
|---|---|
| Société | dénomination, forme juridique, capital, RCS, adresse, SIRET, TVA intracom, e-mail, délai de paiement, régime de TVA — tout est éditable et écrit dans `llx_const` aux clés de Dolibarr |
| Dictionnaires | taux de TVA, conditions de règlement, modes de paiement, unités, **puis les 64 tables `llx_c_*`** de la base avec leur nombre de lignes |
| Base de données | serveur et version, hôte et port, base, nombre de tables et de colonnes, schéma de référence, conformité de la fiche société, modules Dolibarr activés |

```
Serveur                  11.4.5-MariaDB
Hôte                     MRI:3307
Base                     dolibarr
Tables                   410
Colonnes                 5362
Schéma de référence      Dolibarr 24.0.1
Conformité de la société complète
```

## Dictionnaires

Le crate `db::dict` lit les tables `llx_c_*` de Dolibarr, sans les dupliquer :

| Dictionnaire | Table | Usage |
|---|---|---|
| Taux de TVA | `llx_c_tva` | sélecteur de taux dans les formulaires produit et devis |
| Unités | `llx_c_units` | choix de l'unité produit |
| Modes de paiement | `llx_c_paiement` | modes du formulaire d'encaissement |
| Conditions de règlement | `llx_c_payment_term` | délais |

Ajouter une unité ou un taux dans Dolibarr le rend immédiatement disponible ici :
rien n'est recopié dans le code.

La recherche du bandeau cherche dans les tiers, les devis et les factures.
Les statuts sont des pastilles : brouillon, validée, payée, **en retard**
(calculé sur l'échéance réelle, qui vaut par défaut date + 30 jours).

## Édition

| Écran | Ce qu'on peut faire |
|---|---|
| `/tiers/{id}/modifier` | corriger une fiche client (nom, adresse, SIRET, TVA…) |
| `/devis/{id}/modifier` | remplacer toutes les lignes d'un devis **en brouillon** : ajouter, retirer, changer les quantités, poser des sections |
| `/devis/{id}/valider` | passer un devis de brouillon à validé |

Deux règles protègent la comptabilité :

- **un devis validé ne se modifie plus** — il faut le laisser tel quel, la modification
  est refusée avec un message ;
- **un devis ne se facture qu'une fois** — la seconde tentative renvoie
  `ce devis a deja ete facture (facture n° 73)`. La vérification passe par
  `llx_element_element`, pas par une convention maison.

Dans le formulaire d'édition, une ligne sans libellé mais avec un nom de section
ouvre une section : c'est ainsi qu'on met « Reprise étanchéité » à part.

## Avoirs

```bash
cargo run -q -p dolibarr-rs -- invoice avoir --id 73
# avoir  rowid=278  ref=AV2611-0001  total_ttc=-4109.6
```

**On ne supprime jamais une facture validée** : on l'annule par un avoir. La
convention Dolibarr est respectée — `type = 2`, lignes à quantité **négative**,
référence `AV`, et `fk_facture_source` qui pointe sur la facture d'origine. Le
PDF sort avec la mention « Avoir sur la facture n° FA2611-0001 ».

Le bouton « Créer un avoir » apparaît sur toute facture validée. Un avoir ne
peut pas recevoir d'avoir, et une facture n'en reçoit qu'un.

## Journal et GED

Deux tables que Dolibarr utilise et que nous remplissons désormais aussi :

| Événement | Écrit dans `llx_actioncomm` |
|---|---|
| Devis validé | « Devis PR2611-0002 validé » |
| Devis facturé | « Devis PR2611-0002 facturé » |
| Facture validée | « Facture FA2611-0001 validée » |
| Règlement | « Règlement de 3 109,60 € sur la facture FA2611-0001 » |

Le type d'action est `AC_OTH_AUTO`, celui que Dolibarr réserve aux événements
automatiques. Chaque ligne porte `elementtype` et `fk_element`, donc elle
s'affiche dans l'onglet Agenda de la fiche, côté Dolibarr comme ici.

À la validation d'un devis ou d'une facture, **le PDF est déposé dans la GED** :
fichier écrit sous `documents/<modulepart>/<réf>/<réf>.pdf` (racine réglable par
`DOLIBARR_DOCUMENTS`) et indexé dans `llx_ecm_files` avec `src_object_type`.
Un PDF qui n'est pas archivé n'est pas une pièce.

## Facture directe et relances

| Écran | Contenu |
|---|---|
| `/factures/nouveau` | créer une facture sans devis (client + 6 lignes) |
| `/relances` | factures échues non réglées, avec jours de retard, reste dû et **mail prêt à partir** |

```
FA2611-0002   M&Mme Roure   2026-10-13   20 j   1 080,00 €   [Préparer le mail]
```

Le lien `mailto:` ouvre le client courrier avec l'objet et le corps déjà écrits :

> Bonjour,
>
> Sauf erreur de notre part, la facture FA2611-0002 d'un montant de 1 080,00 EUR TTC,
> échéance du 2026-10-13, demeure impayée (20 jours de retard).
>
> Je vous serais reconnaissant de bien vouloir procéder au règlement, ou de me faire
> savoir si un point bloque.

L'envoi reste une action humaine : pas de SMTP branché, donc rien ne part tout seul.
C'est volontaire — une relance qui part sur le mauvais client coûte plus cher qu'une
relance manuelle.

## Tâches planifiées

Pas de démon, pas de cron PHP : **un binaire et un planificateur système**.

```bash
dolibarr-rs tache relances            # liste les factures échues
dolibarr-rs tache relances --noter    # + un événement d'agenda par facture
dolibarr-rs tache sauvegarde          # dump SQL + documents
dolibarr-rs tache purge --jours 90    # supprime les vieilles sauvegardes
```

```
1 facture(s) en retard, 1440 au total
  FA2611-0002    M&Mme Roure                  20 j  reste 1440
```

Planification sous Windows (tous les jours à 8 h) :

```powershell
schtasks /create /tn "Dolibarr relances" /sc daily /st 08:00 ^
  /tr "dolibarr-rs.exe tache relances --noter"
```

Sous Linux, une unité `systemd` + `OnCalendar=*-*-* 08:00:00`.

## Notes d'exploitation

- `2_keys.sql` et `3_data.sql` se chargent avec `--force`.