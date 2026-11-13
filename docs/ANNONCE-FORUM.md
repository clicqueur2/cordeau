# Annonce forum — noyau Dolibarr en Rust

> Section suggérée : **forum.dolibarr.org → Développement** (ou « Autres
> projets »). Pas dans le fil d'un module : on ne vend pas un remplacement, on
> propose un noyau qui lit la même base.
>
> Les chiffres ci-dessous sont vérifiables : `tools/verifier.ps1` les produit,
> et `COMPARAISON-MESUREE.md` donne le protocole des temps de réponse.

---

**Sujet : un noyau Dolibarr réécrit en Rust — même base, je cherche des testeurs**

Bonjour,

Je réécris le noyau de Dolibarr en Rust. Pas un fork, pas un remplacement :
**le même schéma de base**, à l'octet près. Dolibarr PHP lit la même base sans
rien voir, et on peut revenir en arrière à tout moment.

C'est un travail d'architecte-maçon, pas d'éditeur : je l'ai écrit pour ma
propre entreprise et je le publie sous GPL-3.0 parce que c'est une œuvre dérivée
de Dolibarr.

**Ce qui marche aujourd'hui**

- base **Dolibarr 24.0.1 utilisée telle quelle** — 487 tables lues, 410 tables
  du cœur intactes (empreinte vérifiée à chaque passe) ; migrations depuis les
  bases 12, 19 et 23 testées
- **298 routes**, **66 écrans** : devis, commandes, factures, avoirs, règlements,
  factures de situation, achats, chantiers, banques, comptabilité, tickets,
  documents, agenda, portail client
- **PDF conformes au droit français** : mentions légales, CGV, RIB, TVA sur
  encaissements, dates françaises
- **Factur-X** : le PDF avec le XML EN 16931 attaché, code d'exonération (VATEX)
  lu dans le dictionnaire de TVA
- fichiers bancaires **SEPA** : `pain.001` et `pain.008`, IBAN vérifiés par la clé
  modulo 97
- **API compatible** `/api/index.php` et serveur **MCP** (26 outils), pour piloter
  l'ERP depuis un agent
- extension sans recompiler : modules déclaratifs (un JSON = un écran)

**Comment c'est vérifié**

**1 057 tests**, `clippy -D warnings` propre, et `tools/verifier.ps1` en **huit
étages** : formatage, analyse statique, tests, empreinte du schéma, chaînes de
migration, chaîne métier (devis → facture → règlement → PDF), dix écrans,
annuaire. Le script sort en erreur au premier échec.

**Les temps de réponse**

Mesurés sur la même machine et **la même base** (copie de production : 932
tiers, 236 factures), modules du cœur seulement, build `release`, PHP 8.3 avec
opcache :

| Page | Dolibarr 24 | Cordeau |
|---|---|---|
| Connexion | 25 ms | 0,5 ms |
| Liste des tiers (932) | 46 ms | 4,4 ms |
| Liste des produits | 33 ms | 4,0 ms |
| Liste des devis | 48 ms | 10 ms |
| Fiche de facture | 28 ms | 17 ms |

Le détail, le protocole et **les limites** sont dans `COMPARAISON-MESUREE.md` :
`php -S` sert une requête à la fois, donc ces chiffres disent la latence d'une
requête, pas le débit d'un serveur de production. Sur les pages où il y a du
travail réel par ligne (une fiche de facture), l'écart tombe à 1,6 ×.

**Ce qui ne marche pas encore**

- réception de factures électroniques : le code existe, il faut une plateforme
  agréée (partenariat)
- connecteurs Stripe, PayPal, LDAP, OAuth : non livrés
- installateur **Windows** seulement
- aucun test de charge : je n'ai pas mesuré vingt utilisateurs simultanés

**Ce que je cherche**

Des testeurs qui ont une **vraie base Dolibarr** et qui veulent essayer en
lisant la même donnée, sans migration. Ce qui m'intéresse, ce sont les écarts :
un écran qui affiche autre chose que Dolibarr, un PDF qui diffère, une base
exotique qui coince.

Si des gens sont intéressés, je publie le dépôt et un mode d'emploi « essayer en
dix minutes ».

Merci.

---

## Captures à joindre au post

Les images sont dans le dépôt, jeu de données **fictif** (aucun nom de client) :

- `docs/captures/01-tableau-de-bord.png`
- `docs/captures/02-devis-liste.png`
- `docs/captures/03-devis-fiche.png`
- `docs/captures/04-factures-liste.png`
- `docs/captures/05-tiers-liste.png`
- `docs/captures/06-chantiers.png`

Ordre conseillé dans le message : tableau de bord, fiche de devis, liste des
devis, liste des factures. Les autres servent de complément.
