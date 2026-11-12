# Vérification avant commit (appelée par .git/hooks/pre-commit).
#
#   .\tools\pre-commit.ps1            # formatage + clippy + tests (base MySQL locale)
#   .\tools\pre-commit.ps1 -Rapide    # formatage + clippy seulement
#   .\tools\pre-commit.ps1 -Complet   # + tools\verifier.ps1 (schéma, chaîne métier, écrans)
#
# Sort en code 1 dès la fin : git refuse alors le commit.
# Pour passer outre en connaissance de cause : git commit --no-verify

param(
    [switch]$Rapide,
    [switch]$Complet,
    [string]$Base = "mysql://root:root@127.0.0.1:3307/dolibarr"
)

# Les commandes natives écrivent sur stderr : on ne veut pas que PowerShell les
# transforme en erreurs fatales. On décide sur les codes de sortie, explicitement.
$ErrorActionPreference = "Continue"
$racine = Split-Path -Parent $PSScriptRoot
Set-Location $racine

$env:CARGO_TARGET_DIR = "C:/Users/Maxime/.cargo-target/cordeau"
$env:DOLIBARR_DSN = $Base

# Les tests qui touchent MySQL partagent **une seule base** : lances en
# parallele, deux tests s'effacent mutuellement leurs lignes (`nettoyer()`
# efface toute l'entite) et deux insertions de la meme reference de recette
# entrent en collision. Les echecs sont alors aleatoires et n'ont rien a voir
# avec le code teste. Une base, un fil de tests.
$env:RUST_TEST_THREADS = "1"

$echecs = @()
function Etape($nom, $bloc) {
    Write-Host "  $nom ... " -NoNewline
    try {
        & $bloc
        if ($LASTEXITCODE -ne 0) { throw "code de sortie $LASTEXITCODE" }
        Write-Host "OK" -ForegroundColor Green
    } catch {
        Write-Host "ECHEC : $_" -ForegroundColor Red
        $script:echecs += $nom
    }
}

# L'hote et le port sont extraits du DSN : sans base, les tests se sautent
# silencieusement (pool_or_skip), donc on refuse de faire semblant.
function BaseJoignable($dsn) {
    if ($dsn -notmatch '@(?<hote>[^:/]+):(?<port>\d+)/') { return $false }
    $h = $Matches['hote']; $p = [int]$Matches['port']
    try {
        $c = New-Object System.Net.Sockets.TcpClient
        $t = $c.ConnectAsync($h, $p)
        $ok = $t.Wait(1500) -and $c.Connected
        $c.Close()
        return $ok
    } catch { return $false }
}

Write-Host "Verification avant commit (cordeau)" -ForegroundColor Cyan

Etape "formatage (cargo fmt)" {
    cargo fmt --all -- --check
}

Etape "clippy -D warnings" {
    cargo clippy --workspace --all-targets -- -D warnings
}

if ($Complet) {
    Write-Host ""
    & (Join-Path $PSScriptRoot "verifier.ps1") -Base $Base
    if ($LASTEXITCODE -ne 0) { $echecs += "verifier.ps1" }
} elseif (-not $Rapide) {
    if (BaseJoignable $Base) {
        Etape "tests (cargo test --workspace)" {
            # Les tests ecrivent dans la vraie base : deux suites en parallele se
            # marchent dessus (references, identifiants, tiers). On serialise.
            $verrou = Join-Path $env:TEMP "cordeau-tests.lock"
            $flux = $null
            $attente = [Diagnostics.Stopwatch]::StartNew()
            while (-not $flux) {
                try {
                    $flux = [IO.File]::Open($verrou, 'OpenOrCreate', 'ReadWrite', 'None')
                } catch {
                    if ($attente.Elapsed.TotalSeconds -gt 300) { throw "une autre suite de tests tourne depuis 5 min ($verrou)" }
                    if ($attente.Elapsed.TotalSeconds -lt 2) { Write-Host "`n    une autre suite tourne, attente..." -ForegroundColor Yellow -NoNewline }
                    Start-Sleep -Milliseconds 500
                }
            }
            try {
                $sortie = & cargo test --workspace 2>&1 | Out-String -Stream
                $code = $LASTEXITCODE
                $ko = ($sortie | Select-String -Pattern "^test result: FAILED" -CaseSensitive).Count
                $total = 0
                foreach ($l in $sortie) {
                    if ($l -match "^test result: ok\. (\d+) passed") { $total += [int]$Matches[1] }
                    if ($l -match "^test result: FAILED") { Write-Host "`n    $l" -NoNewline }
                }
                if ($ko -gt 0) {
                    $sortie | Select-String -Pattern "^failures:" -CaseSensitive -Context 0,4 | ForEach-Object {
                        $_.Context.PostContext | ForEach-Object { Write-Host "`n    $_" -NoNewline }
                    }
                    throw "$ko suite(s) en échec (code $code)"
                }
                Write-Host "`n    $total tests " -NoNewline
            } finally {
                $flux.Dispose()
                Remove-Item $verrou -ErrorAction SilentlyContinue
            }
        }
    } else {
        Write-Host "  tests ... SAUTES (base injoignable sur $Base)" -ForegroundColor Yellow
        Write-Host "    Le commit passe, mais les tests n'ont pas ete verifies." -ForegroundColor Yellow
        Write-Host "    Demarrer la base (docker) puis relancer : .\tools\pre-commit.ps1" -ForegroundColor Yellow
    }
}

Write-Host ""
if ($echecs.Count -eq 0) {
    Write-Host "VERT" -ForegroundColor Green
    exit 0
}
Write-Host ("COMMIT REFUSE : " + ($echecs -join ", ")) -ForegroundColor Red
Write-Host "Corriger, ou forcer avec : git commit --no-verify" -ForegroundColor Yellow
exit 1
