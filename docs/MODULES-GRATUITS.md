# Modules gratuits et correctifs

Ce fichier tient l'inventaire de ce qui est **réellement** vérifié, avec sa
source et sa date. Quand une information manque, elle est notée comme manquante
plutôt que complétée au jugé.

Dernière vérification : **3 novembre 2026**, sur `dolistore.com` et
`wiki.dolibarr.org`.

## 1. Les modules gratuits du cœur Dolibarr

Dolibarr 24.0.1 embarque ses modules et ne les vend pas. Ils sont **tous**
recensés dans ce dépôt (`crates/db/src/module.rs`) : 94 entrées, avec leur code
(`modFacture`, `modPropal`…), leur libellé et leur constante d'activation
`MAIN_MODULE_*`.

| | Nombre | Détail |
|---|---|---|
| Modules du cœur recensés | 94 | catalogue complet, activables comme dans Dolibarr |
| Modules qui déclarent un écran (`$this->menu = 1`) | 37 | le reste n'a qu'une page de réglages |
| Modules avec un écran dans ce noyau | 36 | manque Mailman/SPIP (aucune table à lire) |
| Modules avec écriture complète | 8 | prêts, immobilisations, lots, ressources, TVA, adhérents, catégories, dons |

La moitié des modules Dolibarr n'ont pas d'écran métier, même en PHP. « Porter
les 94 » n'a donc pas de sens ; ce qui compte est la couverture des 37 écrans, et
elle est à 36.

## 2. La règle de licence du Dolistore — c'est elle qui nous engage

Texte des conditions d'utilisation du Dolistore, règle n°1, vérifié le
3 novembre 2026 :

> Seuls les modules/extensions diffusés sous une licence approuvée par l'OSI
> peuvent être publiés. De plus, si le module est une œuvre dérivée (utilise du
> code, fonctions ou méthodes, hors API) de Dolibarr (cas de 99 % des extensions
> proposées), il doit être dans une licence compatible avec la licence GPL v3+
> de Dolibarr. **Si aucune licence n'est précisée, en le publiant sur DoliStore,
> vous acceptez que votre module soit publié sous licence GPL v3+.**

Ce que ça signifie pour nous, concrètement :

| Cas | Droit |
|---|---|
| Module gratuit du Dolistore, GPL v3+ | réécriture et distribution possibles, avec attribution de l'auteur |
| Module gratuit, licence non précisée | réputé GPL v3+ par les CGU, même régime |
| Module payant, licence GPL | le **code** reste GPL ; le prix paie la **distribution et le support**. Une réécriture distribuée est donc licite, mais déloyale envers l'auteur — à éviter |
| Module payant, licence commerciale | réécriture **interdite** sans le titulaire des droits |

Conclusion pratique : les modules gratuits sont réécrivables. Les modules payants
ne le sont pas, sauf accord de leur auteur.

## 3. Modules gratuits vérifiés

Liste **partielle et assumée comme telle** : deux fiches ont été lues et
recoupées, pas mille.

| Module | Auteur | Version | Compatibilité | Licence | Source |
|---|---|---|---|---|---|
| **DoliModuleManager** | Nicolas Zaoui | 1.8.1 (17/04/2026) | V14+ / PHP 8.0+ | « GPL v3 or later » (mentions de la fiche) | `dolistore.com/product.php?id=2963` |
| **Metabase in Dolibarr** | — | — | V18 – V22 | gratuit (licence non lue sur la fiche) | `dolistore.com` (liste) |

**DoliModuleManager mérite un mot** : c'est un gestionnaire de paquets pour
modules Dolibarr, avec lecture des tags GitHub et installation depuis un
manifeste `dmm.json`. C'est exactement le problème que ce noyau résout autrement,
avec ses modules déclaratifs (`modules/*.json`). Deux réponses au même besoin,
l'une en PHP dans Dolibarr, l'autre en Rust ici.

> **Ce qui manque encore** : la liste complète des modules gratuits. Elle se lit
> catégorie par catégorie sur `dolistore.com`, elle change chaque mois, et la
> recopier de mémoire produirait une liste fausse. Tant qu'elle n'est pas
> parcourue, ce tableau reste incomplet — et il le dit.

## 4. Les correctifs (« patches »)

Dolibarr corrige ses versions par des commits sur sa branche de maintenance. Ce
noyau n'applique pas ces correctifs : il **réimplémente** le comportement, et
chaque écart trouvé devient un test.

| Mécanisme | Rôle |
|---|---|
| `tools/verifier.ps1` | sept étages : formatage, clippy, tests, empreinte du schéma, chaîne métier, écrans, annuaire |
| `tools/oracle/*.php` | la vraie fonction Dolibarr, conservée comme juge des arrondis |
| `crates/db/tests/` | 19 fichiers d'intégration sur la vraie base |
| 267 tests | unitaires, intégration, rendu PDF, SMTP contre un faux serveur |

Un bogue de Dolibarr qui nous concerne devient **un test rouge**, pas une rustine
posée à la main. On ne recolle pas, on prouve.

## 5. Ce qu'il reste à faire

- [ ] Parcourir les catégories du Dolistore et compléter le tableau §3, avec la
      licence de chaque fiche — la seule façon honnête de le remplir.
- [ ] Tenir une table « fonctionnalité Dolibarr → état dans ce noyau », pour que
      personne ne confonde « écran présent » et « fonction complète ».
- [ ] Publier le noyau sous son propre nom, sans laisser croire qu'il est édité
      par l'équipe Dolibarr (voir `MENTIONS-LEGALES.md`).
