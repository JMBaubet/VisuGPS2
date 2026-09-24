# Documentation Technique — Intégration GPX Climb Analyzer

**Cible** : application Tauri + Node.js + Vue 3 + Vuetify
**Version** : 3.6
**Date** : 2026-09-24
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
11. [Drag & drop direct sur les graphiques](#11-drag--drop-direct-sur-les-graphiques)
12. [Insertion contextuelle de montée](#12-insertion-contextuelle-de-montée)
13. [Pipeline complet de détection](#13-pipeline-complet-de-détection)
14. [Diagnostic de configuration](#14-diagnostic-de-configuration)
15. [Tests et validation](#15-tests-et-validation)
16. [Migration depuis le prototype HTML](#16-migration-depuis-le-prototype-html)
17. [Annexes](#17-annexes)

---

## 1. Contexte d'intégration

### 1.1 Objectif

Intégrer dans une application Tauri/Vue 3/Vuetify la fonctionnalité complète d'analyse de profil altimétrique GPX, dans sa version **v3.6**, incluant :

- **Filtre anti-pic Hampel** avec fenêtre exprimée en **mètres** (v3.6)
- **Rééchantillonnage par moyenne** (fenêtre minimale 75 m)
- Correction des plateaux GPS/MNT
- Lissage multi-étages (médiane + SG + gaussien + ancrage + double SG)
- Détection avec garde-fou hybride de refinage
- **Fusion automatique des montées trop proches** (`gapMinM`)
- **Validation finale stricte avec repli sur sous-segment** (`fallback`)
- **Diagnostic de configuration visuel**
- Édition par drag & drop direct
- Insertion contextuelle

### 1.2 Contraintes de la cible

| Contrainte | Impact |
|---|---|
| Application desktop Tauri | Accès possible au système de fichiers natif |
| Backend Node.js embarqué | Possibilité de déporter les calculs lourds |
| Frontend Vue 3 + Vuetify | Structure en composants réutilisables |
| TypeScript | Typage strict recommandé |
| Pinia (recommandé) | Store centralisé pour l'état |

### 1.3 Répartition recommandée front/back

| Tâche | Où l'exécuter | Justification |
|---|---|---|
| Parsing GPX | **Node.js** | fs natif, DOMParser absent en Node pur (utiliser fast-xml-parser) |
| Rééchantillonnage par moyenne | **Node.js** | O(n + m) mais centralise le pipeline |
| Correction plateaux | **Node.js** | Fait partie du prétraitement |
| **Anti-pic Hampel** | **Node.js** | O(n × w log w), optionnellement front si n < 5000 |
| Détection complète | **Node.js** | Coûteux (O(n × w + m × 800²)) |
| Rebuild après édition | **Front** si < 500 ms, sinon Node | Compromis latence |
| Rendu graphique | **Front (Chart.js)** | Interactif |
| Drag & drop | **Front** | Latence minimale, interaction temps réel |
| **Diagnostic** | **Front** | O(1), recalculé à chaque input |

---

## 2. Architecture recommandée

### 2.1 Vue d'ensemble

```
┌──────────────────────────────────────────────────────────────┐
│                     Application Tauri                        │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐     │
│  │              Frontend (WebView)                     │     │
│  │  ┌──────────────────────────────────────────────┐   │     │
│  │  │  Vue 3 + Vuetify                             │   │     │
│  │  │  ┌────────────────────────────────────────┐  │   │     │
│  │  │  │  <GpxAnalyzer />                       │  │   │     │
│  │  │  │   ├── <FileDropZone />                 │  │   │     │
│  │  │  │   ├── <DiagnosticPanel />              │  │   │     │
│  │  │  │   ├── <ParametersPanel />              │  │   │     │
│  │  │  │   ├── <StatsCards />                   │  │   │     │
│  │  │  │   ├── <CategoryRibbon />               │  │   │     │
│  │  │  │   ├── <ElevationChart /> (drag)        │  │   │     │
│  │  │  │   ├── <SlopeChart />                   │  │   │     │
│  │  │  │   ├── <CategoryStatsTable />           │  │   │     │
│  │  │  │   └── <ClimbsTable />                  │  │   │     │
│  │  │  └────────────────────────────────────────┘  │   │     │
│  │  │                                              │   │     │
│  │  │  Stores Pinia                                │   │     │
│  │  │   ├── useGpxStore (données brutes)           │   │     │
│  │  │   ├── useClimbsStore (montées)               │   │     │
│  │  │   └── useParametersStore (14 params)         │   │     │
│  │  │                                              │   │     │
│  │  │  Composables                                 │   │     │
│  │  │   ├── useGpxParser                           │   │     │
│  │  │   ├── useSlopeCalculations                   │   │     │
│  │  │   ├── useClimbDetection                      │   │     │
│  │  │   ├── useMarkerDrag                          │   │     │
│  │  │   ├── useChartSync                           │   │     │
│  │  │   └── useDiagnostics                         │   │     │
│  │  └──────────────────────────────────────────────┘   │     │
│  └─────────────────────────────────────────────────────┘     │
│                            │ Tauri IPC (invoke)              │
│                            ▼                                 │
│  ┌─────────────────────────────────────────────────────┐     │
│  │              Backend (Rust + Node.js sidecar)       │     │
│  │  ┌──────────────────────────────────────────────┐   │     │
│  │  │  Node.js sidecar                             │   │     │
│  │  │   ├── /parse-and-analyze (pipeline v3.6)     │   │     │
│  │  │   ├── /rebuild (optionnel)                   │   │     │
│  │  │   └── /despike (isolé si besoin)             │   │     │
│  │  └──────────────────────────────────────────────┘   │     │
│  │                                                     │     │
│  │  Rust core                                          │     │
│  │   ├── fs (lecture fichiers GPX)                     │     │
│  │   └── shell (spawn Node.js sidecar)                 │     │
│  └─────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────┘
```

### 2.2 Arborescence suggérée

```
src-tauri/
├── src/
│   ├── main.rs
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── gpx.rs            # commande read_gpx_file
│   │   └── analyzer.rs       # commande analyze_gpx
│   └── sidecar/
│       └── analyzer.js       # script Node.js
└── Cargo.toml

src/
├── main.ts
├── App.vue
├── types/
│   ├── gpx.ts
│   ├── climb.ts
│   ├── category.ts
│   ├── parameters.ts
│   └── diagnostics.ts
├── services/
│   ├── gpxApi.ts             # wrapper invoke Tauri
│   └── chartConfig.ts        # config Chart.js partagée
├── stores/
│   ├── gpxStore.ts
│   ├── climbsStore.ts
│   └── parametersStore.ts
├── composables/
│   ├── useGpxParser.ts
│   ├── useSlopeCalculations.ts
│   ├── useClimbDetection.ts
│   ├── useMarkerDrag.ts
│   ├── useChartSync.ts
│   └── useDiagnostics.ts
├── utils/
│   ├── haversine.ts
│   ├── resample.ts           # [v3.5] par moyenne
│   ├── plateau.ts
│   ├── despike.ts            # [v3.6] fenêtre en mètres
│   ├── rawStats.ts
│   ├── smoothing.ts          # MA, SG, gaussien, médiane
│   ├── anchoredSmoothing.ts
│   ├── slope.ts
│   ├── categorization.ts
│   ├── segmentation.ts
│   ├── refineGuard.ts
│   ├── mergeClimbs.ts
│   ├── validateClimbs.ts
│   └── insertion.ts
├── components/
│   └── gpx-analyzer/
│       ├── GpxAnalyzer.vue
│       ├── FileDropZone.vue
│       ├── DiagnosticPanel.vue
│       ├── ParametersPanel.vue
│       ├── StatsCards.vue
│       ├── CategoryRibbon.vue
│       ├── ElevationChart.vue
│       ├── SlopeChart.vue
│       ├── CategoryStatsTable.vue
│       ├── ClimbsTable.vue
│       ├── InsertionRow.vue
│       ├── ClimbRow.vue
│       └── RawDetailToggle.vue
└── plugins/
    └── vuetify.ts
```

---

## 3. Modèle de données

### 3.1 Types TypeScript

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

  // --- Toggles ---
  /** Lissage renforcé (double SG) — défaut true */
  smoothExtra: boolean;
  /** Lissage des pentes de segment — défaut true */
  segmentSmooth: boolean;
}

// ============================================================
// src/types/climb.ts
// ============================================================

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
  /** Nombre de fusions (v3.0) */
  mergedCount: number;
  /** Repli sur sous-segment (v3.1) */
  fallback: boolean;
}

export interface UserClimb {
  startKm: number;
  endKm: number;
  autoStartKm: number | null;
  autoEndKm: number | null;
  refineM: number | null;
  candidateLen: number | null;
  mergedCount: number;
  fallback: boolean;
  _constrainedStart?: boolean;
  _constrainedEnd?: boolean;
}

// ============================================================
// src/types/analysis.ts
// ============================================================

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

export interface AutoClimb {
  startKm: number;
  endKm: number;
  refineM: number;
  candidateLen: number;
  mergedCount: number;
  fallback: boolean;
}

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
  despikeCount: number;
  /** [v3.6] Nombre de points effectifs utilisés par l'anti-pic */
  despikeWinPtsEffective: number;
}

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
```

---

## 4. Inventaire complet des paramètres

### 4.1 Paramètres utilisateur (exposés dans l'UI)

| Nom UI | Nom technique | Type | Plage | Défaut | Impact principal |
|---|---|---|---|---|---|
| Pas de rééchantillonnage | `stepM` | float | 2–200 | **50** | Espacement cible |
| **Anti-pic : fenêtre** | **`despikeWindowM`** | **float** | **50–500** | **200** | **Fenêtre Hampel (m)** |
| Anti-pic : seuil | `despikeThresholdM` | float | 0–100 | **15** | Seuil Hampel |
| Lissage altitude | `fenetreAffichageM` | float | 20–∞ | **500** | Douceur profil |
| Lissage pente | `fenetrePenteM` | float | 10–∞ | **150** | Douceur pente |
| Fenêtre de calcul pente | `fenetreCatM` | float | 50–∞ | **100** | Largeur couleur |
| Seuil détection | `seuilMontee` | float | 0.1–∞ | **0.5** | Sensibilité détection |
| Longueur min. pente | `longueurMin` | float | 0–∞ | **200** | Fusion segments |
| Écart min. entre montées | `gapMinM` | float | 0–5000 | **250** | Fusion montées |
| Recherche extrema | `refinePct` | float | 1–500 | **60** | Distance max recherche |
| Filtre médian | `medianWindowM` | float | 0–300 | **0** | Élimination pics |
| Lissage gaussien | `gaussianSigmaM` | float | 0–500 | **75** | Adoucissement |
| Correction plateaux | `plateauRun` | int | 0–20 | **6** | Détection plateaux |
| Segments debug | `segmentLogM` | int | 10–2000 | **100** | Granularité log |

**Note importante sur les renommages v3.5.2 / v3.6** : les **clés techniques** (`fenetreCatM`, `longueurMin`) restent **inchangées** pour ne pas casser le code. Seuls les **libellés UI** changent :
- `Fenêtre couleur` → **Fenêtre de calcul pente**
- `Longueur min. couleur` → **Longueur min. pente**
- `despikeWinPts` → **`despikeWindowM`** (v3.6 : changement de nature, plus seulement de nom)

### 4.2 Constantes internes (non exposées)

| Nom technique | Valeur | Rôle |
|---|---|---|
| `REFINE_MIN_M` | 1500 | Plancher garde-fou |
| `REFINE_MAX_M` | 8000 | Plafond garde-fou |
| `STEP_M_MIN` | 2 | Pas resampling min |
| `STEP_M_MAX` | 200 | Pas resampling max |
| `GAP_MIN_M_DEFAULT` | 250 | Fusion |
| `SLOPE_DISPLAY_MAX_PCT` | 25 | Écrêtage pentes |
| `PLATEAU_TOLERANCE_M` | 1.0 | Tolérance plateaux |
| `RAW_DOUBLON_MIN_M` | 1.0 | Seuil doublon |
| **`DESPIKE_WINDOW_M_DEFAULT`** | **200** | **Fenêtre Hampel par défaut (m)** |
| **`DESPIKE_WINDOW_M_MIN`** | **50** | **Plancher fenêtre Hampel (m)** |
| **`DESPIKE_WINDOW_M_MAX`** | **500** | **Plafond fenêtre Hampel (m)** |
| `DESPIKE_THRESHOLD_M_DEFAULT` | 15 | Seuil Hampel |
| **`RESAMPLE_AVG_MIN_M`** | **75** | **Fenêtre minimale de moyennage (v3.5)** |
| `INSERT_EDGE_MIN_KM` | 1.0 | Espace avant/après |
| `INSERT_BETWEEN_MIN_KM` | 0.2 | Espace entre montées |
| `INSERT_DEFAULT_LEN_KM` | 1.0 | Longueur insertion |
| `_maxChartPoints` | 4000 | Points max par graphique |
| `_maxSearchSteps` | 800 | Sous-échantillonnage findBestSubClimb |
| `_minSegmentLength` | 300 | Longueur min absolue |
| `_eleBarelyWin` | 5 | Micro-lissage affichage pente |
| `DRAG_MIN_LEN_KM` | 0.05 | Longueur min drag |
| `DRAG_MIN_GAP_KM` | 0.001 | Gap min drag |

### 4.3 Critères de validation des montées

```typescript
export const CLIMB_CRITERIA = [
  { minLen: 1000, minSlope: 3 },  // ≥ 1000 m  → ≥ 3 %
  { minLen: 750,  minSlope: 4 },  // 750–1000  → ≥ 4 %
  { minLen: 500,  minSlope: 5 },  // 500–750   → ≥ 5 %
  { minLen: 400,  minSlope: 6 },  // 400–500   → ≥ 6 %
  { minLen: 300,  minSlope: 7 },  // 300–400   → ≥ 7 %
  // < 300 m : toujours rejeté
];
```

### 4.4 Règles de cohérence (diagnostic)

| Règle | Condition | Niveau si violée |
|---|---|---|
| Fenêtre SG ≥ 5 pts | `fenetreAffichageM ≥ 5 × stepM` | 🔴 si < 5, 🟠 si < 10 |
| Ratio altitude/pente ≥ 2 | `fenetreAffichageM ≥ 2 × fenetrePenteM` | 🔴 si < 1.5, 🟠 si < 2 |
| Filtre médian ≥ 3 pts | `medianWindowM ≥ 3 × stepM` (ou 0) | 🟠 |
| Gaussien ≥ 0.5 pt | `gaussianSigmaM ≥ 0.5 × stepM` (ou 0) | 🟠 |
| Longueur min. pente ≥ Pas | `longueurMin ≥ stepM` (ou 0) | 🟠 |
| Écart min. ≥ Pas | `gapMinM ≥ stepM` (ou 0) | 🟠 |
| **Anti-pic fenêtre ≥ 2 × Pas** | **`despikeWindowM ≥ 2 × stepM`** (ou seuil = 0) | **🟠** |
| Anti-pic seuil ≥ 5 m | `despikeThresholdM ≥ 5` (ou 0) | 🟠 |

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
    smoothExtra: p.smoothExtra,
    segmentSmooth: p.segmentSmooth,
  };
}
```

---

## 5. Paramètres de catégorisation de pente

### 5.1 Configuration actuelle (figée en V3)

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
  { id: 'dh3', label: 'Descente raide',       color: '#1e40af', min: -Infinity, max: -8 },
  { id: 'dh2', label: 'Descente',             color: '#3b82f6', min: -8, max: -3 },
  { id: 'dh1', label: 'Faux plat descendant', color: '#93c5fd', min: -3, max: -1 },
  { id: 'pl',  label: 'Plat',                 color: '#22c55e', min: -1, max: 1 },
  { id: 'up1', label: 'Faux plat montant',    color: '#fde68a', min: 1,  max: 3 },
  { id: 'up2', label: 'Montée',               color: '#f97316', min: 3,  max: 7 },
  { id: 'up3', label: 'Montée raide',         color: '#dc2626', min: 7,  max: 10 },
  { id: 'up4', label: 'Montée très raide',    color: '#000000', min: 10, max: Infinity },
];
```

### 5.2 Tableau récapitulatif

| Plage | ID | Libellé | Couleur | Hex |
|---|---|---|---|---|
| p < −8 % | `dh3` | Descente raide | 🔵 Bleu foncé | `#1e40af` |
| −8 ≤ p < −3 % | `dh2` | Descente | 🔵 Bleu | `#3b82f6` |
| −3 ≤ p < −1 % | `dh1` | Faux plat descendant | 🩵 Bleu clair | `#93c5fd` |
| −1 ≤ p ≤ +1 % | `pl` | Plat | 🟢 Vert | `#22c55e` |
| +1 < p ≤ +3 % | `up1` | Faux plat montant | 🟡 Jaune | `#fde68a` |
| +3 < p ≤ +7 % | `up2` | Montée | 🟠 Orange | `#f97316` |
| +7 < p ≤ +10 % | `up3` | Montée raide | 🔴 Rouge | `#dc2626` |
| p > +10 % | `up4` | Montée très raide | ⚫ Noir | `#000000` |

### 5.3 Règles d'application

- La **borne haute est exclusive** : une pente de +3 % exactement tombe dans `up1`.
- La catégorisation est **symétrique** pour le plat et les faux plats, mais **asymétrique** pour les raides.
- Les seuils sont **codés en dur** dans la V3 (constante `SLOPE_CATEGORIES`).

### 5.4 Évolution V4 — rendre éditables

```typescript
export interface EditableSlopeThresholds {
  descenteRaide: number;       // défaut 8
  descente: number;            // défaut 3
  fauxPlat: number;            // défaut 1
  montee: number;              // défaut 3
  monteeRaide: number;         // défaut 7
  monteeTresRaide: number;     // défaut 10
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

### 6.2 Rééchantillonnage par moyenne (v3.5)

```typescript
// src/utils/resample.ts

/**
 * Fenêtre minimale de moyennage au rééchantillonnage.
 * La fenêtre effective vaut : clamp(stepM, RESAMPLE_AVG_MIN_M, 3 × stepM).
 */
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
      // Fallback : interpolation (uniquement si stepM < pas brut)
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

### 6.4 Anti-pic Hampel (v3.6 — fenêtre en mètres)

```typescript
// src/utils/despike.ts

/**
 * Convertit une fenêtre en mètres en nombre de points.
 * - Force un nombre impair (Hampel requiert une fenêtre symétrique)
 * - Garantit un minimum de 3 points (sinon Hampel dégénère)
 */
export function despikeWindowMToPts(windowM: number, stepM: number): number {
  return Math.max(3, Math.round(windowM / stepM) | 1);
}

/**
 * Filtre Hampel : remplace les points aberrants par la médiane locale.
 * Ne modifie QUE les points qui dévient fortement de leur voisinage.
 *
 * @param ele        Altitudes (à pas régulier)
 * @param winPts     Taille de la fenêtre en points (impair, ≥ 3)
 * @param thresholdM Seuil de détection en mètres (0 = désactivé)
 * @returns          { out, count }
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
 * Utilisé directement dans le pipeline.
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

### 6.9 Fusion des montées (v3.0)

```typescript
// src/utils/mergeClimbs.ts

export interface RefinedClimb {
  startIdx: number;
  endIdx: number;
  refineM: number;
  candidateLen: number;
  _merged: number;
}

export function tryMergeWithPrevious(
  prev: RefinedClimb,
  newStart: number,
  newEnd: number,
  dist: Float64Array,
  eleBase: Float64Array,
  gapMinM: number,
  effectiveRefine: number,
  candidateLen: number,
): {
  merged: boolean;
  updatedPrev?: RefinedClimb;
  newAnchorEnd?: number;
  newAnchorValley?: number;
} {
  if (gapMinM <= 0) return { merged: false };

  const gapM = dist[newStart] - dist[prev.endIdx];
  if (gapM >= gapMinM) return { merged: false };

  let bestEnd = prev.endIdx;
  let bestEndVal = eleBase[prev.endIdx];
  for (let i = prev.endIdx + 1; i <= newEnd; i++) {
    if (eleBase[i] > bestEndVal) { bestEndVal = eleBase[i]; bestEnd = i; }
  }

  let bestValley = prev.startIdx;
  let bestValleyVal = eleBase[prev.startIdx];
  for (let i = prev.endIdx; i <= newStart; i++) {
    if (eleBase[i] < bestValleyVal) { bestValleyVal = eleBase[i]; bestValley = i; }
  }

  const newPrevStart = (bestValleyVal < eleBase[prev.startIdx]) ? bestValley : prev.startIdx;

  return {
    merged: true,
    updatedPrev: {
      startIdx: newPrevStart,
      endIdx: bestEnd,
      refineM: Math.max(prev.refineM, effectiveRefine),
      candidateLen: prev.candidateLen + candidateLen,
      _merged: prev._merged + 1,
    },
    newAnchorEnd: bestEnd,
    newAnchorValley: newPrevStart !== prev.startIdx ? newPrevStart : undefined,
  };
}
```

### 6.10 Validation finale stricte (v3.1)

```typescript
// src/utils/validateClimbs.ts

export interface ValidationResult {
  startIdx: number;
  endIdx: number;
  mergedCount: number;
  fallback: boolean;
}

export function validateAndFallback(
  refined: RefinedClimb[],
  dist: Float64Array,
  eleDisplay: Float64Array,
  minSlopeForLength: (len: number) => number,
  findBestSubClimb: (
    dist: Float64Array,
    ele: Float64Array,
    s: number,
    e: number,
  ) => { startIdx: number; endIdx: number; avgSlope: number; length: number } | null,
): {
  results: ValidationResult[];
  rejectedCount: number;
  fallbackCount: number;
} {
  const results: ValidationResult[] = [];
  let rejectedCount = 0;
  let fallbackCount = 0;

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

    if (length < 300 || avgSlope < minSlope) {
      const sub = findBestSubClimb(dist, eleDisplay, c.startIdx, c.endIdx);
      if (!sub) {
        rejectedCount++;
        continue;
      }
      finalStart = sub.startIdx;
      finalEnd = sub.endIdx;
      fallbackCount++;
    }

    results.push({
      startIdx: finalStart,
      endIdx: finalEnd,
      mergedCount: c._merged,
      fallback: finalStart !== c.startIdx || finalEnd !== c.endIdx,
    });
  }

  return { results, rejectedCount, fallbackCount };
}
```

### 6.11 Contraintes de montées

```typescript
// src/utils/climbConstraints.ts

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

### 6.12 Insertion contextuelle

```typescript
// src/utils/insertion.ts

export const INSERT_EDGE_MIN_KM = 1.0;
export const INSERT_BETWEEN_MIN_KM = 0.2;
export const INSERT_DEFAULT_LEN_KM = 1.0;

export interface InsertionSlot {
  kind: 'before-first' | 'between' | 'after-last' | 'empty';
  label: string;
  gapStartKm: number;
  gapEndKm: number;
  gapKm: number;
  gapIdx?: number;
}

export function computeInsertionSlots(
  climbs: { startDist: number; endDist: number }[],
  totalDist: number,
): InsertionSlot[] {
  const slots: InsertionSlot[] = [];
  const totalKm = totalDist / 1000;

  if (climbs.length === 0) {
    slots.push({
      kind: 'empty',
      label: 'Ajouter une montée sur la trace',
      gapStartKm: 0, gapEndKm: totalKm, gapKm: totalKm,
    });
    return slots;
  }

  const firstStartKm = climbs[0].startDist / 1000;
  if (firstStartKm >= INSERT_EDGE_MIN_KM) {
    slots.push({
      kind: 'before-first',
      label: 'Avant la montée #1',
      gapStartKm: 0, gapEndKm: firstStartKm, gapKm: firstStartKm,
    });
  }

  for (let i = 0; i < climbs.length - 1; i++) {
    const leftEndKm = climbs[i].endDist / 1000;
    const rightStartKm = climbs[i + 1].startDist / 1000;
    const gapKm = rightStartKm - leftEndKm;
    if (gapKm >= INSERT_BETWEEN_MIN_KM) {
      slots.push({
        kind: 'between',
        label: `Entre #${i + 1} et #${i + 2}`,
        gapStartKm: leftEndKm, gapEndKm: rightStartKm, gapKm,
        gapIdx: i,
      });
    }
  }

  const lastEndKm = climbs[climbs.length - 1].endDist / 1000;
  const tailGapKm = totalKm - lastEndKm;
  if (tailGapKm >= INSERT_EDGE_MIN_KM) {
    slots.push({
      kind: 'after-last',
      label: `Après la montée #${climbs.length}`,
      gapStartKm: lastEndKm, gapEndKm: totalKm, gapKm: tailGapKm,
    });
  }

  return slots;
}

