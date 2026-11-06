# Essai sur la base de production — 6 novembre 2026

> But : brancher Cordeau sur les vraies données de Pierre Angulaire, et lui
> faire dire les mêmes choses que Dolibarr — sans toucher à la production.

## Ce qui a été fait, dans l'ordre

| Étape | Moyen | Résultat |
|---|---|---|
| Lire la configuration | FTP `ftp.sc1buso0092.universe.wf`, `htdocs/conf/conf.php` | base `sc1buso0092_rbimcomvutbdd165`, latin1, 24.0.1 + multicompany |
| Vérifier que la base répond | sonde PHP qui charge `master.inc.php` | connexion établie, MariaDB 11.4.13, **485 tables, 106 Mo** |
| Extraire une copie | `mariadb-dump --single-transaction --skip-lock-tables`, identifiants dans un fichier temporaire supprimé aussitôt | 3,7 Mo gzip, 485 `CREATE TABLE`, empreinte `9e40e9fc…` |
| Rapatrier | FTP, vérification de l'empreinte avant tout nettoyage | octets identiques |
| Restaurer en local | base **`dolibarr_prod`**, `latin1_swedish_ci` | 485 tables, 236 factures, 932 tiers, 229 devis |
| Auditer | `cordeau audit --entity 7` | 3 sociétés, montants conformes |
| Parcourir les écrans | web sur `127.0.0.1:8095`, compte d'essai **créé dans la copie** | 20 pages sur 20 |
| Comparer le PDF | PDF de production rapatrié par FTP, comparaison texte | tous les montants identiques |

### La garantie, et ce qu'elle vaut

L'audit ouvre `START TRANSACTION READ ONLY` : MariaDB refuse alors toute
écriture, y compris si le code se trompait. Un test le vérifie en essayant un
`INSERT` (refusé, rien ne reste).

Ensuite, la copie a été **modifiée volontairement** — c'est son rôle : un compte
de test y a été créé pour entrer dans l'interface, et les noms de sociétés y ont
été corrigés. **La production n'a reçu aucune écriture** : seuls des `SELECT`,
un dump, et la suppression des fichiers que j'y avais déposés (vérifiée par
relecture du listing FTP).

### Ce que Cordeau écrit quand on le démarre

À savoir avant de le brancher sur une base qui compte : l'interface crée ses
**deux tables de module** au démarrage, `llx_marge_affaire` et
`llx_marge_ligne` (`CREATE TABLE IF NOT EXISTS`, donc sans effet les fois
suivantes). C'est prévu par `REGLE-SCHEMA.md` — un module a le droit à ses
tables sous `llx_<module>_` — mais c'est une écriture, et il faut le dire.
Vérifié sur la copie : 485 tables dans le dump, **487** après un passage de
l'interface. En revanche `llx_cronjob` n'a **pas** été touché (31 lignes, toutes
à Dolibarr).


## Ce que la copie raconte

```
Sociétés :  1  SAS 2AR Architecte     5  2AR Conseils     7  PIERRE ANGULAIRE
Objets   :  932 tiers · 236 factures · 229 devis · 33 commandes
            39 factures fournisseur · 721 produits · 112 chantiers
            240 règlements · 0 écriture comptable · 19 contrats
```

Deux choses à savoir :

- **Il y a bien trois sociétés**, et celle du milieu porte le numéro **5**, pas
  2. L'entité 2 de la base de test locale n'existe pas en production.
- **`llx_accounting_bookkeeping` est vide** : Dolibarr n'a jamais écrit
  d'écriture comptable sur cette instance. Le grand livre de Cordeau le dit et
  reconstitue une proposition depuis les documents — ce n'est pas une
  comptabilité tenue.

## Cinq défauts trouvés, et corrigés

Tous les cinq étaient **invisibles sur la base de test** : elle a été remplie
par Cordeau, qui écrit `NULL` ; Dolibarr écrit la **chaîne vide**.

### 1. Toute facture de particulier était refusée

**Symptôme.** `GET /factures/714/pdf` → 500, « facture non conforme : il manque
SIRET du client, numéro de TVA intracommunautaire du client ».

**Cause.** `est_professionnel` valait vrai dès que la colonne `siret` était
`Some`, **y compris `Some("")`**. En production, **912 tiers sur 932** portent
`siret = ''`. M. Durand, particulier, passait donc pour un professionnel, et sa
facture exigeait des mentions qu'il ne peut pas avoir.

