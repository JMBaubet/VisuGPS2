# Contexte du Projet - VisuGPS2

> **Documentation destinée à Claude Code et aux développeurs**
> Ce fichier fournit le contexte complet du projet pour faciliter le développement assisté par IA.

## Vue d'ensemble

VisuGPS2 est une **application desktop multiplateformes** basée sur Tauri + Vue 3 + Vuetify, conçue pour fonctionner en dual-screen avec communication inter-fenêtres et synchronisation de thème.

### Objectifs du projet
- Créer une application desktop native multi-fenêtres
- Détecter automatiquement la configuration multi-écran
- Ouvrir une fenêtre secondaire sur l'écran opposé
- Synchroniser l'interface et l'état entre fenêtres
- Gérer les affichages (resolution, position, scale factor)

### Caractéristiques principales
1. **Dual-screen support** : Détection automatique et placement sur écrans différents
2. **Synchronisation inter-fenêtres** : Thème dark/light et communication d'événements
3. **Modularité Rust** : Code organisé en modules (display.rs, lib.rs)
4. **Multi-plateforme** : macOS (NSScreen) et Windows (Tauri API, Win32)
5. **Architecture clean** : Séparation claire des responsabilités

## Stack technique

### Frontend
- **Vue 3.4+** avec Composition API et `<script setup>`
- **TypeScript** pour le typage statique
- **Vuetify 3.5+** pour les composants UI Material Design
- **Pinia** pour la gestion d'état (setup stores pattern)
- **Vue Router 4.2+** pour la navigation
- **Vite** comme build tool

### Backend/Desktop
- **Tauri 2.x** pour l'application desktop native
- **Rust** pour le backend Tauri (code modulaire)
- **objc2** + **objc2-app-kit** : Détection NSScreen sur macOS
- **windows crate** : Win32 API pour Windows

### Outils
- **npm** comme gestionnaire de packages
- **Sass** pour les styles (via Vuetify)
- **vite-plugin-vuetify** pour l'auto-import des composants
- **cargo** comme gestionnaire de dépendances Rust

## Architecture du projet

### Structure des fichiers

```
VisuGPS2/
├── src/                       # Code source frontend (Vue 3 + TypeScript)
│   ├── router/
│   │   └── index.ts          # Routes (Accueil, Visualisation, EditionCamera, ScreenBis)
│   ├── stores/
│   │   ├── index.ts          # Configuration Pinia
│   │   └── app.ts            # Store app (thème, displays, loadDisplays)
│   ├── plugins/
│   │   └── vuetify.ts        # Configuration Vuetify
│   ├── views/                # Pages de l'application
│   │   ├── Accueil.vue       # Fenêtre principale avec comm. inter-fenêtres
│   │   ├── ScreenBis.vue     # Fenêtre secondaire
│   │   ├── EditionCamera.vue # Page caméra
│   │   └── Visualisation.vue # Page visualisation
│   ├── components/           # Composants réutilisables
│   ├── assets/               # Images, styles
│   ├── App.vue              # Layout racine (détection multi-fenêtres)
│   └── main.ts              # Point d'entrée
│
├── src-tauri/                # Code Rust (Tauri 2.x)
│   ├── src/
│   │   ├── lib.rs            # Point d'entrée, open_second_window, run()
│   │   ├── display.rs        # Détection écrans (MonitorInfo, get_displays)
│   │   └── main.rs           # Auto-généré
│   ├── capabilities/
│   │   └── default.json      # Permissions fenêtres (main, screen-bis)
│   ├── icons/                # Icônes application
│   ├── Cargo.toml            # Dépendances Rust (objc2, windows, etc.)
│   └── tauri.conf.json       # Config Tauri (windows, build, security)
│
├── docs/                     # Documentation
│   ├── CONTEXT.md           # Ce fichier
│   └── ARCHITECTURE.md       # Architecture détaillée
│
├── .claude/                  # Configuration Claude Code
│   └── worktrees/            # Branches de travail isolées
│       └── condescending-feistel/  # Branche de développement
│
├── Configuration
│   ├── vite.config.ts       # Configuration Vite (allowedHosts)
│   ├── tsconfig.json        # Configuration TypeScript
│   ├── package.json         # Dépendances npm
│   ├── .gitignore
│   └── .env.example
│
└── Documentation utilisateur
    ├── README.md
    └── QUICKSTART.md
```

## Conventions de code

### Vue/TypeScript

