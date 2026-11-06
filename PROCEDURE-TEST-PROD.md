# Tester Cordeau sur une base Dolibarr en production

> Le principe est celui du multi-company : **on ne demande pas au code de ne pas
> déborder, on l'empêche de pouvoir**. L'audit ouvre une transaction
> `START TRANSACTION READ ONLY` — MariaDB refuse alors toute écriture, même si
> le code se trompe.

## Ce qu'il ne faut pas faire

Brancher le noyau directement sur la base de production, même « juste pour
regarder ». Trois raisons :

1. **une erreur de configuration** (mauvais `entity`, mauvais schéma) se paie en
   données, pas en minutes ;
2. **les tâches planifiées** écrivent dans `llx_cronjob` au démarrage ;
3. **la numérotation** est consommée dès qu'un document est créé — un numéro
   manquant dans une suite est un contrôle fiscal qui pose des questions.

## La procédure

### 1. Prendre une copie

Sur le serveur o2switch, par SSH ou par l'interface :

```bash
mysqldump --single-transaction --routines --triggers \
  -u <user> -p <base> > dolibarr-prod-$(date +%F).sql
```

`--single-transaction` évite de verrouiller les tables pendant l'export : le
site continue de servir.

### 2. Restaurer la copie en local

```powershell
$mysql = "C:\Users\Maxime\.mariadb\server\bin\mysql.exe"
& $mysql -h 127.0.0.1 -P 3307 -u root -proot -e "CREATE DATABASE dolibarr_copie"
& $mysql -h 127.0.0.1 -P 3307 -u root -proot -D dolibarr_copie < dolibarr-prod-2026-11-06.sql
```

Le nom de base **diffère** de celui de production : c'est la première des
protections, et la plus simple.

### 3. Lancer l'audit

```powershell
cargo run -q -p cordeau -- audit --dsn "mysql://root:root@127.0.0.1:3307/dolibarr_copie" --entity 7
```

Ce que la commande affiche :

```
Base   : dolibarr_copie
Schema : 410 tables sur 410
Comptes: 24 utilisateur(s) actif(s)

Societes :
     1  Pierre Angulaire
     7  Pierre Angulaire (entite 7)

Objets :
  Tiers                         312
  Factures client               148
  ...

Dernieres factures de l'entite 7 :
  Reference      Client                   Date                   HT          TTC
  F-2611-0015    VIDAL                    2026-11-04       450.00       495.00  validee
  ...
  Comparez ces montants a l'ecran Dolibarr : ils doivent etre identiques.
```

**La dernière ligne est le test.** Ouvrir la même facture dans Dolibarr et
comparer les montants : s'ils diffèrent, c'est la lecture qui est fausse, et on
le sait avant d'avoir touché à quoi que ce soit.

### 4. Ce que l'audit refuse de faire

| Cas | Comportement |
|---|---|
| Une table du noyau manque | **erreur**, pas un zéro |
| La base n'a pas 410 tables | le dit et sort en code d'échec |
| On lui demande d'écrire | la base refuse (transaction en lecture seule) |

Un audit qui afficherait « 0 » sur une table absente ferait croire à une base
vide plutôt qu'à une base cassée. C'est le seul comportement inacceptable ici.

## Ce que l'audit ne vérifie pas encore

- **Les PDF** : l'audit lit les données, il ne régénère pas de document. La
  comparaison de PDF se fait en ouvrant `/factures/:id/pdf` sur la copie.
- **Les écritures comptables** : l'audit compte les lignes de
  `llx_accounting_bookkeeping`, il ne vérifie pas l'équilibre pièce par pièce.
  C'est le rôle de `verifier.ps1` sur la base de test.
- **Les modules tiers** : un module non porté n'est pas lu, et ce n'est pas une
  anomalie — l'audit ne compte que les tables du cœur.

## Passer de la lecture à l'écriture

Quand la lecture est validée, l'écriture se teste **sur la copie**, jamais sur
la production :

```powershell
$env:DOLIBARR_DSN = "mysql://root:root@127.0.0.1:3307/dolibarr_copie"
cargo run -q -p web
```

Puis : créer un devis, le valider, le convertir en facture, vérifier que
Dolibarr PHP lit ces documents. Si quelque chose casse, on jette la copie et on
recommence — la production n'a rien vu.
