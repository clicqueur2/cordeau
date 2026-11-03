# Annonce forum — noyau Dolibarr en Rust

Suggestion de section : **forum.dolibarr.org → Développement** (ou « Autres
projets »). Le message ci-dessous est prêt à copier-coller.

---

**Sujet : un noyau Dolibarr réécrit en Rust — je cherche des testeurs**

Bonjour,

Je réécris le noyau de Dolibarr en Rust. Pas un fork, pas un remplacement :
**le même schéma de base**, à l'octet près. Dolibarr PHP lit la même base sans
rien voir, et on peut revenir en arrière à tout moment.

**Ce qui marche aujourd'hui**

- schéma 24.0.1 identique — 410 tables, empreinte vérifiée automatiquement
- 36 des 37 modules qui ont un écran dans Dolibarr (le 37e, Mailman/SPIP, n'a
  aucune table)
- devis, commandes, factures, avoirs, règlements, achats, chantiers, banques,
  comptabilité, tickets, documents, agenda
- PDF conformes au droit français : mentions légales, CGV, RIB, TVA sur
  encaissements, dates françaises, trois modèles dont deux gabarits distincts
- factures de situation (jalons % du devis), pas seulement des acomptes
- envoi d'e-mails : SMTP direct avec TLS, pièces jointes, campagnes avec cible
- fichiers bancaires SEPA : `pain.001` (virement) et `pain.008` (prélèvement),
  IBAN vérifiés par la clé modulo 97
- un explorateur qui ouvre **les 410 tables** en lecture
- quatre moteurs testés : explosion de nomenclatures, multi-devises, caisse
  (TVA par ligne, clôture, écart), application d'un barème de paie
- extension sans recompiler : modules déclaratifs (un fichier JSON = un écran)
  et greffons en ABI C

**Comment c'est vérifié**

267 tests, `clippy -D warnings`, et un script de vérification en sept étages :
formatage, analyse statique, tests, empreinte du schéma, chaîne métier
(devis → facture → règlement → PDF), écrans, annuaire. La latence mesurée d'une
page est de 2 à 9 ms sur la base de test.

**Ce qui ne marche pas, et qu'il faut savoir avant de tester**

- **aucun PHP n'est exécuté.** Un module du Dolistore en PHP ne tournera pas
  ici. Il faut le décrire en JSON (les écrans) ou l'écrire en Rust (la logique).
- c'est **2 % du volume** de Dolibarr : 32 000 lignes contre 1 630 000. Les
  écrans existent, la profondeur d'un écran Dolibarr n'y est pas.
- **la comptabilité n'est pas tenue** : le grand livre est dérivé des documents,
  rien n'est écrit dans `llx_accounting_bookkeeping`.
- **la paie n'a aucun taux.** Le moteur applique un barème qu'on lui fournit ;
  il n'en contient aucun, volontairement.
- l'interface est du HTML rendu côté serveur, sans JavaScript. Pas d'écran
  tactile de caisse pour l'instant.

**Licence**

GPL-3.0-or-later, comme Dolibarr. C'est une œuvre dérivée et c'est écrit dans le
dépôt : le schéma, les constantes, les identifiants de droits et les libellés
d'écrans viennent de Dolibarr. Le code source est disponible.

**Ce que je cherche**

Des testeurs avec **une copie de leur vraie base**, qui me disent ce qui casse.
Pas besoin de savoir coder : lancer le binaire, se connecter, essayer ses
propres écrans, et me raconter.

Ce qui m'intéresse le plus :

1. les écrans que vous utilisez tous les jours et qui manquent ou sont faux ;
2. les PDF : un devis qui sort mal, c'est le retour le plus utile ;
3. les bases atypiques — beaucoup d'entités, gros volumes, extrafields.

**Pour tester (5 minutes)**

```
git clone <dépôt>
# base de test, jamais la production
mysql -u root -p -e "CREATE DATABASE dolibarr_test"
mysql -u root -p dolibarr_test < schema-24.0.1.sql
cargo run -p web
# http://127.0.0.1:8090/installation  ← l'état de l'installation
```

**Pourquoi ne pas plutôt corriger Dolibarr ?**

Parce que les bugs de PHP ne sont pas des bugs de Dolibarr : ce sont les limites
du langage sur lesquelles le projet bute depuis quinze ans — typage faible,
`max_execution_time`, mémoire, absence de comptabilité à l'octet. Rust ne les
supprime pas tous, mais il en supprime trois : les erreurs de type à l'exécution,
les arrondis flottants (tout est en décimal), et le coût d'un processus par
requête.

Je ne prétends pas faire mieux que l'équipe Dolibarr. Je prétends qu'un noyau
différent, sur **la même base**, est un chemin possible — et je préfère le
montrer que l'affirmer.

Merci à ceux qui voudront essayer. Je réponds à tout, y compris aux « ça ne
marche pas chez moi ».

---

## Notes de publication

- Ne pas poster avant d'avoir **un dépôt public** : l'annonce sans code est une
  promesse, pas un projet.
- Prévoir deux ou trois captures d'écran (facture, PDF, explorateur de tables).
- Éviter le mot « remplacement » : c'est une **compatibilité de base**, pas une
  reprise du projet.
- Répondre vite les trois premiers jours : c'est là que se joue la crédibilité.
