# Instance Dolibarr de référence

Une vraie instance Dolibarr 24.0.1 à côté du noyau, pour **comparer au lieu de
supposer**. Montée le 5 novembre 2026.

## Ce qu'elle est

| | |
|---|---|
| Source | `_vendor/dolibarr-ref` — tag 24.0.1 complet, 89 Mo, **1 529 pages PHP**, 71 modules |
| Base | `dolibarr_ref` sur le MariaDB portable (port 3307), 410 tables |
| Compte | `admin` / `Reference2026!` |
| Adresse | `http://127.0.0.1:8080` |
| Démarrage | `.\tools\demarrer-dolibarr-ref.ps1` |

Elle ne touche pas à la base de travail `dolibarr`. Deux bases, deux vies.

## Ce qui marche

- Le serveur démarre et **la page de connexion de Dolibarr s'affiche** (titre
  « Identifiant @ 24.0.1 »).
- Le schéma est celui du jeu prêt : les 410 tables du noyau, à l'octet, ont
  suffi à Dolibarr pour démarrer. **C'est une vérification en soi** : la
  promesse « même base » tient.
- Les constantes attendues (`MAIN_VERSION`, `MAIN_DB_VERSION`) et
  `documents/install.lock` ont été posées à la main, comme le ferait l'assistant.

## Ce qui a bloqué, et pourquoi

**La connexion en ligne de commande.** Le POST sur `/index.php` porte un jeton
CSRF lié à la session ; en `curl`, la session n'est pas reprise et Dolibarr
répond « token invalide ». Ce n'est pas un défaut de l'instance : c'est `curl`
qui ne tient pas de session comme un navigateur.

**Deux minutes dans un navigateur suffisent** : ouvrir `http://127.0.0.1:8080`,
se connecter, et l'instance est utilisable.

## Les trois pièges rencontrés, à ne pas rechercher

1. **« Driver mysqli for PHP not available ».** Le PHP installé n'a **aucun
   `php.ini`** : `php_mysqli.dll` et `php_pdo_mysql.dll` sont dans `ext\` mais ne
   sont pas chargés. Réglé par un `php.ini` propre, chargé avec `-c` — sans
   toucher au PHP du système.
2. **BOM UTF-8.** `Set-Content -Encoding UTF8` en PowerShell 5.1 écrit un BOM.
   Dans un `.php`, il fait démarrer la sortie avant les en-têtes : PHP refuse
   alors la session et Dolibarr tombe en « erreur technique ». 17 fichiers du
   dépôt portaient ce BOM (dont `verifier.ps1` et plusieurs `Cargo.toml`) ; ils
   sont nettoyés.
3. **`MAIN_NOT_INSTALLED` dans le jeu de données.** La graine contient cette
   constante, et Dolibarr renvoie vers l'assistant tant qu'elle existe. Elle est
   supprimée de `dolibarr_ref`.

## Ce que ça change dans les chiffres

Ma comparaison annonçait **789 écrans**. Le comptage sur la source complète dit
**1 529 pages PHP** réparties sur 71 modules. Mon chiffre était sous-estimé de
moitié — il comptait probablement les seules pages de modules, sans `admin/`
(192 pages à lui seul).

```
admin            192      compta           134      product          111
accountancy       56      hrm               50      comm              49
fourn             48      societe           41      projet            40
adherents         39      modulebuilder     38      webportal         34
user              32      recruitment       31      ai                29
asset             29      takepos           26      expedition        25
```

Le noyau Rust couvre **165 routes**. La route est longue, et elle est
maintenant mesurable.

## Comment s'en servir

Comparer un comportement, c'est répondre à une question précise :

- « Dolibarr affiche-t-il le total HT avant ou après remise ? » → on ouvre la
  fiche, on regarde.
- « Que fait-il d'une facture à 0 % sans motif ? » → on essaie.
- « Quel libellé porte le statut 4 d'un ticket ? » → on lit l'écran.

Le source reste aussi consultable : c'est le même qui sert depuis le début pour
les cas ambigus (`calcul_price_total`, `Categorie::$MAP_ID`, `price.lib.php`).

## Ce qu'elle n'est pas

Ce n'est pas un environnement de production, ni une base de recette partagée.
Elle est jetable : `DROP DATABASE dolibarr_ref` et un redémarrage du script la
reconstituent.
