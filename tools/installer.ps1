# Installe une base Cordeau VIERGE : 410 tables, les dictionnaires Dolibarr,
# un administrateur, un preset de metier. Aucune donnee de demonstration.
#
#   .\tools\installer.ps1 -Base "cordeau" -Login "admin" -Mdp "MotDePasse2026!" -Preset "btp"
#
# Pour refaire une base existante : -Ecraser (destructif, confirmation tapee).

param(
    [string]$Base = "cordeau",
    [string]$Login = "admin",
    [string]$Mdp,
    [string]$Preset = "btp",
    [string]$Hote = "127.0.0.1",
    [string]$Port = "3307",
    [string]$Utilisateur = "root",
    [string]$MotDePasseBd = "root",
    [string]$Mysql = "C:\Users\Maxime\.mariadb\server\bin\mysql.exe",
    [switch]$Ecraser
)

$ErrorActionPreference = "Stop"
Push-Location (Join-Path $PSScriptRoot "..")

try {
    if (-not $Mdp) {
        Write-Host "-Mdp est obligatoire : c'est le mot de passe de l'administrateur." -ForegroundColor Red
        exit 1
    }
    if (-not (Test-Path $Mysql)) {
        Write-Host "Client mysql introuvable : $Mysql" -ForegroundColor Red
        Write-Host "  passe le bon chemin avec -Mysql"
        exit 1
    }
    foreach ($f in @("sql-ready/1_tables.sql", "sql-ready/2_keys.sql", "sql-ready/3_data.sql")) {
        if (-not (Test-Path $f)) {
            Write-Host "Fichier manquant : $f" -ForegroundColor Red
            exit 1
        }
    }

    $cree = $false
    $existe = (& $Mysql -h $Hote -P $Port -u $Utilisateur "-p$MotDePasseBd" -N -B -e "SELECT COUNT(*) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME='$Base'" 2>$null)
    if ($existe -ne "0") {
        if (-not $Ecraser) {
            Write-Host "La base $Base existe deja." -ForegroundColor Yellow
            Write-Host "  Pour la refaire entierement : -Ecraser (elle sera SUPPRIMEE)."
            exit 1
        }
        Write-Host "ATTENTION : la base $Base va etre SUPPRIMEE et refaite." -ForegroundColor Red
        $confirme = Read-Host "Tape SUPPRIMER pour continuer"
        if ($confirme -ne "SUPPRIMER") {
            Write-Host "Annule."
            exit 1
        }
        & $Mysql -h $Hote -P $Port -u $Utilisateur "-p$MotDePasseBd" -e "DROP DATABASE $Base" 2>$null
    }

    Write-Host "1/5  creation de la base $Base"
    & $Mysql -h $Hote -P $Port -u $Utilisateur "-p$MotDePasseBd" -e "CREATE DATABASE $Base CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci" 2>$null
    if ($LASTEXITCODE -ne 0) { throw "creation de la base refusee" }
    $cree = $true

    $etapes = @(
        @{ n = "2/5"; f = "sql-ready/1_tables.sql"; quoi = "410 tables" },
        @{ n = "3/5"; f = "sql-ready/2_keys.sql"; quoi = "index et cles" },
        @{ n = "4/5"; f = "sql-ready/3_data.sql"; quoi = "dictionnaires" }
    )
    # Les scripts de Dolibarr ne sont pas rejouables : `3_data.sql` bute sur
    # « Duplicate entry » des la premiere ligne si les dictionnaires sont deja
    # la. On importe donc en continuant (`--force`), et **on verifie l'etat
    # final** : c'est lui qui dit si l'installation est saine, pas le code de
    # sortie de mysql.
    foreach ($e in $etapes) {
        Write-Host "$($e.n)  $($e.f) - $($e.quoi)"
        cmd /c "`"$Mysql`" --force -D $Base -h $Hote -P $Port -u $Utilisateur -p$MotDePasseBd < `"$($e.f)`" 2>nul"
        if ($e.f -like "*1_tables*" -and $LASTEXITCODE -ne 0) {
            throw "les tables n'ont pas pu etre creees : $($e.f)"
        }
    }

    $tables = (& $Mysql -h $Hote -P $Port -u $Utilisateur "-p$MotDePasseBd" -N -B -D $Base -e "SELECT COUNT(*) FROM information_schema.TABLES WHERE TABLE_SCHEMA='$Base' AND TABLE_TYPE='BASE TABLE'" 2>$null)
    $pays = (& $Mysql -h $Hote -P $Port -u $Utilisateur "-p$MotDePasseBd" -N -B -D $Base -e "SELECT COUNT(*) FROM llx_c_country" 2>$null)
    $paiements = (& $Mysql -h $Hote -P $Port -u $Utilisateur "-p$MotDePasseBd" -N -B -D $Base -e "SELECT COUNT(*) FROM llx_c_paiement" 2>$null)
    Write-Host "     $tables table(s), $pays pays, $paiements moyen(s) de paiement"
    if ([int]$tables -lt 400) {
        throw "seulement $tables tables : l'import des tables a echoue"
    }
    if ([int]$pays -lt 100) {
        throw "seulement $pays pays : les dictionnaires n'ont pas ete importes"
    }

    $dsn = "mysql://${Utilisateur}:${MotDePasseBd}@${Hote}:${Port}/${Base}"

    Write-Host "5/5  administrateur $Login et preset $Preset"
    cargo run -q -p cordeau -- user add --dsn $dsn --login $Login --mdp $Mdp --admin
    if ($LASTEXITCODE -ne 0) { throw "creation de l'administrateur refusee" }
    cargo run -q -p cordeau -- preset appliquer --code $Preset --dsn $dsn
    if ($LASTEXITCODE -ne 0) { throw "application du preset refusee" }

    $lignes = @(
        "# Cordeau - configuration locale, ecrite par tools/installer.ps1",
        "# Ne pas versionner : ce fichier porte un mot de passe de base.",
        "DOLIBARR_DSN=$dsn"
    )
    Set-Content -Path ".env" -Value $lignes -Encoding UTF8

    Write-Host ""
    Write-Host "Base prete : $Base ($tables tables), administrateur $Login, preset $Preset." -ForegroundColor Green
    Write-Host "Lancer :  .\DEMARRER.ps1"
    Write-Host "Puis :    http://127.0.0.1:8090/connexion"
}
catch {
    Write-Host "Echec : $($_.Exception.Message)" -ForegroundColor Red
    if ($cree) {
        Write-Host "La base $Base a ete creee mais l'installation n'a pas abouti." -ForegroundColor Yellow
    }
    exit 1
}
finally {
    Pop-Location
}
