# Script de vérification des prérequis pour Windows
# Utilisation: .\check-requirements.ps1

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "Vérification des prérequis (Windows)" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""

$allOk = $true

function Check-Command {
    param(
        [string]$Command,
        [string]$Name,
        [string]$MinVersion,
        [string]$InstallUrl
    )

    Write-Host "Vérification de $Name... " -NoNewline

    try {
        $version = & $Command --version 2>&1 | Select-Object -First 1
        Write-Host "✓ Installé" -ForegroundColor Green
        Write-Host "  Version: $version"
        if ($MinVersion) {
            Write-Host "  Minimum requis: $MinVersion"
        }
    }
    catch {
        Write-Host "✗ Non installé" -ForegroundColor Red
        if ($InstallUrl) {
            Write-Host "  Installation: $InstallUrl"
        }
        $script:allOk = $false
    }
    Write-Host ""
}

# Vérifications
Check-Command -Command "node" -Name "Node.js" -MinVersion "v18.0.0" -InstallUrl "https://nodejs.org/"
Check-Command -Command "npm" -Name "npm" -MinVersion "9.0.0"
Check-Command -Command "rustc" -Name "Rust" -InstallUrl "https://rustup.rs/"
Check-Command -Command "cargo" -Name "Cargo"

# Vérification Visual Studio Build Tools
Write-Host "Vérification Visual Studio Build Tools... " -NoNewline
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vsWhere) {
    Write-Host "✓ Installé" -ForegroundColor Green
} else {
    Write-Host "✗ Non installé" -ForegroundColor Yellow
    Write-Host "  Recommandé pour Tauri sur Windows"
    Write-Host "  Installation: https://visualstudio.microsoft.com/downloads/"
}
Write-Host ""

# Résumé
Write-Host "==============================================" -ForegroundColor Cyan
if ($allOk) {
    Write-Host "✓ Tous les prérequis sont satisfaits" -ForegroundColor Green
    exit 0
} else {
    Write-Host "✗ Certains prérequis manquent" -ForegroundColor Red
    Write-Host "Consultez le README.md pour les instructions"
    exit 1
}
Write-Host "==============================================" -ForegroundColor Cyan
