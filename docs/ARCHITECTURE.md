# Architecture du Template

> Documentation technique détaillée de l'architecture du projet

## Vue d'ensemble

Ce template suit une architecture en couches séparant clairement les responsabilités :

```
┌─────────────────────────────────────────────┐
│           Vue 3 + Vuetify (UI)             │
├─────────────────────────────────────────────┤
│        Vue Router (Navigation)              │
├─────────────────────────────────────────────┤
│      Pinia (State Management)               │
├─────────────────────────────────────────────┤
│       Tauri API (Bridge)                    │
├─────────────────────────────────────────────┤
│      Rust Backend (Tauri Core)              │
└─────────────────────────────────────────────┘
```

## Couche Frontend (Vue 3)

### Point d'entrée : `src/main.ts`

```typescript
import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import { pinia } from './stores'
import vuetify from './plugins/vuetify'

createApp(App)
  .use(router)      // Navigation
  .use(pinia)       // State management
  .use(vuetify)     // UI components
  .mount('#app')
```

**Ordre d'initialisation important** :
1. Router (pour que les stores puissent l'utiliser)
2. Pinia (pour que les composants puissent accéder aux stores)
3. Vuetify (pour les composants UI)

### Layout principal : `src/App.vue`

Structure :
```
App.vue
├── v-app (racine Vuetify avec thème)
│   └── v-main
│       └── <router-view /> (pages dynamiques)
```

Responsabilités :
- Fournir le layout global avec thème
- Détecter le label de la fenêtre courante
- Router vers le composant approprié (Accueil vs ScreenBis)
- Charger les informations de displays (écrans)
- Ouvrir la fenêtre secondaire si dual-screen détecté
- Synchroniser le thème entre fenêtres

**Détection multi-fenêtres** :
```typescript
const currentWindow = getCurrentWindow()
if (currentWindow.label === 'screen-bis') {
  // Naviguer vers ScreenBis pour la fenêtre secondaire
  await router.replace({ name: 'screenBis' })
}
```

## Couche de navigation (Vue Router)

### Configuration : `src/router/index.ts`

```typescript
const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', name: 'accueil', component: Accueil },
    { path: '/visualisation', name: 'visualisation', component: Visualisation },
    { path: '/edition-camera', name: 'editionCamera', component: EditionCamera },
    { path: '/screen-bis', name: 'screenBis', component: ScreenBis }
  ]
})
```

**Stratégie de routing** :
- `createWebHistory()` : URLs propres sans `#`
- Navigation par `name` recommandée (plus stable que `path`)
- 4 routes : `accueil`, `visualisation`, `editionCamera`, `screenBis`

**Ajout de routes** :
```typescript
{
  path: '/nouvelle-page',
  name: 'nouvelle-page',
  component: () => import('../views/NouvellePage.vue'),
  meta: { requiresAuth: true } // métadonnées optionnelles
}
```

## Couche de gestion d'état (Pinia)

### Configuration : `src/stores/index.ts`

```typescript
import { createPinia } from 'pinia'
export const pinia = createPinia()
```

### Pattern Setup Store : `src/stores/app.ts`

```typescript
export const useAppStore = defineStore('app', () => {
  // State (reactive)
  const isDarkMode = ref(false)

  // Getters (computed)
  const theme = computed(() => isDarkMode.value ? 'dark' : 'light')

  // Actions (functions)
  function toggleDarkMode() {
    isDarkMode.value = !isDarkMode.value
  }

  // Exposition publique
  return { isDarkMode, theme, toggleDarkMode }
})
```

**Avantages du Setup Store** :
- Plus proche de la Composition API
- TypeScript inference automatique
- Plus flexible pour la logique complexe
- Meilleure performance

**Architecture d'un store** :
```
Store
├── State (ref)
├── Getters (computed)
├── Actions (functions)
└── Return (API publique)
```

## Couche UI (Vuetify)

### Configuration : `src/plugins/vuetify.ts`

```typescript
export default createVuetify({
  components,      // Tous les composants Vuetify
  directives,      // Toutes les directives
  icons: { mdi },  // Material Design Icons
  theme: {
    defaultTheme: 'light',
    themes: { light: {...}, dark: {...} }
  }
})
```

**Auto-import** via `vite-plugin-vuetify` :
- Pas besoin d'importer les composants `v-*`
- Tree-shaking automatique en production
- Meilleure DX (Developer Experience)

**Thèmes** :
- Définition dans `vuetify.ts`
- Application via `<v-app :theme="appStore.theme">`
- Changement dynamique via store

## Couche Build (Vite)

### Configuration : `vite.config.ts`

```typescript
export default defineConfig({
  plugins: [
    vue(),                         // Support Vue 3
    vuetify({ autoImport: true })  // Auto-import Vuetify
  ],
  server: {
    port: 1420,        // Port du dev server
    strictPort: true   // Erreur si port occupé
  },
  envPrefix: ['VITE_', 'TAURI_'],  // Variables d'env accessibles
  build: {
    target: ['es2021', 'chrome100', 'safari13'],  // Cibles de compilation
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG
  }
})
```

**Processus de build** :
```
Dev:   Vite Dev Server (port 1420) → Tauri → Fenêtre native
Build: Vite Build → dist/ → Tauri Build → Executable
```

## Couche Desktop (Tauri)

### Configuration : `src-tauri/tauri.conf.json` (Tauri v2)

```json
{
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      { "label": "main", "title": "VisuGPS2", "width": 800, "height": 600, "resizable": false, "maximizable": false, "closable": false },
      { "label": "screen-bis", "width": 8, "height": 6, "visible": false, "closable": false }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "resources": ["settings.default.toml"]
  }
}
```

**Spécificités Tauri v2** :
- `devUrl`/`frontendDist` (et non `devPath`/`distDir` comme en v1)
- Section `app` (et non `tauri`)
- Permissions gérées via `capabilities/default.json` (et non `allowlist`)

### Backend Rust : `src-tauri/src/lib.rs`

```rust
mod display;  // Module séparé pour les displays

use tauri::Manager;
use display::get_displays;

#[tauri::command]
async fn open_second_window(app: tauri::AppHandle) -> Result<(), String> {
    // Logique pour ouvrir une fenêtre secondaire
    // ...
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // 18 commandes : voir COMMANDS.md pour le catalogue complet
            exit_app, get_displays, open_second_window, close_second_window,
            gestionMode::*, settings::*, import_gpx::*
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Architecture modulaire Rust** :

Pour garder le code Rust maintenable, les fonctionnalités sont organisées en modules:

- `lib.rs` : Point d'entrée, orchestration, commandes de fenêtres
- `display.rs` : Détection des écrans (macOS NSScreen, Windows Tauri)
- `gestionMode.rs` : Gestion des modes d'exécution (CRUD, sélection, fichier `.env`)
- `settings.rs` : Système de paramètres de configuration (TOML, chiffrement des secrets)
- `import_gpx.rs` : Import de fichiers GPX (parsing, statistiques, registre de traces)

Chaque module peut être étendu sans surcharger `lib.rs`.

**Communication Frontend ↔ Backend** :

Frontend (TypeScript) :
```typescript
import { invoke } from '@tauri-apps/api/core'
const result = await invoke<string>('my_command', { arg: 'value' })
```

Backend (Rust) :
```rust
#[tauri::command]
fn my_command(arg: String) -> String {
    format!("Processed: {}", arg)
}
```

**Communication Inter-fenêtres** :

Les fenêtres communiquent via le système d'événements Tauri :

Frontend (TypeScript) :
```typescript
import { emit, listen } from '@tauri-apps/api/event'

// Émettre un événement
await emit('event-name', { payload: 'data' })

// Écouter un événement
const unlisten = await listen<T>('event-name', (event) => {
  console.log(event.payload)
})
```

Exemple : Synchronisation du thème dark/light entre fenêtres :
```typescript
// Émission depuis appStore
emit('theme-changed', isDarkMode.value)

// Écoute dans les fenêtres
listen<boolean>('theme-changed', (event) => {
  isDarkMode.value = event.payload
})
```

## Structure des dossiers

### `src/` - Code source frontend

```
src/
├── router/           # Configuration du routing
│   └── index.ts
├── stores/           # Stores Pinia (state management)
│   ├── index.ts      # Configuration
│   ├── app.ts        # Store applicatif (thème, displays, modes d'exécution)
│   ├── settings.ts   # Store des paramètres de configuration
│   ├── traces.ts     # Store des traces GPX importées
│   └── ui.ts         # Store des notifications (snackbar)
├── utils/            # Fonctions utilitaires
│   └── format.ts     # Helpers de formatage (distance, élévation, durée)
├── plugins/          # Plugins Vue (Vuetify, etc.)
│   └── vuetify.ts
├── views/            # Pages complètes (routes)
│   ├── Home.vue
│   └── About.vue
├── components/       # Composants réutilisables
│   ├── Accueil/      # Composants de la page d'accueil
│   │   ├── CircuitsDrawer.vue   # Panneau latéral liste des circuits
│   │   ├── Circuit.vue          # Carte d'un circuit
│   │   ├── AppBar.vue
│   │   ├── ModeExecutionCard.vue
│   │   └── SettingsDrawer.vue
│   └── parameters/   # Composants d'édition des paramètres
│       ├── ParameterCard.vue
│       ├── InputBool.vue
│       └── …
├── assets/           # Ressources statiques
│   └── styles/
├── App.vue          # Layout racine
└── main.ts          # Point d'entrée
```

**Règles de structure** :
- **views/** : Un fichier = une route
- **components/** : Composants réutilisables, sans logique métier
- **stores/** : Un store par domaine fonctionnel
- **assets/** : Images, fonts, styles globaux

### `src-tauri/` - Code Rust (généré)

```
src-tauri/
├── src/
│   ├── lib.rs            # Point d'entrée + orchestration
│   ├── display.rs        # Détection et gestion des écrans
│   ├── gestionMode.rs    # Gestion des modes d'exécution
│   ├── settings.rs       # Système de paramètres de configuration
│   ├── import_gpx.rs     # Import de fichiers GPX
│   └── main.rs           # Point d'entrée (auto-généré)
├── capabilities/
│   │   └── default.json  # Permissions pour les fenêtres
├── icons/                # Icônes de l'application
│   ├── icon.png
│   └── ...
├── settings.default.toml # Paramètres par défaut (embarqué)
├── Cargo.toml            # Dépendances Rust
└── tauri.conf.json       # Configuration Tauri
```

**Modules Rust** :
- `lib.rs` : Orchestration principale, commandes Tauri publiques, plugins
- `display.rs` : Détection des moniteurs (macOS NSScreen, Windows Tauri API)
- `gestionMode.rs` : CRUD des modes d'exécution, lecture/écriture du `.env`, fichier `ModeExe.toml`
- `settings.rs` : Lecture/écriture des paramètres TOML, chiffrement des secrets (AES-256-GCM)
- `import_gpx.rs` : Parsing GPX, calcul de stats (Haversine), détection d'éditeur, registre de traces

**Capacités Tauri** :
- `default.json` : Permissions appliquées aux fenêtres `main` et `screen-bis`
  - `core:default` : permissions de base
  - `opener:default` : ouverture de liens dans le navigateur
  - `dialog:allow-open` : sélecteur de fichiers natif (import GPX)

> Note : Tauri v2 utilise le système de **capabilities** (et non l'`allowlist` de v1). Aucune permission `fs:` n'est déclarée : l'accès disque se fait via `std::fs` côté Rust.

## Flux de données

### 1. Navigation utilisateur

```
User clique
  → router.push()
  → Vue Router change la route
  → <router-view> charge le composant
  → Composant s'affiche
```

### 2. Modification d'état global

```
User action (click)
  → Composant appelle store.action()
  → Store modifie son state (ref)
  → Vue détecte le changement (réactivité)
  → Tous les composants utilisant ce state sont re-rendus
```

### 3. Communication Tauri

```
Frontend appelle invoke()
  → Tauri sérialise les arguments (JSON)
  → Appel de la fonction Rust
  → Rust traite et retourne
  → Tauri désérialise le résultat
  → Frontend reçoit le résultat
```

## Patterns architecturaux

### 1. Single Source of Truth

Chaque donnée a **une seule source de vérité** :
- État local → `ref()` dans le composant
- État partagé → Store Pinia
- Configuration → `.env` ou `tauri.conf.json`

### 2. Unidirectional Data Flow

```
Store (source)
  ↓
Computed/Getter (transformation)
  ↓
Composant (affichage)
  ↓
Action (modification)
  ↓
Store (mise à jour)
```

### 3. Composition over Inheritance

Utiliser la Composition API et les composables :

```typescript
// composables/useTheme.ts
export function useTheme() {
  const store = useAppStore()
  return {
    isDark: computed(() => store.isDarkMode),
    toggle: () => store.toggleDarkMode()
  }
}

// Dans un composant
const { isDark, toggle } = useTheme()
```

### 4. Plugin Architecture

Chaque fonctionnalité majeure = un plugin :
- Router → Plugin de navigation
- Pinia → Plugin de state
- Vuetify → Plugin UI

Avantage : Découplage, testabilité, réutilisabilité

## Performance

### 1. Code splitting

```typescript
// Lazy loading des routes
const About = () => import('../views/About.vue')
```

Résultat : Fichiers JS séparés, chargés à la demande

### 2. Tree shaking

Vite + Vuetify auto-import :
- Seuls les composants utilisés sont inclus
- Build optimisé automatiquement

### 3. Réactivité fine-grained

Vue 3 avec Proxy :
- Détection automatique des changements
- Re-render minimal (composants affectés uniquement)

## Sécurité

### 1. Capabilities Tauri v2

Le système de capabilities remplace l'`allowlist` de Tauri v1. Déclaré dans `src-tauri/capabilities/default.json` :

```json
{
  "permissions": ["core:default", "opener:default", "dialog:allow-open"]
}
```

Seules les permissions strictement nécessaires sont activées. Aucun accès disque direct côté frontend (`fs` non déclaré) : tout passe par les commandes Tauri.

### 2. Content Security Policy

Configuré dans `tauri.conf.json` : `"csp": null` (désactivé — acceptable pour une app desktop sans contenu distant).

### 3. Chiffrement des secrets

Les paramètres de type `secret` sont chiffrés en **AES-256-GCM** avant écriture disque (clé keyring OS en prod, statique en dev). Voir [DATA_STORAGE.md](./DATA_STORAGE.md).

### 4. Variables d'environnement

- Seules les variables `VITE_*` / `TAURI_*` sont exposées au frontend (`envPrefix` dans `vite.config.ts`)
- Variables sensibles uniquement côté Rust
- Pas de secrets dans le code frontend

## Extensibilité

### Ajouter un nouveau plugin Vue

```typescript
// src/plugins/monplugin.ts
export default {
  install(app: App) {
    app.config.globalProperties.$monPlugin = ...
  }
}

// src/main.ts
import monPlugin from './plugins/monplugin'
createApp(App).use(monPlugin)
```

### Ajouter un middleware de routing

```typescript
// src/router/index.ts
router.beforeEach((to, from, next) => {
  // Logique de guard
  if (to.meta.requiresAuth && !isAuthenticated()) {
    next({ name: 'login' })
  } else {
    next()
  }
})
```

### Ajouter une dépendance Rust

```toml
# src-tauri/Cargo.toml
[dependencies]
serde_json = "1.0"
```

## Déploiement

### Build process

```
1. npm run build
   → Vite compile le frontend → dist/

2. npm run tauri build
   → Tauri compile l'app native
   → Inclut dist/ dans l'executable
   → Crée les installers
   → Output: src-tauri/target/release/bundle/
```

### Targets de build

- **macOS** : `.dmg`, `.app`
- **Windows** : `.exe`, `.msi`
- **Linux** : `.deb`, `.AppImage`

Configuration dans `src-tauri/tauri.conf.json` → `tauri.bundle`

## Diagrammes

### Diagramme de composants

```
┌──────────────────────────────────────┐
│           App.vue                    │
│  ┌────────────────────────────────┐ │
│  │      v-app-bar                 │ │
│  └────────────────────────────────┘ │
│  ┌────────────────────────────────┐ │
│  │   v-navigation-drawer          │ │
│  └────────────────────────────────┘ │
│  ┌────────────────────────────────┐ │
│  │      v-main                    │ │
│  │  ┌──────────────────────────┐ │ │
│  │  │   <router-view />        │ │ │
│  │  │   (Home.vue / About.vue) │ │ │
│  │  └──────────────────────────┘ │ │
│  └────────────────────────────────┘ │
└──────────────────────────────────────┘
```

### Diagramme de flux d'installation

```
setup.sh
   │
   ├──→ check-requirements.sh
   │    └──→ Vérifie Node, npm, Rust, Cargo
   │
   ├──→ install-dependencies.sh
   │    ├──→ npm create tauri-app
   │    ├──→ npm install vuetify pinia vue-router
   │    └──→ npm install -D sass vite-plugin-vuetify
   │
   ├──→ create-structure.sh
   │    ├──→ Crée src/router/, stores/, plugins/, views/
   │    ├──→ Génère router/index.ts
   │    ├──→ Génère stores/app.ts
   │    ├──→ Génère plugins/vuetify.ts
   │    ├──→ Génère views/Home.vue, About.vue
   │    └──→ Génère App.vue, main.ts
   │
   └──→ configure-app.sh
        ├──→ Génère vite.config.ts
        └──→ Génère tsconfig.json
```

## Gestion des Modes d'Exécution (Multi-environnement)

### 1. Variables de mode d'exécution dans le store Pinia (`src/stores/app.ts`)

Pour gérer les configurations multi-environnements en DEV et PROD, le store `app` expose plusieurs états :
- `activeModeDev` : Le mode d'exécution actif pour le développement (lu à partir de la variable `APP_ENV_DEV` du fichier `.env`).
- `activeModeProd` : Le mode d'exécution actif pour la production.
- `isDev` : Un booléen indiquant si l'application s'exécute en mode développement.

### 2. Contrôles de l'interface utilisateur (`ModeExecutionCard.vue`)

- **Puces d'état (Chips)** :
  - Un chip **Actif** est affiché sur le mode correspondant à `activeModeProd` (vert si le mode est `OPE`, bleu dans les autres cas).
  - Un chip **Actif Dev** (orange) s'affiche sur le mode correspondant à `activeModeDev` uniquement si l'application s'exécute en mode DEV (`isDev` est vrai).
- **Restrictions sur les actions (Édition/Suppression)** :
  - Les boutons **Supprimer** (`mdi-delete`) et **Modifier** (`mdi-pencil`) sont conditionnellement masqués :
    - Si le mode est le mode de production principal (`OPE`).
    - Si le mode correspond à `activeModeDev` ou à `activeModeProd`. Cela empêche toute suppression accidentelle du mode de développement ou du mode de production actif.

## Gestion des Paramètres (Settings)

L'application intègre un système robuste de gestion des paramètres de configuration.

### Architecture du système de paramètres

1. **Définition (Backend Rust)** :
   - Les paramètres par défaut sont définis dans `src-tauri/settings.default.toml` (embarqué dans l'exécutable).
   - Les paramètres modifiés par l'utilisateur sont sauvegardés dans un fichier `config-dev.toml` (en mode dev) ou `config.toml` (en production) dans le dossier de configuration de l'OS (`Application Support` sur macOS).

2. **Sécurité (Secrets)** :
   - Les paramètres sensibles (type `Secret`, comme les clés API) sont chiffrés avec AES-256-GCM avant écriture sur disque.
   - La clé de chiffrement ("master key") est stockée dans le gestionnaire de mots de passe de l'OS en production (Keyring/Trousseau), ou codée en dur en développement pour éviter les pop-ups macOS incessants lors des recompilations.

3. **Store (Frontend Pinia)** :
   - `src/stores/settings.ts` charge les paramètres via la commande Tauri `get_settings`.
   - Il maintient l'état réactif de chaque paramètre (`value`, `default`, `is_overridden`, etc.).

4. **Interface (Vue)** :
   - Une carte d'édition générique ([ParameterCard.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/parameters/ParameterCard.vue)) s'appuie sur des composants d'entrée spécifiques par type ([InputBool.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/parameters/InputBool.vue), [InputInt.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/parameters/InputInt.vue), etc.) pour modifier, sauvegarder et réinitialiser (undo) les paramètres individuels.
   - Un composant spécifique ([SettingsEditMonitor.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/Accueil/SettingsEditMonitor.vue)) gère la configuration combinée des écrans principal et secondaire.
   - Le menu des paramètres s'affiche dans un panneau latéral ([SettingsDrawer.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/Accueil/SettingsDrawer.vue)).

## Import de traces GPX

L'application permet d'importer des fichiers GPX provenant de plateformes comme Garmin Connect, Strava, OpenRunner ou RideWithGPS.

### Architecture du module GPX

1. **Backend Rust** (`src-tauri/src/import_gpx.rs`) :
   - Le sélecteur de fichier natif est ouvert via le plugin `tauri-plugin-dialog` (pas d'API Tauri 1.x).
   - Le parsing utilise le crate `gpx` (version 0.10) et les calculs géodésiques le crate `geo` (Haversine).
   - Les données sont stockées dans le **dossier du mode d'exécution actif** : `{app_data_dir}/{active_mode}/gpx/` pour les fichiers et `{app_data_dir}/{active_mode}/traces.json` pour le registre.
   - Le registre est sauvegardé avec une **écriture atomique** (fichier `.tmp` + `rename`).
   - Les doublons sont détectés par **empreinte SHA256** du contenu binaire.

2. **Détection heuristique de l'éditeur** :
   - Priorité : `<link href>` → `creator` → nom de track → signature XML → "Inconnu".
   - Éditeurs reconnus : Strava, Garmin Connect, OpenRunner, RideWithGPS.

3. **Store Frontend** (`src/stores/traces.ts`) :
   - Pattern Setup Store (comme `app.ts` et `settings.ts`).
   - Actions `loadTraces()` et `importerGpx()` passent par des commandes Tauri (le frontend ne connaît pas le mode actif).
   - Types `TraceMetadata`, `TraceStats`, `Point3D` en miroir exact des structs Rust.

4. **Composants Vue** :
   - `CircuitsDrawer.vue` : câblage du bouton `mdi-image-plus-outline` sur `importerGpx()`, liste pilotée par le store.
   - `Circuit.vue` : affiche les statistiques calculées (distance, dénivelé, durée) au lieu de données en dur.

5. **Notifications** (`src/stores/ui.ts`) :
   - Store mutualisé pour les snackbars Vuetify (succès, erreur, avertissement, info).

### Stockage par mode d'exécution

```
{app_data_dir}/
├── .env                     # Mode actif (APP_ENV_DEV / APP_ENV_PROD)
├── ModeExe.toml             # Définition des modes
└── {active_mode}/           # Ex : OPE, EVAL_essai
    ├── gpx/                 # Fichiers .gpx copiés (nom unique si doublon)
    ├── traces.json          # Registre des traces importées (Vec<TraceMetadata>)
    ├── config-dev.toml      # Surcharges de paramètres (dev)
    └── config.toml          # Surcharges de paramètres (prod)
```

### Commandes Tauri du module GPX

| Commande | Description |
|----------|-------------|
| `import_gpx_file` | Ouvre le sélecteur natif, parse le GPX, copie le fichier, met à jour le registre. Retourne `TraceMetadata`. |
| `get_traces` | Retourne `Vec<TraceMetadata>` pour le mode d'exécution actif. |
| `delete_trace` | Supprime le fichier GPX + l'entrée du registre (écriture atomique). |
| `update_trace` | Mise à jour partielle (PATCH) d'une trace : `favorite` et/ou `is_displayed` (persistés). |

> Référence complète des 18 commandes Tauri dans [COMMANDS.md](./COMMANDS.md).

---

**Note** : Cette architecture est conçue pour être simple et extensible. Suivez ces patterns pour maintenir la cohérence du projet.

**Dernière mise à jour** : 2026-07-10
