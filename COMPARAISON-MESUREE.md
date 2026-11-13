# Comparaison mesurée — Dolibarr 24.0.1 (PHP) et Cordeau (Rust)

> Mesures faites le 13 novembre 2026, sur la même machine, contre la **même
> base** : une copie de la production de Pierre Angulaire (932 tiers,
> 236 factures, 487 tables).
>
> Ce document ne vend rien : il dit ce qui a été mesuré, comment, et ce que la
> mesure ne prouve pas.

---

## 1. Protocole

| Élément | Valeur |
|---|---|
| Machine | Windows 11, même disque, même MariaDB 11.4.5 (port 3307) |
| Base | `dolibarr_ref` — copie de la production, **identique pour les deux** |
| Dolibarr | 24.0.1, source officielle, **modules du cœur seulement** (société, produit, devis, commande, facture, projet, agenda, banque, comptabilité, taxes, service, fournisseur, expédition, contrat) — **aucun module tiers** : les 215 modules installés en production ont été désactivés |
| PHP | 8.3.33, opcache actif, `php -S` (un seul worker) |
| Cordeau | build `release` (`cargo build --release`), un fil par requête |
| Mesure | nouvelle connexion TCP par requête, 2 requêtes de chauffe, 12 mesures, médiane et 95ᵉ centile, du début de connexion à la fin du corps |

Deux précautions qui comptent :

- **la base est la même** : ce ne sont pas deux jeux d'essai, c'est la vraie
  donnée de l'entreprise, lue par les deux applications ;
- **aucun module tiers** : comparer Cordeau à un Dolibarr chargé de modules
  ajoutés ne mesurerait rien — ni la vitesse de Dolibarr, ni un écart honnête.

---

## 2. Résultats

| Page | Dolibarr 24 (PHP) | Cordeau (Rust) | Facteur |
|---|---|---|---|
| Connexion (formulaire) | 25,0 ms | **0,5 ms** | **50 ×** |
| Liste des tiers (932 fiches) | 45,8 ms | **4,4 ms** | **10,3 ×** |
| Liste des produits | 32,6 ms | **4,0 ms** | **8,1 ×** |
| Liste des devis | 47,8 ms | **10,0 ms** | **4,8 ×** |
| Liste des factures (236) | 27,2 ms | **18,6 ms** | 1,5 × |
| Fiche de facture | 28,3 ms | **17,4 ms** | 1,6 × |

Les 95ᵉ centiles suivent : Dolibarr entre 45 et 66 ms sur les listes, Cordeau
entre 4,8 et 41,8 ms. L'écart le plus faible est sur la fiche de facture, où les
deux applications font le plus de travail par ligne.

**Page d'accueil et PDF : non comparés.** L'accueil Dolibarr répond 302 (il
redirige vers son tableau de bord) et sa génération de PDF passe par une
redirection vers un fichier : mesurer cela demanderait un protocole séparé, et
je ne publie pas un chiffre que je n'ai pas vérifié.

---

## 3. Ce que ces chiffres ne disent pas

- **`php -S` sert une requête à la fois.** Une production php-fpm avec huit
  workers encaisse beaucoup plus de requêtes simultanées. La **latence** d'une
  requête seule est du même ordre, le **débit** ne l'est pas : ne pas publier de
  « x fois plus rapide » sans dire cela.
- Un serveur Rust répond en **0,5 ms** sur une page de connexion parce qu'il n'y
  a presque rien à faire ; sur une fiche de facture, l'écart tombe à 1,6 ×, et
  c'est là que se joue le travail réel.
- Ces mesures sont prises **à chaud** (caches amorcés). À froid, les deux
  applications paient leur démarrage.
- Elles ne mesurent **pas** la consommation mémoire, ni le comportement à vingt
  utilisateurs simultanés. C'est le prochain test à faire, et il demande une
  vraie campagne.

---

## 4. Compatibilité de base : ce qui est prouvé

| Cas | Statut |
|---|---|
| Base **Dolibarr 24.0.1** existante | **utilisée telle quelle** : c'est la base de cette comparaison (487 tables, 932 tiers, 236 factures). Dolibarr PHP lit la même base sans rien voir |
| Base **12 → 24** | migrée et vérifiée automatiquement (307 → 400 tables) |
| Base **19 → 24** | migrée et vérifiée (372 → 406 tables) |
| Base **23 → 24** | migrée et vérifiée (399 → 407 tables) |
| Retour en arrière | possible : Dolibarr PHP relit la base après passage de Cordeau (même schéma, aucune colonne ajoutée au cœur) |

