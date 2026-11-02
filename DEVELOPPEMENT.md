# Développer sur dolibarr-rs

Ce document dit comment le code est organisé, quelles règles ne se négocient pas,
et comment ajouter un écran ou un module sans rien casser.

Lis-le en entier avant d'écrire la première ligne. Les pièges listés au §7 ont
tous coûté une heure à quelqu'un.

---

## 1. Ce que ce projet est, et n'est pas

**C'est** le cœur transactionnel de Dolibarr réécrit en Rust, **sur le schéma de
données exact de Dolibarr 24.0.1** (410 tables, 5 362 colonnes, empreinte
`7293039ac8f4426f3a4a102552ca681228c2e2ffaec7bac10c9e6cc2cae68b2f`).

**Ce n'est pas** Dolibarr complet. 8 modules sur 95. Voir §9 pour la carte.

La conséquence pratique : **les données sont compatibles, le comportement ne
l'est que là où on l'a écrit**. Une instance Dolibarr PHP peut lire cette base.
Elle ne verra simplement pas ce qu'on n'écrit pas.

---

## 2. Architecture

```
crates/
├── money/     arithmétique monétaire (aucune dépendance interne)
├── auth/      mots de passe llx_user (bcrypt $2y$, md5 historique)
├── legal/     mentions obligatoires françaises (aucune dépendance)
├── schema/    parseur du schéma officiel + empreinte SHA-256
├── db/        accès MySQL : un module par domaine
├── app/       cas d'usage : composition des documents → PDF
├── pdf/       rendu Typst (ne connaît que les vues qu'on lui passe)
├── cli/       binaire d'exploitation
└── web/       interface axum + maud, rendu serveur
```

**Règle de dépendance** : `cli` et `web` dépendent de `app` ; `app` dépend de
`db`, `pdf`, `legal` ; `db` dépend de `money` et `auth`. **Jamais l'inverse.**

Pourquoi c'est important : `pdf` ne calcule rien. Il reçoit des montants
**déjà formatés en chaîne**. C'est ce qui garantit que l'écran et le PDF ne
peuvent pas diverger de un centime.

```
db::proposal ──► app::devis_pdf ──► pdf::render_document
      │                 │                    │
   montants en      formate, ajoute      compose, calcule
    Decimal          les mentions          ZÉRO
```

---

## 3. Les six règles non négociables

1. **Jamais de `SELECT *`, jamais d'`INSERT` sans liste de colonnes.**
   Un module Dolistore peut ajouter une colonne à tout moment ; une requête
   étoilée casse ou récupère du bruit.
2. **Jamais de `f64` pour de l'argent.** `rust_decimal` partout. Les colonnes
   sont `DOUBLE(24,8)` : on les lit avec `CAST(x AS CHAR)` puis `Decimal::from_str`.
3. **Filtre `entity` sur toute lecture et toute écriture.** Un oubli = fuite de
   données entre sociétés.
4. **Les erreurs sont des valeurs, jamais des pages blanches.** Pas de
   `unwrap()` sur une requête ; remonter `Result`.
5. **Chaque écriture passe par une transaction** et réessaie sur doublon (1062)
   comme sur interblocage (1213 / 1205).
6. **Un module sans test n'existe pas.** Les tests tournent contre la vraie base.

---

## 4. Ajouter un écran

Trois fichiers, dans cet ordre.

### 4.1 La route — `crates/web/src/main.rs`

```rust
.route("/ma-page", get(ma_page))
```

Puis le gestionnaire, juste avant `fn pdf_response` :

```rust
async fn ma_page(State(app): State<App>) -> Res<Markup> {
    let lignes = db::party::list(&app.pool, ENTITY, 100, 0)?;
    Ok(ui::page(
        "tiers",        // section active (surlignée dans le bandeau)
        "liste",        // sous-onglet actif
        "Ma page",      // titre du document
        html! { ... },
    ))
}
```

`ui::page(section, onglet, titre, contenu)` rend le bandeau, les sous-onglets et
le contenu. Les valeurs de `section` : `accueil`, `tiers`, `produits`, `devis`,
`factures`, `relances`, `parametres`, ou `""` pour aucune.

### 4.2 L'accès aux données — dans `crates/db/`

