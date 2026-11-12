# Finir l'implantation — le plan, et la définition de « prêt »

> Écrit le **9 novembre 2026**. Les chiffres sont **mesurés** sur le dépôt, pas
> estimés. Ce qui n'est pas mesurable aujourd'hui est signalé comme tel.

---

## 1. Où on en est, mesuré

| | |
|---|---:|
| Modules de base (`crates/db`) | **96** |
| Routes HTTP | **257** |
| Fichiers d'écran | **86** |
| Tests qui passent | **1 066** |
| Commits | **221** |
| Styles en ligne restants | **0** (hors largeurs de barres calculées) |
| Écarts au schéma de production | **14 tables, 12 colonnes** — tous expliqués |

Branche 24.0.1, base de production copiée et auditée, PDF conformes au centime.

## 2. Ce que veut dire « prêt »

Cinq conditions. Tant qu'une seule manque, ce n'est pas prêt.

1. **Un utilisateur peut faire son métier sans Dolibarr.** Devis → commande →
   facture → règlement → relances, plus les achats et le grand livre. Pas « les
   écrans existent » : le parcours va au bout, avec les bons montants.
2. **La base reste Dolibarr.** Dolibarr PHP lit et écrit la même base sans rien
   casser, et Cordeau lit ce que Dolibarr a écrit.
3. **Rien ne se perd.** Sauvegarde, restauration, et migration d'une base d'une
   version antérieure, vérifiées sur copie.
4. **Ça ne ment pas.** Aucune liste tronquée muette, aucune erreur avalée, aucun
   `0` affiché à la place d'une table absente.
5. **C'est utilisable.** Lisible à 360 px, au clavier, au lecteur d'écran — et
   rapide sur une base de 500 tables.

## 3. Les quatre chantiers qui restent

### A. Mettre la base à l'épreuve d'une vraie migration — **1 jour**

Le planificateur existe (`cordeau schema planifier`), il n'a jamais été exécuté.
Il faut : prendre une base 19 (ou reconstituer), lancer Dolibarr pour la migrer,
puis vérifier avec Cordeau que tout est là — compteurs, montants, écrans.

**Bloquant pour la condition 3.** Rien d'autre ne peut se dire « prêt » sans ça.

### B. Classer les écarts de schéma — **1 jour**

`schema verify` crie au loup sur toute base réelle (14 tables, 12 colonnes). Il
faut deux références — installation neuve et base montée — ou un classement des
écarts. Tant que l'outil crie toujours, il ne sert à rien.

**Dépend de A** (il faut une base montée pour la seconde référence).

### C. ~~Finir les 109 styles en ligne~~ — **fait**

Trois passes : les largeurs normalisées, puis les couleurs et les panneaux, puis
les 45 valeurs uniques restantes, une par une. Le thème porte désormais les
utilitaires qui manquaient (`.muet`, `.panneau`, `.bloc-alerte`, `.temoin`,
`.zone-code`, `.grille`, `.ligne-total`, `.col-46`…).

**Ce qui reste en ligne est une donnée, pas une mise en forme** :
`style=(format!("width:{}%", …))` — la longueur d'une barre de graphique, qui
vaut ce que vaut le chiffre. Une barre ne se met pas en classe.

Vérifié : la page caisse ne rend plus **aucun** `style="…"`.

### D. Les manques fonctionnels connus — **3 à 5 jours**

| Manque | État | Où |
|---|---|---|
| Contacts : création et modification | **fait, prouvé** | `/contacts` : 303, 0 → 1 en base, puis modification relue en base |
| Factures de situation (cycle 4 jalons) | **fait, prouvé** | `db::situation` ; test d'intégration : quatre situations font exactement le devis |
| Carnet de commandes | **fait** | `db::carnet`, `/commandes/carnet` |
| Marge par chantier | **fait** | `db::carnet::marge_par_chantier`, `/chantiers/marges` |
| Top 5 clients sur le tableau de bord | **fait** | `db::stats::top_clients`, accueil |

