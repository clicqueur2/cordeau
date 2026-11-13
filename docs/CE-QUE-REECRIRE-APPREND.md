# Ce que réécrire Dolibarr apprend sur Dolibarr

> Une deuxième implémentation est le meilleur test d'un schéma : elle oblige à
> écrire noir sur blanc tout ce que la première faisait « parce que c'était
> comme ça ». Voici ce que le portage a révélé, avec, pour chaque point, ce
> qu'on observe, où c'est dans Dolibarr, et comment le reproduire.

Ce document ne dit pas « Dolibarr est mal fait ». Il dit : **voilà ce qui n'est
écrit nulle part et que deux implémentations interprètent différemment.** Chaque
point est un endroit où un correctif — ou trois lignes de documentation —
ferait gagner du temps à tout le monde.

---

## 1. Un statut `2` veut dire trois choses différentes

**Observé** : dans `llx_facture`, `fk_statut = 2` veut dire « payée ». Dans
`llx_propal`, le même `2` veut dire « signée ». Dans `llx_commande`, encore `2`
veut dire « livrée ».

**Conséquence** : un affichage qui ne sait pas de quel document il parle écrit
« payée » sur un devis signé. C'est exactement ce que faisait la première
version de Cordeau, et ça se voit sur une capture d'écran avant même de se voir
dans les tests.

**Reproduction** : créer un devis, le valider, le clôturer en statut 2 (signé) ;
afficher son état avec la logique des factures → « payée ».

**Ce qui aiderait** : une table — ou un paragraphe dans la documentation du
schéma — qui donne, pour chaque table de pièce, la signification de chaque
valeur de statut. Les développeurs de modules tiers se trompent dessus aussi.

---

## 2. Un acompte est lié au devis, mais il ne le facture pas

**Observé** : une facture d'acompte (`llx_facture.type = 3`) porte le lien
`llx_element_element` vers le devis, comme une facture finale. Ce même lien sert
à deux choses : « ce devis a reçu un acompte » et « ce devis est facturé ».

**Conséquence** : un contrôle « ce devis a-t-il déjà été facturé ? » qui lit ce
lien refuse de facturer **dès qu'un acompte existe**. La facture finale ne peut
plus jamais sortir, et l'acompte n'est pas déduit — le client paie deux fois.

**Reproduction** : un devis de 1 000 €, un acompte de 30 % lié au devis, puis
une conversion devis → facture en suivant le lien `propal → facture`.

**Ce qui aiderait** : distinguer le type de la pièce au moment de lire le lien
(`f.type <> 3`), et documenter que l'acompte se déduit de la facture finale,
taux de TVA par taux de TVA.

---

## 3. Le numéro légal : à la création ou à la validation ?

**Observé** : deux politiques coexistent. Un brouillon peut porter
`(PROV<rowid>)` et recevoir son `FA2611-0007` à la validation — c'est ce
qu'affiche Dolibarr — mais rien n'empêche une implémentation de consommer un
numéro légal dès la création du brouillon.

**Conséquence** : un brouillon supprimé laisse un **trou définitif** dans la
suite légale, et rien ne le signale. Sur une comptabilité, un trou de
numérotation est une question à laquelle il faut répondre au contrôle.

**Reproduction** : créer un brouillon, l'effacer, puis créer la facture
suivante ; comparer les deux numéros.

**Ce qui aiderait** : écrire la règle. « Le numéro est attribué à la validation,
sous verrou, dans l'ordre chronologique » — et une contrainte ou un test qui le
garantisse.

---

## 4. Le dictionnaire des unités peut être vidé, et Dolibarr imprime l'erreur

**Observé** : `llx_c_units` porte trois colonnes utiles — `code`, `label`,
`short_label`. Un envoi de formulaire reconstruit à la main sur
`admin/dict.php?id=37` a **vidé** `label` et `short_label` des lignes 1 (pièce)
et 2 (forfait), sans message d'erreur.

**Conséquence** : chaque PDF de facture imprime ensuite
**`ErrorBadValueForParamNotAString`** dans la colonne « Unité ». Dix pages au
lieu de quatre, tableau poussé hors page, client qui appelle. La cause est dans
le cœur : `getLabelOfUnit()` appelle `Translate::trans()` sur `short_label` ;
quand la valeur est `NULL`, `trans()` reçoit une clé qui n'est pas une chaîne et
retourne ce texte d'erreur, qui est ensuite **imprimé tel quel**.

**Reproduction** : mettre `short_label = NULL` sur une unité utilisée, puis
régénérer un PDF qui s'en sert.

**Ce qui aiderait** : deux gardes, une ligne chacune — refuser un dictionnaire
sans libellé à l'écriture, et rendre une chaîne vide plutôt qu'un message
d'erreur à l'affichage. Nous avons dû réparer les deux lignes en SQL, en
production, un soir.

---

## 5. Le code d'exonération de TVA existe, mais il n'est pas rempli

**Observé** : `llx_c_tva.einvoice_vatex` existe depuis la 23.0 — la colonne qui
porte le code BT-120 de Factur-X. Sur une base réelle de production, **aucun
taux ne la porte**.

**Conséquence** : une ligne à 0 % part sans code d'exonération. Les plateformes
agréées la refusent à l'envoi — et on ne le découvre qu'au moment de la
réforme, c'est-à-dire au pire moment.

**Reproduction** : générer un Factur-X pour une facture contenant une ligne à
0 %, lire le XML : pas de `ExemptionReasonCode`.

**Ce qui aiderait** : semer les codes VATEX des quelques taux exonérés courants
(autoliquidation, franchise en base, travaux sur existant de plus de deux ans),
et un contrôle qui refuse d'émettre un XML muet.

---

## Ce que ce document n'est pas

Ce n'est pas une liste de reproches : ces cinq points sont des **interprétations
possibles**, pas des fautes. Chacun d'eux a coûté du temps pendant le portage ;
c'est le seul titre de ce document.

Si un mainteneur de Dolibarr veut les reproduire, les harnais sont dans le dépôt
(`tools/`), et la base de test se crée en une commande.