Un fichier par domaine, avec la même ossature :

```rust
const SELECT_COLS: &str = "rowid, entity, nom, DATE_FORMAT(datec, '%Y-%m-%d')";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())   // jamais row.get : panique sur NULL
}

pub fn list(pool: &Pool, entity: i64, limit: u32, offset: u32) -> Result<Vec<X>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!("SELECT {SELECT_COLS} FROM llx_x WHERE entity = ? ORDER BY rowid LIMIT ? OFFSET ?");
    let rows: Vec<Row> = conn.exec(sql, (entity, limit, offset))?;
    Ok(rows.iter().map(map_row).collect())
}
```

### 4.3 Le style — `crates/web/src/ui.rs`

Composants disponibles : `euros()`, `pastille(statut, paye, en_retard)`,
`carte(libelle, valeur, note, alerte)`, `vide(message, action)`, `flash()`.
Le CSS est en haut du fichier, sous forme de variables — changer la palette se
fait à un seul endroit.

---

## 5. Ajouter un module métier (une nouvelle table)

1. **Vérifier que la table existe** dans `schema-24.0.1.json`. Si elle n'y est
   pas, c'est que Dolibarr ne la crée pas non plus : le module ne l'aura pas.
2. Créer `crates/db/src/mon_module.rs` sur le modèle de `party.rs`.
3. Déclarer `pub mod mon_module;` dans `crates/db/src/lib.rs`.
4. Écrire `crates/db/tests/mon_module.rs` — voir §6.
5. Ajouter l'accès dans `crates/web/src/main.rs`.
6. **Ne jamais ajouter de colonne, d'index ni de table.** Le schéma appartient
   à Dolibarr. Une colonne ajoutée par nous casserait la compatibilité.

---

## 6. Tester

**Une commande pour tout** :

```powershell
.\tools\verifier.ps1              # formatage, clippy, tests, schéma, chaîne métier, écrans
.\tools\verifier.ps1 -Reseau      # + les tests réseau (annuaire)
```

Elle sort en code 1 au premier échec — utilisable en pré-commit ou en CI.
Chaque étage est indépendant et dit lequel a cassé.

