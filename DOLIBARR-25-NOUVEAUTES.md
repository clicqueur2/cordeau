# Dolibarr 25.0.0 — ce qui est annoncé pour la majeure de janvier 2027

> **Source** : `raw.githubusercontent.com/Dolibarr/dolibarr/develop/ChangeLog`,
> section `***** ChangeLog for 25.0.0 compared to 24.0 *****`, lu le
> **9 novembre 2026**.
>
> Le ChangeLog livré avec la 24.0.1 s'arrête à la 24 — les nouveautés de 25
> n'existent que sur la branche `develop`. La page wiki `Roadmap and Release
> 25.0.0` répond **404** : elle n'est pas encore écrite.

---

## 1. Où on en est dans le calendrier

| | |
|---|---|
| Version en production (PA) | 24.0.1 |
| Prochaine majeure | **25.0.0 — janvier 2027** |
| Gel de la branche | **novembre 2026, c'est-à-dire maintenant** |
| Ce qui sort d'ici là | 24.0.2 et 24.0.3 — **correctifs seulement** |

## 2. Ce que la section 25.0.0 contient aujourd'hui

Un bloc **WARNING** — la liste des changements qui **cassent** les modules
externes. Pas encore de liste `NEW:` : elle se remplit pendant le gel.

## 3. Les ruptures annoncées, et ce qu'elles nous font

| Ce que Dolibarr 25 change | Effet sur Cordeau |
|---|---|
| **Colonnes renommées** : `date_create` → `date_creation` dans `llx_expensereport`, `llx_multicurrency`, `llx_holiday` ; `datecreate` dans `llx_element_contact` ; `date_c` dans `llx_ecm_directories` et `llx_ecm_files` ; `date_creat` dans `llx_mailing` et `llx_mailing_unsubscribe` | **Le plus grave pour nous : notre empreinte de schéma change, et toute requête qui lit ces colonnes casse.** À vérifier chez nous : ces tables sont-elles lues, et sous quel nom de colonne ? |
| Hooks renommés (webportal `webPortalX…`, trigger `BILL_SUPPLIER_CANCELED` → `BILL_SUPPLIER_CANCEL`) | Aucun effet : Cordeau n'implémente pas de hooks PHP |
| `getRights()` remplacé par `hasUserPermission()` | Aucun effet : nous ne portons pas la couche PHP |
| `img_object()` ne préfixe plus `object_` | Aucun effet |
| Bibliothèque **MathPHP supprimée** | Aucun effet |
| Gestionnaire de menu « smartphone » supprimé | Aucun effet |
| **41 anciennes URLs** (`commande/note.php?id=X`, `product/note.php?id=X`…) renvoient 404, sans redirection | Aucun effet direct — mais c'est un rappel : Dolibarr casse des liens sans prévenir, donc nos routes ne doivent pas dépendre des siennes |

## 4. La conclusion qui compte

Sur les sept ruptures, **une seule nous atteint** : le renommage de colonnes de
date dans sept tables. Les autres sont des affaires de PHP.

C'est exactement ce que la roadmap laissait prévoir (`DOLIBARR-ROADMAP.md` §3), et
c'est maintenant daté et chiffré : **janvier 2027**, sept tables concernées.

**À faire d'ici là** : vérifier, dans notre code, si `date_create`, `datecreate`,
`date_c` ou `date_creat` sont lus quelque part. Si oui, prévoir la lecture des
deux noms — le portage doit fonctionner sur 24 **et** sur 25.

### Vérifié le jour même : oui, on les lit

| Colonne | Occurrences | Où |
|---|---:|---|
| `date_create` | 10 | `conges.rs` (congés = `llx_holiday`) |
| `date_creat` | 3 | mailing |
| `date_c` | 5 | `ged.rs` (documents = `llx_ecm_files`) |
| `datecreate` | 0 | — |

Donc **18 lectures à adapter**, sur trois modules : congés, mailing, documents.
Rien de bloquant aujourd'hui — la production est en 24.0.1 — mais c'est le
premier chantier de janvier 2027, et il est maintenant localisé.

## 5. Méthode

```bash
curl -s https://raw.githubusercontent.com/Dolibarr/dolibarr/develop/ChangeLog
```

La section `25.0.0` est en tête du fichier. Elle ne contient aujourd'hui que le
bloc `WARNING:` ; les entrées `NEW:` apparaîtront pendant le gel de novembre et
la beta, jusqu'à la sortie de janvier 2027.
