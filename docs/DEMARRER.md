# Tester le noyau en 5 minutes

Aucun besoin des sources Dolibarr : le SQL prêt à importer est dans `sql-ready/`.

## Ce qu'il faut

- **Windows, Linux ou macOS** avec Rust (`rustup` : <https://rustup.rs>)
- **MariaDB ou MySQL** en service (n'importe quelle version récente)
- 5 minutes

## 1. Créer une base et importer le schéma

```powershell
.\installer.ps1 -Base dolibarr_testus -Utilisateur root -MotDePasse root
```

Sur Linux/macOS, l'équivalent :

```bash
mysql -u root -p -e "CREATE DATABASE dolibarr_testus"
mysql -u root -p dolibarr_testus < sql-ready/1_tables.sql
mysql -u root -p dolibarr_testus < sql-ready/2_keys.sql
mysql -u root -p dolibarr_testus < sql-ready/3_data.sql
```

La base contient alors les **410 tables** de Dolibarr 24.0.1 et les données de
référence (pays, devises, taux de TVA, moyens de paiement).

## 2. Lancer le noyau

```powershell
$env:DOLIBARR_DSN = "mysql://root:root@127.0.0.1:3306/dolibarr_testus"
$env:DOLIBARR_DOCUMENTS = "$PWD\documents"
cargo run -q -p web
```

Si le paquet n'est pas trouvé : `cargo run -q -p web`.

## 3. Ouvrir

```
http://127.0.0.1:8090/installation     ← l'état de l'installation
```

La page dit ce qui manque. Sur une base fraîche, il manque l'administrateur et
le nom de la société : c'est normal, c'est ce que l'assistant d'installation
fera à l'étape suivante (aujourd'hui il diagnostique, il n'écrit pas encore).

## 4. Essayer avec **sa** base, sans risque

C'est le cas le plus intéressant : pointer le noyau sur une copie d'une vraie
base Dolibarr.

```powershell
$env:DOLIBARR_LECTURE_SEULE = "1"     # toute écriture répond 403
$env:DOLIBARR_DSN = "mysql://user:motdepasse@127.0.0.1:3306/ma_base_dolibarr"
cargo run -q -p web
```

Les comptes existants fonctionnent : Dolibarr stocke les mots de passe en bcrypt
(`$2y$`), que le noyau vérifie. Les droits, les entités et les données sont les
mêmes — c'est la même base.

## 5. Ce qui marchera, et ce qui ne marchera pas

| | |
|---|---|
| Se connecter avec ses comptes | oui |
| Voir ses tiers, devis, factures, règlements | oui |
| Sortir un PDF de devis ou de facture | oui |
| Ouvrir **n'importe laquelle des 410 tables** | oui, en lecture |
| Ses modules PHP du Dolistore | **non** — aucun PHP n'est exécuté |
| Ses extrafields à l'écran | **non** — les données sont là, les écrans ne les montrent pas |
| Écrire dans la base | **non** tant que `DOLIBARR_LECTURE_SEULE` est posé |

## En cas de problème

Le noyau écrit ses erreurs dans un fichier de journal (`DOLIBARR_LOGS`) et la
page `/installation` donne le premier diagnostic : nombre de tables, présence
d'un administrateur, nom de la société, SMTP.

Si la base n'a pas 410 tables, c'est qu'elle n'est pas en 24.0.1 : les colonnes
manquantes produiront des erreurs d'écran, pas de corruption. Revenir en arrière,
c'est arrêter le binaire — rien n'a été écrit.
