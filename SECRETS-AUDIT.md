# Audit des secrets et des fichiers à exclure

Audit du **5 novembre 2026**, sur le commit `d5a9a4e` (branche `master`).

Périmètre : les **238 fichiers suivis par git** (`git ls-files`). Recherche
`git grep` sur les motifs `password / passwd / mot de passe / secret / token /
apikey / bearer / BEGIN PRIVATE KEY`, chaînes de connexion
(`mysql://`, `postgres://`…), `IBAN / BIC / SWIFT`, `SIREN / SIRET / TVA intra`,
adresses e-mail, numéros de téléphone français, chemins personnels
(`C:/Users`, `/Users/`, `X:\…`), et `*.pem / *.key / *.p12 / *.ppf / id_rsa`.

`_vendor/` et `target/` sont hors périmètre (non suivis, ignorés).
`sql-ready/` a été contrôlé séparément (§B.6).

Conclusion en une ligne : **aucune clé d'API, aucun mot de passe de production,
aucun fichier `.env`, aucune clé privée dans le dépôt**. En revanche **un IBAN
bancaire réel, le nom et l'adresse d'un client particulier, un e-mail
professionnel et un numéro de téléphone au format réel** sont dans le code de
test et dans le README : à traiter avant publication, ce n'est pas un détail.

---

## A. Vrais positifs — à corriger avant publication

### A.1 — IBAN bancaire réel (gravité haute)

`FR7616807003483848689521643` (Banque Populaire, compte de l'entreprise),
recopié dans **quatre** fixtures :

| Fichier | Ligne | Contexte |
|---|---|---|
| `crates/pdf/src/lib.rs` | 771 | `document_de_test()`, module `tests_gabarits` (`#[cfg(test)]`, ligne 751) — forme espacée `FR76 1680 7003 4838 4868 9521 643` |
| `crates/facturx/src/lib.rs` | 960 | module `tests` (`#[cfg(test)]`, ligne 903) |
| `crates/facturx/tests/pdf_join.rs` | 47 | test d'intégration |
| `crates/db/src/sepa.rs` | 268 et 269 | module `tests` (`#[cfg(test)]`, ligne 245) |

Avec l'IBAN, la fixture `crates/pdf/src/lib.rs:771` porte aussi la référence
`PROV612` (ligne 765) et l'adresse `17 chemin du Grimpillon` (ligne 776) : c'est
une copie de données réelles, pas un jeu inventé.

**Pourquoi c'est un vrai problème** : un IBAN publié avec le nom du titulaire
permet une fraude au faux RIB / un détournement de virement, et l'IBAN est
permanent (le changer casse tous les mandats SEPA en cours).

**Correction** : remplacer partout par l'IBAN de test déjà utilisé dans
`crates/db/tests/banque.rs:39` (`FR7600000000000000000000000`) ou un IBAN
synthétique valide modulo 97.

### A.2 — Nom et adresse d'un client particulier (gravité haute, RGPD)

| Fichier | Ligne | Contenu |
|---|---|---|
| `crates/legal/src/lib.rs` | 332 | `nom: "M&Mme Roure"` |
| `crates/legal/tests/conformite.rs` | 40 | `nom: "SARL Sanchez"` |
| `crates/pdf/examples/apercu.rs` | 92 | `name: "M. et Mme Roure"` |
| `crates/pdf/examples/devis.rs` | 69 | `name: "M. et Mme Roure"` |
| `crates/pdf/src/lib.rs` | 785 | `name: "M. et Mme Roure"` (fixture) |
| `crates/pdf/tests/render.rs` | 20 | `name: "M. et Mme Roure"` |
| `crates/facturx/src/lib.rs` | 927 | `nom: "M. Charcosset"` |
| `crates/facturx/tests/pdf_join.rs` | 25 | `nom: "M. Charcosset"` |
| `crates/sepa/src/lib.rs` | 459 | `nom: "M. Charcosset"` |
| `README.md` | 26, 28, 386, 416 | `M&Mme Roure` dans les exemples de commandes et de sorties |

L'adresse `17 chemin du Grimpillon du Monteillier`, `69410`
Saint-Cyr-au-Mont-d'Or figure en plus à `crates/legal/src/lib.rs:320`,
`crates/legal/tests/conformite.rs:20`, `crates/pdf/examples/{apercu.rs:84,
devis.rs:61}`, `crates/pdf/src/lib.rs:777`, `crates/pdf/tests/render.rs:9`,
`README.md:193`, et comme **texte d'aide (`placeholder`) dans le formulaire de
création de tiers** : `crates/web/src/main.rs:2934`, `3661`, `4053`.

C'est le nom d'un client réel associé à l'adresse de son chantier (donnée
personnelle : nom + adresse d'une personne physique). Dans les fixtures, cette
adresse est utilisée comme adresse **de l'entreprise**, ce qui la rend d'autant
plus visible.

**Correction** : `M. Dupont` / `1 rue de l'Exemple` / `00000 Ville`. Le
`placeholder` du formulaire doit devenir générique (« 12 rue des Lilas,
69000 »).

### A.3 — E-mail professionnel réel (gravité moyenne)

`contact@maconnerie-architecture.fr` —

| Fichier | Ligne | Contexte |
|---|---|---|
| `README.md` | 196 | exemple `company set --email` |
| `crates/legal/src/lib.rs` | 325 | fixture société |
| `crates/web/src/courriel.rs` | 198 | test `adresse_valide` |

Ce n'est pas un secret (l'adresse est publique par nature), mais c'est
l'identifiant direct du mainteneur dans un dépôt qui se veut un projet open
source neutre, et cela relie le code à l'entreprise. À remplacer par
`contact@exemple.fr`, déjà utilisé partout ailleurs.