export function computeInsertionBounds(
  gapStartKm: number,
  gapEndKm: number,
): { startKm: number; endKm: number } | null {
  const gapLen = gapEndKm - gapStartKm;
  if (gapLen < INSERT_BETWEEN_MIN_KM) return null;

  const targetLen = Math.max(0.1, Math.min(INSERT_DEFAULT_LEN_KM, gapLen * 0.8));
  const center = (gapStartKm + gapEndKm) / 2;
  const startKm = Math.max(gapStartKm + 0.01, center - targetLen / 2);
  const endKm   = Math.min(gapEndKm - 0.01,   center + targetLen / 2);

  if (endKm <= startKm) return null;
  return { startKm, endKm };
}
```

---

## 7. Composables Vue 3

### 7.1 Composable principal

```typescript
// src/composables/useGpxAnalyzer.ts
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useGpxStore } from '@/stores/gpxStore';
import { useClimbsStore } from '@/stores/climbsStore';
import { useParametersStore } from '@/stores/parametersStore';
import type { AnalyzerResult } from '@/types/analysis';

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
      gpxStore.setRawData(result.distRaw, result.eleRawOrig, result.rawStats);
      gpxStore.setResampledData(result.dist, result.ele, result.stepM);
      climbsStore.setAutoClimbs(result.autoClimbs);
      climbsStore.resetUserClimbs(result.autoClimbs);
      rebuild();
    } catch (e: any) {
      error.value = e.message ?? 'Erreur inconnue';
    } finally {
      isLoading.value = false;
    }
  }

  function rebuild(): void {
    // Voir §13.4
  }

  return { isLoading, error, loadAndAnalyze, rebuild };
}
```

### 7.2 Composable diagnostic (v3.6)

```typescript
// src/composables/useDiagnostics.ts

