#!/bin/bash

# Script principal d'installation
# Orchestre tous les scripts d'installation et de configuration

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo ""
echo "================================================"
echo "Installation Template Tauri + Vue + Vuetify"
echo "================================================"
echo ""

# Rendre les scripts exécutables
chmod +x check-requirements.sh
chmod +x install-dependencies.sh
chmod +x create-structure.sh
chmod +x configure-app.sh
chmod +x dev.sh
chmod +x build.sh

echo -e "${BLUE}[1/4]${NC} Vérification des prérequis..."
echo ""

# Étape 1 : Vérification des prérequis
if ! ./check-requirements.sh; then
    echo ""
    echo -e "${RED}[ERREUR]${NC} Les prérequis ne sont pas satisfaits"
    echo "Veuillez installer les outils manquants et réessayer"
    exit 1
fi

echo ""
echo -e "${BLUE}[2/4]${NC} Installation des dépendances..."
echo ""

# Étape 2 : Installation des dépendances
./install-dependencies.sh

echo ""
echo -e "${BLUE}[3/4]${NC} Création de la structure du projet..."
echo ""

# Étape 3 : Création de la structure
./create-structure.sh

echo ""
echo -e "${BLUE}[4/4]${NC} Configuration de l'application..."
echo ""

# Étape 4 : Configuration
./configure-app.sh

echo ""
echo "================================================"
echo -e "${GREEN}Installation terminée avec succès !${NC}"
echo "================================================"
echo ""
echo "Pour démarrer l'application :"
echo "  ./dev.sh"
echo ""
echo "Pour compiler l'application :"
echo "  ./build.sh"
echo ""
echo "Structure créée :"
echo "  src/router/     - Configuration Vue Router"
echo "  src/stores/     - Stores Pinia"
echo "  src/views/      - Pages de l'application"
echo "  src/components/ - Composants réutilisables"
echo "  src/plugins/    - Plugins (Vuetify, etc.)"
echo ""
