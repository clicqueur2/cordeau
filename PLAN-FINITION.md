# Finir l'implantation — le plan, et la définition de « prêt »

> Écrit le **9 novembre 2026**. Les chiffres sont **mesurés** sur le dépôt, pas
> estimés. Ce qui n'est pas mesurable aujourd'hui est signalé comme tel.

---

## 1. Où on en est, mesuré

| | |
|---|---:|
| Modules de base (`crates/db`) | **95** |
| Routes HTTP | **254** |
| Fichiers d'écran | **85** |
| Tests qui passent | **1 045** |
| Commits | **200** |
| Styles en ligne restants | **109** |
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

### C. Finir les 109 styles en ligne — **2 jours**

Fichier par fichier, un œil sur le rendu. Ils bloquent les cinq thèmes : un écran
qui écrit `color:#78736c` en dur restera gris sous « Minuit ».

Mécanique, sans risque métier, mais à vérifier visuellement — donc lent.

### D. Les manques fonctionnels connus — **3 à 5 jours**

| Manque | Effort | Pourquoi |
|---|---:|---|
| Contacts : création et modification | ½ j | on les affiche, on ne peut pas les créer |
| Factures de situation (cycle 4 jalons) | 2 j | décidé, jamais construit ; sans cycle, la déduction casse |
| Carnet de commandes + marge par chantier | 1 j | les deux mesures qui manquent au pilotage |
| Top 5 clients sur le tableau de bord | 2 h | la fonction SQL existe, l'affichage manque |

### E. La publication — **2 h**

**200 commits qui n'existent qu'à un seul endroit.** Aucun remote, aucun tag.
C'est le seul risque qui peut tout effacer d'un coup, et c'est le moins cher à
supprimer.

## 4. L'ordre

```
E  publication          (2 h, à faire aujourd'hui — protège tout le reste)
A  migration réelle     (1 j)
B  classement schéma    (1 j, dépend de A)
D  manques métier       (3-5 j, commencer par les situations)
C  styles en ligne      (2 j, en parallèle, sans dépendance)
```

**Total : 7 à 9 jours de travail** pour les conditions 1 à 5, hors aléas.

## 5. L'échéance extérieure

**Janvier 2027 : Dolibarr 25.0.0.** Sept tables renomment leurs colonnes de date ;
nous en lisons **18** dans congés, mailing et documents. La stratégie n'est pas
tranchée (lire les deux noms, ou détecter la version une fois au démarrage).

Il faut la trancher **avant** la sortie, pas après. C'est un demi-jour de design
et un jour de mise en œuvre, à caler entre D et C.

## 6. Comment on prouve que c'est prêt

Rien ne se déclare fini sans ces quatre commandes, et sans avoir lu leur sortie :

```powershell
.\tools\verifier.ps1                                    # 7 étages, 1 045 tests
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
