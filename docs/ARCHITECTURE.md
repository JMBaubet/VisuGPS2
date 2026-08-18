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
    { path: '/nettoyage', name: 'nettoyage', component: Nettoyage },
    { path: '/screen-bis', name: 'screenBis', component: ScreenBis }
  ]
})
```

**Stratégie de routing** :
- `createWebHistory()` : URLs propres sans `#`
- Navigation par `name` recommandée (plus stable que `path`)
- 5 routes : `accueil`, `visualisation`, `editionCamera`, `nettoyage`, `screenBis`

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
            // 29 commandes : voir COMMANDS.md pour le catalogue complet
            exit_app, get_displays, open_second_window, close_second_window,
            gestionMode::*, settings::*, import_gpx::*, cleaning::*
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
- `import_gpx.rs` : Import de fichiers GPX (parsing, statistiques, registre de traces, points avec distance cumulée, persistance des keyframes)
- `cleaning.rs` : Nettoyage de trace GPX (détection d'anomalies, persistance des décisions, finalisation)

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
│   ├── keyframes.ts  # Store de persistance des keyframes (loadKeyframes, saveKeyframes, clearKeyframes)
│   ├── edition.ts    # Store de la vue d'édition caméra (trace, lecture, keyframes)
│   ├── cleaning.ts   # Store de la vue de nettoyage de trace (détection, corrections, finalisation)
│   └── ui.ts         # Store des notifications (snackbar)
├── algorithms/       # Logique métier isolée, sans dépendance UI
│   ├── keyframeGenerator.ts  # Génération + interpolation des keyframes caméra (simple + délégation frustum)
│   ├── frustum.ts            # Algorithme de frustum (spec §3.3) — placement récursif par visibilité
│   └── headingChanges.ts     # Analyse des changements de cap entre keyframes (Δcap, sens, taux °/km, couleur MD)
├── utils/            # Fonctions utilitaires (named exports)
│   ├── format.ts     # Helpers de formatage (distance, élévation, durée)
│   └── geo.ts        # Utilitaires géographiques (Haversine, bearing, cap)
├── plugins/          # Plugins Vue (Vuetify, etc.)
│   └── vuetify.ts
├── views/            # Pages complètes (routes)
│   ├── Home.vue
│   ├── Cleaning.vue  # Nettoyage de trace GPX (route `/nettoyage`) — carte, panneau des cas, table des points
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
   │   │   ├── EditionMap.vue       # Carte Mapbox GL satellite + terrain (trace, curseur GL CircleLayer, lecture rAF)
	   │   │   ├── EditionToolbar.vue   # Barre d'outils supérieure (Home, toggle cadre ViewPort, sélecteur Algorithme, Gap min)
   │   │   ├── ViewportFrame.vue    # Overlay CSS du cadre ViewPort 16:9 (masque sombre + trait blanc)
   │   │   ├── PlaybackControls.vue # Bandeau bas — Composant A (colonne boutons + graphe §4.6)
   │   │   ├── ProgressGraph.vue    # Graphe SVG d'avancement (§4.6) — timeline proportionnelle, 3 zones, auto-scroll
   │   │   ├── TelemetryHud.vue     # Overlay — Composant C (HUD télémétrie caméra ↔ curseur)
   │   │   ├── HeadingChangesPanel.vue # Overlay — tableau des changements de cap brutaux (à la demande)
   │   │   ├── DistanceHud.vue      # Overlay — HUD distance parcourue/total (barre bas, orange)
   │   │   └── CameraEditor.vue     # Composant B — édition des keyframes (widgets manipulation directe)
│   ├── Cleaning/     # Composants de la vue de nettoyage de trace
│   │   ├── CleaningToolbar.vue    # Barre d'outils (retour accueil, titre trace, Enregistrer, Finaliser)
│   │   ├── CleaningMap.vue        # Carte Mapbox — trace complète (verte), linestring corrigé (jaune), segment surligné, branches décalées, labels anti-revouvrement, points supprimés en rouge, drag direct des points (cas manuels)
│   │   ├── CleaningCasesPanel.vue # Panneau des anomalies (liste des cas, validation « Corriger & valider » / « Conserver tel quel »)
│   │   └── CleaningPointTable.vue # Table simplifiée des points (numéro, suppression, indicateur « Déplacé »)
│   ├── parameters/   # Composants d'édition des paramètres
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
│   ├── cleaning.rs       # Nettoyage de trace GPX (détection, persistance, finalisation)
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
- `cleaning.rs` : Détection d'anomalies (rebroussement ~180°), persistance des décisions (`cleaning/{trace_id}.json`), finalisation (GPX nettoyé + backup + régénération geojson/stats/hash)

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
   - Le panneau latéral des paramètres ([SettingsDrawer.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/Accueil/SettingsDrawer.vue)) est **entièrement dynamique** : aucune entrée n'est codée en dur. L'arbre des catégories est construit par le composable [useSettingsTree.ts](file:///Volumes/Externe/Dev/VisuGPS2/src/composables/useSettingsTree.ts) à partir de `[_meta]` et filtré selon la route active ; chaque catégorie est rendue par [SettingsCategory.vue](file:///Volumes/Externe/Dev/VisuGPS2/src/components/Accueil/SettingsCategory.vue). Les indicateurs visuels sont : icône en orange si critique, libellé en bleu si surchargé (remontés au niveau catégorie). Une prop `showSystem` (défaut `true`) masque les sections système — la vue Édition l'utilise à `false` pour n'afficher que ses propres catégories.

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
   - Types `TraceMetadata`, `TraceStats`, `Point3D`, `TracePoint` en miroir exact des structs Rust.
   - Action `getTracePoints(traceId)` : appelle `get_trace_points` pour récupérer les points enrichis (altitude + distance cumulée 3D). Utilisé par la vue d'édition caméra.
   - Getter `sortedTracesByDistance` : trie les traces par distance Haversine croissante au centre courant de la carte (`mapCenter`).
   - Getter `visibleTracesByDistance` : filtre `sortedTracesByDistance` pour ne garder que les traces dont l'ID figure dans `visibleTraceIds`. Utilisé par `CircuitsDrawer.vue`.
   - État `visibleTraceIds` (réactif, `Set<string>`) : identifiants des traces visibles dans le viewport courant. Mis à jour par `Map.vue` via l'action `setVisibleTraceIds()`. État UI éphémère, non persisté.
   - Action `updateMapCenter(lat, lon)` : appelée par `Map.vue` sur `moveend` (debounce) pour synchroniser le tri.
   - Action `setVisibleTraceIds(ids)` : appelée par `Map.vue` après `queryRenderedFeatures` + `getClusterLeaves`.
   - État `focusedTraceId` : id de la trace « focus » temporaire (clic Info dans `Circuit.vue`), observé par `Map.vue` pour isoler et cadrer la trace (cf. §5). État UI éphémère, non persisté.

4. **Composants Vue** :
   - `CircuitsDrawer.vue` : câblage du bouton `mdi-image-plus-outline` sur `importerGpx()`, liste pilotée par le store, filtrée par viewport (`visibleTracesByDistance`), plafonnée au paramètre `Accueil.nbrCircuits.list`.
   - `Circuit.vue` : affiche les statistiques calculées (distance, dénivelé) ; deux lignes d'icônes d'action masquées par opacité hors survol — ligne de titre (Éditer, Groupes, Météo, Visualiser) et ligne Distance/Dénivelé (Supprimer, Exporter, Info, Affichage, Favoris) ; extension `v-expand-transition` au clic Info (date d'import, source, lien) ; déclenche le focus carte via `tracesStore.focusedTraceId`. **Badge de nettoyage** : un `v-chip` orange « À nettoyer » est affiché tant que `cleaning_status !== 'clean'`, et l'icône Éditer devient **`mdi-broom`** (au lieu de `mdi-pencil`). Le bouton **Éditer** sélectionne la trace (`editionStore.selectTrace`) puis navigue vers la vue `editionCamera` — ou vers la vue `/nettoyage` si la trace n'est pas « clean » (cf. § « Nettoyage de trace GPX » ci-dessous). **Indicateur d'avancement de l'édition** : au montage, `Circuit` charge le fichier keyframes du **viewport paramétré** (`Edition.Camera.viewportDefaut`) via `keyframesStore.loadKeyframes` et calcule le ratio de segments verrouillés ; l'icône Éditer est **colorée** selon ce ratio (vert 100 % / jaune > 50 % / orange ≥ 10 % / rouge sinon, mêmes seuils que la toolbar d'édition) et **forcée visible quand l'édition est incomplète** (non verte), même sans survol ; masquée uniquement si **verte et non survolée** ; visible au survol quelle que soit la couleur. Les autres boutons de la ligne de titre (Groupes, Météo) restent à câbler.

5. **Carte Mapbox** (`src/components/Accueil/Map.vue`) :
   - Carte Mapbox GL (style `standard`, token depuis `Systeme.Key.mapBox`).
   - **Sources GeoJSON** : `traces` (clusterisée, points de départ, `cluster: true`, `clusterRadius: 50`, `clusterMaxZoom: 14`), `favorites` (LineString favoris), `displayed-traces` (LineString dégradé, `lineMetrics: true`), `focus-traces` (LineString isolée en mode focus).
   - **Couches** (du bas vers le haut) : `clusters` / `cluster-count` / `unclustered-point` (points de départ) ; `favorites-line` (couleur favori, épaisseur 6) ; `displayed-traces-line` (dégradé bleu→rouge, épaisseur 4) ; `focus-traces-line` (même dégradé, masquée par défaut).
   - **Synchronisation carte ↔ store** : `moveend` (debounce 150 ms) → `tracesStore.updateMapCenter()` → `scheduleVisibleRefresh()` → à l'état stable (`idle`) : `queryRenderedFeatures` sur les couches `unclustered-point` et `clusters`, `getClusterLeaves` pour extraire les feuilles, dédoublonnage → `tracesStore.setVisibleTraceIds()` → invalidation du getter `visibleTracesByDistance` → mise à jour de la liste. Pendant un focus, le calcul est court-circuité (stabilité du drawer).
   - **Vue mémorisée (centrage/zoom)** : le centre (`[2.0, 43.7]`) et le zoom (`5.15`) par défaut (France/Espagne) sont restaurés depuis les paramètres **cachés** `Carte.Vue.*` (centreLat/centreLng/zoom, floats — groupe non listé dans `_meta`, donc invisible du drawer). Au montage, `loadPersistedView()` lit ces valeurs et les passe à `initializeMap` (via le constructeur Mapbox). À chaque `moveend` (debounce 150 ms, **hors focus**), `persistMapView()` sauvegarde le centre/zoom courant via `updateSetting` — n'écrivant que ce qui a changé (`lastSavedView`). La vue est ainsi restituée au lancement **et** au retour sur la vue (la carte est remontée à chaque navigation, aucun KeepAlive).
   - **Interactions** : clic cluster → `easeTo` vers le centre au zoom d'expansion ; clic point → popup (nom, source, coordonnées) ; curseur `pointer` au survol.
   - **Focus carte** : `watch(tracesStore.focusedTraceId)` → sauvegarde de la vue, masquage des couches favoris/affichées, affichage isolé de la trace dans `focus-traces-line`, cadrage par `fitBounds`, retour par `flyTo` (durée `Carte.Traces.dureeFlyTo`).
   - **Réactivité** : `watch(traces)` → `setData()` sur les sources + `scheduleVisibleRefresh()` pour recalculer les visibles ; `watch(settings)` → `setPaintProperty` pour le style ; événement `idle` → consomme `pendingVisibleRefresh` pour effectuer le calcul après stabilisation du rendu.

6. **Utilitaire géographique** (`src/utils/geo.ts`) :
   - Fonctions nommées exportées (pattern `format.ts`) : `toRadians()`, `haversineMeters()`.
   - R = 6 371 000 m (cohérent avec le backend Rust `import_gpx.rs`).

5. **Notifications** (`src/stores/ui.ts`) :
   - Store mutualisé pour les snackbars Vuetify (succès, erreur, avertissement, info).

## Nettoyage de trace GPX (`/nettoyage`)

Une trace GPX n'est **valide** que si elle est « propre ». Les fichiers GPX édités (OpenRunner, etc.) contiennent souvent des anomalies de relevé : **points isolés hors trace** (ex. point 946) ou **aller-retours inutiles** (ex. points 711/791) — détectables par un **changement de cap proche de 180°** au point de demi-tour. Une trace non « clean » ne peut **pas** entrer en édition caméra.

### État de nettoyage (`cleaning_status`)

`TraceMetadata` porte un champ `cleaning_status` (Rust + TS) : `"clean" | "needs_review" | "in_progress"`, défaut **`"clean"`** (`#[serde(default)]`, rétrocompatibilité avec les registres antérieurs). Il est posé **à l'import** : détection automatique des anomalies (§ ci-dessous) → `"needs_review"` si anomalies, sinon `"clean"`.

**Blocage de l'édition caméra** : une trace non « clean » ne peut pas entrer dans la vue d'édition caméra. Le bouton Éditer de l'accueil (`Circuit.vue`) redirige vers `/nettoyage` ; `EditionCamera.vue` dispose d'un **garde-fou** qui redirige également vers `/nettoyage` au montage si `cleaning_status !== 'clean'`.

### Architecture

1. **Module backend** (`src-tauri/src/cleaning.rs`) :
   - **Détection** : pour chaque point, le cap vers le point précédent et vers le point suivant sont comparés ; s'ils sont quasi identiques (différence ≤ tolérance), le point est un **rebroussement** (~180°). Les rebroussements proches (fenêtre de 25 index) sont regroupés en un **cas** ; la zone de déviation est délimitée par retraçage symétrique (points jumeaux aller/retour). Classification : branche courte (< 100 m) → `spike` (point isolé, suggestion : supprimer l'apex) ; sinon → `out_and_back` (suggestion : supprimer le demi-tour + le retour). Le type `manual` (créé par l'utilisateur) n'est jamais produit par la détection.
   - **Tolérance paramétrable** : `Nettoyage.Cap.toleranceDeg` (float, défaut 5.0, min 1.0, max 20.0, step 0.5, unité `°`), lu via `read_tolerance_deg` (repli sur 5° si absent). Déclaré dans `src-tauri/settings.default.toml` avec l'entrée `[_meta.views.nettoyage]` (icône `mdi-broom`) et le groupe `[_meta.groups."Nettoyage.Cap"]` (« Nettoyage — Détection »).
   - **Persistance des décisions** : fichier de travail `{mode}/cleaning/{trace_id}.json` (écriture atomique). Chaque cas porte un état de validation `pending` / `corrected` / `kept`, des plages de suppression et des points **déplacés** (`MovedPoint` : index original + nouvelles coordonnées — les index sont **originaux**).
   - **Cycle de vie** : `needs_review` (anomalies à l'import) → `in_progress` (première sauvegarde partielle) → `clean` (finalisation). `reset_cleaning` ramène à `needs_review`.
   - **Finalisation** : refuse tant que **tous** les cas ne sont pas validés ; applique les corrections (suppressions + déplacements), génère le **GPX nettoyé** (1.1, `<trkseg>` unique, lat/lon 6 décimales, altitude et timestamp préservés), sauvegarde l'original en **`{filename}.gpx.orig`** (une seule fois, jamais écrasé), régénère les dérivés (geojson, stats, hash) et supprime le fichier de travail.

2. **Store Frontend** (`src/stores/cleaning.ts`) — Pattern Setup Store :
   - Types miroir des structs Rust (`CleaningCaseKind`, `CleaningCase`, `Correction`, `MovedPoint`, `CleaningState`).
   - État : `selectedTraceId`, `state` (détection + décisions), `points` (index GPX), `currentCaseIndex`, `toleranceDeg` + état UI éphémère `createMode` / `createStartIndex` / `movePointIndex`.
   - Getters : `hasCases`, `currentCase`, `currentZone`, `correctedZoneCoords` (linestring corrigé : suppressions retirées, déplacements appliqués), `allValidated` (tous les cas ≠ `pending`), `validatedCount`, `isDeletedCount`.
   - Actions : `load` (reprise du travail en cours via `get_cleaning_state`, sinon détection via `detect_trace_anomalies`), `reDetect`, validation manuelle des cas (« Corriger & valider » / « Conserver tel quel » pour les faux positifs), `applySuggestion`, `toggleDeletePoint`/`addDeleteRange`/`clearCorrection` (suppressions), `setMovedPoint`/`clearMovedPoint`/`startMovePoint`/`stopMovePoint` (déplacements), `toggleCreateMode`/`cancelCreate`/`createManualCase`/`removeCase` (cas manuels), `save` (sauvegarde partielle via `save_cleaning_state`, le GPX original reste intact), `finalize` (`finalize_cleaning`), `reset` (`reset_cleaning`).

3. **Vue** (`src/views/Cleaning.vue`, route `/nettoyage`) :
   - Plein écran (style Accueil/EditionCamera) : toolbar + carte Mapbox + panneau des cas + table des points + drawer Paramètres.
   - Garde-fou : sans trace sélectionnée → retour à l'accueil. Détection de modifications non sauvegardées (dialog de retour), boutons **Enregistrer** (sauvegarde partielle) et **Finaliser** (actif uniquement quand tous les cas sont validés).

4. **Composants** (`src/components/Cleaning/`) :
   - `CleaningToolbar.vue` : barre d'outils (retour accueil, titre « Nettoyage — {trace} », Enregistrer, Finaliser).
   - `CleaningMap.vue` : carte Mapbox GL — **trace complète en ligne continue verte** (contexte global, bouton flottant « Trace complète » pour le cadrage), **segment courant** (zone du cas) surligné, **linestring corrigé** (ligne jaune à halo blanc : résultat réel des suppressions/déplacements, mis à jour en direct), branches **aller/retour** décalées perpendiculairement (`line-offset`) et colorées différemment pour les passages superposés, points numérotés (index GPX) **cliquables** avec **anti-revouvrement des labels** (positions **mémorisées par cas** : une fois affichés, les labels restent stables pour un clic fiable), points supprimés en **rouge**, **déplacement direct à la souris** des points des cas **manuels** (drag ; un simple clic bascule la suppression), **sélecteur des points proches du curseur** (liste cliquable des N° d'ordre, verrouillée après un clic ambigu, quand plusieurs points sont superposés), **mode « Créer une anomalie »** (2 clics : début puis fin, snap au point de trace le plus proche).
   - `CleaningCasesPanel.vue` : liste des anomalies (n°/total validés, type, écart de cap, zone), validation de chaque cas, bouton **« Créer une anomalie »** (désignation d'un segment sur la carte) et **« Supprimer ce cas »** (cas manuel non validé).
   - `CleaningPointTable.vue` : table **simplifiée** des points du segment courant (numéro, case de suppression, indicateur **« Déplacé »** cliquable pour annuler) — le déplacement se fait directement sur la carte, pas dans le tableau.

5. **Responsabilité de validation** : la détection est **propositive** — chaque cas (détecté ou créé manuellement) doit être **validé par l'utilisateur** (« Corriger & valider » ou « Conserver tel quel » pour les faux positifs) avant de passer au suivant ; la **finalisation n'est possible que quand tous les cas sont validés**. Le GPX original n'est remplacé qu'à la finalisation.

### Commandes Tauri du module Nettoyage

| Commande | Description |
|----------|-------------|
| `detect_trace_anomalies` | Re-parse le GPX original et retourne les anomalies détectées (aucune persistance). |
| `get_cleaning_state` | Retourne le fichier de travail `cleaning/{trace_id}.json` s'il existe, sinon une détection fraîche. |
| `save_cleaning_state` | Sauvegarde partielle du travail (écriture atomique) et passe la trace en `"in_progress"`. |
| `reset_cleaning` | Abandonne les corrections en cours (supprime le fichier) et repasse en `"needs_review"`. |
| `finalize_cleaning` | Applique les corrections validées, génère le GPX nettoyé + backup `.orig`, régénère geojson/stats/hash, repasse la trace en `"clean"`. Refuse tant que des cas sont `pending`. |

## Vue d'édition caméra (`/edition-camera`)

La vue d'édition caméra (Phase 2 de la spec « Visualisation GPX sur MapBox ») est l'interface de réglage de la caméra qui suit une trace. Elle dispose d'un **playback fonctionnel** avec : génération de keyframes, boucle d'animation `requestAnimationFrame`, contrôles de lecture (Composant A), **graphe SVG d'avancement (§4.6)** et HUD de télémétrie (Composant C). S'y ajoutent le **Composant B** (édition fine des keyframes via `CameraEditor`) et l'**algorithme intelligent de frustum** (spec §3.3) — le tout décrit ci-dessous.

### Architecture

1. **Génération des keyframes** (`src/algorithms/keyframeGenerator.ts`) :
   - Module isolé **sans dépendance UI** (spec §3.2) — ne dépend que de `utils/geo`. Remplaçable sans impacter le reste de l'application.
   - Définit les types du format JSON figé (spec §3.4) : `CamState`, `TraceurPoint`, `Keyframe`, `KeyframeSet`.
   - `generateKeyframes(traceId, feature, sampleStepM = 250, tracePoints?, algorithm = 'frustum', minKeyframeGapM = 1000, terrainSampler?, viewport?)` — deux algorithmes au même format de sortie :
     - **`'simple'` (MVP)** : construit la polyligne indexée par distance cumulée (Haversine), échantillonne un keyframe tous les `KEYFRAME_STEP_M`, interpole la position exacte sur la polyligne à la distance cible, et oriente chaque caméra vers le keyframe suivant (`bearing()`). `time_ms = distance × MS_PER_METER` (vitesse défaut 4000 ms/km). La caméra suit fidèlement la trace.
     - **`'frustum'` (spec §3.3)** : délègue à `src/algorithms/frustum.ts` (placement récursif par visibilité, cf. ci-dessous).
   - **Viewport par ratio d'écran** : `ViewportAspect = '16:9' | '4:3'` et `VIEWPORTS_BY_ASPECT` (1920×1080 / 1440×1080 — même hauteur, le FOV horizontal seul diffère). Le viewport est transmis à l'algorithme et stocké dans le champ `viewport` du JSON. Un jeu de keyframes est **distinct par ratio** (`{trace_id}_169.json` / `{trace_id}_43.json`).
   - Helpers purs d'interpolation (consommés par la boucle de lecture) : `findSegment` (dichotomie O(log n)), `interpolateCam` (bearing interpolé **sur le cercle** — `lerpAngle`, chemin le plus court — pour un **lissage fluide** du cap d'un point de RdV au suivant, sans à-coup ni rotation 360°), `interpolateTraceur`.
   - **Polyligne indexée par distance** : `buildTracePolyline(feature)` (fallback GeoJSON 2D) et `buildTracePolylineFromPoints(points)` (depuis `TracePoint[]` backend, distances Haversine 2D recalculées + altitude) construisent un tableau de `PolyVertex { lng, lat, d, altitude }` indexé par distance cumulée. `samplePolylineAt(poly, distanceM)` retourne `{ lng, lat, altitude }` (interpolation linéaire de l'altitude entre sommets encadrants). Le curseur les utilise pour avancer **le long de la trace réelle** (et non entre les keyframes échantillonnés), de sorte qu'il épouse les virages au lieu de tirer des cordes droites.

1-bis. **Algorithme de frustum** (`src/algorithms/frustum.ts`, spec §3.3) — placement récursif par visibilité, module pur sans Mapbox :
   - **Principe** : départ avec les keyframes A (départ) et Z (arrivée) ; pour chaque segment, la caméra vole la **corde A→Z** (position interpolée le long de la ligne directe), le traceur suit la trace courbe. Chaque point GPX intermédiaire est testé : projection **frustum perspective complète** (`isInFrustum`, base orthonormée avant/droite/haut + focale en pixels, FOV vertical ~36,87°, **marge `FOV_MARGIN = 0.85`** pour que le curseur reste confortablement dans le cadre) **et** ligne de visée contre le relief (`losOccluded`).
   - **Occlusion par le relief** : `losOccludedByTerrain` échantillonne l'altitude DEM le long de la LOS caméra→point (pas de 50 m), tracée depuis la **position au sol de la caméra** (déduite via `mercatorToLngLat` de sa position 3D — et non du centre de la corde, qui est ~2,4 km en avant). Le module reste indépendant de Mapbox via le type `TerrainSampler = (lng, lat) => number | null` fourni par l'appelant. Sans DEM, fallback sur les points de trace intermédiaires.
   - **Réorientation oblique** : si un point est masqué par le relief, on tente une réorientation du bearing **face au versant** en oblique (30°/45°/60° des deux côtés de la trace, jamais à 90°) ; on garde le candidat qui rend le plus de points visibles.
   - **Insertion récursive** : si l'échec persiste (hors champ latéral/vertical), on insère un keyframe N au point de **moindre courbure** (`bestInsertionPoint`, courbure **moyenne** sur une fenêtre ±3 points — la moyenne évite de pénaliser les bords où la fenêtre est tronquée) et on réitère sur [A,N] et [N,Z]. Anti-surabondance : découpage limité par `minKeyframeGapM` (et `gap/2` en affinage).
   - **Affinage à deux passes** : après résolution, chaque segment est re-validé avec le bearing **interpolé** réel de la lecture (`lerpAngle` entre les caps des deux keyframes) — la résolution par cap constant peut laisser des points sortir du cadre quand la caméra tourne — et re-découpé si besoin.
   - **Modèle d'altitude exagéré** : la caméra est à `terrain(centre) × 1.5 + dist × cos(pitch)` et le traceur à `terrain(traceur) × 1.5` quand le DEM est disponible — cohérent avec le rendu `setTerrain({ exaggeration: 1.5 })` (les altitudes GPX de la route, non exagérées, créaient des fausses occlusions).

2. **Store édition** (`src/stores/edition.ts`) — Pattern Setup Store, état UI éphémère non persisté :
   - **Sélection / cadre** : `selectedTraceId`, `showViewportFrame` (défaut **affiché**) ; **ratio d'écran** `viewportAspect` (`'16:9'` défaut | `'4:3'`) + action `setViewportAspect(aspect)` (affiche le cadre, remet à zéro la sélection keyframe — EditionMap rechargera le fichier du ratio choisi) ; actions `selectTrace`, `clearSelection`, `toggleViewportFrame`.
   - **Algorithme de génération** : `keyframeAlgorithm` (`'frustum'` défaut | `'simple'`), `minKeyframeGapM` (défaut 1000, bornes 200–5000, pas 50) ; actions `setKeyframeAlgorithm`, `setMinKeyframeGapM` qui **forcent la régénération** (EditionMap écoute le changement) et remettent à zéro la sélection du keyframe en cours.
   - **Lecture** : `keyframeSet`, `isPlaying`, `speed` (0.5/1/2/4), `currentTimeMs`.
   - **Getters** : `hasKeyframes`, `totalDistanceM/Km`, `totalDurationMs`, `currentDistanceM/Km`, `progressRatio`, `currentSegment`, `interpolatedCam` (interpolation entre keyframes), `interpolatedTraceur` (échantillonné **le long de la polyligne réelle** à la distance courante — suit les virages, inclut `altitude` interpolée), `markerDistanceM` (haversine cam↔traceur), `markerRelativeBearing` (cap relatif normalisé [-180,180]), `altitudeAtDistance(m)` (closure capturant la polyligne réactive — retourne l'altitude interpolée à une distance donnée, utilisée par le tooltip du ProgressGraph), `currentKeyframe` (keyframe situé à la position courante, tolérance 1 m — pilote le mode du CameraEditor), `canGoNextRdv`/`canGoPrevRdv`.
   - **Actions édition keyframes (Composant B)** : `updateKeyframe(distanceM, updates)` (mute le `cam` d'un keyframe, **sans sauvegarde automatique**), `addKeyframe(distanceM, cam?, traceur?)` (insère un keyframe — interpolation depuis les voisins si non fourni), `removeKeyframe(distanceM)` (supprime — le **km 0 est intouchable**, min 2 conservés), `saveKeyframes()` (sauvegarde **explicite** sur disque), `selectKeyframe(distanceM)`.
   - **Actions lecture** : `setKeyframeSet(set, feature, tracePoints?)` (reset temps + pause, construit la polyligne depuis les `tracePoints` backend si fournis, sinon depuis la feature GeoJSON), `play`/`pause`/`togglePlay`, `setSpeed`, `seekToDistance`, `tick(deltaMs)` (avance le temps de `delta × speed`, **pause auto en fin de course**), `goToNextRdv`/`goToPrevRdv` (**pause auto** si lecture + seek + sélection du keyframe).
   - **Changements de cap (analyse)** : `headingChanges` (tous les virages entre keyframes consécutifs via `computeHeadingChanges` — module pur `algorithms/headingChanges.ts`), `brutalHeadingChanges` (filtrés par taux ≥ seuil), `headingChangeThresholdDegPerKm` (défaut 45, bornes 10–1000, pas 5) + `setHeadingChangeThreshold` (**simple filtre d'affichage**, aucune régénération), `showHeadingChangesPanel` + `toggleHeadingChangesPanel` (panneau tableau). Critère « brutal » : taux = `|Δcap| / Δdist` en °/km ; sens de rotation = signe de `bearingDelta` (≥ 0 → **horaire**, < 0 → **anti-horaire**).
   - **Verrous de segments (mode validation)** : `validationMode` + `toggleValidationMode()` (bouton `mdi-camera-lock`), `lockedSegmentFromDistances` (Set des distances de départ des segments verrouillés, dérivé des flags `locked` portés par les keyframes), `currentSegmentFromDistance`, `isSegmentLocked(fromDistanceM)` / `isKeyframeLocked(distanceM)` (un keyframe est verrouillé s'il borde un segment verrouillé). **Traits bleus persistés** dans `Keyframe.marks` (distances du curseur à chaque pose — plusieurs par segment, écrits par `markValidationClick`, supprimés par `lockSegment`, relecture depuis le JSON keyframes). Actions `markValidationClick()` (clic carte ou Entrée → pose un trait bleu persisté + segment déverrouillé pendant **ce parcours**), `lockSegment` / `unlockSegment` / `toggleSegmentLock` (bascule par **double-clic** sur le segment de la timeline), persistance via `saveKeyframes()`. Un `watch(currentSegmentFromDistance)` verrouille le segment **précédent** quand le curseur le quitte **sans marque posée pendant ce parcours** (verrouille + efface ses traits persistés) ; une marque posée pendant le parcours le protège uniquement pour ce parcours, puis est **consommée** à la sortie — un re-parcours sans Entrée le reverrouillera (aucune purge globale au retour km0, une vérification peut se faire segment par segment). Les **seeks manuels** (clic timeline, navigation RdV, km0, animation fly-to — signal `suppressAutoLock`) sont **neutres** : ni verrouillage, ni consommation de marque ; seule la progression naturelle de la lecture (`tick`) traverse et verrouille. **Gardes** : `updateKeyframe` / `removeKeyframe` bloqués si le keyframe est verrouillé, `addKeyframe` bloqué dans un segment verrouillé. Verrouillage monotone (le clic ne déverrouille jamais) ; verrous et marques réinitialisés à la régénération des keyframes.

2-bis. **Store keyframes** (`src/stores/keyframes.ts`) — Pattern Setup Store, persistance des keyframes sur disque :
   - Actions `loadKeyframes(traceId, viewportAspect)` : charge les keyframes du ratio depuis `keyframes/{trace_id}_169.json` / `{trace_id}_43.json` via `get_keyframes`. Retourne `null` si absent ou invalide (validation minimale : `trace_id` + `keyframes` non vide).
   - Actions `saveKeyframes(set)` : sauvegarde un `KeyframeSet` via `save_keyframes` (écriture atomique côté Rust). Le ratio (et donc le fichier cible) est **dérivé du champ `viewport`** du jeu — jamais de désynchronisation possible.
   - Actions `clearKeyframes(traceId, viewportAspect)` : supprime le fichier via `delete_keyframes` (tolérant si absent).
   - Utilisé par `EditionMap.vue` : charge les keyframes persistés en priorité, sinon génère et sauvegarde (best-effort).

3. **Vue** (`src/views/EditionCamera.vue`) :
   - `v-main` en **colonne flex** : un wrapper carte (`position: relative`, `flex: 1`) contenant `EditionMap` + overlays `ViewportFrame`, `TelemetryHud` et `DistanceHud`, puis `PlaybackControls` en bandeau bas fixe.
   - Au montage : précharge `appStore`, `settingsStore`, `tracesStore`. **Garde-fou** : si `selectedTraceId` est `null` (rechargement direct), `router.replace({ name: 'accueil' })` ; si la trace sélectionnée n'est pas « clean » (`cleaning_status !== 'clean'`), `router.replace({ name: 'nettoyage' })` (une trace doit être nettoyée avant l'édition caméra, cf. § « Nettoyage de trace GPX »).

4. **Carte + lecture** (`src/components/Edition/EditionMap.vue`) :
   - Carte Mapbox GL **dédiée** (distincte de `Accueil/Map.vue`). Style `standard-satellite` ; **terrain/élévation** via source `raster-dem` (`mapbox-terrain-rgb`) + `setTerrain({ exaggeration: 1.5 })` ; pitch 60° par défaut (spec §7).
   - **Grille terrain fine pour l'occlusion** (`buildTerrainGrid`) : `queryTerrainElevation` ne lit que les tuiles DEM chargées pour la caméra courante (zoom ~10 = ~60 m/px au moment de la génération, trop grossier pour les buttes côtières). On télécharge donc les tuiles **`mapbox.terrain-rgb` à zoom 13 (~7 m/px)** sur l'emprise de la trace (pad ~2,5 km pour couvrir la LOS), on décode l'altitude RGB→m (`-10000 + ((R·256² + G·256 + B) × 0.1)`) dans un canvas, et on construit une grille ~100 m (`TERRAIN_GRID_STEP_DEG = 0.001`) exagérée ×1,5. `sampleTerrain(lng, lat)` sert de `TerrainSampler` à l'algorithme frustum ; la **régénération est déclenchée** dès que la grille est prête si la génération initiale n'a pas pu en bénéficier (`scheduleTerrainRegeneration`).
   - Charge la géométrie via `tracesStore.getTraceGeometry` (LineString **rouge paramétrable** `Edition.Couleurs.trace`, épaisseur paramétrable `Edition.Couleurs.epaisseurTrace`), les points riches via `tracesStore.getTracePoints` (altitude + distance), pose le **curseur bordé de blanc** (CircleLayer WebGL `edition-marker-dot`, couleur paramétrable `Edition.Couleurs.curseur`, source GeoJSON point mise à jour via `source.setData()`), **positionne directement la caméra au départ (km 0)** — pas de vue globale (`fitBounds` d'emprise supprimé), **puis charge les keyframes persistés du ratio actif** via `ensureAspectKeyframes(aspect, { active })`. Si absents ou invalides, les génère et les sauvegarde (best-effort) via `keyframesStore.saveKeyframes` avant de les pousser dans `editionStore` avec les `tracePoints` (polyligne enrichie altitude). Avant la génération, le composant charge et applique les **paramètres d'édition** (`settingsStore.loadSettings()` + `editionStore.applySettings(true)`) : algorithme, gap min, zoom/pitch par défaut (`defaultZoom`/`defaultPitch` transmis à `generateKeyframes`) et viewport initial. Les couleurs/épaisseur de la trace et la couleur du curseur sont **réactives** (`watch` → `map.setPaintProperty`). **Écran vierge au lancement** : la carte est montée masquée (`.map-loading` : `opacity: 0`) tant que `editionStore.editionViewReady` est `false` ; la vue (carte + overlays + bandeau) n'est révélée qu'au premier événement Mapbox **`idle`** (toutes les tuiles visibles chargées) — `revealView` positionne la caméra au km 0 puis **force le recalcul de l'altitude caméra** (`repositionCameraForTerrain` : le setter `center` de Mapbox v3 ne recalcule que si le centre change, d'où un double saut sub-pixel ; le relief chargé relève le terrain, sans ce recalcul le curseur serait décalé vers le bas) — avec un **`v-progress-circular` centré** pendant le chargement.
   - **Deux fichiers par trace (un par ratio)** : `ensureAspectKeyframes(aspect, { active, force? })` charge le fichier persisté du ratio sinon le génère (`generateKeyframesFor`, viewport par ratio) ; ne pousse dans le store que le **ratio actif**. À l'ouverture, le ratio actif (16:9 par défaut) est chargé/généré puis **l'autre ratio est garanti en arrière-plan**. Le changement de ratio (watcher `viewportAspect`) re-charge/génère les deux ; un changement d'**algorithme/gap régénère les deux fichiers** (`force`). Les ratios générés avant la grille fine du relief sont mémorisés (`aspectsPendingTerrain`) et **régénérés** dès que la grille est prête.
   - **Boucle d'animation** pilotée par `editionStore.isPlaying` : une `requestAnimationFrame` calcule le delta réel (`performance.now()`), déclenche `tick(delta)`, puis applique l'état interpolé (`map.jumpTo` pour la caméra, `source.setData()` pour le curseur). En pause, un `watch(currentTimeMs)` **anime** le déplacement vers la nouvelle position (clic timeline, navigation RdV…) sur la durée paramétrable `Edition.Camera.dureeFlyTo` (100–1000 ms, défaut 250) : `startSeekAnimation` pose `currentTimeMs` frame par frame avec un easing cubique (caméra + marqueur + curseur timeline glissent ensemble), en court-circuitant le watcher (`suppressSeekWatch`). La boucle de lecture annule toute animation de seek en cours. Arrêt propre du rAF au démontage.
   - **Mode validation** : un handler `map.on('click')` (le premier clic de la carte édition) appelle `editionStore.markValidationClick()` — en validation, un clic sur la carte signale un « problème » sur le segment courant (il reste déverrouillé).
   - **CircleLayer vs Marker DOM** : le curseur utilise une CircleLayer (pipeline WebGL) au lieu d'un `mapboxgl.Marker` (élément DOM). Cela garantit une synchronisation parfaite avec le terrain 3D pendant les mouvements rapides de caméra (virages serrés, lacets), là où un Marker DOM se désynchronise de la projection WebGL.

5. **Composants** (`src/components/Edition/`) :
   - `EditionToolbar.vue` : `v-app-bar` semi-transparente. Titre de la trace, **bouton ViewPort** (**flip-flop** 16:9 / 4:3 — icône `mdi-monitor` en 16:9 / `mdi-monitor-small` en 4:3, chaque ratio exploite son propre fichier keyframes et son cadre ; **couleur** = avancement du verrouillage : vert 100 %, jaune > 50 %, orange ≥ 10 %, rouge sinon), bouton **Mode validation** (`mdi-camera-lock`, surligné quand actif, arme le verrouillage automatique des segments pendant la lecture), bouton **Paramètres** (`mdi-cog-outline` → émet `open-settings` ; **flip-flop** : ouvre/ferme le panneau, surligné `primary` quand actif — comme sur l'Accueil) et bouton **Quitter** (`mdi-location-exit`, retour à l'accueil). Algorithme / Gap min / zoom/pitch / viewport / couleurs sont gérés dans le **panneau Paramètres** (cf. `SettingsDrawer`), plus dans la barre.
   - `ViewportFrame.vue` : overlay CSS pur (z-index 5, `pointer-events: none`). Rectangle **au ratio sélectionné (16:9 ou 4:3)** centré (plus grand possible, mesuré au `ResizeObserver`), masque sombre ~70 % via `box-shadow` gigantesque, trait blanc 2 px, libellé « ViewPort · 16:9 · 1920×1080 » (ou 4:3 · 1440×1080). **En mode validation**, le cadre et son libellé passent au **bleu** (`#2196F3`, classe `.mode-validation`). Purement informatif.
   - `PlaybackControls.vue` (Composant A, spec §4.3) : bandeau bas compact (~82 px), en **layout horizontal** — le graphe SVG d'avancement à gauche + une colonne de boutons à droite (2 rangées alignées sur les zones du graphe : RdV précédent/suivant en **vert** (comme les ticks RdV) au-dessus, km0 / Play-Pause / **lecture accélérée** (double chevron `mdi-chevron-double-right`, ×`acceleration`) / dernier point en dessous), **regroupés du même côté que les widgets d'édition**. **Vitesse normale 1×** ; le bouton double chevron bascule la lecture accélérée au facteur paramétrable `Edition.Playback.acceleration` (1.5–8, défaut 2). La navigation entre RdV délègue au store (`goToNextRdv`/`goToPrevRdv`).
   - `ProgressGraph.vue` (spec §4.6) : graphe SVG d'avancement inséré dans `PlaybackControls`. Timeline horizontale **proportionnelle à la trace** (3 px / 100 m), **alignée à droite quand elle est plus courte que le viewport** (ResizeObserver + `margin-left: auto`). Trois zones de haut en bas : **Points de RdV** (ticks cliquables, keyframes, hauteur limitée et centrés, rendus **au-dessus** du curseur ; chaque tick est **scindé en deux zones verticales** — zone supérieure = **ZOOM** (rouge `#F44336` si ≠ `zoomDefaut`), zone inférieure = **PITCH** (orange `#FF9800` si ≠ `pitchDefaut`), vert `#4CAF50` sinon — pour identifier les RdV au cap modifié vs tilt/zoom ; **trait rouge** en haut de zone pour chaque segment **non verrouillé** (disparaît à la validation) ; entre deux ticks consécutifs, une **bande colorée** pour **chaque** changement de cap, **sous** le curseur orange — couleur = sens (teal **horaire** / deep-purple **anti-horaire**), **épaisseur** = intensité du taux : trait 2 px sous le seuil puis +2 px par bande de 30 °/km (45, 75, 105, 135…), tooltip SVG « Δcap · distance · taux » ; **double-clic** sur un segment → **bascule du verrou** (Verrouiller / Déverrouiller selon l'état, `toggleSegmentLock`)), **Avancement** (piste + jauge jaune + repères 10 km cliquables + curseur 3 px **étendu verticalement** sur toute la hauteur des zones RdV — couleur pilotée par le paramètre `Edition.Couleurs.curseur` — + avancement), **Graduation** (libellés « X km » — premier/dernier alignés sur les bords pour rester visibles, dernière dizaine masquée si le total est à < 2,5 km d'un multiple de 10 — + tooltip de survol blanc, distance + altitude via `altitudeAtDistance`). Clic → `seekToDistance` + `selectKeyframe` (sélection pour l'éditeur). **Scroll DOM natif** (viewport `overflow-x: auto`, scrollbar masquée en CSS) pour un rendu fiable sur les traces longues (contrairement à une translation SVG, cullée par le moteur de rendu). **Auto-scroll** centré sur le curseur (~30 % du viewport) pendant la lecture : boucle `requestAnimationFrame` dédiée, détection du scroll utilisateur par **comparaison de valeur** (`lastProgrammaticScrollLeft`) et non par un flag booléen (les events `scroll` sont asynchrones — un flag levé autour de `scrollLeft` serait déjà réinitialisé quand l'event se déclenche). Suspendu 1,5 s après un scroll manuel (molette ou drag).
   - `TelemetryHud.vue` (Composant C, spec §4.5) : overlay coin supérieur droit, fond semi-transparent sombre, texte blanc. Paramètres caméra (Zoom/Pitch/Bearing/Lng/Lat) + section Traceur (Altitude interpolée depuis la polyligne) + relation caméra↔curseur (Distance/Cap). **Masqué par défaut** : visible uniquement si le paramètre `Edition.Camera.afficherTelemetrie` (bool) est actif et si des keyframes sont chargés ; bouton **Fermer** en en-tête (comme le panneau des caps brutaux, `pointer-events: auto`) — la fermeture **vaut pour la session** (la sauvegarde d'un autre paramètre ne réaffiche pas le HUD ; seul un changement du paramètre lui-même le réaffiche). **Lignes Zoom et Pitch colorées** comme les ticks RdV de la timeline : **vert** quand la valeur est celle par défaut (`zoomDefaut`/`pitchDefaut`), **orange** (pitch ≠ défaut) / **rouge** (zoom ≠ défaut) sinon — comparaison avec **tolérance** (moitié du pas de réglage) pour absorber la dérive de l'interpolation en lecture.
   - `DistanceHud.vue` : overlay **barre du bas** (à gauche des boutons d'action), fond semi-transparent sombre, affiche `X.XX / Y.YY km` (distance parcourue en **orange** `#FF9800`, distance totale en blanc atténué). **En mode validation**, le fond passe au **bleu** (`rgba(33,150,243,.85)`). Intégré dans `CameraEditor`. Masqué sans keyframes.
   - `HeadingChangesPanel.vue` : overlay **tableau** (haut-gauche, `pointer-events: auto`), affiché par `showHeadingChangesPanel` — **affichage par défaut paramétrable** (`Edition.Camera.afficherCapBrutaux`, comme le HUD de télémétrie), fermé par son bouton Fermer. Liste les changements de cap **brutaux** (`brutalHeadingChanges`) : `Départ km`, `Arrivée km`, `Δ dist km` (2 décimales), `Δ cap` (signé), `Taux °/km` — **code couleur par colonne** : Départ/Arrivée **jaune** pour le segment sous le **curseur d'avance** (auto-scroll centré), **Δ dist/Δ cap** colorés par le **sens** (teal horaire / deep-purple anti-horaire), **Taux °/km** coloré **jaune → rouge** par bande de taux (45, 75, 105, 135… via `headingRateColor`). Clic sur une ligne → `seekToDistance` + `selectKeyframe` (début du virage). Légende (teinte sens + code couleur taux) + **champ seuil °/km** (10–1000, pas 5 — filtre du tableau). `v-table` compact, fond sombre translucide, scroll interne si trop de lignes.
   - `CameraEditor.vue` (Composant B, spec « Interface de contrôle MapBox ») : widgets de manipulation directe superposés sur la carte, **pilotés par la position de lecture** (`currentKeyframe`). **Hors RdV** → bouton « Ajouter un point de RdV » (sous le compas). **Sur un RdV** → widgets : **switch Cible** (pitch à 0° + croix bleue + drag sur carte pour viser, sauvegarde `cam.lng/lat`), **sliders Pitch/Zoom customs** (drag vertical + molette ±1 pas, double-clic ou clic sur valeur orange pour remettre les **valeurs par défaut des paramètres** `Edition.Camera.pitchDefaut`/`zoomDefaut`, **vert** sur la valeur par défaut sinon **bleu**), **CompassBandeau** (bandeau ±90°, défilement **infini** sur 3 copies -360°…720°, drag + molette ±1°, repère rouge fixe). **Barre d'actions** en bas : Undo (restaure la baseline), Supprimer (grisé sur le km 0), Sauvegarder (**grisé tant que non modifié** — sauvegarde explicite). **Verrouillage carte** : sur un RdV, toutes les interactions Mapbox sont désactivées tant que le mode Cible est inactif. **Keyframes verrouillés** : si le keyframe courant borde un segment verrouillé (mode validation), un badge cadenas s'affiche et les widgets (sliders, compas, Cible, Undo/Supprimer, Ajouter) sont **désactivés** — la protection réelle est portée par les gardes du store. Raccourcis : Espace Play/Pause, flèches ←/→ navigation RdV.

6. **Déclencheur** (`src/components/Accueil/Circuit.vue`) :
   - Le bouton **Éditer** appelle `editerCircuit()` : `editionStore.selectTrace(trace.id)` puis — si `cleaning_status !== 'clean'` (trace à nettoyer) → `cleaningStore.selectTrace(trace.id)` + `router.push({ name: 'nettoyage' })` ; sinon → `router.push({ name: 'editionCamera' })`.

### Carte satellite + terrain (vs. Accueil/Map.vue)

| Aspect | `Accueil/Map.vue` | `Edition/EditionMap.vue` |
|---|---|---|
| Rôle | Navigation, clustering, favoris | Rendu « cinematic » + lecture d'une trace |
| Style | `mapbox://styles/mapbox/standard` | `mapbox://styles/mapbox/standard-satellite` |
| Terrain | Non | `raster-dem` `mapbox-terrain-rgb`, exaggeration 1.5 |
| Pitch | 0 (défaut) | 60° (défaut, spec §7) |
| Sources | `traces` (clusters), `favorites`, `displayed-traces`, `focus-traces` | `edition-trace` (LineString), `edition-terrain` (DEM) |
| Marker | Aucun | CircleLayer WebGL jaune (`edition-marker-dot`), mis à jour via `source.setData()` |
| Animation | Aucune | Boucle `requestAnimationFrame` pilotée par `editionStore` |

### Stockage par mode d'exécution

```
{app_data_dir}/
├── .env                     # Mode actif (APP_ENV_DEV / APP_ENV_PROD)
├── ModeExe.toml             # Définition des modes
└── {active_mode}/           # Ex : OPE, EVAL_essai
    ├── gpx/                 # Fichiers .gpx copiés (nom unique si doublon)
    ├── traces.json          # Registre des traces importées (Vec<TraceMetadata>)
    ├── keyframes/           # Keyframes persistés (un {trace_id}.json par trace)
    ├── cleaning/            # Travail de nettoyage (un {trace_id}.json par trace en cours)
    ├── config-dev.toml      # Surcharges de paramètres (dev)
    └── config.toml          # Surcharges de paramètres (prod)
```

### Commandes Tauri du module GPX

| Commande | Description |
|----------|-------------|
| `import_gpx_file` | Ouvre le sélecteur natif, parse le GPX, copie le fichier, met à jour le registre. Retourne `TraceMetadata`. |
| `get_traces` | Retourne `Vec<TraceMetadata>` pour le mode d'exécution actif. |
| `delete_trace` | Supprime le fichier GPX + le GeoJSON + les keyframes + l'entrée du registre (écriture atomique). |
| `update_trace` | Mise à jour partielle (PATCH) d'une trace : `favorite` et/ou `is_displayed` (persistés). |
| `get_trace_geometry` | Retourne la géométrie GeoJSON d'une trace (cache, ou régénéré depuis le GPX). |
| `get_trace_points` | Retourne les points d'une trace avec altitude et distance cumulée 3D (re-parse le GPX). |
| `save_keyframes` | Sauvegarde un jeu de keyframes dans `keyframes/{trace_id}.json` (écriture atomique). |
| `get_keyframes` | Charge les keyframes persistés d'une trace (`None` si absent). |
| `delete_keyframes` | Supprime le fichier keyframes d'une trace (tolérant si absent). |

> Référence complète des 29 commandes Tauri dans [COMMANDS.md](./COMMANDS.md).

---

**Note** : Cette architecture est conçue pour être simple et extensible. Suivez ces patterns pour maintenir la cohérence du projet.

**Dernière mise à jour** : 2026-08-18
