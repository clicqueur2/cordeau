# Publication open source — état des lieux

Constat au **5 novembre 2026**, commit `d5a9a4e`, branche `master`.
Établi par lecture du dépôt uniquement. **Aucune commande `cargo build` ni
`cargo test` n'a été lancée** (un autre travail était en cours sur le même
répertoire de compilation) : tout ce qui concerne les tests est donc rapporté
comme *déclaré par la doc*, jamais comme *vérifié*.

État git : `238` fichiers suivis, **un seul remote : aucun** (`git remote -v`
vide), **aucun tag**, une seule branche.

---

## 1. Ce qui est déjà fait

| Élément | État | Preuve |
|---|---|---|
| Licence | **GPL-3.0-or-later**, texte intégral de la GPLv3 (35 149 o, en-tête « Version 3, 29 June 2007 ») | `LICENCE.txt` |
| Licence dans les manifestes | `license = "GPL-3.0-or-later"` au niveau workspace, héritée par les crates (`license.workspace = true` vérifié sur `cli`, `web`, `mcp`) | `Cargo.toml:8`, `crates/*/Cargo.toml` |
| Attribution de l'œuvre dérivée | métadonnées `oeuvre-derivee` / `attribution` | `Cargo.toml:19-22` |
| Notice légale | œuvre dérivée assumée, ce qui est repris de Dolibarr, ce qui ne l'est pas, règle sur les greffons, marques | `MENTIONS-LEGALES.md` (2 776 o) |
| README | 17 712 o : état des crates, 19 sections (tiers, produits, devis/PDF, factures, avoirs, règlements, écrans, dictionnaires, GED, relances, tâches planifiées, exploitation) | `README.md` |
| Guide de démarrage | 2 965 o, 5 étapes, Windows + Linux/macOS, tableau « ce qui marche / ce qui ne marche pas » | `DEMARRER.md` |
| Guide de développement | 14 770 o : architecture, règles de code, recettes, pièges | `DEVELOPPEMENT.md` |
| SQL prêt à importer | `sql-ready/1_tables.sql` (232 907 o), `2_keys.sql` (101 681 o), `3_data.sql` (613 312 o) — versionnés, donc **un clone suffit pour démarrer**, sans les sources Dolibarr | `DEMARRER.md:3`, `installer.ps1:41-46` |
| Installeur de base | `installer.ps1` : refuse d'écraser une base existante, cherche le client `mysql` (PATH + 3 chemins Windows), importe dans l'ordre, vérifie le nombre de tables, imprime le DSN | `installer.ps1` |
| Script de vérification | `tools/verifier.ps1` : formatage, clippy `-D warnings`, tests, conformité du schéma, chaîne devis→facture→règlement→PDF, 30 écrans, contrôle « sans session on tombe sur la connexion » | `tools/verifier.ps1:39-136` |
| Pré-commit | `tools/pre-commit.ps1` | présent |
| Référence du schéma | Dolibarr 24.0.1, commit amont `b795838`, 410 tables, 5 362 colonnes, empreinte SHA-256 publiée, manifeste `schema-24.0.1.json` | `README.md:137-147` |
| Textes de lancement | message forum prêt à copier (4 787 o) et post LinkedIn (6 687 o) | `ANNONCE-FORUM.md`, `PUBLICATION/linkedin-dolibarr-rust.md` |
| Documentation d'analyse | écart avec Dolibarr, FEC, comparaisons, conformité ERP, plateformes agréées, modules gratuits, feuille de route, règle de schéma | 9 fichiers `.md` à la racine |

Le fond est là : licence propre, attribution correcte, README dense, SQL
embarqué, outillage de vérification. Ce n'est pas un dépôt nu.

---

## 2. Ce qui manque (bloquant avant un push public)

