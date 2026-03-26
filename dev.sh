#!/bin/bash

# Script de démarrage en mode développement
# Lance l'application Tauri avec hot-reload

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo "=============================================="
echo "Démarrage en mode développement"
echo "=============================================="
echo ""

# Vérifier que le projet est initialisé
if [ ! -d "node_modules" ]; then
    echo -e "${RED}[ERREUR]${NC} node_modules n'existe pas"
    echo "Exécutez d'abord: ./setup.sh"
    exit 1
fi

if [ ! -d "src-tauri" ]; then
    echo -e "${RED}[ERREUR]${NC} Le projet Tauri n'est pas initialisé"
    echo "Exécutez d'abord: ./setup.sh"
    exit 1
fi

echo -e "${BLUE}[INFO]${NC} Démarrage du serveur de développement..."
echo ""
echo "L'application va s'ouvrir dans quelques instants"
echo "Hot-reload activé : les modifications sont appliquées automatiquement"
echo ""
echo -e "${YELLOW}Appuyez sur Ctrl+C pour arrêter${NC}"
echo ""

# Lancer Tauri en mode dev
npm run tauri dev
