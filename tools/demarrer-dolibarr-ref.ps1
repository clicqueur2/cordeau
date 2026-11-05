# Demarre l'instance Dolibarr de reference.

# A quoi ca sert : comparer une vraie instance Dolibarr 24.0.1 a Cordeau, sur la
# meme machine, avec la meme base de depart. Un ecran qui « a l'air juste » se
# verifie en regardant ce que Dolibarr affiche pour les memes donnees.
#
# Ce n'est pas la base de travail : c'est `dolibarr_ref`, a cote.

param(
    [string]$Port = "8080",
    [string]$Racine = "$PSScriptRoot\.."
)

$ErrorActionPreference = "Stop"
$ref = Join-Path $Racine "_vendor\dolibarr-ref"
$ini = Join-Path $env:TEMP "php-ref.ini"

if (-not (Test-Path $ref)) {
    Write-Host "Source Dolibarr absente : $ref" -ForegroundColor Red
    Write-Host "Telecharger https://github.com/Dolibarr/dolibarr/archive/refs/tags/24.0.1.zip"
    Write-Host "et extraire dans _vendor/dolibarr-ref"
    exit 1
}

# Le PHP installe n'a aucun php.ini : mysqli et pdo_mysql sont sur le disque mais
# pas charges. On fabrique un ini qui les charge, sans toucher au PHP du systeme.
if (-not (Test-Path $ini)) {
    $phpDir = Split-Path -Parent (Get-Command php).Source
    @"
extension_dir="$phpDir\ext"
extension=mysqli
extension=pdo_mysql
extension=mbstring
extension=openssl
extension=curl
extension=gd
extension=zip
extension=intl
extension=fileinfo
extension=exif
date.timezone = Europe/Paris
memory_limit = 512M
display_errors = On
"@ | Set-Content $ini -Encoding ASCII
    Write-Host "php.ini de reference ecrit : $ini"
}

# Ce que l'assistant d'installation ecrirait : le verrou et les constantes.
$lock = Join-Path $ref "documents\install.lock"
if (-not (Test-Path $lock)) {
    New-Item -ItemType Directory -Path (Split-Path $lock) -Force | Out-Null
    New-Item -ItemType File -Path $lock -Force | Out-Null
}

Write-Host ""
Write-Host "Dolibarr de reference sur http://127.0.0.1:$Port" -ForegroundColor Cyan
Write-Host "  compte : admin / Reference2026!"
Write-Host "  base   : dolibarr_ref (port 3307)"
Write-Host ""
Write-Host "Ctrl+C pour arreter."
php -c $ini -S "127.0.0.1:$Port" -t (Join-Path $ref "htdocs")
