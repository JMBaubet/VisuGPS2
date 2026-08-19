# Contexte du Projet - VisuGPS2

> **Documentation destinée à Claude Code et aux développeurs**
> Ce fichier fournit le contexte complet du projet pour faciliter le développement assisté par IA.

## Vue d'ensemble

VisuGPS2 est une **application desktop multiplateformes** basée sur Tauri + Vue 3 + Vuetify, conçue pour fonctionner en dual-screen avec communication inter-fenêtres et synchronisation de thème.

Le **workflow principal** de l'application est : **import GPX → nettoyage (si anomalies) → édition caméra**. Une trace n'est **valide** que si elle est « propre » ; les traces importées présentant des anomalies (points hors trace, aller-retours) doivent être nettoyées via la vue `/nettoyage` avant de pouvoir entrer en édition caméra.

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
│   │   └── index.ts          # Routes (Accueil, Visualisation, EditionCamera, Nettoyage, ScreenBis)
│   ├── stores/
│   │   ├── index.ts          # Configuration Pinia
│   │   ├── app.ts            # Store applicatif (thème, displays, modes d'exécution)
│   │   ├── settings.ts       # Store des paramètres de configuration
│   │   ├── traces.ts         # Store des traces GPX importées
│   │   ├── keyframes.ts      # Store de persistance des keyframes (loadKeyframes, saveKeyframes, clearKeyframes)
│   │   ├── edition.ts        # Store de la vue d'édition caméra (trace, lecture, keyframes)
│   │   ├── cleaning.ts       # Store du nettoyage en 3 étapes (détections par phase, validation d.étape)
│   │   └── ui.ts             # Store des notifications (snackbar)
│   ├── algorithms/           # Logique métier isolée, sans dépendance UI
│   │   └── keyframeGenerator.ts  # Génération + interpolation des keyframes caméra
│   ├── utils/
│   │   ├── format.ts         # Helpers de formatage (distance, élévation, durée)
│   │   └── geo.ts            # Utilitaires géographiques (Haversine, tri par distance)
│   ├── plugins/
│   │   └── vuetify.ts        # Configuration Vuetify
│   ├── views/                # Pages de l'application
│   │   ├── Accueil.vue       # Fenêtre principale avec comm. inter-fenêtres
│   │   ├── ScreenBis.vue     # Fenêtre secondaire
│   │   ├── EditionCamera.vue # Édition caméra (carte satellite+terrain, cadre ViewPort)
│   │   ├── Cleaning.vue      # Nettoyage de trace GPX (carte, panneau des cas, table des points)
│   │   └── Visualisation.vue # Page visualisation
│   ├── components/           # Composants réutilisables
│   │   ├── Accueil/          # Composants de la page d'accueil
│   │   ├── Edition/          # Composants de la vue d'édition caméra (carte, toolbar, cadre, lecture, HUD télémétrie/distance, graphe)
│   │   ├── Cleaning/         # Composants de la vue de nettoyage (toolbar, carte, panneau des cas, table des points)
│   │   └── parameters/       # Composants d'édition des paramètres
│   ├── assets/               # Images, styles
│   ├── App.vue              # Layout racine (détection multi-fenêtres)
│   └── main.ts              # Point d'entrée
│
├── src-tauri/                # Code Rust (Tauri 2.x)
│   ├── src/
│   │   ├── lib.rs            # Point d'entrée, orchestration, run()
│   │   ├── display.rs        # Détection écrans (MonitorInfo, get_displays)
│   │   ├── gestionMode.rs   # Gestion modes d'exécution (CRUD, .env)
│   │   ├── settings.rs       # Paramètres de configuration (TOML, secrets)
│   │   ├── import_gpx.rs     # Import de fichiers GPX (parsing, stats, registre)
│   │   ├── cleaning.rs       # Nettoyage en 3 étapes (détections par phase, validation d'étape)
│   │   └── main.rs           # Auto-généré (délègue à lib.rs)
│   ├── capabilities/
│   │   └── default.json      # Permissions fenêtres (main, screen-bis)
│   ├── settings.default.toml # Paramètres par défaut (embarqué)
│   ├── icons/                # Icônes application
│   ├── Cargo.toml            # Dépendances Rust
│   └── tauri.conf.json       # Config Tauri (windows, build, security)
│
├── docs/                     # Documentation
│   ├── CONTEXT.md           # Ce fichier
│   ├── ARCHITECTURE.md       # Architecture détaillée
│   ├── EXTENDING.md         # Guide d'extension
│   ├── SPEC_IMPORT_GPX.md   # Spécification module import GPX
│   ├── SPEC_AFFICHAGE_TRACES.md # Spécification favoris & affichage carte (à implémenter)
│   └── …
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

