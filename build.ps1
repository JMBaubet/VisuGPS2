# Script de build pour Windows

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "Build de production (Windows)" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""

if (-not (Test-Path "src-tauri")) {
    Write-Host "[ERREUR] Le projet Tauri n'est pas initialisé" -ForegroundColor Red
    Write-Host "Exécutez d'abord: .\setup.ps1"
    exit 1
}

Write-Host "[ATTENTION] Cette opération peut prendre plusieurs minutes" -ForegroundColor Yellow
Write-Host ""

Write-Host "[INFO] Compilation en cours..." -ForegroundColor Blue
Write-Host ""

npm run tauri build

Write-Host ""
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "[OK] Build terminé avec succès" -ForegroundColor Green
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Les fichiers se trouvent dans:"
Write-Host "  src-tauri\target\release\bundle\"
Write-Host ""
