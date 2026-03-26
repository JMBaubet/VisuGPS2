#!/bin/bash

# Script de vérification des prérequis
# Compatible macOS et Linux
# Pour Windows, utiliser check-requirements.ps1

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "=============================================="
echo "Vérification des prérequis"
echo "=============================================="
echo ""

all_ok=true

# Fonction de vérification
check_command() {
    local command=$1
    local name=$2
    local min_version=$3
    local install_url=$4

    echo -n "Vérification de $name... "

    if command -v $command &> /dev/null; then
        version=$($command --version 2>&1 | head -n1)
        echo -e "${GREEN}✓${NC} Installé"
        echo "  Version: $version"
        if [ ! -z "$min_version" ]; then
            echo "  Minimum requis: $min_version"
        fi
    else
        echo -e "${RED}✗${NC} Non installé"
        if [ ! -z "$install_url" ]; then
            echo "  Installation: $install_url"
        fi
        all_ok=false
    fi
    echo ""
}

# Vérifications principales
check_command "node" "Node.js" "v18.0.0" "https://nodejs.org/"
check_command "npm" "npm" "9.0.0" ""
check_command "rustc" "Rust" "" "https://rustup.rs/"
check_command "cargo" "Cargo" "" ""

# Vérification macOS
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo -n "Vérification Xcode Command Line Tools... "
    if xcode-select -p &> /dev/null; then
        echo -e "${GREEN}✓${NC} Installé"
    else
        echo -e "${RED}✗${NC} Non installé"
        echo "  Installation: xcode-select --install"
        all_ok=false
    fi
    echo ""
fi

# Résumé
echo "=============================================="
if [ "$all_ok" = true ]; then
    echo -e "${GREEN}✓ Tous les prérequis sont satisfaits${NC}"
    exit 0
else
    echo -e "${RED}✗ Certains prérequis manquent${NC}"
    echo "Consultez le README.md pour les instructions"
    exit 1
fi
echo "=============================================="
