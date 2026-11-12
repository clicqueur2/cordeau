# Vérification complète de cordeau.
#
#   .\tools\verifier.ps1                 # tout, sauf le réseau
#   .\tools\verifier.ps1 -Reseau         # + les appels aux API de l'État
#   .\tools\verifier.ps1 -Base "mysql://root:root@127.0.0.1:3307/dolibarr"
#
# Sort en erreur (code 1) au premier échec : utilisable en CI ou en pré-commit.

param(
    [string]$Base = "mysql://root:root@127.0.0.1:3307/dolibarr",
    [switch]$Reseau
)

# Les commandes natives écrivent sur stderr : on ne veut pas que PowerShell les
# transforme en erreurs fatales. On décide sur les codes de sortie, explicitement.
$ErrorActionPreference = "Continue"
$racine = Split-Path -Parent $PSScriptRoot
Set-Location $racine

$env:DOLIBARR_DSN = $Base
$env:CARGO_TARGET_DIR = "C:/Users/Maxime/.cargo-target/cordeau"
if ($Reseau) { $env:DOLIBARR_ANNUAIRE_TEST = "1" }

# Les tests qui touchent MySQL partagent **une seule base** : lances en
# parallele, deux tests s'effacent mutuellement leurs lignes (`nettoyer()`
# efface toute l'entite) et deux insertions de la meme reference de recette
# entrent en collision. Les echecs sont alors aleatoires et n'ont rien a voir
# avec le code teste. Une base, un fil de tests.
$env:RUST_TEST_THREADS = "1"

$echecs = @()
function Etape($nom, $bloc) {
    Write-Host ""
    Write-Host "=== $nom" -ForegroundColor Cyan
    try {
        & $bloc
        if ($LASTEXITCODE -ne 0) { throw "code de sortie $LASTEXITCODE" }
        Write-Host "    OK" -ForegroundColor Green
    } catch {
        Write-Host "    ECHEC : $_" -ForegroundColor Red
        $script:echecs += $nom
    }
}

# --- 1. Formatage -----------------------------------------------------------
Etape "Formatage (cargo fmt --check)" {
    cargo fmt --all -- --check
}

# --- 2. Analyse statique ----------------------------------------------------
Etape "Analyse statique (clippy -D warnings)" {
    cargo clippy --workspace --all-targets -- -D warnings
}

# --- 3. Tests unitaires et d'intégration ------------------------------------
Etape "Tests (unitaires + intégration + doctests)" {
    $sortie# Un seul fil : les tests ecrivent dans la meme base reelle et se
# bloquent mutuellement (deadlock 1213) quand ils tournent en parallele.
$sortie = & cargo test --workspace -- --test-threads=1 2>&1 | Out-String -Stream
    $code = $LASTEXITCODE
    $sortie | Select-String -Pattern "^test result" | ForEach-Object { Write-Host "    $($_.Line)" }
    $total = 0
    foreach ($l in $sortie) { if ($l -match "^test result: ok\. (\d+) passed") { $total += [int]$Matches[1] } }
    $ko = ($sortie | Select-String -Pattern "^test result: FAILED" -CaseSensitive).Count
    Write-Host "    total : $total tests, $ko suite(s) en échec"
    if ($ko -gt 0 -or $code -ne 0) { $sortie | Select-String -Pattern "^failures:" -CaseSensitive -Context 0,4 | ForEach-Object { Write-Host $_.Context.PostContext }; throw "$ko suite(s) en échec (code $code)" }
}

# --- 4. Conformité du schéma ------------------------------------------------
Etape "Schéma (empreinte Dolibarr 24.0.1)" {
    cargo run -q -p cordeau -- schema verify --dsn $Base | Select-Object -Last 1
    if ($LASTEXITCODE -ne 0) { throw "schéma incompatible" }
}

# --- 4b. Chaîne de migration ------------------------------------------------
# Ne touche à aucune base : vérifie que 12, 19 et 23 montent encore en 24, et
# que les quatre empreintes sont là. Une version ajoutée ou un script renommé
# casserait la chaîne en silence ; ce garde-fou le dit.
Etape "Chaînes de migration (12, 19, 23 → 24)" {
    $sortie = powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "verifier-migrations.ps1") 2>&1
    $sortie | ForEach-Object { Write-Host "    $_" }
    if ($LASTEXITCODE -ne 0) { throw "chaîne de migration incomplète" }
}

