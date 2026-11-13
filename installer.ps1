# Installateur Cordeau — raccourci.
#
# Le vrai script est `tools/installer.ps1` : il cree la base, les 410 tables, le
# compte administrateur et applique un preset de modules. Celui-ci existe pour
# qu'on trouve un installateur la ou on le cherche, sans en maintenir deux.
#
#   .\installer.ps1 -Base cordeau_test

param(
    [string]$Base = "cordeau_test",
    [string]$Admin = "admin",
    [string]$Mdp = ""
)

$ErrorActionPreference = "Stop"
$reel = Join-Path $PSScriptRoot "tools\installer.ps1"
if (-not (Test-Path $reel)) {
    Write-Host "tools\installer.ps1 introuvable : le depot est incomplet." -ForegroundColor Red
    exit 1
}

& $reel -Base $Base -Admin $Admin -Mdp $Mdp
exit $LASTEXITCODE