| Élément | État réel | Preuve |
|---|---|---|
| **Nom du produit** | **non tranché**. `NOM.md` propose 8 noms, recommande « Cordeau » puis « Giron », et laisse 4 cases non cochées : domaine `.fr`/`.com`, recherche INPI classe 9/42, comptes GitHub, registre de paquets. En parallèle, tout le dépôt utilise `cordeau` — le préfixe « Doli » que `NOM.md:8-22` qualifie lui-même de « plus mauvais des choix possibles » (risque de confusion avec la famille Dolibarr). | `NOM.md:61-72` ; `Cargo.toml`, `crates/cli/Cargo.toml:2` (`name = "cordeau"`) |
| **Dépôt distant** | aucun. Pas d'`origin`, aucun tag, aucune release. | `git remote -v`, `git tag` (vides) |
| **CI** | **absente** : pas de dossier `.github` (vérifié : `Test-Path .github` → `False`), aucun workflow. Et `tools/verifier.ps1` ne peut pas servir de CI tel quel (§4, point 4). | disque |
| **Captures d'écran** | **aucune** : `git ls-files` ne contient aucune image (`.png`, `.jpg`, `.jpeg`, `.svg`, `.webp`, `.gif` → 0 résultat). Un projet avec 30 écrans et zéro capture ne se fait pas cliquer. | `git ls-files` |
| **Fichiers communautaires** | absents : `CONTRIBUTING.md`, `CHANGELOG.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `.gitattributes`, `rust-toolchain.toml`. La version de Rust requise n'est déclarée nulle part (`edition = "2021"`, MSRV inconnue). | racine du dépôt |
| **Version / release** | `0.1.0` partout (`Cargo.toml:6`), aucun tag, aucun binaire publié. | `git tag` |
| **Chiffres du README** | `README.md:93` annonce « 107 tests ». Le dépôt contient **569 attributs `#[test]` / `#[tokio::test]`** dans `crates/` (comptage des attributs, pas une exécution) et 40 fichiers de tests d'intégration. L'un des deux chiffres est périmé. | `README.md:93` vs comptage |
| **Données personnelles** | IBAN réel, nom et adresse de clients réels, e-mail et téléphone réels dans le code de test et le README. Détail et localisation : `SECRETS-AUDIT.md` §A. | `SECRETS-AUDIT.md` |
| **Artefacts versionnés** | `greffons/greffon_exemple.dll` (binaire) et `server.err` sont suivis par git. | `SECRETS-AUDIT.md` §D.1 |
| **Licence des polices** | `crates/pdf/assets/fonts/*.ttf` (DejaVu) sans fichier de licence joint. | `SECRETS-AUDIT.md` §D.3 |

---

## 3. Contradictions de la documentation (à corriger par le mainteneur — je ne
modifie aucun fichier existant)

1. **L'assistant d'installation écrit bel et bien.**
   `DEMARRER.md:45-47` affirme que sur une base fraîche « il manque
   l'administrateur […] c'est ce que l'assistant d'installation fera à l'étape
   suivante (aujourd'hui il diagnostique, il n'écrit pas encore) ».
   Or `crates/web/src/pages.rs:3009` contient un `form method="post"
   action="/installation"` (login, mot de passe ≥ 8 caractères, e-mail,
   société) et le gestionnaire `installation_creer` crée réellement
   l'administrateur (`db::user::creer`) puis sème les modules
   (`db::module::semer_modules_portes`), avec refus si un admin existe déjà.
   La doc est en retard sur le code — dans le bon sens, mais elle décourage un
   lecteur qui pouvait démarrer.
2. **La page se contredit elle-même** : son sous-titre dit « Rien n'est modifié
   par cette page : elle constate, elle ne répare pas », et la même page porte
   le formulaire de création d'administrateur.
3. **Commande de lancement fausse, deux endroits.**
   `DEMARRER.md:34` et `installer.ps1:89` donnent
   `cargo run -p cordeau --bin cordeau-web`. Le binaire `cordeau-web`
   appartient au paquet **`web`** (`crates/web/Cargo.toml`, `[[bin]] name =
   "cordeau-web"`), pas au paquet `cordeau` (`crates/cli/Cargo.toml`,
   `[[bin]] name = "cordeau"`). La forme utilisée ailleurs et cohérente avec
   les manifestes est `cargo run -q -p web` (`README.md:265`,
   `tools/verifier.ps1:116`). Le repli proposé `DEMARRER.md:37`
   (`cargo run --bin cordeau-web`) n'a pas été testé — interdit ici.
4. **Import Linux sans `--force`.** `DEMARRER.md:21-23` importe `2_keys.sql` et
   `3_data.sql` sans `--force`, alors que `README.md:118-120` et
   `installer.ps1:64-74` expliquent que ces deux fichiers contiennent des
   doublons assumés (deux régions `7301`, index déjà déclaré en ligne) et
   exigent `--force`. Copié tel quel, l'import s'arrête au premier doublon.
