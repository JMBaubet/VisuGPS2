# Spécification — Atelier d'édition / montage (Modes 1 & 2)

> Documentation de référence du module d'édition caméra : pré-calcul des
> keyframes (Mode 1) et atelier de montage (Mode 2).
> Source de vérité : `src-tauri/src/edition.rs`, `src/composables/useKeyframeEngine.ts`,
> `src/views/EditionCamera.vue` et les composants `src/components/EditionCamera/`.

## Table des matières

1. [Vue d'ensemble](#vue-densemble)
2. [Architecture](#architecture)
3. [Pré-calcul (Mode 1)](#pré-calcul-mode-1)
4. [Moteur de fusion (Blending)](#moteur-de-fusion-blending)
5. [Live preview & lecture](#live-preview--lecture)
6. [Interface utilisateur](#interface-utilisateur)
7. [Paramètres](#paramètres)
8. [Hors périmètre](#hors-périmètre)

---

## Vue d'ensemble

L'atelier d'édition permet de générer une animation 3D (vol de drone) le long
d'une trace GPX et de la retoucher artistiquement sans relancer le calcul lourd.

- **Mode 1 — Pré-calcul** : génération automatique des keyframes (algorithme de
  zone morte + altitudes Mapbox). Déclenché paresseusement au clic « Éditer ».
- **Mode 2 — Édition** : atelier de montage appliquant des **overrides caméra**
  (zoom, pitch, bearing) sur des plages temporelles, avec live preview.

Le **Mode 3** (Relecture finale immersive, bouton « Visualiser ») et les
améliorations V2 (§12 de la spécification d'origine) sont **hors périmètre**.

---

## Architecture

### Flux de données

```
GPX importé (traces.json + geojson/{id}.geojson)
        │
        ▼  clic « Éditer »
┌─────────────────────────────────────────────────────┐
│ Vue EditionCamera.vue                                │
│  ├── Map3D.vue         (Mapbox terrain 3D)           │
│  │     └── pré-calcul  (Mode 1, queryTerrainElevation)
│  ├── useLivePreview    (refusion + jumpTo)           │
│  ├── usePlayback       (boucle RAF + marqueur)       │
│  ├── Timeline.vue      (curseur + play/pause)        │
│  ├── AltitudeProfile.vue (profile SVG)               │
│  ├── OverrideList.vue  (liste des overrides)         │
│  └── OverridePanel.vue (édition d'un override)        │
└─────────────────────────────────────────────────────┘
        │  invoke()
        ▼
┌─────────────────────────────────────────────────────┐
│ edition.rs (Rust)                                    │
│  has_raw_keyframes / get_raw_keyframes / save_raw…   │
│  get_montage_overrides / save_montage_overrides      │
│  delete_keyframes (cascade)                          │
└─────────────────────────────────────────────────────┘
        │
        ▼
{mode_dir}/keyframes/{traceId}_*.json
```

### Fichiers

| Fichier | Rôle |
|---|---|
| `src-tauri/src/edition.rs` | Persistance disque des keyframes (6 commandes Tauri). |
| `src/utils/keyframes.ts` | Types TS miroir des structs Rust + factories. |
| `src/utils/easing.ts` | Fonctions d'easing (`linear`, `smoothstep`, `easeInOut`) + LERP/SLERP. |
| `src/composables/useKeyframeEngine.ts` | Pré-calcul + interpolation + fusion (logique pure). |
| `src/composables/useLivePreview.ts` | Refusion automatique + `jumpTo` sur changement curseur/overrides. |
| `src/composables/usePlayback.ts` | Boucle `requestAnimationFrame` + marqueur traceur + trace parcourue. |
| `src/stores/keyframes.ts` | Store Pinia (Setup) : état keyframes/overrides/lecture. |
| `src/components/EditionCamera/Map3D.vue` | Map Mapbox terrain 3D (pré-calcul + live preview + marqueur). |
| `src/components/EditionCamera/PrecomputeOverlay.vue` | Overlay de progression du pré-calcul. |
| `src/components/EditionCamera/Timeline.vue` | Timeline interactive (curseur + contrôles lecture). |
| `src/components/EditionCamera/AltitudeProfile.vue` | Profile d'altitude SVG (cliquable pour seek). |
| `src/components/EditionCamera/OverridePanel.vue` | Édition d'un override (sliders + plage + easing). |
| `src/components/EditionCamera/OverrideList.vue` | Liste des overrides (activer/éditer/supprimer). |
| `src/components/EditionCamera/EditionSettingsPanel.vue` | Panneau de paramètres dédié (réutilise `ParameterCard`). |
| `src/views/EditionCamera.vue` | Vue assemblant tous les composants + orchestration. |

---

## Pré-calcul (Mode 1)

Déclenché au clic « Éditer » dans `Circuit.vue` si aucun cache n'existe
(`has_raw_keyframes` → false). Implémenté dans `useKeyframeEngine.precomputeKeyframes`.

### Étapes

1. **Résolution canonique** : `map.resize(width, height)` force la map à la
   résolution de référence (1920×1080 par défaut, paramétrable). Cela garantit
   que les calculs de zone morte (`map.project()` → pixels) sont indépendants
   de l'écran réel. Stocké dans `reference_viewport`.
2. **Échantillonnage** : la LineString est échantillonnée à `sampleRate` (100 ms)
   par interpolation linéaire sur la distance cumulée. La vitesse est uniforme
   (`speedFactor` ms/m = 4000 ms/km).
3. **Altitudes** : `map.queryTerrainElevation([lng, lat])` par lots de 50, délai
   100 ms entre lots (limites de taux Mapbox, §8). Source primaire Mapbox ;
   `null` en cas d'échec.
4. **Zone morte** (§3 du doc d'origine) : pour chaque échantillon, la position
   du traceur est projetée. Si elle sort de la zone centrale (`deadZoneX`/`Y` %
   du viewport canonique), la caméra se recentre sur le traceur. Le bearing est
   calculé par anticipation (`anticipationTime`) et lissé
   (`bearingSmoothThreshold`). Le zoom est ajusté par altitude
   (`zoomAltitudeFactor`) pour éviter la traversée du relief.

### Résolution canonique & multi-écrans

À la lecture, `jumpTo(center, zoom, pitch, bearing)` reproduit le **même cadrage
géographique** sur n'importe quel écran (vidéoprojecteur 4K inclus) : un écran
plus large révèle simplement plus de contexte périphérique sans décaler le
traceur. Le pré-calcul est donc identique qu'on édite sur laptop ou sur écran 4K.

### Cache

Les keyframes bruts sont persistés dans `{mode_dir}/keyframes/{traceId}_raw_keyframes.json`.
Au prochain clic « Éditer », le cache est détecté (`has_raw_keyframes` → true) et
chargé directement, sans recalcul. L'icône « Éditer » du circuit passe en vert
lorsqu'un cache existe (`Circuit.vue::hasKeyframesCache`).

---

## Moteur de fusion (Blending)

Implémenté dans `useKeyframeEngine.blendKeyframes`. Quasi instantané (la doc
l'impose), exécuté côté frontend.

Pour chaque keyframe, on calcule la contribution de chaque override actif via
son **facteur d'application** :

- **0** en dehors de la plage `[start_time, end_time]`.
- **1** au cœur de la plage.
- **Transition 0↔1** aux bords via la fonction d'easing de l'override
  (`smoothstep` par défaut) sur une fenêtre de 500 ms. Le `damping` (0..1)
  réduit l'amplitude maximale (effet d'inertie).

Les overrides se composent par interpolation successive (le dernier appliqué a
le dernier mot sur les champs qu'il définit).

**Règle critique** : le `center` (lng/lat) n'est **jamais** modifié par un
override (§6.2) — il reste issu de l'algorithme de zone morte pour garantir que
le traceur reste dans le cadre. Seuls `zoom`/`pitch`/`bearing` sont surchargeables
(valeurs absolues ou offsets relatifs).

La refusion est déclenchée automatiquement par `useLivePreview` dès qu'un
override change (`watch(overrides, { deep: true })`).

---

## Live preview & lecture

### Live preview

`useLivePreview` applique `map.jumpTo` avec l'état caméra interpolé (LERP
position/zoom/pitch, **SLERP bearing** pour éviter le tour complet par 0°) à
chaque déplacement du curseur de timeline ou chaque changement d'override.

### Lecture (preview interne à l'éditeur)

`usePlayback` lance une boucle `requestAnimationFrame` qui :
- avance `currentTime` selon `playbackSpeed` et le `speed_multiplier` de
  l'override actif à l'instant courant (ralenti/accéléré local, §12.4) ;
- déplace le marqueur traceur (point rouge, `Map3D.moveTraceurMarker`) ;
- met à jour la trace déjà parcourue (LineString dynamique,
  `Map3D.updateTrail`).

> Ce n'est **pas** le Mode 3 (Relecture finale plein écran) — c'est un preview
> intégré à l'atelier pour vérifier le rendu.

---

## Interface utilisateur

### Layout de la vue `EditionCamera.vue`

```
┌──────────────────────────────────────────────────────────┐
│ Toolbar : Home | Nom trace | Statut pré-calcul | Réglages │
├──────────────────────────────────────┬───────────────────┤
│                                       │ OverrideList      │
│         Map3D (terrain 3D)            │  (liste séquences)│
│         + PrecomputeOverlay           ├───────────────────┤
│                                       │ OverridePanel     │
│                                       │  (édition sliders)│
├──────────────────────────────────────┴───────────────────┤
│ AltitudeProfile (SVG cliquable)                          │
│ Timeline (curseur + play/pause + vitesse)                │
└──────────────────────────────────────────────────────────┘
```

### Workflow d'édition d'un override (§10.3)

1. **Se positionner** : déplacer le curseur de timeline (ou cliquer le profile).
2. **Créer une plage** : bouton « Nouvelle » dans OverrideList, ou boutons
   « Début/Fin au curseur » dans OverridePanel.
3. **Ajuster** : sliders zoom (8–20), pitch (0–85°), bearing (0–360°), offsets
   relatifs (repliés), easing + damping.
4. **Valider** : bouton « Appliquer l'override » → sauvegarde + refusion
   immédiate + mise à jour live preview.

---

## Paramètres

Définis dans `src-tauri/settings.default.toml` sous `EditionCamera.*`. Ils sont
**délibérément absents** de `[_meta.views.editionCamera].groups` : ils
n'apparaissent donc QUE dans le panneau dédié `EditionSettingsPanel.vue`, jamais
dans le `SettingsDrawer` générique.

| Groupe | Paramètres |
|---|---|
| `Viewport` | `largeurReference`, `hauteurReference` (résolution canonique) |
| `PreCalcul` | `sampleRate` |
| `Cam` | `defaultZoom`, `defaultPitch`, `defaultBearing` |
| `ZoneMorte` | `deadZoneX`, `deadZoneY`, `anticipationTime`, `bearingSmoothThreshold` |
| `FlyTo` | `minFlyDuration`, `maxFlyDuration`, `speedFactor`, `zoomAltitudeFactor` |
| `Traceur` | `markerColor`, `markerSize`, `markerAltitudeOffset`, `showTrail` |

---

## Hors périmètre (phase 2 / Mode 3)

- **Mode 3** (Relecture finale plein écran, bouton « Visualiser »).
- **§12** : splines Catmull-Rom, anti-collision terrain (raycasting), POI auto,
  easing personnalisé de vitesse global, export vidéo, audio, marqueur 3D glTF,
  undo/redo.
- **Types d'édition Mode 2 non-cœur** : FlyTo spécifique, Messages programmés,
  POI (manuels/auto). Le schéma JSON les anticipe (`messages: []`, `pois: []`)
  pour une phase 2 sans migration.

---

**Dernière mise à jour** : 2026-08-03
