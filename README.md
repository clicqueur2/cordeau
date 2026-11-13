# Cordeau

**Dolibarr, même base, autre moteur.** Un noyau d'ERP écrit en Rust qui lit la
base Dolibarr telle quelle — Dolibarr PHP continue de la lire sans rien voir, et
on revient en arrière quand on veut.

![Tableau de bord](docs/captures/01-tableau-de-bord.png)

---

## Le problème que ça règle

Dolibarr est un bon logiciel avec un défaut de fabrication : chaque page est
reconstruite à chaque clic, en PHP, avec des dizaines de requêtes SQL.

Cordeau garde **la même base de données** — le même schéma, à l'octet près — et
reconstruit la page autrement. Rien à migrer, rien à importer : on pointe
Cordeau sur la base existante, il lit. Et si ça ne plaît pas, Dolibarr PHP la
relit comme avant.

## Ce que ça donne, mesuré

Même machine, **même base** (copie de production : 932 tiers, 236 factures),
modules du cœur seulement, build `release`, PHP 8.3 avec opcache :

| Page | Dolibarr 24 | Cordeau |
|---|---|---|
| Connexion | 25 ms | **0,5 ms** |
| Liste des tiers (932 fiches) | 46 ms | **4,4 ms** |
| Liste des produits | 33 ms | **4,0 ms** |
| Liste des devis | 48 ms | **10 ms** |
| Fiche de facture | 28 ms | **17 ms** |

![Comparaison des temps de réponse](docs/captures/graphique-temps.svg)

Le protocole complet, les 95ᵉ centiles et **les limites** de cette mesure sont
dans [COMPARAISON-MESUREE.md](docs/COMPARAISON-MESUREE.md). En résumé : sur une fiche
de facture — là où il y a du vrai travail par ligne — l'écart est de 1,6 ×, pas
de 10 ×. `php -S` sert une requête à la fois, donc ces chiffres disent la
latence, pas le débit à vingt utilisateurs.

## Ce que ça fait

| Domaine | État |
|---|---|
| Tiers, contacts, doublons | liste, fiche, création, fusion, import/export CSV |
| Devis, commandes, contrats | lignes, remises, TVA par taux, PDF, envoi par mail |
| Factures, avoirs, situations | validation, numérotation légale, acomptes déduits, PDF |
| Règlements, lettrage | affectation, solde, lettrage comptable, fichiers SEPA `pain.001` / `pain.008` |
| Achats | commandes fournisseurs, réceptions, factures d'achat |
| Chantiers | carnet de commandes, marge par affaire, Gantt |
| Comptabilité | grand livre, balance, FEC, à-nouveaux, TVA sur encaissements |
| Banques | comptes, relevés, import CSV, rapprochement |
| Caisse, portail client, agenda, tickets, documents | oui |
| **Factur-X** | XML EN 16931 attaché au PDF, codes d'exonération (VATEX) lus dans le dictionnaire |
| **API** | compatible Dolibarr sous `/api/index.php` (devis, factures, commandes, tiers, contacts, projets, documents) |
| **MCP** | 26 outils pour piloter l'ERP depuis un agent |

**298 routes, 66 écrans, 22 crates.** Les écrans qui manquent par rapport à
Dolibarr sont listés plus bas, sans détour.

## La même base, vraiment

| Cas | Statut |
|---|---|
| Base Dolibarr **24.0.1** existante | **utilisée telle quelle** — c'est la base de toutes les mesures ci-dessus |
| Base **12 → 24** | migrée et vérifiée (307 → 400 tables) |
| Base **19 → 24** | migrée et vérifiée (372 → 406) |
| Base **23 → 24** | migrée et vérifiée (399 → 407) |
| Retour en arrière | Dolibarr PHP relit la base après passage de Cordeau |

Le schéma du cœur est **inchangé** : 410 tables, empreinte vérifiée à chaque
passe de vérification. Les tables propres aux fonctions qui n'existent pas chez
Dolibarr (marges, journal de caisse) sont préfixées `llx_<module>_`.

## Essayer en cinq minutes

```
cargo build --release
.\tools\installer.ps1 -Base cordeau_test        # base vierge : 410 tables, admin, preset BTP
.\target\release\cordeau-web.exe                # http://127.0.0.1:8090
```

Sur une base existante, il n'y a rien à installer :

```
$env:DOLIBARR_DSN = "mysql://user:motdepasse@127.0.0.1:3306/dolibarr"
cargo run -p web --release
```

