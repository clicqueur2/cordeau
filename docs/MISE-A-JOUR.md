# Monter une base Dolibarr d'une version à l'autre — ce que Cordeau sait, et ce qu'il doit apprendre

> Écrit le **9 novembre 2026**, après lecture de la roadmap Dolibarr, du
> ChangeLog de la 25.0.0 et de l'outillage déjà présent dans le dépôt.

---

## La règle, avant tout

**Dolibarr migre. Cordeau constate.**

Le schéma appartient à Dolibarr : ses 31 scripts de migration, ses clés, ses
colonnes. Si Cordeau se mettait à `ALTER TABLE`, deux moteurs de migration
finiraient par se contredire, et la base ne serait plus ni l'un ni l'autre.

Cordeau fait donc trois choses, et rien d'autre :

1. **détecter** la version du schéma qu'il a sous les yeux ;
2. **refuser** de servir une base qu'il ne connaît pas, avec un message qui dit
   quoi faire ;
3. **vérifier** qu'après migration la base est intacte.

## Ce qui existe déjà dans le dépôt

| Outil | Ce qu'il fait |
|---|---|
| `cordeau schema manifest --src install/mysql/tables --version X --commit sha` | produit l'**empreinte** du schéma d'une version donnée |
| `cordeau schema verify` | compare la base à l'empreinte attendue |
| `cordeau schema prepare` | crée une base neuve à partir des scripts d'installation |
| `cordeau audit --dsn …` | lit une base inconnue **sans pouvoir y écrire** (`START TRANSACTION READ ONLY`) |
| `REGLE-SCHEMA.md` | l'empreinte du cœur en vigueur : `7293039a…8b2f` |

C'est exactement la chaîne qu'il faut pour une montée de version : on empreint
avant, on migre, on vérifie après.

## Les scripts de migration, là où ils sont

`_vendor/dolibarr-src/htdocs/install/mysql/migration/` — **31 fichiers SQL**, de
`7.0.0-8.0.0.sql` à `repair.sql`. Un fichier par saut de version.

C'est Dolibarr qui les joue, pas nous. Mais ils nous disent **ce qui change**,
version par version, et c'est la matière pour préparer chaque montée.

## Comment on reconnaît la version d'une base

Dolibarr écrit sa version dans ses constantes : `MAIN_VERSION_LAST_UPGRADE`
(lue par `htdocs/install/check.php`). C'est la première chose à interroger — avant
toute requête métier.

### Mesuré sur la copie de production, le 9 novembre 2026

```
MAIN_VERSION_LAST_INSTALL   24.0.1
MAIN_VERSION_LAST_UPGRADE   24.0.1
tables                      487
```

La base est donc **à jour**, et le nom de la constante d'installation (24.0.1)
montre qu'elle a été réinstallée, pas montée depuis la 19 comme le laissait croire
le commentaire de son `conf.php`.

## L'empreinte : ce qu'elle dit, et ce qu'elle ne peut pas dire

`schema verify` sur la **base de test** (installation neuve, 412 tables) :

```
empreinte          7293039a…8b2f   ← identique à REGLE-SCHEMA.md
tables absentes    0
colonnes absentes  0
types differents   0
```

Sur la **copie de production** (487 tables, donc base réelle) :

```
tables absentes    14
colonnes absentes  12
types differents   68
tables inconnues   89
```

**Ce n'est pas une alerte : c'est une leçon.** Les 14 tables « absentes » sont
celles de modules **non installés** (bookcal 4, printer_receipt 2, quickmemo 3,
stocktransfer 1…) : Dolibarr ne crée une table de module qu'à son activation. Une
installation neuve les a toutes ; une vraie base n'a que celles de ses modules.

Les 12 colonnes absentes sont plus intéressantes : `llx_facture.retained_warranty_amount`,
`llx_salary.num_payment`, `llx_societe_perentity.fk_account`, `llx_user.idpers1`…
Elles existent dans le schéma d'**installation neuve**, pas dans une base
**montée** — parce que Dolibarr ajoute certaines colonnes par `ALTER` de migration
sans les mettre dans les `CREATE TABLE`.

**Conséquence, et c'est un chantier** : une empreinte unique ne suffit pas. Il en
faut **deux** — « installation neuve » et « base montée depuis N » — ou bien un
`verify` qui classe chaque écart au lieu de le compter comme une faute :

| Écart | Verdict |
|---|---|
| table d'un module non activé | **normal** |
| colonne présente au `CREATE` mais absente après montée | **normal si**, et seulement si, aucune migration ne l'ajoute |
| colonne du cœur absente dans les deux références | **cassé** |

## La procédure de montée, telle qu'elle doit se dérouler

### 1. Toujours sur une copie

```bash
mysqldump --single-transaction --routines --triggers -u <user> -p <base> > avant.sql
```

Jamais sur la production : une migration à moitié passée ne se rattrape pas.

### 2. Empreindre l'état de départ

```bash
cordeau schema verify --dsn "mysql://…/copie" > avant.txt
```

On garde la sortie. C'est la pièce qui dit d'où l'on part.

### 3. Laisser Dolibarr migrer

L'upgrade se fait par **Dolibarr**, pas par Cordeau (écran `/install/` ou
`install/mysql/migration/*.sql` joués dans l'ordre par son propre moteur).

### 4. Constater, sans écrire

```bash
cordeau audit --dsn "mysql://…/copie" --entity 7
```

L'audit ouvre `START TRANSACTION READ ONLY` : il ne peut rien casser. Ce qu'il
doit dire : mêmes sociétés, mêmes compteurs, mêmes montants.

### 5. Comparer les écrans

```bash
python tools/compare-prod.py     # compteurs et totaux, base contre page
python tools/balayage-ecrans.py  # tous les GET, à la recherche de 500
```

Un écart sur un compteur après migration = une table que la migration a vidée ou
renommée. C'est le genre de chose qu'on ne voit pas à l'œil.

## Le premier vrai test : janvier 2027

La 25.0.0 renomme des colonnes de date dans **sept tables** (`DOLIBARR-25-NOUVEAUTES.md`).
Chez nous, **18 lectures** les utilisent : `conges.rs` (`llx_holiday`, 10),
mailing (3), `ged.rs` (`llx_ecm_files`, 5).

Deux stratégies possibles, à trancher :

- **lire les deux noms** — robuste, mais chaque requête porte une condition ;
- **détecter la version une fois**, au démarrage, et choisir la requête — plus
  propre, mais il faut que le choix soit visible dans le code.

La seconde est la bonne : un portage qui doit fonctionner sur 24 **et** sur 25 ne
peut pas semer des `COALESCE` partout.

## Ce qui manque, et que je propose de construire

| Manque | Pourquoi c'est nécessaire |
|---|---|
| `cordeau schema diff` | savoir **ce qui** diffère entre deux versions, pas seulement « ça diffère » |
| Détection de version au démarrage | refuser une base inconnue au lieu d'afficher des colonnes vides |
| Un manifeste par version dans le dépôt (23, 24, 25) | l'empreinte existe pour 24 ; les autres sont à produire |
| Un test de migration dans `verifier.ps1` | monter une base 23 → 24 en test, et vérifier qu'elle passe |

Voilà l'état réel. Rien n'est fait pour les montées de version aujourd'hui — mais
tout l'outillage pour les faire proprement existe déjà.
