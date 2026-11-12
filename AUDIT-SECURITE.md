# Audit de sécurité — 10 novembre 2026

> Périmètre : le dépôt `cordeau` (24 crates, 1 099 tests) et ses surfaces
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

## Passe du 12 novembre (suite) — logique metier, paniques, outils

### Revue « logique metier » — 12 constats, 6 corriges

| Constat | Correction |
|---|---|
| **critique** — le droit d'**ecriture** n'etait verifie que sur 21 prefixes d'URL sur 71 : ailleurs `exiges` etait vide, donc **aucun controle** — un simple utilisateur connecte pouvait payer un salaire, valider un don, encaisser en caisse | la carte couvre tous les prefixes, et une ecriture sans droit mappe est **refusee** (403 nomme, journalise) au lieu d'etre libre |
| **eleve** — les identifiants d'ecriture 9011..9019 etaient utilises sans etre **definis** : impossible de les attribuer, tout non-administrateur etait refuse sans recours | les neuf droits existent dans `llx_rights_def`, avec un libelle ; `coeur` vide, donc une autorisation « lire » de Dolibarr n'ouvre pas l'ecriture |
| **eleve** — le garde echouait en **ouvert** (`unwrap_or(true)`) : base muette ou utilisateur supprime = tous les ecrans ouverts | echec ferme (`false`) : « ne pas savoir » ne vaut pas « autoriser » |
| **eleve** — `payment::delete` ne ramenait jamais une facture de « payee » a « validee » : reste du invisible dans les deux filtres de relance | `CASE WHEN soldee THEN 2 WHEN fk_statut = 2 THEN 1` |
| **eleve** — supprimer un reglement laissait son ecriture au grand livre et au FEC | l'ecriture du reglement part avec lui (filtre journal BQ + `doc_type`) |
| **eleve** — `invoice delete` (CLI) supprimait une facture **validee**, son numero legal, ses reglements et son ecriture | refuse : « emettez un avoir, ou remettez-la en brouillon » |
| moyen — le PDF n'etait pas archive en validation de masse, et l'echec etait avale | signale au bilan, comme a l'unite |
| moyen — `fk_unit` n'etait pas recopie du devis vers la facture : le PDF client perdait « m2 », « ml », « forfait » | l'unite voyage (devis, commande, avoir, recurrentes, caisse = piece) |

