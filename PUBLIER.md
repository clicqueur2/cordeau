# Publier Cordeau

> **Publié le 10 novembre 2026.** Le dépôt est sur GitHub :
> **`https://github.com/clicqueur2/cordeau`** — **privé**.
> Branche `main`, étiquette `v0.1.0`, **242 commits**.

## Ce qui est en place

| | |
|---|---|
| Remote | `origin` → `https://github.com/clicqueur2/cordeau.git` |
| Branche | `main` (renommée depuis `master` au moment de la poussee) |
| Étiquette | `v0.1.0`, annotée, poussee |
| Authentification | gestionnaire d'identifiants de Windows (aucun jeton dans `.git/config`) |
| Visibilité | **privé** — la page renvoie 404 sans être connecté |

Vérifié après coup, avec `git ls-remote origin` :

```
e5c1e282…  HEAD
e5c1e282…  refs/heads/main
350e8cee…  refs/tags/v0.1.0
1852b13e…  refs/tags/v0.1.0^{}
```

## Comment republier

```powershell
cd Z:\11_PROJETS_DIVERS\dolibarr-rs
git push
git push --tags
```

ou, en une commande qui vérifie d'abord que l'arbre est propre :

```powershell
.\tools\publier.ps1 -Url "https://github.com/clicqueur2/cordeau.git"
```

## Le rendre public, si tu veux

Un dépôt privé ne se trouve pas. Pour que quelqu'un puisse l'installer, le lire
ou signaler un bug, il faut le passer en public — dans **Settings → General →
Danger Zone → Change visibility**. Deux choses à faire **avant** :

1. `SECRETS-AUDIT.md` : vérifier qu'aucune clé ne traîne dans l'historique.
   Aujourd'hui, aucune ne doit y être — mais un dépôt public ne se dépublie pas.
2. `MENTIONS-LEGALES.md` : Cordeau est une **œuvre dérivée** de Dolibarr 24.0.1
   (GPL-3.0-or-later). La licence et l'attribution doivent rester en tête du
   README, et le fichier `LICENSE` doit être présent.

## Ce que la publication change

- **Un troisième endroit.** Le code n'existe plus seulement sur ce disque et sur
  `X:`.
- **L'historique lisible.** Chaque commit dit *pourquoi*.
- **Le partage possible.** Un lien vers un dépôt vert (1 083 tests) vaut mieux
  qu'un message « j'ai fait un fork » sur le forum Dolibarr.

## Vérifier avant de publier

```powershell
.\tools\verifier.ps1          # 8 étages, 1 083 tests
git status --porcelain         # doit être vide
```
