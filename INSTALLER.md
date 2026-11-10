# Installer Cordeau de zéro

> Une base **vierge**, comme Dolibarr : 410 tables, les dictionnaires officiels,
> **aucune donnée**. Trois fichiers SQL dans `sql-ready/` — ce sont ceux que
> Dolibarr livre dans `htdocs/install/mysql/`.

## Ce qu'il faut

- **MariaDB** ou **MySQL** (testé sur MariaDB 11.4).
- **Rust** (`cargo`), ou le binaire `cordeau` déjà compilé.

## Installation en une commande

```powershell
.\tools\installer.ps1 `
    -Base "cordeau" `
    -Login "admin" `
    -Mdp "UnMotDePasseSolide!" `
    -Preset "btp"
```

Le script fait, dans l'ordre :

1. **crée la base** (refuse si elle existe déjà, sauf `-Ecraser`) ;
2. joue **`sql-ready/1_tables.sql`** (410 tables), **`2_keys.sql`** (index et clés),
   **`3_data.sql`** (dictionnaires : pays, devises, taux de TVA, unités…) ;
3. crée le **compte administrateur** (`cordeau user add --admin`) ;
4. applique le **preset de métier** (`cordeau preset appliquer --code …`) ;
5. écrit **`.env`** avec la chaîne de connexion, pour que `DEMARRER.ps1` n'ait
   plus rien à demander.

À la fin :

```powershell
.\DEMARRER.ps1          # interface sur http://127.0.0.1:8090
```

## À la main, si tu préfères

```bash
mysql -u root -p -e "CREATE DATABASE cordeau CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
mysql -u root -p cordeau < sql-ready/1_tables.sql
mysql -u root -p cordeau < sql-ready/2_keys.sql
mysql -u root -p cordeau < sql-ready/3_data.sql

cargo run -p cordeau -- user add \
    --dsn "mysql://root:motdepasse@127.0.0.1:3306/cordeau" \
    --login admin --mdp "UnMotDePasseSolide!" --admin

cargo run -p cordeau -- preset appliquer --code btp \
    --dsn "mysql://root:motdepasse@127.0.0.1:3306/cordeau"
```

## Les presets de métier

Un preset **allume des modules** ; il n'en éteint aucun et n'écrit **aucune
donnée métier**.

```bash
cordeau preset lister
```

| Code | Métier |
|---|---|
| `nue` | le minimum vital : tiers, produits, devis, factures |
| `btp` | chantiers, achats, stock, lots, marges, congés, bulletins, notes de frais |
| `architecte` | missions, contrats, projets, notes de frais |
| `logistique` | stock, expéditions, achats |
| `commerce` | caisse, tickets, banque |
| `bureau-etudes` | projets, contrats, temps passé |
| `association` | adhérents, dons, événements |

## Ce que l'installation ne fait pas

- **Aucune donnée de démonstration** : pas de client fictif, pas de produit, pas
  de tarif. Une base pré-remplie de faux est une base qu'on ne nettoie jamais.
- **Aucune constante d'entreprise** : ni RIB, ni SIRET, ni mention légale, ni
  logo. Ce sont des données à toi, et une mention fausse sur une facture ne se
  rattrape pas. Tu les saisis dans **Paramètres → Société**.
- **Aucun module externe** du Dolistore : ils ont besoin de PHP.

## Vérifier que l'installation est saine

```powershell
cordeau schema verify --dsn "mysql://root:motdepasse@127.0.0.1:3306/cordeau"
cordeau audit --dsn "…" --entity 1
```

Puis, dans l'interface : `/parametres` affiche l'état (schéma complet ou non,
administrateurs, modules), et `/presets` montre chaque socle module par module.

## Remettre à zéro

```powershell
.\tools\installer.ps1 -Base "cordeau" -Login "admin" -Mdp "…" -Preset "btp" -Ecraser
```

`-Ecraser` **supprime la base** et la refait. C'est le seul geste destructeur du
script, et il demande une confirmation tapée à la main.
