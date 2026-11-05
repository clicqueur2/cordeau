# Installe une base de test Dolibarr 24.0.1 pour le noyau Rust.
#
#   .\installer.ps1 -Base dolibarr_testus -Utilisateur root -MotDePasse root
#
# Le script ne touche a rien d'autre qu'a la base nommee : s'il existe deja une
# base de ce nom, il s'arrete au lieu de l'ecraser.

param(
    [string]$Base = "dolibarr_testus",
    [string]$Utilisateur = "root",
    [string]$MotDePasse = "",
    [string]$Hote = "127.0.0.1",
    [int]$Port = 3306,
    # Chemin du client mysql. Vide = on cherche dans le PATH.
    [string]$Mysql = "",
    [switch]$Forcer
)

$ErrorActionPreference = "Stop"

# --- trouver le client mysql ------------------------------------------------
if (-not $Mysql) {
    $candidats = @(
        "mysql",
        "C:\Users\$env:USERNAME\.mariadb\server\bin\mysql.exe",
        "C:\xampp\mysql\bin\mysql.exe",
        "C:\Program Files\MariaDB 11.4\bin\mysql.exe"
    )
    foreach ($c in $candidats) {
        $trouve = Get-Command $c -ErrorAction SilentlyContinue
        if ($trouve) { $Mysql = $trouve.Source; break }
        if (Test-Path $c) { $Mysql = $c; break }
    }
}
if (-not $Mysql -or -not (Test-Path $Mysql)) {
    Write-Host "Client mysql introuvable. Passer -Mysql <chemin>" -ForegroundColor Red
    exit 1
}

$racine = Split-Path -Parent $MyInvocation.MyCommand.Path
foreach ($f in @("1_tables.sql", "2_keys.sql", "3_data.sql")) {
    if (-not (Test-Path (Join-Path $racine "sql-ready\$f"))) {
        Write-Host "sql-ready\$f manquant. Regenerer avec : cargo run -p cordeau -- schema prepare --src <install/mysql>" -ForegroundColor Red
        exit 1
    }
}

$motDePasseArg = @()
if ($MotDePasse) { $motDePasseArg = @("-p$MotDePasse") }

function Executer([string]$requete) {
    & $Mysql -h $Hote -P $Port -u $Utilisateur @motDePasseArg -e $requete 2>&1
}

# --- la base existe-t-elle deja ? -----------------------------------------
$existe = Executer "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = '$Base';"
if ($existe -and -not $Forcer) {
    Write-Host "La base '$Base' existe deja. Rien n'a ete touche." -ForegroundColor Yellow
    Write-Host "Pour la recreer : -Forcer, ou choisir un autre nom (-Base autre_nom)."
    exit 1
}

Write-Host "Creation de la base $Base..." -ForegroundColor Cyan
Executer "CREATE DATABASE IF NOT EXISTS $Base CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;" | Out-Null

foreach ($f in @("1_tables.sql", "2_keys.sql", "3_data.sql")) {
    Write-Host "  import de $f..." -ForegroundColor Cyan
    $chemin = Join-Path $racine "sql-ready\$f"
    # Le fichier des cles recree des index deja poses par les tables : on
    # laisse mysql continuer au lieu de s'arreter au premier doublon.
    $sup = @()
    $sup = @("--force")   # doublons d'index et de donnees de reference : attendus
    Get-Content $chemin -Raw -Encoding UTF8 |
        & $Mysql -h $Hote -P $Port -u $Utilisateur @motDePasseArg @sup $Base
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Import de $f en echec." -ForegroundColor Red
        exit 1
    }
}

# --- verification ---------------------------------------------------------
$nb = (& $Mysql -h $Hote -P $Port -u $Utilisateur @motDePasseArg -N -e "SELECT COUNT(*) FROM information_schema.TABLES WHERE TABLE_SCHEMA='$Base';")
Write-Host ""
Write-Host "Base '$Base' prete : $nb tables." -ForegroundColor Green
Write-Host ""
Write-Host "Pour lancer le noyau :" -ForegroundColor Cyan
Write-Host "  `$env:DOLIBARR_DSN = `"mysql://$Utilisateur`:$MotDePasse@$Hote`:$Port/$Base`""
Write-Host "  `$env:DOLIBARR_DOCUMENTS = `"$PWD\documents`""
Write-Host "  cargo run -q -p web"
Write-Host ""
Write-Host "Puis ouvrir http://127.0.0.1:8090/installation"