5. **Deux bases et deux ports selon le document** : `installer.ps1` et
   `DEMARRER.md` visent `dolibarr_testus` sur **3306** ; `README.md`,
   `DEVELOPPEMENT.md` et `tools/verifier.ps1` visent `dolibarr` sur **3307**
   (MariaDB portable de l'auteur). Chaque document est cohérent seul, mais un
   lecteur qui enchaîne installeur puis vérificateur pointe sur une base qui
   n'existe pas.
6. **`README.md:5`** renvoie à un fichier **hors dépôt**
   (`C:\Users\Maxime\Documents\Default Project\DOLIBARR-FORUM-PROBLEMES.md`) :
   lien mort pour tout lecteur.
7. **`README.md:102-105`** commence par `cd _vendor/dolibarr-src` sans jamais
   indiquer d'où vient ce dossier ni comment le cloner (quel dépôt, quelle
   branche, quel commit). L'étape est pourtant présentée comme l'étape 1 du
   démarrage. `DEMARRER.md` s'en passe très bien (le SQL est versionné), mais
   les deux documents ne se parlent pas.
8. **`tools/verifier.ps1:50`** — ligne `$sortie# Un seul fil : les tests…` :
   le commentaire est collé à la variable, la ligne ne fait rien (le fichier
   parse, vérifié avec `[Parser]::ParseFile`). Reste de fusion à nettoyer.

---

## 4. Test d'installation à froid : quelqu'un qui clone peut-il démarrer ?

Réponse honnête : **presque, et pas avec la commande écrite.**

### Scénario A — suivre `DEMARRER.md` seul

| Étape | Verdict |
|---|---|
| Prérequis : Rust (`rustup`) + MariaDB/MySQL | OK, documenté (`DEMARRER.md:7-9`). Version de Rust non précisée : aucune `rust-toolchain.toml`. |
| 1. Créer et importer la base | **OK sous Windows** : `installer.ps1` trouve le client `mysql` dans le PATH ou dans 3 chemins Windows connus, refuse d'écraser, importe les 3 fichiers avec `--force`. **Casse sous Linux/macOS** : la variante manuelle (`DEMARRER.md:19-24`) n'a pas `--force` (point 3.4). |
| 2. Poser `DOLIBARR_DSN` et `DOLIBARR_DOCUMENTS`, puis lancer | **ÉCHEC** : la commande `cargo run -p cordeau --bin cordeau-web` ne peut pas aboutir, le binaire appartient au paquet `web` (point 3.3). Un lecteur qui applique le repli de la ligne 37 est dans l'inconnu. |
| 3. Ouvrir `/installation` | **OK, et mieux que documenté** : la page liste schéma / administrateurs / utilisateurs / modules / SMTP et propose le formulaire de création de l'administrateur (login `admin` pré-rempli, mot de passe ≥ 8). Après validation, redirection vers `/connexion?ok=installe`. |
| 4. Se connecter | OK une fois l'admin créé : `db::user::authentifier` vérifie le bcrypt `$2y$` de Dolibarr. |
| 5. Utiliser l'application | OK sur une base fraîche (dictionnaires importés par `3_data.sql`). |

**Trou principal** : une seule ligne fausse (`DEMARRER.md:34`) sépare un lecteur
de l'écran de connexion. C'est le défaut le plus rentable à corriger.

### Scénario B — suivre `README.md` §« Démarrage »

**Bloqué dès l'étape 1** : `cd _vendor/dolibarr-src` suppose un dossier absent
d'un clone (il est dans `.gitignore`), et aucune commande ne le recrée (point
3.7). Les étapes suivantes utilisent en plus un `mysqld.exe` au chemin
`C:/Users/Maxime/.mariadb/...` (point 3.5). Ce paragraphe décrit l'atelier de
l'auteur, pas une installation reproductible. Il devrait renvoyer à
`DEMARRER.md` et ne garder que la régénération du SQL.

### Scénario C — lancer `tools/verifier.ps1` après avoir suivi `DEMARRER.md`

Trois blocages indépendants :

1. **`tools/verifier.ps1:21`** force
   `$env:CARGO_TARGET_DIR = "C:/Users/Maxime/.cargo-target/cordeau"` — un
   chemin qui n'existe pas chez un autre utilisateur, et que cargo tentera de
   créer sous `C:\Users\Maxime` (échec de droits probable). Idem
   `tools/pre-commit.ps1:22`.
2. **Base par défaut différente** : `-Base` vaut
   `mysql://root:root@127.0.0.1:3307/dolibarr` (`tools/verifier.ps1:10`) alors
   que `installer.ps1` crée `dolibarr_testus` sur 3306.
3. **Windows uniquement** : PowerShell avec `Invoke-WebRequest`, donc
   inutilisable tel quel comme CI GitHub Actions Linux sans réécriture (pwsh
   existe sur les runners, mais le chemin `C:/Users/Maxime` du point 1 le tue).
