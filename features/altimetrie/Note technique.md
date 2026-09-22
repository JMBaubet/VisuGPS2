# Documentation Technique — Intégration GPX Climb Analyzer

**Cible** : application Tauri + Node.js + Vue 3 + Vuetify
**Version** : 2.3
**Date** : 2026-09-21
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
13. [Tests et validation](#13-tests-et-validation)
14. [Migration depuis le prototype HTML](#14-migration-depuis-le-prototype-html)
15. [Annexes](#15-annexes)

---

## 1. Contexte d'intégration

### 1.1 Objectif

Intégrer dans une application Tauri/Vue 3/Vuetify la fonctionnalité d'analyse de profil altimétrique GPX, dans sa version **v2.3**, incluant :
- Détection automatique avec garde-fou de refinage hybride
- Édition directe par drag & drop sur le graphique
- Insertion contextuelle de montées entre les montées existantes
- Segmentation par catégorie de pente
- Drop-zone persistante

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
| Calculs (distance, pente, lissage) | **Node.js ou Front** | Node = libère le main thread |
| Détection des montées | **Node.js** | Coûteux (O(n × w)), mieux hors UI |
| Rendu graphique | **Front (Chart.js / ECharts)** | Interactif |
| Édition manuelle + drag & drop | **Front** | Latence minimale, interaction temps réel |
| Reconstruction après édition | **Front** si < 500 ms, sinon Node | Compromis |

**Recommandation forte** : tout calcul lourd (parsing + détection initiale) en Node.js via une commande Tauri (IPC), et les recalculs légers (édition manuelle, drag) en front.

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
│  │  │  │   ├── <FileDropZone /> (persistant)    │  │   │     │
│  │  │  │   ├── <ParametersPanel />              │  │   │     │
│  │  │  │   ├── <StatsCards />                   │  │   │     │
│  │  │  │   ├── <CategoryRibbon />               │  │   │     │
│  │  │  │   ├── <ElevationChart /> (drag actif)  │  │   │     │
│  │  │  │   ├── <SlopeChart /> (repliable)       │  │   │     │
│  │  │  │   ├── <CategoryStatsTable />           │  │   │     │
│  │  │  │   └── <ClimbsTable /> (insertions)     │  │   │     │
│  │  │  └────────────────────────────────────────┘  │   │     │
│  │  │                                              │   │     │
│  │  │  Stores Pinia                                │   │     │
│  │  │   ├── useGpxStore (données brutes)           │   │     │
│  │  │   ├── useClimbsStore (montées)               │   │     │
│  │  │   └── useParametersStore (paramètres)        │   │     │
│  │  │                                              │   │     │
│  │  │  Composables                                 │   │     │
│  │  │   ├── useGpxParser                           │   │     │
│  │  │   ├── useSlopeCalculations                   │   │     │
│  │  │   ├── useClimbDetection                      │   │     │
│  │  │   ├── useMarkerDrag (v0.5)                   │   │     │
│  │  │   └── useChartSync                           │   │     │
│  │  └──────────────────────────────────────────────┘   │     │
│  └─────────────────────────────────────────────────────┘     │
│                            │                                 │
│                            │ Tauri IPC (invoke)              │
│                            ▼                                 │
│  ┌─────────────────────────────────────────────────────┐     │
│  │              Backend (Rust + Node.js sidecar)       │     │
│  │  ┌──────────────────────────────────────────────┐   │     │
│  │  │  Node.js sidecar                             │   │     │
│  │  │   ├── /parse-gpx (fast-xml-parser)           │   │     │
│  │  │   ├── /compute-detection                     │   │     │
│  │  │   └── /rebuild (optionnel)                   │   │     │
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
│   └── parameters.ts
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
│   ├── useMarkerDrag.ts      # v0.5 — drag & drop poignées
│   └── useChartSync.ts
├── utils/
│   ├── haversine.ts
│   ├── smoothing.ts
│   ├── slope.ts
│   ├── categorization.ts
│   ├── segmentation.ts
│   ├── refineGuard.ts        # v2 — clamp hybride
│   └── insertion.ts          # v2.2 — insertion contextuelle
├── components/
│   └── gpx-analyzer/
│       ├── GpxAnalyzer.vue
│       ├── FileDropZone.vue          # v2.3 — mode compact
│       ├── ParametersPanel.vue
│       ├── StatsCards.vue
│       ├── CategoryRibbon.vue
│       ├── ElevationChart.vue        # v0.5 — drag
│       ├── SlopeChart.vue            # v2.1 — repliable
│       ├── CategoryStatsTable.vue
│       ├── ClimbsTable.vue           # v2.2 — insertions
│       ├── InsertionRow.vue          # v2.2
│       ├── ClimbRow.vue              # v0.3+ — radio, édition
│       └── RawDetailToggle.vue       # v2.3 — états visuels
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
  fileName?: string;   // v2.3 — pour la drop-zone compacte
  points: GpxPoint[];
  createdAt?: string;
  creator?: string;
}

// ============================================================
// src/types/parameters.ts
// ============================================================

/**
 * Paramètres de détection automatique et d'affichage.
 * Tous ces paramètres sont accessibles à l'utilisateur sauf
 * mention contraire (INTERNAL).
 */
export interface AnalyzerParameters {
  // --- Lissages ---
  /** Lissage altitude (pts) — 3 à ∞, défaut 50 */
  fenetreAffichage: number;

  /** Lissage pente instantanée (pts) — 1 à ∞, défaut 30 */
  fenetrePente: number;

  /** Fenêtre de catégorisation (m) — 50 à ∞, défaut 500 */
  fenetreCatM: number;

  // --- Détection ---
  /** Seuil de détection de montée (%) — 0.1 à ∞, défaut 0.5 */
  seuilMontee: number;

  /**
   * Pourcentage de refinage (v2) — 1 à 500, défaut 60.
   * effectiveRefine = clamp(refinePct% × candidate.length, 1500, 8000)
   */
  refinePct: number;

  // --- Segmentation ---
  /** Longueur min d'un segment de catégorie (m) — défaut 250 (v2.1) */
  longueurMin: number;

  // --- INTERNAL (non exposés à l'utilisateur) ---
  /** Nombre max de points affichés par graphique — INTERNAL, défaut 4000 */
  _maxChartPoints?: number;

  /** Pas du sous-échantillonnage de recherche sous-segment — INTERNAL */
  _maxSearchSteps?: number;
}

// ============================================================
// src/types/climb.ts
// ============================================================

export interface Climb {
  /** Index début dans le tableau dist */
  startIdx: number;

  /** Index fin dans le tableau dist */
  endIdx: number;

  /** Référence vers la montée éditable source (pour reset) */
  ref: UserClimb;

  /** Marqueur d'invalidité (bornes incohérentes) */
  invalid: boolean;

  /** Contrainte appliquée automatiquement sur le début */
  constrainedStart: boolean;

  /** Contrainte appliquée automatiquement sur la fin */
  constrainedEnd: boolean;

  /** Distance départ (m) */
  startDist: number;

  /** Distance fin (m) */
  endDist: number;

  /** Longueur du segment (m) */
  length: number;

  /** Dénivelé positif (m) */
  dPlus: number;

  /** Pente moyenne (%) */
  avgSlope: number;

  /** Altitude au départ (m) */
  altStart: number;

  /** Altitude à l'arrivée (m) */
  altEnd: number;

  /** v2 — Garde-fou effectif calculé pour cette montée (m), null si manuel */
  refineM: number | null;

  /** v2 — Longueur du candidat détecté avant raffinement (m) */
  candidateLen: number | null;
}

export interface UserClimb {
  /** Borne actuelle début (km) */
  startKm: number;

  /** Borne actuelle fin (km) */
  endKm: number;

  /** Borne automatique début (km) — null si créée manuellement */
  autoStartKm: number | null;

  /** Borne automatique fin (km) — null si créée manuellement */
  autoEndKm: number | null;

  /** v2 — Garde-fou effectif utilisé par la détection auto */
  refineM: number | null;

  /** v2 — Longueur du candidat détecté avant raffinement */
  candidateLen: number | null;

  // Flags internes posés lors de la contrainte
  _constrainedStart?: boolean;
  _constrainedEnd?: boolean;
}

// ============================================================
// src/types/category.ts
// ============================================================

export type SlopeCategoryId =
  | 'dh3' | 'dh2' | 'dh1'
  | 'pl'
  | 'up1' | 'up2' | 'up3' | 'up4';

export interface SlopeCategory {
  id: SlopeCategoryId;
  label: string;
  color: string;
  min: number;
  max: number;
}

export interface CategorySegment {
  startIdx: number;
  endIdx: number;
  key: SlopeCategoryId;
  startDist: number;
  endDist: number;
  length: number;
  dPlus: number;
  avgSlope: number;
  altStart: number;
  altEnd: number;
}

// ============================================================
// src/types/analysis.ts
// ============================================================

export interface AnalyzerState {
  // Données sources
  track: GpxTrack | null;

  // Tableaux calculés (Float64Array pour la performance)
  dist: Float64Array | null;
  eleRaw: Float64Array | null;
  eleDisplay: Float64Array | null;
  slopeRaw: Float64Array | null;
  slopeDisplay: Float64Array | null;
  slopeForCat: Float64Array | null;

  // Résultats
  catSegments: CategorySegment[];
  climbs: Climb[];
  markers: Array<{ dist: number; type: 'valley' | 'summit'; climbIdx: number }>;

  // État utilisateur
  userClimbs: UserClimb[];
  autoClimbs: Array<{
    startKm: number;
    endKm: number;
    refineM: number;
    candidateLen: number;
  }>;

  // v2.3 — UI state
  selectedClimbIdx: number | null;
  showRawDetail: boolean;               // toggle "mettre en avant l'altitude brute"
  slopeChartExpanded: boolean;          // repli du graphe pente

  // v0.5 — drag state (transitoire)
  draggingHandle: 'valley' | 'summit' | null;
  dragPreview: { handle: 'valley' | 'summit'; km: number } | null;
}
```

---

## 4. Inventaire complet des paramètres

### 4.1 Paramètres utilisateur (exposés dans l'UI)

| Nom technique | Nom UI (FR) | Type | Plage | Défaut | Impact principal |
|---|---|---|---|---|---|
| `fenetreAffichage` | Lissage altitude | number (int) | 3 – ∞ pts | **50** | Douceur du profil affiché |
| `fenetrePente` | Lissage pente | number (int) | 1 – ∞ pts | **30** | Douceur de la courbe de pente instantanée |
| `fenetreCatM` | Fenêtre catégorisation | number (float) | 50 – ∞ m | **500** | Élargit la pente pour la couleur |
| `seuilMontee` | Seuil détection | number (float) | 0.1 – ∞ % | **0.5** | Sensibilité de la détection |
| **`refinePct`** | **Garde-fou refinage** | number (float) | 1 – 500 % | **60** | v2 — % appliqué à la longueur détectée, clamps 1500/8000 m |
| `longueurMin` | Longueur min. catégorie | number (float) | 0 – ∞ m | **250** *(v2.1)* | Fusion des micro-segments |

### 4.2 Constantes internes (non exposées)

| Nom technique | Valeur | Rôle |
|---|---|---|
| `REFINE_MIN_M` | 1500 | Plancher du garde-fou hybride |
| `REFINE_MAX_M` | 8000 | Plafond du garde-fou hybride |
| `INSERT_EDGE_MIN_KM` | 1.0 | Espace minimum avant #1 / après #N pour afficher le bouton d'insertion |
| `INSERT_BETWEEN_MIN_KM` | 0.2 | Espace minimum entre deux montées pour afficher le bouton d'insertion |
| `INSERT_DEFAULT_LEN_KM` | 1.0 | Longueur cible d'une montée insérée |
| `_maxChartPoints` | 4000 | Points max par graphique |
| `_maxSearchSteps` | 800 | Pas du sous-échantillonnage dans `findBestSubClimb` |
| `_eleBarelyWin` | 5 | Fenêtre du micro-lissage pour la pente brute affichée |
| `_minSegmentLength` | 300 | Longueur minimale absolue pour une montée |
| `_constraintStartOffset` | 0.001 km | Décalage min entre deux montées contraintes |
| `_constraintEndOffset` | 0.1 km | Longueur min après contrainte |
| `_safetyDistMin` | 0.5 m | Distance min pour diviser (pente instantanée) |
| `_safetyDistWindow` | 0.5 m | Distance min dans une fenêtre glissante |

### 4.3 Critères de validation des montées

Ces critères sont **affichés à l'utilisateur** dans l'UI mais **non modifiables** dans la version 2.x.

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

### 4.4 Validation des paramètres

```typescript
// src/utils/parameterValidation.ts

export function validateParameters(p: AnalyzerParameters): AnalyzerParameters {
  return {
    fenetreAffichage: Math.max(3, p.fenetreAffichage),
    fenetrePente:     Math.max(1, p.fenetrePente),
    fenetreCatM:      Math.max(50, p.fenetreCatM),
    seuilMontee:      Math.max(0.1, p.seuilMontee),
    refinePct:        Math.min(500, Math.max(1, p.refinePct)),   // v2
    longueurMin:      Math.max(0, p.longueurMin),
  };
}
```

---

## 5. Paramètres de catégorisation de pente

### 5.1 Configuration actuelle (figée en V2)

```typescript
// src/utils/categorization.ts

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
- La **catégorisation est symétrique** pour le plat et les faux plats, mais **asymétrique** pour les raides.
- Les seuils sont **codés en dur** dans la V2 (constante `SLOPE_CATEGORIES`).

### 5.4 Évolution V3 — rendre éditables

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

export function haversine(lat1: number, lon1: number, lat2: number, lon2: number): number {
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

### 6.2 Lissage

```typescript
// src/utils/smoothing.ts

export function movingAverage(arr: ArrayLike<number>, win: number): Float64Array {
  const n = arr.length;
  const res = new Float64Array(n);
  if (win < 2 || n < 2) {
    for (let i = 0; i < n; i++) res[i] = arr[i];
    return res;
  }

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

export function anchoredMovingAverage(
  raw: Float64Array,
  anchorIdxs: number[],
  win: number,
): Float64Array {
  const n = raw.length;
  const base = movingAverage(raw, win);

  const set = new Set<number>([0, n - 1]);
  for (const i of anchorIdxs) if (i > 0 && i < n - 1) set.add(i);
  const anchors = Array.from(set).sort((a, b) => a - b);

  const result = Float64Array.from(base);

  for (let k = 0; k < anchors.length - 1; k++) {
    const a = anchors[k];
    const b = anchors[k + 1];
    if (b - a < 2) continue;

    const corrA = raw[a] - base[a];
    const corrB = raw[b] - base[b];

    for (let i = a; i <= b; i++) {
      const t = (i - a) / (b - a);
      result[i] = base[i] + corrA * (1 - t) + corrB * t;
    }
  }
  return result;
}
```

### 6.3 Pente

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

export function computeSlopeWindow(
  ele: Float64Array,
  dist: Float64Array,
  winM: number,
): Float64Array {
  const n = ele.length;
  const s = new Float64Array(n);
  if (n < 2) return s;

  for (let i = 0; i < n; i++) {
    let lo = i;
    while (lo > 0 && dist[i] - dist[lo - 1] <= winM) lo--;
    let hi = i;
    while (hi < n - 1 && dist[hi + 1] - dist[i] <= winM) hi++;

    const dDist = dist[hi] - dist[lo];
    if (dDist > 0.5) {
      s[i] = ((ele[hi] - ele[lo]) / dDist) * 100;
    }
  }
  return s;
}
```

### 6.4 Garde-fou hybride (v2)

```typescript
// src/utils/refineGuard.ts

export const REFINE_MIN_M = 1500;
export const REFINE_MAX_M = 8000;
export const REFINE_PCT_DEFAULT = 60;

/**
 * Calcule le garde-fou de refinage effectif pour une montée candidate.
 * @param candidateLength  Longueur du candidat détecté (m)
 * @param refinePct        Pourcentage utilisateur (1–500)
 * @returns                Valeur en mètres, clampée entre REFINE_MIN_M et REFINE_MAX_M
 */
export function computeEffectiveRefine(
  candidateLength: number,
  refinePct: number,
): number {
  const pctValue = (refinePct * candidateLength) / 100;
  return Math.max(REFINE_MIN_M, Math.min(REFINE_MAX_M, pctValue));
}
```

### 6.5 Catégorisation

```typescript
// src/utils/categorization.ts

import type { SlopeCategory, SlopeCategoryId } from '@/types/category';

export const SLOPE_CATEGORIES: SlopeCategory[] = [ /* voir §5.1 */ ];

const CAT_BY_ID = new Map(SLOPE_CATEGORIES.map((c) => [c.id, c]));

export function categorize(slope: number): SlopeCategoryId {
  for (const c of SLOPE_CATEGORIES) {
    if (slope >= c.min && slope < c.max) return c.id;
  }
  return 'pl';
}

export function colorForSlope(slope: number): string {
  return CAT_BY_ID.get(categorize(slope))?.color ?? '#94a3b8';
}

export function labelForSlope(slope: number): string {
  return CAT_BY_ID.get(categorize(slope))?.label ?? '—';
}
```

### 6.6 Segmentation

```typescript
// src/utils/segmentation.ts

import { categorize } from './categorization';
import type { CategorySegment, SlopeCategoryId } from '@/types/category';

interface RawSegment {
  startIdx: number;
  endIdx: number;
  key: SlopeCategoryId;
}

export function mergeShortSegments(
  raw: RawSegment[],
  dist: Float64Array,
  minLen: number,
): RawSegment[] {
  let changed = true;
  while (changed && raw.length > 1) {
    changed = false;
    for (let i = 0; i < raw.length; i++) {
      const s = raw[i];
      if (dist[s.endIdx] - dist[s.startIdx] >= minLen) continue;

      const leftLen  = i > 0 ? dist[raw[i - 1].endIdx] - dist[raw[i - 1].startIdx] : -1;
      const rightLen = i < raw.length - 1 ? dist[raw[i + 1].endIdx] - dist[raw[i + 1].startIdx] : -1;

      if (rightLen >= leftLen) {
        raw[i + 1].startIdx = s.startIdx;
      } else {
        raw[i - 1].endIdx = s.endIdx;
      }
      raw.splice(i, 1);
      changed = true;
      break;
    }
  }

  if (raw.length > 1) {
    const res: RawSegment[] = [raw[0]];
    for (let i = 1; i < raw.length; i++) {
      const last = res[res.length - 1];
      if (last.key === raw[i].key) {
        last.endIdx = raw[i].endIdx;
      } else {
        res.push(raw[i]);
      }
    }
    return res;
  }
  return raw;
}

export function segmentByCategory(
  slopeForCat: Float64Array,
  dist: Float64Array,
  ele: Float64Array,
  minLen: number,
): CategorySegment[] {
  const n = slopeForCat.length;
  const cat: SlopeCategoryId[] = new Array(n);
  for (let i = 0; i < n; i++) cat[i] = categorize(slopeForCat[i]);

  const raw: RawSegment[] = [];
  let start = 0;
  for (let i = 1; i <= n; i++) {
    if (i === n || cat[i] !== cat[start]) {
      const endIdx = i === n ? i - 1 : i;
      raw.push({ startIdx: start, endIdx, key: cat[start] });
      start = i;
    }
  }

  const merged = mergeShortSegments(raw, dist, minLen);
  return merged.map((s) => enrichSegment(s, dist, ele, slopeForCat));
}

function enrichSegment(
  s: RawSegment,
  dist: Float64Array,
  ele: Float64Array,
  slope: Float64Array,
): CategorySegment {
  const startDist = dist[s.startIdx];
  const endDist = dist[s.endIdx];
  const length = endDist - startDist;

  let dPlus = 0;
  for (let k = s.startIdx + 1; k <= s.endIdx; k++) {
    const d = ele[k] - ele[k - 1];
    if (d > 0) dPlus += d;
  }

  let weighted = 0;
  for (let k = s.startIdx + 1; k <= s.endIdx; k++) {
    weighted += slope[k] * (dist[k] - dist[k - 1]);
  }

  return {
    ...s,
    startDist,
    endDist,
    length,
    dPlus,
    avgSlope: length > 0 ? weighted / length : 0,
    altStart: ele[s.startIdx],
    altEnd: ele[s.endIdx],
  };
}
```

### 6.7 Détection des montées

```typescript
// src/utils/climbDetection.ts

import { CLIMB_CRITERIA } from '@/constants/climbCriteria';
import { computeEffectiveRefine } from './refineGuard';

export function minSlopeForLength(len: number): number {
  for (const c of CLIMB_CRITERIA) {
    if (len >= c.minLen) return c.minSlope;
  }
  return Infinity;
}

export function findBestSubClimb(
  dist: Float64Array,
  ele: Float64Array,
  startIdx: number,
  endIdx: number,
  maxSteps = 800,
) {
  const n = endIdx - startIdx + 1;
  if (n < 2) return null;

  const cum = new Float64Array(n);
  for (let k = 1; k < n; k++) {
    const d = ele[startIdx + k] - ele[startIdx + k - 1];
    cum[k] = cum[k - 1] + (d > 0 ? d : 0);
  }

  const step = n > maxSteps ? Math.ceil(n / maxSteps) : 1;
  let best: any = null;

  for (let si = 0; si < n - 1; si += step) {
    for (let ei = si + 1; ei < n; ei += step) {
      const len = dist[startIdx + ei] - dist[startIdx + si];
      if (len < 300) continue;
      if (best && len <= best.length) continue;

      const dP = cum[ei] - cum[si];
      const s = (dP / len) * 100;
      if (s >= minSlopeForLength(len)) {
        best = {
          startIdx: startIdx + si,
          endIdx: startIdx + ei,
          length: len,
          dPlus: dP,
          avgSlope: s,
        };
      }
    }
  }
  return best;
}
```

### 6.8 Contraintes anti-chevauchement + drag

```typescript
// src/utils/climbConstraints.ts

import type { UserClimb } from '@/types/climb';

const START_OFFSET_KM = 0.001; // +1 m
const END_OFFSET_KM = 0.1;     // +100 m

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

// v0.5 — Contrainte temps réel pendant le drag d'une poignée
const DRAG_MIN_LEN_KM = 0.05;   // 50 m minimum
const DRAG_MIN_GAP_KM = 0.001;  // 1 m entre montées

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

  let lo = 0;
  let hi = totalKm;
  if (prevClimb) lo = prevClimb.endKm + DRAG_MIN_GAP_KM;
  if (nextClimb) hi = nextClimb.startKm - DRAG_MIN_GAP_KM;

  if (handle === 'valley') {
    return Math.max(lo, Math.min(climb.endKm - DRAG_MIN_LEN_KM, km));
  } else {
    return Math.max(climb.startKm + DRAG_MIN_LEN_KM, Math.min(hi, km));
  }
}
```

### 6.9 Insertion contextuelle (v2.2)

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

/**
 * Calcule la liste des slots d'insertion disponibles en fonction
 * des montées actuelles.
 */
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

  // Avant #1
  const firstStartKm = climbs[0].startDist / 1000;
  if (firstStartKm >= INSERT_EDGE_MIN_KM) {
    slots.push({
      kind: 'before-first',
      label: 'Avant la montée #1',
      gapStartKm: 0, gapEndKm: firstStartKm, gapKm: firstStartKm,
    });
  }

  // Entre deux montées
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

  // Après #N
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

/**
 * Calcule les bornes d'une nouvelle montée à insérer dans un gap donné.
 */
export function computeInsertionBounds(
  gapStartKm: number,
  gapEndKm: number,
): { startKm: number; endKm: number } | null {
  const gapLen = gapEndKm - gapStartKm;
  if (gapLen < INSERT_BETWEEN_MIN_KM) return null;

  const targetLen = Math.max(
    0.1,
    Math.min(INSERT_DEFAULT_LEN_KM, gapLen * 0.8),
  );
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

import { computed, ref } from 'vue';
import { useGpxStore } from '@/stores/gpxStore';
import { useClimbsStore } from '@/stores/climbsStore';
import { useParametersStore } from '@/stores/parametersStore';
import { invoke } from '@tauri-apps/api/core';

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

      const result = await invoke<{
        track: any;
        dist: number[];
        eleRaw: number[];
        autoClimbs: Array<{
          startKm: number;
          endKm: number;
          refineM: number;
          candidateLen: number;
        }>;
      }>('analyze_gpx', {
        content,
        parameters: paramsStore.parameters,
      });

      gpxStore.setTrack(result.track);
      gpxStore.setDistances(Float64Array.from(result.dist));
      gpxStore.setElevations(Float64Array.from(result.eleRaw));

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
    // Voir §6 modules + §21 doc Algorithmes
  }

  return { isLoading, error, loadAndAnalyze, rebuild };
}
```

### 7.2 Composable de drag & drop (v0.5)

```typescript
// src/composables/useMarkerDrag.ts

import { ref, onBeforeUnmount } from 'vue';
import type { Chart } from 'chart.js';
import type { UserClimb } from '@/types/climb';
import { applyDragConstraint } from '@/utils/climbConstraints';

const HANDLE_BAND_TOP = -12;
const HANDLE_BAND_BOTTOM = 34;
const HANDLE_THRESHOLD_PX = 14;

export function useMarkerDrag(
  elevChart: Chart | null,
  slopeChart: Chart | null,
  getSelectedClimb: () => UserClimb | null,
  getAllClimbs: () => UserClimb[],
  getTotalKm: () => number,
  onCommit: (handle: 'valley' | 'summit', km: number) => void,
) {
  const draggingHandle = ref<'valley' | 'summit' | null>(null);
  const dragPreview = ref<{ handle: 'valley' | 'summit'; km: number } | null>(null);

  let panOriginalState: { elev?: boolean; slope?: boolean } | null = null;

  function findHandleAt(chart: Chart, px: number, py: number, climb: UserClimb) {
    const { chartArea, scales } = chart;
    if (py < chartArea.top + HANDLE_BAND_TOP || py > chartArea.top + HANDLE_BAND_BOTTOM) {
      return null;
    }

    const x = scales.x;
    const xV = x.getPixelForValue(climb.startKm);
    const xS = x.getPixelForValue(climb.endKm);
    const dV = Math.abs(px - xV);
    const dS = Math.abs(px - xS);

    if (dV < HANDLE_THRESHOLD_PX && dV <= dS) return 'valley';
    if (dS < HANDLE_THRESHOLD_PX) return 'summit';
    return null;
  }

  function onGlobalPointerDown(e: PointerEvent) {
    if (draggingHandle.value) {
      e.stopImmediatePropagation();
      e.preventDefault();
      return;
    }

    const climb = getSelectedClimb();
    if (!climb) return;

    let chart: Chart | null = null;
    if (elevChart && e.target === elevChart.canvas) chart = elevChart;
    else if (slopeChart && e.target === slopeChart.canvas) chart = slopeChart;
    if (!chart) return;

    const rect = chart.canvas.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const py = e.clientY - rect.top;

    const handle = findHandleAt(chart, px, py, climb);
    if (!handle) return;

    e.stopImmediatePropagation();
    e.preventDefault();
    startDrag(chart, handle, climb);
  }

  function startDrag(chart: Chart, handle: 'valley' | 'summit', climb: UserClimb) {
    draggingHandle.value = handle;
    dragPreview.value = {
      handle,
      km: handle === 'valley' ? climb.startKm : climb.endKm,
    };
    document.body.classList.add('no-select');

    panOriginalState = {
      elev: elevChart?.options.plugins?.zoom?.pan?.enabled,
      slope: slopeChart?.options.plugins?.zoom?.pan?.enabled,
    };
    if (elevChart) (elevChart.options.plugins as any).zoom.pan.enabled = false;
    if (slopeChart) (slopeChart.options.plugins as any).zoom.pan.enabled = false;

    // v2.1 — désactive le tooltip pour ne pas masquer les courbes
    for (const ch of [elevChart, slopeChart]) {
      if (!ch) continue;
      (ch.options.plugins as any).tooltip.enabled = false;
      ch.tooltip?.setActiveElements([], { x: 0, y: 0 });
      ch.tooltip?.update();
    }

    window.addEventListener('pointermove', onMove, true);
    window.addEventListener('pointerup', onUp, true);
    window.addEventListener('pointercancel', onUp, true);
  }

  let lastKm: number | null = null;

  function onMove(e: PointerEvent) {
    const handle = draggingHandle.value;
    const climb = getSelectedClimb();
    if (!handle || !climb) return;

    const chart = elevChart ?? slopeChart;
    if (!chart) return;

    const rect = chart.canvas.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const km = chart.scales.x.getValueForPixel(px);
    if (km == null || !isFinite(km)) return;

    const constrainedKm = applyDragConstraint(
      handle, km, climb, getAllClimbs(), getTotalKm(),
    );
    if (constrainedKm === lastKm) return;
    lastKm = constrainedKm;

    dragPreview.value = { handle, km: constrainedKm };
    chart.draw();
    slopeChart?.draw();

    e.stopImmediatePropagation();
    e.preventDefault();
  }

  function onUp(e: PointerEvent) {
    const handle = draggingHandle.value;
    const preview = dragPreview.value;
    if (!handle) return;

    window.removeEventListener('pointermove', onMove, true);
    window.removeEventListener('pointerup', onUp, true);
    window.removeEventListener('pointercancel', onUp, true);

    draggingHandle.value = null;
    dragPreview.value = null;
    lastKm = null;
    document.body.classList.remove('no-select');

    if (elevChart && panOriginalState) {
      (elevChart.options.plugins as any).zoom.pan.enabled = panOriginalState.elev !== false;
    }
    if (slopeChart && panOriginalState) {
      (slopeChart.options.plugins as any).zoom.pan.enabled = panOriginalState.slope !== false;
    }
    panOriginalState = null;

    for (const ch of [elevChart, slopeChart]) {
      if (!ch) continue;
      (ch.options.plugins as any).tooltip.enabled = true;
    }

    if (preview) onCommit(handle, preview.km);

    e.stopImmediatePropagation();
    e.preventDefault();
  }

  window.addEventListener('pointerdown', onGlobalPointerDown, true);

  onBeforeUnmount(() => {
    window.removeEventListener('pointerdown', onGlobalPointerDown, true);
    window.removeEventListener('pointermove', onMove, true);
    window.removeEventListener('pointerup', onUp, true);
    window.removeEventListener('pointercancel', onUp, true);
  });

  return { draggingHandle, dragPreview };
}
```

### 7.3 Composable de synchronisation des graphiques

```typescript
// src/composables/useChartSync.ts

import { ref } from 'vue';
import type { Chart } from 'chart.js';

export function useChartSync() {
  const elevChart = ref<Chart | null>(null);
  const slopeChart = ref<Chart | null>(null);
  const syncing = ref(false);

  function syncZoom(source: Chart): void {
    if (syncing.value) return;
    const target = source === elevChart.value ? slopeChart.value : elevChart.value;
    if (!target) return;

    syncing.value = true;
    target.options.scales!.x!.min = source.scales.x.min;
    target.options.scales!.x!.max = source.scales.x.max;
    target.update('none');
    syncing.value = false;
  }

  function zoomTo(kmStart: number, kmEnd: number): void {
    if (!elevChart.value || !slopeChart.value) return;
    const pad = Math.max(0.05, (kmEnd - kmStart) * 0.05);

    for (const ch of [elevChart.value, slopeChart.value]) {
      ch.options.scales!.x!.min = kmStart - pad;
      ch.options.scales!.x!.max = kmEnd + pad;
      ch.update('none');
    }
  }

  return { elevChart, slopeChart, syncing, syncZoom, zoomTo };
}
```

---

## 8. Composants Vuetify

### 8.1 Drop zone persistante (v2.3)

```vue
<!-- src/components/gpx-analyzer/FileDropZone.vue -->
<template>
  <div
    class="drop-zone"
    :class="{ dragover: isDragover, compact: !!fileName }"
    @click="pickFile"
    @dragover.prevent="isDragover = true"
    @dragleave="isDragover = false"
    @drop.prevent="onDrop"
  >
    <div class="drop-zone-icon">📁</div>
    <div class="drop-zone-text">
      <template v-if="!fileName">
        Glissez-déposez un fichier <strong>.gpx</strong> ici<br>
        ou <strong>cliquez pour choisir</strong>
      </template>
      <template v-else>
        <strong>{{ fileName }}</strong> — glissez un nouveau fichier
        <strong>.gpx</strong> ou <strong>cliquez pour changer</strong>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';

defineProps<{ fileName?: string | null }>();
const emit = defineEmits<{ (e: 'file', file: File): void }>();

const isDragover = ref(false);

function pickFile() {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.gpx,.txt,application/gpx+xml,text/xml';
  input.onchange = (e) => {
    const f = (e.target as HTMLInputElement).files?.[0];
    if (f) emit('file', f);
  };
  input.click();
}

function onDrop(e: DragEvent) {
  isDragover.value = false;
  const f = e.dataTransfer?.files?.[0];
  if (f) emit('file', f);
}
</script>
```

### 8.2 Panneau de paramètres (avec refinePct)

```vue
<!-- src/components/gpx-analyzer/ParametersPanel.vue -->
<template>
  <v-card class="pa-4">
    <v-card-title class="text-h6">Paramètres de détection automatique</v-card-title>

    <v-row dense>
      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.fenetreAffichage"
          label="Lissage altitude" type="number" :min="3" :step="2"
          suffix="pts" density="compact" @update:model-value="onChange" />
      </v-col>

      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.fenetrePente"
          label="Lissage pente" type="number" :min="1" :step="2"
          suffix="pts" density="compact" @update:model-value="onChange" />
      </v-col>

      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.fenetreCatM"
          label="Fenêtre catégorisation" type="number" :min="50" :step="100"
          suffix="m" density="compact" @update:model-value="onChange" />
      </v-col>

      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.seuilMontee"
          label="Seuil détection" type="number" :min="0.1" :step="0.1"
          suffix="%" density="compact" @update:model-value="onChange" />
      </v-col>

      <v-col cols="12" md="4">
        <v-text-field v-model.number="params.longueurMin"
          label="Longueur min. catégorie" type="number" :min="0" :step="10"
          suffix="m" density="compact" @update:model-value="onChange" />
      </v-col>

      <!-- v2 — Garde-fou hybride -->
      <v-col cols="12" md="4">
        <v-text-field
          v-model.number="params.refinePct"
          label="Garde-fou refinage"
          type="number"
          :min="1"
          :max="500"
          :step="5"
          suffix="%"
          density="compact"
          @update:model-value="onChange"
        >
          <template #append-inner>
            <v-chip size="x-small" color="deep-purple" variant="flat">Avancé</v-chip>
          </template>
          <template #details>
            <span class="text-caption text-grey">
              clamp(pct × longueur, 1500 m – 8000 m)
            </span>
          </template>
        </v-text-field>
      </v-col>
    </v-row>

    <v-alert type="info" variant="tonal" density="compact" class="mt-4">
      <div class="text-caption font-weight-bold mb-2">
        Critères de validation d'une montée
      </div>
      <div class="d-flex flex-wrap ga-2">
        <v-chip size="small" color="deep-orange">≥ 1000 m · ≥ 3 %</v-chip>
        <v-chip size="small" color="deep-orange">750–1000 m · ≥ 4 %</v-chip>
        <v-chip size="small" color="deep-orange">500–750 m · ≥ 5 %</v-chip>
        <v-chip size="small" color="deep-orange">400–500 m · ≥ 6 %</v-chip>
        <v-chip size="small" color="deep-orange">300–400 m · ≥ 7 %</v-chip>
        <v-chip size="small" variant="outlined">&lt; 300 m · rejetée</v-chip>
      </div>
    </v-alert>
  </v-card>
</template>

<script setup lang="ts">
import { reactive } from 'vue';
import { useParametersStore } from '@/stores/parametersStore';
import { debounce } from '@/utils/debounce';

const paramsStore = useParametersStore();
const params = reactive({ ...paramsStore.parameters });

const emit = defineEmits<{ (e: 'change'): void }>();

const onChange = debounce(() => {
  paramsStore.setParameters({ ...params });
  emit('change');
}, 250);
</script>
```

### 8.3 Toggle « Mettre en avant l'altitude brute » (v2.3)

```vue
<!-- src/components/gpx-analyzer/RawDetailToggle.vue -->
<template>
  <label
    class="toggle-brut"
    :class="{
      'user-on': modelValue,
      'auto-on': !modelValue && forcedByEditing,
    }"
  >
    <input type="checkbox" :checked="modelValue"
      @change="$emit('update:modelValue', ($event.target as HTMLInputElement).checked)">
    <span>Mettre en avant l'altitude brute</span>
    <span v-if="!modelValue && forcedByEditing" class="auto-badge">Auto</span>
  </label>
</template>

<script setup lang="ts">
defineProps<{
  modelValue: boolean;
  forcedByEditing: boolean;
}>();
defineEmits<{ (e: 'update:modelValue', v: boolean): void }>();
</script>
```

### 8.4 Tableau des montées avec insertion contextuelle (v2.2)

```vue
<!-- src/components/gpx-analyzer/ClimbsTable.vue -->
<template>
  <v-card class="pa-4">
    <div class="d-flex justify-space-between align-center mb-3">
      <v-card-title class="text-h6 pa-0">Montées détectées</v-card-title>
      <v-btn size="small" variant="outlined" prepend-icon="mdi-refresh"
        @click="$emit('reload')">Recharger auto</v-btn>
    </div>

    <v-table density="compact">
      <thead>
        <tr>
          <th style="width:30px"></th>
          <th>#</th>
          <th class="text-right">Vallée (km)</th>
          <th class="text-right">Sommet (km)</th>
          <th class="text-right">Long.</th>
          <th class="text-right">D+</th>
          <th class="text-right">Pente</th>
          <th class="text-right">Refine</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        <template v-for="(slot, si) in slots" :key="'slot-'+si">
          <!-- Rangée d'insertion -->
          <tr class="insertion-row">
            <td colspan="9" class="text-center">
              <v-btn
                size="x-small"
                variant="outlined"
                color="primary"
                prepend-icon="mdi-plus"
                @click="$emit('insert', slot)"
              >
                {{ slot.label }}
                <template #append>
                  <span class="text-caption text-grey ml-2">
                    · {{ slot.gapKm >= 1 ? slot.gapKm.toFixed(2)+' km' : (slot.gapKm*1000).toFixed(0)+' m' }} libres
                  </span>
                </template>
              </v-btn>
            </td>
          </tr>
          <!-- Ligne de montée -->
          <ClimbRow
            v-if="slot.followingClimbIndex !== undefined"
            :climb="climbs[slot.followingClimbIndex]"
            :index="slot.followingClimbIndex"
            :selected="selectedIdx === slot.followingClimbIndex"
            :total-km="totalKm"
            @select="$emit('select', slot.followingClimbIndex)"
            @deselect="$emit('deselect')"
            @edit="(f, v) => $emit('edit', slot.followingClimbIndex!, f, v)"
            @reset="$emit('reset', slot.followingClimbIndex!)"
            @delete="$emit('delete', slot.followingClimbIndex!)"
            @zoom="$emit('zoom', $event)"
          />
        </template>
      </tbody>
    </v-table>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import ClimbRow from './ClimbRow.vue';
import { computeInsertionSlots } from '@/utils/insertion';
import type { Climb } from '@/types/climb';

const props = defineProps<{
  climbs: Climb[];
  selectedIdx: number | null;
  totalDist: number;
}>();

const emit = defineEmits<{
  (e: 'select', idx: number): void;
  (e: 'deselect'): void;
  (e: 'edit', idx: number, field: 'startKm'|'endKm', value: number): void;
  (e: 'reset', idx: number): void;
  (e: 'delete', idx: number): void;
  (e: 'insert', slot: any): void;
  (e: 'reload'): void;
  (e: 'zoom', range: { kmStart: number; kmEnd: number }): void;
}>();

const totalKm = computed(() => props.totalDist / 1000);

// Interleave slots and climb indices
const slots = computed(() => {
  const rawSlots = computeInsertionSlots(
    props.climbs.map(c => ({ startDist: c.startDist, endDist: c.endDist })),
    props.totalDist,
  );
  // Insère les index de montées après chaque slot
  return rawSlots.map((slot, i) => ({
    ...slot,
    followingClimbIndex:
      slot.kind === 'before-first' ? 0 :
      slot.kind === 'between'     ? (slot.gapIdx! + 1) :
      undefined,
  })).concat(
    props.climbs.length > 0 && !rawSlots.some(s => s.kind === 'empty')
      ? [] // les lignes sont insérées dans le template ci-dessus
      : []
  );
});
</script>
```

---

## 9. Communication avec le backend Node.js

### 9.1 Sidecar Node.js

```javascript
// src-tauri/sidecar/analyzer.js
const { parentPort, workerData } = require('worker_threads');
const { XMLParser } = require('fast-xml-parser');

const REFINE_MIN_M = 1500;
const REFINE_MAX_M = 8000;

function parseGpx(xmlContent) {
  const parser = new XMLParser({
    ignoreAttributes: false,
    attributeNamePrefix: '@_',
  });
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

  return {
    name: doc?.gpx?.trk?.name ?? 'Trace sans nom',
    points,
    creator: doc?.gpx?.['@_creator'],
  };
}

function computeEffectiveRefine(candidateLength, refinePct) {
  const pctValue = (refinePct * candidateLength) / 100;
  return Math.max(REFINE_MIN_M, Math.min(REFINE_MAX_M, pctValue));
}

// Tous les algorithmes ici (haversine, movingAverage, ...) — voir §6

function analyze(content, parameters) {
  const track = parseGpx(content);
  // ... pipeline complet avec refinePct au lieu de maxRefineM

  return {
    track,
    dist: Array.from(dist),
    eleRaw: Array.from(eleRaw),
    autoClimbs,  // chaque élément contient { startKm, endKm, refineM, candidateLen }
  };
}

parentPort.postMessage(analyze(workerData.content, workerData.parameters));
```

### 9.2 Commande Tauri

```rust
// src-tauri/src/commands/analyzer.rs
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Deserialize)]
pub struct AnalyzerParameters {
    pub fenetre_affichage: u32,
    pub fenetre_pente: u32,
    pub fenetre_cat_m: f64,
    pub seuil_montee: f64,
    pub refine_pct: f64,       // v2 — remplace max_refine_m
    pub longueur_min: f64,
}

#[derive(Serialize)]
pub struct AutoClimb {
    pub start_km: f64,
    pub end_km: f64,
    pub refine_m: f64,          // v2
    pub candidate_len: f64,     // v2
}

#[derive(Serialize)]
pub struct AnalyzerResult {
    pub track: serde_json::Value,
    pub dist: Vec<f64>,
    pub ele_raw: Vec<f64>,
    pub auto_climbs: Vec<AutoClimb>,
}

#[tauri::command]
pub async fn analyze_gpx(
    content: String,
    parameters: AnalyzerParameters,
) -> Result<AnalyzerResult, String> {
    let output = Command::new("node")
        .arg("sidecar/analyzer.js")
        .env("ANALYZER_INPUT", serde_json::to_string(&(content, parameters)).unwrap())
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
}
```

### 9.3 Enregistrement des commandes

```rust
// src-tauri/src/main.rs
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::gpx::read_gpx_file,
            commands::analyzer::analyze_gpx,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri app");
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
  e.stopImmediatePropagation();  // ← empêche TOUT autre listener
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

### 11.4 Désactivation du tooltip pendant le drag (v2.1)

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
        size="x-small"
        variant="outlined"
        color="primary"
        prepend-icon="mdi-plus"
        @click="$emit('insert')"
      >
        {{ label }}
        <span class="text-caption text-grey ml-2">· {{ gapText }} libres</span>
      </v-btn>
    </td>
  </tr>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  label: string;
  gapKm: number;
}>();
defineEmits<{ (e: 'insert'): void }>();

const gapText = computed(() =>
  props.gapKm >= 1
    ? `${props.gapKm.toFixed(2)} km`
    : `${(props.gapKm * 1000).toFixed(0)} m`
);
</script>

<style scoped>
.insertion-row {
  background: #fafbfc;
  border-bottom: 1px dashed #e2e8f0;
}
.insertion-row:hover {
  background: #f1f5f9;
}
</style>
```

### 12.4 Algorithme d'insertion

Voir §6.9 pour l'implémentation TypeScript complète.

---

## 13. Tests et validation

### 13.1 Tests unitaires (Vitest)

```typescript
// tests/unit/haversine.test.ts
import { describe, it, expect } from 'vitest';
import { haversine } from '@/utils/haversine';

describe('haversine', () => {
  it('retourne 0 pour un point identique', () => {
    expect(haversine(45.0, 6.0, 45.0, 6.0)).toBe(0);
  });

  it('calcule la distance Paris–Lyon', () => {
    const d = haversine(48.8566, 2.3522, 45.7640, 4.8357);
    expect(d / 1000).toBeCloseTo(392, 0);
  });
});

// tests/unit/refineGuard.test.ts
import { computeEffectiveRefine, REFINE_MIN_M, REFINE_MAX_M } from '@/utils/refineGuard';

describe('computeEffectiveRefine (v2)', () => {
  it('clamp au plancher pour petites montées', () => {
    expect(computeEffectiveRefine(300, 60)).toBe(REFINE_MIN_M);
    expect(computeEffectiveRefine(1000, 60)).toBe(REFINE_MIN_M);
  });

  it('applique le pct au milieu de la plage', () => {
    expect(computeEffectiveRefine(5000, 60)).toBe(3000);
    expect(computeEffectiveRefine(8000, 50)).toBe(4000);
  });

  it('clamp au plafond pour montées très longues', () => {
    expect(computeEffectiveRefine(20000, 60)).toBe(REFINE_MAX_M);
    expect(computeEffectiveRefine(30000, 500)).toBe(REFINE_MAX_M);
  });
});
```

### 13.2 Tests d'intégration

```typescript
// tests/integration/analyze.test.ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { computeAuto } from '@/utils/analysis';

describe('computeAuto sur trace Morzine', () => {
  it('détecte 7 montées avec refinePct=60', () => {
    const content = readFileSync('tests/fixtures/morzine.gpx', 'utf-8');
    const result = computeAuto(content, {
      fenetreAffichage: 50,
      fenetrePente: 30,
      fenetreCatM: 500,
      seuilMontee: 0.5,
      refinePct: 60,
      longueurMin: 250,
    });
    expect(result.autoClimbs.length).toBe(7);
  });
});
```

### 13.3 Jeu de tests de référence

| Fichier | Points | Montées attendues | Cas test |
|---|---|---|---|
| `morzine.gpx` | ~2 700 | 7 | Référence principale |
| `sortie-courte.gpx` | ~500 | 1 | Test rapide |
| `plat.gpx` | ~1 000 | 0 | Anti-faux-positifs |
| `sans-ele.gpx` | ~100 | Erreur | Gestion d'erreur |
| `100k-points.gpx` | 100 000 | Variable | Performance |

---

## 14. Migration depuis le prototype HTML

### 14.1 Correspondances

| Élément prototype (v2.3) | Équivalent Vue/Tauri |
|---|---|
| `<script>` global | Modules TS dans `src/utils/` |
| `STATE` objet global | Stores Pinia |
| `renderAll()` | Watchers Vue + computed |
| Plugin `segmentationPlugin` | `chartjs-plugin-annotation` ou plugin custom |
| Plugin `markerDragPlugin` | Composable `useMarkerDrag` |
| Interception window pointerdown | Idem, dans le composable |
| `document.getElementById` | `ref` Vue |
| Drag & drop DOM | Composant `<FileDropZone />` |
| Boutons d'insertion | `<InsertionRow />` dans `<ClimbsTable />` |
| `alert()` | `<v-snackbar>` |
| `confirm()` | `<v-dialog>` |
| Toggle brut | `<RawDetailToggle />` |

### 14.2 Ordre de migration recommandé

1. **Port des utilitaires purs** (§6) en TypeScript avec tests unitaires — dont `refineGuard.ts` et `insertion.ts`.
2. **Mise en place du store Pinia** pour les paramètres, les données, et l'UI state.
3. **Composant `<FileDropZone>` persistant** + chargement via Tauri.
4. **Composant `<ParametersPanel>`** avec les 6 paramètres (dont `refinePct`).
5. **Composant `<CategoryRibbon>`** (le plus simple visuellement).
6. **Composants `<ElevationChart>` et `<SlopeChart>`** + sync zoom + repli du graphe pente.
7. **Composant `<StatsCards>`**.
8. **Composant `<ClimbsTable>`** avec `<ClimbRow>` et `<InsertionRow>`.
9. **Composable `useMarkerDrag`** (drag & drop des poignées).
10. **Toggle `<RawDetailToggle>`** avec état utilisateur / auto.
11. **Désélection** (clic radio + Échap).
12. **Tests d'intégration** sur les traces de référence.

### 14.3 Points d'attention

| Point | Risque | Mitigation |
|---|---|---|
| Performance des stores Pinia | Recalcul à chaque mutation | `watch` + debounce |
| Réactivité Vue sur Float64Array | Non réactive par défaut | `shallowRef` |
| Chart.js + Vue | Fuite mémoire si non détruit | `onBeforeUnmount` → `chart.destroy()` |
| Sync zoom | Boucle infinie | Flag `syncing` |
| **Interception drag** | Conflit avec plugin-zoom | Capture `pointerdown` au window |
| **Tooltip pendant drag** | Masque les courbes | Désactivation dynamique |
| Tauri IPC | Blocage main thread | Commandes async |

---

## 15. Annexes

### 15.1 Résumé des paramètres par catégorie

#### Paramètres UI (exposés)

| Nom | Type | Défaut (v2.3) | Plage |
|---|---|---|---|
| `fenetreAffichage` | int | 50 | 3 – ∞ |
| `fenetrePente` | int | 30 | 1 – ∞ |
| `fenetreCatM` | float | 500 | 50 – ∞ |
| `seuilMontee` | float | 0.5 | 0.1 – ∞ |
| **`refinePct`** | float | **60** | **1 – 500** |
| **`longueurMin`** | float | **250** | 0 – ∞ |

#### Constantes internes (figées v2.3)

| Nom | Valeur | Rôle |
|---|---|---|
| `REFINE_MIN_M` | 1500 m | Plancher du garde-fou hybride |
| `REFINE_MAX_M` | 8000 m | Plafond du garde-fou hybride |
| `INSERT_EDGE_MIN_KM` | 1.0 km | Espace min avant #1 / après #N |
| `INSERT_BETWEEN_MIN_KM` | 0.2 km | Espace min entre deux montées |
| `INSERT_DEFAULT_LEN_KM` | 1.0 km | Longueur cible d'une insertion |
| `_maxChartPoints` | 4000 | Points max par graphique |
| `_maxSearchSteps` | 800 | Pas de recherche sous-segment |
| `_eleBarelyWin` | 5 | Micro-lissage altitude |
| `_minSegmentLength` | 300 | Longueur min absolue |
| `_constraintStartOffset` | 0.001 km | Décalage anti-chevauchement |
| `_constraintEndOffset` | 0.1 km | Longueur min après contrainte |
| `DRAG_MIN_LEN_KM` | 0.05 km | Longueur min pendant drag |
| `DRAG_MIN_GAP_KM` | 0.001 km | Gap min pendant drag |

#### Critères de validation (affichés, non éditables)

| Longueur | Pente min |
|---|---|
| ≥ 1000 m | 3 % |
| 750–1000 m | 4 % |
| 500–750 m | 5 % |
| 400–500 m | 6 % |
| 300–400 m | 7 % |
| < 300 m | rejeté |

#### Catégories de pente (affichées, non éditables)

| Plage | Couleur | Hex |
|---|---|---|
| p < −8 % | Bleu foncé | `#1e40af` |
| −8 ≤ p < −3 % | Bleu | `#3b82f6` |
| −3 ≤ p < −1 % | Bleu clair | `#93c5fd` |
| −1 ≤ p ≤ +1 % | Vert | `#22c55e` |
| +1 < p ≤ +3 % | Jaune | `#fde68a` |
| +3 < p ≤ +7 % | Orange | `#f97316` |
| +7 < p ≤ +10 % | Rouge | `#dc2626` |
| p > +10 % | Noir | `#000000` |

### 15.2 Checklist d'intégration

- [ ] Utilitaires TS portés (haversine, smoothing, slope, categorisation, segmentation)
- [ ] **`refineGuard.ts`** avec `computeEffectiveRefine` + tests
- [ ] **`insertion.ts`** avec `computeInsertionSlots` + `computeInsertionBounds`
- [ ] **`climbConstraints.ts`** avec `applyConstraints` + `applyDragConstraint`
- [ ] Store Pinia paramètres avec validation (dont `refinePct`)
- [ ] Store Pinia données GPX (track, dist, ele, fileName)
- [ ] Store Pinia montées (auto + user avec refineM/candidateLen)
- [ ] Commande Tauri `read_gpx_file`
- [ ] Commande Tauri `analyze_gpx` (sidecar Node.js avec refinePct)
- [ ] Composant `<FileDropZone>` **persistant** (mode compact après chargement)
- [ ] Composant `<ParametersPanel>` (6 paramètres UI dont refinePct)
- [ ] Composant `<CategoryRibbon>` (bande + légende)
- [ ] Composant `<ElevationChart>` + `<SlopeChart>` (repliable)
- [ ] Synchronisation du zoom entre graphiques
- [ ] **Composable `useMarkerDrag`** (drag poignées vallée/sommet)
- [ ] **Interception `pointerdown` au window en capture**
- [ ] **Désactivation pan + tooltip pendant drag**
- [ ] **Colonne Refine avec tooltip détaillé**
- [ ] **Composant `<InsertionRow>`** (boutons d'insertion contextuelle)
- [ ] **Désélection radio + Échap**
- [ ] **Toggle brut avec états user/auto distincts**
- [ ] Composant `<StatsCards>` (9 statistiques)
- [ ] Composant `<CategoryStatsTable>` (8 lignes)
- [ ] Composant `<ClimbsTable>` éditable avec drag
- [ ] Boutons : recharger auto, reset, delete (le bouton « + Ajouter » est supprimé)
- [ ] Contraintes anti-chevauchement (édition numérique + drag)
- [ ] Indicateurs ⚠ et • sur les lignes modifiées/contraintes
- [ ] Gestion d'erreur (fichier invalide, sans altitude)
- [ ] Tests unitaires sur les 6 traces de référence
- [ ] Documentation utilisateur (aide contextuelle)

### 15.3 Ressources

- **GPX format** : https://www.topografix.com/GPX/1/1/
- **Chart.js** : https://www.chartjs.org/docs/latest/
- **chartjs-plugin-zoom** : https://www.chartjs.org/chartjs-plugin-zoom/
- **Vuetify 3** : https://vuetifyjs.com/en/
- **Tauri v2** : https://v2.tauri.app/
- **Pinia** : https://pinia.vuejs.org/
- **fast-xml-parser** : https://github.com/NaturalIntelligence/fast-xml-parser

---

**Fin du document d'intégration technique**