# Script de démarrage en mode développement pour Windows

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "Démarrage en mode développement (Windows)" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""

if (-not (Test-Path "node_modules")) {
    Write-Host "[ERREUR] node_modules n'existe pas" -ForegroundColor Red
    Write-Host "Exécutez d'abord: .\setup.ps1"
    exit 1
}

if (-not (Test-Path "src-tauri")) {
    Write-Host "[ERREUR] Le projet Tauri n'est pas initialisé" -ForegroundColor Red
    Write-Host "Exécutez d'abord: .\setup.ps1"
    exit 1
}

Write-Host "[INFO] Démarrage du serveur de développement..." -ForegroundColor Blue
Write-Host ""
Write-Host "L'application va s'ouvrir dans quelques instants"
Write-Host "Hot-reload activé"
Write-Host ""
Write-Host "Appuyez sur Ctrl+C pour arrêter" -ForegroundColor Yellow
Write-Host ""

npm run tauri dev