Détail : [INSTALLER.md](INSTALLER.md). ⚠️ **Windows uniquement** pour
l'installateur.

## Le portail client

![Fiche de devis](docs/captures/03-devis-fiche.png)

Chaque client a son accès (`/public/portail`) : ses devis, ses factures, ses
PDF — et rien d'autre. La session est distincte de celle du bureau, et chaque
page vérifie que la pièce appartient bien au client avant de l'afficher.

## Comment c'est vérifié

**1 057 tests**, `clippy -D warnings` propre, et un script qui refuse de
continuer au premier échec :

```
.\tools\verifier.ps1
```

Huit étages : formatage, analyse statique, tests, **empreinte du schéma**
Dolibarr, chaînes de migration (12, 19, 23 → 24), chaîne métier complète
(devis → facture → règlement → PDF), dix écrans, annuaire.

## Ce qui ne marche pas encore

- **réception de factures électroniques** : le code existe, il faut une
  plateforme agréée — un partenariat, pas une ligne de Rust ;
- **connecteurs externes** (Stripe, PayPal, LDAP, OAuth) : non livrés ;
- **installateur Windows seulement** ;
- **pas de test de charge** : personne n'a mesuré vingt utilisateurs simultanés ;
- **66 écrans**, pas les 400 de Dolibarr : les fonctions rares (GRH avancée, MRP
  fin, multi-entrepôts complexe) ne sont pas toutes là.

## Les captures

| | |
|---|---|
| ![Devis](docs/captures/02-devis-liste.png) | ![Factures](docs/captures/04-factures-liste.png) |
| ![Tiers](docs/captures/05-tiers-liste.png) | ![Chantiers](docs/captures/06-chantiers.png) |

Jeu de démonstration **fictif** : aucun nom de client réel n'apparaît dans ce
dépôt.

## Documents

| Fichier | Contenu |
|---|---|
| [COMPARAISON-MESUREE.md](docs/COMPARAISON-MESUREE.md) | la comparaison de vitesse, protocole et limites |
| [GUIDE-UTILISATION.md](docs/GUIDE-UTILISATION.md) | le mode d'emploi écran par écran |
| [INSTALLER.md](INSTALLER.md) | installation de zéro |
| [REFERENCE-COMMANDES.md](docs/REFERENCE-COMMANDES.md) | la ligne de commande, commande par commande |
| [MISE-A-JOUR.md](docs/MISE-A-JOUR.md) | monter une base d'une version à l'autre |
| [CE-QUE-REECRIRE-APPREND.md](docs/CE-QUE-REECRIRE-APPREND.md) | ce qu'une deuxième implémentation révèle du schéma Dolibarr |
| [ANNONCE-FORUM.md](docs/ANNONCE-FORUM.md) | le texte de l'annonce au forum, prêt à copier |
| [AUDIT-SECURITE.md](docs/AUDIT-SECURITE.md) | l'audit, ses correctifs et ce qui reste ouvert |
| [DEVELOPPEMENT.md](docs/DEVELOPPEMENT.md) | règles de code, schéma, tests |

## Un projet de test, offert

Cordeau est né d'une lecture du forum Dolibarr : la lenteur de certains écrans
sur une base qui grossit, et les zones grises du schéma quand on veut brancher
quelque chose à côté. C'est un **projet de test** — il pourrait devenir une
« version 30 », ou rester une curiosité.

**Il est offert à qui veut le reprendre.** La licence est GPL-3.0 comme
Dolibarr, puisque c'est une œuvre dérivée. Ce qui compte, c'est que ça serve : à
Dolibarr, à un développeur qui cherche une base pour expérimenter, ou à un
artisan qui veut des écrans qui répondent.

Ce n'est pas mon métier — je suis architecte et maçon. En revanche, c'est testé :
**1 057 tests**, huit étages de vérification, et une empreinte du schéma Dolibarr
contrôlée à chaque passe.

## Licence et attribution

**GPL-3.0-or-later** — voir [LICENSE](LICENSE).

Cordeau est une **œuvre dérivée de Dolibarr ERP & CRM 24.0.1** (GPL-3.0-or-later,
<https://www.dolibarr.org>). Le schéma de base, les dictionnaires et les
conventions métier viennent de Dolibarr. Le code Rust est original : aucune ligne
PHP de Dolibarr n'est recopiée — les deux seuls fichiers `.php` du dépôt sont nos
harnais d'oracle, qui chargent les sources officielles à l'exécution pour
comparer les résultats.