**Correction.** `db::party` lit ces colonnes avec `opt_txt` (chaîne vide →
absence) ; `app::client_legal` refuse aussi une valeur blanche. Et la règle
change : les identifiants du client ne **bloquent plus** la facture, ils sont
signalés sur la fiche (`legal::mentions_client_manquantes`). Seule l'identité du
vendeur bloque encore — c'est notre propre fiche.

**Preuve.** PDF de `F-2611-0015` produit : 780,00 / 78,00 / 858,00, trois
lignes, unité `FF` — identiques au PDF de production au centime près.

### 2. La forme juridique s'imprimait « 57 »

**Symptôme.** En-tête : « PIERRE ANGULAIRE — **57** au capital de 5100 ».

**Cause.** Dolibarr ne stocke pas « SAS » dans
`MAIN_INFO_SOCIETE_FORME_JURIDIQUE` : il stocke le **code** du dictionnaire
(`57`), que son administration traduit. Cordeau imprimait le code.

**Correction.** `db::company` résout le code dans `llx_c_forme_juridique`
(filtré par pays, puis sans filtre), sans jamais réécrire la constante.
Le capital est mis en forme : `5100` → `5 100 €`.

### 3. L'assurance décennale manquait sur les documents

**Symptôme.** Le PDF de production porte « Assurance Responsabilité Civile et
Décennale : Contrat N° 436456220001 … GROUPAMA ». Celui de Cordeau, rien.

**Cause.** Cordeau avait inventé ses propres constantes
(`DOLIBARR_RS_ASSURANCE_*`), vides sur une base Dolibarr. La donnée réelle est
dans **`MAIN_INFO_PROFID5`**.

**Correction.** `company` lit `MAIN_INFO_PROFID5` ; la mention est imprimée en
texte libre, prioritaire sur la reconstruction. C'est une mention légale
obligatoire pour un artisan (art. L132-1 du code de l'artisanat).

### 4. Les conditions de règlement disparaissaient des factures

**Symptôme.** Aucune ligne « Conditions de règlement » sur le PDF.

**Deux causes, l'une sur l'autre.**

1. Dolibarr sépare `libelle` (devis) et `libelle_facture`. La condition 83 dit
   « Paiement intégral à la signature » pour un devis, « Le règlement est dû à
   réception de la facture, par virement » pour la facture. Cordeau imprimait le
   premier — il annonçait au client une échéance qui n'était pas la sienne.
2. Cette condition appartient à l'**entité 1** et les factures de l'entité 7 la
   référencent. Le filtre par entité la faisait disparaître. Un
   `fk_cond_reglement` est une **référence** : on la suit, on ne la filtre pas.

**Correction.** `Condition::libelle_facture_fr()` ; `reglement::condition()`
retombe sur une lecture par `rowid` quand l'entité ne la voit pas.

### 5. Un document inexistant répondait 500

**Symptôme.** `/devis/1` → HTTP 500 « Devis introuvable ».

**Correction.** `Erreur` répond 404 quand le message dit « introuvable ». Une
adresse qui ne mène nulle part n'est pas une panne du serveur.

## Ce qui reste ouvert

| Sujet | Constat | Pourquoi ce n'est pas corrigé ici |
|---|---|---|
| SIREN, NAF-APE, « Type d'opérations » | présents sur le PDF Dolibarr, absents du nôtre | aucune obligation légale ; à ajouter quand la mise en page sera reprise |
| Écritures comptables | 0 ligne en production | rien à lire ; le grand livre reconstitue et le dit à l'écran |
| Six chantiers de l'entité 7 | `fk_soc = 1`, un tiers de l'entité 1 (« Forster Systèmes de Profils SARL ») | **donnée de production à corriger dans Dolibarr** : M & Mme Mesnier, SCI Boca, etc. pointent tous vers le même tiers |
| Factures de situation | `situation_cycle_ref` présent, `type = 5` | l'en-tête porte le delta et les lignes le cumul — Cordeau lit l'en-tête, donc juste ; le cycle complet reste à faire |
| Modules tiers (InfraS, Subtotal, multicompany) | 75 tables hors cœur | assumé : non portés, non lus |
| Documents (GED) | la copie est faite **sans les fichiers** | le PDF est régénéré depuis les données, pas relu du disque |

## Ce qu'il faut retenir

La base de test ne prouvait rien sur un point : elle ne contenait que des
données écrites par Cordeau. Une heure sur une copie de production a trouvé
**cinq défauts**, dont un qui empêchait d'éditer la moindre facture de
particulier — c'est-à-dire l'essentiel de l'activité.

La règle qui en sort : **une chaîne vide n'est pas une valeur.** Dolibarr ne
stocke pas `NULL` dans ses colonnes texte. Toute lecture qui confond les deux se
trompe, et se trompe silencieusement.
