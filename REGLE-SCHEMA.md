# Règle de schéma : le cœur à l'octet, les modules à côté

Décision du **4 novembre 2026**. Elle remplace la règle absolue « aucune table
ajoutée » qui valait jusqu'ici.

## La règle

**1. Les 410 tables du cœur sont intangibles.** Dolibarr 24.0.1, à l'octet.
Empreinte `7293039ac8f4426f3a4a102552ca681228c2e2ffaec7bac10c9e6cc2cae68b2f`.
Aucune colonne ajoutée, aucun type changé, aucune table renommée. Une base qui
respecte ça s'ouvre dans Dolibarr, et l'inverse aussi.

**2. Un module peut créer ses propres tables.** Préfixées `llx_<module>_`, elles
ne touchent pas au cœur et ne sont pas dans le manifeste. Elles sont
**légitimes** : le module officiel de facturation électronique en crée trois
(`llx_einvoicing_document`, `llx_einvoicing_extlinks`,
`llx_einvoicing_lifecycle_msg`).

**3. Ce qui manque casse, ce qui s'ajoute non.** La compatibilité se juge sur ce
qui **manque** (table absente, colonne absente, type changé). Une table en plus
ne rend pas la base incompatible — mais elle doit se voir.

## Pourquoi cette décision

Parce que le schéma du cœur ne suffit pas, et que Dolibarr l'a toujours su.

Les objets de la facturation électronique — l'historique des 14 statuts, le lien
entre une facture et l'objet chez la plateforme, la règle de traitement d'un
flux — **n'existent pas** dans les 410 tables. Le module officiel les range
ailleurs, dans ses propres tables. Sans le droit d'en créer, ces données
n'auraient pas de place : on les aurait mises dans un fichier, sans jointure
possible, et l'écran n'aurait jamais pu dire « telle facture a reçu tel statut à
telle heure ».

C'est exactement ce qui est arrivé au journal de caisse et au scellé : rangés
hors base, ils fonctionnent, mais on ne peut pas les interroger en SQL. La
prochaine fois, on ne refera pas cette erreur.

Ce que la règle **ne** permet pas : modifier une table du cœur pour y loger ce
qu'un module veut. Un `ALTER TABLE llx_facture ADD ...` reste interdit.

## Comment c'est vérifié

`schema verify` compare la base au manifeste et dit trois choses :

```
tables absentes    0
tables en plus     1 (modules)
  table de module llx_einvoicing_document (facturation electronique)
tables inconnues   1
  table inconnue  tmp_sauvegarde_2024
RESULTAT           compatible
```

- **table de module** : le préfixe est dans le registre
  (`db::PREFIXES_MODULE`), le module est nommé, la compatibilité tient.
- **table inconnue** : elle ne bloque pas, mais elle se signale. Une table
  oubliée par un `mysqldump` mal défait n'est pas un module.
- `tables absentes` ou `types differents` non nuls : **incompatible**, point.

Le registre est une liste explicite :

| Préfixe | Module |
|---|---|
| `llx_einvoicing_` | facturation électronique |
| `llx_accounting_` | comptabilité avancée |
| `llx_abricot_` | abricot |

Ajouter un module, c'est ajouter une ligne. Ce n'est pas une permission
implicite : c'est un nom qu'on donne.

## Ce qui reste ouvert

Les données qu'on range en `llx_const` (instantané de clôture) et en fichier
(journal de caisse, scellé) auraient leur place dans des tables préfixées. Rien
n'est migré : ces mécanismes fonctionnent et sont vérifiés. Le jour où une
jointure SQL devient nécessaire — un état de caisse sur trente jours, une
comparaison d'exercices —, la règle est là pour l'autoriser.
