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
            // 20 commandes : voir COMMANDS.md pour le catalogue complet
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
│   ├── edition.ts    # Store de la vue d'édition caméra (trace, lecture, keyframes)
│   └── ui.ts         # Store des notifications (snackbar)
├── algorithms/       # Logique métier isolée, sans dépendance UI
│   └── keyframeGenerator.ts  # Génération + interpolation des keyframes caméra
├── utils/            # Fonctions utilitaires (named exports)
│   ├── format.ts     # Helpers de formatage (distance, élévation, durée)
│   └── geo.ts        # Utilitaires géographiques (Haversine, bearing, cap)
├── plugins/          # Plugins Vue (Vuetify, etc.)
│   └── vuetify.ts
├── views/            # Pages complètes (routes)
│   ├── Home.vue
│   └── About.vue
├── components/       # Composants réutilisables
│   ├── Accueil/      # Composants de la page d'accueil
│   │   ├── CircuitsDrawer.vue   # Panneau latéral liste des circuits (triée par distance)
│   │   ├── Circuit.vue          # Carte d'un circuit (2 lignes d'icônes d'action, extension Info, focus carte, édition caméra)
│   │   ├── Map.vue              # Carte Mapbox GL (clusters, favoris, traces affichées, focus)
│   │   ├── AppBar.vue
│   │   ├── ModeExecutionCard.vue
│   │   ├── SettingsDrawer.vue   # Drawer de paramètres (dynamique, piloté par [_meta])
│   │   └── SettingsCategory.vue # Rendu d'une catégorie (accordéon / aplati / handler)
│   ├── Edition/      # Composants de la vue d'édition caméra
│   │   ├── EditionMap.vue       # Carte Mapbox GL satellite + terrain (trace, marker, lecture rAF)
│   │   ├── EditionToolbar.vue   # Barre d'outils supérieure (Home, toggle cadre ViewPort)
│   │   ├── ViewportFrame.vue    # Overlay CSS du cadre ViewPort 16:9 (masque sombre + trait blanc)
│   │   ├── PlaybackControls.vue # Bandeau bas — Composant A (Play/Pause, vitesse, distance)
│   │   └── TelemetryHud.vue     # Overlay — Composant C (HUD télémétrie caméra ↔ marqueur)
│   └── parameters/   # Composants d'édition des paramètres
│       ├── ParameterCard.vue
│       ├── InputBool.vue
│       └── …
├── composables/      # Logique réutilisable (Composition API)
│   └── useSettingsTree.ts       # Construit l'arbre catégories/params du drawer (filtré par route)
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

### 4. Synchronisation carte ↔ liste (filtrage viewport)

```
Utilisateur déplace/zoome la carte
  → Map.vue : événement moveend (debounce 150 ms)
  → tracesStore.updateMapCenter(lat, lng)
  → Map.vue : scheduleVisibleRefresh() (attend l'état stable de la carte)
  → Map.vue (à l'état stable / idle) : refreshVisibleTraceIds()
      - queryRenderedFeatures({ layers: ['unclustered-point'] }) → IDs points individuels
      - queryRenderedFeatures({ layers: ['clusters'] }) → pour chaque cluster
          → source.getClusterLeaves(clusterId, pointCount, 0) → IDs feuilles
      - Dédoublonnage (Set) → tracesStore.setVisibleTraceIds(ids)
  → Invalidation du getter computed visibleTracesByDistance
  → CircuitsDrawer.vue : la v-for se met à jour automatiquement
  → L'utilisateur voit uniquement les circuits visibles dans le viewport,
    triés par distance croissante, plafonnés à nbrCircuits
```

