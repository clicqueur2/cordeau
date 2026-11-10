# Publie le depot sur un remote : une commande, deux verifications.
#
#   .\tools\publier.ps1 -Url "https://github.com/<compte>/cordeau.git"
#
# Le jeton n'est jamais ecrit dans le script : git le demande une fois, ou il est
# deja dans le gestionnaire d'identifiants de Windows.

param(
    [Parameter(Mandatory = $true)][string]$Url,
    [string]$Branche = "main",
    [string]$Etiquette = "v0.1.0"
)

$ErrorActionPreference = "Stop"
Push-Location (Join-Path $PSScriptRoot "..")

try {
    # 1. Rien a publier dans le desordre : un arbre sale, c'est un commit oublie.
    $sale = git status --porcelain
    if ($sale) {
        Write-Host "Arbre non propre : commite ou jette avant de publier." -ForegroundColor Red
        $sale | ForEach-Object { Write-Host "  $_" }
        exit 1
    }

    # 2. Le remote. On ne remplace pas un origin existant sans le dire.
    $existant = git remote get-url origin 2>$null
    if ($LASTEXITCODE -eq 0 -and $existant) {
        if ($existant -ne $Url) {
            Write-Host "origin pointe deja sur $existant." -ForegroundColor Yellow
            Write-Host "Pour changer : git remote set-url origin $Url"
            exit 1
        }
    }
    else {
        git remote add origin $Url
        if ($LASTEXITCODE -ne 0) { throw "git remote add a echoue" }
    }

    # 3. La branche, puis l'etiquette.
    git push -u origin $Branche
    if ($LASTEXITCODE -ne 0) { throw "git push a echoue (jeton ? droits ?)" }

    $aEtiq = git tag --list $Etiquette
    if ($aEtiq) {
        git push origin $Etiquette
        if ($LASTEXITCODE -ne 0) { throw "git push de l'etiquette a echoue" }
    }

    Write-Host "Publie : $Url" -ForegroundColor Green
    git log --oneline -1
}
finally {
    Pop-Location
}
