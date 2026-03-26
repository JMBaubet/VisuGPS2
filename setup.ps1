# Script principal d'installation pour Windows
# Orchestre tous les scripts d'installation

Write-Host ""
Write-Host "================================================" -ForegroundColor Cyan
Write-Host "Installation Template Tauri + Vue + Vuetify" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "[1/4] Vérification des prérequis..." -ForegroundColor Blue
Write-Host ""

# Étape 1 : Vérification
& .\check-requirements.ps1
if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Host "[ERREUR] Les prérequis ne sont pas satisfaits" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "[2/4] Installation des dépendances..." -ForegroundColor Blue
Write-Host ""

# Étape 2 : Installation
& .\install-dependencies.ps1

Write-Host ""
Write-Host "[3/4] Création de la structure..." -ForegroundColor Blue
Write-Host ""

# Étape 3 : Structure
& .\create-structure.ps1

Write-Host ""
Write-Host "[4/4] Configuration..." -ForegroundColor Blue
Write-Host ""

# Étape 4 : Configuration
& .\configure-app.ps1

Write-Host ""
Write-Host "================================================" -ForegroundColor Cyan
Write-Host "Installation terminée !" -ForegroundColor Green
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Note: Pour une installation complète, utilisez Git Bash sous Windows" -ForegroundColor Yellow
Write-Host "et exécutez ./setup.sh au lieu de setup.ps1" -ForegroundColor Yellow
Write-Host ""
Write-Host "Pour démarrer l'application :"
Write-Host "  .\dev.ps1"
Write-Host ""