**Reste ouvert** (non corrige, dit franchement) : la deduction de l'acompte n'est
faite que sur le chemin commande → facture (devis → facture refacture
l'acompte) ; le controle des factures de situation est a sens unique ; un numero
legal peut etre consomme par un brouillon supprime ; le numero d'une facture
validee n'est pas force dans l'ordre chronologique ; un encaissement superieur au
du est accepte (le trop-percu est masque par l'ecretage du solde) ; aucun garde
n'empeche deux commandes issues du meme devis.

### Revue « paniques et ressources » — 11 constats, 8 corriges

| Constat | Correction |
|---|---|
| **haute** — le MCP mourait sur `creer_devis` des qu'un montant etait illisible (`"qty":"deux"`) | `money::tenter`, erreur JSON-RPC nommee par ligne ; un nombre JSON est lu comme un nombre |
| **haute** — `POST /prets` avec `nbterm=1e9` : une boucle de 4 milliards d'iterations puis une allocation mortelle (le processus entier tombait) | borne a 600 echeances, dans `echeancier` **et** dans `create` (refus explicite) |
| moyenne — API : `money::dec` sur `pourcentage` et `montant` ⇒ 500 muet, meme sur `"30,5"` | `money::tenter` ⇒ 400 nomme |
| moyenne — `extrafields::php_tableau` : compte lu dans la base (`a:100000000:{`) et `split_at` hors frontiere UTF-8 | compte borne a 64, frontiere de caractere verifiee ; **test** ajoute |
| moyenne — verrous de session du middleware en `unwrap()` : un panic sous verrou rendait 500 sur **toutes** les requetes | `auth::sessions()` recupere la table empoisonnee |
| faible — PDF temporaire du MCP ecrit sous un nom previsible dans le dossier partage | nom unique (processus + horodatage) et `create_new` : un fichier deja present est refuse, jamais suivi |

**Reste ouvert** : `money::line_totals` peut deborder `Decimal` (montant ×
quantite proche de 8e28) et `rust_decimal` panique sur ses operateurs — la
borne n'est posee qu'aux portes qui passent par `money::tenter` ; le journal de
caisse se relit entierement a chaque ticket (O(n2) cumule, pas de rotation) ; le
serveur MCP lit stdin sans plafond et rien n'englobe les outils dans
`catch_unwind` ; `mrp::exploser` ne depile pas son chemin (faux « Cycle » sur une
nomenclature en diamant) ; l'import CSV prend une connexion MySQL par ligne.

### Outils, cette fois installes et passes

**semgrep 1.185.0** (`uv tool install semgrep`) : 21 constats sur 308 fichiers.

| Regle | Nombre | Verdict |
|---|---|---|
| `unsafe-usage` | 10 | le chargeur de greffons (FFI) — deja audite par le cluster `unsafe-boundary`, et la DLL n'est plus chargee depuis le depot |
| `temp-dir` | 8 | un seul dans du code de production (le PDF du MCP, corrige ci-dessus) ; les sept autres sont dans des tests |
| `args` | 3 | faux positifs : `std::env::args()` de la ligne de commande et de deux exemples |

Limites constatees : un scan **par dossier** rend 0 fichier sur cette machine
(semgrep ne resout pas les chemins de repertoire sous Windows), il faut passer
la liste `git ls-files '*.rs'` ; et `p/rust` ne contient que **11 regles**. Un
scan vert ne prouve donc presque rien, et ne remplace pas la revue manuelle.

**cargo-audit 0.23.0** : 497 dependances, **3 vulnerabilites**, 9 avertissements.

| Crate | Constat | Traitement |
|---|---|---|
| `lopdf` 0.35 | debordement de pile sur un PDF profondement imbrique (7.5) | **corrige** : passe en 0.42, compilation et tests verts |
| `quick-xml` 0.38 | deux denis de service par XML (7.5) | **non corrigeable ici** : la version vient de `typst` (styles bibliographiques), qui ne lit que des fichiers embarques. A suivre a chaque montee de typst |
| `bincode` 1.3.3 | plus maintenu | depend de `typst` aussi, sur des donnees internes ; rien a faire de notre cote |

---

## Passe du 12 novembre (fin) — les cinq chantiers de fond

### 1. Comptabilite des ventes

| Constat | Correction |
|---|---|
| le chemin **devis -> facture** ne deduisait pas l'acompte deja encaisse (seul le chemin commande -> facture le faisait) : le client payait deux fois la part versee | `acomptes_du_devis` cherche les acomptes valides par les **trois** marques qui coexistent (lien `llx_element_element`, `ref_ext = PA-ACOMPTE-<id>` des automatisations, `ref_ext = AC-<ref>` du noyau) et les deduit par taux de TVA ; **test d'integration** : 1 000 HT - 300 d'acompte = 700 HT / 840 TTC |
| le garde « ce devis a deja ete facture » comptait **l'acompte** comme une facturation : impossible de sortir la facture finale des qu'un acompte existait | le garde ignore le type acompte |
| les **situations** ne controlaient que les situations : un devis deja facture en une fois passait inapercu, et les situations s'ajoutaient par-dessus | refus explicite, avec le numero de la facture en cause |
| le **numero legal** etait attribue au **brouillon** : le supprimer laissait un trou dans la suite, et rien n'obligeait a valider dans l'ordre chronologique | un brouillon porte `(PROV<rowid>)` ; `FA<periode>-<nnnn>` est attribue **a la validation**, sous le verrou de numerotation. Comme Dolibarr |

### 2. Montants

`money::tenter` refuse au-dela de **10^12** (mille milliards) : `rust_decimal`
**panique** quand une multiplication depasse 7,9e28, ce qui tuait le fil de la
requete. Le formulaire de devis lit ses nombres avec `tenter` et nomme le champ
fautif ; `line_totals` borne ses entrees en derniere barriere (donnee relue en
base, fichier importe). Test : `MAX_MONTANT == 10^12`, et la constante est
verifiee contre la chaine.

### 3. Vieillissement

| Constat | Correction |
|---|---|
| chaque ticket de caisse relisait **tout** le journal pour trouver la derniere ligne (O(n2) cumule, verrou tenu pendant la lecture) | lecture par la fin, par blocs de 4 Kio : cout constant. Pas de rotation — la chaine de hachage doit rester entiere, et le cout ne depend plus de l'age du journal. Test : 400 ecritures, chaine verifiable |
| l'import CSV prenait **une connexion MySQL par ligne** | une connexion hors de la boucle, et les noms existants charges en une requete |

### 4. Serveur MCP

Ligne stdin bornee a **4 Mio** (au-dela : erreur -32600 et la ligne est jetee
jusqu'au saut suivant, pour que la requete suivante soit lue au bon endroit), et
`catch_unwind` autour de chaque outil : une panique rend une erreur JSON-RPC au
lieu de tuer le service. Verifie en reel : `initialize` -> 200 logique,
ligne de 5 Mio -> -32600, `tools/list` -> reponse normale.

### 5. Nomenclature

`mrp::exploser` empilait son chemin sans jamais le depiler : la meme piece
presente dans deux sous-ensembles (cas ordinaire) etait declaree « Cycle ».
Chaque noeud pose desormais une marque de fermeture, et le chemin ne contient
plus que la branche en cours. Test : diamant A -> B, C -> vis, quantites 2 + 3.

### Suite : les trois points qui restaient

| Constat | Correction |
|---|---|
| **trop-percu masque** : un encaissement au-dela du du etait accepte, et `balance` ecrétait le reste a zero — le client avait paye 300 EUR de trop et rien ne le disait | refuse, avec le reste du et l'ecart nommes : « le reglement (1500) depasse le reste du (1200.00) de 300.00 : corrigez le montant, ou emettez un avoir ». Test : 1500 refuse, 1200 solde |
| **deux commandes du meme devis** : le chemin devis -> facture avait son garde, pas devis -> commande — deux commandes, donc deux factures, pour un seul devis | garde **sous le verrou de numerotation** (sinon deux clics simultanes passent tous les deux avant que le lien n'existe) ; test : la seconde commande est refusee, et trois devis donnent bien trois references uniques |
| **acompte non deduit des situations** : 30 % d'acompte puis quatre situations de 25 % facturaient 130 % du devis | l'acompte (memes trois marques) est deduit de la situation finale, et toute situation qui ferait depasser le TTC du devis est refusee en nommant le deja-facture. Test : 3 500 TTC = 1 750 + 1 390 + 360, une quatrieme situation est refusee |

### Ce qui reste ouvert, et pourquoi

- **Numerotation Dolibarr PHP** : notre noyau attribue le numero legal a la
  validation, Dolibarr PHP le sien a sa maniere ; les deux outils partagent la
  table, pas la politique de numerotation. Un document cree par PHP garde ses
  conventions.
- **Deduction de l'acompte sur les situations intermediaires** : seule la
  situation finale deduit l'acompte (les intermediaires facturent leur
  pourcentage). Le plafond global, lui, empeche tout depassement.

---

## Passe du 12 novembre (cloture) — concurrence, recursion, Factur-X

### Quatre courses qui produisaient des pieces fausses

| Constat | Correction |
|---|---|
| **critique** — le garde « ce devis a deja ete facture » etait lu **hors du verrou**, et le lien `llx_element_element` pose **apres** que le verrou soit rendu : deux clics simultanes creaient deux factures pour un devis | garde, creation et lien tiennent sous le **meme** `GET_LOCK` ; le lien est pose avant de le rendre, puisque c'est lui qui rend le garde vrai pour l'appel suivant. Meme chose pour devis -> commande (deja sous verrou) |
| **eleve** — les situations lisaient le cumul **avant** le verrou et reutilisaient une liste perimee : deux situations de 60 % facturaient 120 % du devis | la lecture du cumul et le controle passent sous le verrou, sur la connexion qui ecrit (`situations_avec`) |
| **eleve** — l'acompte etait insere en facture **type 0**, puis requalifie type 3 par un `UPDATE` hors transaction : un echec laissait une facture de 30 % invisible pour la deduction, donc facturee deux fois | le type et la reference externe partent **dans l'INSERT** (`NewInvoice::ref_ext`), sous le verrou : une seule ecriture |
| **eleve** — `payment::delete` ne prenait aucun verrou : il recalculait `paye` sur un instantane et pouvait ecraser un encaissement concurrent | meme `GET_LOCK` que la creation du reglement |

### Une recursion qui tuait le processus

`sauvegarde::compter` suivait les liens symboliques : un lien vers son propre
parent faisait recurrer jusqu'a la mort de la pile — donc du **processus web**,
atteignable par un simple `GET /parametres/sauvegarde`. `copier` avait le garde,
`compter`, qui parcourt le meme arbre, ne l'avait pas. Les deux refusent
desormais les liens et bornent la profondeur a 64.

### Factur-X : une crate validee que rien n'appelait

`facturx` produisait un XML EN 16931 et savait l'attacher a un PDF, avec ses
tests verts — et **aucun** appel dans le web, la CLI ou le MCP. Autrement dit :
la reforme francaise etait « prete » sur le papier seulement.

Branchement fait :

- `app::facture_facturx(pool, entity, id, profil)` : le PDF habituel, XML
  attache, construit a partir de la **meme** facture que le PDF (memes lignes,
  memes totaux — les deux ne peuvent pas diverger) ;
- `cordeau invoice facturx --id N --out f.pdf [--profil minimum|basic|en16931]` ;
- verifie en reel : PDF de 46 328 octets, XML de 4 239 octets relu **depuis le
  PDF produit** (preuve que la piece jointe est bien la) ; profil BASIC :
  4 236 octets.

### Ce qui reste ouvert

- `einvoicing` (reception via plateforme agreee) reste non branchee : il faut une
  application declaree chez une plateforme, donc un partenariat, pas du code.
- `devises`, `paie` ne sont dependues par personne : code ecrit, jamais appele.
- `impex` : l'import des produits et des factures prend encore une connexion par
  groupe de lignes (le correctif des tiers n'a pas ete applique aux deux autres),
  et le regroupement y est en O(n2).
- Deux codes de TVA d'exoneration (VATEX) ne sont pas lus depuis le dictionnaire :
  une ligne a 0 % sur un assujetti sera signalee par `regles_metier` plutot que
  remplie automatiquement.

---

## Reste ouvert

| # | Gravité | Constat | Proposition |
|---|---|---|---|
| 8 | **corrigé** | les deux verrous de `garde.rs` toléraient mal l'empoisonnement (`.unwrap()`) — un panic sous verrou rendait 500 sur toutes les requêtes suivantes | repris en `if let Ok(…)` ; **reste** : 4 `unwrap()` dans `auth.rs` (sessions) |
| 9 | **moyen** | cookie de session sans `Secure` : si l'application passe derrière HTTPS, le cookie peut fuir par une requête HTTP | poser `Secure` quand `X-Forwarded-Proto: https` |
| 10 | **corrigé** | dépendances non auditées | `cargo-audit 0.23.0` installé et passé : 3 vulnérabilités, `lopdf` corrigée, `quick-xml` et `bincode` viennent de `typst` (voir la passe du 12) |
| 11 | **corrigé** | scan par règles | semgrep 1.185.0 installé et passé sur 308 fichiers : 21 constats, un seul dans du code de production (corrigé), les autres sont le FFI deja audité, des tests, et trois faux positifs |
| 12 | **corrigé** | la clé d'API (`DOLAPIKEY`) était comparée par `==` | `egal_temps_constant` (ou exclusif octet par octet), avec test unitaire |

---

## Ce que cet audit ne dit pas

- Il ne dit **pas** que le logiciel est sûr : il dit ce qu'un examen manuel de
  deux heures a trouvé, et ce qu'il n'a pas pu couvrir.
- Il ne couvre **pas** la logique métier (une facture validée deux fois, un
  droit contourné par une route oubliée).
- Il ne remplace pas un test d'intrusion sur une instance exposée.
