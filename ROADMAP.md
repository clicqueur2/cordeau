# Feuille de route — Cordeau

> Décidée le **6 novembre 2026**. Ce fichier est la référence : il dit dans quel
> ordre les chantiers se font, et pourquoi celui-là avant celui-là.

## Où on en est, mesuré

| | |
|---|---|
| Modules Dolibarr portés (couche base) | **36 / 73** |
| Routes | 239 |
| Fichiers d'écran | 60 |
| Modules `db` | 84 |
| Tests | **888**, 0 échec |
| `main.rs` / `pages.rs` | 242 / 162 lignes (contre 5 196 / 2 921) |

Le corps general tient debout : tiers, produits, devis, commandes, factures,
achats, stock, comptabilite, banque, projets, RH, et une vingtaine de modules
secondaires. Ce qui manque est liste plus bas, avec un ordre.

## Les quatre chantiers, dans l'ordre

### 1. Finir le corps general

**Pourquoi d'abord.** Un preset metier n'est credible que s'il active des
modules qui existent. Livrer un preset « BTP » dont la moitie des cases ne fait
rien serait pire que pas de preset du tout.

Restent, par ordre d'utilite reelle :

| Module | Ce qu'il apporte | Pour qui |
|---|---|---|
| `variants` | declinaisons d'un produit (taille, teinte, finition) | commerce, negoce |
| `workstation` | postes de charge, lie a la nomenclature | fabrication, atelier |
| `salaries` | fiches de paie (la crate `paie` existe, non branchee) | tout employeur |
| `recruitment` | offres et candidatures | 3 salaries et plus |
| `eventorganization` | inscriptions a un evenement | formation, portes ouvertes |
| `bookcal` | reservation de creneaux | rendez-vous, location |
| `printing` | etiquettes et impressions | stock, atelier |
| `barcode` | codes-barres | stock, caisse |

Ne seront **pas** portes : `asterisk`, `dav`, `ftp`, `webservices`, `webhook`,
`zapier`, `paypal`, `stripe`, `ai`, `datapolicy`, `debugbar`, `modulebuilder`,
`langs`, `conf`. Ce sont des connecteurs : sans leur homologue externe, le code
serait mort.

### 2. Presets de base par metier

Une option, a la creation de la base **et** depuis Parametres > Modules, qui
installe un socle coherent :

```
Base nue          le minimum : tiers, produits, facturation
BTP / maconnerie  + chantiers, nomenclatures, marges, situations, retenues
Logistique        + stock, expeditions, receptions, transferts, inventaires
Commerce          + variantes, commandes, caisse, mailing
Bureau d'etudes   + projets, ressources, temps passe, notes de frais
Association       + adherents, dons, partenariats, evenements
```

Chaque preset est une **liste de modules actives** et de **constantes posees**,
pas une base differente. Le schema reste identique a l'octet : c'est la
selection qui change, et elle se defait.

Contrainte : un preset ne doit jamais ecrire de donnees metier (pas de tiers,
pas de produit). Une base pre-remplie de faux clients est une base qu'on ne
nettoie jamais.

### 3. Passe d'interface (skill `ui-ux-pro-max`)

**Seulement apres 1 et 2.** Refaire une interface sur un perimetre qui bouge
encore, c'est deux fois le travail.

Objectifs : systeme de jetons (couleurs, espacements, typographie), composants
coherents (tableaux, formulaires, filtres, pastilles), etats vides utiles,
accessibilite (contrastes, navigation clavier, cibles tactiles), mode sombre,
et densite adaptee a un ecran de bureau de comptable.

L'interface actuelle est sobre et lisible, mais elle a ete ecrite ecran par
ecran : les jetons et les composants sont a unifier, pas a inventer.

### 4. Multi-company par defaut

Aujourd'hui l'entite est portee par la session et tout fonctionne, mais c'est
un mecanisme qu'il faut connaitre pour s'en servir. Ce qu'il reste :

- choisir l'entite **a la connexion**, pas apres ;
- un bandeau qui dit toujours dans quelle societe on travaille (il existe,
  a rendre evident) ;
- des listes qui disent quand elles sont **filtrees** par entite ;
- l'export et la sauvegarde par entite ;
- un garde-fou : creer une facture dans la mauvaise societe doit etre difficile.

## Ce qui ne change jamais

Ces trois regles valent pour tous les chantiers ci-dessus.

1. **Les 410 tables du cceur restent intactes.** Empreinte
   `7293039ac8f4426f3a4a102552ca681228c2e2ffaec7bac10c9e6cc2cae68b2f`. Une
   base qui la respecte s'ouvre dans Dolibarr, et l'inverse aussi. Un module
   peut creer ses tables sous `llx_<module>_` (voir `REGLE-SCHEMA.md`).
2. **Rien n'est declare termine sans que les tests tournent.** `verifier.ps1`
   doit dire « TOUT EST VERT », pas « ca devrait marcher ».
3. **Un manque s'ecrit.** Un module qui ne fait pas une chose le dit dans sa
   doc et sur son ecran. C'est la regle qui a produit les meilleurs choix du
   projet : les receptions ne bougent pas le stock, `marquer_envoye` n'envoie
   rien, `cout_matiere` echoue plutot que de rendre un total partiel.

## Defauts de fond encore ouverts

| Defaut | Etat |
|---|---|
| Acompte non deduit de la facture finale | **corrige** le 06/11 (test a l'appui) |
| `fk_projet` jamais renseigne | **corrige** le 06/11 (propage devis → commande → facture) |
| Validation sans generation de PDF | ouvert |
| Marges absentes du gabarit PDF | ouvert |