Mesuré sur la copie de production, entité 7 : **60 580,70 €** facturés,
**17 612,27 €** d'achats, **42 968,43 €** de marge (70,93 %) sur cinq chantiers.
Le carnet est vide — et il le dit : la base ne porte aucune commande ouverte.

### E. La publication — **2 h** *(à la fin, sur ta demande)*

**200 commits qui n'existent qu'à un seul endroit.** Aucun remote, aucun tag.

**En attendant**, une sauvegarde complète est sur `X:` :
`X:\11_PROJETS_DIVERS\SAUVEGARDES\cordeau-2026-11-09.zip` — 201 Mo, 19 410
fichiers, `.git` inclus (vérifié). Refait à chaque étape importante.

## 4. L'ordre

```
A  migration réelle     (1 j)
B  classement schéma    (1 j, dépend de A)
D  manques métier       (3-5 j, commencer par les situations)
C  styles en ligne      (2 j, en parallèle, sans dépendance)
E  publication          (2 h, à la fin)
```

**Total : 7 à 9 jours de travail** pour les conditions 1 à 5, hors aléas.

## 5. L'échéance extérieure

**Janvier 2027 : Dolibarr 25.0.0.** Sept tables renomment leurs colonnes de date ;
nous en lisions **18** dans congés, mailing et documents.

**Tranché et fait** : `crates/db/src/colonnes.rs` demande à `information_schema`
quel nom existe, une fois par table et par processus, et les trois modules
construisent leur SQL avec. Prouvé sur la copie de production en 24 **et** sur
une copie de cette même base où les trois colonnes ont été renommées en
`date_creation` pour simuler la 25 — les mêmes pages répondent 200.

Les cinq autres tables annoncées sont déclarées dans la même table de
correspondance : le jour où on les lira, le nom sera bon sans y penser.

## 6. Comment on prouve que c'est prêt

Rien ne se déclare fini sans ces quatre commandes, et sans avoir lu leur sortie :

```powershell
.\tools\verifier.ps1                                    # 8 étages, 1 088 tests
cordeau audit --dsn … --entity 7                        # lecture seule
python tools\balayage-ecrans.py                         # tous les GET, zéro 500
python tools\compare-prod.py                            # compteurs base contre page
```

Et un parcours manuel, fait une fois, sur la copie : un devis créé, envoyé,
signé, converti en facture, réglé, relancé.

## 7. Ce que ce plan ne promet pas

- Que les 79 modules du catalogue Dolibarr soient tous portés. Ils ne le seront
  pas : les modules tiers et les connecteurs externes sont hors périmètre.
- Que la 25 soit suivie dans la semaine. Elle sera **anticipée**, pas courue après.
- Que « prêt » veuille dire « meilleur que Dolibarr ». Il veut dire **utilisable
  à la place de Dolibarr**, sur cette base-là, sans rien perdre.


---

## Programme de l executuer de migration (a ecrire)

cordeau schema migrer --dsn <base> --depuis 12.0.5 --vers 24.0.1 --src <migration> [--simuler]

1. lire MAIN_VERSION_LAST_UPGRADE, refuser si inconnue ou plus recente que la cible ;
2. appeler migration::chemin (deja ecrit, 5 tests) : il refuse un trou dans la chaine ;
3. --simuler (defaut) : afficher le plan, ne rien jouer ;
4. sinon, pour chaque script : START TRANSACTION, lire le fichier, decouper par
   schema::split_statements, jouer instruction par instruction, COMMIT ;
   au premier echec : ROLLBACK, ecrire le numero d instruction et le message,
   s arreter. Jamais de demi-migration.
5. journal : un fichier horodate par etape, avec le nom du script et le nombre
   d instructions jouees ;
6. a la fin : comparer l empreinte obtenue a schema-<version>.json, et le dire.

**Pre-requis** : travailler sur une copie. La commande doit refuser de s executer
si la base est celle de production (constante MAIN_INFO_SOCIETE_NOM differente
de la copie attendue, ou absence de marqueur ZZ_COPIE).
