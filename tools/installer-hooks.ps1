# Installe les hooks git de ce dépôt (pre-commit + pre-push).
#
#   .\tools\installer-hooks.ps1
#
# À relancer après un clone : .git/hooks n'est pas versionné.
# Les scripts réellement exécutés vivent dans tools/ et sont, eux, versionnés.

$ErrorActionPreference = "Stop"
$racine = Split-Path -Parent $PSScriptRoot
Set-Location $racine

$hooks = Join-Path $racine ".git/hooks"
if (-not (Test-Path $hooks)) { throw "pas de dépôt git ici : $racine" }

$preCommit = @'
#!/bin/sh
# Installe par tools/installer-hooks.ps1 - verifie le code avant chaque commit.
# Contournement : git commit --no-verify
cd "$(git rev-parse --show-toplevel)" || exit 1
exec powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/pre-commit.ps1
'@

$prePush = @'
#!/bin/sh
# Installe par tools/installer-hooks.ps1 - verification complete avant un push.
# Contournement : git push --no-verify
cd "$(git rev-parse --show-toplevel)" || exit 1
exec powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/pre-commit.ps1 -Complet
'@

# LF obligatoire : un hook en CRLF casse sous sh ("bad interpreter").
[IO.File]::WriteAllText((Join-Path $hooks "pre-commit"), ($preCommit -replace "`r`n", "`n"))
[IO.File]::WriteAllText((Join-Path $hooks "pre-push"), ($prePush -replace "`r`n", "`n"))

Write-Host "Hooks installes :" -ForegroundColor Green
Write-Host "  pre-commit  -> tools/pre-commit.ps1            (fmt + clippy + tests)"
Write-Host "  pre-push    -> tools/pre-commit.ps1 -Complet   (verifier.ps1 au complet)"
Write-Host ""
Write-Host "Essai a blanc : .\tools\pre-commit.ps1"
