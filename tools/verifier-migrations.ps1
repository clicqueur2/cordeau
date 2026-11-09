# Garde-fou des migrations : verifie que les chaines sont completes.
#
# Ne touche a aucune base. Il repond a une seule question, vite : « les trois
# chemins de migration tiennent-ils toujours ? » Une version ajoutee ou un
# script renomme casserait la chaine en silence ; ce script le dit.
#
# Les migrations reelles (12, 19, 23 -> 24) ont ete jouees le 9 novembre 2026 :
# 307 -> 400, 372 -> 406, 399 -> 407 tables. Le detail est dans PLAN-FINITION.md.

$ErrorActionPreference = "Stop"
Push-Location (Join-Path $PSScriptRoot "..")

$migration = "_vendor/dolibarr-src/htdocs/install/mysql/migration"
$binaire = "cargo"
$env:CARGO_TARGET_DIR = "C:/Users/Maxime/.cargo-target/dolibarr-rs"

$departs = @("12.0.0", "19.0.0", "23.0.0")
$attendus = @{ "12.0.0" = 12; "19.0.0" = 5; "23.0.0" = 1 }
$echecs = 0

foreach ($depart in $departs) {
    $sortie = & $binaire run -q -p cordeau -- schema planifier `
        --depuis $depart --vers 24.0.0 --src $migration 2>&1
    $ligne = ($sortie | Select-Object -First 1)
    $attendues = $attendus[$depart]
    if ($ligne -match "^(\d+) etape") {
        $trouvees = [int]$Matches[1]
        if ($trouvees -eq $attendues) {
            "  OK    $depart -> 24.0.0 : $trouvees etapes"
        } else {
            "  ECART $depart -> 24.0.0 : $trouvees etapes au lieu de $attendues"
            $echecs++
        }
    } else {
        "  ECHEC $depart : $ligne"
        $echecs++
    }
}

# Les quatre empreintes doivent etre la : sans elles, `schema verify` n'a pas
# de reference et ne peut rien affirmer.
foreach ($v in @("12.0.5", "19.0.4", "23.0.3", "24.0.1")) {
    $f = "schema-$v.json"
    if (Test-Path $f) {
        "  OK    empreinte $v"
    } else {
        "  ECHEC empreinte $v absente"
        $echecs++
    }
}

Pop-Location
if ($echecs -gt 0) {
    "Migrations : $echecs defaut(s)"
    exit 1
}
"Migrations : les trois chaines tiennent."