1. **Composition API uniquement** avec `<script setup lang="ts">`
2. **Imports explicites** : toujours importer ce qui est utilisé
3. **Typage fort** : utiliser TypeScript partout
4. **Pas de Options API** : tout en Composition API

### Stores Pinia

- **Setup Stores pattern** (fonction avec `ref` et `computed`)
- Pas de syntaxe `defineStore({ state, actions, getters })`
- Un store = un fichier dans `src/stores/`

Exemple :
```typescript
export const useMyStore = defineStore('mystore', () => {
  const data = ref<string>('')
  const computed = computed(() => data.value.toUpperCase())

  function action() {
    data.value = 'new value'
  }

  return { data, computed, action }
})
```

### Composants Vue

- **Single File Components** (.vue)
- Template, script, style dans cet ordre
- Pas de scoped styles sauf cas particulier (Vuetify gère les styles)

Exemple :
```vue
<template>
  <v-card>
    <v-card-title>{{ title }}</v-card-title>
  </v-card>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const title = ref('Mon titre')
</script>

<style>
/* Styles globaux si nécessaire */
</style>
```

### Vuetify

- **Auto-import activé** via vite-plugin-vuetify
- Pas besoin d'importer les composants `v-*`
- Utiliser la syntaxe v3 (pas de `v-slot:prepend` mais `<template v-slot:prepend>`)
- Icônes MDI : `mdi-icon-name`

### Router

- **Routes déclarées dans** `src/router/index.ts`
- Lazy loading pour les routes non critiques : `component: () => import('../views/Page.vue')`
- Navigation avec `useRouter()` et `router.push()`

## Patterns et bonnes pratiques

### 1. Séparation des responsabilités

- **Views** : Pages complètes avec layout
- **Components** : Composants réutilisables sans logique métier
- **Stores** : Logique métier et état partagé
- **Router** : Configuration des routes uniquement

### 2. Gestion d'état

- État local : `ref()` dans le composant
- État partagé : Store Pinia
- Props pour la communication parent → enfant
- Emits pour enfant → parent

### 3. Styling

- Utiliser les composants Vuetify autant que possible
- Pas de CSS custom sauf nécessaire
- Classes utilitaires Vuetify pour spacing : `ma-4`, `pa-2`, etc.

### 4. Communication Tauri

**Invoquer une commande Rust** :
```typescript
import { invoke } from '@tauri-apps/api/core'

// Appeler get_displays pour détecter les écrans
const displays = await invoke<MonitorInfo[]>('get_displays')

// Appeler open_second_window pour ouvrir ScreenBis
await invoke('open_second_window')
```

**Commandes disponibles** :
- `get_displays()` : Retourne la liste des écrans (MonitorInfo[])
- `open_second_window()` : Ouvre la fenêtre ScreenBis sur l'écran opposé
- `exit_app()` : Ferme et quitte proprement l'application depuis le backend Rust

### 5. Communication inter-fenêtres

Les fenêtres communiquent via les événements Tauri :

```typescript
import { emit, listen } from '@tauri-apps/api/event'

// Fenêtre Accueil : Envoyer un nombre aléatoire à ScreenBis
const value = Math.floor(Math.random() * 100) + 1
await emit('accueil-to-screenbis', value)

// Fenêtre ScreenBis : Écouter les événements d'Accueil
const unlisten = await listen<number>('accueil-to-screenbis', (event) => {
  receivedValue.value = event.payload
})
```

**Synchronisation du thème** :
- Accueil émet 'theme-changed' avec isDarkMode.value
- Toutes les fenêtres écoutent et mettent à jour leur appStore.isDarkMode
- Le thème Vuetify se met à jour automatiquement via :theme="appStore.theme"

## Scripts d'installation

### Workflow d'installation

```
setup.sh
  ↓
  ├─→ check-requirements.sh    (Vérifie Node, Rust, etc.)
  ├─→ install-dependencies.sh  (npm install + packages)
  ├─→ create-structure.sh      (Crée dossiers + fichiers)
  └─→ configure-app.sh         (Configure Vite + TS)
```

### Modifications des scripts

Si vous modifiez les scripts :
1. **check-requirements.sh** : Ajouter de nouveaux prérequis
2. **install-dependencies.sh** : Ajouter de nouvelles dépendances npm
3. **create-structure.sh** : Modifier la structure ou les fichiers générés
4. **configure-app.sh** : Changer les configs Vite/TypeScript

⚠️ **Important** : Maintenir la compatibilité macOS/Windows (bash + PowerShell)

