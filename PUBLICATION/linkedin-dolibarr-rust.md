# Publication — Dolibarr en Rust

Sujet : réécrire le noyau de Dolibarr en Rust. Méthode retenue : **rédiger ici,
publier à la main** — la clé Publora n'est pas sur cette machine, et je ne
pilote pas un compte LinkedIn par scraping.

Tous les chiffres ci-dessous sont **mesurés** sur le dépôt, pas estimés. C'est
l'intérêt du sujet : on peut tout prouver.

## L'angle

Pas « Rust c'est mieux ». L'angle est plus étroit et plus défendable :

> **On peut réécrire un ERP sans trahir ses données.**

Même base, même schéma, mêmes droits, mêmes menus — et pourtant un autre langage,
d'autres bugs, d'autres limites. C'est vérifiable, donc crédible.

## Le calendrier de 30 jours

| Jour | Sujet | Le chiffre qui porte le post |
|---|---|---|
| 1 | Pourquoi réécrire Dolibarr | 1 630 232 lignes de PHP / 410 tables |
| 2 | La contrainte : même base | empreinte de schéma `compatible` |
| 3 | Ce que ça donne en Rust | 41 251 lignes, 22 crates |
| 4 | Les menus | 12 menus lus dans `llx_menu`, pas recopiés |
| 5 | Le bug qui a tout appris | `enabled` est du texte, pas un entier |
| 6 | Installation | une commande, 410 tables |
| 7 | Tester sans risque | `403` sur toute écriture |
| 8 | Les PDF | 3 modèles, 4 pages → 2 |
| 9 | Le SEPA | `pain.001`, IBAN modulo 97 |
| 10 | La comptabilité | balance = zéro sur 3 écritures |
| 11 | La caisse | 3 × 19,99 € à 20 % = 71,96 € |
| 12 | Les récurrences | 31 janvier → 28 février |
| 13 | La paie | aucun taux inventé, et pourquoi |
| 14 | Les traductions | 15 617 clés `fr_FR` |
| 15 | Les greffons | ABI C, pas de recompilation |
| 16 | La licence | GPL conservée, œuvre dérivée |
| 17 | Sauvegarde | structure en clair, données chiffrées |
| 18 | Les tests | 376 tests, un seul fil |
| 19 | Ce qui manque | l'honnêteté comme argument |
| 20 | La fiche facture | 81 actions contre 5 |
| 21 | Extraire ou deviner | la méthode des fiches |
| 22 | Le coût réel | ce que 2,5 % veut dire |
| 23 | Les agents | 12 en parallèle, zéro collision |
| 24 | Le nom | pourquoi pas « DoliRust » |
| 25 | Le modèle d'extension | JSON contre ABI |
| 26 | La question du fork | base partagée, pas guerre |
| 27 | Ce que Dolibarr fait mieux | 30 langues, 20 ans |
| 28 | La démo | 5 minutes pour essayer |
| 29 | Appel aux testeurs | pointez-le sur votre base |
| 30 | Bilan de 30 jours | les chiffres, les trous |

## Post 1 — Pourquoi réécrire Dolibarr

Dolibarr, c'est 1 630 232 lignes de PHP. 4 469 fichiers. Vingt ans de corrections.

Je le réécris en Rust. Pas un fork : la même base de données, à l'octet près.

Ce qui m'intéresse n'est pas le langage. C'est une question simple : est-ce qu'un
ERP peut changer de moteur sans changer de données ?

Réponse courte : oui. 410 tables, mêmes identifiants de droits, mêmes mots de
passe bcrypt. Dolibarr PHP lit la même base sans rien voir. On peut revenir en
arrière à tout moment.

Le reste, c'est du travail : 41 251 lignes contre 1,6 million, soit 2,5 %.

Je vais raconter ce que ça donne, chiffres en main. Y compris ce qui casse.

## Post 2 — La contrainte qui rend le projet possible

La règle que je me suis imposée : **le schéma SQL ne bouge pas**.

