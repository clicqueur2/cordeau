# Dolistore — le catalogue, mesuré

Ce dossier contient le **catalogue entier** de [dolistore.com](https://www.dolistore.com)
(catégorie *Modules/Plugins*), relevé le **9 novembre 2026**, et l'analyse qui
en sort : ce que l'écosystème Dolibarr a que nous n'avons pas.

| Fichier | Ce que c'est |
|---|---|
| `dolistore.json` | Les 670 modules : identifiant, titre, résumé, description, prix, version de Dolibarr compatible, URL |
| `DOLISTORE.md` | L'analyse : familles, prix, ce qui nous manque |
| `moissonner.py` | Le moissonneur (`scrapling`), rejouable |
| `analyser.py` | Le classement et le document, rejouables |

## Rejouer

```powershell
pip install scrapling
python outils\dolistore\moissonner.py
python outils\dolistore\analyser.py
```

Le moissonneur fait 56 requêtes, une page à la fois, avec 0,3 s d'attente. Le
site n'a ni API ni sitemap de produits : la pagination est `index.php?cat=68&p=N`,
douze produits par page.

## Ce qu'on ne peut pas mesurer, et pourquoi

Dolistore **ne publie pas** le nombre de téléchargements, ni un classement par
popularité. La page catégorie ne propose que *date / prix / nom* :

```html
<select id="selectProductSort">
  <option value="date:desc">Date: newest first</option>
  <option value="price:asc">Price: Lowest first</option>
  <option value="price:desc">Price: Highest first</option>
  <option value="name:asc">Product Name: A to Z</option>
  <option value="name:desc">Product Name: Z to A</option>
</select>
```

Vérifié aussi sur une fiche produit : aucun compteur de ventes ni de
téléchargements dans le HTML. « Les plus téléchargés » n'existe donc pas comme
donnée publique — ce qui existe, c'est le catalogue, et c'est ce qui est ici.

Si un jour il faut une mesure de popularité, les pistes qui, elles, publient
quelque chose :

- **GitHub** : étoiles et forks des modules open source ;
- **le wiki Dolibarr** : la page des modules complémentaires, avec leur suivi ;
- **Dolistore, fiche par fiche** : la date de dernière mise à jour — un module
  mis à jour en 2020 n'est plus un module vivant.
