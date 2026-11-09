# Le ChangeLog de Dolibarr — ce qu'il dit de la branche 24

> **Source** : `_vendor/dolibarr-ref/ChangeLog`, livré avec la source 24.0.1.
> **13 389 lignes**, **458 sections** de version. Lu le 9 novembre 2026 par
> extraction, pas par lecture intégrale — voir la méthode en bas.

---

## 1. Ce que porte la branche qu'on porte

| Section | Lignes | Contenu |
|---|---:|---|
| `24.0.1 compared to 24.0.0` | 46 | **43 FIX**, 0 nouveauté |
| `24.0.0 compared to 23.0` | 190 | **178 NEW**, 2 FIX |

Traduction : **24.0.1 est une version de correction pure** — 43 correctifs, aucune
nouveauté. C'est la version que porte la production de Pierre Angulaire, et c'est
la meilleure situation possible pour un portage : la branche ne bouge plus que
par corrections, donc le schéma non plus.

## 2. Les changements de schéma de la 24

Deux lignes seulement, dans les 190 de la 24.0.0 :

- **`NEW: Add column einvoice_vate on VAT dictionary`** → une colonne ajoutée au
  dictionnaire de TVA (`llx_c_tva`, table du cœur) ;
- `NEW: Contact - List - Add column State & Region (#37553)` → une **colonne
  d'affichage** dans une liste, pas une colonne de table.

**Conséquence pour Cordeau** : la règle « cœur à l'octet » reste tenable, mais
elle doit tolérer **une colonne ajoutée** au dictionnaire de TVA entre 23 et 24.
Notre `REGLE-SCHEMA.md` parle d'empreinte exacte : cette empreinte est donc
celle de **24.0.1**, pas celle de 23. Toute base restée en 23 sera vue comme
« différente » — ce qui est vrai, et doit être dit plutôt que corrigé à l'aveugle.

## 3. Ce que ça dit des prochaines majeures

La cadence est de deux majeures par an (voir `DOLIBARR-ROADMAP.md`), et une
majeure apporte en moyenne **~180 nouveautés** (mesure sur 24.0.0). Chacune est
susceptible de toucher le schéma.

**Échéance** : janvier 2027. C'est là qu'il faudra relire cette section, refaire
l'empreinte, et décider si Cordeau suit ou reste sur 24.

## 4. Comment ce fichier a été produit

```bash
# le fichier est dans la source vendue, pas sur le réseau
_vendor/dolibarr-ref/ChangeLog
```

Extraction par script : les sections sont délimitées par
`***** ChangeLog for X compared to Y *****` ; chaque ligne commence par une
catégorie (`FIX:`, `NEW:`, `SEC:`…). On compte par catégorie, on cherche les
mots `ADD COLUMN`, `ALTER TABLE`, `new table`, `new field`.

Ce qui n'a **pas** été fait : lire les 13 389 lignes une à une. Ce serait long et
sans valeur — ce qui compte pour un portage, c'est le **volume**, la **nature**
(correctifs ou nouveautés) et les **changements de schéma**.
