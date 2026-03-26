# Script d'installation des dépendances pour Windows
# Installe Tauri, Vue, Vuetify, Pinia et Vue Router

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "Installation des dépendances (Windows)" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""

# Vérifier si package.json existe
if (Test-Path "package.json") {
    Write-Host "[INFO] package.json existe déjà" -ForegroundColor Blue
    Write-Host "Installation des dépendances existantes..."
    npm install
} else {
    Write-Host "[INFO] Création du projet Tauri avec Vue..." -ForegroundColor Blue

    # Créer un dossier temporaire
    $TempDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ([System.Guid]::NewGuid()))
    Write-Host "[INFO] Utilisation du dossier temporaire: $TempDir" -ForegroundColor Blue

    # Créer le projet Tauri dans le dossier temporaire
    Push-Location $TempDir
    npm create tauri-app@latest . -- --manager npm --template vue-ts --yes
    Pop-Location

    # Copier les fichiers générés vers le dossier courant
    Write-Host "[INFO] Copie des fichiers du projet Tauri..." -ForegroundColor Blue

    # Copier les fichiers (sans écraser les existants)
    if (Test-Path "$TempDir\package.json") { Copy-Item "$TempDir\package.json" . -ErrorAction SilentlyContinue }
    if (Test-Path "$TempDir\package-lock.json") { Copy-Item "$TempDir\package-lock.json" . -ErrorAction SilentlyContinue }
    if (Test-Path "$TempDir\index.html") { Copy-Item "$TempDir\index.html" . -ErrorAction SilentlyContinue }
    if (Test-Path "$TempDir\tsconfig.json") { Copy-Item "$TempDir\tsconfig.json" . -ErrorAction SilentlyContinue }
    if (Test-Path "$TempDir\tsconfig.node.json") { Copy-Item "$TempDir\tsconfig.node.json" . -ErrorAction SilentlyContinue }
    if (Test-Path "$TempDir\vite.config.ts") { Copy-Item "$TempDir\vite.config.ts" . -ErrorAction SilentlyContinue }

    # Copier les dossiers
    if (Test-Path "$TempDir\src") { Copy-Item "$TempDir\src" . -Recurse -ErrorAction SilentlyContinue }
    if (Test-Path "$TempDir\src-tauri") { Copy-Item "$TempDir\src-tauri" . -Recurse -ErrorAction SilentlyContinue }
    if (Test-Path "$TempDir\public") { Copy-Item "$TempDir\public" . -Recurse -ErrorAction SilentlyContinue }

    # Nettoyer le dossier temporaire
    Remove-Item $TempDir -Recurse -Force

    Write-Host "[OK] Projet Tauri créé" -ForegroundColor Green
    Write-Host ""

    Write-Host "[INFO] Installation des dépendances de base..." -ForegroundColor Blue
    npm install
}

Write-Host ""
Write-Host "[INFO] Installation de Vuetify 3..." -ForegroundColor Blue
npm install vuetify@^3.5.0 @mdi/font

Write-Host ""
Write-Host "[INFO] Installation de Pinia..." -ForegroundColor Blue
npm install pinia

Write-Host ""
Write-Host "[INFO] Installation de Vue Router..." -ForegroundColor Blue
npm install vue-router@^4.2.0

Write-Host ""
Write-Host "[INFO] Installation des dépendances de développement..." -ForegroundColor Blue
npm install -D sass vite-plugin-vuetify

Write-Host ""
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "[OK] Toutes les dépendances sont installées" -ForegroundColor Green
Write-Host "==============================================" -ForegroundColor Cyan