### A.4 — Téléphone au format réel (gravité moyenne)

`crates/legal/tests/conformite.rs:28` — `telephone: Some("06 15 49 13 71")`.
Le reste du dépôt utilise `0600000000` (`crates/db/tests/contact.rs:67`).
Numéro au format d'un mobile français réel : à remplacer par `0600000000`.
(À vérifier par le mainteneur : si c'est bien son numéro, c'est une donnée
personnelle publiée.)

### A.5 — Identité du mainteneur (gravité faible)

- `crates/db/tests/utilisateurs.rs:47-48` et `:67` —
  `nom: "Rigot"`, `prenom: "Maxime"`, assertion `Maxime Rigot`.
- `tools/verifier.ps1:96` — `$env:DOLIBARR_VERIF_LOGIN = "maxime"` par défaut.
- `crates/i18n/src/lib.rs:42, 498-504, 526` — `"Maxime"` en exemple.

Pas un secret, mais le nom complet du mainteneur n'a pas à être le jeu de test
par défaut. `Dupont` / `jean.dupont`.

### A.6 — Chemins personnels absolus (gravité faible, casse chez les autres)

| Fichier | Ligne | Contenu |
|---|---|---|
| `README.md` | 5 | `C:\Users\Maxime\Documents\Default Project\DOLIBARR-FORUM-PROBLEMES.md` (fichier **hors dépôt** : lien mort pour tout lecteur) |
| `README.md` | 108-109 | `C:/Users/Maxime/.mariadb/server/bin/mysqld.exe` |
| `DEVELOPPEMENT.md` | 180 | `$env:CARGO_TARGET_DIR="C:/Users/Maxime/.cargo-target/dolibarr-rs"` |
| `DEVELOPPEMENT.md` | 287-288 | `mysqld.exe` + `my.ini` de l'auteur |
| `DEVELOPPEMENT.md` | 359 | `"C:/Users/Maxime/.cargo-target/dolibarr-rs/debug/dolibarr-mcp.exe"` |
| `tools/verifier.ps1` | 21 | `$env:CARGO_TARGET_DIR = "C:/Users/Maxime/.cargo-target/dolibarr-rs"` |
| `tools/pre-commit.ps1` | 22 | idem |
| `tools/oracle/README.md` | 29 | `C:/Users/Maxime/.cargo-target/ag-oracle` |

Le cas `tools/verifier.ps1:21` n'est pas seulement cosmétique : le script impose
un répertoire de compilation qui n'existe pas chez un autre utilisateur, avant
même d'appeler `cargo` (voir `PUBLICATION.md` §4).

---

## B. Faux positifs — vérifiés, rien à faire

### B.1 — Chaînes de connexion `mysql://root:root@127.0.0.1:3307/dolibarr`

Sept emplacements : `crates/cli/src/main.rs:21` (`DEFAULT_DSN`),
`crates/web/src/main.rs:116`, `crates/mcp/src/main.rs:20`,
`crates/db/src/{pos_ventes,recur_generation,recurrence}.rs` (doc de module),
`tools/pre-commit.ps1:13`, `tools/verifier.ps1:5/10`, plus `README.md:24/124`,
`DEMARRER.md:32/56`, `DEVELOPPEMENT.md:179/291/297/360`.
**Faux positif** : base locale de développement, `root`/`root` sur `127.0.0.1`,
documentée comme telle. Ce n'est pas une fuite.

### B.2 — Mot de passe de sauvegarde par défaut

`crates/db/src/sauvegarde.rs:79-82` — `DOLIBARR_DB_PASSWORD` avec défaut `root`,
transmis à `mysqldump` par `--password=`. Défaut local, pas un secret.
Point d'hygiène mineur : le mot de passe apparaît dans la ligne de commande du
processus (`ps` le voit). À signaler dans la doc, pas à corriger ici.