## Application exemple

L'application générée par `setup.sh` contient :

### Pages
- **Home** (`/`) : Page d'accueil avec liste des technologies
- **About** (`/about`) : Page à propos avec infos sur le template

### Fonctionnalités
- Navigation drawer (menu hamburger)
- Toggle thème dark/light (via store Pinia)
- App bar avec titre
- Routing fonctionnel

### Store exemple
`src/stores/app.ts` gère le thème :
- `isDarkMode` : état du thème
- `theme` : computed qui retourne 'dark' ou 'light'
- `toggleDarkMode()` : action pour basculer

## Variables d'environnement

Fichier `.env` (copier depuis `.env.example`) :

```env
VITE_APP_NAME="Mon Application"
VITE_API_URL="http://localhost:3000"
```

Utilisation dans le code :
```typescript
const appName = import.meta.env.VITE_APP_NAME
```

⚠️ Seules les variables préfixées `VITE_` sont accessibles côté frontend.

## Points d'extension

### Ajouter une nouvelle page

1. Créer `src/views/MaPage.vue`
2. Ajouter la route dans `src/router/index.ts`
3. Ajouter un lien dans `src/App.vue` (navigation drawer)

### Ajouter un nouveau store

1. Créer `src/stores/monstore.ts`
2. Utiliser le pattern setup store
3. Importer et utiliser dans les composants

### Ajouter une dépendance

1. Modifier `install-dependencies.sh` et `install-dependencies.ps1`
2. Ajouter `npm install package-name`
3. Documenter dans README.md

### Ajouter une commande Tauri

1. Modifier `src-tauri/src/main.rs`
2. Définir `#[tauri::command]`
3. Ajouter dans `.invoke_handler()`
4. Appeler avec `invoke()` côté frontend

## Configuration Tauri

Fichier `src-tauri/tauri.conf.json` :

```json
{
  "package": {
    "productName": "mon-app",
    "version": "1.0.0"
  },
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devPath": "http://localhost:1420",
    "distDir": "../dist"
  },
  "tauri": {
    "windows": [
      {
        "title": "Mon Application",
        "width": 800,
        "height": 600
      }
    ]
  }
}
```

## Réutilisation du template

Pour créer un nouveau projet :

```bash
# 1. Copier le template
cp -r "Base Tauri" "MonNouveauProjet"
cd "MonNouveauProjet"

# 2. Nettoyer les fichiers générés
rm -rf node_modules src-tauri package-lock.json

# 3. Réinstaller
./setup.sh

# 4. Personnaliser
# - src-tauri/tauri.conf.json (productName, version)
# - package.json (name, version, description)
# - src/App.vue (titre)
# - src-tauri/icons/ (icônes de l'app)
```

## Dépannage pour Claude Code

### Erreurs communes

1. **Import non résolu** : Vérifier que le fichier existe et le chemin est correct
2. **Composant Vuetify non reconnu** : C'est normal, ils sont auto-importés
3. **Store non trouvé** : Vérifier que le store est exporté avec `export const`
4. **Route ne fonctionne pas** : Vérifier le `name` dans router et `router-link :to`

### Commandes utiles

```bash
# Développement
./dev.sh

# Vérifier les erreurs TypeScript
npx tsc --noEmit

# Voir les routes
cat src/router/index.ts

# Voir les stores
ls src/stores/
```

## Philosophie du code

Ce template privilégie :
- ✅ **Simplicité** plutôt que complexité
- ✅ **Convention** plutôt que configuration
- ✅ **Lisibilité** plutôt qu'optimisation prématurée
- ✅ **Documentation** plutôt que commentaires dans le code
- ✅ **Réutilisabilité** plutôt que cas particuliers

## Notes importantes pour Claude Code

1. **Ne pas ajouter ESLint/Prettier** : C'est un choix délibéré (environnement minimal)
2. **Ne pas modifier les scripts sans raison** : Ils sont testés et fonctionnels
3. **Toujours documenter** : Nouveaux stores, routes, composants
4. **Respecter la structure** : Ne pas créer de nouveaux dossiers sans justification
5. **TypeScript strict** : Toujours typer les paramètres et retours

## Ressources

- Tauri : https://tauri.app/
- Vue 3 : https://vuejs.org/
- Vuetify 3 : https://vuetifyjs.com/
- Pinia : https://pinia.vuejs.org/
- Vue Router : https://router.vuejs.org/

