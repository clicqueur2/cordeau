# Modules gratuits et correctifs

Ce fichier tient l'inventaire de ce qui est **réellement** disponible, et de la
règle qui s'applique. Rien n'est inventé : quand une information manque, elle est
notée comme manquante.

## 1. Les modules gratuits du cœur Dolibarr

Dolibarr 24.0.1 embarque ses modules et ne les vend pas. Ils sont **tous**
recensés dans ce dépôt (`crates/db/src/module.rs`) : 94 entrées, avec leur code
(`modFacture`, `modPropal`…), leur libellé et leur constante d'activation
`MAIN_MODULE_*`.

État réel, mesuré le 3 novembre :

| | Nombre | Détail |
|---|---|---|
| Modules du cœur recensés | 94 | catalogue complet, activables comme dans Dolibarr |
| Modules qui déclarent un écran (`$this->menu = 1`) | 37 | le reste n'a qu'une page de réglages |
| Modules avec un écran dans ce noyau | 36 | manque Mailman/SPIP (aucune table à lire) |
| Modules avec écriture complète | 8 | prêts, immobilisations, lots, ressources, TVA, adhérents, catégories, dons |

Autrement dit : **la moitié des modules Dolibarr n'ont pas d'écran métier**, même
en PHP. « Porter les 94 modules » n'a donc pas de sens ; ce qui compte est la
couverture des 37 écrans, et elle est à 36.

## 2. Les modules externes gratuits (Dolistore)

Le Dolistore publie des modules payants **et** gratuits. Ce dépôt ne peut pas les
embarquer tels quels : ce sont des modules **PHP**, et ce noyau ne fait pas
tourner de PHP.

Deux voies, et une seule est propre :

1. **Réécrire le module en Rust** (crate + `plugin.rs`) — coûteux, un par un.
2. **Le décrire en déclaratif** (`modules/*.json`) — un fichier suffit, sans
   recompiler. C'est ce qui existe aujourd'hui : cinq modules ainsi chargés
   (modèles d'e-mails, moyens de paiement, pays, profils expéditeurs,
   propositions fournisseurs).

**Règle de licence** : un module du Dolistore est sous GPL (la plupart) ou sous
licence commerciale. Un module GPL peut être réécrit et distribué avec ce noyau,
en conservant l'attribution de son auteur. Un module commercial **ne peut pas**
être réécrit et diffusé : seule une réécriture faite par le titulaire des droits
le peut.

> **Ce qui manque, et qui n'est pas inventable** : la liste nominative des modules
> gratuits du Dolistore, avec leur licence et leur auteur. Elle se lit sur
> <https://www.dolistore.com> — elle change régulièrement, et la recopier ici
> sans vérification produirait une liste fausse dans les six mois.

## 3. Les correctifs (« patches »)

Dolibarr corrige ses versions par des commits sur sa branche de maintenance. Ce
noyau n'applique pas ces correctifs : il **réimplémente** le comportement, et
chaque écart trouvé devient un test.

Ce qui existe ici, en remplacement :

| Mécanisme | Rôle |
|---|---|
| `tools/verifier.ps1` | sept étages : formatage, clippy, tests, empreinte du schéma, chaîne métier, écrans, annuaire |
| `tools/oracle/*.php` | la vraie fonction Dolibarr, conservée comme juge des arrondis |
| `crates/db/tests/` | 19 fichiers de tests d'intégration sur la vraie base |
| 267 tests | unitaires, intégration, rendu PDF, SMTP contre un faux serveur |

Un bogue de Dolibarr qui nous concerne devient donc **un test rouge**, pas une
rustine appliquée à la main. C'est la différence entre patcher un logiciel et le
réécrire : on ne recolle pas, on prouve.

## 4. Ce qu'il reste à faire pour être honnête sur ce sujet

- [ ] Vérifier la licence de chaque module externe réécrit, et l'inscrire ici.
- [ ] Tenir une table « fonctionnalité Dolibarr → état dans ce noyau », pour que
      personne ne croie qu'un écran présent vaut une fonction complète.
- [ ] Publier le noyau sous son propre nom, sans laisser croire qu'il est édité
      par l'équipe Dolibarr (voir `MENTIONS-LEGALES.md`).