### B.3 — Secrets de test

| Fichier | Ligne | Valeur | Nature |
|---|---|---|---|
| `crates/einvoicing/tests/norme_afnor.rs` | 46 | `secret: "s3cr3t"` | fixture |
| `crates/einvoicing/tests/norme_afnor.rs` | 64, 88… | `"access_token":"j1"` | faux serveur |
| `crates/mail/src/lib.rs` | 506 | `mot_de_passe: "secret"` | fixture |
| `crates/db/tests/utilisateurs.rs` | 46, 120 | `MotDePasse!2026`, `NouveauSecret2026` | fixture |
| `crates/auth/src/lib.rs` | 146-180 | empreintes bcrypt/md5 de test | vecteurs de test |
| `installer.ps1` | 3, 10 | `-MotDePasse root` | défaut documenté |

### B.4 — IBAN, SIRET, SIREN factices

- `crates/db/tests/banque.rs:39` — `FR7600000000000000000000000` (factice).
- `crates/pdf/src/lib.rs:34` — `IBAN FR76 1234 5678 9012 3456 7890 123` (exemple de doc).
- `README.md:195`, `crates/legal/src/lib.rs:322-323`,
  `crates/legal/tests/conformite.rs:24-25` — `12345678900012`,
  `FR12345678900`, `98765432100019` (factices).
- `crates/annuaire/src/lib.rs:226` — `213801855` = SIREN de la ville de Lyon,
  donnée publique, utilisée comme test de l'API `recherche-entreprises`.

### B.5 — Clé d'API par défaut

`crates/web/src/api.rs:32-37` — `cle_attendue()` retourne `"dolibarr-rs"` si
`DOLIBARR_API_KEY` n'est pas posée ; `DEVELOPPEMENT.md:312` le documente.
**Ne pas confondre avec une fuite** : aucune clé réelle n'est écrite. En
revanche c'est un **identifiant par défaut connu et non modifiable sans
redémarrage** : quiconque atteint le port 8090 avec `DOLAPIKEY: dolibarr-rs`
passe l'authentification de l'API. Point de durcissement, pas de publication
(voir §C).

### B.6 — `sql-ready/*.sql`

Contrôlé à part (le périmètre de recherche l'excluait) :
`sql-ready/1_tables.sql` (232 907 o), `2_keys.sql` (101 681 o),
`3_data.sql` (613 312 o). Aucun `INSERT INTO llx_user`, aucune occurrence de
`Pierre Angulaire`, `maconnerie-architecture`, `Rigot`, `MAIN_INFO` ni IBAN.
`3_data.sql` ne contient que les dictionnaires Dolibarr (pays, devises, TVA,
unités…). Les adresses `robot@domain.com` et `no-reply@mydomain.tld`
(`3_data.sql:4702, 4721`) sont les valeurs d'origine de Dolibarr.
**Rien à exclure.**

### B.7 — Absence de secrets d'infrastructure

`git ls-files` ne contient **aucun** `.env`, `*.pem`, `*.key`, `*.p12`,
`*.pfx`, `id_rsa*`, `*.ppk`. Aucun fichier `.env` sur le disque non plus.
Le mot « password » de `LICENCE.txt:340` est le texte de la GPL.
Les dossiers `logs/` (191 o), `server.log` (87 o), `target/` et `.cargo/` sont
bien ignorés par `.gitignore` — sauf deux exceptions, voir §D.

---

## C. Points de sécurité (pas des fuites, à traiter en même temps)

1. **Clé d'API par défaut** (`api.rs:35`) : refuser de démarrer si
   `DOLIBARR_API_KEY` est absente et que l'adresse d'écoute n'est pas
   `127.0.0.1` (`DOLIBARR_WEB_ADDR`, voir `PUBLICATION.md` §4).
2. **`DOLIBARR_API_ENTITY`** (`api.rs:22`) : défaut `1`. Une clé = une entité,
   mais rien n'empêche une clé d'être réutilisée sur une autre entité si la
   variable n'est pas posée. À documenter.
3. **`DOLIBARR_VERIF_MDP`** vide par défaut (`tools/verifier.ps1:97`) : le test
   d'interface se connecte avec un mot de passe vide si la variable n'est pas
   posée ; il échoue proprement, mais le message n'explique pas quoi poser.

---

## D. Fichiers à exclure de la publication

### D.1 — Suivis par git alors qu'ils ne devraient pas l'être

| Fichier | Taille | Pourquoi | Action |
|---|---|---|---|
| `greffons/greffon_exemple.dll` | 143 872 o | **binaire compilé Windows** versionné. Produit par `crates/greffon-exemple` (`cargo build -p greffon-exemple`). `.gitignore` ne couvre ni `*.dll` ni `*.so` ni `*.exe`. Un clone Linux reçoit un `.so`-manquant et un `.dll` inutile. | `git rm --cached` + règle `*.dll` / `*.so` / `*.dylib` dans `.gitignore` |
| `server.err` | 0 o | artefact d'exécution (sortie d'erreur du serveur), versionné parce que `.gitignore` ne connaît que `*.log`. Le fichier est **vide**, il n'apporte rien. | `git rm --cached` + règle `*.err` |
| `crates/web/src/main.rs` | 230 010 o | **modifié non commité** au moment de l'audit (`git status`) — avec `crates/web/src/actions_masse.rs`. | publier depuis un arbre propre, pas depuis cet état |