**Commandes disponibles** (voir [COMMANDS.md](./COMMANDS.md) pour la référence complète avec signatures) :

**Application & affichage** :
- `exit_app()` : Quitte proprement l'application depuis le backend Rust
- `get_displays()` : Retourne la liste des écrans (MonitorInfo[])
- `open_second_window()` : Ouvre la fenêtre ScreenBis sur l'écran secondaire
- `close_second_window()` : Ferme la fenêtre ScreenBis

**Modes d'exécution** :
- `get_execution_env()` : Retourne les infos d'environnement (is_dev, active_mode_dev, active_mode_prod)
- `get_modes()` : Retourne la liste des modes d'exécution (ModeInfo[])
- `create_mode(nom, descrition)` : Crée un nouveau mode d'exécution
- `update_mode(old_nom, new_nom, descrition)` : Modifie un mode existant
- `delete_mode(nom)` : Supprime un mode d'exécution
- `select_mode(nom)` : Change le mode actif (redémarre l'app)

**Paramètres** :
- `get_settings()` : Retourne les paramètres fusionnés (défaut + surcharges, secrets masqués)
- `get_settings_meta()` : Retourne l'organisation du drawer (table `[_meta]` : vues, groupes système, actions, handlers, libellés/icônes)
- `update_setting(path, value)` : Met à jour un paramètre (chiffre les secrets)
- `reset_setting(path)` : Rétablit un paramètre à sa valeur par défaut
- `get_setting_value(path)` : Retourne la valeur effective d'un paramètre (déchiffrée pour les secrets)

**Traces GPX** :
- `import_gpx_file()` : Importe un fichier GPX via le sélecteur natif (retourne TraceMetadata)
- `get_traces()` : Retourne la liste des traces importées pour le mode actif
- `delete_trace(traceId)` : Supprime une trace (fichier GPX + entrée du registre)
- `update_trace(traceId, favorite?, isDisplayed?)` : Met à jour partiellement une trace (PATCH)
- `get_trace_geometry(traceId)` : Retourne la géométrie GeoJSON d'une trace (cache, ou régénéré depuis le GPX)
- `get_trace_points(traceId)` : Retourne les points d'une trace avec altitude et distance cumulée 3D (re-parse le GPX)
- `save_keyframes(traceId, keyframesJson)` : Sauvegarde un jeu de keyframes dans `keyframes/{trace_id}.json` (écriture atomique)
- `get_keyframes(traceId)` : Charge les keyframes persistés d'une trace (`null` si absent)
- `delete_keyframes(traceId)` : Supprime le fichier keyframes d'une trace (tolérant si absent)

**Nettoyage de trace** (pipeline de 3 étapes ; chaque commande prend une `phase` `"spike"` / `"roundabout"` / `"out_and_back"`) :
- `detect_trace_anomalies(traceId, phase, toleranceDeg, roundaboutParams)` : Détecte les anomalies de la phase sur le GPX courant (étape 1 : rebroussements ~180° ; étape 2 : ronds-points — cumul d'angle), sans persistance
- `get_cleaning_state(traceId, phase, toleranceDeg, roundaboutParams)` : Retourne le travail de la phase (`traces/{trace_id}/cleaning.{phase}.json`), sinon une détection fraîche
- `save_cleaning_state(traceId, phase, stateJson)` : Sauvegarde partielle des corrections (écriture atomique), passe en `in_progress`, mémorise la phase
- `reset_cleaning(traceId)` : Abandonne les corrections (toutes phases) et repasse à l'étape 1, `needs_review`
- `validate_phase(traceId, phase, stateJson)` : Applique les corrections validées de la phase, réécrit le GPX (backup `{filename}.gpx.orig` une seule fois), régénère geojson/stats/hash, avance `cleaning_phase` (la trace reste `needs_review` tant que l'étape 3 n'existe pas ; refuse les cas `pending` et l'étape 3)

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

## Application VisuGPS2

### Pages
- **Accueil** (`/`) : Fenêtre principale — drawer gauche (liste circuits triée par distance au centre de la carte ; **icône Éditer colorée selon l'avancement de l'édition pour le viewport paramétré** — forcée visible quand l'édition est incomplète, masquée uniquement si complète (verte) et non survolée, visible au survol quelle que soit la couleur), carte Mapbox (clusters de points de départ, popups ; **centrage/zoom mémorisés** dans les paramètres cachés `Carte.Vue.*` et restitués au lancement / au retour sur la vue), drawer droit (paramètres)
- **ScreenBis** (`/screen-bis`) : Fenêtre secondaire pour le dual-screen
- **Visualisation** (`/visualisation`) : Stub (toolbar Home uniquement, vue réservée à la visualisation 3D)
- **Nettoyage** (`/nettoyage`) : Nettoyage de trace GPX en **3 étapes** avec **widget « boîte à états »** dans la toolbar (1 Pts hors trace → 2 Rond-Points → 3 Aller/Retour, ✓/✗, navigation séquentielle, **étape courante du pipeline surlignée**) — carte Mapbox (trace complète en vert, segment courant surligné, linestring corrigé jaune, branches aller/retour décalées et colorées, points numérotés cliquables avec anti-revouvrement, sélecteur des points proches du curseur, points supprimés en rouge, déplacement direct à la souris des points des cas manuels), panneau des anomalies (validation manuelle « Valider » / « Faux positif », **poubelle rouge** sauf pour les cas « Aller-retour », bouton **« Modifier un segment »**, affichage **angle cumulé / tours / sens** pour les ronds-points), table simplifiée des points du segment (numéro, suppression avec case d'en-tête tout supprimer / tout remettre, indicateur « Déplacé », **zone élargie d'une marge pour les ronds-points**), boutons **Enregistrer** (sauvegarde partielle) et **Valider l'étape** (applique la phase, réécrit le GPX, passe à l'étape suivante — **jamais actif sur une étape déjà validée**). L'étape 3 « Aller/Retour » est **non implémentée** (simple emplacement, cliquable pour revenir) : la trace reste `needs_review`. Accessible depuis le bouton Éditer d'un circuit « À nettoyer » (icône `mdi-broom`) et via le garde-fou d'`EditionCamera`. Une trace non « clean » ne peut pas entrer en édition caméra.
- **EditionCamera** (`/edition-camera`) : Vue d'édition caméra — carte Mapbox satellite + terrain, trace sélectionnée, curseur (CircleLayer WebGL synchronisé terrain, couleur paramétrable), **au lancement la vue reste vierge (carte masquée, spinner centré) jusqu'au chargement complet des tuiles, puis est révélée directement au km 0** (sans vue globale intermédiaire), lecture (keyframes générés par l'algorithme **frustum** (spec §3.3, placement récursif par visibilité + occlusion relief via grille `terrain-rgb` fine + réorientation oblique du cap) ou **simple** (échantillonnage régulier) — **algorithme et gap min gérés dans le panneau Paramètres** (drawer) — cap **lissé** entre les RdV (`lerpAngle`), boucle rAF), **deux fichiers keyframes par trace** (16:9 → `{trace_id}_169.json`, 4:3 → `{trace_id}_43.json`) sélectionnés via le **bouton ViewPort** de la toolbar (flip-flop 16:9 / 4:3 — icône monitor / monitor-small, couleur = avancement du verrouillage : cadre + fichier du ratio choisi, l'autre garanti en arrière-plan), **panneau Paramètres** (bouton `mdi-cog-outline` en **flip-flop** → `SettingsDrawer` limité aux **catégories de la vue** — sections système masquées via `show-system=false` : Génération (algorithme, gap min), Caméra (zoom/pitch/viewport par défaut, durée du déplacement caméra `dureeFlyTo` 100–1000 ms, affichage HUD télémétrie et panneau caps), Lecture (accélération `acceleration` ×1.5–8) et Couleurs (couleur/épaisseur de la trace, curseur) — appliqués en direct au store), contrôle de lecture compact (colonne boutons à droite du graphe : RdV précédent/suivant verts, km0 / Play-Pause / **lecture accélérée** (double chevron, ×`acceleration`) / dernier point ; vitesse 1× ou facteur accéléré paramétrable), **graphe SVG d'avancement** (timeline proportionnelle 3px/100m alignée à droite sur traces courtes, 3 zones RdV/avancement/graduation — **ticks RdV bicolores** : zone supérieure zoom (rouge si ≠ défaut), zone inférieure pitch (orange si ≠ défaut), vert par défaut — curseur étendu dont la couleur suit le paramètre `Edition.Couleurs.curseur`, clic seek + sélection, tooltip survol altitude, auto-scroll), **changements de cap brutaux** (bande colorée entre deux ticks RdV sur la timeline — couleur = sens (teal horaire / deep-purple anti-horaire), **épaisseur** = intensité du taux (2 px sous le seuil, +2 px par bande de 30 °/km), seuil réglable dans le tableau — + **tableau à la demande** cliquable — Départ/Arrivée en jaune sur le segment courant, sens en Δ dist/Δ cap, Taux coloré jaune→rouge par bande), **verrous de segments** (mode **validation** via bouton `mdi-camera-lock` : **clic carte ou touche Entrée** = problème, segments sans clic verrouillés pendant la lecture — trait rouge en haut de la timeline pour les segments non verrouillés, **double-clic** sur un segment pour basculer le verrou, keyframes bordant un verrou non modifiables ; **en mode validation**, le **cadre ViewPort** et le **fond de l'indicateur de distance** passent au **bleu**), **Composant B — édition des keyframes** (CameraEditor : switch Cible, sliders Pitch/Zoom customs avec valeurs par défaut issues des paramètres (`pitchDefaut`/`zoomDefaut`), CompassBandeau défilement infini, Undo/Supprimer/Sauvegarder, km0 non supprimable), **raccourcis clavier** (Espace Play/Pause, flèches ←/→ navigation RdV), HUD télémétrie (caméra + altitude traceur + relation caméra↔curseur, **masqué par défaut** — paramètre `afficherTelemetrie`, **bouton Fermer** valable pour la session, **lignes Zoom/Pitch colorées** — vert au défaut, orange/rouge sinon), **HUD distance** (barre bas, distance parcourue orange / totale), cadre ViewPort au ratio sélectionné (overlay CSS). Déclenchée par le bouton Éditer d'un circuit.

### Fonctionnalités
- Import/suppression/mise à jour de traces GPX (favoris et affichage persistés)
- **Nettoyage de trace GPX** en **3 étapes séquentielles** (Pts hors trace → Rond-Points → Aller/Retour), avec **widget « boîte à états »** dans la toolbar (✓/✗ par étape, navigation séquentielle) : étape 1 = **détection combinée d'origine** (points isolés hors trace **+** aller-retours, rebroussement ~180°, tolérance `Nettoyage.Cap.toleranceDeg`) — **modifie le GPX** ; étape 2 = **ronds-points** (tours soutenus — cumul d'angle, paramètres `Nettoyage.RondPoints.*`, correction manuelle dans une zone élargie d'une marge) — **modifie le GPX** ; étape 3 = aller/retour (**non implémentée**, simple emplacement — **ne modifiera pas le GPX**, produira un autre type de fichier). À l'import, la trace est signalée « à nettoyer » dès qu'une anomalie existe (toutes étapes). Validation d'étape = réécriture du GPX (entrée de l'étape suivante), sauvegardes partielles (`traces/{trace_id}/cleaning.{phase}.json`), backup `{filename}.gpx.orig` une seule fois. La trace **reste `needs_review`** tant que l'étape 3 n'existe pas. Une trace non « clean » ne peut pas entrer en édition caméra.
- Carte Mapbox GL avec **clustering** des points de départ (token via paramètre `Systeme.Key.mapBox`)
- Affichage des traces favorites et forcées (dégradé bleu → rouge, couches Mapbox dédiées)
- **Synchronisation carte ↔ liste** : la liste des circuits est triée par distance au centre courant de la carte (Haversine), recalculée en temps réel sur `moveend` (debounce 150 ms)
- **Focus carte** : clic sur Info d'un circuit isole et cadre la trace (`fitBounds`), puis revient à la vue précédente (`flyTo`) — durée paramétrable (`Carte.Traces.dureeFlyTo`)
- Clic sur un cluster → zoom d'expansion ; clic sur un point → popup (nom, source, coordonnées)
- Toggle thème dark/light (synchronisé entre fenêtres)
- Gestion des modes d'exécution (OPE / EVAL_*)
- Système de paramètres TOML avec chiffrement des secrets
- Détection multi-écrans et placement automatique

### Store exemple
`src/stores/app.ts` gère le thème et l'environnement d'exécution :
- `isDarkMode` : état du thème
- `theme` : computed qui retourne 'dark' ou 'light'
- `toggleDarkMode()` : action pour basculer
- `displays` : liste des écrans détectés
- `isDev`, `activeMode`, `modes` : environnement et modes d'exécution

Autres stores existants :
- `src/stores/settings.ts` : paramètres de configuration (pattern Setup Store)
- `src/stores/traces.ts` : traces GPX importées (pattern Setup Store). Expose `traces`, `loading`, `mapCenter`, `focusedTraceId` (trace « focus » temporaire, clic Info), `visibleTraceIds` (IDs visibles dans le viewport), `traceCount`, `sortedTracesByDistance` (tri Haversine par rapport au centre de la carte), `visibleTracesByDistance` (filtrage viewport, tri par distance), et les actions `loadTraces`, `importerGpx`, `supprimerTrace`, `updateTrace`, `getTraceGeometry`, `getTracePoints` (points avec altitude + distance cumulée 3D), `updateMapCenter`, `setVisibleTraceIds`.
- `src/stores/keyframes.ts` : persistance des keyframes sur disque (pattern Setup Store). Actions `loadKeyframes(traceId)` (charge depuis `keyframes/{trace_id}.json`, retourne `null` si absent/invalide), `saveKeyframes(set)` (sauvegarde via écriture atomique), `clearKeyframes(traceId)` (supprime le fichier, tolérant si absent).
- `src/stores/edition.ts` : état de la vue d'édition caméra (pattern Setup Store). Côté sélection : `selectedTraceId`, `showViewportFrame` (affiché par défaut), **`viewportAspect`** (`'16:9'` défaut / `'4:3'`, + `setViewportAspect` qui recharge le fichier keyframes du ratio) + actions `selectTrace`/`clearSelection`/`toggleViewportFrame`. Côté algorithme : `keyframeAlgorithm` (`'frustum'` défaut / `'simple'`), `minKeyframeGapM` (200–5000, pas 50) + `setKeyframeAlgorithm`/`setMinKeyframeGapM` (forcent la régénération des deux fichiers). Côté lecture : `keyframeSet`, `isPlaying`, `speed`, `currentTimeMs` ; getters `interpolatedCam`/`interpolatedTraceur` (interpolation caméra/curseur avec altitude, bearing lissé par `lerpAngle`), `currentDistanceKm`, `totalDistanceKm`, `markerDistanceM`, `markerRelativeBearing`, `altitudeAtDistance(m)` (closure capturant la polyligne — altitude interpolée à une distance donnée, utilisée par le tooltip du ProgressGraph), `currentKeyframe` (keyframe à la position courante, pilote le CameraEditor), `canGoNextRdv`/`canGoPrevRdv` ; **changements de cap** : `headingChanges`/`brutalHeadingChanges` (analyse des virages entre keyframes, module pur `algorithms/headingChanges.ts`), `headingChangeThresholdDegPerKm` (45°/km, 10–1000, pas 5, + `setHeadingChangeThreshold` — filtre d'affichage), `showHeadingChangesPanel` (+ toggle) ; **verrous de segments** : `validationMode` (+ toggle via `mdi-camera-lock`), `lockedSegmentFromDistances`, `currentSegmentFromDistance`, `markValidationClick`, `toggleSegmentLock`/`lockSegment`/`unlockSegment` (persistés via les flags `locked`/`marks` de chaque keyframe — **traits bleus persistés** dans le fichier keyframes, protection par parcours), gardes sur `updateKeyframe`/`removeKeyframe`/`addKeyframe` pour les keyframes verrouillés ; **édition keyframes (Composant B)** : `updateKeyframe(distance, updates)` (mute cam sans sauvegarde auto), `addKeyframe` (insertion + interpolation voisins), `removeKeyframe` (**km 0 intouchable**, min 2 conservés), `saveKeyframes()` (sauvegarde explicite), `selectKeyframe` ; lecture : `setKeyframeSet(set, feature, tracePoints?)` (construit la polyligne enrichie altitude depuis les points backend), `play`/`pause`/`togglePlay`, `setSpeed`, `seekToDistance`, `tick(deltaMs)`, `goToNextRdv`/`goToPrevRdv` (pause auto + seek + sélection). État UI éphémère non persisté.
- `src/stores/cleaning.ts` : état de la vue de nettoyage de trace (pattern Setup Store). Expose `selectedTraceId` (posé avant la navigation vers `/nettoyage`), `state` (détection + décisions, persisté dans `traces/{trace_id}/cleaning.{phase}.json`), `points` (index GPX), `currentCaseIndex`, **`currentPhase`** (étape en cours, reflet de `cleaning_phase`), `toleranceDeg` (paramètre `Nettoyage.Cap.toleranceDeg`), **`roundaboutParams`** (paramètres `Nettoyage.RondPoints.*`), l'état UI éphémère `createMode`/`createStartIndex`/`movePointIndex` (modification de segment et déplacement), les getters `hasCases`, `currentCase`, `currentZone`, **`zoneStart`/`zoneEnd`** (zone du cas, **élargie de la marge pour les ronds-points**), `correctedZoneCoords` (coordonnées `[lon, lat]` du segment courant **après corrections**), `allValidated` (tous les cas ≠ `pending`), `validatedCount`, `isDeletedCount`, `isZoneFullyDeleted`, et les actions `load` (détection de la phase courante depuis `cleaning_phase` + **auto-validation des étapes vides**), **`goToPhase`** (navigation du widget, étape 3 exclue), **`phaseValidated`**, `reDetect`, `goToCase`, `toggleDeletePoint`/`addDeleteRange`/`clearCorrection`/`setZoneDeleted` (suppressions — la suggestion de détection est appliquée automatiquement à la validation ; `clearCorrection` remet aussi le cas à « À traiter »), `setMovedPoint`/`clearMovedPoint`/`startMovePoint`/`stopMovePoint` (déplacement géographique des points), `toggleCreateMode`/`cancelCreate`/`createManualCase`/`removeCase` (cas « Modification de segment », **supprimables même après validation**), `validateCurrentCase('corrected' | 'kept')` (validation manuelle « Valider » / « Faux positif »), `save` (sauvegarde partielle, passe la trace en `in_progress`), **`validatePhase`** (ex-`finalize` : applique la phase, réécrit le GPX, avance la phase, recharge), `reset` (abandon → étape 1, `needs_review`).
- `src/stores/ui.ts` : notifications snackbar mutualisées (pattern Setup Store)

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

1. Créer un module dans `src-tauri/src/` (ou ajouter la commande dans un module existant)
2. Définir `#[tauri::command]`
3. Enregistrer le module dans `src-tauri/src/lib.rs` (`mod mon_module;`)
4. Ajouter la commande dans `.invoke_handler()` de `lib.rs`
5. Appeler avec `invoke()` côté frontend
6. Si la commande utilise un plugin, l'ajouter via `.plugin(...)` dans `lib.rs` et la permission dans `capabilities/default.json`

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

**gestionMode.rs - Gestion des modes d'exécution**
```rust
pub struct ModeInfo { nom, descrition, création, révision }
pub fn read_active_mode(app_data_dir: &Path, is_dev: bool) -> String { ... }
#[tauri::command]
pub async fn get_execution_env(app: AppHandle) -> Result<ExecutionEnv, String> { ... }
#[tauri::command]
pub async fn get_modes(app: AppHandle) -> Result<Vec<ModeInfo>, String> { ... }
// + create_mode, update_mode, delete_mode, select_mode
```

**settings.rs - Paramètres de configuration**
```rust
pub struct SettingDefinition { path, description, documentation, type, default, value, min, max, step, critical, unit, choices, icon, is_overridden }
// icon: Option<String> — icône MDI optionnelle pour le drawer (défaut = icône par type)
pub struct SettingsMeta { system, views, groups }   // organisation du drawer (table [_meta])
pub fn init_settings_state(app_handle: &AppHandle) -> Result<Arc<RwLock<SettingsState>>, String> { ... }
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<Vec<SettingDefinition>, String> { ... }
// + get_settings_meta, update_setting, reset_setting, get_setting_value
```

**import_gpx.rs - Import de traces GPX**
```rust
pub struct Point3D { lat, lon, alt }
pub struct TraceStats { start_point, end_point, distance_m, positive_elevation_m, … }
pub struct TraceMetadata {
    id, name, source, source_url, activity_type, filename, import_date, stats, hash,
    favorite,        // favori (persisté, #[serde(default)])
    is_displayed,    // affichage carte (persisté, #[serde(default)])
    cleaning_status, // "clean" | "needs_review" | "in_progress" (#[serde(default)] → "clean")
    cleaning_phase,  // "spike" | "roundabout" | "out_and_back" | "" (propre) (#[serde(default)])
}
#[tauri::command]
pub async fn import_gpx_file(app: AppHandle) -> Result<TraceMetadata, String> { ... }
#[tauri::command]
pub async fn get_traces(app: AppHandle) -> Result<Vec<TraceMetadata>, String> { ... }
#[tauri::command]
pub async fn delete_trace(app: AppHandle, trace_id: String) -> Result<(), String> { ... }
#[tauri::command]
pub async fn update_trace(
    app: AppHandle, trace_id: String,
    favorite: Option<bool>, is_displayed: Option<bool>,
) -> Result<(), String> { ... }
```

**lib.rs - Orchestration principale**
```rust
mod display;
mod gestionMode;
mod settings;
mod import_gpx;
mod cleaning;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| { /* init settings + display */ })
        .invoke_handler(tauri::generate_handler![/* toutes les commandes */])
        .run(tauri::generate_context!())
}
```

**Dépendances Rust** :
- `objc2` + `objc2-app-kit` + `objc2-foundation` : Accès NSScreen sur macOS
- `windows` : Win32 API (GetForegroundWindow, GetMonitorInfo)
- `tauri` : APIs principales (Manager, invoke_handler, generate_handler)
- `tauri-plugin-dialog` : Sélecteur de fichiers natif (Tauri 2.x)
- `tauri-plugin-opener` : Ouverture de liens dans le navigateur
- `serde` + `serde_json` : Sérialisation
- `toml` : Lecture/écriture des fichiers de configuration
- `chrono` : Dates et heures
- `gpx` : Parsing des fichiers GPX
- `geo` : Calculs géodésiques (Haversine)
- `sha2` + `hex` : Empreintes SHA256 (anti-doublon)
- `uuid` : Identifiants uniques
- `keyring` + `aes-gcm` : Chiffrement des secrets
- `dotenvy` : Variables d'environnement

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

**Dernière mise à jour** : 2026-08-19
**Version du projet** : 0.0.1
**Status** : En développement actif
**Fonctionnalités** : Dual-screen, inter-window communication, theme sync, display detection, multi-env execution modes, settings management (TOML + secrets chiffrés), import/suppression/mise à jour de traces GPX (favoris/affichage persistés), carte Mapbox (clustering points de départ, traces favorites/affichées dégradé, focus carte sur clic Info, synchronisation liste triée par distance), vue d'édition caméra (carte satellite + terrain, curseur jaune CircleLayer WebGL, lecture par keyframes + boucle rAF, contrôle compact Play/Pause + navigation RdV/km0/dernier, graphe SVG d'avancement timeline proportionnelle curseur orange alignée à droite sur traces courtes, **édition des keyframes (Composant B) : switch Cible, sliders Pitch/Zoom, compas, Undo/Supprimer/Sauvegarder**, raccourcis clavier Espace/flèches, HUD télémétrie caméra + altitude traceur, HUD distance parcourue/total, cadre ViewPort 16:9)