**Git refuse un commit sale.** Les hooks sont installés (à relancer après un
clone, `.git/hooks` n'est pas versionné) :

```powershell
.\tools\installer-hooks.ps1       # installe pre-commit et pre-push
```

| Moment | Ce qui tourne | Durée (~à chaud) |
|---|---|---|
| `git commit` | `cargo fmt --check` + `clippy -D warnings` + `cargo test --workspace` | 20 s |
| `git push` | `verifier.ps1` complet : les trois ci-dessus + schéma + chaîne métier + écrans | 1 min 30 |
| à la main | `.\tools\pre-commit.ps1 -Rapide` (fmt + clippy seuls) | 5 s |

Si la base locale ne répond pas, les tests sont **signalés comme sautés** en
jaune — le commit passe, mais le hook dit clairement qu'il n'a pas vérifié.
Contournement assumé et tracé : `git commit --no-verify`.

**Une seule suite de tests à la fois.** Les tests écrivent dans la vraie base :
deux exécutions simultanées se marchent dessus (références de facture, tiers,
identifiants). Le hook prend un verrou (`%TEMP%\dolibarr-rs-tests.lock`) et
attend son tour ; ne pas lancer `cargo test` à la main pendant qu'un hook tourne.

À la main :

```powershell
$env:DOLIBARR_DSN="mysql://root:root@127.0.0.1:3307/dolibarr"
$env:CARGO_TARGET_DIR="C:/Users/Maxime/.cargo-target/dolibarr-rs"
cargo test --workspace
```

**Sans `DOLIBARR_DSN`, les tests d'intégration s'ignorent proprement** — la CI
reste verte sur une machine sans base.

Règles de test, apprises à la dure :

- **Un jeu de données par test.** Deux tests qui utilisent le même nom de tiers
  se détruisent mutuellement : ils tournent en parallèle.
- **Jamais de `COUNT(*)` global en assertion.** Vérifier la ligne créée, pas le
  compteur de la table.
- **Nettoyer avant, pas seulement après.** Un test interrompu laisse des restes
  qui font échouer le suivant.
- **Les clés étrangères bloquent le ménage** : supprimer les enfants avant les
  parents (`llx_bank_url` → `llx_bank` → `llx_bank_account`).

L'oracle PHP (`crates/money/tests/oracle.rs` + `tools/oracle/`) compare notre
arithmétique à la vraie fonction `calcul_price_total()` de Dolibarr sur 200 cas.
**Tolérance zéro.** C'est la seule preuve sérieuse que les montants sont justes.

---

## 7. Les pièges qui coûtent une heure

| Piège | Ce qu'il faut faire |
|---|---|
| `llx_societe` utilise `nom` et `statut`, pas `name`/`status` | lire le schéma, pas la mémoire |
| `llx_actioncomm` a `id`, pas `rowid` | idem |
| `row.get(i)` **panique** sur NULL | `row.get_opt(i).and_then(|r| r.ok())` |
| `params!{}` + placeholders `?` = erreur runtime | placeholders nommés `:nom` |
| `last_insert_id()` n'existe pas sur `Queryable` | sur `PooledConn` / `Transaction` |
| Un montant `Decimal` imprimé via `normalize()` perd les zéros | `format!("{:.2}", x)` |
| `COUNT+1` pour numéroter boucle à l'infini sur collision | `MAX(suffixe)+1` + décalage d'essai |
| Deux écritures concurrentes se bloquent | réessayer sur 1213 et 1205 |
| Le NAS (Z:) casse l'incremental de cargo | `.cargo/config.toml` pointe le target sur le SSD |
| Une erreur SQL avec accents part en double encodage | envoyer le SQL en ASCII pur (`\u00e9`) |

---

## 8. Conventions Dolibarr à respecter

Ces valeurs ne sont pas des choix esthétiques : Dolibarr les lit.

| Élément | Convention |
|---|---|
| Code client | `CU` + AAMM + `-` + 5 chiffres |
| Réf. devis / facture / avoir / règlement | `PR` / `FA` / `AV` / `PAY` + AAMM + `-` + 4 chiffres |
| Ligne de section | `product_type = 9`, `special_code = 104777`, montants à zéro |
| Avoir | `llx_facture.type = 2`, quantités **négatives**, `fk_facture_source` |
| Lien devis → facture | `llx_element_element` (`propal` → `facture`) |
| Règlement | `llx_paiement` + `llx_paiement_facture` + **`llx_bank`** (sinon invisible en banque) |
| Société | `MAIN_INFO_*` dans `llx_const`, entity 0 |
| Mots de passe | `pass` + `pass_encoding = 'bcrypt'`, format `$2y$` |
| Fichiers | `documents/<modulepart>/<réf>/<réf>.pdf` + `llx_ecm_files` |

---

## 9. Carte du code

| Fichier | Rôle | Lignes |
|---|---|---|
| `db/party.rs` | tiers `llx_societe`, code client | ~330 |
| `db/product.rs` | produits, unités | ~300 |
| `db/proposal.rs` | devis, sections, numérotation `PR` | ~500 |
| `db/invoice.rs` | factures, avoirs, `FA`/`AV` | ~600 |
| `db/payment.rs` | règlements, lettrage, ligne bancaire | ~420 |
| `db/user.rs` | comptes, groupes, authentification | ~260 |
| `db/company.rs` | société dans `llx_const` | ~180 |
| `db/dict.rs` | dictionnaires `llx_c_*` | ~200 |
| `db/journal.rs` | agenda `llx_actioncomm` | ~170 |
| `db/ged.rs` | documents `llx_ecm_files` | ~150 |
| `db/impex.rs` | import/export CSV | ~260 |
| `db/relance.rs` | factures échues, texte de relance | ~170 |
| `db/sauvegarde.rs` | dump + documents | ~200 |
| `legal/` | mentions obligatoires | ~410 |
| `pdf/` | rendu Typst + gabarit | ~520 |
| `app/` | composition des documents | ~300 |
| `web/` | interface | ~2 100 |

---

## 10. Ce qui reste à faire

Par ordre de valeur, avec l'effort estimé :

| Chantier | Effort | Pourquoi |
|---|---|---|
| Import de produits et de factures (CSV) | 3 j | l'import ne fait que les tiers |
| Gabarits PDF multiples | 4–6 j | un seul gabarit aujourd'hui |
| Tâches planifiées (relances auto, purge) | 3–4 j | tout est manuel |
| Utilisateur affiché dans le bandeau | 1 j | demande l'extracteur dans 15 gestionnaires |
| Permissions fines par écran | 5–8 j | suppose de définir les rôles métier d'abord |
| Modules suivants (projets, stocks, banque) | 15–35 j chacun | voir la doc Dolibarr |

**Ce qu'il ne faut pas faire** : copier du code Dolibarr ou d'un module du
Dolistore. Le comportement se spécifie à partir du **résultat observable** (le
PDF, les totaux), jamais à partir de la source. C'est la méthode de l'oracle, et
c'est aussi ce qui protège juridiquement.

---

## 11. Démarrer en 5 minutes

```powershell
# 1. base de test (MariaDB portable, port 3307)
C:/Users/Maxime/.mariadb/server/bin/mysqld.exe `
  --defaults-file=C:/Users/Maxime/.mariadb/my.ini --port=3307 --console

# 2. vérifier que la base est bien celle qu'on attend
cargo run -q -p dolibarr-rs -- schema verify --dsn mysql://root:root@127.0.0.1:3307/dolibarr

# 3. tests
cargo test --workspace

# 4. interface
$env:DOLIBARR_DSN="mysql://root:root@127.0.0.1:3307/dolibarr"
cargo run -q -p web        # http://127.0.0.1:8090
```

Si `schema verify` dit « incompatible », **ne code pas** : la base a dérivé et
tout ce que tu écriras sera faux.


---

## API REST et serveur MCP

### API REST (`dolibarr-web`, prefixe `/api`)

Authentification par en-tete **`DOLAPIKEY`**. La cle vient de la variable
d'environnement `DOLIBARR_API_KEY` (defaut : `dolibarr-rs`). L'API ne passe
**pas** par la session du navigateur : le middleware de session est applique
aux ecrans seulement.

```bash
curl -H "DOLAPIKEY: cle" http://127.0.0.1:8090/api/ping
curl -H "DOLAPIKEY: cle" http://127.0.0.1:8090/api/tiers?limit=3
curl -H "DOLAPIKEY: cle" -H "Content-Type: application/json" \
     -d '{"tiers":17,"lignes":[{"label":"Prestation","qty":"1","pu_ht":"500","tva":"10"}]}' \
     http://127.0.0.1:8090/api/devis
```

| Methode | Route | Effet |
|---|---|---|
| GET | `/api/ping` | version et schema |
| GET/POST | `/api/tiers` | liste / creation |
| GET | `/api/produits` | catalogue |
| GET/POST | `/api/devis` | liste / creation |
| GET | `/api/devis/:id` | detail + lignes + situations |
| POST | `/api/devis/:id/valider` | validation |
| POST | `/api/devis/:id/situation` | `{"pourcentage":"40"}` |
| POST | `/api/devis/:id/commande` | cree la commande |
| GET | `/api/factures`, `/api/factures/:id` | liste / detail |
| POST | `/api/factures/:id/reglements` | `{"montant":"500","mode":"virement"}` |
| GET | `/api/commandes` | liste |
| GET | `/api/banques` | comptes et soldes |

Les montants sortent en **chaines decimales** (`"4109.6"`), jamais en flottants.

### Serveur MCP (`crates/mcp`, binaire `dolibarr-mcp`)

JSON-RPC 2.0 sur stdin/stdout — 13 outils :

```
lister_tiers  creer_tiers  lister_devis  lire_devis  creer_devis
valider_devis  facturer_situation  creer_commande
lister_factures  lire_facture  encaisser_facture
solde_banque  solde_tva
```

Declaration dans opencode (`~/.config/opencode/opencode.json`) :

```json
{
  "mcp": {
    "dolibarr": {
      "type": "local",
      "command": ["C:/Users/Maxime/.cargo-target/dolibarr-rs/debug/dolibarr-mcp.exe"],
      "environment": { "DOLIBARR_DSN": "mysql://root:root@127.0.0.1:3307/dolibarr" },
      "enabled": true
    }
  }
}
```

Test rapide sans client :

```powershell
'{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | & dolibarr-mcp.exe
```