### D.2 — Présents sur le disque, correctement ignorés

| Chemin | Taille | Statut |
|---|---|---|
| `server.log` | 87 o | ignoré (`*.log`) |
| `logs/erreurs.log` | 191 o | ignoré (`logs/`) |
| `target/` | — | ignoré |
| `.cargo/config.toml` | 223 o | ignoré (`.cargo/config.toml`), mais **indispensable en local** : il redirige `target-dir` vers `C:` (le dépôt vit sur le NAS `Z:`) |
| `_vendor/` | — | ignoré, sert uniquement à régénérer `sql-ready/` |

### D.3 — Fichiers de plus de 1 Mo : à garder, avec justification

| Fichier | Taille | Pourquoi il reste |
|---|---|---|
| `schema-24.0.1.json` | 1 507 623 o | manifeste du schéma 24.0.1, utilisé par `schema verify` (`README.md:124`, `tools/verifier.ps1:64`) et cité comme référence (`README.md:137-147`). Sans lui, la vérification d'empreinte ne tourne pas. |
| `crates/pdf/assets/fonts/DejaVuSans.ttf` | 757 076 o | police du rendu PDF Typst. **Aucun fichier de licence joint** dans `crates/pdf/assets/` : la licence DejaVu exige que son texte accompagne la redistribution des fichiers de police. À ajouter (fichier neuf, pas de modification). |
| `crates/pdf/assets/fonts/DejaVuSans-Bold.ttf` | 705 684 o | idem |

Autres gros fichiers, tous légitimes (code ou données nécessaires au démarrage à
froid) : `sql-ready/3_data.sql` (613 312 o), `sql-ready/1_tables.sql`
(232 907 o), `crates/web/src/main.rs` (230 010 o), `Cargo.lock` (119 534 o),
`sql-ready/2_keys.sql` (101 681 o). `Cargo.lock` **doit** rester (le dépôt
produit des binaires).

### D.4 — Rien d'autre

Aucun dump de base, aucun PDF, aucune archive, aucun fichier `.bak`, aucun
cache, aucune capture d'écran, aucune vidéo dans les fichiers suivis
(`git ls-files` ne contient aucune extension image — voir `PUBLICATION.md` §2).

---

## E. Lacunes du `.gitignore` (à corriger, je ne le modifie pas)

```
manque : *.dll  *.so  *.dylib       → greffons/greffon_exemple.dll est suivi
manque : *.err                      → server.err est suivi
manque : *.pprof  *.sql.gz  *.dump  → aucun aujourd'hui, mais `tache sauvegarde`
                                       écrit des dumps (DOLIBARR_SAUVEGARDES)
```

Déjà couvert et vérifié : `target/`, `**/*.rs.bk`, `documents/`, `logs/`,
`*.log`, `*.pdf`, `.env`, `.env.*`, `.cargo/config.toml`, `_vendor/`,
`Thumbs.db`, `desktop.ini`.

Remarque : `*.pdf` ignore aussi d'éventuels documents légitimes (une maquette
PDF jointe à la doc ne serait pas versionnée). Sans conséquence aujourd'hui.

---

## F. À faire avant le premier push (ordre d'exécution)

1. Remplacer l'IBAN réel par un IBAN factice — 4 fichiers, 5 occurrences (§A.1).
2. Remplacer les noms et l'adresse des clients réels — 10 fichiers + 3
   `placeholder` (§A.2).
3. Remplacer l'e-mail et le téléphone réels (§A.3, §A.4).
4. Remplacer `Maxime Rigot` / login `maxime` par un jeu de test neutre (§A.5).
5. Neutraliser les chemins `C:/Users/Maxime` et retirer le lien mort
   `README.md:5` (§A.6).
6. Sortir `greffons/greffon_exemple.dll` et `server.err` du suivi, compléter
   `.gitignore` (§D.1, §E).
7. Ajouter la licence des polices DejaVu (§D.3).
8. Commiter depuis un arbre propre (les deux fichiers `crates/web/src/*.rs`
   modifiés aujourd'hui ne sont pas dans le commit audité).
