# Nom du noyau : pourquoi pas « DoliRust »

Question posée, réponse directe : **« DoliRust » est le plus mauvais des choix
possibles**, pas le meilleur.

## Pourquoi

1. **« Doli » n'est pas un mot libre.** L'association Dolibarr l'a construit
   comme préfixe de sa famille de produits : Dolibarr, DoliStore, DoliWamp,
   DoliCloud, DoliModuleManager. Un projet tiers qui s'appelle « DoliRust »
   se présente comme un produit de cette famille. C'est exactement ce qu'un
   juge appelle un risque de confusion.
2. **La GPL ne donne aucun droit sur le nom.** Elle donne le droit d'utiliser,
   modifier et redistribuer **le code**. Elle ne dit rien des marques. On peut
   donc réécrire Dolibarr légalement et s'appeler DoliRust illégalement.
3. **Chercher « DoliRust » sur un moteur renvoie Dolibarr.** Un nom qui ramène
   au projet d'origine n'est pas un nom, c'est une redirection.

Ce qui est permis, en revanche, c'est l'usage **descriptif** :

> « Cordeau — noyau ERP en Rust, compatible avec la base de données
> Dolibarr 24.0.1 »

Là, on dit ce que le logiciel fait, sans se faire passer pour un autre.

## Les critères d'un bon nom

| Critère | Pourquoi |
|---|---|
| Prononçable au téléphone | un nom qu'on doit épeler trois fois ne se vend pas |
| Libre en `.fr` **et** `.com` | à vérifier avant de s'attacher |
| Aucun résultat sur le mot + « ERP » | sinon on hérite du référencement de quelqu'un d'autre |
| Pas de préfixe « Doli », « Doli », « Bar » | confusion avec l'existant |
| Un mot du métier, pas de l'informatique | le client est artisan, pas développeur |
| Une syllabe d'attaque dure | se retient mieux qu'un mot mou |

## Huit candidats, tous dans le vocabulaire du bâtiment

| Nom | D'où il vient | Ce qu'il évoque pour un artisan |
|---|---|---|
| **Cordeau** | la ficelle tendue entre deux piquets qui donne l'alignement | la référence, la ligne juste — et « tirer le cordeau », c'est commencer un ouvrage |
| **Aplomb** | la verticalité vérifiée | « d'aplomb » : juste, solide |
| **Bauge** | la terre crue mêlée de paille | le matériau qui tient debout sans ciment |
| **Giron** | la largeur de la marche d'escalier | la mesure juste, celle qui rend l'ouvrage praticable |
| **Moellon** | la pierre de maçonnerie brute, avant taille | la matière première, avant le logiciel |
| **Tallus** | le talus, le fruit d'un mur | la tenue, l'assise |
| **Niveau** | l'instrument de mesure | mais trop générique pour être trouvable |
| **Trait** | la ligne tracée à la craie sur la pierre | le tracé de l'architecte et du tailleur |

## Mon avis

**Cordeau.** Trois raisons :

1. il dit ce que fait un ERP — *tirer une ligne et s'y tenir* ;
2. il est déjà dans la bouche d'un maçon (aucune explication à donner) ;
3. il ne ressemble à rien d'autre dans le logiciel libre français.

Et si le nom est pris : **Giron**, pour la même raison — la mesure qui rend
l'ouvrage utilisable.

## Ce qu'il faut faire avant de s'attacher à un nom

- [ ] Vérifier la disponibilité du domaine `.fr` et `.com` chez un registrar.
- [ ] Chercher le mot + « logiciel » et mot + « ERP » : si un concurrent sort,
      changer tout de suite.
- [ ] Vérifier l'absence de marque déposée à l'INPI (base gratuite, recherche par
      classe 9 « logiciels » et 42 « services informatiques »).
- [ ] Réserver les comptes GitHub et le nom sur les registres de paquets avant
      toute publication.

> Tant que ces quatre cases ne sont pas cochées, ce document propose des noms,
> il n'en choisit aucun. Un nom non vérifié est un nom qu'on perdra.
