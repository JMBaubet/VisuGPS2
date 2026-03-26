# Script de création de la structure pour Windows
# Crée l'arborescence du projet et les fichiers de base

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "Création de la structure du projet (Windows)" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "[INFO] Création des dossiers..." -ForegroundColor Blue

# Créer la structure de dossiers
New-Item -ItemType Directory -Force -Path "src\router" | Out-Null
New-Item -ItemType Directory -Force -Path "src\stores" | Out-Null
New-Item -ItemType Directory -Force -Path "src\plugins" | Out-Null
New-Item -ItemType Directory -Force -Path "src\views" | Out-Null
New-Item -ItemType Directory -Force -Path "src\components" | Out-Null
New-Item -ItemType Directory -Force -Path "src\assets\styles" | Out-Null

Write-Host "[OK] Dossiers créés" -ForegroundColor Green
Write-Host ""

Write-Host "[INFO] Création des fichiers de base..." -ForegroundColor Blue
Write-Host "Utiliser le script bash ou copier les fichiers manuellement" -ForegroundColor Yellow
Write-Host ""

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "[OK] Structure de base créée" -ForegroundColor Green
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Note: Les fichiers sources doivent être créés via le script bash" -ForegroundColor Yellow
Write-Host "ou utilisez Git Bash sous Windows pour exécuter create-structure.sh" -ForegroundColor Yellow