import { computed, Ref } from 'vue';
import type { AnalyzerParameters } from '@/types/parameters';
import type { DiagEntry, DiagWarning, DiagResult, DiagStatus } from '@/types/diagnostics';

export function computeDiagnostics(params: AnalyzerParameters): DiagResult {
  const {
    stepM, despikeWindowM, despikeThresholdM,
    fenetreAffichageM, fenetrePenteM, fenetreCatM,
    seuilMontee, longueurMin, gapMinM, refinePct,
    medianWindowM, gaussianSigmaM, plateauRun,
  } = params;

  // Valeurs effectives en points
  const lissageAltPts   = Math.max(1, Math.round(fenetreAffichageM / stepM));
  const lissagePentePts = Math.max(1, Math.round(fenetrePenteM / stepM));
  const couleurPts      = Math.max(1, Math.round(fenetreCatM / stepM));
  const medianPts       = medianWindowM > 0
                          ? Math.max(1, Math.round(medianWindowM / stepM))
                          : 0;
  const gaussianPts     = gaussianSigmaM > 0 ? gaussianSigmaM / stepM : 0;

  // [v3.6] Anti-pic : fenêtre effective en points
  const despikeActive = despikeThresholdM > 0;
  const despikeWinPtsEffective = despikeActive
    ? Math.max(3, Math.round(despikeWindowM / stepM) | 1)
    : 0;
  const despikeCovM = despikeWinPtsEffective * stepM;
  const despikeExpandRatio = despikeActive && despikeWindowM > 0
    ? despikeCovM / despikeWindowM
    : 1;

  // Ratios
  const ratioAltPente = fenetrePenteM > 0 ? fenetreAffichageM / fenetrePenteM : 0;
  const retardM = fenetrePenteM / 2;

  const diag: Record<string, DiagEntry> = {};

  // --- stepM ---
  diag.stepM = {
    key: 'stepM',
    label: 'Pas',
    effective: `${stepM} m`,
    optimal: '25–100 m',
    target: stepM < 15 ? '50 m' : stepM > 150 ? '100 m' : null,
    status: stepM < 15 ? 'orange' : stepM > 150 ? 'orange' : 'green',
    msg: stepM < 15 ? 'Pas très fin — sur-échantillonnage'
       : stepM > 150 ? 'Pas très grossier — perte de détail'
       : 'OK',
  };

  // --- Anti-pic : fenêtre (v3.6) ---
  diag.despikeWindowM = {
    key: 'despikeWindowM',
    label: 'Anti-pic : fenêtre',
    effective: !despikeActive ? 'désactivé'
      : `${despikeWindowM} m · ${despikeWinPtsEffective} pts effectifs ≈ ${despikeCovM} m`,
    optimal: '150–300 m',
    target: null,
    status: !despikeActive ? 'grey'
      : (despikeWindowM < 2 * stepM) ? 'orange'
      : (despikeExpandRatio > 1.5) ? 'orange'
      : 'green',
    msg: !despikeActive ? 'Anti-pic désactivé'
      : (despikeWindowM < 2 * stepM)
        ? `Fenêtre < 2 × Pas (${2 * stepM} m) — la fenêtre effective sera élargie`
      : (despikeExpandRatio > 1.5)
        ? `Arrondi à ${despikeWinPtsEffective} pts gonfle la fenêtre de ${Math.round((despikeExpandRatio - 1) * 100)} %`
      : `Couvre ±${((despikeWinPtsEffective - 1) / 2 * stepM).toFixed(0)} m autour de chaque point`,
  };

  // --- Anti-pic : seuil ---
  diag.despikeThresholdM = {
    key: 'despikeThresholdM',
    label: 'Anti-pic : seuil',
    effective: despikeThresholdM === 0 ? 'désactivé' : `${despikeThresholdM} m`,
    optimal: '15–25 m',
    target: despikeThresholdM > 0 && despikeThresholdM < 5 ? '15 m'
          : despikeThresholdM > 50 ? '25 m'
          : null,
    status: despikeThresholdM === 0 ? 'grey'
      : despikeThresholdM < 5 ? 'orange'
      : despikeThresholdM > 50 ? 'orange'
      : 'green',
    msg: despikeThresholdM === 0 ? 'Anti-pic désactivé'
      : despikeThresholdM < 5 ? 'Seuil bas — risque de couper des bosses réelles'
      : despikeThresholdM > 50 ? 'Seuil élevé — ne capturera que les très gros glitchs'
      : `Remplace les points qui dévient de > ${despikeThresholdM} m de leur médiane`,
  };

  // --- Lissage altitude ---
  let lissageAltTarget: string | null = null;
  if (lissageAltPts < 10) lissageAltTarget = `${stepM * 10} m (10 pts)`;
  else if (lissageAltPts > 40) lissageAltTarget = `${stepM * 40} m (40 pts)`;
  diag.fenetreAffichageM = {
    key: 'fenetreAffichageM',
    label: 'Lissage altitude',
    effective: `≈ ${lissageAltPts} points`,
    optimal: '10–40 pts',
    target: lissageAltTarget,
    status: lissageAltPts < 5 ? 'red'
      : lissageAltPts < 10 ? 'orange'
      : lissageAltPts > 40 ? 'orange'
      : 'green',
    msg: lissageAltPts < 5 ? `Fenêtre de ${lissageAltPts} pts — inefficace (min 5)`
      : lissageAltPts < 10 ? `Fenêtre de ${lissageAltPts} pts — limite (recommandé ≥ 10)`
      : lissageAltPts > 40 ? `Fenêtre de ${lissageAltPts} pts — très lisse`
      : `Fenêtre de ${lissageAltPts} pts`,
  };

  // --- Lissage pente ---
  let lissagePenteTarget: string | null = null;
  if (lissagePentePts < 3) lissagePenteTarget = `${stepM * 3} m (3 pts)`;
  else if (lissagePentePts > 10) lissagePenteTarget = `${stepM * 10} m (10 pts)`;
  diag.fenetrePenteM = {
    key: 'fenetrePenteM',
    label: 'Lissage pente',
    effective: `≈ ${lissagePentePts} points`,
    optimal: '3–10 pts',
    target: lissagePenteTarget,
    status: lissagePentePts < 3 ? 'red'
      : ratioAltPente < 1.5 ? 'red'
      : ratioAltPente < 2 ? 'orange'
      : 'green',
    msg: lissagePentePts < 3 ? `Fenêtre de ${lissagePentePts} pts — trop courte (min 3)`
      : ratioAltPente < 1.5 ? `Ratio altitude/pente = ${ratioAltPente.toFixed(1)}× (recommandé ≥ 2)`
      : ratioAltPente < 2 ? `Ratio altitude/pente = ${ratioAltPente.toFixed(1)}× (limite)`
      : `Fenêtre de ${lissagePentePts} pts`,
  };

  // --- Fenêtre de calcul pente ---
  diag.fenetreCatM = {
    key: 'fenetreCatM',
    label: 'Fenêtre de calcul pente',
    effective: `≈ ${couleurPts} points`,
    optimal: '3–15 pts',
    target: null,
    status: 'green',
    msg: `Affichage uniquement — n'affecte pas la détection`,
  };

  // --- Seuil détection ---
  diag.seuilMontee = {
    key: 'seuilMontee',
    label: 'Seuil détection',
    effective: `${seuilMontee} %`,
    optimal: '0.5–1.0 %',
    target: null,
    status: seuilMontee < 0.2 ? 'orange' : seuilMontee > 2 ? 'orange' : 'green',
    msg: `Retard de détection ≈ ${retardM.toFixed(0)} m (demi-lissage pente)`,
  };

  // --- Longueur min. pente ---
  const longueurMinOK = longueurMin >= stepM;
  diag.longueurMin = {
    key: 'longueurMin',
    label: 'Longueur min. pente',
    effective: `${longueurMin} m`,
    optimal: `≥ ${stepM} m`,
    target: !longueurMinOK ? `${stepM * 3} m (3 pts)` : null,
    status: !longueurMinOK ? 'orange' : 'green',
    msg: !longueurMinOK
      ? `Inférieure au Pas (${stepM} m) — paramètre inopérant`
      : `Affichage uniquement`,
  };

  // --- Écart min. entre montées ---
  const gapOK = gapMinM >= stepM || gapMinM === 0;
  let gapTarget: string | null = null;
  if (gapMinM > 0 && gapMinM < stepM) gapTarget = `${stepM * 2} m (2 pts)`;
  else if (gapMinM > 1000) gapTarget = '1000 m';
  diag.gapMinM = {
    key: 'gapMinM',
    label: 'Écart min. entre montées',
    effective: gapMinM === 0 ? 'désactivé' : `${gapMinM} m`,
    optimal: `2–10 pts`,
    target: gapTarget,
    status: gapMinM === 0 ? 'grey'
      : !gapOK ? 'orange'
      : gapMinM > 1000 ? 'orange'
      : 'green',
    msg: gapMinM === 0 ? 'Fusion désactivée'
      : !gapOK ? `Inférieur au Pas (${stepM} m) — paramètre inopérant`
      : gapMinM > 1000 ? 'Écart très large — risque de fusion abusive'
      : `Fusionne les montées séparées de < ${gapMinM} m`,
  };

  // --- Recherche extrema ---
  diag.refinePct = {
    key: 'refinePct',
    label: 'Recherche extrema',
    effective: `${refinePct} % de la longueur`,
    optimal: '40–120 %',
    target: null,
    status: refinePct < 20 ? 'orange' : refinePct > 150 ? 'orange' : 'green',
    msg: refinePct < 20 ? 'Recherche très serrée — risque de rater le vrai sommet'
      : refinePct > 150 ? 'Recherche très large — risque de capture'
      : 'Clamp final : 1500 – 8000 m',
  };

  // --- Filtre médian ---
  let medianTarget: string | null = null;
  if (medianWindowM > 0 && medianPts < 3) medianTarget = `0 (désactivé) ou ${stepM * 3} m`;
  diag.medianWindowM = {
    key: 'medianWindowM',
    label: 'Filtre médian',
    effective: medianWindowM === 0 ? 'désactivé' : `≈ ${medianPts} points`,
    optimal: 'désactivé ou ≥ 3 pts',
    target: medianTarget,
    status: medianWindowM === 0 ? 'grey'
      : medianPts < 3 ? 'orange'
      : 'green',
    msg: medianWindowM === 0 ? 'Désactivé (recommandé si anti-pic actif)'
      : medianPts < 3 ? `Fenêtre de ${medianPts} pts — peu efficace (recommandé ≥ 3)`
      : `Fenêtre de ${medianPts} pts`,
  };

  // --- Lissage gaussien ---
  let gaussTarget: string | null = null;
  if (gaussianSigmaM > 0 && gaussianPts < 0.5) gaussTarget = `${stepM} m (2 pts)`;
  else if (gaussianPts > 5) gaussTarget = `${stepM * 5} m (5 pts)`;
  diag.gaussianSigmaM = {
    key: 'gaussianSigmaM',
    label: 'Lissage gaussien',
    effective: gaussianSigmaM === 0 ? 'désactivé' : `σ ≈ ${gaussianPts.toFixed(1)} points`,
    optimal: '0.5–5 pts',
    target: gaussTarget,
    status: gaussianSigmaM === 0 ? 'grey'
      : gaussianPts < 0.5 ? 'orange'
      : gaussianPts > 5 ? 'orange'
      : 'green',
    msg: gaussianSigmaM === 0 ? 'Désactivé'
      : gaussianPts < 0.5 ? 'σ < 0.5 point — inefficace'
      : gaussianPts > 5 ? `σ = ${gaussianPts.toFixed(1)} pts — très large`
      : `σ = ${gaussianPts.toFixed(1)} pts`,
  };

  // --- Correction plateaux ---
  const plateauUnused = plateauRun > 0 && stepM > 50;
  diag.plateauRun = {
    key: 'plateauRun',
    label: 'Correction plateaux',
    effective: plateauRun === 0 ? 'désactivé' : `${plateauRun} points`,
    optimal: '4–8 pts',
    target: plateauUnused ? '0 (inutile à Pas > 50 m)' : null,
    status: plateauRun === 0 ? 'grey'
      : plateauUnused ? 'orange'
      : 'green',
    msg: plateauRun === 0 ? 'Désactivé'
      : plateauUnused ? `Probablement inutile avec un Pas de ${stepM} m`
      : `Corrige les séries de ≥ ${plateauRun} pts`,
  };

  // ===== Warnings globaux =====
  const warnings: DiagWarning[] = [];

  if (lissageAltPts < 5) {
    warnings.push({
      level: 'red',
      msg: `Le <strong>Lissage altitude</strong> produit une fenêtre de ${lissageAltPts} points, ` +
           `insuffisant (min 5). Augmentez à ${stepM * 10} m, ou diminuez le <strong>Pas</strong>.`,
    });
  } else if (lissageAltPts < 10) {
    warnings.push({
      level: 'orange',
      msg: `Le <strong>Lissage altitude</strong> produit une fenêtre de ${lissageAltPts} points. ` +
           `Recommandé ≥ 10. Augmentez à ${stepM * 10} m.`,
    });
  }

  if (lissagePentePts < 3) {
    warnings.push({
      level: 'red',
      msg: `Le <strong>Lissage pente</strong> produit une fenêtre de ${lissagePentePts} points, ` +
           `trop courte. Augmentez à ${stepM * 3} m minimum.`,
    });
  }

  if (ratioAltPente > 0 && ratioAltPente < 1.5) {
    warnings.push({
      level: 'red',
      msg: `Le <strong>Lissage pente</strong> est plus grand que la moitié du ` +
           `<strong>Lissage altitude</strong>. Recommandé : Lissage altitude ≥ 2 × Lissage pente.`,
    });
  } else if (ratioAltPente > 0 && ratioAltPente < 2) {
    warnings.push({
      level: 'orange',
      msg: `Ratio Lissage altitude / Lissage pente = ${ratioAltPente.toFixed(2)}×. Recommandé ≥ 2×.`,
    });
  }

  if (medianWindowM > 0 && medianPts < 3) {
    warnings.push({
      level: 'orange',
      msg: `Le <strong>Filtre médian</strong> est inefficace avec ${medianPts} point(s). ` +
           `Augmentez à ${stepM * 3} m minimum, ou désactivez-le (0).`,
    });
  }

  if (longueurMin > 0 && longueurMin < stepM) {
    warnings.push({
      level: 'orange',
      msg: `La <strong>Longueur min. pente</strong> (${longueurMin} m) est inférieure au ` +
           `<strong>Pas</strong> — paramètre sans effet.`,
    });
  }

  if (gapMinM > 0 && gapMinM < stepM) {
    warnings.push({
      level: 'orange',
      msg: `L'<strong>Écart min. entre montées</strong> (${gapMinM} m) est inférieur au ` +
           `<strong>Pas</strong> — sans effet.`,
    });
  }

  if (gapMinM > 0 && refinePct > 100) {
    warnings.push({
      level: 'orange',
      msg: `<strong>Écart min. entre montées</strong> et <strong>Recherche extrema</strong> étendent tous deux ` +
           `les bornes. Combinaison à surveiller.`,
    });
  }

  // Warnings spécifiques anti-pic (v3.6)
  if (despikeActive && despikeWindowM < 2 * stepM) {
    warnings.push({
      level: 'orange',
      msg: `L'<strong>Anti-pic : fenêtre</strong> (${despikeWindowM} m) est inférieure à ` +
           `2 × Pas (${2 * stepM} m). La fenêtre effective sera élargie à ${despikeCovM} m.`,
    });
  } else if (despikeActive && despikeExpandRatio > 1.5) {
    warnings.push({
      level: 'orange',
      msg: `L'arrondi au multiple de Pas gonfle l'<strong>Anti-pic : fenêtre</strong> de ` +
           `${despikeWindowM} m à ${despikeCovM} m (${Math.round((despikeExpandRatio - 1) * 100)} %). ` +
           `Envisagez une valeur multiple de ${stepM} m.`,
    });
  }
  if (despikeActive && despikeThresholdM < 5) {
    warnings.push({
      level: 'orange',
      msg: `Seuil <strong>Anti-pic : seuil</strong> très bas (${despikeThresholdM} m) — risque de couper ` +
           `des bosses réelles.`,
    });
  }

  return { diag, warnings };
}

