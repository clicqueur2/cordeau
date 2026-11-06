# Demarre l'interface Cordeau et affiche l'adresse a ouvrir.
#
#   .\demarrer.ps1                    # sur la copie de production
#   .\demarrer.ps1 -Base dolibarr    # sur la base de test
#   .\demarrer.ps1 -Port 8091        # si le port est deja pris
#
# Rien n'est ecrit dans la base au demarrage, sauf les deux tables de module
# (llx_marge_affaire, llx_marge_ligne), creees si elles manquent.

param(
    [string]$Base = "dolibarr_prod",
    [int]$Port = 8090,
    [string]$Utilisateur = "root",
    [string]$MotDePasse = "root",
    [string]$Hote = "127.0.0.1",
    [int]$PortBase = 3307
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$env:CARGO_TARGET_DIR = "C:/Users/Maxime/.cargo-target/dolibarr-rs"
$env:DOLIBARR_DSN = "mysql://${Utilisateur}:${MotDePasse}@${Hote}:${PortBase}/${Base}"
$env:DOLIBARR_WEB_ADDR = "127.0.0.1:${Port}"
$env:DOLIBARR_DOCUMENTS = "C:/Users/Maxime/.dolibarr-documents"
$env:DOLIBARR_SAUVEGARDES = "C:/Users/Maxime/.dolibarr-sauvegardes"
$env:DOLIBARR_MYSQLDUMP = "C:/Users/Maxime/.mariadb/server/bin/mysqldump.exe"
$env:DOLIBARR_DB_PASSWORD = $MotDePasse

# Un serveur deja lance sur ce port ne se remplace pas : on le dit.
$deja = Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue
if ($deja) {
    Write-Host "Le port $Port est deja utilise (pid $($deja[0].OwningProcess))." -ForegroundColor Yellow
    Write-Host "Ouvrir http://127.0.0.1:$Port/ ou relancer avec -Port 8091."
    exit 0
}

Write-Host "Base   : $Base"
Write-Host "Ecoute : http://127.0.0.1:$Port/"
Write-Host "Arret  : Ctrl+C dans cette fenetre"
Write-Host ""

& cargo run -q -p web --bin cordeau-web
