# Audit de sécurité — 10 novembre 2026

> Périmètre : le dépôt `cordeau` (24 crates, 1 088 tests) et ses surfaces
> exposées — interface web, API REST, portail client, MCP.
>
> **Ce qui n'avait pas pu être fait le 10** : ni `semgrep` ni `cargo-audit`
> n'étaient installés. L'audit du 10 est donc **manuel et ciblé** : il lit le
> code, il ne le passe pas au crible d'un moteur. `semgrep` a été installé le
> 12 au soir ; `cargo-audit` reste absent. Voir la passe du 12 en fin de
> document.

---

## Corrigé aujourd'hui

### 1. CRITIQUE — panique sur une base neuve

`db::tableau_bord::delai_moyen` lisait `AVG(DATEDIFF(…))` dans un `f64`. Sur une
base sans règlement, la requête rend **NULL**, et le pilote MySQL lève **avant**
que l'`Option` ne soit visible. Conséquence mesurée : le fil de travail meurt, la
page d'accueil répond **000**, et la même panique se répète à chaque
rafraîchissement.

C'est exactement le cas d'une installation neuve — donc le premier écran que
verrait quelqu'un qui clone le dépôt.

```rust
// avant : Option<f64>            → NULL casse le décodage
// après : Option<Option<f64>>    → NULL devient 0,0
```

Vérifié : sur `cordeau_test` (base vierge), `/` répond **200**.

### 2. ÉLEVÉ — aucun frein sur la connexion

Rien ne limitait les tentatives : un mot de passe se devinait par force brute à
quelques milliers d'essais par minute. Ajout d'un compteur **en mémoire**, par
adresse : **cinq échecs en quinze minutes** ferment la porte, réponse **429**.

Vérifié : essais 1 à 5 → 200 (échec normal), essai 6 et 7 → **429**.

Le compteur vit en mémoire et ne survit pas à un redémarrage — c'est accepté :
un attaquant ne redémarre pas le serveur, et un compteur persistant en base
serait une écriture par tentative ratée.

### 3. ÉLEVÉ — mot de passe du portail en SHA-256 simple

Un seul SHA-256 se casse par force brute sur GPU en quelques secondes. La
fonction itère maintenant **100 000 tours**, sans nouvelle dépendance et sans
changer le format stocké (64 caractères hexadécimaux).

Limite assumée : ce n'est pas Argon2 ni bcrypt. Un vrai KDF ajouterait une
dépendance (`argon2`) et changerait le format ; à faire quand le portail servira
en production.

Vérifié : compte créé avec l'empreinte itérée → connexion **200**, mauvais mot de
passe **refusé**.

---

## Vérifié, rien à corriger

### 4. Injection SQL — aucune trouvée

Trois familles de requêtes construites par `format!` :

| Cas | Pourquoi c'est sûr |
|---|---|
| Colonnes constantes (`SELECT_COLS`) | ce sont des littéraux du code |
| Noms de table/colonne variables (`categorie`, `divers`, `extrafields`) | ils viennent d'**énumérations** ou passent le garde `identifiant_sql` (alphanumérique + `_` seulement) |
| Expressions (`divers`) | liste blanche `EXPRESSION_SQL` : `NOW()`, `CURDATE()`, `0`, `1`, `'phy'` |

**Toutes les valeurs** (noms, références, montants) passent par des paramètres
liés. Aucune concaténation de saisie.

### 5. XSS — aucune trouvée

`maud` échappe par défaut. Les `PreEscaped` recensés (9) portent : la feuille de
style (constante), les icônes SVG (constantes) et deux scripts d'interface
(chaînes fixes). Aucune donnée utilisateur n'y entre.

### 6. Traversée de chemin — bloquée

`/api/index.php/documents/download` refuse `..`, les chemins absolus et un
`modulepart` douteux. Vérifié : document inexistant → 404.

### 7. Secrets dans le dépôt — aucun

Recherche des marqueurs personnels et des clés : aucun mot de passe, aucune clé
d'API, aucun identifiant de connexion en clair dans le code versionné.
`_vendor/` (sources Dolibarr, 400 Mo) et `.env` sont ignorés.

---

## Passe du 12 novembre — revue `rust-review` (Trail of Bits)