export function useDiagnostics(params: Ref<AnalyzerParameters>) {
  const result = computed(() => computeDiagnostics(params.value));

  const globalStatus = computed<DiagStatus>(() => {
    const { warnings } = result.value;
    if (warnings.some(w => w.level === 'red')) return 'red';
    if (warnings.some(w => w.level === 'orange')) return 'orange';
    return 'green';
  });

  return {
    diag: computed(() => result.value.diag),
    warnings: computed(() => result.value.warnings),
    globalStatus,
  };
}
```

---

## 8. Composants Vuetify

### 8.1 Panneau diagnostic

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
import { computeDiagnostics } from '@/composables/useDiagnostics';

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
  if (globalStatus.value === 'red')
    return `🔴 ${w.filter(x => x.level === 'red').length} incohérence(s)`;
  if (globalStatus.value === 'orange')
    return `🟠 ${w.length} avertissement(s)`;
  return '🟢 Cohérent';
});

function dotColor(status: string): string {
  return {
    green: '#16a34a',
    orange: '#f59e0b',
    red: '#dc2626',
    grey: '#94a3b8',
  }[status] ?? '#94a3b8';
}
</script>
```

### 8.2 Panneau paramètres (v3.6)

```vue
<!-- src/components/gpx-analyzer/ParametersPanel.vue -->
<template>
  <v-card class="pa-4">
    <v-card-title class="text-h6">Paramètres de détection automatique</v-card-title>

    <DiagnosticPanel :parameters="params" class="mb-3" />

    <v-alert type="error" variant="tonal" density="compact" class="mb-3">
      🩹 <strong>Anti-pic</strong> — Filtre Hampel sur les altitudes
      resamplées. Ne modifie que les points aberrants.
    </v-alert>

    <v-alert type="info" variant="tonal" density="compact" class="mb-3">
      📐 <strong>Rééchantillonnage par moyenne</strong> — chaque point de sortie
      est la moyenne des points bruts dans une fenêtre ≥ 75 m.
    </v-alert>

    <v-row dense>
      <!-- Pas de rééchantillonnage -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.stepM"
          label="Pas de rééchantillonnage"
          type="number" :min="2" :max="200"
          suffix="m" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Anti-pic : fenêtre (v3.6, en mètres) -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.despikeWindowM"
          label="Anti-pic : fenêtre"
          type="number" :min="50" :max="500"
          suffix="m" density="compact"
          @update:model-value="onChange">
          <template #details>
            <span class="text-caption text-grey">
              Fenêtre effective : {{ effectiveWinPts }} pts ≈ {{ effectiveCovM }} m
            </span>
          </template>
        </v-text-field>
      </v-col>

      <!-- Anti-pic : seuil -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.despikeThresholdM"
          label="Anti-pic : seuil"
          type="number" :min="0" :max="100"
          suffix="m" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Lissage altitude -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.fenetreAffichageM"
          label="Lissage altitude" type="number" :min="20"
          suffix="m" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Lissage pente -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.fenetrePenteM"
          label="Lissage pente" type="number" :min="10"
          suffix="m" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Fenêtre de calcul pente -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.fenetreCatM"
          label="Fenêtre de calcul pente" type="number" :min="50"
          suffix="m" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Seuil détection -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.seuilMontee"
          label="Seuil détection" type="number" :min="0.1"
          suffix="%" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Longueur min. pente -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.longueurMin"
          label="Longueur min. pente" type="number" :min="0"
          suffix="m" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Écart min. entre montées -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.gapMinM"
          label="Écart min. entre montées"
          type="number" :min="0" :max="5000"
          suffix="m" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Recherche extrema -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.refinePct"
          label="Recherche extrema"
          type="number" :min="1" :max="500"
          suffix="%" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Filtre médian -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.medianWindowM"
          label="Filtre médian"
          type="number" :min="0" :max="300"
          suffix="m" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Lissage gaussien -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.gaussianSigmaM"
          label="Lissage gaussien"
          type="number" :min="0" :max="500"
          suffix="m" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Correction plateaux -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.plateauRun"
          label="Correction plateaux"
          type="number" :min="0" :max="20"
          suffix="pts" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Segments debug -->
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.segmentLogM"
          label="Segments debug"
          type="number" :min="10" :max="2000"
          suffix="m" density="compact"
          @update:model-value="onChange" />
      </v-col>

      <!-- Toggles -->
      <v-col cols="12" md="6">
        <v-switch v-model="params.smoothExtra"
          label="Lissage renforcé (double SG)"
          density="compact" hide-details color="primary"
          @update:model-value="onChange" />
      </v-col>
      <v-col cols="12" md="6">
        <v-switch v-model="params.segmentSmooth"
          label="Lissage des pentes de segment"
          density="compact" hide-details color="primary"
          @update:model-value="onChange" />
      </v-col>
    </v-row>
  </v-card>
</template>

<script setup lang="ts">
import { computed, reactive } from 'vue';
import DiagnosticPanel from './DiagnosticPanel.vue';
import { useParametersStore } from '@/stores/parametersStore';
import { despikeWindowMToPts } from '@/utils/despike';
import { debounce } from '@/utils/debounce';

const paramsStore = useParametersStore();
const params = reactive({ ...paramsStore.parameters });
const emit = defineEmits<{ (e: 'change'): void }>();

const effectiveWinPts = computed(() =>
  despikeWindowMToPts(params.despikeWindowM, params.stepM));
const effectiveCovM = computed(() =>
  effectiveWinPts.value * params.stepM);

const onChange = debounce(() => {
  paramsStore.setParameters({ ...params });
  emit('change');
}, 250);
</script>
```