**Points clés** :
- Le calcul utilise l'événement `idle` de Mapbox (carte dans un état stable, clusters rendus) pour garantir que `queryRenderedFeatures` retourne des résultats fiables. Un drapeau `pendingVisibleRefresh` est levé par `scheduleVisibleRefresh()` et consommé par le handler `idle`.
- Le pattern d'epoch (`visibleEpoch`) annule les résultats périmés si un nouveau `moveend` survient pendant les appels asynchrones à `getClusterLeaves`.
- Pendant un focus (`focusedTraceId` positionné), `refreshVisibleTraceIds()` est court-circuité pour garder le drawer stable (cohérent avec `moveend`).

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
   - Les paramètres par défaut sont définis dans `src-tauri/settings.default.toml` (embarqué dans l'exécutable). Chaque paramètre porte `description`, `documentation` (Markdown), `type`, `default`, et optionnellement `min`/`max`/`step`/`unit`/`choices`/`critical`/`icon` (icône MDI pour le drawer).
   - Ce même fichier contient une **table spéciale `[_meta]`** qui décrit l'organisation du drawer : vues (associées aux noms de routes), groupes système communs à toutes les vues, actions (entrées non-paramètres comme les modes d'exécution), handlers (catégories à carte dédiée, ex. `Affichage.moniteurs`), et libellés/icônes des catégories. Cette table est **exclue du « flatten »** des paramètres et exposée par la commande `get_settings_meta`.
   - Les paramètres modifiés par l'utilisateur sont sauvegardés dans un fichier `config-dev.toml` (en mode dev) ou `config.toml` (en production) dans le dossier de configuration de l'OS (`Application Support` sur macOS).

2. **Sécurité (Secrets)** :
   - Les paramètres sensibles (type `Secret`, comme les clés API) sont chiffrés avec AES-256-GCM avant écriture sur disque.
   - La clé de chiffrement ("master key") est stockée dans le gestionnaire de mots de passe de l'OS en production (Keyring/Trousseau), ou codée en dur en développement pour éviter les pop-ups macOS incessants lors des recompilations.

3. **Store (Frontend Pinia)** :
   - `src/stores/settings.ts` charge les paramètres via la commande Tauri `get_settings`, et l'organisation du drawer via `get_settings_meta` (état `meta`, chargé une fois).
   - Il maintient l'état réactif de chaque paramètre (`value`, `default`, `is_overridden`, etc.).
   - L'état d'ouverture du drawer (`isSettingsDrawerOpen`) vit dans le store `app`.

