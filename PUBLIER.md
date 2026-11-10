# Publier Cordeau

> **L'état au 10 novembre 2026** : le dépôt est complet et vert (1 081 tests), mais
> il **n'a aucun remote**. Tout vit sur ce disque et dans
> `X:\11_PROJETS_DIVERS\SAUVEGARDES\`. Cette page dit comment le sortir, et ce
> qui manque pour le faire.

## Ce qu'il manque pour publier

Deux choses, et deux seulement :

1. **une adresse de dépôt** — par exemple `https://github.com/<compte>/cordeau.git`.
   Le dépôt doit être **privé** : il contient `_vendor/dolibarr-src` (GPL, donc
   rediffusable) mais aussi des fichiers de travail qui n'ont pas à être publics.
2. **un jeton** — GitHub n'accepte plus le mot de passe en ligne de commande. Un
   *personal access token* avec le droit `repo` suffit.

`gh` (GitHub CLI) n'est **pas installé** sur cette machine : `gh --version`
répond « commande introuvable ». On passe donc par `git` seul.

## La commande, quand tu auras les deux

```powershell
$env:GITHUB_TOKEN = "ghp_..."          # une fois par session
.\tools\publier.ps1 -Url "https://github.com/<compte>/cordeau.git"
```

Le script fait, dans l'ordre :

1. refuse de continuer si l'arbre n'est pas propre ;
2. ajoute le remote `origin` s'il n'existe pas ;
3. pousse la branche `main` ;
4. pousse l'étiquette `v0.1.0`.

Il ne fait **rien d'autre** : pas de réécriture d'historique, pas de force-push.

## À la main, si tu préfères

```powershell
git remote add origin https://<jeton>@github.com/<compte>/cordeau.git
git push -u origin main
git push origin v0.1.0
```

Le jeton dans l'URL est écrit **en clair dans `.git/config`** : c'est pratique et
c'est un secret sur le disque. Préfère le gestionnaire d'identifiants :

```powershell
git remote add origin https://github.com/<compte>/cordeau.git
git push -u origin main        # Git demande le jeton, et le retient
```

## Ce qui a été fait sans réseau

- **étiquette `v0.1.0`** posée localement, annotée : elle marque l'état publié.
  Elle partira avec le `git push --tags`.
- **sauvegarde ZIP** à chaque étape dans `X:\11_PROJETS_DIVERS\SAUVEGARDES\`,
  `.git` inclus : le dépôt est récupérable même si ce disque meurt.

## Vérifier avant de publier

```powershell
.\tools\verifier.ps1          # 8 étages, 1 081 tests
git status --porcelain         # doit être vide
git log --oneline -1
```

## Licence

Cordeau est une **œuvre dérivée** de Dolibarr ERP & CRM 24.0.1 (GPL-3.0-or-later).
Il doit donc être publié sous GPL-3.0-or-later. Voir `MENTIONS-LEGALES.md`.
