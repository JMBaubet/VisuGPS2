#!/bin/bash

# Script de configuration de l'application
# Configure Vite, TypeScript et autres fichiers de config

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "=============================================="
echo "Configuration de l'application"
echo "=============================================="
echo ""

echo -e "${BLUE}[INFO]${NC} Configuration de Vite..."

# Créer/Mettre à jour vite.config.ts
cat > vite.config.ts << 'EOF'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vuetify from 'vite-plugin-vuetify'

export default defineConfig({
  plugins: [
    vue(),
    vuetify({ autoImport: true }),
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
})
EOF

echo -e "${GREEN}[OK]${NC} Vite configuré"
echo ""

echo -e "${BLUE}[INFO]${NC} Configuration de TypeScript..."

# Créer/Mettre à jour tsconfig.json
cat > tsconfig.json << 'EOF'
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "module": "ESNext",
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "skipLibCheck": true,
    "types": ["vite/client"],

    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "preserve",

    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src/**/*.ts", "src/**/*.d.ts", "src/**/*.tsx", "src/**/*.vue"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
EOF

echo -e "${GREEN}[OK]${NC} TypeScript configuré"
echo ""

echo "=============================================="
echo -e "${GREEN}[OK]${NC} Configuration terminée"
echo "=============================================="