## Fonctionnalités spécifiques à VisuGPS2

### Dual-screen Support

**Détection automatique** :
1. App.vue charge les displays via `appStore.loadDisplays()` (commande Rust)
2. Si 2+ écrans détectés → `invoke('open_second_window')`
3. Rust vérifie la position de la fenêtre main et place screen-bis sur l'autre écran

**Placement des fenêtres (Configurable)** :
Le choix de l'écran (principal, secondaire) pour chaque fenêtre est paramétrable par l'utilisateur via le panneau des réglages (paramètres de type `monitor_selection` : `Affichage.moniteurs.principal`, `Affichage.moniteurs.secondaire`).
Les valeurs peuvent être des critères génériques (`origin`, `other`, `builtin`, `external`) ou un nom d'écran spécifique détecté.

- **macOS** : Utilise `NSScreen.screens()` pour lister les écrans.
  - Différenciation entre écran interne (`builtin`) et externe (`external`).
  - Position X/Y = frame.origin pour chaque écran
  - Scale factor = backingScaleFactor

- **Windows** : Utilise Tauri `available_monitors()` et Win32 API
  - Active monitor rect détecté au setup avec `GetForegroundWindow()`
  - Les écrans génériques de type "DISPLAY_X" détectés physiquement peuvent être filtrés si besoin.

**MonitorInfo struct** :
```rust
pub struct MonitorInfo {
    pub name: Option<String>,      // Nom de l'écran
    pub position: (i32, i32),      // X, Y coordinates
    pub size: (u32, u32),          // Width, height pixels
    pub scale_factor: f64,         // DPI scaling
}
```

### Communication inter-fenêtres

**Pattern d'implémentation** :
1. Les fenêtres sont déclarées statiquement dans tauri.conf.json (visible: false pour screen-bis)
2. App.vue détecte le label de la fenêtre actuelle avec `getCurrentWindow().label`
3. Router navigue vers le composant approprié
4. Les événements Tauri synchronisent l'état entre fenêtres

**Événements utilisés** :
- `accueil-to-screenbis` : Nombres 1-100 (Accueil → ScreenBis)
- `screenbis-to-accueil` : Nombres 501-600 (ScreenBis → Accueil)
- `theme-changed` : Boolean (isDarkMode) - synchronisé entre tous
- `screenbis-closed` : Émis par ScreenBis lors de sa fermeture pour informer l'AppBar et réinitialiser `appStore.isScreenBisOpen` à `false`

### Architecture Rust modulaire

**display.rs - Détection des écrans**
```rust
pub struct MonitorInfo { ... }
pub fn get_all_monitors() -> Vec<MonitorInfo> { ... }
#[tauri::command]
pub fn get_displays(window: tauri::Window) -> Vec<MonitorInfo> { ... }
```

**lib.rs - Orchestration principale**
```rust
mod display;
use display::get_displays;

#[tauri::command]
async fn open_second_window(app: AppHandle) -> Result<(), String> { ... }

pub fn run() { ... }
```

**Dépendances Rust** :
- `objc2` + `objc2-app-kit` + `objc2-foundation` : Accès NSScreen sur macOS
- `windows` : Win32 API (GetForegroundWindow, GetMonitorInfo)
- `tauri` : APIs principales (Manager, invoke_handler, generate_handler)
- `serde` : Sérialisation MonitorInfo

### Gestion Dynamique des Modes d'Exécution

L'application intègre un système robuste de gestion des modes d'exécution (définis dans un fichier de configuration Lu par Rust et exposé au Frontend) :

1. **Variables du Store** :
   - `activeModeDev` correspond à l'environnement pointé en développement.
   - `activeModeProd` correspond à l'environnement actif en production.

2. **Logique d'Interface (`ModeExecutionCard.vue`)** :
   - Un chip **Actif** s'affiche en vert pour le mode `OPE` et en bleu pour tout autre mode de production actif.
   - Un chip **Actif Dev** (orange) s'affiche uniquement en mode développement pour le mode correspondant à `activeModeDev`.
   - Les boutons d'action (Éditer/Supprimer) sont désactivés/masqués pour le mode `OPE`, ainsi que pour les modes actifs de développement (`activeModeDev`) et de production (`activeModeProd`).

---

**Dernière mise à jour** : 2026-05-26
**Version du projet** : 0.0.1
**Status** : En développement actif
**Fonctionnalités** : Dual-screen, inter-window communication, theme sync, display detection, multi-env execution modes