Le schéma du cœur est **inchangé** : 410 tables, empreinte vérifiée à chaque
passe de `tools/verifier.ps1`. Les tables propres aux fonctions qui n'existent
pas chez Dolibarr (marges, journal de caisse) sont préfixées `llx_<module>_`.

---

## 5. Installation

Oui, il y a un installateur : **`tools/installer.ps1`** + `INSTALLER.md`.

Il a été essayé **de zéro** sur une base vierge (`cordeau_test`) : 410 tables
créées, 251 pays, 16 moyens de paiement, compte administrateur créé, preset
« BTP » appliqué (20 modules), page d'accueil en 200. Commandes disponibles :
`cordeau user add`, `cordeau preset lister|appliquer`.

À savoir avant de le proposer : il est écrit pour **Windows + MariaDB locale**.
Un portage Linux est du travail de configuration, pas de code — mais il n'est pas
fait, et il faut le dire.

---

## 6. Ce qu'il manque pour poster sur le forum Dolibarr

| Point | État |
|---|---|
| Un dépôt public ou une archive à télécharger | dépôt **privé** aujourd'hui. Sans un lien accessible, une annonce ne donne rien à essayer |
| Une licence claire | **GPL-3.0** (œuvre dérivée de Dolibarr), fichier `LICENSE` + section d'attribution dans le README |
| Des captures d'écran | à faire : 4 ou 5 écrans réels avec les données de l'entreprise (pas des maquettes) |
| Une démonstration en ligne | pas d'instance publique. Un `docker compose` ou un lien de démo vaut mieux qu'un discours |
| Un mode d'emploi pour tester | `INSTALLER.md` existe ; il faut une page « essayer en 10 minutes » |
| La liste honnête de ce qui **ne** marche pas | à écrire (voir §8) — c'est ce qui fait la crédibilité d'une annonce technique |
| La règle du forum | poster en **Développement** ou « Autres projets », pas dans le fil d'un module ; ne pas se présenter comme un remplaçant de Dolibarr |

---

## 7. Ce qu'il ne faut pas écrire

L'ancienne version de `ANNONCE-FORUM.md` annonçait **267 tests** (il y en a
1 057), « 36 des 37 modules » (formulation floue : il y a 66 écrans et 298
routes) et « 2 à 9 ms par page » sans dire sur quoi. Une annonce qui exagère se
fait démolir en trois réponses, et le projet perd sa crédibilité avant d'avoir
été essayé.

Ce qui est vrai et vérifiable :

- **1 057 tests**, `clippy -D warnings` propre, et un script de vérification en
  **huit étages** (formatage, analyse statique, tests, empreinte du schéma,
  chaînes de migration, chaîne métier devis → facture → règlement → PDF,
  10 écrans, annuaire) ;
- **22 crates** Rust, **298 routes**, **66 écrans**, **26 outils MCP** ;
- **487 tables** Dolibarr lues, **410 tables du cœur** intactes ;
- PDF conformes au droit français (mentions, CGV, RIB, TVA sur encaissements,
  dates françaises), **factures de situation**, **Factur-X** (XML EN 16931
  attaché au PDF, vérifié) ;
- portail client, API compatible `/api/index.php`, serveur MCP.

---

## 8. Ce qui ne marche pas encore (à écrire dans l'annonce)

- **Réception de factures électroniques** : le code existe (`einvoicing`) mais il
  faut une **plateforme agréée** — donc un partenariat, pas une ligne de Rust.
- **Connecteurs externes** (Stripe, PayPal, LDAP, OAuth) : non livrés, ils
  demandent des clés et un annuaire.
- **Installateur Windows uniquement**.
- **Pas de test de charge** : personne n'a mesuré Cordeau à vingt utilisateurs
  simultanés.
- **Interface** : 66 écrans, pas les 400 de Dolibarr. Les fonctions rares (GRH
  avancée, MRP fin, multi-entrepôts complexe) ne sont pas toutes là.