Pas une table ajoutée. Pas une colonne. L'empreinte du schéma doit rester
identique à celle de Dolibarr 24.0.1.

Conséquence : chaque fois que je veux stocker quelque chose, je n'ai pas le
droit de créer une table. Il faut trouver où Dolibarr l'aurait mis.

Exemple : « quelle échéance récurrente a déjà été facturée ? »
Dolibarr a `date_last_gen`. Mais y écrire ferait sauter une génération à son
propre planificateur. J'ai donc utilisé une constante : `llx_const`, la table
prévue pour ça.

C'est plus lent à écrire. Mais à la fin, la base reste la base.

## Post 3 — Ce que 2,5 % veut dire

41 251 lignes de Rust contre 1 630 232 de PHP.

Rapport brut : 2,5 %.
Rapport des écrans : 14 %.

Ces deux chiffres racontent la même chose : Rust dit plus par ligne, mais un
écran Dolibarr reste plus profond que le mien. Une fiche facture Dolibarr, c'est
7 114 lignes de PHP et 81 actions. La mienne en a 5.

Je préfère publier les deux chiffres que le flatteur.

## Post 4 — Le bug qui m'a le plus appris

J'ai branché la vraie table de menus de Dolibarr dans mon noyau. 338 lignes.

Le serveur a refusé toutes les connexions. Pas une erreur : une fermeture sèche.

Cause : `llx_menu.enabled` n'est pas un entier. C'est du **texte**, et parfois
du PHP : `isModEnabled("adherent")`. Mon code attendait un nombre. Le pilote
MySQL n'a pas pu convertir. Il a paniqué.

Deuxième leçon dans le même fichier : `enabled` **vide** veut dire « toujours
actif ». Je l'avais lu comme « désactivé ». Un menu disparaissait.

Mon test avec deux lignes bidon validait les deux erreurs.
Les 338 lignes réelles les ont trouvées en trois secondes.

## Post 5 — Tester sur sa propre base

Le vrai argument du projet n'est pas la vitesse. C'est celui-ci :

```
DOLIBARR_LECTURE_SEULE=1
```

Le noyau lit votre base Dolibarr. Il refuse **toute** écriture : 403 sur
n'importe quelle méthode qui n'est pas GET.

Vos comptes fonctionnent (bcrypt, comme Dolibarr). Vos droits, vos entités, vos
devis, vos factures, vos PDF. Vos 410 tables, en lecture.

Vous ne pouvez rien casser. Vous jugez.

C'est la seule démonstration honnête : on ne demande pas de croire, on demande
d'essayer.

## Post 6 — Ce qui manque, et pourquoi je le dis

Une réécriture sérieuse se juge sur ce qu'elle avoue.

Manquent aujourd'hui :
— la comptabilité tenue (lettrage, à-nouveaux, clôture)
— la paie réelle : je n'ai aucun taux, et je refuse d'en inventer
— le point de vente tactile
— les extrafields à l'écran
— 30 langues : j'en ai une
— et 0 module tiers compatible, parce qu'aucun PHP ne tourne

Ce n'est pas de la modestie. C'est que le jour où quelqu'un teste, il tombera
sur exactement ces trous. Autant qu'il les connaisse avant.

## Règles de rédaction, pour la suite

1. **Un chiffre mesuré par post.** Pas « beaucoup », pas « significatif ».
2. **Pas de « révolutionnaire ».** Le projet est technique, pas magique.
3. **Dire les trous.** C'est ce qui distingue un projet d'une annonce.
4. **Pas de tiret cadratin, pas de « il est important de ».** Du texte direct.
5. **Finir par une question concrète**, pas par « et vous, qu'en pensez-vous ? ».

## Ce qu'il reste à faire

- [ ] Publier les 30 posts (manuellement, ou avec une clé Publora)
- [ ] Illustrer 6 posts avec des captures : menus, PDF, explorateur de tables
- [ ] Répondre aux commentaires sous 24 h les trois premières semaines