4. **Interface (Vue)** :
   - Une carte d'édition générique ([ParameterCard.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/parameters/ParameterCard.vue)) s'appuie sur des composants d'entrée spécifiques par type ([InputBool.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/parameters/InputBool.vue), [InputInt.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/parameters/InputInt.vue), etc.) pour modifier, sauvegarder et réinitialiser (undo) les paramètres individuels.
   - Un composant spécifique ([SettingsEditMonitor.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/Accueil/SettingsEditMonitor.vue)) gère la configuration combinée des écrans principal et secondaire.
   - Le panneau latéral des paramètres ([SettingsDrawer.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/Accueil/SettingsDrawer.vue)) est **entièrement dynamique** : aucune entrée n'est codée en dur. L'arbre des catégories est construit par le composable [useSettingsTree.ts](file:///Volumes/Externe/Dev/VisuGPS2/src/composables/useSettingsTree.ts) à partir de `[_meta]` et filtré selon la route active ; chaque catégorie est rendue par [SettingsCategory.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/Accueil/SettingsCategory.vue). Les indicateurs visuels sont : icône en orange si critique, libellé en bleu si surchargé (remontés au niveau catégorie).

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
   - Getter `sortedTracesByDistance` : trie les traces par distance Haversine croissante au centre courant de la carte (`mapCenter`).
   - Getter `visibleTracesByDistance` : filtre `sortedTracesByDistance` pour ne garder que les traces dont l'ID figure dans `visibleTraceIds`. Utilisé par `CircuitsDrawer.vue`.
   - État `visibleTraceIds` (réactif, `Set<string>`) : identifiants des traces visibles dans le viewport courant. Mis à jour par `Map.vue` via l'action `setVisibleTraceIds()`. État UI éphémère, non persisté.
   - Action `updateMapCenter(lat, lon)` : appelée par `Map.vue` sur `moveend` (debounce) pour synchroniser le tri.
   - Action `setVisibleTraceIds(ids)` : appelée par `Map.vue` après `queryRenderedFeatures` + `getClusterLeaves`.
   - État `focusedTraceId` : id de la trace « focus » temporaire (clic Info dans `Circuit.vue`), observé par `Map.vue` pour isoler et cadrer la trace (cf. §5). État UI éphémère, non persisté.

4. **Composants Vue** :
   - `CircuitsDrawer.vue` : câblage du bouton `mdi-image-plus-outline` sur `importerGpx()`, liste pilotée par le store, filtrée par viewport (`visibleTracesByDistance`), plafonnée au paramètre `Accueil.nbrCircuits.list`.
   - `Circuit.vue` : affiche les statistiques calculées (distance, dénivelé) ; deux lignes d'icônes d'action masquées par opacité hors survol — ligne de titre (Éditer, Groupes, Météo, Visualiser) et ligne Distance/Dénivelé (Supprimer, Exporter, Info, Affichage, Favoris) ; extension `v-expand-transition` au clic Info (date d'import, source, lien) ; déclenche le focus carte via `tracesStore.focusedTraceId`. Le bouton **Éditer** (`mdi-pencil`) sélectionne la trace (`editionStore.selectTrace`) puis navigue vers la vue `editionCamera` (cf. § « Vue d'édition caméra » ci-dessous). Les autres boutons de la ligne de titre (Groupes, Météo) restent à câbler.

5. **Carte Mapbox** (`src/components/Accueil/Map.vue`) :
   - Carte Mapbox GL (style `standard`, token depuis `Systeme.Key.mapBox`).
   - **Sources GeoJSON** : `traces` (clusterisée, points de départ, `cluster: true`, `clusterRadius: 50`, `clusterMaxZoom: 14`), `favorites` (LineString favoris), `displayed-traces` (LineString dégradé, `lineMetrics: true`), `focus-traces` (LineString isolée en mode focus).
   - **Couches** (du bas vers le haut) : `clusters` / `cluster-count` / `unclustered-point` (points de départ) ; `favorites-line` (couleur favori, épaisseur 6) ; `displayed-traces-line` (dégradé bleu→rouge, épaisseur 4) ; `focus-traces-line` (même dégradé, masquée par défaut).
   - **Synchronisation carte ↔ store** : `moveend` (debounce 150 ms) → `tracesStore.updateMapCenter()` → `scheduleVisibleRefresh()` → à l'état stable (`idle`) : `queryRenderedFeatures` sur les couches `unclustered-point` et `clusters`, `getClusterLeaves` pour extraire les feuilles, dédoublonnage → `tracesStore.setVisibleTraceIds()` → invalidation du getter `visibleTracesByDistance` → mise à jour de la liste. Pendant un focus, le calcul est court-circuité (stabilité du drawer).
   - **Interactions** : clic cluster → `easeTo` vers le centre au zoom d'expansion ; clic point → popup (nom, source, coordonnées) ; curseur `pointer` au survol.
   - **Focus carte** : `watch(tracesStore.focusedTraceId)` → sauvegarde de la vue, masquage des couches favoris/affichées, affichage isolé de la trace dans `focus-traces-line`, cadrage par `fitBounds`, retour par `flyTo` (durée `Carte.Traces.dureeFlyTo`).
   - **Réactivité** : `watch(traces)` → `setData()` sur les sources + `scheduleVisibleRefresh()` pour recalculer les visibles ; `watch(settings)` → `setPaintProperty` pour le style ; événement `idle` → consomme `pendingVisibleRefresh` pour effectuer le calcul après stabilisation du rendu.

6. **Utilitaire géographique** (`src/utils/geo.ts`) :
   - Fonctions nommées exportées (pattern `format.ts`) : `toRadians()`, `haversineMeters()`.
   - R = 6 371 000 m (cohérent avec le backend Rust `import_gpx.rs`).

5. **Notifications** (`src/stores/ui.ts`) :
   - Store mutualisé pour les snackbars Vuetify (succès, erreur, avertissement, info).

## Vue d'édition caméra (`/edition-camera`)

La vue d'édition caméra (Phase 2 de la spec « Visualisation GPX sur MapBox ») est l'interface de réglage de la caméra qui suit une trace. Elle dispose désormais d'un **playback fonctionnel** : génération de keyframes, boucle d'animation `requestAnimationFrame`, contrôles de lecture (Composant A) et HUD de télémétrie (Composant C). Reste à venir : le graphe SVG d'avancement (§4.6), le Composant B (édition fine des keyframes) et l'algorithme intelligent de frustum Phase 1.

### Architecture

1. **Génération des keyframes** (`src/algorithms/keyframeGenerator.ts`) :
   - Module isolé **sans dépendance UI** (spec §3.2) — ne dépend que de `utils/geo`. Remplaçable sans impacter le reste de l'application.
   - Définit les types du format JSON figé (spec §3.4) : `CamState`, `TraceurPoint`, `Keyframe`, `KeyframeSet`.
   - `generateKeyframes(traceId, feature, sampleStepM = 250)` : construit la polyligne indexée par distance cumulée (Haversine), échantillonne un keyframe tous les `KEYFRAME_STEP_M`, interpole la position exacte sur la polyligne à la distance cible, et oriente chaque caméra vers le keyframe suivant (`bearing()`). `time_ms = distance × MS_PER_METER` (vitesse défaut 4000 ms/km).
   - Helpers purs d'interpolation (consommés par la boucle de lecture) : `findSegment` (dichotomie O(log n)), `interpolateCam` (bearing interpolé **sur le cercle** pour éviter une rotation à 360° au wrap), `interpolateTraceur`.
   - **Polyligne indexée par distance** : `buildTracePolyline(feature)` et `samplePolylineAt(poly, distanceM)` exposent la trace complète (tous les points GPX) indexée par distance cumulée. Le marker les utilise pour avancer **le long de la trace réelle** (et non entre les keyframes échantillonnés), de sorte qu'il épouse les virages au lieu de tirer des cordes droites.
   - **Algorithme volontairement simple (MVP)** : la caméra suit fidèlement la trace. Il sera remplacé par l'algorithme de frustum (spec §3.3) sans impacter le reste.

2. **Store** (`src/stores/edition.ts`) — Pattern Setup Store, état UI éphémère non persisté :
   - **Sélection / cadre** : `selectedTraceId`, `showViewportFrame` ; actions `selectTrace`, `clearSelection`, `toggleViewportFrame`.
   - **Lecture** : `keyframeSet`, `isPlaying`, `speed` (0.5/1/2/4), `currentTimeMs`.
   - **Getters** : `hasKeyframes`, `totalDistanceM/Km`, `totalDurationMs`, `currentDistanceM/Km`, `progressRatio`, `currentSegment`, `interpolatedCam` (interpolation entre keyframes), `interpolatedTraceur` (échantillonné **le long de la polyligne réelle** à la distance courante — suit les virages), `markerDistanceM` (haversine cam↔traceur), `markerRelativeBearing` (cap relatif normalisé [-180,180]).
   - **Actions** : `setKeyframeSet(set, feature)` (reset temps + pause, construit la polyligne depuis la feature), `play`/`pause`/`togglePlay`, `setSpeed`, `seekToDistance`, `tick(deltaMs)` (avance le temps de `delta × speed`, **pause auto en fin de course**).

3. **Vue** (`src/views/EditionCamera.vue`) :
   - `v-main` en **colonne flex** : un wrapper carte (`position: relative`, `flex: 1`) contenant `EditionMap` + overlays `ViewportFrame` et `TelemetryHud`, puis `PlaybackControls` en bandeau bas fixe.
   - Au montage : précharge `appStore`, `settingsStore`, `tracesStore`. **Garde-fou** : si `selectedTraceId` est `null` (rechargement direct), `router.replace({ name: 'accueil' })`.

4. **Carte + lecture** (`src/components/Edition/EditionMap.vue`) :
   - Carte Mapbox GL **dédiée** (distincte de `Accueil/Map.vue`). Style `standard-satellite` ; **terrain/élévation** via source `raster-dem` (`mapbox-terrain-rgb`) + `setTerrain({ exaggeration: 1.5 })` ; pitch 60° par défaut (spec §7).
   - Charge la géométrie via `tracesStore.getTraceGeometry` (LineString blanche), pose le **marker jaune bordé de blanc** (`.edition-marker`), cadre sur l'emprise (`fitBounds`), **puis génère les keyframes** et les pousse dans `editionStore`.
   - **Boucle d'animation** pilotée par `editionStore.isPlaying` : une `requestAnimationFrame` calcule le delta réel (`performance.now()`), déclenche `tick(delta)`, puis applique l'état interpolé (`map.jumpTo` pour la caméra, `marker.setLngLat` pour le traceur). En pause, un `watch(currentTimeMs)` repositionne (l'utilisateur garde la main sur la carte). Arrêt propre du rAF au démontage.

5. **Composants** (`src/components/Edition/`) :
   - `EditionToolbar.vue` : `v-app-bar` semi-transparente. Bouton **Home**, titre de la trace, toggle **cadre ViewPort**.
   - `ViewportFrame.vue` : overlay CSS pur (z-index 5, `pointer-events: none`). Rectangle **16:9 centré** (plus grand possible, mesuré au `ResizeObserver`), masque sombre ~70 % via `box-shadow` gigantesque, trait blanc 2 px. Purement informatif.
   - `PlaybackControls.vue` (Composant A, spec §4.3) : bandeau bas. Bouton **Play/Pause**, `v-btn-toggle` vitesse (0.5×/1×/2×/4×), affichage `Distance parcourue : X.XX km / Y.YY km`.
   - `TelemetryHud.vue` (Composant C, spec §4.5) : overlay coin supérieur droit, fond semi-transparent sombre, texte blanc. Paramètres caméra (Zoom/Pitch/Bearing/Lng/Lat) + relation caméra↔marqueur (Distance/Cap). Masqué sans keyframes.

6. **Déclencheur** (`src/components/Accueil/Circuit.vue`) :
   - Le bouton **Éditer** (`mdi-pencil`) appelle `editerCircuit()` : `editionStore.selectTrace(trace.id)` puis `router.push({ name: 'editionCamera' })`.

### Carte satellite + terrain (vs. Accueil/Map.vue)

| Aspect | `Accueil/Map.vue` | `Edition/EditionMap.vue` |
|---|---|---|
| Rôle | Navigation, clustering, favoris | Rendu « cinematic » + lecture d'une trace |
| Style | `mapbox://styles/mapbox/standard` | `mapbox://styles/mapbox/standard-satellite` |
| Terrain | Non | `raster-dem` `mapbox-terrain-rgb`, exaggeration 1.5 |
| Pitch | 0 (défaut) | 60° (défaut, spec §7) |
| Sources | `traces` (clusters), `favorites`, `displayed-traces`, `focus-traces` | `edition-trace` (LineString), `edition-terrain` (DEM) |
| Marker | Aucun | Marker jaune DOM custom, repositionné à chaque frame |
| Animation | Aucune | Boucle `requestAnimationFrame` pilotée par `editionStore` |

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

> Référence complète des 20 commandes Tauri dans [COMMANDS.md](./COMMANDS.md).

---

**Note** : Cette architecture est conçue pour être simple et extensible. Suivez ces patterns pour maintenir la cohérence du projet.

**Dernière mise à jour** : 2026-08-05
