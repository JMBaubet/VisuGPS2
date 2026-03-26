#!/bin/bash

# Script d'installation des dépendances
# Installe Tauri, Vue, Vuetify, Pinia et Vue Router

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

echo "=============================================="
echo "Installation des dépendances"
echo "=============================================="
echo ""

# Vérifier si package.json existe déjà
if [ -f "package.json" ]; then
    echo -e "${BLUE}[INFO]${NC} package.json existe déjà"
    echo "Installation des dépendances existantes..."
    npm install
else
    echo -e "${BLUE}[INFO]${NC} Création du projet Tauri avec Vue..."

    # Créer un dossier temporaire pour le projet Tauri
    TEMP_DIR=$(mktemp -d)
    echo -e "${BLUE}[INFO]${NC} Utilisation du dossier temporaire: $TEMP_DIR"

    # Créer le projet Tauri dans le dossier temporaire
    (cd "$TEMP_DIR" && npm create tauri-app@latest . -- --manager npm --template vue-ts --yes)

    # Déplacer les fichiers générés vers le dossier courant
    echo -e "${BLUE}[INFO]${NC} Copie des fichiers du projet Tauri..."

    # Copier les fichiers et dossiers (en évitant d'écraser les fichiers existants comme README.md)
    cp -n "$TEMP_DIR/package.json" . 2>/dev/null || true
    cp -n "$TEMP_DIR/package-lock.json" . 2>/dev/null || true
    cp -n "$TEMP_DIR/index.html" . 2>/dev/null || true
    cp -n "$TEMP_DIR/tsconfig.json" . 2>/dev/null || true
    cp -n "$TEMP_DIR/tsconfig.node.json" . 2>/dev/null || true
    cp -n "$TEMP_DIR/vite.config.ts" . 2>/dev/null || true

    # Copier les dossiers
    [ -d "$TEMP_DIR/src" ] && cp -r "$TEMP_DIR/src" . 2>/dev/null || true
    [ -d "$TEMP_DIR/src-tauri" ] && cp -r "$TEMP_DIR/src-tauri" . 2>/dev/null || true
    [ -d "$TEMP_DIR/public" ] && cp -r "$TEMP_DIR/public" . 2>/dev/null || true

    # Nettoyer le dossier temporaire
    rm -rf "$TEMP_DIR"

    echo -e "${GREEN}[OK]${NC} Projet Tauri créé"
    echo ""

    # Installer les dépendances de base
    echo -e "${BLUE}[INFO]${NC} Installation des dépendances de base..."
    npm install
fi

echo ""
echo -e "${BLUE}[INFO]${NC} Installation de Vuetify 3..."
npm install vuetify@^3.5.0 @mdi/font

echo ""
echo -e "${BLUE}[INFO]${NC} Installation de Pinia..."
npm install pinia

echo ""
echo -e "${BLUE}[INFO]${NC} Installation de Vue Router..."
npm install vue-router@^4.2.0

echo ""
echo -e "${BLUE}[INFO]${NC} Installation des dépendances de développement..."
npm install -D sass vite-plugin-vuetify

echo ""
echo "=============================================="
echo -e "${GREEN}[OK]${NC} Toutes les dépendances sont installées"
echo "=============================================="