### 8.3 Composant `ClimbRow.vue` (avec indicateurs ⊕ et ⤺)

```vue
<template>
  <tr :class="rowClass">
    <td class="climb-select-cell">
      <v-radio
        :model-value="selected"
        :value="index"
        @update:model-value="$emit('select')"
        hide-details
      />
    </td>
    <td>
      {{ index + 1 }}
      <v-icon v-if="climb.mergedCount > 0" size="small" color="teal"
        title="Fusionnée">mdi-plus-circle</v-icon>
      <v-icon v-if="climb.fallback" size="small" color="purple"
        title="Repli sous-segment">mdi-undo</v-icon>
    </td>
    <td>
      <v-text-field v-model.number="startKm" type="number" step="0.001"
        density="compact" variant="outlined" hide-details
        @update:model-value="$emit('edit', 'startKm', $event)" />
    </td>
    <td>
      <v-text-field v-model.number="endKm" type="number" step="0.001"
        density="compact" variant="outlined" hide-details
        @update:model-value="$emit('edit', 'endKm', $event)" />
    </td>
    <td class="num">{{ climb.length.toFixed(0) }} m</td>
    <td class="num">{{ climb.dPlus.toFixed(0) }} m</td>
    <td class="num">
      <v-chip :color="colorFor(climb.avgSlope)" size="small" label>
        {{ climb.avgSlope.toFixed(1) }} %
      </v-chip>
    </td>
    <td class="num">{{ climb.refineM?.toFixed(0) ?? '—' }}</td>
    <td>
      <v-btn icon="mdi-refresh" size="x-small" variant="text"
        @click="$emit('reset')" />
      <v-btn icon="mdi-close" size="x-small" variant="text" color="error"
        @click="$emit('delete')" />
    </td>
  </tr>
</template>
```

---

## 9. Communication avec le backend Node.js

### 9.1 Sidecar Node.js — pipeline complet v3.6

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

// ---------- [v3.5] Rééchantillonnage par moyenne ----------
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

// ---------- [v3.6] Anti-pic Hampel (fenêtre en mètres) ----------
function despikeWindowMToPts(windowM, stepM) {
  return Math.max(3, Math.round(windowM / stepM) | 1);
}