# --- 5. Chaîne de bout en bout (système) ------------------------------------
Etape "Chaîne métier (devis → facture → règlement → PDF)" {
    $d = cargo run -q -p cordeau -- party add --nom "Verif Bout-en-Bout" 2>&1
    $soc = [regex]::Match($d, 'rowid=(\d+)').Groups[1].Value
    $p = cargo run -q -p cordeau -- proposal add --soc $soc --ligne "1|1000|20|Verification" 2>&1
    $dev = [regex]::Match($p, 'rowid=(\d+)').Groups[1].Value
    # Un devis se valide puis se cloture (statut 2 = signe) avant facturation,
    # comme dans Dolibarr : c'est la lecon des extractions du 4 novembre.
    cargo run -q -p cordeau -- proposal validate --id $dev 2>&1 | Out-Null
    cargo run -q -p cordeau -- proposal close --id $dev --statut 2 2>&1 | Out-Null
    $f = cargo run -q -p cordeau -- invoice from-proposal --devis $dev 2>&1
    $fac = [regex]::Match($f, 'rowid=(\d+)').Groups[1].Value
    Write-Host "    tiers=$soc devis=$dev facture=$fac"
    cargo run -q -p cordeau -- invoice validate --id $fac | Out-Null
    cargo run -q -p cordeau -- payment add --facture $fac --montant 1200 --mode virement 2>&1 | Select-Object -First 1 | ForEach-Object { Write-Host "    $_" }
    cargo run -q -p cordeau -- invoice pdf --id $fac --out "$env:TEMP\verif.pdf" 2>&1 | ForEach-Object { Write-Host "    $_" }
    $taille = (Get-Item "$env:TEMP\verif.pdf").Length
    Write-Host "    PDF : $taille octets"
    if ($taille -lt 2000) { throw "PDF suspect (trop petit)" }
    # Ménage. `invoice delete` refuse une facture validée — c'est la règle de
    # production, et ce script ne la contourne pas : il efface lui-même les
    # lignes qu'il vient de créer, en SQL, dans la base de travail.
    $mysql = "mysql"
    if ($env:DOLIBARR_MYSQLDUMP) {
        $mysql = Join-Path (Split-Path $env:DOLIBARR_MYSQLDUMP) "mysql.exe"
    }
    $outil = Get-Command $mysql -ErrorAction SilentlyContinue
    if (-not $outil -or -not $env:DOLIBARR_DB_PASSWORD) {
        Write-Host "    ménage SQL sauté : client mysql ou mot de passe absent (les pièces d'essai restent en base)"
        Remove-Item "$env:TEMP\verif.pdf" -ErrorAction SilentlyContinue
        return
    }
    $hote = "127.0.0.1"
    $port = "3307"
    $base = "dolibarr"
    if ($env:DOLIBARR_DSN -match '@([^:]+):(\d+)/([A-Za-z0-9_]+)') {
        $hote = $Matches[1]
        $port = $Matches[2]
        $base = $Matches[3]
    }
    # Les règlements d'abord : la clé étrangère interdit de les supprimer tant
    # que le lien existe, donc on lit leurs identifiants, on casse le lien,
    # puis on supprime.
    $pay = (& $mysql -h $hote -P $port -u root "-p$env:DOLIBARR_DB_PASSWORD" -D $base -N `
        -e "SELECT GROUP_CONCAT(fk_paiement) FROM llx_paiement_facture WHERE fk_facture = $fac" 2>$null)
    $pay = ($pay | Out-String).Trim()
    $sql = "DELETE FROM llx_paiement_facture WHERE fk_facture = $fac; "
    if ($pay -and $pay -ne "NULL") {
        $sql += "DELETE FROM llx_paiement WHERE rowid IN ($pay); "
    }
    $sql += "DELETE FROM llx_facturedet WHERE fk_facture = $fac; " +
            "DELETE FROM llx_element_element WHERE fk_target = $fac; " +
            "DELETE FROM llx_accounting_bookkeeping WHERE fk_doc = $fac; " +
            "DELETE FROM llx_facture WHERE rowid = $fac; " +
            "DELETE FROM llx_propaldet WHERE fk_propal = $dev; " +
            "DELETE FROM llx_propal WHERE rowid = $dev; " +
            "DELETE FROM llx_societe WHERE rowid = $soc;"
    & $mysql -h $hote -P $port -u root "-p$env:DOLIBARR_DB_PASSWORD" -D $base -e $sql 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "ménage SQL en échec (code $LASTEXITCODE)" }
    Remove-Item "$env:TEMP\verif.pdf" -ErrorAction SilentlyContinue
}

# --- 6. Interface web (système) ---------------------------------------------
# Identifiants de verification : jamais en clair dans un depot public.
if (-not $env:DOLIBARR_VERIF_LOGIN) { $env:DOLIBARR_VERIF_LOGIN = "maxime" }
if (-not $env:DOLIBARR_VERIF_MDP) { $env:DOLIBARR_VERIF_MDP = "" }

Etape "Interface web (10 écrans)" {
    $pages = @("/", "/tiers", "/produits", "/devis", "/factures", "/relances",
               "/parametres", "/parametres/dictionnaires", "/parametres/base",
               "/parametres/utilisateurs", "/parametres/import", "/parametres/sauvegarde")
    # Les 18 listes des modules secondaires et les 5 formulaires de saisie.
    $pages += @("/liste/Adherent", "/liste/Don", "/liste/Reception",
                "/liste/Categorie", "/liste/ExpenseReport", "/liste/Holiday",
                "/liste/Mailing", "/liste/Prelevement", "/liste/Ficheinter",
                "/liste/Bookmark", "/liste/Tax", "/liste/Salaries", "/liste/Loan",
                "/liste/ProductBatch", "/liste/Accounting", "/liste/Asset",
                "/liste/Resource", "/liste/PaymentByBankTransfer",
                "/liste/Loan/nouveau", "/liste/Asset/nouveau",
                "/liste/ProductBatch/nouveau", "/liste/Resource/nouveau",
                "/liste/Tax/nouveau")
    # Sans mot de passe de vérification, on ne peut pas se connecter : autant le
    # dire que d'échouer sur une page de connexion. Le crochet pre-push tourne
    # sans ce secret, et un push ne doit pas dépendre d'un mot de passe.
    if (-not $env:DOLIBARR_VERIF_MDP) {
        Write-Host "    mot de passe de vérification absent (DOLIBARR_VERIF_MDP) — écrans non testés"
        return
    }
    $vivant = $false
    try { Invoke-WebRequest -Uri "http://127.0.0.1:8090/connexion" -UseBasicParsing -TimeoutSec 3 -ErrorAction Stop | Out-Null; $vivant = $true } catch {}
    if (-not $vivant) {
        Write-Host "    serveur non démarré — écrans non testés (lancer : cargo run -q -p web)"
        return
    }
    $s = New-Object Microsoft.PowerShell.Commands.WebRequestSession
    Invoke-WebRequest -Uri "http://127.0.0.1:8090/connexion" -Method POST -UseBasicParsing -TimeoutSec 10 -WebSession $s -Body @{login=$env:DOLIBARR_VERIF_LOGIN;mdp=$env:DOLIBARR_VERIF_MDP} | Out-Null
    $ko = 0
    foreach ($p in $pages) {
        try {
            $r = Invoke-WebRequest -Uri ("http://127.0.0.1:8090" + $p) -UseBasicParsing -TimeoutSec 15 -WebSession $s
            Write-Host ("    {0,-32} {1}" -f $p, $r.StatusCode)
            if ($r.StatusCode -ne 200) { $ko++ }
        } catch { Write-Host ("    {0,-32} KO" -f $p) -ForegroundColor Red; $ko++ }
    }
    # Le garde-fou : sans session, on doit atterrir sur la page de connexion.
    $anonyme = New-Object Microsoft.PowerShell.Commands.WebRequestSession
    $r = Invoke-WebRequest -Uri "http://127.0.0.1:8090/factures" -UseBasicParsing -TimeoutSec 10 -WebSession $anonyme
    $titre = [regex]::Match($r.Content, '<h1[^>]*>(.*?)</h1>').Groups[1].Value
    Write-Host "    sans session                $($r.StatusCode) / $titre"
    if ($titre -notmatch "Connexion") { throw "une page protégée a répondu sans session" }
    if ($ko -gt 0) { throw "$ko écran(s) en échec" }
}

# --- 7. Réseau (optionnel) --------------------------------------------------
if ($Reseau) {
    Etape "Annuaire (BAN + INSEE)" {
        $s = & cargo test -p annuaire 2>&1 | Out-String -Stream
        $code = $LASTEXITCODE
        $s | Select-String -Pattern "^test result" | ForEach-Object { Write-Host "    $($_.Line)" }
        if ($code -ne 0) { throw "tests réseau en échec" }
    }
}

# --- Bilan ------------------------------------------------------------------
Write-Host ""
if ($echecs.Count -eq 0) {
    Write-Host "TOUT EST VERT" -ForegroundColor Green
    exit 0
} else {
    Write-Host ("ECHECS : " + ($echecs -join ", ")) -ForegroundColor Red
    exit 1
}