Vingt-deux clusters d'analyse prévus (mémoire, concurrence, paniques, logique,
FFI, ressources, hygiène statique). **Sept ont tourné** : `panic-dos-1`,
`input-os-safety`, `unsafe-boundary`, `memory-safety-1`,
`concurrency-locking`, `async-runtime`, `error-handling-1`.

Résultat : **33 constats**, dont **18 corrigés** (12 par la revue, 6 depuis).
Les quinze autres clusters n'ont pas été exécutés — ce document ne prétend donc
pas que le dépôt est exempt de ce qu'ils auraient trouvé.

Les sous-agents nommés du greffon n'existent pas sous opencode : les workers ont
tourné en sous-agents génériques, et le dédoublonnage / tri des faux positifs a
été fait à la main.

### Corrigé — sûreté mémoire et chemins

| Constat | Correction |
|---|---|
| `Path::join` avec un composant absolu écrasait le préfixe : une référence `C:\…` sortait de `documents/` | garde `composant_sur` sur les **composants** (`..`, `:`, `\\`, chemin absolu refusés) dans `ged::archiver` et `ged::supprimer` |
| `Vec::with_capacity(duree)` sur une durée venue de l'extérieur : allocation géante avant tout travail | capacité plafonnée |
| `sauvegarde::copier` testait `is_dir()` puis copiait : TOCTOU, un lien symbolique pouvait faire copier une cible hors du dossier | `symlink_metadata` et refus des liens, sans jamais les suivre |
| une DLL du dépôt (`crates/greffons`) était chargée par le chargeur de greffons | le chargeur ne charge plus que depuis le dossier de greffons configuré |

### Corrigé — disponibilité et entrées hostiles

| Constat | Correction |
|---|---|
| `limit` d'API non plafonné : `limit=10^9` chargeait la table en mémoire | plafond dur |
| un argument non numérique tuait le serveur MCP | argument invalide = erreur rendue, pas de panique |
| prix JSON en **nombre** (`12.5`) désérialisé en chaîne vide → ligne à 0 € | nombre et chaîne acceptés, valeur conservée |
| prix ou TVA illisibles dans l'import CSV → 0 € silencieux | la pièce est refusée et nommée |
| solde de clôture illisible → zéro silencieux | erreur nommée |
| import CSV comptait une facture comme créée alors que la validation échouait (brouillon sans numéro légal) | la validation est lue, l'échec est compté |
| PDF non archivé : l'échec était avalé | signalé |

### Corrigé — concurrence

