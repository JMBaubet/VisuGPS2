# Documentation Technique — Intégration GPX Climb Analyzer

**Cible** : application Tauri + Node.js + Vue 3 + Vuetify, ou intégration standalone web
**Version** : 3.9.2
**Date** : 2026-10-17
**Public** : développeur full-stack intégrant la fonctionnalité

---

## Table des matières

1. [Contexte d'intégration](#1-contexte-dintégration)
2. [Architecture recommandée](#2-architecture-recommandée)
3. [Modèle de données](#3-modèle-de-données)
4. [Inventaire complet des paramètres](#4-inventaire-complet-des-paramètres)
5. [Paramètres de catégorisation de pente](#5-paramètres-de-catégorisation-de-pente)
6. [Modules TypeScript à porter](#6-modules-typescript-à-porter)
7. [Composables Vue 3](#7-composables-vue-3)
8. [Composants Vuetify](#8-composants-vuetify)
9. [Communication avec le backend Node.js](#9-communication-avec-le-backend-nodejs)
10. [Intégration Tauri](#10-intégration-tauri)
11. [Intégration Mapbox GL JS](#11-intégration-mapbox-gl-js)
12. [Widget profil (canvas)](#12-widget-profil-canvas)
13. [Drag & drop sur le profil](#13-drag--drop-sur-le-profil)
14. [Groupes de montées](#14-groupes-de-montées)
15. [Création manuelle](#15-création-manuelle)
16. [Suppression de montées](#16-suppression-de-montées)
17. [Pipeline complet de détection](#17-pipeline-complet-de-détection)
18. [Diagnostic de configuration](#18-diagnostic-de-configuration)
19. [Export JSON](#19-export-json)
20. [Tests et validation](#20-tests-et-validation)
21. [Migration depuis le prototype HTML](#21-migration-depuis-le-prototype-html)
22. [Annexes](#22-annexes)

---

## 1. Contexte d'intégration

### 1.1 Objectif

Intégrer dans une application Tauri/Vue 3/Vuetify (ou standalone web) la fonctionnalité complète d'analyse de profil altimétrique GPX, dans sa version **v3.9.2**, incluant :

- **Filtre anti-pic Hampel** avec fenêtre en **mètres** (v3.6)
- **Rééchantillonnage par moyenne** (fenêtre minimale 75 m, v3.5)
- Correction des plateaux GPS/MNT
- Lissage multi-étages (médiane + SG + gaussien + ancrage + double SG)
- Détection avec garde-fou hybride de refinage
- **Fusion automatique** des montées trop proches (`gapMinM`) avec **conservation des `mergedParts`** (v3.9.0)
- **Validation finale stricte** avec repli sur sous-segment (`fallback`)
- **Carte Mapbox 3D** avec terrain DEM, courbes de niveau optionnelles, et **trace colorée par catégorie** (v3.8)
- **Trace sur la carte = points bruts du GPX** (v3.9.2), pas la grille resamplée
- **Widget profil en canvas 2D natif** avec échelles fixes (50 m/px full, 10 m/px montée) (v3.8.3)
- **Catégorisation non ancrée** pour éviter les changements de couleur pendant le drag (v3.8.1)
- **Widget montée avec profil complet + scroll centré** sur la montée sélectionnée (v3.9.2)
- **Séparation des montées fusionnées** en leurs sous-montées d'origine (`✂`), avec persistance du groupe via `groupId`/`subIndex` (v3.9.0/v3.9.1)
- **Refusion d'un groupe** (`🔗`), en préservant les bornes éditées (v3.9.1)
- **Suppression** de montées individuelles avec renumérotation (v3.9.2)
- **Création manuelle** par double-clic + drag (v3.9.0)
- **Multi-marqueurs sur la carte** pour les montées groupées (v3.9.2)
- **Chargement automatique de la carte** si token présent (v3.9.2)
- **Diagnostic de configuration visuel**
- **Export JSON** avec `groupId`/`subIndex`

### 1.2 Contraintes de la cible

| Contrainte | Impact |
|---|---|
| Application desktop Tauri OU web | Accès fichier natif en Tauri, drag & drop en web |
| Backend Node.js embarqué (optionnel) | Possibilité de déporter les calculs lourds |
| Frontend Vue 3 + Vuetify | Structure en composants réutilisables |
| TypeScript | Typage strict recommandé |
| Pinia | Store centralisé pour l'état |
| Mapbox GL JS v3.7+ | Nécessite un token `pk.*` |

### 1.3 Répartition recommandée front/back

| Tâche | Où l'exécuter | Justification |
|---|---|---|
| Parsing GPX | **Node.js** (Tauri) ou **front** (web) | fs natif ou FileReader |
| Rééchantillonnage par moyenne | **Node.js** | O(n + m) mais centralise le pipeline |
| Correction plateaux | **Node.js** | Prétraitement |
| **Anti-pic Hampel** | **Node.js** | O(n × w log w), optionnellement front si n < 5000 |
| Détection complète | **Node.js** | Coûteux (O(n × w + m × 800²)) |
| Rebuild après édition | **Front** si < 500 ms | Compromis latence |
| Rendu carte Mapbox | **Front** | Interactif, Mapbox GL JS |
| **Rendu profil canvas** | **Front** | Contrôle total, perfs |
| Drag & drop des poignées | **Front** | Latence minimale |
| **Séparation/refusion/suppression** | **Front** | Mutation locale + rebuild |
| **Création manuelle** | **Front** | Interaction temps réel |
| Diagnostic | **Front** | O(1), à chaque input |
| Export JSON | **Front** | Blob natif |

---

## 2. Architecture recommandée

### 2.1 Vue d'ensemble

```
┌──────────────────────────────────────────────────────────────────────────┐
│                       Application Tauri / Web                            │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                    Frontend (WebView / Navigateur)                  │  │
│  │  ┌──────────────────────────────────────────────────────────────┐  │  │
│  │  │  Vue 3 + Vuetify                                             │  │  │
│  │  │  ┌────────────────────────────────────────────────────────┐  │  │  │
│  │  │  │  <GpxAnalyzer />                                       │  │  │  │
│  │  │  │   ├── <AppToolbar />           (drop, debug, params)    │  │  │  │
│  │  │  │   ├── <ClimbsPanel />          (colonne gauche)         │  │  │  │
│  │  │  │   │    ├── <StatsMini />                               │  │  │  │
│  │  │  │   │    ├── <ClimbCard />       (par montée, groupée ?) │  │  │  │
│  │  │  │   │    └── <GroupBrace />                              │  │  │  │
│  │  │  │   ├── <CenterPanel />          (carte + profil)        │  │  │  │
│  │  │  │   │    ├── <MapPanel />        (Mapbox GL JS)          │  │  │  │
│  │  │  │   │    └── <ProfilePanel />    (canvas 2D)             │  │  │  │
│  │  │  │   └── <ParamsPanel />          (colonne droite)        │  │  │  │
│  │  │  │        ├── <MapboxTokenField />                        │  │  │  │
│  │  │  │        ├── <DiagnosticPanel />                         │  │  │  │
│  │  │  │        └── <ParametersForm />                          │  │  │  │
│  │  │  └────────────────────────────────────────────────────────┘  │  │  │
│  │  │                                                              │  │  │
│  │  │  Stores Pinia                                                │  │  │
│  │  │   ├── useGpxStore          (points bruts + resamplés)         │  │  │
│  │  │   ├── useClimbsStore       (montées + groupes)                │  │  │
│  │  │   ├── useParametersStore   (15 params + 2 toggles)            │  │  │
│  │  │   └── useMapStore          (token, basemap, ready)            │  │  │
│  │  │                                                              │  │  │
│  │  │  Composables                                                  │  │  │
│  │  │   ├── useGpxParser                                            │  │  │
│  │  │   ├── useSlopeCalculations                                    │  │  │
│  │  │   ├── useClimbDetection                                       │  │  │
│  │  │   ├── useClimbGroups       (split/refuse/normalize)           │  │  │
│  │  │   ├── useManualClimb       (création double-clic + drag)      │  │  │
│  │  │   ├── useMarkerDrag         (drag poignées canvas)            │  │  │
│  │  │   ├── useMapbox             (init, layers, trace, markers)    │  │  │
│  │  │   ├── useProfileCanvas      (rendu canvas, scroll centré)     │  │  │
│  │  │   └── useDiagnostics                                        │  │  │
│  │  └──────────────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                            │ Tauri IPC (invoke)                          │
│                            ▼                                             │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │              Backend (Rust + Node.js sidecar)                      │  │
│  │  ┌──────────────────────────────────────────────────────────────┐  │  │
│  │  │  Node.js sidecar                                             │  │  │
│  │  │   ├── /parse-and-analyze    (pipeline v3.9.2)                │  │  │
│  │  │   ├── /rebuild              (optionnel)                      │  │  │
│  │  │   └── /despike              (isolé si besoin)                │  │  │
│  │  └──────────────────────────────────────────────────────────────┘  │  │
│  │  Rust core                                                         │  │
│  │   ├── fs (lecture fichiers GPX)                                    │  │
│  │   └── shell (spawn Node.js sidecar)                                │  │
│  └────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Arborescence suggérée

```
src-tauri/
├── src/
│   ├── main.rs
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── gpx.rs                # read_gpx_file
│   │   └── analyzer.rs           # analyze_gpx
│   └── sidecar/
│       └── analyzer.js           # Script Node.js pipeline v3.9.2
└── Cargo.toml

src/
├── main.ts
├── App.vue
├── types/
│   ├── gpx.ts
│   ├── climb.ts                  # UserClimb, AutoClimb, Climb
│   ├── category.ts
│   ├── parameters.ts
│   ├── diagnostics.ts
│   └── mapbox.ts                 # MAP_STATE, basemap
├── services/
│   ├── gpxApi.ts                 # Wrapper invoke Tauri
│   └── mapboxLoader.ts           # Chargement paresseux de Mapbox GL JS
├── stores/
│   ├── gpxStore.ts
│   ├── climbsStore.ts
│   ├── parametersStore.ts
│   └── mapStore.ts
├── composables/
│   ├── useGpxParser.ts
│   ├── useSlopeCalculations.ts
│   ├── useClimbDetection.ts
│   ├── useClimbGroups.ts         # split, refuse, normalize
│   ├── useManualClimb.ts         # double-clic + drag
│   ├── useMarkerDrag.ts          # drag poignées
│   ├── useMapbox.ts              # init, layers, trace, markers, zoom
│   ├── useProfileCanvas.ts       # rendu canvas, scroll centré
│   └── useDiagnostics.ts
├── utils/
│   ├── haversine.ts
│   ├── resample.ts               # moyenne
│   ├── plateau.ts
│   ├── despike.ts                # fenêtre en mètres
│   ├── rawStats.ts
│   ├── smoothing.ts              # MA, SG, gaussien, médiane
│   ├── anchoredSmoothing.ts
│   ├── slope.ts
│   ├── categorization.ts
│   ├── segmentation.ts
│   ├── refineGuard.ts
│   ├── mergeClimbs.ts            # avec mergedParts
│   ├── validateClimbs.ts         # avec préservation mergedParts
│   ├── climbConstraints.ts
│   ├── insertion.ts
│   ├── manualClimb.ts            # création
│   ├── deleteClimb.ts            # suppression
│   ├── buildTraceFeatures.ts     # carte : points bruts
│   ├── buildMapMarkers.ts        # mono/groupe
│   ├── zoomHelpers.ts            # fitBounds, _computeBounds
│   └── groupHelpers.ts           # groupColor, normalizeGroups
├── components/
│   └── gpx-analyzer/
│       ├── GpxAnalyzer.vue
│       ├── AppToolbar.vue
│       ├── ClimbsPanel.vue
│       ├── ClimbCard.vue
│       ├── GroupBrace.vue
│       ├── CenterPanel.vue
│       ├── MapPanel.vue
│       ├── ProfilePanel.vue
│       ├── ParamsPanel.vue
│       ├── DiagnosticPanel.vue
│       └── ParametersForm.vue
└── plugins/
    └── vuetify.ts
```

---

## 3. Modèle de données

### 3.1 Types TypeScript — GPX et paramètres

```typescript
// ============================================================
// src/types/gpx.ts
// ============================================================

export interface GpxPoint {
  lat: number;
  lon: number;
  ele: number;
}

export interface GpxTrack {
  name: string;
  fileName?: string;
  points: GpxPoint[];
  creator?: string;
}

// ============================================================
// src/types/parameters.ts
// ============================================================

export interface AnalyzerParameters {
  // --- Rééchantillonnage ---
  /** Pas de rééchantillonnage (m) — 2 à 200, défaut 50 */
  stepM: number;

  // --- Anti-pic Hampel (v3.6) ---
  /** Anti-pic : fenêtre (m) — 50 à 500, défaut 200 */
  despikeWindowM: number;
  /** Anti-pic : seuil (m) — 0 à 100, défaut 15 */
  despikeThresholdM: number;

  // --- Lissages en mètres ---
  /** Lissage altitude (m) — défaut 500 */
  fenetreAffichageM: number;
  /** Lissage pente (m) — défaut 150 */
  fenetrePenteM: number;
  /** Fenêtre de calcul pente (m) — défaut 100 */
  fenetreCatM: number;

  // --- Détection ---
  /** Seuil détection (%) — défaut 0.5 */
  seuilMontee: number;
  /** Longueur min. pente (m) — défaut 200 */
  longueurMin: number;

  // --- Fusion des montées ---
  /** Écart min. entre montées (m) — 0 à 5000, défaut 250 */
  gapMinM: number;

  // --- Garde-fou refinage ---
  /** Recherche extrema (%) — 1 à 500, défaut 60 */
  refinePct: number;

  // --- Filtres ---
  /** Filtre médian (m) — 0 à 300, défaut 0 */
  medianWindowM: number;
  /** Lissage gaussien (m) — 0 à 500, défaut 75 */
  gaussianSigmaM: number;
  /** Correction plateaux (pts) — 0 à 20, défaut 6 */
  plateauRun: number;

  // --- Debug ---
  /** Segments debug (m) — défaut 100 */
  segmentLogM: number;

  // --- Export ---
  /** Pas du profil exporté (m) — 10 à 500, défaut 50 */
  exportProfileStepM: number;

  // --- Toggles ---
  /** Lissage renforcé (double SG) — défaut true */
  smoothExtra: boolean;
  /** Lissage des pentes de segment — défaut true */
  segmentSmooth: boolean;
}
```

### 3.2 Types TypeScript — Montées (avec groupes)

```typescript
// ============================================================
// src/types/climb.ts
// ============================================================

/** Bornes d'une sous-montée conservées lors d'une fusion. */
export interface MergedPart {
  startKm: number;
  endKm: number;
}

/** Montée détectée automatiquement (sortie de computeAuto). */
export interface AutoClimb {
  startKm: number;
  endKm: number;
  refineM: number | null;
  candidateLen: number | null;
  /** Nombre de fusions ayant contribué à cette montée (0 si autonome). */
  mergedCount: number;
  /** Sous-montées d'origine conservées pour permettre la séparation. */
  mergedParts: MergedPart[] | null;
  /** Vrai si un repli ⤺ a eu lieu lors de la validation. */
  fallback: boolean;
}

/** Montée éditable par l'utilisateur. */
export interface UserClimb {
  startKm: number;
  endKm: number;
  autoStartKm: number | null;
  autoEndKm: number | null;
  refineM: number | null;
  candidateLen: number | null;
  mergedCount: number;
  mergedParts: MergedPart[] | null;
  fallback: boolean;
  /** Identifiant entier du groupe (null si autonome). */
  groupId: number | null;
  /** Position dans le groupe (1, 2, 3…). */
  subIndex: number | null;
  /** Appliqué par rebuild() lors des contraintes anti-chevauchement. */
  _constrainedStart?: boolean;
  _constrainedEnd?: boolean;
}

/** Montée calculée à partir de UserClimb via rebuild(). */
export interface Climb {
  startIdx: number;
  endIdx: number;
  ref: UserClimb;
  invalid: boolean;
  constrainedStart: boolean;
  constrainedEnd: boolean;
  startDist: number;
  endDist: number;
  length: number;
  dPlus: number;
  avgSlope: number;
  altStart: number;
  altEnd: number;
  refineM: number | null;
  candidateLen: number | null;
  mergedCount: number;
  fallback: boolean;
  groupId: number | null;
  subIndex: number | null;
}

// ============================================================
// src/types/category.ts
// ============================================================

export interface SlopeCategory {
  id: string;
  label: string;
  color: string;
  min: number;
  max: number;
}

/** Segment de catégorie contigu, partagé entre carte et profil. */
export interface CatSegment {
  startIdx: number;
  endIdx: number;
  startDist: number;
  endDist: number;
  length: number;
  dPlus: number;
  avgSlope: number;
  altStart: number;
  altEnd: number;
  key: string;
}
```

### 3.3 Types TypeScript — État global

```typescript
// ============================================================
// src/types/state.ts
// ============================================================

import type { GpxPoint } from './gpx';
import type { AutoClimb, UserClimb } from './climb';

export interface RawStats {
  n: number;
  dPlus: number;
  dMoins: number;
  gapMin: number;
  gapMed: number;
  gapMax: number;
  gapMean: number;
  eleMin: number;
  eleMax: number;
  doublons: number;
}

/** Résultat de `rebuild()` — vue enrichie pour le rendu. */
export interface RebuildResult {
  dist: number[];
  eleRaw: number[];
  eleDisplay: number[];
  slopeRaw: number[];
  slopeDisplay: number[];
  slopeForCat: number[];
  catSegments: CatSegment[];
  climbs: Climb[];
}

export interface AppState {
  // Données brutes (fichier GPX)
  rawPoints: GpxPoint[] | null;
  distRaw: number[] | null;
  eleRawOrig: number[] | null;
  totalDistRawM: number;
  rawStats: RawStats | null;

  // Données resamplées + nettoyées
  dist: number[] | null;
  eleRaw: number[] | null;
  stepM: number;
  fileName: string | null;

  // Montées
  autoClimbs: AutoClimb[];
  userClimbs: UserClimb[];

  // UI
  selectedClimbIdx: number | null;
  profileMode: 'full' | 'climb';
  pendingNewClimb: { valleyKm: number; summitKm: number } | null;
  newClimbDragging: boolean;

  // Groupes
  nextGroupId: number;

  // Cache de rendu
  eleDisplay: number[] | null;
  __lastData: RebuildResult | null;
}

export interface MapState {
  map: any | null;              // mapboxgl.Map
  ready: boolean;
  loading: boolean;
  basemap: 'standard' | 'satellite';
  tokenErrorShown: boolean;
}
```

### 3.4 Types TypeScript — Analyse complète

```typescript
// ============================================================
// src/types/analysis.ts
// ============================================================

import type { GpxPoint } from './gpx';
import type { AutoClimb } from './climb';
import type { RawStats } from './state';

export interface AnalyzerResult {
  track: {
    name: string;
    points: GpxPoint[];
  };
  distRaw: number[];
  eleRawOrig: number[];
  dist: number[];
  ele: number[];
  stepM: number;
  avgWinM: number;
  rawStats: RawStats;
  autoClimbs: AutoClimb[];
  /** Nombre de points effectifs corrigés par l'anti-pic (v3.6). */
  despikeCount: number;
  /** Nombre de points effectifs dans la fenêtre Hampel. */
  despikeWinPtsEffective: number;
}
```

### 3.5 Types TypeScript — Diagnostic et export

```typescript
// ============================================================
// src/types/diagnostics.ts
// ============================================================

export type DiagStatus = 'green' | 'orange' | 'red' | 'grey';

export interface DiagEntry {
  key: string;
  label: string;
  effective: string;
  optimal: string;
  target: string | null;
  status: DiagStatus;
  msg: string;
}

export interface DiagWarning {
  level: 'red' | 'orange';
  msg: string;
}

export interface DiagResult {
  diag: Record<string, DiagEntry>;
  warnings: DiagWarning[];
}

// ============================================================
// src/types/export.ts
// ============================================================

export interface ExportSegment {
  index: number;
  startKm: number;
  endKm: number;
  startProgress: number;
  endProgress: number;
  categoryId: string;
  avgSlopePct: number;
  dPlusM: number;
}

export interface ExportSlopeProfileEntry {
  kmStart: number;
  kmEnd: number;
  eleStartM: number;
  eleEndM: number;
  avgSlopePct: number;
  categoryId: string;
}

export interface ExportClimb {
  index: number;
  groupId: number | null;
  subIndex: number | null;
  startKm: number;
  endKm: number;
  startProgress: number;
  endProgress: number;
  dPlusM: number;
  avgSlopePct: number;
  altStartM: number;
  altEndM: number;
  mergedCount: number;
  fallback: boolean;
  slopeProfile: ExportSlopeProfileEntry[];
  slopeProfileStepM: number;
}

export interface ExportData {
  metadata: {
    file: string;
    exportedAt: string;
    version: string;
    totalDistanceKm: number;
    totalDistanceSampledKm: number;
    trailingGapM: number;
    stepM: number;
    totalPoints: number;
    segmentsCount: number;
    climbsCount: number;
    profileStepM: number;
  };
  legend: {
    note: string;
    fenetreCatM: number;
    longueurMin: number;
    categories: Array<{ id: string; label: string; minPct: number | null; maxPct: number | null }>;
  };
  segments: ExportSegment[];
  climbs: ExportClimb[];
}
```

---

## 4. Inventaire complet des paramètres

### 4.1 Paramètres utilisateur (15 numériques + 2 toggles)

| Nom UI | Nom technique | Type | Plage | Défaut | Impact |
|---|---|---|---|---|---|
| Pas de rééchantillonnage | `stepM` | float | 2–200 | **50** | Espacement cible |
| Anti-pic : fenêtre | `despikeWindowM` | float | 50–500 | **200** | Fenêtre Hampel (m) |
| Anti-pic : seuil | `despikeThresholdM` | float | 0–100 | **15** | Seuil Hampel (m) |
| Lissage altitude | `fenetreAffichageM` | float | 20–∞ | **500** | Douceur profil + catégorisation |
| Lissage pente | `fenetrePenteM` | float | 10–∞ | **150** | Douceur pente |
| Fenêtre de calcul pente | `fenetreCatM` | float | 50–∞ | **100** | Largeur couleur |
| Seuil détection | `seuilMontee` | float | 0.1–∞ | **0.5** | Sensibilité détection |
| Longueur min. pente | `longueurMin` | float | 0–∞ | **200** | Fusion segments catégorie |
| Écart min. entre montées | `gapMinM` | float | 0–5000 | **250** | Fusion montées |
| Recherche extrema | `refinePct` | float | 1–500 | **60** | Distance max recherche |
| Filtre médian | `medianWindowM` | float | 0–300 | **0** | Élimination pics isolés |
| Lissage gaussien | `gaussianSigmaM` | float | 0–500 | **75** | Adoucissement |
| Correction plateaux | `plateauRun` | int | 0–20 | **6** | Détection plateaux |
| Segments debug | `segmentLogM` | int | 10–2000 | **100** | Granularité log |
| Pas du profil exporté | `exportProfileStepM` | float | 10–500 | **50** | Granularité JSON |
| Double SG | `smoothExtra` | bool | — | **ON** | Second passage SG |
| Lissage segments | `segmentSmooth` | bool | — | **ON** | Médiane mobile 3 sur logs |

**Note importante** : les clés techniques sont **stables** entre versions. Seuls les libellés UI changent.

### 4.2 Constantes internes (non exposées)

| Nom | Valeur | Rôle |
|---|---|---|
| `REFINE_MIN_M` | 1500 | Plancher garde-fou |
| `REFINE_MAX_M` | 8000 | Plafond garde-fou |
| `STEP_M_MIN` | 2 | Pas resampling min |
| `STEP_M_MAX` | 200 | Pas resampling max |
| `GAP_MIN_M_DEFAULT` | 250 | Fusion |
| `SLOPE_DISPLAY_MAX_PCT` | 25 | Écrêtage pentes |
| `PLATEAU_TOLERANCE_M` | 1.0 | Tolérance plateaux |
| `RAW_DOUBLON_MIN_M` | 1.0 | Seuil doublon |
| `DESPIKE_WINDOW_M_DEFAULT` | 200 | Fenêtre Hampel (m) |
| `DESPIKE_WINDOW_M_MIN` | 50 | Plancher fenêtre |
| `DESPIKE_WINDOW_M_MAX` | 500 | Plafond fenêtre |
| `DESPIKE_THRESHOLD_M_DEFAULT` | 15 | Seuil Hampel |
| `RESAMPLE_AVG_MIN_M` | 75 | Fenêtre minimale de moyennage |
| `EXPORT_PROFILE_STEP_M_DEFAULT` | 50 | Granularité profil |
| `EXPORT_PROFILE_STEP_M_MIN` | 10 | Plancher |
| `EXPORT_PROFILE_STEP_M_MAX` | 500 | Plafond |
| `INSERT_EDGE_MIN_KM` | 1.0 | Espace avant/après |
| `INSERT_BETWEEN_MIN_KM` | 0.2 | Espace entre montées |
| `MANUAL_MIN_LEN_KM` | 0.05 | Longueur min. création |
| **`PROFILE_SCALE_X_FULL`** | **50** | **m/px profil complet** |
| **`PROFILE_SCALE_X_CLIMB`** | **10** | **m/px widget montée** |
| **`PROFILE_SCALE_Y`** | **10** | **m/px vertical** |
| `PROFILE_MIN_HEIGHT` | 110 | Hauteur mini canvas |
| `PROFILE_HANDLE_PAD` | 200 | Marge m autour d'une montée |
| **`MAP_TERRAIN_EXAGGERATION`** | **1** | **Exagération terrain 3D** |
| **`MAP_CLIMB_MARGIN`** | **0.20** | **Marge zoom montée** |
| **`MAP_MIN_SPAN_DEG`** | **0.0014** | **Emprise minimale (~150 m)** |
| **`CONTOUR_BASE_COLOR`** | **`#7a5733`** | **Couleur contours normaux** |
| **`CONTOUR_INDEX_COLOR`** | **`#5e3f1d`** | **Couleur contours d'index** |
| `_maxChartPoints` | 4000 | Points max par graphique |
| `_maxSearchSteps` | 800 | Sous-échantillonnage findBestSubClimb |
| `_minSegmentLength` | 300 | Longueur min absolue |
| `DRAG_MIN_LEN_KM` | 0.05 | Longueur min drag |
| `DRAG_MIN_GAP_KM` | 0.001 | Gap min drag |

### 4.3 Critères de validation

```typescript
export const CLIMB_CRITERIA = [
  { minLen: 1000, minSlope: 3 },  // ≥ 1000 m  → ≥ 3 %
  { minLen: 750,  minSlope: 4 },  // 750–1000  → ≥ 4 %
  { minLen: 500,  minSlope: 5 },  // 500–750   → ≥ 5 %
  { minLen: 400,  minSlope: 6 },  // 400–500   → ≥ 6 %
  { minLen: 300,  minSlope: 7 },  // 300–400   → ≥ 7 %
  // < 300 m : rejeté
];
```

### 4.4 Règles de cohérence (diagnostic)

| Règle | Condition | Niveau |
|---|---|---|
| Fenêtre SG ≥ 5 pts | `fenetreAffichageM ≥ 5 × stepM` | 🔴 si < 5, 🟠 si < 10 |
| Ratio altitude/pente ≥ 2 | `fenetreAffichageM ≥ 2 × fenetrePenteM` | 🔴 si < 1.5, 🟠 si < 2 |
| Filtre médian ≥ 3 pts | `medianWindowM ≥ 3 × stepM` (ou 0) | 🟠 |
| Gaussien ≥ 0.5 pt | `gaussianSigmaM ≥ 0.5 × stepM` (ou 0) | 🟠 |
| Longueur min. pente ≥ Pas | `longueurMin ≥ stepM` (ou 0) | 🟠 |
| Écart min. ≥ Pas | `gapMinM ≥ stepM` (ou 0) | 🟠 |
| Anti-pic fenêtre ≥ 2 × Pas | `despikeWindowM ≥ 2 × stepM` (ou seuil = 0) | 🟠 |
| Anti-pic seuil ≥ 5 m | `despikeThresholdM ≥ 5` (ou 0) | 🟠 |
| Pas du profil exporté ≤ Longueur min. pente | `exportProfileStepM ≤ longueurMin` | 🟠 |

### 4.5 Validation des paramètres

```typescript
// src/utils/parameterValidation.ts

export function validateParameters(p: AnalyzerParameters): AnalyzerParameters {
  return {
    stepM: Math.max(2, Math.min(200, p.stepM)),
    despikeWindowM: Math.max(50, Math.min(500, p.despikeWindowM)),
    despikeThresholdM: Math.max(0, Math.min(100, p.despikeThresholdM)),
    fenetreAffichageM: Math.max(20, p.fenetreAffichageM),
    fenetrePenteM: Math.max(10, p.fenetrePenteM),
    fenetreCatM: Math.max(50, p.fenetreCatM),
    seuilMontee: Math.max(0.1, p.seuilMontee),
    longueurMin: Math.max(0, p.longueurMin),
    gapMinM: Math.max(0, Math.min(5000, p.gapMinM)),
    refinePct: Math.max(1, Math.min(500, p.refinePct)),
    medianWindowM: Math.max(0, Math.min(300, p.medianWindowM)),
    gaussianSigmaM: Math.max(0, Math.min(500, p.gaussianSigmaM)),
    plateauRun: Math.max(0, Math.min(20, p.plateauRun)),
    segmentLogM: Math.max(10, Math.min(2000, p.segmentLogM)),
    exportProfileStepM: Math.max(10, Math.min(500, p.exportProfileStepM)),
    smoothExtra: !!p.smoothExtra,
    segmentSmooth: !!p.segmentSmooth,
  };
}
```

---

## 5. Paramètres de catégorisation de pente

### 5.1 Configuration (figée depuis V3)

```typescript
// src/utils/categorization.ts

export interface SlopeCategory {
  id: string;
  label: string;
  color: string;
  min: number;
  max: number;
}

export const SLOPE_CATEGORIES: SlopeCategory[] = [
  { id: 'dh3', label: 'Descente raide',       color: '#1e40af', min: -Infinity, max: -7 },
  { id: 'dh2', label: 'Descente',             color: '#3b82f6', min: -7, max: -3 },
  { id: 'dh1', label: 'Faux plat descendant', color: '#93c5fd', min: -3, max: -1 },
  { id: 'pl',  label: 'Plat',                 color: '#22c55e', min: -1, max: 1 },
  { id: 'up1', label: 'Faux plat montant',    color: '#fde68a', min: 1,  max: 3 },
  { id: 'up2', label: 'Montée',               color: '#f97316', min: 3,  max: 7 },
  { id: 'up3', label: 'Montée raide',         color: '#dc2626', min: 7,  max: 10 },
  { id: 'up4', label: 'Montée très raide',    color: '#000000', min: 10, max: Infinity },
];

export const CATEGORY_BY_ID: Record<string, SlopeCategory> =
  Object.fromEntries(SLOPE_CATEGORIES.map(c => [c.id, c]));

export function categorize(slope: number): string {
  for (const c of SLOPE_CATEGORIES) if (slope >= c.min && slope < c.max) return c.id;
  return 'pl';
}

export function colorForSlope(slope: number): string {
  return CATEGORY_BY_ID[categorize(slope)].color;
}
```

### 5.2 Tableau récapitulatif

| Plage | ID | Libellé | Couleur | Hex |
|---|---|---|---|---|
| p < −7 % | `dh3` | Descente raide | Bleu foncé | `#1e40af` |
| −7 ≤ p < −3 % | `dh2` | Descente | Bleu | `#3b82f6` |
| −3 ≤ p < −1 % | `dh1` | Faux plat descendant | Bleu clair | `#93c5fd` |
| −1 ≤ p ≤ +1 % | `pl` | Plat | Vert | `#22c55e` |
| +1 < p ≤ +3 % | `up1` | Faux plat montant | Jaune | `#fde68a` |
| +3 < p ≤ +7 % | `up2` | Montée | Orange | `#f97316` |
| +7 < p ≤ +10 % | `up3` | Montée raide | Rouge | `#dc2626` |
| p > +10 % | `up4` | Montée très raide | Noir | `#000000` |

**Règles** :
- La **borne haute est exclusive** : une pente de +3 % exactement tombe dans `up1`
- La catégorisation est **symétrique** pour les faux plats, **asymétrique** pour les raides

### 5.3 Groupes de montées — helpers

```typescript
// src/utils/groupHelpers.ts

/**
 * Couleur stable pour un groupe (angle d'or → teinte bien répartie).
 */
export function groupColor(gid: number): string {
  const hue = (gid * 137.508) % 360;
  return `hsl(${hue.toFixed(0)}, 62%, 46%)`;
}

/**
 * Numérotation d'affichage : "#N" (autonome) ou "#N.M" (groupé).
 * Calculée à partir du tableau trié de montées.
 */
export function computeClimbLabels(climbs: { groupId: number | null; subIndex: number | null }[]): string[] {
  let mainCounter = 0;
  const groupCounters: Record<number, number> = {};
  const labels: string[] = [];

  for (const c of climbs) {
    if (c.groupId != null) {
      if (groupCounters[c.groupId] === undefined) groupCounters[c.groupId] = 0;
      groupCounters[c.groupId]++;
      const sub = c.subIndex != null ? c.subIndex : groupCounters[c.groupId];
      labels.push(`#${c.groupId}.${sub}`);
    } else {
      mainCounter++;
      labels.push(`#${mainCounter}`);
    }
  }
  return labels;
}
```

---

## 6. Modules TypeScript à porter

### 6.1 Utilitaires de base

```typescript
// src/utils/haversine.ts

const R = 6371000;

export function haversine(lat1: number, lon1: number,
                          lat2: number, lon2: number): number {
  const phi1 = (lat1 * Math.PI) / 180;
  const phi2 = (lat2 * Math.PI) / 180;
  const dphi = ((lat2 - lat1) * Math.PI) / 180;
  const dlam = ((lon2 - lon1) * Math.PI) / 180;

  const a = Math.sin(dphi / 2) ** 2
          + Math.cos(phi1) * Math.cos(phi2) * Math.sin(dlam / 2) ** 2;

  return 2 * R * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
}

export function computeCumulativeDistances(
  points: Array<{ lat: number; lon: number }>,
): Float64Array {
  const n = points.length;
  const dist = new Float64Array(n);
  for (let i = 1; i < n; i++) {
    dist[i] = dist[i - 1] + haversine(
      points[i - 1].lat, points[i - 1].lon,
      points[i].lat, points[i].lon,
    );
  }
  return dist;
}
```

### 6.2 Rééchantillonnage par moyenne

```typescript
// src/utils/resample.ts

export const RESAMPLE_AVG_MIN_M = 75;

export function resampleUniform(
  distRaw: Float64Array,
  eleRaw: Float64Array,
  stepM: number,
): { dist: Float64Array; ele: Float64Array; stepM: number; avgWinM: number } {
  const n = distRaw.length;
  const totalM = distRaw[n - 1];
  const m = Math.floor(totalM / stepM) + 1;
  const dist = new Float64Array(m);
  const ele  = new Float64Array(m);

  const avgWin = Math.min(Math.max(stepM, RESAMPLE_AVG_MIN_M), 3 * stepM);
  const halfW = avgWin / 2;

  let startPtr = 0;
  for (let k = 0; k < m; k++) {
    const dCenter = k * stepM;
    dist[k] = dCenter;
    const lo = dCenter - halfW;
    const hi = dCenter + halfW;

    while (startPtr < n - 1 && distRaw[startPtr] < lo) startPtr++;

    let sumEle = 0;
    let count = 0;
    for (let i = startPtr; i < n; i++) {
      if (distRaw[i] >= hi) break;
      if (distRaw[i] >= lo) {
        sumEle += eleRaw[i];
        count++;
      }
    }

    if (count > 0) {
      ele[k] = sumEle / count;
    } else {
      // Fallback : interpolation linéaire
      let jj = 0;
      while (jj < n - 2 && distRaw[jj + 1] < dCenter) jj++;
      const d0 = distRaw[jj];
      const d1 = distRaw[jj + 1];
      if (d1 - d0 < 1e-6) {
        ele[k] = eleRaw[jj];
      } else {
        const t = (dCenter - d0) / (d1 - d0);
        ele[k] = eleRaw[jj] + (eleRaw[jj + 1] - eleRaw[jj]) * t;
      }
    }
  }

  return { dist, ele, stepM, avgWinM: avgWin };
}
```

### 6.3 Correction plateaux

```typescript
// src/utils/plateau.ts

export function plateauCorrect(
  ele: Float64Array,
  minRun: number,
  tolerance: number,
): { out: Float64Array; count: number } {
  const n = ele.length;
  const out = Float64Array.from(ele);
  if (n < 3 || minRun < 2) return { out, count: 0 };

  let count = 0;
  let i = 0;
  while (i < n) {
    let j = i + 1;
    while (j < n && Math.abs(ele[j] - ele[i]) <= tolerance) j++;
    const runLen = j - i;
    if (runLen >= minRun && i > 0 && j < n) {
      const eStart = ele[i - 1];
      const eEnd = ele[j];
      const span = j - (i - 1);
      for (let k = i; k < j; k++) {
        const t = (k - (i - 1)) / span;
        out[k] = eStart + (eEnd - eStart) * t;
      }
      count++;
    }
    i = j;
  }
  return { out, count };
}
```

### 6.4 Anti-pic Hampel

```typescript
// src/utils/despike.ts

/**
 * Convertit une fenêtre en mètres en nombre de points.
 * - Force un nombre impair
 * - Garantit un minimum de 3 points
 */
export function despikeWindowMToPts(windowM: number, stepM: number): number {
  return Math.max(3, Math.round(windowM / stepM) | 1);
}

/**
 * Filtre Hampel : remplace les points aberrants par la médiane locale.
 * Ne modifie QUE les points qui dévient fortement de leur voisinage.
 */
export function despikeAltitude(
  ele: Float64Array,
  winPts: number,
  thresholdM: number,
): { out: Float64Array; count: number } {
  const n = ele.length;
  if (n < 3 || winPts < 3 || thresholdM <= 0) {
    return { out: Float64Array.from(ele), count: 0 };
  }
  if (winPts % 2 === 0) winPts += 1;
  const half = Math.floor(winPts / 2);
  const out = Float64Array.from(ele);
  const buf: number[] = [];
  let count = 0;

  for (let i = 0; i < n; i++) {
    buf.length = 0;
    for (let j = -half; j <= half; j++) {
      const idx = Math.max(0, Math.min(n - 1, i + j));
      buf.push(ele[idx]);
    }
    buf.sort((a, b) => a - b);
    const median = buf[half];
    if (Math.abs(ele[i] - median) > thresholdM) {
      out[i] = median;
      count++;
    }
  }
  return { out, count };
}

/**
 * Wrapper : convertit mètres → points puis applique Hampel.
 */
export function applyDespike(
  ele: Float64Array,
  windowM: number,
  thresholdM: number,
  stepM: number,
): { out: Float64Array; count: number; winPtsEffective: number } {
  const winPts = despikeWindowMToPts(windowM, stepM);
  const result = despikeAltitude(ele, winPts, thresholdM);
  return { ...result, winPtsEffective: winPts };
}
```

### 6.5 Statistiques brutes

```typescript
// src/utils/rawStats.ts

import type { RawStats } from '../types/state';

const RAW_DOUBLON_MIN_M = 1.0;

export function computeRawStats(
  distRaw: Float64Array,
  eleRawOrig: Float64Array,
): RawStats {
  const n = distRaw.length;
  let dPlus = 0, dMoins = 0;
  const gaps: number[] = [];

  for (let i = 1; i < n; i++) {
    const dDist = distRaw[i] - distRaw[i - 1];
    if (dDist > 0.01) gaps.push(dDist);
    if (dDist < RAW_DOUBLON_MIN_M) continue;
    const d = eleRawOrig[i] - eleRawOrig[i - 1];
    if (d > 0) dPlus += d; else dMoins -= d;
  }

  gaps.sort((a, b) => a - b);
  const gapMin = gaps[0] ?? 0;
  const gapMax = gaps[gaps.length - 1] ?? 0;
  const gapMed = gaps[Math.floor(gaps.length / 2)] ?? 0;
  const gapMean = gaps.length ? gaps.reduce((a, b) => a + b, 0) / gaps.length : 0;

  let eleMin = Infinity, eleMax = -Infinity;
  for (const e of eleRawOrig) {
    if (e < eleMin) eleMin = e;
    if (e > eleMax) eleMax = e;
  }

  return {
    n, dPlus, dMoins, gapMin, gapMed, gapMax, gapMean,
    eleMin, eleMax, doublons: n - gaps.length,
  };
}
```

### 6.6 Lissages

```typescript
// src/utils/smoothing.ts

export function movingAverage(arr: Float64Array, win: number): Float64Array {
  const n = arr.length;
  const res = new Float64Array(n);
  if (win < 2 || n < 2) { res.set(arr); return res; }

  const half = Math.floor(win / 2);
  const prefix = new Float64Array(n + 1);
  for (let i = 0; i < n; i++) prefix[i + 1] = prefix[i] + arr[i];

  for (let i = 0; i < n; i++) {
    const lo = Math.max(0, i - half);
    const hi = Math.min(n - 1, i + half);
    res[i] = (prefix[hi + 1] - prefix[lo]) / (hi - lo + 1);
  }
  return res;
}

export function savitzkyGolay(arr: Float64Array, win: number, order = 2): Float64Array {
  const n = arr.length;
  if (win < 5 || n < 5) return Float64Array.from(arr);
  if (win % 2 === 0) win += 1;
  const half = Math.floor(win / 2);
  const m = half;
  const coefs = new Float64Array(win);
  for (let k = -half; k <= half; k++) {
    coefs[k + half] = 3 * m * m + 3 * m - 1 - 5 * k * k;
  }
  const out = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    let sum = 0, wsum = 0;
    for (let j = -half; j <= half; j++) {
      const idx = i + j;
      if (idx < 0 || idx >= n) continue;
      const c = coefs[j + half];
      sum += c * arr[idx]; wsum += c;
    }
    out[i] = wsum > 0 ? sum / wsum : arr[i];
  }
  return out;
}

export function gaussianFilter(arr: Float64Array, sigma: number): Float64Array {
  const n = arr.length;
  if (!sigma || sigma <= 0 || n < 2) return Float64Array.from(arr);

  const half = Math.ceil(3 * sigma);
  const kernel = new Float64Array(2 * half + 1);
  let ksum = 0;
  for (let k = -half; k <= half; k++) {
    const v = Math.exp(-(k * k) / (2 * sigma * sigma));
    kernel[k + half] = v; ksum += v;
  }
  for (let i = 0; i < kernel.length; i++) kernel[i] /= ksum;

  const out = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    let sum = 0, wsum = 0;
    for (let j = -half; j <= half; j++) {
      const idx = i + j;
      if (idx < 0 || idx >= n) continue;
      const w = kernel[j + half];
      sum += w * arr[idx]; wsum += w;
    }
    out[i] = wsum > 0 ? sum / wsum : arr[i];
  }
  return out;
}

export function medianFilter(arr: Float64Array, win: number): Float64Array {
  const n = arr.length;
  if (!win || win < 3 || n < 3) return Float64Array.from(arr);
  if (win % 2 === 0) win += 1;

  const half = Math.floor(win / 2);
  const out = new Float64Array(n);
  const buf: number[] = [];

  for (let i = 0; i < n; i++) {
    buf.length = 0;
    for (let j = -half; j <= half; j++) {
      const idx = Math.max(0, Math.min(n - 1, i + j));
      buf.push(arr[idx]);
    }
    buf.sort((a, b) => a - b);
    out[i] = buf[half];
  }
  return out;
}
```

### 6.7 Lissage ancré

```typescript
// src/utils/anchoredSmoothing.ts

import { savitzkyGolay, gaussianFilter } from './smoothing';

export function anchoredMovingAverage(
  raw: Float64Array,
  anchorIdxs: number[],
  stepM: number,
  fenetreAffichageM: number,
  sigmaM: number,
  doubleSG: boolean,
): Float64Array {
  const n = raw.length;
  if (n < 2) return Float64Array.from(raw);

  const winPts = Math.max(5, Math.round(fenetreAffichageM / stepM) | 1);
  let base = savitzkyGolay(raw, winPts, 2);

  if (sigmaM > 0) {
    const sigmaPts = sigmaM / stepM;
    base = gaussianFilter(base, sigmaPts);
  }

  const set = new Set<number>([0, n - 1]);
  for (const i of anchorIdxs) if (i > 0 && i < n - 1) set.add(i);
  const anchors = Array.from(set).sort((a, b) => a - b);

  const result = Float64Array.from(base);
  for (let k = 0; k < anchors.length - 1; k++) {
    const a = anchors[k], b = anchors[k + 1];
    if (b - a < 2) continue;
    const corrA = raw[a] - base[a];
    const corrB = raw[b] - base[b];
    for (let i = a; i <= b; i++) {
      const t = (i - a) / (b - a);
      result[i] = base[i] + corrA * (1 - t) + corrB * t;
    }
  }

  if (doubleSG) {
    const winDouble = Math.max(31, Math.round(fenetreAffichageM * 2 / stepM) | 1);
    const final = savitzkyGolay(result, winDouble, 2);
    for (const a of anchors) final[a] = result[a];
    return final;
  }
  return result;
}
```

### 6.8 Pentes

```typescript
// src/utils/slope.ts

export function computeSlope(ele: Float64Array, dist: Float64Array, minD = 0.05): Float64Array {
  const n = ele.length;
  const s = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const i1 = Math.max(0, i - 1);
    const i2 = Math.min(n - 1, i + 1);
    const d = dist[i2] - dist[i1];
    if (d > minD) s[i] = ((ele[i2] - ele[i1]) / d) * 100;
  }
  return s;
}

export function computeSlopeWindow(ele: Float64Array, dist: Float64Array, winM: number): Float64Array {
  const n = ele.length;
  const s = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    let lo = i;
    while (lo > 0 && dist[i] - dist[lo - 1] <= winM) lo--;
    let hi = i;
    while (hi < n - 1 && dist[hi + 1] - dist[i] <= winM) hi++;
    const dDist = dist[hi] - dist[lo];
    if (dDist > 0.5) s[i] = ((ele[hi] - ele[lo]) / dDist) * 100;
  }
  return s;
}

export function clipSlope(slope: Float64Array, maxPct = 25): Float64Array {
  const out = new Float64Array(slope.length);
  for (let i = 0; i < slope.length; i++) {
    const v = slope[i];
    out[i] = v > maxPct ? maxPct : (v < -maxPct ? -maxPct : v);
  }
  return out;
}
```

### 6.9 Détection (hystérésis, candidats, subClimb, computeAuto)

```typescript
// src/utils/detection.ts

import type { AnalyzerParameters } from '../types/parameters';
import type { AutoClimb } from '../types/climb';
import { CLIMB_CRITERIA } from './categorization';

const REFINE_MIN_M = 1500;
const REFINE_MAX_M = 8000;
const SLOPE_DISPLAY_MAX_PCT = 25;

export interface Event { idx: number; type: 'start' | 'summit' }
export interface Candidate { startIdx: number; endIdx: number; key: string }
export interface SubClimb { startIdx: number; endIdx: number; length: number; dPlus: number; avgSlope: number }

export function minSlopeForLength(len: number): number {
  for (const c of CLIMB_CRITERIA) if (len >= c.minLen) return c.minSlope;
  return Infinity;
}

export function detectEvents(slope: Float64Array, seuil: number): Event[] {
  const ev: Event[] = [];
  let inC = false;
  for (let i = 0; i < slope.length; i++) {
    if (!inC && slope[i] > seuil) { ev.push({ idx: i, type: 'start' }); inC = true; }
    else if (inC && slope[i] < -seuil) { ev.push({ idx: i, type: 'summit' }); inC = false; }
  }
  if (inC) ev.push({ idx: slope.length - 1, type: 'summit' });
  return ev;
}

export function buildClimbCandidates(events: Event[], n: number): Candidate[] {
  const segs: Candidate[] = [];
  let prev = 0;
  let cur: 'climb' | 'non-climb' = (events.length && events[0].type === 'summit') ? 'climb' : 'non-climb';

  for (const e of events) {
    if (e.idx > prev) segs.push({ startIdx: prev, endIdx: e.idx, key: cur });
    cur = e.type === 'start' ? 'climb' : 'non-climb';
    prev = e.idx;
  }
  if (prev < n - 1) segs.push({ startIdx: prev, endIdx: n - 1, key: cur });
  return segs.filter(s => s.key === 'climb');
}

export function findBestSubClimb(
  dist: Float64Array,
  ele: Float64Array,
  startIdx: number,
  endIdx: number,
): SubClimb | null {
  const n = endIdx - startIdx + 1;
  if (n < 2) return null;
  const cum = new Float64Array(n);
  for (let k = 1; k < n; k++) {
    const d = ele[startIdx + k] - ele[startIdx + k - 1];
    cum[k] = cum[k - 1] + (d > 0 ? d : 0);
  }
  const step = n > 800 ? Math.ceil(n / 800) : 1;
  let best: SubClimb | null = null;
  for (let si = 0; si < n - 1; si += step) {
    for (let ei = si + 1; ei < n; ei += step) {
      const len = dist[startIdx + ei] - dist[startIdx + si];
      if (len < 300) continue;
      if (best && len <= best.length) continue;
      const dP = cum[ei] - cum[si];
      const s = (dP / len) * 100;
      if (s >= minSlopeForLength(len)) {
        best = { startIdx: startIdx + si, endIdx: startIdx + ei, length: len, dPlus: dP, avgSlope: s };
      }
    }
  }
  return best;
}

// Helpers inline (dupliqués pour éviter la dépendance circulaire)
function findIdxAtDist(dist: Float64Array, targetM: number): number {
  let lo = 0, hi = dist.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (dist[mid] < targetM) lo = mid + 1; else hi = mid;
  }
  return lo;
}
function argminRange(arr: Float64Array, lo: number, hi: number): number {
  let best = lo, v = arr[lo];
  for (let i = lo + 1; i <= hi; i++) if (arr[i] < v) { v = arr[i]; best = i; }
  return best;
}
function argmaxRange(arr: Float64Array, lo: number, hi: number): number {
  let best = lo, v = arr[lo];
  for (let i = lo + 1; i <= hi; i++) if (arr[i] > v) { v = arr[i]; best = i; }
  return best;
}

export function computeAuto(
  dist: Float64Array,
  eleRaw: Float64Array,
  params: AnalyzerParameters,
): { autoClimbs: AutoClimb[] } {
  const { stepM, fenetreAffichageM, fenetrePenteM, seuilMontee, refinePct,
          medianWindowM, gaussianSigmaM, gapMinM } = params;
  const n = eleRaw.length;

  // Filtre médian optionnel
  let eleMedian = eleRaw;
  if (medianWindowM > 0) {
    const winPts = Math.max(3, Math.round(medianWindowM / stepM) | 1);
    eleMedian = medianFilter(eleRaw, winPts);
  }

  // Lissage SG
  const winSGPts = Math.max(5, Math.round(fenetreAffichageM / stepM) | 1);
  const eleBase = savitzkyGolay(eleMedian, winSGPts, 2);

  // Pente lissée + clip
  const winSlopePts = Math.max(1, Math.round(fenetrePenteM / stepM));
  const slopeBase = clipSlope(
    movingAverage(computeSlope(eleBase, dist), winSlopePts),
    SLOPE_DISPLAY_MAX_PCT,
  );

  // Candidats
  const candidates = buildClimbCandidates(detectEvents(slopeBase, seuilMontee), n);

  // Sous-segment optimal
  const candidatesWithSub: SubClimb[] = [];
  for (const c of candidates) {
    const sub = findBestSubClimb(dist, eleBase, c.startIdx, c.endIdx);
    if (sub) candidatesWithSub.push(sub);
  }

  // Raffinement + Fusion (avec conservation des mergedParts)
  interface Refined {
    startIdx: number;
    endIdx: number;
    refineM: number;
    candidateLen: number;
    _merged: number;
    _mergedParts: { startKm: number; endKm: number }[] | null;
  }

  const anchors: number[] = [];
  const refined: Refined[] = [];
  let lastEnd = -1;

  for (const c of candidatesWithSub) {
    const candidateLen = c.length;
    const effectiveRefine = Math.max(
      REFINE_MIN_M,
      Math.min(REFINE_MAX_M, refinePct * candidateLen / 100),
    );
    const hiIdx = findIdxAtDist(dist, Math.min(dist[n - 1], dist[c.endIdx] + effectiveRefine));
    const newEnd = (hiIdx > c.endIdx) ? argmaxRange(eleBase, c.endIdx, hiIdx) : c.endIdx;
    if (newEnd <= c.startIdx) continue;
    if (newEnd <= lastEnd) continue;
    const naturalLow = lastEnd + 1;
    const safetyLow = findIdxAtDist(dist, Math.max(0, dist[c.startIdx] - effectiveRefine));
    const loIdx = Math.max(naturalLow, safetyLow);
    const newStart = (loIdx < c.startIdx) ? argminRange(eleBase, loIdx, c.startIdx) : c.startIdx;
    if (newStart >= newEnd) continue;

    // Fusion
    if (refined.length > 0 && gapMinM > 0) {
      const prev = refined[refined.length - 1];
      const gapM = dist[newStart] - dist[prev.endIdx];
      if (gapM < gapMinM) {
        // Conservation des bornes de la sous-montée initiale
        if (!prev._mergedParts) {
          prev._mergedParts = [{
            startKm: dist[prev.startIdx] / 1000,
            endKm: dist[prev.endIdx] / 1000,
          }];
        }
        const combinedEnd = argmaxRange(eleBase, prev.endIdx, newEnd);
        const midValley = argminRange(eleBase, prev.endIdx, newStart);
        let newPrevStart = prev.startIdx;
        if (eleBase[midValley] < eleBase[prev.startIdx]) {
          newPrevStart = midValley;
          anchors[anchors.length - 2] = midValley;
        }
        // Ajouter la nouvelle sous-montée
        prev._mergedParts.push({
          startKm: dist[newStart] / 1000,
          endKm: dist[newEnd] / 1000,
        });
        prev.startIdx = newPrevStart;
        prev.endIdx = combinedEnd;
        prev.candidateLen += candidateLen;
        prev.refineM = Math.max(prev.refineM, effectiveRefine);
        prev._merged += 1;
        anchors[anchors.length - 1] = combinedEnd;
        lastEnd = combinedEnd;
        continue;
      }
    }

    anchors.push(newStart, newEnd);
    refined.push({
      startIdx: newStart, endIdx: newEnd,
      refineM: effectiveRefine, candidateLen,
      _merged: 0, _mergedParts: null,
    });
    lastEnd = newEnd;
  }

  // Lissage ancré final
  const eleDisplay = anchoredMovingAverage(
    eleRaw, anchors, stepM, fenetreAffichageM, gaussianSigmaM, params.smoothExtra,
  );

  // Validation stricte + préservation de mergedParts
  const autoClimbs: AutoClimb[] = [];
  for (const c of refined) {
    let finalStart = c.startIdx;
    let finalEnd = c.endIdx;
    const length = dist[finalEnd] - dist[finalStart];
    let dPlus = 0;
    for (let k = finalStart + 1; k <= finalEnd; k++) {
      const d = eleDisplay[k] - eleDisplay[k - 1];
      if (d > 0) dPlus += d;
    }
    const avgSlope = length > 0 ? (dPlus / length) * 100 : 0;
    const minSlope = minSlopeForLength(length);
    let fallbackFlag = false;

    if (length < 300 || avgSlope < minSlope) {
      const sub = findBestSubClimb(dist, eleDisplay, c.startIdx, c.endIdx);
      if (!sub) continue;
      finalStart = sub.startIdx;
      finalEnd = sub.endIdx;
      fallbackFlag = true;
    }

    autoClimbs.push({
      startKm: dist[finalStart] / 1000,
      endKm: dist[finalEnd] / 1000,
      refineM: c.refineM,
      candidateLen: c.candidateLen,
      mergedCount: c._merged,
      mergedParts: c._mergedParts,
      fallback: fallbackFlag,
    });
  }

  return { autoClimbs };
}
```

**Note** : les fonctions `medianFilter`, `savitzkyGolay`, `movingAverage`, `computeSlope`, `clipSlope`, `anchoredMovingAverage` doivent être importées depuis leurs modules respectifs.

### 6.10 Contraintes anti-chevauchement

```typescript
// src/utils/climbConstraints.ts

import type { UserClimb } from '../types/climb';

const START_OFFSET_KM = 0.001;
const END_OFFSET_KM = 0.1;
const DRAG_MIN_LEN_KM = 0.05;
const DRAG_MIN_GAP_KM = 0.001;

export function applyConstraints(userClimbs: UserClimb[]): UserClimb[] {
  const sorted = [...userClimbs].sort((a, b) => a.startKm - b.startKm);
  let previousEndKm = -Infinity;

  for (const c of sorted) {
    c._constrainedStart = false;
    c._constrainedEnd = false;

    if (c.startKm < previousEndKm) {
      c._constrainedStart = true;
      c.startKm = previousEndKm + START_OFFSET_KM;
    }
    if (c.endKm <= c.startKm) {
      c._constrainedEnd = true;
      c.endKm = c.startKm + END_OFFSET_KM;
    }
    previousEndKm = c.endKm;
  }
  return sorted;
}

export function applyDragConstraint(
  handle: 'valley' | 'summit',
  km: number,
  climb: UserClimb,
  allClimbs: UserClimb[],
  totalKm: number,
): number {
  const sorted = [...allClimbs].sort((a, b) => a.startKm - b.startKm);
  const pos = sorted.indexOf(climb);
  const prevClimb = pos > 0 ? sorted[pos - 1] : null;
  const nextClimb = pos < sorted.length - 1 ? sorted[pos + 1] : null;

  const lo = prevClimb ? prevClimb.endKm + DRAG_MIN_GAP_KM : 0;
  const hi = nextClimb ? nextClimb.startKm - DRAG_MIN_GAP_KM : totalKm;

  if (handle === 'valley') {
    return Math.max(lo, Math.min(climb.endKm - DRAG_MIN_LEN_KM, km));
  } else {
    return Math.max(climb.startKm + DRAG_MIN_LEN_KM, Math.min(hi, km));
  }
}
```

### 6.11 Groupes — séparation, refusion, normalisation

```typescript
// src/utils/climbGroups.ts

import type { UserClimb } from '../types/climb';

/**
 * Sépare une montée fusionnée en ses sous-montées d'origine.
 * Alloue un nouveau groupId, retire la montée fusionnée,
 * insère les sous-montées triées, retourne les nouvelles montées et le gid.
 */
export function splitMergedClimb(
  mergedIdx: number,
  userClimbs: UserClimb[],
  nextGroupId: number,
): { newClimbs: UserClimb[]; gid: number } | null {
  const merged = userClimbs[mergedIdx];
  if (!merged) return null;
  if (!Array.isArray(merged.mergedParts) || merged.mergedParts.length < 2) {
    return null;
  }

  const gid = nextGroupId;

  // Retirer la montée fusionnée
  userClimbs.splice(mergedIdx, 1);

  // Créer les sous-montées
  const newClimbs: UserClimb[] = merged.mergedParts.map((part, k) => ({
    startKm: part.startKm,
    endKm: part.endKm,
    autoStartKm: null,
    autoEndKm: null,
    refineM: null,
    candidateLen: null,
    mergedCount: 0,
    mergedParts: null,
    fallback: false,
    groupId: gid,
    subIndex: k + 1,
  }));

  for (const nc of newClimbs) userClimbs.push(nc);
  userClimbs.sort((a, b) => a.startKm - b.startKm);

  return { newClimbs, gid };
}

/**
 * Refusionne tous les membres d'un groupe en une seule montée fusionnée.
 * Les `mergedParts` sont reconstruits avec les bornes **actuelles**.
 */
export function refuseGroup(
  gid: number,
  userClimbs: UserClimb[],
): { merged: UserClimb } | null {
  const members = userClimbs
    .filter(c => c.groupId === gid)
    .sort((a, b) => a.startKm - b.startKm);

  if (members.length < 2) return null;

  const startKm = members[0].startKm;
  const endKm = members[members.length - 1].endKm;
  const mergedParts = members.map(m => ({ startKm: m.startKm, endKm: m.endKm }));

  // Retirer les membres
  for (let i = userClimbs.length - 1; i >= 0; i--) {
    if (userClimbs[i].groupId === gid) userClimbs.splice(i, 1);
  }

  // Créer la montée fusionnée
  const merged: UserClimb = {
    startKm,
    endKm,
    autoStartKm: null,
    autoEndKm: null,
    refineM: null,
    candidateLen: null,
    mergedCount: members.length - 1,
    mergedParts,
    fallback: false,
    groupId: null,
    subIndex: null,
  };
  userClimbs.push(merged);
  userClimbs.sort((a, b) => a.startKm - b.startKm);

  return { merged };
}

/**
 * Nettoie les groupes orphelins (moins de 2 membres).
 */
export function normalizeGroups(userClimbs: UserClimb[]): void {
  const counts: Record<number, number> = {};
  for (const c of userClimbs) {
    if (c.groupId != null) counts[c.groupId] = (counts[c.groupId] || 0) + 1;
  }
  for (const c of userClimbs) {
    if (c.groupId != null && counts[c.groupId] < 2) {
      c.groupId = null;
      c.subIndex = null;
    }
  }
}
```

### 6.12 Suppression

```typescript
// src/utils/deleteClimb.ts

import type { UserClimb } from '../types/climb';

/**
 * Supprime une montée de la liste, en gérant :
 * - la renumérotation des subIndex du groupe affecté
 * - la dissolution du groupe si < 2 membres restants
 * - l'ajustement de la sélection (si applicable)
 *
 * Le caller doit ajuster `selectedClimbIdx` après l'appel.
 */
export function deleteClimbFromList(
  idx: number,
  userClimbs: UserClimb[],
): { deletedGroupId: number | null; dissolved: boolean } {
  if (idx < 0 || idx >= userClimbs.length) {
    return { deletedGroupId: null, dissolved: false };
  }

  const removed = userClimbs[idx];
  userClimbs.splice(idx, 1);

  let dissolved = false;
  if (removed.groupId != null) {
    const siblings = userClimbs
      .filter(c => c.groupId === removed.groupId)
      .sort((a, b) => a.startKm - b.startKm);

    // Renuméroter les subIndex
    siblings.forEach((c, k) => { c.subIndex = k + 1; });

    // Dissoudre si < 2 membres
    if (siblings.length < 2) {
      for (const c of siblings) {
        c.groupId = null;
        c.subIndex = null;
      }
      dissolved = true;
    }
  }

  return { deletedGroupId: removed.groupId, dissolved };
}
```

### 6.13 Création manuelle

```typescript
// src/utils/manualClimb.ts

import type { UserClimb } from '../types/climb';

const MANUAL_MIN_LEN_KM = 0.05;

/**
 * Vérifie qu'une plage [a, b] ne chevauche aucune montée existante.
 */
export function isFreeRange(aKm: number, bKm: number, userClimbs: UserClimb[]): boolean {
  const lo = Math.min(aKm, bKm);
  const hi = Math.max(aKm, bKm);
  for (const c of userClimbs) {
    if (lo < c.endKm && hi > c.startKm) return false;
  }
  return true;
}

/**
 * Vérifie qu'un point n'est pas dans une montée existante.
 */
export function isFreeKm(km: number, userClimbs: UserClimb[]): boolean {
  for (const c of userClimbs) {
    if (km >= c.startKm && km <= c.endKm) return false;
  }
  return true;
}

/**
 * Crée une nouvelle montée manuelle.
 * Retourne la montée créée ou null si refusée.
 */
export function createManualClimb(
  valleyKm: number,
  summitKm: number,
  userClimbs: UserClimb[],
): UserClimb | null {
  if (summitKm <= valleyKm + MANUAL_MIN_LEN_KM) return null;
  if (!isFreeRange(valleyKm, summitKm, userClimbs)) return null;

  const newClimb: UserClimb = {
    startKm: valleyKm,
    endKm: summitKm,
    autoStartKm: null,
    autoEndKm: null,
    refineM: null,
    candidateLen: null,
    mergedCount: 0,
    mergedParts: null,
    fallback: false,
    groupId: null,
    subIndex: null,
  };

  userClimbs.push(newClimb);
  userClimbs.sort((a, b) => a.startKm - b.startKm);
  return newClimb;
}
```

### 6.14 Construction de la trace carte (points bruts)

```typescript
// src/utils/buildTraceFeatures.ts

import type { GpxPoint } from '../types/gpx';
import type { CatSegment } from '../types/category';
import { CATEGORY_BY_ID } from './categorization';

export interface TraceFeature {
  type: 'Feature';
  geometry: { type: 'LineString'; coordinates: [number, number][] };
  properties: { color: string; categoryId: string };
}

export interface TraceFeatureCollection {
  type: 'FeatureCollection';
  features: TraceFeature[];
}

/**
 * Construit la FeatureCollection de la trace colorée par catégorie.
 *
 * IMPORTANT (v3.9.2) : utilise les POINTS BRUTS du fichier GPX (rawPoints),
 * pas la grille rééchantillonnée. La catégorie de chaque point brut est
 * déterminée par sa distance cumulée (distRaw[i]) comparée aux bornes
 * des catSegments (parcours monotone, O(n)).
 */
export function buildTraceFeatures(
  rawPoints: GpxPoint[] | null,
  distRaw: number[] | null,
  catSegments: CatSegment[],
): TraceFeatureCollection {
  const empty: TraceFeatureCollection = { type: 'FeatureCollection', features: [] };
  if (!rawPoints || !distRaw || rawPoints.length < 2 || catSegments.length === 0) {
    return empty;
  }

  const n = rawPoints.length;

  // 1. Catégoriser chaque point brut par distance cumulée
  const catOf = new Array<string>(n);
  let segIdx = 0;
  for (let i = 0; i < n; i++) {
    const d = distRaw[i];
    while (segIdx < catSegments.length - 1 && d > catSegments[segIdx].endDist) segIdx++;
    while (segIdx > 0 && d < catSegments[segIdx].startDist) segIdx--;
    catOf[i] = catSegments[segIdx].key;
  }

  // 2. Grouper les points consécutifs de même catégorie
  //    (le point de transition est inclus dans les deux LineStrings)
  const features: TraceFeature[] = [];
  let startIdx = 0;
  for (let i = 1; i <= n; i++) {
    if (i === n || catOf[i] !== catOf[startIdx]) {
      const endInc = (i < n) ? i : n - 1;
      const coords: [number, number][] = [];
      for (let k = startIdx; k <= endInc; k++) {
        coords.push([rawPoints[k].lon, rawPoints[k].lat]);
      }
      if (coords.length >= 2) {
        const key = catOf[startIdx];
        const cat = CATEGORY_BY_ID[key];
        if (cat) {
          features.push({
            type: 'Feature',
            geometry: { type: 'LineString', coordinates: coords },
            properties: { color: cat.color, categoryId: key },
          });
        }
      }
      startIdx = i;
    }
  }

  return { type: 'FeatureCollection', features };
}
```

### 6.15 Construction des marqueurs (mono/groupe)

```typescript
// src/utils/buildMapMarkers.ts

import type { UserClimb } from '../types/climb';
import type { GpxPoint } from '../types/gpx';

export interface MarkerFeature {
  type: 'Feature';
  geometry: { type: 'Point'; coordinates: [number, number] };
  properties: { kind: 'valley' | 'summit'; role: 'selected' | 'member' };
}

/**
 * Construit les marqueurs vallée/sommet.
 *
 * - Si la montée sélectionnée est autonome : 2 marqueurs (role='selected').
 * - Si elle appartient à un groupe (v3.9.2) : TOUS les points vallée/sommet
 *   de TOUS les membres du groupe, avec role='selected' pour le membre
 *   actif et role='member' pour les autres.
 */
export function buildMapMarkers(
  selectedClimbIdx: number | null,
  userClimbs: UserClimb[],
  latLonAtDist: (dist: number) => [number, number],
): { type: 'FeatureCollection'; features: MarkerFeature[] } {
  const features: MarkerFeature[] = [];
  if (selectedClimbIdx == null || !userClimbs[selectedClimbIdx]) {
    return { type: 'FeatureCollection', features };
  }

  const selected = userClimbs[selectedClimbIdx];
  const gid = selected.groupId;

  if (gid != null) {
    const members = userClimbs.filter(c => c.groupId === gid);
    for (const m of members) {
      const isSelf = (m === selected);
      features.push({
        type: 'Feature',
        geometry: { type: 'Point', coordinates: latLonAtDist(m.startKm * 1000) },
        properties: { kind: 'valley', role: isSelf ? 'selected' : 'member' },
      });
      features.push({
        type: 'Feature',
        geometry: { type: 'Point', coordinates: latLonAtDist(m.endKm * 1000) },
        properties: { kind: 'summit', role: isSelf ? 'selected' : 'member' },
      });
    }
  } else {
    features.push({
      type: 'Feature',
      geometry: { type: 'Point', coordinates: latLonAtDist(selected.startKm * 1000) },
      properties: { kind: 'valley', role: 'selected' },
    });
    features.push({
      type: 'Feature',
      geometry: { type: 'Point', coordinates: latLonAtDist(selected.endKm * 1000) },
      properties: { kind: 'summit', role: 'selected' },
    });
  }

  return { type: 'FeatureCollection', features };
}
```

### 6.16 Zoom helpers

```typescript
// src/utils/zoomHelpers.ts

export const MAP_CLIMB_MARGIN = 0.20;
export const MAP_MIN_SPAN_DEG = 0.0014;

export function computeBounds(
  startM: number,
  endM: number,
  latLonAtDist: (dist: number) => [number, number],
): [[number, number], [number, number]] {
  const startLL = latLonAtDist(startM);
  const endLL = latLonAtDist(endM);

  let minLng = Math.min(startLL[0], endLL[0]);
  let maxLng = Math.max(startLL[0], endLL[0]);
  let minLat = Math.min(startLL[1], endLL[1]);
  let maxLat = Math.max(startLL[1], endLL[1]);

  const cLng = (minLng + maxLng) / 2;
  const cLat = (minLat + maxLat) / 2;
  if (maxLng - minLng < MAP_MIN_SPAN_DEG) {
    minLng = cLng - MAP_MIN_SPAN_DEG / 2;
    maxLng = cLng + MAP_MIN_SPAN_DEG / 2;
  }
  if (maxLat - minLat < MAP_MIN_SPAN_DEG) {
    minLat = cLat - MAP_MIN_SPAN_DEG / 2;
    maxLat = cLat + MAP_MIN_SPAN_DEG / 2;
  }

  const dLng = maxLng - minLng;
  const dLat = maxLat - minLat;
  const padLng = dLng * MAP_CLIMB_MARGIN;
  const padLat = dLat * MAP_CLIMB_MARGIN;

  return [
    [minLng - padLng, minLat - padLat],
    [maxLng + padLng, maxLat + padLat],
  ];
}
```

### 6.17 Export JSON

```typescript
// src/utils/exportData.ts

import type { RebuildResult } from '../types/state';
import type { UserClimb } from '../types/climb';
import type { ExportData, ExportSegment, ExportClimb, ExportSlopeProfileEntry } from '../types/export';
import type { AnalyzerParameters } from '../types/parameters';
import { SLOPE_CATEGORIES } from './categorization';

function buildClimbSlopeProfile(
  dist: number[],
  eleDisplay: number[],
  startIdx: number,
  endIdx: number,
  profileStepM: number,
  catSegments: RebuildResult['catSegments'],
): ExportSlopeProfileEntry[] {
  const profile: ExportSlopeProfileEntry[] = [];
  const climbStartDist = dist[startIdx];
  const climbEndDist = dist[endIdx];
  if (climbEndDist <= climbStartDist || profileStepM <= 0) return profile;

  const relevantSegs = catSegments.filter(
    s => s.endDist > climbStartDist && s.startDist < climbEndDist,
  );

  for (const seg of relevantSegs) {
    const segStart = Math.max(seg.startDist, climbStartDist);
    const segEnd = Math.min(seg.endDist, climbEndDist);
    const segLen = segEnd - segStart;
    if (segLen <= 0) continue;

    const nChunks = Math.max(1, Math.ceil(segLen / profileStepM));
    const chunkLen = segLen / nChunks;

    for (let c = 0; c < nChunks; c++) {
      const chunkStartDist = segStart + c * chunkLen;
      const chunkEndDist = (c === nChunks - 1) ? segEnd : segStart + (c + 1) * chunkLen;

      const iStart = findIdxAtDist(dist, chunkStartDist);
      const iEnd = findIdxAtDist(dist, chunkEndDist);
      if (iEnd <= iStart) continue;

      let dPlus = 0;
      for (let k = iStart + 1; k <= iEnd; k++) {
        const d = eleDisplay[k] - eleDisplay[k - 1];
        if (d > 0) dPlus += d;
      }
      const length = dist[iEnd] - dist[iStart];
      const avgSlope = length > 0 ? (dPlus / length) * 100 : 0;

      profile.push({
        kmStart: +(dist[iStart] / 1000).toFixed(4),
        kmEnd: +(dist[iEnd] / 1000).toFixed(4),
        eleStartM: +eleDisplay[iStart].toFixed(1),
        eleEndM: +eleDisplay[iEnd].toFixed(1),
        avgSlopePct: +avgSlope.toFixed(2),
        categoryId: seg.key,
      });
    }
  }
  return profile;
}

function findIdxAtDist(dist: number[], targetM: number): number {
  let lo = 0, hi = dist.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (dist[mid] < targetM) lo = mid + 1; else hi = mid;
  }
  return lo;
}

export function buildExportData(
  fileName: string,
  stepM: number,
  totalDistRawM: number,
  params: AnalyzerParameters,
  data: RebuildResult,
): ExportData {
  const { dist, eleDisplay, catSegments, climbs } = data;
  const totalDistSampledM = dist[dist.length - 1];
  const trailingGapM = Math.max(0, Math.round(totalDistRawM - totalDistSampledM));
  const profileStepM = params.exportProfileStepM;

  const segments: ExportSegment[] = catSegments.map((s, i) => ({
    index: i,
    startKm: +(s.startDist / 1000).toFixed(4),
    endKm: +(s.endDist / 1000).toFixed(4),
    startProgress: +(s.startDist / totalDistRawM).toFixed(5),
    endProgress: +(s.endDist / totalDistRawM).toFixed(5),
    categoryId: s.key,
    avgSlopePct: +s.avgSlope.toFixed(2),
    dPlusM: Math.round(s.dPlus),
  }));

  const climbFeatures: ExportClimb[] = climbs.filter(c => !c.invalid).map((c, i) => {
    const slopeProfile = buildClimbSlopeProfile(
      dist, eleDisplay, c.startIdx, c.endIdx, profileStepM, catSegments,
    );
    return {
      index: i,
      groupId: c.groupId,
      subIndex: c.subIndex,
      startKm: +(c.startDist / 1000).toFixed(4),
      endKm: +(c.endDist / 1000).toFixed(4),
      startProgress: +(c.startDist / totalDistRawM).toFixed(5),
      endProgress: +(c.endDist / totalDistRawM).toFixed(5),
      dPlusM: Math.round(c.dPlus),
      avgSlopePct: +c.avgSlope.toFixed(2),
      altStartM: +c.altStart.toFixed(1),
      altEndM: +c.altEnd.toFixed(1),
      mergedCount: c.mergedCount || 0,
      fallback: !!c.fallback,
      slopeProfile,
      slopeProfileStepM: profileStepM,
    };
  });

  const legendCategories = SLOPE_CATEGORIES.map(c => ({
    id: c.id,
    label: c.label,
    minPct: isFinite(c.min) ? c.min : null,
    maxPct: isFinite(c.max) ? c.max : null,
  }));

  return {
    metadata: {
      file: fileName || 'trace.gpx',
      exportedAt: new Date().toISOString(),
      version: '3.9.2',
      totalDistanceKm: +(totalDistRawM / 1000).toFixed(3),
      totalDistanceSampledKm: +(totalDistSampledM / 1000).toFixed(3),
      trailingGapM,
      stepM,
      totalPoints: dist.length,
      segmentsCount: segments.length,
      climbsCount: climbFeatures.length,
      profileStepM,
    },
    legend: {
      note: "Bornes hautes exclusives. startProgress/endProgress = fraction 0–1 de la longueur réelle.",
      fenetreCatM: params.fenetreCatM,
      longueurMin: params.longueurMin,
      categories: legendCategories,
    },
    segments,
    climbs: climbFeatures,
  };
}
```

---

## 7. Composables Vue 3

### 7.1 `useGpxAnalyzer`

```typescript
// src/composables/useGpxAnalyzer.ts
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useGpxStore } from '@/stores/gpxStore';
import { useClimbsStore } from '@/stores/climbsStore';
import { useParametersStore } from '@/stores/parametersStore';
import type { AnalyzerResult } from '@/types/analysis';
import type { UserClimb } from '@/types/climb';

export function useGpxAnalyzer() {
  const gpxStore = useGpxStore();
  const climbsStore = useClimbsStore();
  const paramsStore = useParametersStore();

  const isLoading = ref(false);
  const error = ref<string | null>(null);

  async function loadAndAnalyze(filePath: string): Promise<void> {
    isLoading.value = true;
    error.value = null;
    try {
      const content: string = await invoke('read_gpx_file', { path: filePath });
      const result = await invoke<AnalyzerResult>('analyze_gpx', {
        content,
        parameters: paramsStore.parameters,
      });

      gpxStore.setTrack(result.track);
      gpxStore.setRawData(result.distRaw, result.eleRawOrig, result.rawStats, result.track.points);
      gpxStore.setResampledData(result.dist, result.ele, result.stepM);
      climbsStore.setAutoClimbs(result.autoClimbs);
      climbsStore.resetUserClimbs(result.autoClimbs.map(a => ({
        startKm: a.startKm,
        endKm: a.endKm,
        autoStartKm: a.startKm,
        autoEndKm: a.endKm,
        refineM: a.refineM,
        candidateLen: a.candidateLen,
        mergedCount: a.mergedCount,
        mergedParts: a.mergedParts,
        fallback: a.fallback,
        groupId: null,
        subIndex: null,
      } as UserClimb)));
      climbsStore.resetGroups();
      climbsStore.rebuild();
    } catch (e: any) {
      error.value = e.message ?? 'Erreur inconnue';
    } finally {
      isLoading.value = false;
    }
  }

  return { isLoading, error, loadAndAnalyze };
}
```

### 7.2 `useClimbGroups`

```typescript
// src/composables/useClimbGroups.ts

import { useClimbsStore } from '@/stores/climbsStore';

/**
 * Expose les actions de gestion des groupes de montées.
 */
export function useClimbGroups() {
  const store = useClimbsStore();

  function splitClimb(idx: number): void {
    store.splitClimb(idx);
  }

  function refuseGroup(gid: number): void {
    store.refuseGroup(gid);
  }

  function canSplit(idx: number): boolean {
    const c = store.userClimbs[idx];
    return !!c?.mergedParts && c.mergedParts.length >= 2;
  }

  function canRefuse(gid: number | null): boolean {
    if (gid == null) return false;
    return store.userClimbs.filter(c => c.groupId === gid).length >= 2;
  }

  return { splitClimb, refuseGroup, canSplit, canRefuse };
}
```

### 7.3 `useManualClimb`

```typescript
// src/composables/useManualClimb.ts

import { ref } from 'vue';
import { useClimbsStore } from '@/stores/climbsStore';
import { useProfileCanvas } from './useProfileCanvas';

const MANUAL_MIN_LEN_KM = 0.05;

export function useManualClimb() {
  const store = useClimbsStore();
  const profile = useProfileCanvas();

  const pendingNewClimb = ref<{ valleyKm: number; summitKm: number } | null>(null);
  const newClimbDragging = ref(false);

  /**
   * Démarre la création par double-clic. Refuse si la vallée tombe
   * dans une montée existante.
   */
  function startCreation(km: number): boolean {
    if (!store.isFreeKm(km)) {
      store.flashInvalidPosition(km);
      return false;
    }
    pendingNewClimb.value = { valleyKm: km, summitKm: km };
    profile.requestRedraw();
    return true;
  }

  /**
   * Met à jour le sommet pendant le drag.
   */
  function updateSummit(km: number): void {
    if (!pendingNewClimb.value) return;
    pendingNewClimb.value.summitKm = Math.max(
      pendingNewClimb.value.valleyKm + MANUAL_MIN_LEN_KM,
      km,
    );
    profile.requestRedraw();
  }

  /**
   * Valide ou annule la création au relâchement.
   */
  function finish(): void {
    if (!pendingNewClimb.value) return;
    const { valleyKm, summitKm } = pendingNewClimb.value;
    const pending = pendingNewClimb.value;
    pendingNewClimb.value = null;
    newClimbDragging.value = false;

    if (summitKm > valleyKm + MANUAL_MIN_LEN_KM && store.isFreeRange(valleyKm, summitKm)) {
      store.addManualClimb(valleyKm, summitKm);
    } else {
      profile.requestRedraw();
    }
  }

  /**
   * Annule la création (Échap).
   */
  function cancel(): void {
    if (!pendingNewClimb.value) return;
    pendingNewClimb.value = null;
    newClimbDragging.value = false;
    profile.requestRedraw();
  }

  return {
    pendingNewClimb,
    newClimbDragging,
    startCreation,
    updateSummit,
    finish,
    cancel,
  };
}
```

### 7.4 `useProfileCanvas`

```typescript
// src/composables/useProfileCanvas.ts

import { ref, onMounted, onBeforeUnmount, watch } from 'vue';
import { useGpxStore } from '@/stores/gpxStore';
import { useClimbsStore } from '@/stores/climbsStore';
import { CATEGORY_BY_ID } from '@/utils/categorization';
import { groupColor } from '@/utils/groupHelpers';

export const PROFILE_SCALE_X_FULL = 50;
export const PROFILE_SCALE_X_CLIMB = 10;
export const PROFILE_SCALE_Y = 10;
export const PROFILE_MIN_HEIGHT = 110;
export const PROFILE_HANDLE_PAD = 200;

export function useProfileCanvas() {
  const gpxStore = useGpxStore();
  const climbsStore = useClimbsStore();

  const canvasRef = ref<HTMLCanvasElement | null>(null);
  const scrollRef = ref<HTMLDivElement | null>(null);

  let viewState: {
    viewStartM: number;
    viewEndM: number;
    scaleXActual: number;
    canvasWidth: number;
    canvasHeight: number;
    mode: 'full' | 'climb';
  } | null = null;

  function requestRedraw(): void {
    if (!canvasRef.value) return;
    draw();
  }

  function draw(): void {
    const canvas = canvasRef.value;
    const scroll = scrollRef.value;
    if (!canvas || !scroll) return;

    const data = climbsStore.__lastData;
    const dist = gpxStore.dist;
    if (!data || !dist) {
      drawEmpty(canvas);
      return;
    }

    const totalDist = dist[dist.length - 1];
    let viewStartM: number, viewEndM: number, scaleX: number, showHandles: boolean;

    if (climbsStore.profileMode === 'climb' && climbsStore.selectedClimbIdx != null) {
      // Mode montée : trace COMPLÈTE à 10 m/px
      viewStartM = 0;
      viewEndM = totalDist;
      scaleX = PROFILE_SCALE_X_CLIMB;
      showHandles = true;
    } else {
      // Mode full : 50 m/px
      viewStartM = 0;
      viewEndM = totalDist;
      scaleX = PROFILE_SCALE_X_FULL;
      showHandles = false;
    }

    const viewWidthM = viewEndM - viewStartM;
    const canvasWidth = Math.max(2, Math.ceil(viewWidthM / scaleX));
    const scaleXActual = scaleX;

    const ele = data.eleDisplay;
    let emin = Infinity, emax = -Infinity;
    const iStart = findIdxAtDist(dist, viewStartM);
    const iEnd = findIdxAtDist(dist, viewEndM);
    for (let i = iStart; i <= iEnd; i++) {
      if (ele[i] < emin) emin = ele[i];
      if (ele[i] > emax) emax = ele[i];
    }
    if (!isFinite(emin)) { emin = 0; emax = 100; }
    const range = Math.max(20, emax - emin);
    const canvasHeight = Math.max(PROFILE_MIN_HEIGHT, Math.ceil(range / PROFILE_SCALE_Y) + 24);

    canvas.width = canvasWidth;
    canvas.height = canvasHeight;
    canvas.style.width = canvasWidth + 'px';
    canvas.style.height = canvasHeight + 'px';

    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const yPad = 12;
    const drawH = canvasHeight - 2 * yPad;
    const eleToY = (e: number) => yPad + (1 - (e - emin) / range) * drawH;
    const distToX = (d: number) => (d - viewStartM) / scaleXActual;

    // Fond
    ctx.fillStyle = '#fafbfc';
    ctx.fillRect(0, 0, canvasWidth, canvasHeight);

    // Fills colorés par catégorie — via catSegments
    ctx.globalAlpha = 0.55;
    for (const seg of data.catSegments) {
      if (seg.endDist < viewStartM - 100 || seg.startDist > viewEndM + 100) continue;
      const cat = CATEGORY_BY_ID[seg.key];
      if (!cat) continue;
      ctx.fillStyle = cat.color;
      ctx.beginPath();
      const iS = Math.max(0, seg.startIdx);
      const iE = Math.min(dist.length - 1, seg.endIdx);
      for (let i = iS; i <= iE; i++) {
        const x = distToX(dist[i]);
        const y = eleToY(ele[i]);
        if (i === iS) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.lineTo(distToX(dist[iE]), canvasHeight - 2);
      ctx.lineTo(distToX(dist[iS]), canvasHeight - 2);
      ctx.closePath();
      ctx.fill();
    }
    ctx.globalAlpha = 1;

    // Courbe
    ctx.beginPath();
    let started = false;
    for (let i = 0; i < dist.length; i++) {
      if (dist[i] < viewStartM - 200 || dist[i] > viewEndM + 200) continue;
      const x = distToX(dist[i]);
      const y = eleToY(ele[i]);
      if (!started) { ctx.moveTo(x, y); started = true; }
      else ctx.lineTo(x, y);
    }
    ctx.strokeStyle = '#1e293b';
    ctx.lineWidth = 1.4;
    ctx.stroke();

    // Ligne de base
    ctx.beginPath();
    ctx.moveTo(0, canvasHeight - 1.5);
    ctx.lineTo(canvasWidth, canvasHeight - 1.5);
    ctx.strokeStyle = '#cbd5e1';
    ctx.lineWidth = 1;
    ctx.stroke();

    // Marqueurs montées
    for (let i = 0; i < data.climbs.length; i++) {
      const c = data.climbs[i];
      if (c.invalid) continue;
      const isSel = i === climbsStore.selectedClimbIdx;
      const isGrouped = c.groupId != null;
      const gColor = isGrouped ? groupColor(c.groupId!) : null;

      const xV = distToX(c.startDist);
      if (xV >= -4 && xV <= canvasWidth + 4) {
        ctx.beginPath();
        ctx.moveTo(xV, 0);
        ctx.lineTo(xV, canvasHeight);
        ctx.strokeStyle = isSel ? '#16a34a' : (isGrouped ? gColor! : 'rgba(22,163,74,0.35)');
        ctx.lineWidth = isSel ? 2 : (isGrouped ? 1.6 : 1);
        ctx.setLineDash([]);
        ctx.stroke();
      }
      const xS = distToX(c.endDist);
      if (xS >= -4 && xS <= canvasWidth + 4) {
        ctx.beginPath();
        ctx.moveTo(xS, 0);
        ctx.lineTo(xS, canvasHeight);
        ctx.strokeStyle = isSel ? '#dc2626' : (isGrouped ? gColor! : 'rgba(220,38,38,0.35)');
        ctx.lineWidth = isSel ? 2 : (isGrouped ? 1.6 : 1);
        ctx.setLineDash(isGrouped ? [] : [4, 3]);
        ctx.stroke();
        ctx.setLineDash([]);
      }
    }

    // Surbrillance sélectionnée
    if (climbsStore.selectedClimbIdx != null) {
      const c = data.climbs[climbsStore.selectedClimbIdx];
      if (c && !c.invalid) {
        const xV = distToX(c.startDist);
        const xS = distToX(c.endDist);
        ctx.fillStyle = 'rgba(250, 204, 21, 0.14)';
        ctx.fillRect(xV, 0, xS - xV, canvasHeight);
      }
    }

    // Poignées
    if (showHandles && climbsStore.selectedClimbIdx != null) {
      const c = data.climbs[climbsStore.selectedClimbIdx];
      if (c && !c.invalid) {
        drawHandle(ctx, distToX(c.startDist), '#16a34a');
        drawHandle(ctx, distToX(c.endDist), '#dc2626');
      }
    }

    viewState = {
      viewStartM, viewEndM, scaleXActual,
      canvasWidth, canvasHeight,
      mode: climbsStore.profileMode,
    };

    // Scroll centré (mode montée, hors drag)
    if (climbsStore.profileMode === 'climb' && !climbsStore.__profileDragging && climbsStore.selectedClimbIdx != null) {
      const c = data.climbs[climbsStore.selectedClimbIdx];
      if (c && !c.invalid) {
        const centerM = (c.startDist + c.endDist) / 2;
        const centerPx = (centerM - viewStartM) / scaleXActual;
        const targetScroll = Math.max(0, centerPx - scroll.clientWidth / 2);
        requestAnimationFrame(() => { scroll.scrollLeft = targetScroll; });
      }
    } else if (climbsStore.profileMode === 'full') {
      requestAnimationFrame(() => { scroll.scrollLeft = 0; });
    }
  }

  function drawEmpty(canvas: HTMLCanvasElement): void {
    const scroll = scrollRef.value;
    canvas.width = scroll?.clientWidth || 800;
    canvas.height = PROFILE_MIN_HEIGHT;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.fillStyle = '#f8fafc';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = '#cbd5e1';
    ctx.font = '14px -apple-system, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('Aucune trace chargée', canvas.width / 2, canvas.height / 2 + 4);
  }

  function drawHandle(ctx: CanvasRenderingContext2D, px: number, color: string): void {
    const size = 10;
    const y = 6;
    ctx.beginPath();
    ctx.moveTo(px, y + size * 1.6);
    ctx.lineTo(px - size, y);
    ctx.lineTo(px + size, y);
    ctx.closePath();
    ctx.fillStyle = color;
    ctx.fill();
    ctx.strokeStyle = 'white';
    ctx.lineWidth = 2;
    ctx.stroke();
  }

  function findIdxAtDist(dist: number[], targetM: number): number {
    let lo = 0, hi = dist.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (dist[mid] < targetM) lo = mid + 1; else hi = mid;
    }
    return lo;
  }

  // Re-render sur changement de sélection ou de données
  watch(() => [climbsStore.__lastData, climbsStore.selectedClimbIdx, climbsStore.profileMode], () => {
    requestRedraw();
  });

  onMounted(() => { requestRedraw(); });

  return { canvasRef, scrollRef, requestRedraw, draw, getViewState: () => viewState };
}
```

### 7.5 `useMapbox`

```typescript
// src/composables/useMapbox.ts

import { ref } from 'vue';
import { useGpxStore } from '@/stores/gpxStore';
import { useClimbsStore } from '@/stores/climbsStore';
import { useMapStore } from '@/stores/mapStore';
import { buildTraceFeatures } from '@/utils/buildTraceFeatures';
import { buildMapMarkers } from '@/utils/buildMapMarkers';
import { computeBounds } from '@/utils/zoomHelpers';

export const MAP_BASEMAPS = {
  standard: 'mapbox://styles/mapbox/standard',
  satellite: 'mapbox://styles/mapbox/satellite-streets-v12',
};

export const MAP_TERRAIN_EXAGGERATION = 1;
export const CONTOUR_BASE_COLOR = '#7a5733';
export const CONTOUR_INDEX_COLOR = '#5e3f1d';

export function useMapbox() {
  const gpxStore = useGpxStore();
  const climbsStore = useClimbsStore();
  const mapStore = useMapStore();

  async function loadMapboxGL(): Promise<void> {
    if ((window as any).mapboxgl) return;
    if (mapStore.loading) {
      await new Promise<void>((resolve, reject) => {
        const check = setInterval(() => {
          if (!mapStore.loading) {
            clearInterval(check);
            if ((window as any).mapboxgl) resolve();
            else reject(new Error('Échec chargement Mapbox'));
          }
        }, 50);
      });
      return;
    }
    mapStore.loading = true;
    try {
      await new Promise<void>((resolve, reject) => {
        if (!document.querySelector('link[data-mapbox]')) {
          const link = document.createElement('link');
          link.rel = 'stylesheet';
          link.href = 'https://api.mapbox.com/mapbox-gl-js/v3.7.0/mapbox-gl.css';
          (link as any).dataset.mapbox = '1';
          document.head.appendChild(link);
        }
        const script = document.createElement('script');
        script.src = 'https://api.mapbox.com/mapbox-gl-js/v3.7.0/mapbox-gl.js';
        script.onload = () => resolve();
        script.onerror = () => reject(new Error('Impossible de charger Mapbox GL JS'));
        document.head.appendChild(script);
      });
    } finally {
      mapStore.loading = false;
    }
  }

  async function initMapboxMap(token: string): Promise<void> {
    await loadMapboxGL();
    const mapboxgl = (window as any).mapboxgl;
    if (mapStore.map) { mapStore.map.remove(); mapStore.map = null; }
    mapboxgl.accessToken = token;

    const map = new mapboxgl.Map({
      container: 'map',
      style: MAP_BASEMAPS[mapStore.basemap],
      center: [2.5, 46.5],
      zoom: 5, pitch: 60, bearing: 0,
      antialias: true,
    });
    mapStore.map = map;
    mapStore.tokenErrorShown = false;
    map.addControl(new mapboxgl.NavigationControl({ visualizePitch: true }), 'top-left');

    map.on('error', (e: any) => {
      const msg = e?.error?.message || '';
      if (!mapStore.tokenErrorShown && /token|401|403|unauthorized/i.test(msg)) {
        mapStore.tokenErrorShown = true;
      }
    });

    return new Promise<void>((resolve) => {
      map.on('load', () => {
        setupMapLayers(map);
        mapStore.ready = true;
        refreshMapAll();
        if (gpxStore.rawPoints?.length) fitMapToTrace(0);
        resolve();
      });
    });
  }

  function setupMapLayers(map: any): void {
    // 1. Terrain
    if (!map.getSource('mapbox-dem')) {
      map.addSource('mapbox-dem', {
        type: 'raster-dem',
        url: 'mapbox://mapbox.mapbox-terrain-dem-v1',
        tileSize: 512, maxzoom: 14,
      });
      map.setTerrain({ source: 'mapbox-dem', exaggeration: MAP_TERRAIN_EXAGGERATION });
    }

    // 2. Brume
    try { map.setFog({ 'range': [-0.5, 2], 'color': '#dbeafe', 'horizon-blend': 0.1 }); } catch (e) {}

    // 3. Trace (DOIT exister avant les contours)
    if (!map.getSource('trace-line')) {
      map.addSource('trace-line', { type: 'geojson', data: { type: 'FeatureCollection', features: [] } });
      map.addLayer({
        id: 'trace-line-layer',
        type: 'line',
        source: 'trace-line',
        layout: { 'line-cap': 'round', 'line-join': 'round' },
        paint: { 'line-width': 6, 'line-color': ['get', 'color'] },
      });
    }

    // 4. Marqueurs
    if (!map.getSource('climb-markers')) {
      map.addSource('climb-markers', { type: 'geojson', data: { type: 'FeatureCollection', features: [] } });
      map.addLayer({
        id: 'climb-markers-circle',
        type: 'circle',
        source: 'climb-markers',
        paint: {
          'circle-radius': ['match', ['get', 'role'], 'selected', 10, 8],
          'circle-color': ['match', ['get', 'kind'], 'valley', '#16a34a', 'summit', '#dc2626', '#888'],
          'circle-stroke-width': ['match', ['get', 'role'], 'selected', 4, 2.5],
          'circle-stroke-color': '#fff',
        },
      });
    }

    // 5. Contours (sous la trace)
    if (!map.getSource('mapbox-terrain-contours')) {
      map.addSource('mapbox-terrain-contours', {
        type: 'vector',
        url: 'mapbox://mapbox.mapbox-terrain-v2',
      });

      map.addLayer({
        id: 'contour-lines',
        type: 'line',
        source: 'mapbox-terrain-contours',
        'source-layer': 'contour',
        layout: { 'line-join': 'round', 'line-cap': 'round', 'visibility': 'none' },
        paint: {
          'line-color': CONTOUR_BASE_COLOR,
          'line-width': ['interpolate', ['linear'], ['zoom'], 10, 0.3, 13, 0.6, 16, 1.0],
          'line-opacity': ['interpolate', ['linear'], ['zoom'], 10, 0.15, 14, 0.35, 17, 0.50],
        },
      }, 'trace-line-layer');

      map.addLayer({
        id: 'contour-index',
        type: 'line',
        source: 'mapbox-terrain-contours',
        'source-layer': 'contour',
        filter: ['>=', ['coalesce', ['get', 'index'], 0], 5],
        layout: { 'line-join': 'round', 'line-cap': 'round', 'visibility': 'none' },
        paint: {
          'line-color': CONTOUR_INDEX_COLOR,
          'line-width': ['interpolate', ['linear'], ['zoom'], 10, 0.7, 13, 1.2, 16, 2.0],
          'line-opacity': ['interpolate', ['linear'], ['zoom'], 10, 0.25, 14, 0.50, 17, 0.65],
        },
      }, 'trace-line-layer');
    }
  }

  function updateContourVisibility(show: boolean): void {
    const map = mapStore.map;
    if (!map || !mapStore.ready) return;
    const visible = show ? 'visible' : 'none';
    for (const layerId of ['contour-lines', 'contour-index']) {
      if (map.getLayer(layerId)) {
        map.setLayoutProperty(layerId, 'visibility', visible);
      }
    }
  }

  function updateMapTrace(): void {
    const map = mapStore.map;
    if (!map || !mapStore.ready) return;
    const src = map.getSource('trace-line');
    if (!src) return;
    const data = climbsStore.__lastData;
    if (!data) { src.setData({ type: 'FeatureCollection', features: [] }); return; }
    src.setData(buildTraceFeatures(gpxStore.rawPoints, gpxStore.distRaw, data.catSegments));
  }

  function updateMapMarkers(): void {
    const map = mapStore.map;
    if (!map || !mapStore.ready) return;
    const src = map.getSource('climb-markers');
    if (!src) return;
    src.setData(buildMapMarkers(climbsStore.selectedClimbIdx, climbsStore.userClimbs, gpxStore.latLonAtDist));
  }

  function refreshMapAll(): void {
    updateMapTrace();
    updateMapMarkers();
  }

  function zoomMapToClimb(climb: any, duration = 800): void {
    const map = mapStore.map;
    if (!map || !mapStore.ready || !climb) return;

    if (climb.groupId != null) {
      return zoomMapToGroup(climb.groupId, duration);
    }

    const startM = (climb.startKm != null) ? climb.startKm * 1000 : climb.startDist;
    const endM = (climb.endKm != null) ? climb.endKm * 1000 : climb.endDist;
    if (!isFinite(startM) || !isFinite(endM) || endM <= startM) return;

    const bounds = computeBounds(startM, endM, gpxStore.latLonAtDist);
    map.fitBounds(bounds, { padding: 40, duration, pitch: 60, bearing: map.getBearing() });
  }

  function zoomMapToGroup(gid: number, duration = 800): void {
    const map = mapStore.map;
    if (!map || !mapStore.ready) return;
    const members = climbsStore.userClimbs.filter(c => c.groupId === gid);
    if (members.length === 0) return;
    let minKm = Infinity, maxKm = -Infinity;
    for (const m of members) {
      if (m.startKm < minKm) minKm = m.startKm;
      if (m.endKm > maxKm) maxKm = m.endKm;
    }
    const bounds = computeBounds(minKm * 1000, maxKm * 1000, gpxStore.latLonAtDist);
    map.fitBounds(bounds, { padding: 40, duration, pitch: 60, bearing: map.getBearing() });
  }

  function fitMapToTrace(duration = 800): void {
    const map = mapStore.map;
    if (!map || !mapStore.ready || !gpxStore.rawPoints?.length) return;
    let minLng = Infinity, maxLng = -Infinity, minLat = Infinity, maxLat = -Infinity;
    for (const p of gpxStore.rawPoints) {
      if (p.lon < minLng) minLng = p.lon;
      if (p.lon > maxLng) maxLng = p.lon;
      if (p.lat < minLat) minLat = p.lat;
      if (p.lat > maxLat) maxLat = p.lat;
    }
    map.fitBounds([[minLng, minLat], [maxLng, maxLat]], { padding: 40, duration, pitch: 60 });
  }

  function setMapBasemap(kind: 'standard' | 'satellite'): void {
    mapStore.basemap = kind;
    const map = mapStore.map;
    if (!map) return;
    mapStore.ready = false;
    map.setStyle(MAP_BASEMAPS[kind]);
    map.once('style.load', () => {
      setupMapLayers(map);
      mapStore.ready = true;
      refreshMapAll();
      if (climbsStore.selectedClimbIdx != null) {
        const climb = climbsStore.userClimbs[climbsStore.selectedClimbIdx];
        if (climb) zoomMapToClimb(climb, 0);
      }
    });
  }

  return {
    loadMapboxGL,
    initMapboxMap,
    setupMapLayers,
    updateContourVisibility,
    updateMapTrace,
    updateMapMarkers,
    refreshMapAll,
    zoomMapToClimb,
    zoomMapToGroup,
    fitMapToTrace,
    setMapBasemap,
  };
}
```

---

## 8. Composants Vuetify

### 8.1 `GpxAnalyzer.vue` (root)

```vue
<!-- src/components/gpx-analyzer/GpxAnalyzer.vue -->
<template>
  <div class="gpx-analyzer">
    <AppToolbar
      :file-name="gpxStore.fileName"
      :raw-count="gpxStore.rawStats?.n ?? 0"
      :resampled-count="gpxStore.dist?.length ?? 0"
      @file-selected="onFileSelected"
      @toggle-debug="showDebug = !showDebug"
      @toggle-params="showParams = !showParams"
    />
    <div class="main-layout">
      <ClimbsPanel
        :climbs="climbsStore.__lastData?.climbs ?? []"
        :selected-idx="climbsStore.selectedClimbIdx"
        @select="onClimbSelected"
        @split="onSplit"
        @refuse="onRefuse"
        @delete="onDelete"
      />
      <CenterPanel
        :climbs="climbsStore.__lastData?.climbs ?? []"
        :selected-idx="climbsStore.selectedClimbIdx"
        @close-climb="onCloseClimb"
      />
      <ParamsPanel
        v-show="showParams"
        :parameters="paramsStore.parameters"
        @update:parameters="onParamsUpdate"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useGpxStore } from '@/stores/gpxStore';
import { useClimbsStore } from '@/stores/climbsStore';
import { useParametersStore } from '@/stores/parametersStore';
import { useGpxAnalyzer } from '@/composables/useGpxAnalyzer';
import AppToolbar from './AppToolbar.vue';
import ClimbsPanel from './ClimbsPanel.vue';
import CenterPanel from './CenterPanel.vue';
import ParamsPanel from './ParamsPanel.vue';

const gpxStore = useGpxStore();
const climbsStore = useClimbsStore();
const paramsStore = useParametersStore();
const { loadAndAnalyze } = useGpxAnalyzer();

const showDebug = ref(false);
const showParams = ref(true);

function onFileSelected(file: File | string): void {
  if (typeof file === 'string') loadAndAnalyze(file);
  else if ((file as any).path) loadAndAnalyze((file as any).path);
}

function onClimbSelected(idx: number): void {
  climbsStore.selectClimb(idx);
}

function onCloseClimb(): void {
  climbsStore.deselectClimb();
}

function onSplit(idx: number): void {
  climbsStore.splitClimb(idx);
}

function onRefuse(gid: number): void {
  climbsStore.refuseGroup(gid);
}

function onDelete(idx: number): void {
  climbsStore.deleteClimb(idx);
}

function onParamsUpdate(p: any): void {
  paramsStore.setParameters(p);
  climbsStore.recomputeAuto();
}
</script>

<style scoped>
.gpx-analyzer {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  background: #f1f5f9;
}
.main-layout {
  display: flex;
  flex: 1;
  overflow: hidden;
}
</style>
```

### 8.2 `ClimbCard.vue`

```vue
<!-- src/components/gpx-analyzer/ClimbCard.vue -->
<template>
  <div
    class="climb-card"
    :class="{ selected, grouped: climb.groupId != null }"
    :style="cardStyle"
    @click="$emit('select')"
  >
    <button class="del-btn" @click.stop="$emit('delete')" title="Supprimer">✕</button>

    <div class="climb-card-header">
      <span class="climb-num" :style="numStyle">{{ label }}</span>
      <span class="climb-km">km {{ (climb.startDist / 1000).toFixed(3) }}</span>
      <span class="climb-badges">
        <span v-if="climb.mergedCount > 0" class="badge merged">⊕ {{ climb.mergedCount }}</span>
        <span v-if="climb.fallback" class="badge fallback">⤺</span>
        <span v-if="climb.groupId != null" class="badge grouped">🔗 {{ climb.groupId }}</span>
      </span>
      <button
        v-if="canSplit"
        class="split-btn"
        @click.stop="$emit('split')"
      >
        ✂ Séparer
      </button>
      <button
        v-if="climb.groupId != null"
        class="refuse-btn"
        @click.stop="$emit('refuse', climb.groupId)"
      >
        🔗 Refusionner
      </button>
    </div>

    <div class="climb-card-body">
      <div class="metric">
        <span class="label">Long.</span>
        <span class="value">{{ (climb.length / 1000).toFixed(2) }} km</span>
      </div>
      <div class="metric">
        <span class="label">D+</span>
        <span class="value">{{ climb.dPlus.toFixed(0) }} m</span>
      </div>
      <span class="slope-pill" :style="{ background: colorForSlope(climb.avgSlope) }">
        {{ climb.avgSlope.toFixed(1) }} %
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { colorForSlope } from '@/utils/categorization';
import { groupColor } from '@/utils/groupHelpers';
import type { Climb } from '@/types/climb';

const props = defineProps<{
  climb: Climb;
  label: string;
  selected: boolean;
}>();

defineEmits<{
  (e: 'select'): void;
  (e: 'split'): void;
  (e: 'refuse', gid: number): void;
  (e: 'delete'): void;
}>();

const canSplit = computed(() =>
  Array.isArray(props.climb.ref.mergedParts) && props.climb.ref.mergedParts.length >= 2
);

const cardStyle = computed(() => {
  if (props.climb.groupId == null) return {};
  return { borderLeft: `4px solid ${groupColor(props.climb.groupId)}` };
});

const numStyle = computed(() => {
  if (props.climb.groupId == null || props.selected) return {};
  return { background: groupColor(props.climb.groupId), color: 'white' };
});
</script>
```

### 8.3 `DiagnosticPanel.vue`

```vue
<!-- src/components/gpx-analyzer/DiagnosticPanel.vue -->
<template>
  <v-card class="diagnostic-panel pa-3 mb-3" variant="outlined">
    <div class="diag-header d-flex align-center mb-2">
      <span class="mr-2">🔍</span>
      <strong>Diagnostic de configuration</strong>
      <v-spacer />
      <v-chip :color="chipColor" size="small" label>
        {{ statusText }}
      </v-chip>
    </div>

    <div class="diag-grid">
      <div
        v-for="(entry, key) in diag"
        :key="key"
        class="diag-row"
        :title="entry.msg"
      >
        <span class="status-dot" :style="{ color: dotColor(entry.status) }">●</span>
        <span class="diag-label">{{ entry.label }}</span>
        <span class="diag-value">{{ entry.effective }}</span>
      </div>
    </div>

    <div v-if="warnings.length" class="diag-warnings mt-3">
      <div
        v-for="(w, i) in warnings"
        :key="i"
        class="diag-warning"
        :class="w.level"
      >
        <span v-html="w.msg" />
      </div>
    </div>
    <div v-else class="diag-empty text-caption text-grey mt-2">
      Aucun problème détecté. Configuration cohérente.
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { AnalyzerParameters } from '@/types/parameters';
import { computeDiagnostics } from '@/utils/diagnostics';

const props = defineProps<{ parameters: AnalyzerParameters }>();

const result = computed(() => computeDiagnostics(props.parameters));
const diag = computed(() => result.value.diag);
const warnings = computed(() => result.value.warnings);

const globalStatus = computed(() => {
  if (warnings.value.some(w => w.level === 'red')) return 'red';
  if (warnings.value.some(w => w.level === 'orange')) return 'orange';
  return 'green';
});

const chipColor = computed(() => ({
  green: 'success',
  orange: 'warning',
  red: 'error',
}[globalStatus.value]));

const statusText = computed(() => {
  const w = warnings.value;
  if (globalStatus.value === 'red') return `🔴 ${w.filter(x => x.level === 'red').length} incohérence(s)`;
  if (globalStatus.value === 'orange') return `🟠 ${w.length} avertissement(s)`;
  return '🟢 Cohérent';
});

function dotColor(status: string): string {
  return ({ green: '#16a34a', orange: '#f59e0b', red: '#dc2626', grey: '#94a3b8' } as any)[status] ?? '#94a3b8';
}
</script>
```

### 8.4 `ParametersForm.vue` (extrait — section Mapbox + params clés)

```vue
<!-- src/components/gpx-analyzer/ParametersForm.vue (extrait) -->
<template>
  <v-card class="pa-4">
    <v-card-title class="text-h6">Paramètres</v-card-title>

    <!-- Section Mapbox -->
    <div class="section-title">Mapbox</div>
    <v-text-field
      v-model="mapboxToken"
      label="Token public (pk.*)"
      density="compact"
      :append-inner-icon="'mdi-check'"
      @click:append-inner="onInitMap"
    />
    <v-switch
      v-model="showContours"
      label="Courbes de niveau"
      density="compact"
      hide-details
      @update:model-value="onToggleContours"
    />
    <div class="text-caption text-grey">{{ mapStatus }}</div>

    <DiagnosticPanel :parameters="params" class="mt-3" />

    <!-- Préparation -->
    <div class="section-title">Préparation</div>
    <v-text-field v-model.number="params.stepM" label="Pas de rééchantillonnage" type="number" suffix="m" density="compact" />
    <v-text-field v-model.number="params.despikeWindowM" label="Anti-pic : fenêtre" type="number" suffix="m" density="compact" />
    <v-text-field v-model.number="params.despikeThresholdM" label="Anti-pic : seuil" type="number" suffix="m" density="compact" />
    <v-text-field v-model.number="params.plateauRun" label="Correction plateaux" type="number" suffix="pts" density="compact" />

    <!-- Lissage -->
    <div class="section-title">Lissage</div>
    <v-text-field v-model.number="params.fenetreAffichageM" label="Lissage altitude" type="number" suffix="m" density="compact" />
    <v-text-field v-model.number="params.fenetrePenteM" label="Lissage pente" type="number" suffix="m" density="compact" />
    <v-text-field v-model.number="params.fenetreCatM" label="Fenêtre de calcul pente" type="number" suffix="m" density="compact" />
    <v-text-field v-model.number="params.medianWindowM" label="Filtre médian" type="number" suffix="m" density="compact" />
    <v-text-field v-model.number="params.gaussianSigmaM" label="Lissage gaussien" type="number" suffix="m" density="compact" />

    <!-- Détection -->
    <div class="section-title">Détection</div>
    <v-text-field v-model.number="params.seuilMontee" label="Seuil détection" type="number" suffix="%" density="compact" />
    <v-text-field v-model.number="params.longueurMin" label="Longueur min. pente" type="number" suffix="m" density="compact" />
    <v-text-field v-model.number="params.gapMinM" label="Écart min. entre montées" type="number" suffix="m" density="compact" />
    <v-text-field v-model.number="params.refinePct" label="Recherche extrema" type="number" suffix="%" density="compact" />

    <!-- Export & Debug -->
    <div class="section-title">Export & Debug</div>
    <v-text-field v-model.number="params.exportProfileStepM" label="Pas du profil exporté" type="number" suffix="m" density="compact" />
    <v-text-field v-model.number="params.segmentLogM" label="Segments debug" type="number" suffix="m" density="compact" />

    <!-- Options -->
    <v-switch v-model="params.smoothExtra" label="Lissage renforcé (double SG)" density="compact" hide-details />
    <v-switch v-model="params.segmentSmooth" label="Lissage des pentes de segment" density="compact" hide-details />
  </v-card>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import DiagnosticPanel from './DiagnosticPanel.vue';
import { useParametersStore } from '@/stores/parametersStore';
import { useMapbox } from '@/composables/useMapbox';
import { validateParameters } from '@/utils/parameterValidation';

const props = defineProps<{ parameters: any }>();
const emit = defineEmits<{ (e: 'update:parameters', p: any): void }>();

const paramsStore = useParametersStore();
const mapbox = useMapbox();

const params = reactive({ ...paramsStore.parameters });
const mapboxToken = ref(localStorage.getItem('mapboxToken') || '');
const showContours = ref(localStorage.getItem('showContours') === 'true');
const mapStatus = ref('');

watch(params, () => {
  const validated = validateParameters({ ...params });
  emit('update:parameters', validated);
}, { deep: true });

async function onInitMap(): Promise<void> {
  const token = mapboxToken.value.trim();
  if (!token) { mapStatus.value = 'Token requis'; return; }
  localStorage.setItem('mapboxToken', token);
  mapStatus.value = 'Chargement…';
  try {
    await mapbox.initMapboxMap(token);
    mapStatus.value = 'Carte OK';
  } catch (err: any) {
    mapStatus.value = 'Erreur : ' + err.message;
  }
}

function onToggleContours(v: boolean | null): void {
  localStorage.setItem('showContours', String(!!v));
  mapbox.updateContourVisibility(!!v);
}
</script>
```

---

## 9. Communication avec le backend Node.js

### 9.1 Sidecar Node.js — pipeline complet v3.9.2

```javascript
// src-tauri/sidecar/analyzer.js
const { XMLParser } = require('fast-xml-parser');

const REFINE_MIN_M = 1500;
const REFINE_MAX_M = 8000;
const SLOPE_DISPLAY_MAX_PCT = 25;
const PLATEAU_TOLERANCE_M = 1.0;
const RAW_DOUBLON_MIN_M = 1.0;
const RESAMPLE_AVG_MIN_M = 75;

// ---------- Parse GPX ----------
function parseGpx(xmlContent) {
  const parser = new XMLParser({ ignoreAttributes: false, attributeNamePrefix: '@_' });
  const doc = parser.parse(xmlContent);
  const trkpts = doc?.gpx?.trk?.trkseg?.trkpt ?? [];
  const points = [];
  for (const pt of Array.isArray(trkpts) ? trkpts : [trkpts]) {
    const lat = parseFloat(pt['@_lat']);
    const lon = parseFloat(pt['@_lon']);
    const ele = parseFloat(pt.ele);
    if (isNaN(lat) || isNaN(lon) || isNaN(ele)) continue;
    points.push({ lat, lon, ele });
  }
  return { name: doc?.gpx?.trk?.name ?? 'Trace sans nom', points };
}

// ---------- Haversine ----------
function haversine(lat1, lon1, lat2, lon2) {
  const R = 6371000;
  const phi1 = lat1 * Math.PI / 180, phi2 = lat2 * Math.PI / 180;
  const dphi = (lat2 - lat1) * Math.PI / 180;
  const dlam = (lon2 - lon1) * Math.PI / 180;
  const a = Math.sin(dphi/2)**2 + Math.cos(phi1)*Math.cos(phi2)*Math.sin(dlam/2)**2;
  return 2 * R * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
}

// ---------- Rééchantillonnage par moyenne ----------
function resampleUniform(distRaw, eleRaw, stepM) {
  const n = distRaw.length;
  const totalM = distRaw[n - 1];
  const m = Math.floor(totalM / stepM) + 1;
  const dist = new Array(m);
  const ele  = new Array(m);

  const avgWin = Math.min(Math.max(stepM, RESAMPLE_AVG_MIN_M), 3 * stepM);
  const halfW = avgWin / 2;
  let startPtr = 0;

  for (let k = 0; k < m; k++) {
    const dCenter = k * stepM;
    dist[k] = dCenter;
    const lo = dCenter - halfW;
    const hi = dCenter + halfW;

    while (startPtr < n - 1 && distRaw[startPtr] < lo) startPtr++;

    let sumEle = 0, count = 0;
    for (let i = startPtr; i < n; i++) {
      if (distRaw[i] >= hi) break;
      if (distRaw[i] >= lo) { sumEle += eleRaw[i]; count++; }
    }

    if (count > 0) {
      ele[k] = sumEle / count;
    } else {
      let jj = 0;
      while (jj < n - 2 && distRaw[jj + 1] < dCenter) jj++;
      const d0 = distRaw[jj], d1 = distRaw[jj + 1];
      if (d1 - d0 < 1e-6) {
        ele[k] = eleRaw[jj];
      } else {
        const t = (dCenter - d0) / (d1 - d0);
        ele[k] = eleRaw[jj] + (eleRaw[jj + 1] - eleRaw[jj]) * t;
      }
    }
  }
  return { dist, ele, stepM, avgWinM: avgWin };
}

// ---------- Plateaux ----------
function plateauCorrect(ele, minRun, tolerance) {
  const n = ele.length;
  if (n < 3 || minRun < 2) return { out: ele.slice(), count: 0 };
  const out = ele.slice();
  let count = 0;
  let i = 0;
  while (i < n) {
    let j = i + 1;
    while (j < n && Math.abs(ele[j] - ele[i]) <= tolerance) j++;
    const runLen = j - i;
    if (runLen >= minRun && i > 0 && j < n) {
      const eStart = ele[i - 1], eEnd = ele[j];
      const span = j - (i - 1);
      for (let k = i; k < j; k++) {
        const t = (k - (i - 1)) / span;
        out[k] = eStart + (eEnd - eStart) * t;
      }
      count++;
    }
    i = j;
  }
  return { out, count };
}

// ---------- Anti-pic Hampel ----------
function despikeWindowMToPts(windowM, stepM) {
  return Math.max(3, Math.round(windowM / stepM) | 1);
}

function despikeAltitude(ele, winPts, thresholdM) {
  const n = ele.length;
  if (n < 3 || winPts < 3 || thresholdM <= 0) return { out: ele.slice(), count: 0 };
  if (winPts % 2 === 0) winPts += 1;
  const half = Math.floor(winPts / 2);
  const out = ele.slice();
  const buf = new Array(2 * half + 1);
  let count = 0;

  for (let i = 0; i < n; i++) {
    let k = 0;
    for (let j = -half; j <= half; j++) {
      const idx = Math.max(0, Math.min(n - 1, i + j));
      buf[k++] = ele[idx];
    }
    buf.sort((a, b) => a - b);
    const median = buf[half];
    if (Math.abs(ele[i] - median) > thresholdM) {
      out[i] = median;
      count++;
    }
  }
  return { out, count };
}

// ---------- Stats brutes ----------
function computeRawStats(distRaw, eleRawOrig) {
  const n = distRaw.length;
  let dPlus = 0, dMoins = 0;
  for (let i = 1; i < n; i++) {
    const dDist = distRaw[i] - distRaw[i-1];
    if (dDist < RAW_DOUBLON_MIN_M) continue;
    const d = eleRawOrig[i] - eleRawOrig[i-1];
    if (d > 0) dPlus += d; else dMoins -= d;
  }
  const gaps = [];
  for (let i = 1; i < n; i++) {
    const g = distRaw[i] - distRaw[i-1];
    if (g > 0.01) gaps.push(g);
  }
  gaps.sort((a, b) => a - b);
  const median = gaps.length ? gaps[Math.floor(gaps.length / 2)] : 0;
  const min = gaps.length ? gaps[0] : 0;
  const max = gaps.length ? gaps[gaps.length-1] : 0;
  const mean = gaps.length ? gaps.reduce((a,b)=>a+b,0) / gaps.length : 0;
  let eleMin = Infinity, eleMax = -Infinity;
  for (const e of eleRawOrig) { if (e < eleMin) eleMin = e; if (e > eleMax) eleMax = e; }
  return { n, dPlus, dMoins, gapMin: min, gapMed: median, gapMax: max, gapMean: mean,
           eleMin, eleMax, doublons: n - gaps.length };
}

// ---------- Lissages ----------
function movingAverage(arr, win) {
  const n = arr.length;
  if (win < 2 || n < 2) return arr.slice();
  const half = Math.floor(win / 2);
  const p = new Array(n + 1).fill(0);
  for (let i = 0; i < n; i++) p[i+1] = p[i] + arr[i];
  const res = new Array(n);
  for (let i = 0; i < n; i++) {
    const lo = Math.max(0, i - half), hi = Math.min(n - 1, i + half);
    res[i] = (p[hi+1] - p[lo]) / (hi - lo + 1);
  }
  return res;
}

function savitzkyGolay(arr, win, order = 2) {
  const n = arr.length;
  if (win < 5 || n < 5) return arr.slice();
  if (win % 2 === 0) win += 1;
  const half = Math.floor(win / 2);
  const m = half;
  const coefs = new Array(win);
  for (let k = -half; k <= half; k++) {
    coefs[k + half] = 3 * m * m + 3 * m - 1 - 5 * k * k;
  }
  const out = new Array(n);
  for (let i = 0; i < n; i++) {
    let sum = 0, wsum = 0;
    for (let j = -half; j <= half; j++) {
      const idx = i + j;
      if (idx < 0 || idx >= n) continue;
      const c = coefs[j + half];
      sum += c * arr[idx]; wsum += c;
    }
    out[i] = wsum > 0 ? sum / wsum : arr[i];
  }
  return out;
}

function gaussianFilter(arr, sigma) {
  const n = arr.length;
  if (!sigma || sigma <= 0 || n < 2) return arr.slice();
  const half = Math.ceil(3 * sigma);
  const kernel = new Float64Array(2 * half + 1);
  let ksum = 0;
  for (let k = -half; k <= half; k++) {
    const v = Math.exp(-(k * k) / (2 * sigma * sigma));
    kernel[k + half] = v; ksum += v;
  }
  for (let i = 0; i < kernel.length; i++) kernel[i] /= ksum;
  const out = new Array(n);
  for (let i = 0; i < n; i++) {
    let sum = 0, wsum = 0;
    for (let j = -half; j <= half; j++) {
      const idx = i + j;
      if (idx < 0 || idx >= n) continue;
      const w = kernel[j + half];
      sum += w * arr[idx]; wsum += w;
    }
    out[i] = wsum > 0 ? sum / wsum : arr[i];
  }
  return out;
}

function medianFilter(arr, win) {
  const n = arr.length;
  if (!win || win < 3 || n < 3) return arr.slice();
  if (win % 2 === 0) win += 1;
  const half = Math.floor(win / 2);
  const out = new Array(n);
  const buf = new Array(2 * half + 1);
  for (let i = 0; i < n; i++) {
    let k = 0;
    for (let j = -half; j <= half; j++) {
      const idx = Math.max(0, Math.min(n - 1, i + j));
      buf[k++] = arr[idx];
    }
    buf.sort((a, b) => a - b);
    out[i] = buf[half];
  }
  return out;
}

// ---------- Pentes ----------
function computeSlope(ele, dist, minD = 0.05) {
  const n = ele.length, s = new Array(n).fill(0);
  for (let i = 0; i < n; i++) {
    const i1 = Math.max(0, i - 1), i2 = Math.min(n - 1, i + 1);
    const d = dist[i2] - dist[i1];
    if (d > minD) s[i] = ((ele[i2] - ele[i1]) / d) * 100;
  }
  return s;
}

function clipSlope(slope, maxPct) {
  const out = new Array(slope.length);
  for (let i = 0; i < slope.length; i++) {
    const v = slope[i];
    out[i] = v > maxPct ? maxPct : (v < -maxPct ? -maxPct : v);
  }
  return out;
}

// ---------- Ancrage ----------
function anchoredMovingAverage(raw, anchorIdxs, stepM, fenetreAffichageM, sigmaM, doubleSG) {
  const n = raw.length;
  if (n < 2) return raw.slice();
  const winPts = Math.max(5, Math.round(fenetreAffichageM / stepM) | 1);
  let base = savitzkyGolay(raw, winPts, 2);
  if (sigmaM > 0) {
    const sigmaPts = sigmaM / stepM;
    base = gaussianFilter(base, sigmaPts);
  }
  const set = new Set([0, n - 1, ...anchorIdxs.filter(i => i > 0 && i < n - 1)]);
  const anchors = [...set].sort((a, b) => a - b);
  const result = base.slice();
  for (let k = 0; k < anchors.length - 1; k++) {
    const a = anchors[k], b = anchors[k + 1];
    if (b - a < 2) continue;
    const corrA = raw[a] - base[a];
    const corrB = raw[b] - base[b];
    for (let i = a; i <= b; i++) {
      const t = (i - a) / (b - a);
      result[i] = base[i] + corrA * (1 - t) + corrB * t;
    }
  }
  let final = result;
  if (doubleSG) {
    const winDouble = Math.max(31, Math.round(fenetreAffichageM * 2 / stepM) | 1);
    final = savitzkyGolay(result, winDouble, 2);
    for (const a of anchors) final[a] = result[a];
  }
  return final;
}

// ---------- Helpers ----------
function findIdxAtDist(dist, targetM) {
  let lo = 0, hi = dist.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (dist[mid] < targetM) lo = mid + 1; else hi = mid;
  }
  return lo;
}

function argminRange(arr, lo, hi) {
  let best = lo, v = arr[lo];
  for (let i = lo + 1; i <= hi; i++) if (arr[i] < v) { v = arr[i]; best = i; }
  return best;
}

function argmaxRange(arr, lo, hi) {
  let best = lo, v = arr[lo];
  for (let i = lo + 1; i <= hi; i++) if (arr[i] > v) { v = arr[i]; best = i; }
  return best;
}

// ---------- Hystérésis ----------
const CLIMB_CRITERIA = [
  { minLen: 1000, minSlope: 3 }, { minLen: 750, minSlope: 4 },
  { minLen: 500, minSlope: 5 }, { minLen: 400, minSlope: 6 },
  { minLen: 300, minSlope: 7 }
];

function minSlopeForLength(len) {
  for (const c of CLIMB_CRITERIA) if (len >= c.minLen) return c.minSlope;
  return Infinity;
}

function detectEvents(slope, seuil) {
  const ev = []; let inC = false;
  for (let i = 0; i < slope.length; i++) {
    if (!inC && slope[i] > seuil) { ev.push({ idx: i, type: 'start' }); inC = true; }
    else if (inC && slope[i] < -seuil) { ev.push({ idx: i, type: 'summit' }); inC = false; }
  }
  if (inC) ev.push({ idx: slope.length - 1, type: 'summit' });
  return ev;
}

function buildClimbCandidates(events, n) {
  const segs = []; let prev = 0;
  let cur = (events.length && events[0].type === 'summit') ? 'climb' : 'non-climb';
  for (const e of events) {
    if (e.idx > prev) segs.push({ startIdx: prev, endIdx: e.idx, key: cur });
    cur = e.type === 'start' ? 'climb' : 'non-climb';
    prev = e.idx;
  }
  if (prev < n - 1) segs.push({ startIdx: prev, endIdx: n - 1, key: cur });
  return segs.filter(s => s.key === 'climb');
}

function findBestSubClimb(dist, eleMetric, startIdx, endIdx) {
  const n = endIdx - startIdx + 1;
  if (n < 2) return null;
  const cum = new Float64Array(n);
  for (let k = 1; k < n; k++) {
    const d = eleMetric[startIdx + k] - eleMetric[startIdx + k - 1];
    cum[k] = cum[k - 1] + (d > 0 ? d : 0);
  }
  const step = n > 800 ? Math.ceil(n / 800) : 1;
  let best = null;
  for (let si = 0; si < n - 1; si += step) {
    for (let ei = si + 1; ei < n; ei += step) {
      const len = dist[startIdx + ei] - dist[startIdx + si];
      if (len < 300) continue;
      if (best && len <= best.length) continue;
      const dP = cum[ei] - cum[si];
      const s = (dP / len) * 100;
      if (s >= minSlopeForLength(len)) {
        best = { startIdx: startIdx + si, endIdx: startIdx + ei,
                 length: len, dPlus: dP, avgSlope: s };
      }
    }
  }
  return best;
}

// ---------- computeAuto (avec mergedParts) ----------
function computeAuto(dist, eleRaw, params) {
  const { stepM, fenetreAffichageM, fenetrePenteM, seuilMontee, refinePct,
          medianWindowM, gaussianSigmaM, gapMinM } = params;
  const n = eleRaw.length;

  let eleMedian = eleRaw;
  if (medianWindowM > 0) {
    const winPts = Math.max(3, Math.round(medianWindowM / stepM) | 1);
    eleMedian = medianFilter(eleRaw, winPts);
  }

  const winSGPts = Math.max(5, Math.round(fenetreAffichageM / stepM) | 1);
  const eleBase = savitzkyGolay(eleMedian, winSGPts, 2);

  const winSlopePts = Math.max(1, Math.round(fenetrePenteM / stepM));
  const slopeBase = clipSlope(
    movingAverage(computeSlope(eleBase, dist), winSlopePts),
    SLOPE_DISPLAY_MAX_PCT
  );

  const candidates = buildClimbCandidates(detectEvents(slopeBase, seuilMontee), n);

  const candidatesWithSub = [];
  for (const c of candidates) {
    const sub = findBestSubClimb(dist, eleBase, c.startIdx, c.endIdx);
    if (sub) candidatesWithSub.push(sub);
  }

  const anchors = [];
  const refined = [];
  let lastEnd = -1;

  for (const c of candidatesWithSub) {
    const candidateLen = c.length;
    const effectiveRefine = Math.max(
      REFINE_MIN_M,
      Math.min(REFINE_MAX_M, refinePct * candidateLen / 100)
    );
    const hiIdx = findIdxAtDist(dist, Math.min(dist[n - 1], dist[c.endIdx] + effectiveRefine));
    const newEnd = (hiIdx > c.endIdx) ? argmaxRange(eleBase, c.endIdx, hiIdx) : c.endIdx;
    if (newEnd <= c.startIdx) continue;
    if (newEnd <= lastEnd) continue;

    const naturalLow = lastEnd + 1;
    const safetyLow = findIdxAtDist(dist, Math.max(0, dist[c.startIdx] - effectiveRefine));
    const loIdx = Math.max(naturalLow, safetyLow);
    const newStart = (loIdx < c.startIdx) ? argminRange(eleBase, loIdx, c.startIdx) : c.startIdx;
    if (newStart >= newEnd) continue;

    // Fusion avec conservation des mergedParts
    if (refined.length > 0 && gapMinM > 0) {
      const prev = refined[refined.length - 1];
      const gapM = dist[newStart] - dist[prev.endIdx];
      if (gapM < gapMinM) {
        if (!prev._mergedParts) {
          prev._mergedParts = [{
            startKm: dist[prev.startIdx] / 1000,
            endKm: dist[prev.endIdx] / 1000
          }];
        }
        const combinedEnd = argmaxRange(eleBase, prev.endIdx, newEnd);
        const midValley = argminRange(eleBase, prev.endIdx, newStart);
        let newPrevStart = prev.startIdx;
        if (eleBase[midValley] < eleBase[prev.startIdx]) {
          newPrevStart = midValley;
          anchors[anchors.length - 2] = midValley;
        }
        prev._mergedParts.push({
          startKm: dist[newStart] / 1000,
          endKm: dist[newEnd] / 1000
        });
        prev.startIdx = newPrevStart;
        prev.endIdx = combinedEnd;
        prev.candidateLen += candidateLen;
        prev.refineM = Math.max(prev.refineM || 0, effectiveRefine);
        prev._merged = (prev._merged || 0) + 1;
        anchors[anchors.length - 1] = combinedEnd;
        lastEnd = combinedEnd;
        continue;
      }
    }

    anchors.push(newStart, newEnd);
    refined.push({
      startIdx: newStart, endIdx: newEnd,
      refineM: effectiveRefine, candidateLen,
      _merged: 0, _mergedParts: null
    });
    lastEnd = newEnd;
  }

  const eleDisplay = anchoredMovingAverage(
    eleRaw, anchors, stepM, fenetreAffichageM, gaussianSigmaM, params.smoothExtra
  );

  const autoClimbs = [];
  for (const c of refined) {
    let finalStart = c.startIdx;
    let finalEnd = c.endIdx;
    let length = dist[finalEnd] - dist[finalStart];
    let dPlus = 0;
    for (let k = finalStart + 1; k <= finalEnd; k++) {
      const d = eleDisplay[k] - eleDisplay[k - 1];
      if (d > 0) dPlus += d;
    }
    let avgSlope = length > 0 ? (dPlus / length) * 100 : 0;
    const minSlope = minSlopeForLength(length);
    let fallbackFlag = false;

    if (length < 300 || avgSlope < minSlope) {
      const sub = findBestSubClimb(dist, eleDisplay, c.startIdx, c.endIdx);
      if (!sub) continue;
      finalStart = sub.startIdx;
      finalEnd = sub.endIdx;
      fallbackFlag = true;
    }

    autoClimbs.push({
      startKm: dist[finalStart] / 1000,
      endKm: dist[finalEnd] / 1000,
      refineM: c.refineM,
      candidateLen: c.candidateLen,
      mergedCount: c._merged || 0,
      mergedParts: c._mergedParts || null,
      fallback: fallbackFlag
    });
  }

  return { autoClimbs };
}

// ---------- analyze ----------
function analyze(content, parameters) {
  const track = parseGpx(content);
  const n = track.points.length;

  const distRaw = new Float64Array(n);
  for (let i = 1; i < n; i++) {
    distRaw[i] = distRaw[i - 1] + haversine(
      track.points[i - 1].lat, track.points[i - 1].lon,
      track.points[i].lat, track.points[i].lon,
    );
  }

  const eleRawOrig = Float64Array.from(track.points.map(p => p.ele));
  const rawStats = computeRawStats(distRaw, eleRawOrig);

  let eleForResample = eleRawOrig;
  if (parameters.plateauRun > 0) {
    eleForResample = plateauCorrect(eleRawOrig, parameters.plateauRun, PLATEAU_TOLERANCE_M).out;
  }

  const { dist, ele, avgWinM } = resampleUniform(distRaw, eleForResample, parameters.stepM);

  let eleFinal = ele;
  let despikeCount = 0;
  const despikeWinPtsEffective = Math.max(3,
    Math.round(parameters.despikeWindowM / parameters.stepM) | 1);
  if (parameters.despikeThresholdM > 0) {
    const spikeRes = despikeAltitude(ele, despikeWinPtsEffective, parameters.despikeThresholdM);
    eleFinal = spikeRes.out;
    despikeCount = spikeRes.count;
  }

  const { autoClimbs } = computeAuto(dist, eleFinal, parameters);

  return {
    track: { name: track.name, points: track.points },
    distRaw: Array.from(distRaw),
    eleRawOrig: Array.from(eleRawOrig),
    dist: Array.from(dist),
    ele: Array.from(eleFinal),
    stepM: parameters.stepM,
    avgWinM,
    rawStats,
    autoClimbs,
    despikeCount,
    despikeWinPtsEffective,
  };
}

process.stdin.on('data', (chunk) => {
  const { content, parameters } = JSON.parse(chunk.toString());
  const result = analyze(content, parameters);
  process.stdout.write(JSON.stringify(result));
});
```

### 9.2 Commande Tauri

```rust
// src-tauri/src/commands/analyzer.rs
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Deserialize)]
pub struct AnalyzerParameters {
    pub step_m: f64,
    pub despike_window_m: f64,
    pub despike_threshold_m: f64,
    pub fenetre_affichage_m: f64,
    pub fenetre_pente_m: f64,
    pub fenetre_cat_m: f64,
    pub seuil_montee: f64,
    pub longueur_min: f64,
    pub gap_min_m: f64,
    pub refine_pct: f64,
    pub median_window_m: f64,
    pub gaussian_sigma_m: f64,
    pub plateau_run: u32,
    pub segment_log_m: u32,
    pub export_profile_step_m: f64,
    pub smooth_extra: bool,
    pub segment_smooth: bool,
}

#[tauri::command]
pub async fn analyze_gpx(
    content: String,
    parameters: AnalyzerParameters,
) -> Result<serde_json::Value, String> {
    let mut child = Command::new("node")
        .arg("sidecar/analyzer.js")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().ok_or("no stdin")?;
        let input = serde_json::json!({
            "content": content,
            "parameters": parameters,
        });
        stdin.write_all(input.to_string().as_bytes())
            .map_err(|e| e.to_string())?;
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
}
```

---

## 10. Intégration Tauri

### 10.1 Configuration `tauri.conf.json`

```json
{
  "tauri": {
    "bundle": {
      "resources": ["sidecar/analyzer.js"]
    },
    "allowlist": {
      "fs": {
        "readFile": true,
        "scope": ["$HOME/**", "$DOCUMENT/**"]
      },
      "dialog": { "open": true },
      "shell": {
        "sidecar": true
      }
    }
  }
}
```

### 10.2 Ouverture d'un fichier GPX

```typescript
// src/services/fileDialog.ts
import { open } from '@tauri-apps/plugin-dialog';

export async function pickGpxFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    filters: [{ name: 'GPX', extensions: ['gpx', 'txt'] }],
  });
  return typeof selected === 'string' ? selected : null;
}
```

### 10.3 Lecture du fichier

```rust
// src-tauri/src/commands/gpx.rs
use std::fs;

#[tauri::command]
pub async fn read_gpx_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}
```

### 10.4 Intégration Mapbox dans Tauri

- Mapbox GL JS est chargé **paresseusement** via `loadMapboxGL()` (CDN).
- Le token est stocké dans `localStorage` (persiste entre sessions).
- Aucune configuration Tauri supplémentaire n'est nécessaire pour Mapbox.

---

## 11. Intégration Mapbox GL JS

Voir section 7.5 (`useMapbox`) pour l'implémentation complète du composable.

### 11.1 Ordre des couches (critique)

```javascript
// 1. Terrain (raster-dem + setTerrain)
// 2. Brume (setFog)
// 3. Trace colorée (trace-line-layer) ← DOIT exister avant les contours
// 4. Marqueurs (climb-markers-circle)
// 5. Contours (contour-lines + contour-index, beforeId='trace-line-layer')
```

**Ne pas inverser** : les contours utilisent `'trace-line-layer'` comme `beforeId`. Si la trace n'existe pas encore, `addLayer` échoue silencieusement et les contours n'apparaissent jamais (bug corrigé en v3.8.5).

### 11.2 Chargement automatique au démarrage (v3.9.2)

```typescript
// À l'init de l'app
const saved = localStorage.getItem('mapboxToken');
if (saved) {
  initMapboxMap(saved).catch(err => {
    console.warn('[Mapbox] init auto échouée :', err);
  });
}
```

### 11.3 Trace colorée par catégorie (points bruts)

Voir section 6.14 (`buildTraceFeatures`). Point clé : **utiliser `rawPoints`, pas `dist`** (grille resamplée). La catégorie de chaque point brut est déterminée par sa distance cumulée (`distRaw[i]`), avec un parcours monotone de `catSegments`.

---

## 12. Widget profil (canvas)

Voir section 7.4 (`useProfileCanvas`) pour l'implémentation complète.

### 12.1 Échelles FIXES (v3.8.3 + v3.9.2)

```typescript
export const PROFILE_SCALE_X_FULL = 50;    // m/px
export const PROFILE_SCALE_X_CLIMB = 10;   // m/px (v3.9.2 : 10 au lieu de 5)
export const PROFILE_SCALE_Y = 10;         // m/px
```

La largeur du canvas est **strictement** `plageVisibleM / scaleX`. Ne JAMAIS agrandir à la largeur du conteneur (bug v3.8.3).

### 12.2 Scroll centré (v3.9.2)

En mode montée, le profil affiche **toute la trace** à 10 m/px. Le canvas est donc plus large que le conteneur. Après chaque rendu, on ajuste `scrollLeft` pour centrer la montée sélectionnée :

```typescript
if (profileMode === 'climb' && !dragging && selectedClimbIdx != null) {
  const centerM = (c.startDist + c.endDist) / 2;
  const centerPx = (centerM - viewStartM) / scaleXActual;
  const targetScroll = Math.max(0, centerPx - scroll.clientWidth / 2);
  requestAnimationFrame(() => { scroll.scrollLeft = targetScroll; });
}
```

**Ne pas re-scroller** pendant un drag (état `__profileDragging`).

---

## 13. Drag & drop sur le profil

### 13.1 Détection de poignée

```typescript
function handleAt(cx: number, cy: number): 'valley' | 'summit' | null {
  if (profileMode !== 'climb' || selectedClimbIdx == null) return null;
  if (cy > 30) return null;   // Bande des 30 premiers px
  const c = climbs[selectedClimbIdx];
  if (!c || c.invalid) return null;

  const xV = (c.startDist - viewStartM) / scaleXActual;
  const xS = (c.endDist - viewStartM) / scaleXActual;
  const th = 14;   // Seuil de clic ±14 px
  const dV = Math.abs(cx - xV);
  const dS = Math.abs(cx - xS);
  if (dV < th && dV <= dS) return 'valley';
  if (dS < th) return 'summit';
  return null;
}
```

### 13.2 Cycle de drag

```
mousedown sur poignée
  → __profileDragging = 'valley' | 'summit'
  → cursor = 'ew-resize'

mousemove
  → km = viewStartM/1000 + (canvasX - rect.left) * scaleXActual / 1000
  → km = applyDragConstraint(handle, km, climb)
  → climb.startKm ou climb.endKm = km
  → rebuild() + renderProfile() + renderClimbsList()
  → updateMapMarkers()

mouseup
  → __profileDragging = null
  → normalizeGroups()
  → renderAll()
```

### 13.3 Clamp à la vue

```typescript
const marginM = scaleXActual * 14;
distM = Math.max(viewStartM + marginM, Math.min(viewEndM - marginM, distM));
```

Empêche de perdre la poignée si l'utilisateur tire au-delà du bord.

---

## 14. Groupes de montées

Voir section 6.11 (`climbGroups.ts`) pour l'implémentation.

### 14.1 Flux complet

```
Détection avec fusion (computeAuto)
  → mergedParts conservés
  → mergedCount > 0

Clic sur ✂ Séparer
  → groupId = nextGroupId++
  → userClimbs = [...sub-montées avec subIndex=1,2,3...]
  → zoomMapToGroup(gid)
  → sélection = première sous-montée

Clic sur 🔗 Refusionner
  → members = userClimbs.filter(c => c.groupId === gid)
  → mergedParts = members.map(m => ({startKm, endKm}))
  → userClimbs = [...autres montées, merged]
  → merged.groupId = null
  → merged.mergedParts = mergedParts

Édition d'une sous-montée (drag)
  → si groupe reste avec 1 seul membre après édition :
    normalizeGroups() dissout le groupe
```

### 14.2 Numérotation `#N.M`

```typescript
let mainCounter = 0;
const groupCounters: Record<number, number> = {};

for (const c of climbs) {
  if (c.groupId != null) {
    groupCounters[c.groupId] = (groupCounters[c.groupId] || 0) + 1;
    const sub = c.subIndex ?? groupCounters[c.groupId];
    label = `#${c.groupId}.${sub}`;
  } else {
    mainCounter++;
    label = `#${mainCounter}`;
  }
}
```

### 14.3 Couleur stable

```typescript
function groupColor(gid: number): string {
  const hue = (gid * 137.508) % 360;   // Angle d'or
  return `hsl(${hue.toFixed(0)}, 62%, 46%)`;
}
```

### 14.4 Multi-marqueurs sur la carte (v3.9.2)

Voir section 6.15 (`buildMapMarkers`). Si la montée sélectionnée est groupée, on affiche **tous** les points vallée/sommet de **tous** les membres du groupe, avec `role='selected'` pour le membre actif (rayon 10, contour 4) et `role='member'` pour les autres (rayon 8, contour 2.5).

---

## 15. Création manuelle

Voir section 6.13 (`manualClimb.ts`) pour les fonctions pures.

### 15.1 Machine à états

```
idle
  → dblclick sur canvas (mode full, position libre)
  → pending (valleyKm posée, summitKm = valleyKm)

pending
  → mousedown + drag
  → dragging (summitKm mis à jour à chaque mousemove)

dragging
  → mouseup
  → validation :
    - summitKm > valleyKm + 50 m ?
    - isFreeRange(valleyKm, summitKm) ?
    → si OUI : addManualClimb()
    → si NON : annulation

pending / dragging
  → Échap
  → annulation (retour à idle)
```

### 15.2 Composables Vue

Voir section 7.3 (`useManualClimb`).

### 15.3 Validation des chevauchements

```typescript
export function isFreeRange(aKm: number, bKm: number, userClimbs: UserClimb[]): boolean {
  const lo = Math.min(aKm, bKm);
  const hi = Math.max(aKm, bKm);
  for (const c of userClimbs) {
    if (lo < c.endKm && hi > c.startKm) return false;
  }
  return true;
}
```

### 15.4 Indicateurs visuels

- Zone colorée : verte si valide, rouge si chevauchement
- Ligne vallée verte, ligne sommet rouge pointillée (ou rouge vif si invalide)
- Curseur `crosshair` sur le canvas
- Hint textuel dans le header

---

## 16. Suppression de montées

Voir section 6.12 (`deleteClimb.ts`) pour la fonction pure.

### 16.1 Bouton ✕

Positionné en `position: absolute; top: 8px; right: 8px` sur chaque carte de montée. Toujours visible (sans hover).

### 16.2 Comportement

1. Retire la montée de `userClimbs`
2. **Renumérote les `subIndex`** du groupe affecté
3. **Dissout le groupe** s'il ne reste qu'un membre
4. **Ajuste `selectedClimbIdx`** :
   - Si c'était la montée supprimée → désélectionne, mode full
   - Si l'index était supérieur → décrémente
5. `renderAll()`

### 16.3 Exemple

```typescript
function deleteClimb(idx: number): void {
  const { deletedGroupId, dissolved } = deleteClimbFromList(idx, userClimbs);
  if (selectedClimbIdx === idx) {
    selectedClimbIdx = null;
    profileMode = 'full';
  } else if (selectedClimbIdx != null && selectedClimbIdx > idx) {
    selectedClimbIdx -= 1;
  }
  renderAll();
}
```

---

## 17. Pipeline complet de détection

### 17.1 Pseudocode global

```typescript
function analyze(content: string, params: AnalyzerParameters): AnalyzerResult {
  // 1. Parse GPX
  const track = parseGpx(content);

  // 2. Distances cumulées
  const distRaw = computeCumulativeDistances(track.points);

  // 3. Altitudes d'origine + stats
  const eleRawOrig = Float64Array.from(track.points.map(p => p.ele));
  const rawStats = computeRawStats(distRaw, eleRawOrig);

  // 4. Correction plateaux (optionnelle)
  let eleForResample = eleRawOrig;
  if (params.plateauRun > 0) {
    eleForResample = plateauCorrect(eleRawOrig, params.plateauRun, PLATEAU_TOLERANCE_M).out;
  }

  // 5. Rééchantillonnage par moyenne
  const { dist, ele, avgWinM } = resampleUniform(distRaw, eleForResample, params.stepM);

  // 6. Anti-pic Hampel
  let eleFinal = ele;
  let despikeCount = 0;
  const despikeWinPts = despikeWindowMToPts(params.despikeWindowM, params.stepM);
  if (params.despikeThresholdM > 0) {
    const spikeRes = despikeAltitude(ele, despikeWinPts, params.despikeThresholdM);
    eleFinal = spikeRes.out;
    despikeCount = spikeRes.count;
  }

  // 7. Détection complète
  const { autoClimbs } = computeAuto(dist, eleFinal, params);

  return {
    track: { name: track.name, points: track.points },
    distRaw: Array.from(distRaw),
    eleRawOrig: Array.from(eleRawOrig),
    dist: Array.from(dist),
    ele: Array.from(eleFinal),
    stepM: params.stepM,
    avgWinM,
    rawStats,
    autoClimbs,
    despikeCount,
    despikeWinPtsEffective: despikeWinPts,
  };
}
```

### 17.2 Ordre des étapes dans `computeAuto`

1. Filtre médian (optionnel)
2. Savitzky-Golay (altitude de base)
3. Pente lissée + clip
4. Hystérésis (`detectEvents`)
5. Candidats (`buildClimbCandidates`)
6. Sous-segment optimal (`findBestSubClimb`)
7. Raffinement + Fusion (`argminRange`/`argmaxRange`, `gapMinM` + `mergedParts`)
8. Lissage ancré final (`anchoredMovingAverage`)
9. Validation stricte + repli (`⤺`, avec préservation `mergedParts`)

### 17.3 Log debug

Le pipeline émet des logs dans le panneau debug :

```
📐 PIPELINE v3.9.2
   Trace brute : 3312 points · longueur 110.245 km
   Pas brut : min 0.3 m · médian 22.4 m · moy 33.1 m · max 892.5 m
   D+ brut (doublons ignorés) : 2874 m
📐 Rééchantillonnage (moyenne) : 2190 pts à pas 50 m · fenêtre 75 m
🩹 Anti-pic (fenêtre=200 m → 5 pts ≈ 250 m, seuil=15 m) : 2 point(s) corrigé(s) sur 2190
[v3.9.2] 2 fusion(s)
[v3.9.2] 3 conservées, 1 replis
```

### 17.4 Pipeline de reconstruction (`rebuild`)

```typescript
function rebuild(): RebuildResult | null {
  if (!dist || !eleRaw) return null;
  const params = getParams();

  // 1. Contraintes anti-chevauchement
  const sorted = applyConstraints(userClimbs);

  // 2. Ancrage
  const anchors: number[] = [];
  const resolved = sorted.map(c => {
    const startIdx = findIdxAtDist(dist, c.startKm * 1000);
    const endIdx = findIdxAtDist(dist, c.endKm * 1000);
    if (startIdx >= endIdx) {
      return { startIdx, endIdx, ref: c, invalid: true, constrainedStart: c._constrainedStart, constrainedEnd: c._constrainedEnd };
    }
    anchors.push(startIdx, endIdx);
    return { startIdx, endIdx, ref: c, invalid: false, constrainedStart: c._constrainedStart, constrainedEnd: c._constrainedEnd };
  });

  // 3. Courbe affichée (ancrée)
  const eleDisplay = anchoredMovingAverage(
    eleRaw, anchors, stepM,
    params.fenetreAffichageM, params.gaussianSigmaM, params.smoothExtra,
  );

  // 4. Pentes pour l'affichage
  const slopeRaw = clipSlope(computeSlope(movingAverage(eleRaw, 5), dist), SLOPE_DISPLAY_MAX_PCT);
  const slopeSemi = computeSlope(eleDisplay, dist);
  const winSlopePts = Math.max(1, Math.round(params.fenetrePenteM / stepM));
  const slopeDisplay = clipSlope(movingAverage(slopeSemi, winSlopePts), SLOPE_DISPLAY_MAX_PCT);

  // 5. Catégorisation NON ancrée (v3.8.1)
  const winSGCatPts = Math.max(5, Math.round(params.fenetreAffichageM / stepM) | 1);
  const eleBaseCat = savitzkyGolay(eleRaw, winSGCatPts, 2);
  const slopeForCat = computeSlopeWindow(eleBaseCat, dist, params.fenetreCatM);

  // 6. Métriques par montée
  const climbs = resolved.map(r => {
    if (r.invalid) {
      return { ...r, length: 0, dPlus: 0, avgSlope: 0, altStart: 0, altEnd: 0,
               mergedCount: r.ref.mergedCount || 0, fallback: r.ref.fallback || false,
               groupId: r.ref.groupId, subIndex: r.ref.subIndex };
    }
    let dPlus = 0;
    for (let k = r.startIdx + 1; k <= r.endIdx; k++) {
      const d = eleDisplay[k] - eleDisplay[k - 1];
      if (d > 0) dPlus += d;
    }
    const length = dist[r.endIdx] - dist[r.startIdx];
    return {
      ...r, length, dPlus,
      avgSlope: length > 0 ? (dPlus / length) * 100 : 0,
      altStart: eleDisplay[r.startIdx], altEnd: eleDisplay[r.endIdx],
      mergedCount: r.ref.mergedCount || 0, fallback: r.ref.fallback || false,
      groupId: r.ref.groupId, subIndex: r.ref.subIndex,
    };
  });

  // 7. Segmentation
  const catSegments = segmentByCategory(slopeForCat, dist, eleDisplay, params.longueurMin);

  return { dist, eleRaw, eleDisplay, slopeRaw, slopeDisplay, slopeForCat, catSegments, climbs };
}
```

---

## 18. Diagnostic de configuration

Voir section 7.4 pour le composant Vue.

### 18.1 Fonction `computeDiagnostics`

```typescript
export function computeDiagnostics(params: AnalyzerParameters): DiagResult {
  const {
    stepM, despikeWindowM, despikeThresholdM,
    fenetreAffichageM, fenetrePenteM, fenetreCatM,
    seuilMontee, longueurMin, gapMinM, refinePct,
    medianWindowM, gaussianSigmaM, plateauRun,
    exportProfileStepM,
  } = params;

  // Valeurs effectives
  const lissageAltPts = Math.max(1, Math.round(fenetreAffichageM / stepM));
  const lissagePentePts = Math.max(1, Math.round(fenetrePenteM / stepM));
  const couleurPts = Math.max(1, Math.round(fenetreCatM / stepM));
  const medianPts = medianWindowM > 0 ? Math.max(1, Math.round(medianWindowM / stepM)) : 0;
  const gaussianPts = gaussianSigmaM > 0 ? gaussianSigmaM / stepM : 0;
  const despikeActive = despikeThresholdM > 0;
  const despikeWinPtsEffective = despikeActive
    ? Math.max(3, Math.round(despikeWindowM / stepM) | 1)
    : 0;
  const despikeCovM = despikeWinPtsEffective * stepM;
  const despikeExpandRatio = despikeActive && despikeWindowM > 0
    ? despikeCovM / despikeWindowM
    : 1;
  const ratioAltPente = fenetrePenteM > 0 ? fenetreAffichageM / fenetrePenteM : 0;
  const retardM = fenetrePenteM / 2;

  const diag: Record<string, DiagEntry> = {};

  // ... (chaque paramètre avec status green/orange/red/grey)

  const warnings: DiagWarning[] = [];
  if (lissageAltPts < 5) warnings.push({ level: 'red', msg: '...' });
  else if (lissageAltPts < 10) warnings.push({ level: 'orange', msg: '...' });

  // ... (toutes les règles)

  return { diag, warnings };
}
```

### 18.2 Statuts

| Statut | Signification |
|---|---|
| 🟢 green | Optimal |
| 🟠 orange | Limite / avertissement |
| 🔴 red | Incohérence critique |
| ⚪ grey | Désactivé / non applicable |

---

## 19. Export JSON

Voir section 6.17 (`exportData.ts`) pour l'implémentation complète.

### 19.1 Structure

```json
{
  "metadata": {
    "file": "trace.gpx",
    "exportedAt": "ISO8601",
    "version": "3.9.2",
    "totalDistanceKm": 4.28,
    "totalDistanceSampledKm": 4.25,
    "trailingGapM": 30,
    "stepM": 50,
    "totalPoints": 86,
    "segmentsCount": 4,
    "climbsCount": 1,
    "profileStepM": 50
  },
  "legend": {
    "note": "...",
    "fenetreCatM": 100,
    "longueurMin": 200,
    "categories": [...]
  },
  "segments": [...],
  "climbs": [
    {
      "index": 0,
      "groupId": null,
      "subIndex": null,
      "startKm": 0,
      "endKm": 3.95,
      "startProgress": 0,
      "endProgress": 0.91933,
      "dPlusM": 284,
      "avgSlopePct": 7.67,
      "altStartM": 503.1,
      "altEndM": 786.9,
      "mergedCount": 0,
      "fallback": false,
      "slopeProfile": [...],
      "slopeProfileStepM": 50
    }
  ]
}
```

### 19.2 Nouveautés v3.9.1

Ajout de `groupId` et `subIndex` dans chaque `climb`.

### 19.3 Contrat d'alignement

Chaque tranche de `slopeProfile` appartient **intégralement** à un seul segment global. Sa `categoryId` est **strictement identique** à celle du segment qui la contient.

---

## 20. Tests et validation

### 20.1 Tests unitaires (Vitest)

```typescript
// tests/unit/despike.test.ts
import { describe, it, expect } from 'vitest';
import { despikeAltitude, despikeWindowMToPts } from '@/utils/despike';

describe('despikeWindowMToPts (v3.6)', () => {
  it('convertit mètres → points', () => {
    expect(despikeWindowMToPts(200, 50)).toBe(5);
    expect(despikeWindowMToPts(100, 50)).toBe(3);
    expect(despikeWindowMToPts(400, 50)).toBe(9);
  });
});

describe('despikeAltitude (Hampel)', () => {
  it('corrige un pic isolé', () => {
    const ele = Float64Array.from([1180, 1182, 1183, 1250, 1185, 1186, 1187]);
    const { out, count } = despikeAltitude(ele, 5, 15);
    expect(count).toBe(1);
    expect(out[3]).toBe(1183);
  });

  it('ne modifie pas une progression régulière', () => {
    const ele = Float64Array.from([1180, 1182, 1183, 1190, 1198, 1205, 1210]);
    const { count } = despikeAltitude(ele, 5, 15);
    expect(count).toBe(0);
  });
});

// tests/unit/resample.test.ts
describe('resampleUniform (moyenne)', () => {
  it('produit un pas constant', () => {
    const distRaw = new Float64Array([0, 5, 12, 25, 40]);
    const eleRaw = new Float64Array([100, 101, 102, 103, 104]);
    const { dist } = resampleUniform(distRaw, eleRaw, 10);
    expect(dist[0]).toBe(0);
    expect(dist[1]).toBe(10);
  });

  it('moyenne les points dans la fenêtre', () => {
    const distRaw = new Float64Array(20);
    const eleRaw = new Float64Array(20);
    for (let i = 0; i < 20; i++) {
      distRaw[i] = i * 10;
      eleRaw[i] = (i % 2 === 0) ? 100 : 200;
    }
    const { ele, avgWinM } = resampleUniform(distRaw, eleRaw, 25);
    expect(avgWinM).toBe(75);
  });
});

// tests/unit/groups.test.ts
describe('Groupes (v3.9.1)', () => {
  it('splitMergedClimb crée groupId + subIndex', () => {
    const userClimbs: UserClimb[] = [{
      startKm: 1, endKm: 5,
      autoStartKm: null, autoEndKm: null,
      refineM: null, candidateLen: null,
      mergedCount: 2, mergedParts: [
        { startKm: 1, endKm: 2 },
        { startKm: 3, endKm: 5 },
      ],
      fallback: false, groupId: null, subIndex: null,
    }];
    const res = splitMergedClimb(0, userClimbs, 1);
    expect(res).not.toBeNull();
    expect(userClimbs.length).toBe(2);
    expect(userClimbs[0].groupId).toBe(1);
    expect(userClimbs[0].subIndex).toBe(1);
    expect(userClimbs[1].subIndex).toBe(2);
  });

  it('refuseGroup reconstruit mergedParts', () => {
    const userClimbs: UserClimb[] = [
      { startKm: 1, endKm: 2, autoStartKm: null, autoEndKm: null,
        refineM: null, candidateLen: null, mergedCount: 0, mergedParts: null,
        fallback: false, groupId: 1, subIndex: 1 },
      { startKm: 3, endKm: 5, autoStartKm: null, autoEndKm: null,
        refineM: null, candidateLen: null, mergedCount: 0, mergedParts: null,
        fallback: false, groupId: 1, subIndex: 2 },
    ];
    const res = refuseGroup(1, userClimbs);
    expect(res).not.toBeNull();
    expect(userClimbs.length).toBe(1);
    expect(userClimbs[0].mergedParts).toEqual([
      { startKm: 1, endKm: 2 },
      { startKm: 3, endKm: 5 },
    ]);
  });

  it('normalizeGroups dissout les groupes à 1 membre', () => {
    const userClimbs: UserClimb[] = [
      { startKm: 1, endKm: 2, autoStartKm: null, autoEndKm: null,
        refineM: null, candidateLen: null, mergedCount: 0, mergedParts: null,
        fallback: false, groupId: 1, subIndex: 1 },
    ];
    normalizeGroups(userClimbs);
    expect(userClimbs[0].groupId).toBeNull();
  });
});

// tests/unit/buildTraceFeatures.test.ts
describe('buildTraceFeatures (v3.9.2)', () => {
  it('utilise les points bruts, pas la grille resamplée', () => {
    const rawPoints = [
      { lat: 45, lon: 6, ele: 100 },
      { lat: 45, lon: 6.001, ele: 105 },
      { lat: 45, lon: 6.002, ele: 110 },
    ];
    const distRaw = [0, 100, 200];
    const catSegments = [
      { startIdx: 0, endIdx: 1, startDist: 0, endDist: 100,
        length: 100, dPlus: 5, avgSlope: 5, altStart: 100, altEnd: 105, key: 'up2' },
      { startIdx: 2, endIdx: 2, startDist: 100, endDist: 200,
        length: 100, dPlus: 5, avgSlope: 5, altStart: 105, altEnd: 110, key: 'up2' },
    ];
    const fc = buildTraceFeatures(rawPoints, distRaw, catSegments);
    expect(fc.features.length).toBeGreaterThan(0);
    expect(fc.features[0].geometry.type).toBe('LineString');
  });
});
```

### 20.2 Tests d'intégration

- **Pipeline complet** : charger un GPX, vérifier le nombre de montées, les D+, les catégories
- **Fusion + séparation** : vérifier que les `mergedParts` sont bien transmis
- **Refusion** : vérifier que les bornes éditées sont conservées
- **Suppression** : vérifier la renumérotation
- **Création manuelle** : vérifier le refus en cas de chevauchement
- **Trace carte** : vérifier que `buildTraceFeatures` utilise `rawPoints`
- **Multi-marqueurs** : vérifier que la sélection d'un membre de groupe affiche tous les marqueurs

### 20.3 Jeu de tests de référence

| Fichier | Points | Caractéristique |
|---|---|---|
| `bormio-mortirolo.gpx` | ~3300 | Éditeur Openrunner |
| `gps-dense.gpx` | ~10000 | GPS haute fréquence |
| `glitch.gpx` | ~2000 | 1–2 pts aberrants |
| `fragments.gpx` | ~2000 | Faux-plats multiples |
| `sous-seuil.gpx` | ~1500 | Fragments + pente faible |
| `plat.gpx` | ~1000 | Aucune montée |

---

## 21. Migration depuis le prototype HTML

### 21.1 Correspondances

| Prototype HTML (v3.7.2) | v3.9.2 |
|---|---|
| `<script>` global | Modules TS + composables Vue |
| `STATE` objet global | Stores Pinia |
| `renderAll()` | Watchers Vue + composables |
| Plugin `segmentationPlugin` | Intégré dans `useProfileCanvas` |
| Plugin `markerDragPlugin` | Intégré dans `useProfileCanvas` |
| Interception window pointerdown | Idem, dans le composable |
| `document.getElementById` | `ref` Vue |
| Drag & drop DOM | `<FileDropZone />` |
| `alert()` | `<v-snackbar>` |
| Diagnostic | `<DiagnosticPanel />` + `useDiagnostics` |
| **Chart.js profil** | **Canvas 2D natif (`useProfileCanvas`)** |
| **Pas de carte** | **Mapbox GL JS (`useMapbox`)** |
| **Pas de groupes** | **`groupId` + `subIndex` + `mergedParts`** |
| **Pas de séparation** | **✂ Séparer** |
| **Pas de refusion** | **🔗 Refusionner** |
| **Pas de suppression** | **✕** |
| **Pas de création manuelle** | **Double-clic + drag** |

### 21.2 Ordre de migration recommandé

1. **Port des utilitaires purs** (§6) avec tests unitaires
2. **Types TypeScript** (§3) — en premier pour typer le reste
3. **Stores Pinia** (paramètres, données, montées, map)
4. **Composables de données** (`useGpxAnalyzer`, `useDiagnostics`)
5. **Composant `<FileDropZone>`**
6. **Composant `<DiagnosticPanel>`**
7. **Composant `<ParametersForm>`**
8. **Composant `<ClimbsPanel>`** avec `<ClimbCard>`
9. **Composable `useMapbox`**
10. **Composant `<MapPanel>`**
11. **Composable `useProfileCanvas`**
12. **Composant `<ProfilePanel>`**
13. **Composable `useMarkerDrag`** (drag poignées)
14. **Composable `useManualClimb`** (création)
15. **Composable `useClimbGroups`** (séparation/refusion)
16. **Suppression** (bouton ✕)
17. **Tests d'intégration** sur traces de référence

### 21.3 Points d'attention

| Point | Risque | Mitigation |
|---|---|---|
| Performance stores Pinia | Recalcul à chaque mutation | `watch` + debounce |
| Réactivité Float64Array | Non réactive | Convertir en `number[]` ou `shallowRef` |
| Mapbox GL JS | Fuite mémoire | `map.remove()` dans `onBeforeUnmount` |
| Sync zoom | Boucle infinie | Flag `syncing` |
| **Ordre couches Mapbox** | **Contours invisibles** | **Trace avant contours** |
| **Échelle X profil** | **Étirement trace** | **`canvasWidth = viewWidthM / scaleX`** |
| **Coloration profil** | **Divergence avec carte** | **`catSegments`, pas point-par-point** |
| **Catégorisation** | **Changement de couleur au drag** | **`eleBaseCat` (non ancrée)** |
| **Trace carte** | **Grille resamplée** | **`rawPoints`, pas `dist`** |
| Anti-pic sur 100k points | Latence | Worker ou Node.js |
| Diagnostic recalculé trop souvent | Perf | Debounce 250 ms |
| **Dissolution groupe** | **Référence orpheline** | **`normalizeGroups()` après chaque opération** |
| **SubIndex orphelins** | **Numérotation incohérente** | **Renuméroter après suppression** |

---

## 22. Annexes

### 22.1 Résumé des paramètres UI (15 + 2 toggles)

| Nom UI | Type | Défaut v3.9.2 | Plage |
|---|---|---|---|
| Pas de rééchantillonnage | float | **50** | 2–200 |
| Anti-pic : fenêtre | float | **200** | 50–500 |
| Anti-pic : seuil | float | 15 | 0–100 |
| Lissage altitude | float | **500** | 20–∞ |
| Lissage pente | float | 150 | 10–∞ |
| Fenêtre de calcul pente | float | 100 | 50–∞ |
| Seuil détection | float | 0.5 | 0.1–∞ |
| Longueur min. pente | float | 200 | 0–∞ |
| Écart min. entre montées | float | 250 | 0–5000 |
| Recherche extrema | float | 60 | 1–500 |
| Filtre médian | float | 0 | 0–300 |
| Lissage gaussien | float | 75 | 0–500 |
| Correction plateaux | int | 6 | 0–20 |
| Segments debug | int | 100 | 10–2000 |
| Pas du profil exporté | float | 50 | 10–500 |
| Lissage renforcé (double SG) | bool | ON | — |
| Lissage segments | bool | ON | — |

### 22.2 Constantes internes

| Nom | Valeur |
|---|---|
| `REFINE_MIN_M` | 1500 |
| `REFINE_MAX_M` | 8000 |
| `STEP_M_MIN` / `STEP_M_MAX` | 2 / 200 |
| `SLOPE_DISPLAY_MAX_PCT` | 25 |
| `PLATEAU_TOLERANCE_M` | 1.0 |
| `RAW_DOUBLON_MIN_M` | 1.0 |
| `DESPIKE_WINDOW_M_DEFAULT` | 200 |
| `DESPIKE_WINDOW_M_MIN/MAX` | 50/500 |
| `DESPIKE_THRESHOLD_M_DEFAULT` | 15 |
| `RESAMPLE_AVG_MIN_M` | 75 |
| `EXPORT_PROFILE_STEP_M_DEFAULT` | 50 |
| `EXPORT_PROFILE_STEP_M_MIN/MAX` | 10/500 |
| `INSERT_EDGE_MIN_KM` | 1.0 |
| `INSERT_BETWEEN_MIN_KM` | 0.2 |
| `MANUAL_MIN_LEN_KM` | 0.05 |
| `PROFILE_SCALE_X_FULL` | **50** |
| `PROFILE_SCALE_X_CLIMB` | **10** |
| `PROFILE_SCALE_Y` | 10 |
| `PROFILE_MIN_HEIGHT` | 110 |
| `PROFILE_HANDLE_PAD` | 200 |
| `MAP_TERRAIN_EXAGGERATION` | 1 |
| `MAP_CLIMB_MARGIN` | 0.20 |
| `MAP_MIN_SPAN_DEG` | 0.0014 |
| `CONTOUR_BASE_COLOR` | `#7a5733` |
| `CONTOUR_INDEX_COLOR` | `#5e3f1d` |
| `_maxChartPoints` | 4000 |
| `_maxSearchSteps` | 800 |
| `_minSegmentLength` | 300 |
| `DRAG_MIN_LEN_KM` | 0.05 |
| `DRAG_MIN_GAP_KM` | 0.001 |

### 22.3 Critères de validation

| Longueur | Pente min |
|---|---|
| ≥ 1000 m | 3 % |
| 750–1000 m | 4 % |
| 500–750 m | 5 % |
| 400–500 m | 6 % |
| 300–400 m | 7 % |
| < 300 m | rejeté |

### 22.4 Catégories de pente

Voir §5.1.

### 22.5 Checklist d'intégration v3.9.2

- [ ] Utilitaires TS portés (haversine, resample, plateau, rawStats, smoothing, slope)
- [ ] `resample.ts` avec **moyennage** + constante `RESAMPLE_AVG_MIN_M = 75`
- [ ] `despike.ts` avec `despikeWindowMToPts` + `applyDespike`
- [ ] `detection.ts` avec `computeAuto` + `mergedParts` préservés
- [ ] `climbConstraints.ts` avec `applyConstraints` + `applyDragConstraint`
- [ ] `insertion.ts` avec `computeInsertionSlots` + `computeInsertionBounds`
- [ ] **`climbGroups.ts`** avec `splitMergedClimb`, `refuseGroup`, `normalizeGroups`
- [ ] **`deleteClimb.ts`** avec `deleteClimbFromList`
- [ ] **`manualClimb.ts`** avec `isFreeRange`, `isFreeKm`, `createManualClimb`
- [ ] **`buildTraceFeatures.ts`** utilisant `rawPoints`
- [ ] **`buildMapMarkers.ts`** gérant mono + groupe
- [ ] **`zoomHelpers.ts`** avec `computeBounds`
- [ ] Store Pinia paramètres (15 + 2 toggles)
- [ ] Store Pinia données GPX (raw + resampled + eleFinal + avgWinM + despikeWinPtsEffective)
- [ ] Store Pinia montées (avec `mergedCount`, `mergedParts`, `fallback`, `groupId`, `subIndex`)
- [ ] Store Pinia map (token, basemap, ready, loading)
- [ ] Composable `useDiagnostics` avec règles v3.9.2
- [ ] Composable `useMapbox` avec auto-load token
- [ ] Composable `useProfileCanvas` avec échelles fixes + scroll centré
- [ ] Composable `useMarkerDrag` avec clamp à la vue
- [ ] Composable `useManualClimb` (double-clic + drag)
- [ ] Composable `useClimbGroups` (split/refuse)
- [ ] Commande Tauri `read_gpx_file`
- [ ] Commande Tauri `analyze_gpx` (sidecar Node.js v3.9.2)
- [ ] Composant `<FileDropZone>` avec info conversion points
- [ ] Composant `<DiagnosticPanel>`
- [ ] Composant `<ParametersForm>` (15 + 2 toggles)
- [ ] Composant `<ClimbsPanel>` avec `<ClimbCard>` et `<GroupBrace>`
- [ ] Composant `<MapPanel>` (init + layers)
- [ ] Composant `<ProfilePanel>` (canvas + scroll)
- [ ] **Ordre couches Mapbox** : trace → contours (beforeId)
- [ ] **Échelles profil fixes** : 50 / 10 m/px
- [ ] **Trace carte sur points bruts**
- [ ] **Multi-marqueurs sur groupe**
- [ ] **Bouton ✂ Séparer**
- [ ] **Bouton 🔗 Refusionner**
- [ ] **Bouton ✕ Supprimer**
- [ ] **Double-clic + drag pour créer**
- [ ] **Annulation création par Échap**
- [ ] **Auto-load carte si token présent**
- [ ] **Courbes de niveau optionnelles**
- [ ] **Refus création si chevauchement**
- [ ] **Normalisation des groupes orphelins**
- [ ] Tests unitaires sur 6 traces de référence
- [ ] Test d'invariant : aucune montée affichée sous son seuil
- [ ] Test d'invariant : `mergedParts` conservés après fusion
- [ ] Test d'invariant : échelles profil strictement constantes
- [ ] Test d'invariant : `buildTraceFeatures` utilise `rawPoints`

### 22.6 Ressources

- **GPX format** : https://www.topografix.com/GPX/1/1/
- **Mapbox GL JS** : https://docs.mapbox.com/mapbox-gl-js/
- **Mapbox terrain source** : https://docs.mapbox.com/mapbox-gl-js/example/3d-terrain/
- **Mapbox contours** : https://docs.mapbox.com/mapbox-gl-js/example/vector-source/
- **Vuetify 3** : https://vuetifyjs.com/en/
- **Tauri v2** : https://v2.tauri.app/
- **Pinia** : https://pinia.vuejs.org/
- **fast-xml-parser** : https://github.com/NaturalIntelligence/fast-xml-parser

---

**Fin du document d'intégration technique v3.9.2**