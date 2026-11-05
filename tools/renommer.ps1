# Renomme le produit partout, en une commande.
#
# Le nom du logiciel est defini a deux endroits seulement :
#   1. `crates/app/src/lib.rs`  -> const PRODUIT (l'interface)
#   2. ce script               -> le nom dans les documents et les paquets
#
# Tout le reste lit la constante. Ce script sert pour ce que Rust ne peut pas
# atteindre : le README, la documentation, les scripts, les noms de binaires.
#
# Il ne touche PAS : `llx_*`, `DOLIBARR_DSN`, le schema. Ces noms-la sont la
# compatibilite avec Dolibarr ; les changer casserait la base.

param(
    [Parameter(Mandatory = $true)][string]$Nom,
    [string]$Ancien = "cordeau"
)

$ErrorActionPreference = "Stop"
$racine = Split-Path -Parent $PSScriptRoot
$minuscule = $Ancien.ToLower()
$nouveau = $Nom
$nouveauMinuscule = $Nom.ToLower()

Write-Host "Renommage : $Ancien -> $Nom"

# 1. La constante Rust.
$app = Join-Path $racine "crates\app\src\lib.rs"
$texte = Get-Content $app -Raw -Encoding UTF8
$texte = $texte -replace "pub const PRODUIT: &str = `"$Ancien`";", "pub const PRODUIT: &str = `"$Nom`";"
Set-Content $app -Value $texte -Encoding UTF8 -NoNewline
Write-Host "  const PRODUIT mise a jour"

# 2. Les fichiers de texte : documentation, scripts, manifestes.
$cibles = @()
$cibles += Get-ChildItem $racine -Filter *.md -File
$cibles += Get-ChildItem (Join-Path $racine "tools") -Include *.ps1, *.sh, *.py -File -Recurse
$cibles += Get-ChildItem $racine -Filter installer.ps1 -File
$cibles += Get-ChildItem (Join-Path $racine "crates") -Filter Cargo.toml -File -Recurse
$cibles += Get-ChildItem $racine -Filter Cargo.toml -File

$touches = 0
foreach ($f in $cibles) {
    $t = Get-Content $f.FullName -Raw -Encoding UTF8
    $avant = $t
    # Les noms de binaires et de paquets d'abord : `cordeau-web` ne doit pas
    # devenir `nouveau-web` par accident de prefixe.
    $t = $t -replace "$minuscule-", "$nouveauMinuscule-"
    $t = $t -replace $Ancien, $nouveau
    if ($t -ne $avant) {
        Set-Content $f.FullName -Value $t -Encoding UTF8 -NoNewline
        $touches++
    }
}

# 3. Le dossier du depot lui-meme (il porte le nom).
Write-Host "  $touches fichier(s) de texte renommes"
Write-Host ""
Write-Host "Reste a faire a la main :"
Write-Host "  - renommer le dossier du depot si tu veux qu'il porte le nom"
Write-Host "  - verifier : cargo build --workspace, puis tools\verifier.ps1"