| Constat | Correction |
|---|---|
| mutex de politique tenu **pendant** la requête SQL : tout le serveur se sérialisait derrière le verrou | verrou relâché avant le SQL |
| le frein de connexion ne couvrait pas le portail client | frein réutilisé sur `/public/portail` |
| `caisse_journal::ajouter` lisait, calculait le rang puis écrivait **sans verrou** : deux ventes simultanées prenaient le même rang et la chaîne se scindait (le commentaire affirmait l'inverse) | `File::lock` sur tout le cycle lire/calculer/écrire ; **test de concurrence à 8 fils** ajouté |
| 190 handlers `async` sans aucun `await` d'E/S : chacun bloquait un worker tokio sur MySQL, et quelques pages lentes suffisaient à figer le serveur | `fil_propre` : chaque requête s'exécute sur son propre fil (runtime minimal, plafond de 64), comme PHP. **Aucun handler modifié.** Mesuré : 30 requêtes simultanées → 30 × 200 en 218 ms |

### Corrigé — montants

`money::dec` fait `.expect()` : n'importe quelle chaîne non validée tuait le fil.
Ajout de **`money::tenter`**, faillible, qui accepte la virgule française, les
espaces de milliers et le symbole € ; l'API s'en sert désormais et rend **400**
avec le nom du champ. `dec` reste paniquant, et sa documentation dit maintenant
pourquoi : constantes du code uniquement, jamais une valeur venue d'un client.

Vérifié en réel : `pu_ht: "abc"` → **400** « pu_ht : « abc » n'est pas un
nombre » ; `pu_ht: "12,50"` → **200**, devis à 12,50 HT / 15,00 TTC.

### Corrigé — commentaires qui décrivaient une règle inexistante

Sept affirmations fausses retirées ou remplacées par ce que le code fait :

- `code_pour` annonçait « tout le reste est un 500 » alors qu'il rend **400** sur
  une demande mal formée ;
- `salaire.rs` promettait que `datev` ne peut pas précéder `datep` — **aucun
  contrôle n'existe** ;
- `salaire.rs` annonçait un montant « toujours positif » — le zéro passe ;
- `caisse_journal` affirmait qu'aucune écriture simultanée ne pouvait produire le
  même rang — faux, et c'était la plus dangereuse : elle promettait l'intégrité
  que le module ne tenait pas ;
- le contrat de greffon se présentait comme utilisable par tous les langages — il
  transporte une allocation, donc un `.c` avec son propre `malloc` fait un
  `free` inter-tas ;
- les chiffres du README et des documents de finition (tests, commits).

### Toujours ouvert

| Gravité | Constat | Pourquoi ce n'est pas corrigé ici |
|---|---|---|
| ~~moyen~~ **corrigé** | cookie de session sans `Secure` | `Secure` posé quand `X-Forwarded-Proto: https` ; vérifié en réel : sans en-tête → pas de `Secure`, avec → `Secure`. Absent en HTTP simple, sans quoi le cookie serait inutilisable en développement |
| ~~moyen~~ **corrigé** | `DOLAPIKEY` comparée par `==` | `egal_temps_constant` : ou exclusif octet par octet, sortie constante ; test unitaire ajouté |
| **moyen** | dépendances non auditées (`cargo-audit` absent) | `cargo install cargo-audit && cargo audit` |
| **faible** | `money::dec` panique encore | voulu pour les constantes ; tout appelant qui lui passe une saisie est un défaut à corriger au cas par cas |
| ~~faible~~ **corrigé** | 4 `unwrap()` dans `auth.rs` sur les verrous de sessions | un panic sous verrou rendait 500 sur toutes les requêtes suivantes ; `auth::sessions()` récupère la table empoisonnée (`into_inner`) au lieu de refuser le service |
| **structurant** | les 15 clusters `rust-review` non exécutés | mémoire-2, concurrence-données ×2, paniques-2, récursion ×3, erreurs-2, logique ×2, FFI ×2, hygiène statique, ressources |

### semgrep

Installé le 12 novembre (`uv tool install semgrep`, **1.185.0**) après l'échec du
10. Deux limites constatées :

- **un scan par dossier rend 0 fichier** sur cette machine (semgrep ne résout pas
  les chemins de répertoire sous Windows) — il faut passer la liste
  `git ls-files '*.rs'` ;
- le registre Rust est **mince** : `p/rust` ne contient que **11 règles**. Un scan
  vert ne prouve donc presque rien, et ne remplace pas la revue manuelle.

Le scan par liste de fichiers est lancé sur les 308 fichiers `.rs` du dépôt. Son
résultat est reporté dans le journal de commits, pas ici : un chiffre recopié à
la main dans un document finit toujours par mentir.

---

## Reste ouvert

| # | Gravité | Constat | Proposition |
|---|---|---|---|
| 8 | **corrigé** | les deux verrous de `garde.rs` toléraient mal l'empoisonnement (`.unwrap()`) — un panic sous verrou rendait 500 sur toutes les requêtes suivantes | repris en `if let Ok(…)` ; **reste** : 4 `unwrap()` dans `auth.rs` (sessions) |
| 9 | **moyen** | cookie de session sans `Secure` : si l'application passe derrière HTTPS, le cookie peut fuir par une requête HTTP | poser `Secure` quand `X-Forwarded-Proto: https` |
| 10 | **moyen** | dépendances non auditées : `cargo-audit` absent | `cargo install cargo-audit && cargo audit` |
| 11 | **partiel** | scan par règles : semgrep est installé le 12/11 (1.185.0) ; le scan par dossier rend 0 fichier sous Windows, le scan par liste de fichiers est en cours | voir la section « semgrep » ci-dessus |
| 12 | **faible** | la clé d'API (`DOLAPIKEY`) est comparée par `==` : une comparaison à temps non constant | comparer les empreintes, ou une comparaison à temps constant |

---

## Ce que cet audit ne dit pas

- Il ne dit **pas** que le logiciel est sûr : il dit ce qu'un examen manuel de
  deux heures a trouvé, et ce qu'il n'a pas pu couvrir.
- Il ne couvre **pas** la logique métier (une facture validée deux fois, un
  droit contourné par une route oubliée).
- Il ne remplace pas un test d'intrusion sur une instance exposée.
