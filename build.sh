#!/bin/bash

# Script de build pour la production
# Compile l'application Tauri

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo "=============================================="
echo "Build de production"
echo "=============================================="
echo ""

# Vérifier que le projet est initialisé
if [ ! -d "src-tauri" ]; then
    echo -e "${RED}[ERREUR]${NC} Le projet Tauri n'est pas initialisé"
    echo "Exécutez d'abord: ./setup.sh"
    exit 1
fi

echo -e "${YELLOW}[ATTENTION]${NC} Cette opération peut prendre plusieurs minutes"
echo ""

echo -e "${BLUE}[INFO]${NC} Compilation en cours..."
echo ""

# Build de production
npm run tauri build

echo ""
echo "=============================================="
echo -e "${GREEN}[OK]${NC} Build terminé avec succès"
echo "=============================================="
echo ""
echo "Les fichiers se trouvent dans:"
echo "  src-tauri/target/release/bundle/"
echo ""
