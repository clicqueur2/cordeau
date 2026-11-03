# Mentions légales et respect du travail de Dolibarr

Ce dépôt n'est pas un logiciel indépendant de Dolibarr : c'est une **réécriture
d'une partie de son noyau en Rust**, qui conserve son schéma de base de données
à l'octet près, sa sémantique, ses identifiants de droits et ses libellés
d'écrans. Il est donc, au sens du droit d'auteur, une **œuvre dérivée de
Dolibarr**.

## Licence du noyau

- **Ce dépôt** : GPL-3.0-or-later (voir `LICENCE.txt`).
- **Dolibarr ERP & CRM** : GPL-3.0-or-later, Copyright © les auteurs de Dolibarr
  et leurs contributeurs — <https://github.com/Dolibarr/dolibarr>.

La GPL est conservée, pas contournée : toute redistribution de ce noyau, modifié
ou non, se fait sous GPL-3.0-or-later, avec le code source disponible. Une
version compilée seule, sans son code, n'est pas conforme.

## Ce qui est repris de Dolibarr

| Élément | Nature |
|---|---|
| Le schéma SQL (`schema-24.0.1.json`, 410 tables) | extrait de Dolibarr 24.0.1, non modifié |
| Les constantes `MAIN_MODULE_*`, `MAIN_INFO_*`, `MAIN_MAIL_*` | noms et sémantique |
| Les identifiants de droits (`llx_rights_def` : 11, 16, 21, 41, 81…) | valeurs exactes, relevées dans les `mod*.class.php` |
| Les libellés d'écrans et l'arborescence des menus | repris pour que personne ne soit perdu |
| L'arithmétique des prix (`calcul_price_total`) | vérifiée contre la fonction PHP d'origine, conservée comme oracle de test |

## Ce qui n'est pas repris

Aucune ligne de PHP n'est exécutée par ce noyau. Les deux fichiers PHP présents
sous `tools/oracle/` ne servent **qu'aux tests** : ils appellent la vraie
fonction de Dolibarr pour vérifier nos arrondis au centime. Ils ne sont ni
compilés, ni livrés, ni nécessaires à l'exécution.

## Greffons et modules externes

- Un **module déclaratif** (fichier JSON dans `modules/`) ne contient pas de
  code : il décrit des écrans et du SQL de lecture. Il n'ajoute aucune
  contrainte de licence au-delà de celle du noyau.
- Un **greffon** (bibliothèque `.dll`/`.so` en ABI C) est du code exécuté dans le
  processus. **S'il est distribué avec ce noyau, il doit être sous licence
  compatible GPL.** Un greffon propriétaire interne à une entreprise reste
  possible tant qu'il n'est pas distribué — c'est la règle habituelle de la GPL
  sur les œuvres liées.

## Marques

« Dolibarr » est le nom du projet d'origine. Ce dépôt ne le revendique pas : il
se présente comme un **noyau Dolibarr en Rust**, et ne doit pas être diffusé sous
un nom laissant croire qu'il est édité par l'équipe Dolibarr.

## Signaler un manquement

Si une reprise n'est pas correctement attribuée, c'est un défaut à corriger, pas
un détail : ouvrir un point sur le dépôt et il sera traité.