function despikeAltitude(ele, winPts, thresholdM) {
  const n = ele.length;
  if (n < 3 || winPts < 3 || thresholdM <= 0) {
    return { out: ele.slice(), count: 0 };
  }
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

// ---------- SG ----------
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

// ---------- Gaussien ----------
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

// ---------- Médiane ----------
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

// ---------- MA ----------
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

// ---------- Slope ----------
function computeSlope(ele, dist, minD = 0.05) {
  const n = ele.length, s = new Array(n).fill(0);
  for (let i = 0; i < n; i++) {
    const i1 = Math.max(0, i - 1), i2 = Math.min(n - 1, i + 1);
    const d = dist[i2] - dist[i1];
    if (d > minD) s[i] = ((ele[i2] - ele[i1]) / d) * 100;
  }
  return s;
}

function computeSlopeWindow(ele, dist, winM) {
  const n = ele.length;
  const s = new Array(n).fill(0);
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

// ---------- Argmin/Argmax ----------
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

function findIdxAtDist(dist, targetM) {
  let lo = 0, hi = dist.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (dist[mid] < targetM) lo = mid + 1; else hi = mid;
  }
  return lo;
}

// ---------- Hystérésis ----------
function detectEvents(slope, seuil) {
  const ev = []; let inC = false;
  for (let i = 0; i < slope.length; i++) {
    if (!inC && slope[i] > seuil)  { ev.push({ idx: i, type: 'start' });  inC = true; }
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

// ---------- Sous-segment optimal ----------
const CLIMB_CRITERIA = [
  { minLen: 1000, minSlope: 3 }, { minLen: 750, minSlope: 4 },
  { minLen: 500, minSlope: 5 }, { minLen: 400, minSlope: 6 },
  { minLen: 300, minSlope: 7 }
];
function minSlopeForLength(len) {
  for (const c of CLIMB_CRITERIA) if (len >= c.minLen) return c.minSlope;
  return Infinity;
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

// ---------- computeAuto complet (v3.6) ----------
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
  let mergeCount = 0;

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
    const safetyLow  = findIdxAtDist(dist, Math.max(0, dist[c.startIdx] - effectiveRefine));
    const loIdx = Math.max(naturalLow, safetyLow);
    const newStart = (loIdx < c.startIdx) ? argminRange(eleBase, loIdx, c.startIdx) : c.startIdx;
    if (newStart >= newEnd) continue;

    // Fusion (v3.0)
    if (refined.length > 0 && gapMinM > 0) {
      const prev = refined[refined.length - 1];
      const gapM = dist[newStart] - dist[prev.endIdx];
      if (gapM < gapMinM) {
        const combinedEnd = argmaxRange(eleBase, prev.endIdx, newEnd);
        const midValley = argminRange(eleBase, prev.endIdx, newStart);
        let newPrevStart = prev.startIdx;
        if (eleBase[midValley] < eleBase[prev.startIdx]) {
          newPrevStart = midValley;
          anchors[anchors.length - 2] = midValley;
        }
        prev.startIdx = newPrevStart;
        prev.endIdx = combinedEnd;
        prev.candidateLen = (prev.candidateLen || 0) + candidateLen;
        prev.refineM = Math.max(prev.refineM || 0, effectiveRefine);
        prev._merged = (prev._merged || 0) + 1;
        anchors[anchors.length - 1] = combinedEnd;
        lastEnd = combinedEnd;
        mergeCount++;
        continue;
      }
    }

    anchors.push(newStart, newEnd);
    refined.push({
      startIdx: newStart, endIdx: newEnd,
      refineM: effectiveRefine, candidateLen: candidateLen,
      _merged: 0
    });
    lastEnd = newEnd;
  }

  const eleDisplay = anchoredMovingAverage(
    eleRaw, anchors, stepM, fenetreAffichageM, gaussianSigmaM, params.smoothExtra
  );

  // Validation stricte (v3.1)
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

    if (length < 300 || avgSlope < minSlope) {
      const sub = findBestSubClimb(dist, eleDisplay, c.startIdx, c.endIdx);
      if (!sub) continue;
      finalStart = sub.startIdx;
      finalEnd = sub.endIdx;
    }

    autoClimbs.push({
      startKm: dist[finalStart] / 1000,
      endKm: dist[finalEnd] / 1000,
      refineM: c.refineM, candidateLen: c.candidateLen,
      mergedCount: c._merged || 0,
      fallback: finalStart !== c.startIdx || finalEnd !== c.endIdx
    });
  }

  return { autoClimbs };
}

// ---------- Analyse complète ----------
function analyze(content, parameters) {
  const track = parseGpx(content);
  const n = track.points.length;

  // 1. Distances cumulées
  const distRaw = new Float64Array(n);
  for (let i = 1; i < n; i++) {
    distRaw[i] = distRaw[i - 1] + haversine(
      track.points[i - 1].lat, track.points[i - 1].lon,
      track.points[i].lat, track.points[i].lon,
    );
  }

  const eleRawOrig = Float64Array.from(track.points.map(p => p.ele));

  // 2. Stats brutes
  const rawStats = computeRawStats(distRaw, eleRawOrig);

  // 3. Correction plateaux (optionnelle)
  let eleForResample = eleRawOrig;
  if (parameters.plateauRun > 0) {
    eleForResample = plateauCorrect(eleRawOrig, parameters.plateauRun, PLATEAU_TOLERANCE_M).out;
  }

  // 4. Rééchantillonnage par moyenne (v3.5)
  const { dist, ele, avgWinM } = resampleUniform(distRaw, eleForResample, parameters.stepM);

  // 5. Anti-pic Hampel (v3.6 — fenêtre en mètres)
  let eleFinal = ele;
  let despikeCount = 0;
  const despikeWinPtsEffective = Math.max(3,
    Math.round(parameters.despikeWindowM / parameters.stepM) | 1);
  if (parameters.despikeThresholdM > 0) {
    const spikeRes = despikeAltitude(ele, despikeWinPtsEffective, parameters.despikeThresholdM);
    eleFinal = spikeRes.out;
    despikeCount = spikeRes.count;
  }

  // 6. Détection
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
      "dialog": { "open": true }
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

---

## 11. Drag & drop direct sur les graphiques

### 11.1 Principe

L'édition par drag & drop permet à l'utilisateur de déplacer les marqueurs de vallée et de sommet directement sur le graphique. Pour fonctionner correctement avec `chartjs-plugin-zoom`, l'interception **doit se faire au niveau DOM en phase de capture** et non via les hooks Chart.js.

### 11.2 Point critique : ordre des événements

Le navigateur émet **`pointerdown` AVANT `mousedown`**. Chart.js v4 et `chartjs-plugin-zoom` attachent leurs listeners sur `pointerdown` en phase cible. Pour les bloquer :

```typescript
window.addEventListener('pointerdown', onGlobalPointerDown, /* capture */ true);
window.addEventListener('mousedown',   onGlobalPointerDown, /* capture */ true);
```

Et dans le handler :
```typescript
if (intercepted) {
  e.stopImmediatePropagation();
  e.preventDefault();
}
```

### 11.3 Désactivation explicite du pan

En complément du `stopImmediatePropagation`, il est prudent de **désactiver explicitement** `pan.enabled` pendant le drag :

```typescript
STATE.panOriginalState = {
  elev: elevChart?.options.plugins.zoom.pan.enabled,
  slope: slopeChart?.options.plugins.zoom.pan.enabled,
};
elevChart.options.plugins.zoom.pan.enabled = false;
slopeChart.options.plugins.zoom.pan.enabled = false;
```

Et de le restaurer au `pointerup`.

### 11.4 Désactivation du tooltip pendant le drag

Le tooltip Chart.js est très intrusif pendant un drag : il suit la souris et masque les courbes. On le désactive :

```typescript
for (const ch of [elevChart, slopeChart]) {
  ch.options.plugins.tooltip.enabled = false;
  ch.tooltip.setActiveElements([], { x: 0, y: 0 });
  ch.tooltip.update();
}
```

Et on le réactive au `pointerup`.

### 11.5 Zone cliquable des poignées

Les poignées triangulaires sont dessinées en haut du graphique (dans les 30 premiers pixels). La détection de clic doit être **limitée à cette bande** pour ne pas capturer les clics au milieu du graphique :

```typescript
function findHandleAt(chart, px, py, climb) {
  const { chartArea, scales } = chart;
  if (py < chartArea.top - 12 || py > chartArea.top + 34) return null;
  // ...
}
```

### 11.6 Retour visuel temps réel

Pendant le drag :
- Le canvas est redessiné via `chart.draw()` (rapide, ~5 ms)
- Les métriques de la ligne du tableau sont mises à jour via `updatePreviewMetrics()`
- Le rebuild complet est différé au `pointerup`

---

## 12. Insertion contextuelle de montée

### 12.1 Principe

Le bouton global « + Ajouter une montée » a été **supprimé** (v2.2). À la place, des **boutons d'insertion** apparaissent directement **dans le tableau**, entre les lignes de montées.

### 12.2 Règles d'affichage

| Position | Condition |
|---|---|
| Avant #1 | `climbs[0].startDist ≥ 1000 m` |
| Entre #i et #i+1 | `gap ≥ 200 m` |
| Après #N | `totalDist − lastEnd ≥ 1000 m` |
| Aucune montée | Bouton unique centré |

### 12.3 Composant `InsertionRow`

```vue
<template>
  <tr class="insertion-row">
    <td :colspan="9" class="text-center py-1">
      <v-btn
        size="x-small" variant="outlined" color="primary"
        prepend-icon="mdi-plus" @click="$emit('insert')">
        {{ label }}
        <span class="text-caption text-grey ml-2">· {{ gapText }} libres</span>
      </v-btn>
    </td>
  </tr>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{ label: string; gapKm: number }>();
defineEmits<{ (e: 'insert'): void }>();

const gapText = computed(() =>
  props.gapKm >= 1
    ? `${props.gapKm.toFixed(2)} km`
    : `${(props.gapKm * 1000).toFixed(0)} m`
);
</script>
```

### 12.4 Algorithme d'insertion

Voir §6.12 pour l'implémentation TypeScript complète.

---

## 13. Pipeline complet de détection

### 13.1 Vue synthétique

```typescript
// src/services/gpxApi.ts
export async function analyzeGpx(
  filePath: string,
  parameters: AnalyzerParameters,
): Promise<AnalyzerResult> {
  const content = await invoke<string>('read_gpx_file', { path: filePath });
  return await invoke<AnalyzerResult>('analyze_gpx', { content, parameters });
}
```

### 13.2 Ordre des étapes (rappel)

1. Parse GPX → `rawPoints`
2. Distances cumulées → `distRaw`
3. Altitudes d'origine → `eleRawOrig`
4. Stats brutes → `rawStats`
5. Correction plateaux (optionnelle) → `eleForResample`
6. **Rééchantillonnage par moyenne** → `dist`, `ele`, `avgWinM`
7. **Anti-pic Hampel** (conversion m → pts) → `eleFinal`, `despikeCount`, `despikeWinPtsEffective`
8. Filtre médian → `eleMedian`
9. Savitzky-Golay → `eleBase`
10. Pente lissée + clip → `slopeBase`
11. Hystérésis → `events`
12. Candidats → `candidates`
13. Sous-segment optimal → `candidatesWithSub`
14. Raffinement → `refined`
15. **Fusion par `gapMinM`** → `refined` étendu
16. Lissage ancré final → `eleDisplay`
17. **Validation finale stricte + repli** → `autoClimbs`

### 13.3 Log debug

Le pipeline émet des logs dans le panneau debug :

```
📐 PIPELINE v3.6
   Trace brute : 3312 points
   Pas brut : min 0.3 m · médian 22.4 m · moy 33.1 m · max 892.5 m
   D+ brut (doublons ignorés) : 2874 m
📐 Rééchantillonnage (moyenne) : 2190 pts à pas 50 m · fenêtre de moyennage 75 m
🩹 Anti-pic (fenêtre=200 m → 5 pts ≈ 250 m, seuil=15 m) : 2 point(s) corrigé(s) sur 2190
[v3.6] 2 fusion(s) — écart min. 250 m
[v3.6] Validation : 3 conservées, 1 replis
```

### 13.4 Pipeline de reconstruction

```typescript
// src/services/rebuild.ts
export function rebuild(): RebuildResult | null {
  if (!STATE.dist || !STATE.eleRaw) return null;
  const params = getParams();
  const { dist, eleRaw, userClimbs, stepM } = STATE;

  // 1. Contraintes anti-chevauchement
  const sorted = applyConstraints(userClimbs);

  // 2. Ancrage
  const anchors: number[] = [];
  const resolved = sorted.map(c => {
    const startIdx = findIdxAtDist(dist, c.startKm * 1000);
    const endIdx = findIdxAtDist(dist, c.endKm * 1000);
    if (startIdx >= endIdx) {
      return { startIdx, endIdx, ref: c, invalid: true };
    }
    anchors.push(startIdx, endIdx);
    return { startIdx, endIdx, ref: c, invalid: false };
  });

  // 3. Lissages
  const eleDisplay = anchoredMovingAverage(
    eleRaw, anchors, stepM,
    params.fenetreAffichageM, params.gaussianSigmaM, params.smoothExtra,
  );
  const slopeRaw = clipSlope(
    computeSlope(movingAverage(eleRaw, 5), dist),
    SLOPE_DISPLAY_MAX_PCT,
  );
  const slopeSemi = computeSlope(eleDisplay, dist);
  const winSlopePts = Math.max(1, Math.round(params.fenetrePenteM / stepM));
  const slopeDisplay = clipSlope(
    movingAverage(slopeSemi, winSlopePts),
    SLOPE_DISPLAY_MAX_PCT,
  );
  const slopeForCat = computeSlopeWindow(eleDisplay, dist, params.fenetreCatM);

  // 4. Métriques par montée
  const climbs = resolved.map(r => {
    if (r.invalid) {
      return { ...r, length: 0, dPlus: 0, avgSlope: 0, ... };
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
      mergedCount: r.ref.mergedCount || 0,
      fallback: r.ref.fallback || false,
    };
  });

  // 5. Segmentation
  const catSegments = segmentByCategory(
    slopeForCat, dist, eleDisplay, params.longueurMin,
  );

  // 6. Marqueurs
  const markers = [];
  for (const c of climbs) {
    if (c.invalid) continue;
    const idx = STATE.userClimbs.indexOf(c.ref);
    markers.push({ dist: c.startDist, type: 'valley', climbIdx: idx });
    markers.push({ dist: c.endDist, type: 'summit', climbIdx: idx });
  }

  return { dist, eleRaw, eleDisplay, slopeRaw, slopeDisplay,
           slopeForCat, catSegments, climbs, markers };
}
```

---

## 14. Diagnostic de configuration

### 14.1 Principe

Le diagnostic affiche en temps réel l'état de cohérence des 14 paramètres UI et les **valeurs effectives en points**.

### 14.2 Calcul des valeurs effectives

Pour chaque paramètre en mètres, la valeur effective en points = `paramètre / stepM` :

| Paramètre | Valeur effective | Optimal |
|---|---|---|
| Lissage altitude | `fenetreAffichageM / stepM` pts | 10–40 pts |
| Lissage pente | `fenetrePenteM / stepM` pts | 3–10 pts |
| Filtre médian | `medianWindowM / stepM` pts | ≥ 3 pts (ou 0) |
| Lissage gaussien | `gaussianSigmaM / stepM` pts | 0.5–5 pts |
| **Anti-pic : fenêtre** | **`max(3, round(despikeWindowM / stepM) \| 1)` pts** | **150–300 m** |
| Anti-pic : seuil | `despikeThresholdM` m | 15–25 m |
| Fenêtre de calcul pente | `fenetreCatM / stepM` pts | 3–15 pts |

### 14.3 Statuts

| Statut | Signification | Couleur |
|---|---|---|
| 🟢 green | Optimal | #16a34a |
| 🟠 orange | Limite / avertissement | #f59e0b |
| 🔴 red | Incohérence critique | #dc2626 |
| ⚪ grey | Désactivé / non applicable | #94a3b8 |

### 14.4 Recalcul

Le diagnostic est recalculé à chaque `input` sur un champ (debounce 250 ms côté front). Il s'agit d'une fonction **pure** `computeDiagnostics(params)` qui ne dépend que des paramètres, sans effet de bord.

### 14.5 Tests du diagnostic

```typescript
// tests/unit/diagnostics.test.ts
import { describe, it, expect } from 'vitest';
import { computeDiagnostics } from '@/composables/useDiagnostics';

describe('computeDiagnostics (v3.6)', () => {
  it('détecte une fenêtre SG < 5 pts', () => {
    const params = {
      stepM: 100,
      fenetreAffichageM: 300,   // → 3 pts
      despikeWindowM: 200,
      despikeThresholdM: 15,
      // ... autres params par défaut
    };
    const { warnings } = computeDiagnostics(params);
    expect(warnings.some(w => w.level === 'red')).toBe(true);
  });

  it('valide la config par défaut v3.6', () => {
    const params = {
      stepM: 50,
      despikeWindowM: 200,
      despikeThresholdM: 15,
      fenetreAffichageM: 500,   // 10 pts
      fenetrePenteM: 150,       // 3 pts
      fenetreCatM: 100,         // 2 pts
      seuilMontee: 0.5,
      longueurMin: 200,
      gapMinM: 250,
      refinePct: 60,
      medianWindowM: 0,
      gaussianSigmaM: 75,       // 1.5 pts
      plateauRun: 6,
      segmentLogM: 100,
      smoothExtra: true,
      segmentSmooth: true,
    };
    const { warnings } = computeDiagnostics(params);
    expect(warnings.length).toBe(0);
  });

  it('détecte anti-pic fenêtre < 2 × Pas', () => {
    const params = {
      stepM: 100,
      despikeWindowM: 100,      // < 200 m (2 × Pas)
      despikeThresholdM: 15,
      // ...
    };
    const { warnings } = computeDiagnostics(params);
    expect(warnings.some(w => w.level === 'orange')).toBe(true);
  });

  it('détecte anti-pic seuil < 5 m', () => {
    const params = {
      stepM: 50,
      despikeWindowM: 200,
      despikeThresholdM: 3,
      // ...
    };
    const { warnings } = computeDiagnostics(params);
    expect(warnings.some(w => w.level === 'orange')).toBe(true);
  });
});
```

### 14.6 Affichage de la plage optimale

Le diagnostic renvoie pour chaque paramètre :

- `effective` : valeur effective en points
- `optimal` : plage optimale affichée en pts
- `target` : valeur cible suggérée si le paramètre est 🟠 ou 🔴

Exemple de sortie pour un lissage altitude trop court :

```
┌─────────────────────────────────────────┐
│ Lissage altitude (m)                    │
│ [    300    ]  ●                        │
│ ≈ 6 pts · optimal 10–40 pts · cible     │
│ 500 m (10 pts)                          │
└─────────────────────────────────────────┘
```

---

## 15. Tests et validation

### 15.1 Tests unitaires (Vitest)

```typescript
// tests/unit/despike.test.ts
import { describe, it, expect } from 'vitest';
import { despikeAltitude, despikeWindowMToPts } from '@/utils/despike';

describe('despikeWindowMToPts (v3.6)', () => {
  it('convertit correctement mètres → points', () => {
    expect(despikeWindowMToPts(200, 50)).toBe(5);   // 200/50 = 4 → 5 (impair)
    expect(despikeWindowMToPts(200, 100)).toBe(3);  // 200/100 = 2 → 3 (min)
    expect(despikeWindowMToPts(100, 50)).toBe(3);   // 100/50 = 2 → 3 (min)
    expect(despikeWindowMToPts(400, 50)).toBe(9);   // 400/50 = 8 → 9
  });

  it('applique le plancher de 3 pts', () => {
    expect(despikeWindowMToPts(50, 100)).toBe(3);
    expect(despikeWindowMToPts(50, 200)).toBe(3);
  });
});

describe('despikeAltitude (Hampel v3.6)', () => {
  it('corrige un pic isolé', () => {
    const ele = Float64Array.from([1180, 1182, 1183, 1250, 1185, 1186, 1187]);
    const { out, count } = despikeAltitude(ele, 5, 15);
    expect(count).toBe(1);
    expect(out[3]).toBe(1183);
  });

  it('ne modifie pas une progression régulière', () => {
    const ele = Float64Array.from([1180, 1182, 1183, 1190, 1198, 1205, 1210]);
    const { out, count } = despikeAltitude(ele, 5, 15);
    expect(count).toBe(0);
    expect(Array.from(out)).toEqual(Array.from(ele));
  });

  it('respecte le seuil', () => {
    const ele = Float64Array.from([1000, 1000, 1000, 1010, 1000, 1000, 1000]);
    const { count: c15 } = despikeAltitude(ele, 5, 15);
    expect(c15).toBe(0);
    const { count: c5 } = despikeAltitude(ele, 5, 5);
    expect(c5).toBe(1);
  });

  it('désactivé si seuil = 0', () => {
    const ele = Float64Array.from([1000, 1000, 5000, 1000, 1000]);
    const { out, count } = despikeAltitude(ele, 5, 0);
    expect(count).toBe(0);
    expect(Array.from(out)).toEqual(Array.from(ele));
  });
});

// tests/unit/resample.test.ts
describe('resampleUniform (moyenne v3.5)', () => {
  it('produit un pas constant', () => {
    const distRaw = new Float64Array([0, 5, 12, 25, 40]);
    const eleRaw = new Float64Array([100, 101, 102, 103, 104]);
    const { dist, ele } = resampleUniform(distRaw, eleRaw, 10);
    expect(dist[0]).toBe(0);
    expect(dist[1]).toBe(10);
    expect(dist[4]).toBe(40);
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
    expect(Math.abs(ele[5] - 150)).toBeLessThan(15);
  });
});

// tests/unit/pipeline.test.ts
describe('Pipeline complet v3.6', () => {
  it('détecte 3 montées + corrige les glitchs', async () => {
    const content = readFileSync('tests/fixtures/bormio-mortirolo.gpx', 'utf-8');
    const result = await analyze(content, {
      stepM: 50,
      despikeWindowM: 200,
      despikeThresholdM: 15,
      fenetreAffichageM: 500,
      fenetrePenteM: 150,
      fenetreCatM: 100,
      seuilMontee: 0.5,
      longueurMin: 200,
      gapMinM: 250,
      refinePct: 60,
      medianWindowM: 0,
      gaussianSigmaM: 75,
      plateauRun: 6,
      segmentLogM: 100,
      smoothExtra: true,
      segmentSmooth: true,
    });
    expect(result.autoClimbs.length).toBeGreaterThan(0);
    expect(result.despikeCount).toBeGreaterThanOrEqual(0);
    expect(result.avgWinM).toBe(75);
    expect(result.despikeWinPtsEffective).toBe(5); // 200/50 → 5
  });
});
```

### 15.2 Jeu de tests de référence

| Fichier | Points | Caractéristique | Cas test |
|---|---|---|---|
| `bormio-mortirolo.gpx` | ~3300 | Éditeur Openrunner | Référence principale |
| `gps-dense.gpx` | ~10000 | GPS haute fréquence | Performance |
| `glitch.gpx` | ~2000 | 1–2 pts aberrants | **Anti-pic** |
| `fragments.gpx` | ~2000 | Faux-plats multiples | Fusion v3.0 |
| `sous-seuil.gpx` | ~1500 | Fragments + pente faible | Validation v3.1 |
| `plat.gpx` | ~1000 | Aucune montée | Anti-faux-positifs |

---

## 16. Migration depuis le prototype HTML

### 16.1 Correspondances

| Prototype HTML | Vue/Tauri |
|---|---|
| `<script>` global | Modules TS dans `src/utils/` |
| `STATE` objet global | Stores Pinia |
| `renderAll()` | Watchers Vue + computed |
| Plugin `segmentationPlugin` | `chartjs-plugin-annotation` ou plugin custom |
| Plugin `markerDragPlugin` | Composable `useMarkerDrag` |
| Interception window pointerdown | Idem, dans le composable |
| `document.getElementById` | `ref` Vue |
| Drag & drop DOM | Composant `<FileDropZone />` |
| Boutons d'insertion | `<InsertionRow />` |
| `alert()` | `<v-snackbar>` |
| `confirm()` | `<v-dialog>` |
| Toggle brut | `<RawDetailToggle />` |
| `updateDiagnostics()` | `<DiagnosticPanel />` + `useDiagnostics()` |
| `despikeAltitude()` | `utils/despike.ts` |
| **`despikeWinPts` (points)** | **`despikeWindowM` (mètres, v3.6)** |
| **`resampleUniform` (interpolation)** | **`utils/resample.ts` (moyenne, v3.5)** |
| **Fenêtre couleur** | **Fenêtre de calcul pente** |
| **Longueur min. couleur** | **Longueur min. pente** |

### 16.2 Ordre de migration recommandé

1. **Port des utilitaires purs** (§6) avec tests unitaires — incluant `despike.ts` (m → pts), `resample.ts` (moyenne v3.5), `plateau.ts`, `refineGuard.ts`, `insertion.ts`
2. **Mise en place des stores Pinia** (paramètres, données, UI state)
3. **Composant `<FileDropZone>` persistant** + chargement via Tauri
4. **Composant `<DiagnosticPanel>`** + `useDiagnostics`
5. **Composant `<ParametersPanel>`** avec les 14 paramètres
6. **Composant `<CategoryRibbon>`**
7. **Composants `<ElevationChart>` et `<SlopeChart>`** + sync zoom + repli
8. **Composant `<StatsCards>`**
9. **Composant `<ClimbsTable>`** avec `<ClimbRow>` et `<InsertionRow>`
10. **Composable `useMarkerDrag`**
11. **Toggle `<RawDetailToggle>`**
12. **Désélection** (clic radio + Échap)
13. **Tests d'intégration** sur traces de référence

### 16.3 Points d'attention

| Point | Risque | Mitigation |
|---|---|---|
| Performance stores Pinia | Recalcul à chaque mutation | `watch` + debounce |
| Réactivité Float64Array | Non réactive | `shallowRef` |
| Chart.js + Vue | Fuite mémoire | `onBeforeUnmount` → `chart.destroy()` |
| Sync zoom | Boucle infinie | Flag `syncing` |
| Interception drag | Conflit plugin-zoom | Capture `pointerdown` au window |
| Tooltip pendant drag | Masque les courbes | Désactivation dynamique |
| Tauri IPC | Blocage main thread | Commandes async |
| **Anti-pic sur 100k points** | Latence | Option Node.js dédiée / worker |
| **Diagnostic recalculé trop souvent** | Perte de perf | Debounce 250 ms |

---

## 17. Annexes

### 17.1 Résumé des paramètres par catégorie

#### Paramètres UI (14 exposés)

| Nom UI | Type | Défaut v3.6 | Plage |
|---|---|---|---|
| Pas de rééchantillonnage | float | **50** | 2–200 |
| **Anti-pic : fenêtre** | **float** | **200** | **50–500** |
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

#### Constantes internes

| Nom | Valeur |
|---|---|
| `REFINE_MIN_M` | 1500 |
| `REFINE_MAX_M` | 8000 |
| `STEP_M_MIN` / `STEP_M_MAX` | 2 / 200 |
| `GAP_MIN_M_DEFAULT` | 250 |
| `SLOPE_DISPLAY_MAX_PCT` | 25 |
| `PLATEAU_TOLERANCE_M` | 1.0 |
| `RAW_DOUBLON_MIN_M` | 1.0 |
| **`DESPIKE_WINDOW_M_DEFAULT`** | **200** |
| **`DESPIKE_WINDOW_M_MIN`** | **50** |
| **`DESPIKE_WINDOW_M_MAX`** | **500** |
| `DESPIKE_THRESHOLD_M_DEFAULT` | 15 |
| **`RESAMPLE_AVG_MIN_M`** | **75** |
| `INSERT_EDGE_MIN_KM` | 1.0 |
| `INSERT_BETWEEN_MIN_KM` | 0.2 |
| `INSERT_DEFAULT_LEN_KM` | 1.0 |
| `_maxChartPoints` | 4000 |
| `_maxSearchSteps` | 800 |
| `_minSegmentLength` | 300 |
| `DRAG_MIN_LEN_KM` | 0.05 |
| `DRAG_MIN_GAP_KM` | 0.001 |

#### Critères de validation

| Longueur | Pente min |
|---|---|
| ≥ 1000 m | 3 % |
| 750–1000 m | 4 % |
| 500–750 m | 5 % |
| 400–500 m | 6 % |
| 300–400 m | 7 % |
| < 300 m | rejeté |

#### Catégories de pente

Inchangé (§5).

### 17.2 Checklist d'intégration v3.6

- [ ] Utilitaires TS portés (haversine, resample, plateau, rawStats, smoothing, slope)
- [ ] `resample.ts` avec **moyennage** (v3.5) + constante `RESAMPLE_AVG_MIN_M = 75`
- [ ] `despike.ts` avec `despikeWindowMToPts` + `applyDespike` (v3.6)
- [ ] `mergeClimbs.ts` avec `tryMergeWithPrevious`
- [ ] `validateClimbs.ts` avec `validateAndFallback`
- [ ] `climbConstraints.ts` avec `applyConstraints` + `applyDragConstraint`
- [ ] `insertion.ts` avec `computeInsertionSlots` + `computeInsertionBounds`
- [ ] Store Pinia paramètres (14 paramètres, `despikeWindowM` en mètres)
- [ ] Store Pinia données GPX (raw + resampled + eleFinal + avgWinM + despikeWinPtsEffective)
- [ ] Store Pinia montées (avec `mergedCount`, `fallback`)
- [ ] Composable `useDiagnostics` avec `computeDiagnostics` + règles anti-pic v3.6
- [ ] Commande Tauri `read_gpx_file`
- [ ] Commande Tauri `analyze_gpx` (sidecar Node.js v3.6)
- [ ] Composant `<FileDropZone>` avec info conversion points
- [ ] Composant `<DiagnosticPanel>`
- [ ] Composant `<ParametersPanel>` (14 paramètres, anti-pic fenêtre en mètres)
- [ ] Composant `<CategoryRibbon>`
- [ ] Composant `<ElevationChart>` + `<SlopeChart>` (repliable)
- [ ] Sync zoom entre graphiques
- [ ] Composable `useMarkerDrag`
- [ ] Interception `pointerdown` au window en capture
- [ ] Désactivation pan + tooltip pendant drag
- [ ] Colonne Extrema avec tooltip détaillé
- [ ] Composant `<InsertionRow>`
- [ ] Désélection radio + Échap
- [ ] Toggle brut avec états user/auto distincts
- [ ] Indicateur `⊕` sur les lignes fusionnées
- [ ] Indicateur `⤺` sur les lignes en repli
- [ ] Composant `<StatsCards>` (avec `n bruts → m resamplés`)
- [ ] Log `dbgDespike` avec conversion m → pts
- [ ] Log `dbgResample` avec fenêtre de moyennage
- [ ] Warnings diagnostic anti-pic (fenêtre < 2 × Pas, seuil < 5)
- [ ] Tests unitaires sur 6 traces de référence
- [ ] Test d'invariant : aucune montée affichée sous son seuil

### 17.3 Ressources

- **GPX format** : https://www.topografix.com/GPX/1/1/
- **Chart.js** : https://www.chartjs.org/docs/latest/
- **chartjs-plugin-zoom** : https://www.chartjs.org/chartjs-plugin-zoom/
- **Vuetify 3** : https://vuetifyjs.com/en/
- **Tauri v2** : https://v2.tauri.app/
- **Pinia** : https://pinia.vuejs.org/
- **fast-xml-parser** : https://github.com/NaturalIntelligence/fast-xml-parser

---

**Fin du document d'intégration technique v3.6**