# Audit de sécurité — 10 novembre 2026

> Périmètre : le dépôt `cordeau` (24 crates, 1 088 tests) et ses surfaces
> exposées — interface web, API REST, portail client, MCP.
>
> **Ce qui n'a pas pu être fait** : `semgrep` et `cargo-audit` ne sont pas
> installés sur cette machine (`semgrep : absent`, `cargo-audit : absent`). Un
> scan automatique par règles n'a donc pas tourné. L'audit ci-dessous est
> **manuel et ciblé** : il lit le code, il ne le passe pas au crible d'un
> moteur. C'est moins large, et ce n'est pas équivalent.

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

## Reste ouvert

| # | Gravité | Constat | Proposition |
|---|---|---|---|
| 8 | **corrigé** | les deux verrous de `garde.rs` toléraient mal l'empoisonnement (`.unwrap()`) — un panic sous verrou rendait 500 sur toutes les requêtes suivantes | repris en `if let Ok(…)` ; **reste** : 4 `unwrap()` dans `auth.rs` (sessions) |
| 9 | **moyen** | cookie de session sans `Secure` : si l'application passe derrière HTTPS, le cookie peut fuir par une requête HTTP | poser `Secure` quand `X-Forwarded-Proto: https` |
| 10 | **moyen** | dépendances non auditées : `cargo-audit` absent | `cargo install cargo-audit && cargo audit` |
| 11 | **faible** | pas de scan par règles (semgrep) : des motifs connus peuvent dormir dans 24 crates | `pip install semgrep` puis `p/sql-injection`, `p/rust`, `p/secrets` |
| 12 | **faible** | la clé d'API (`DOLAPIKEY`) est comparée par `==` : une comparaison à temps non constant | comparer les empreintes, ou une comparaison à temps constant |

---

## Ce que cet audit ne dit pas

- Il ne dit **pas** que le logiciel est sûr : il dit ce qu'un examen manuel de
  deux heures a trouvé, et ce qu'il n'a pas pu couvrir.
- Il ne couvre **pas** la logique métier (une facture validée deux fois, un
  droit contourné par une route oubliée).
- Il ne remplace pas un test d'intrusion sur une instance exposée.