4. Le test d'interface exige un serveur déjà lancé sur `127.0.0.1:8090` et un
   compte de vérification : `DOLIBARR_VERIF_LOGIN` (défaut `maxime`) et
   `DOLIBARR_VERIF_MDP` (défaut **vide**). Rien dans la doc n'explique comment
   créer ce compte ; le script se contente d'afficher « serveur non démarré —
   écrans non testés » et poursuit.

### Variables d'environnement : 5 documentées sur 23 lues par le code

Lues par `std::env::var` (relevé sur `crates/**/*.rs`) :

```
DOLIBARR_DSN              documentée (DEMARRER:32, README:24, DEVELOPPEMENT:179)
DOLIBARR_DOCUMENTS        documentée (DEMARRER:33, installer.ps1:88)
DOLIBARR_LECTURE_SEULE    documentée (DEMARRER:55)
DOLIBARR_LOGS             documentée (DEMARRER:78)
DOLIBARR_API_KEY          documentée (DEVELOPPEMENT:311-318)
---
DOLIBARR_WEB_ADDR         NON documentée  (adresse d'écoute du serveur web)
DOLIBARR_MODULES          NON documentée  (dossier des modules déclaratifs)
DOLIBARR_GREFFONS         NON documentée  (dossier des greffons .dll/.so)
DOLIBARR_LANGS            NON documentée
DOLIBARR_LANGS_NOYAU      NON documentée
DOLIBARR_LANGUE           NON documentée
DOLIBARR_SAUVEGARDES      NON documentée  (racine des dumps)
DOLIBARR_SCELLE_CLE       NON documentée  (clé de scellement de la caisse)
DOLIBARR_API_ENTITY       NON documentée  (entité servie par l'API, défaut 1)
DOLIBARR_SMTP_HOST        NON documentée
DOLIBARR_SMTP_PASSWORD    NON documentée
DOLIBARR_ANNUAIRE_TEST    NON documentée  (active les tests réseau BAN/INSEE)
```

(En plus, hors application : `ORACLE_FIXTURE`, lue par les tests d'arrondi, et
`CARGO_TARGET_DIR`, imposée par `tools/pre-commit.ps1` et `tools/verifier.ps1`.)

Lues via le helper `variable()` de `crates/db/src/sauvegarde.rs:26` (donc
`std::env::var` avec défaut) — **aucune n'est documentée** :

```
DOLIBARR_DB_HOST  DOLIBARR_DB_PORT  DOLIBARR_DB_USER
DOLIBARR_DB_PASSWORD  DOLIBARR_DB_NAME  DOLIBARR_MYSQLDUMP
```

Conséquence concrète : `DOLIBARR_WEB_ADDR` décide de l'adresse d'écoute, et
`DOLIBARR_LECTURE_SEULE` est présentée comme le garde-fou principal ; les deux
sont dans la doc. Mais `DOLIBARR_SCELLE_CLE` (clé de scellement de la caisse)
ou `DOLIBARR_SMTP_PASSWORD` ne le sont pas, alors qu'une valeur par défaut
silencieuse y est associée.

### Verdict

- **Windows, quelqu'un qui suit `DEMARRER.md`** : démarre en 5 minutes **à
  condition de remplacer la commande de lancement par `cargo run -q -p web`**.
- **Linux/macOS** : import à refaire avec `--force`, sinon arrêt au premier
  doublon.
- **`README.md` §Démarrage** : non reproductible en l'état.
- **`tools/verifier.ps1`** : ne fonctionne que sur la machine de l'auteur.

---

## 5. Ordre de travail proposé

1. Trancher le nom (`NOM.md`), puis renommer paquets et binaires **avant** la
   première publication — un dépôt renommé après coup casse tous les liens et
   tous les clones.
2. Purger les données personnelles (`SECRETS-AUDIT.md` §A) et les artefacts
   versionnés (§D).
3. Corriger les 8 points du §3 ci-dessus, en priorité `DEMARRER.md:34`.
4. Rendre `tools/verifier.ps1` portable : `CARGO_TARGET_DIR` paramétrable,
   `-Base` aligné sur `installer.ps1`, identifiants de vérification passés en
   paramètres obligatoires.
5. Ajouter `.github/workflows/ci.yml` (Linux, `cargo fmt --check`, clippy,
   `cargo test` sans base — les tests d'intégration s'ignorent sans
   `DOLIBARR_DSN`, cf. `README.md:38-39`), `CONTRIBUTING.md`, `CHANGELOG.md`,
   `rust-toolchain.toml`.
6. Faire 3 captures (tableau de bord, fiche devis, fiche facture) et les mettre
   dans le README.
7. Recréer le dépôt distant, pousser, taguer `v0.1.0`.
8. Publier ensuite seulement `ANNONCE-FORUM.md` et le post LinkedIn.
