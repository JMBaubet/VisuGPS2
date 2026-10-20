// Contrat de rendu de la carte Altimétrie : sources, couches et découpage du
// profil en segments colorés.
//
// La carte affiche :
// - les segments de pente colorés, découpés sur la géométrie d'origine de la
//   trace nettoyée (`trace.geojson`) ;
// - les bornes vallée/sommet de chaque montée ;
// - la montée sélectionnée, en halo sous les segments (les couleurs de pente
//   restent visibles).
//
// Les couleurs viennent de `altimetrieCategories` : elles sont identiques dans
// la liste, le graphe et la carte.

import type {
  CircleLayerSpecification,
  ExpressionSpecification,
  GeoJSONSource,
  LineLayerSpecification,
  Map,
} from 'mapbox-gl'
import type {
  AltimetrieFile,
  AltimetrieProfilePoint,
} from '../../stores/altimetrie'
import {
  colorForCategoryId,
  colorForSlope,
} from '../../utils/altimetrieCategories'
import { haversineMeters } from '../../utils/geo'

export const ALTIMETRIE_MAP_STYLE = 'mapbox://styles/mapbox/standard-satellite'
export const ALTIMETRIE_TERRAIN_SOURCE = 'altimetrie-terrain'

export const ALTIMETRIE_SOURCE_IDS = {
  segments: 'altimetrie-segments',
  markers: 'altimetrie-markers',
  selected: 'altimetrie-selected',
} as const

export const ALTIMETRIE_LAYER_IDS = {
  segmentsCasing: 'altimetrie-segments-casing',
  selectedCasing: 'altimetrie-selected-casing',
  selected: 'altimetrie-selected-line',
  segments: 'altimetrie-segments-line',
  markers: 'altimetrie-climb-markers',
} as const

export function emptyFC(): GeoJSON.FeatureCollection {
  return { type: 'FeatureCollection', features: [] }
}

/** Ajoute les sources et les couches, une seule fois, après le `load`. */
export function addAltimetrieLayers(map: Map): void {
  const empty = emptyFC()

  map.addSource(ALTIMETRIE_SOURCE_IDS.segments, { type: 'geojson', data: empty })
  map.addSource(ALTIMETRIE_SOURCE_IDS.markers, { type: 'geojson', data: empty })
  map.addSource(ALTIMETRIE_SOURCE_IDS.selected, { type: 'geojson', data: empty })

  // Ordre d'empilement (bas → haut) : les segments de pente restent au-dessus
  // de la montée sélectionnée, qui ne sert plus que de halo coloré.
  const segmentsCasing: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.segmentsCasing,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.segments,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': '#ffffff',
      'line-width': 6,
      'line-opacity': 0.75,
    },
  }
  map.addLayer(segmentsCasing)

  const selectedCasing: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.selectedCasing,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.selected,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': '#ffffff',
      'line-width': 12,
      'line-opacity': 0.9,
    },
  }
  map.addLayer(selectedCasing)

  const selected: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.selected,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.selected,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': ['get', 'color'] as unknown as ExpressionSpecification,
      'line-width': 10,
      'line-opacity': 0.95,
    },
  }
  map.addLayer(selected)

  const segments: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.segments,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.segments,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': ['get', 'color'] as unknown as ExpressionSpecification,
      'line-width': 4,
      'line-opacity': 0.95,
    },
  }
  map.addLayer(segments)

  const markers: CircleLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.markers,
    type: 'circle',
    source: ALTIMETRIE_SOURCE_IDS.markers,
    paint: {
      'circle-pitch-alignment': 'map',
      'circle-pitch-scale': 'map',
      'circle-radius': [
        'case',
        ['get', 'selected'],
        6,
        4,
      ] as unknown as ExpressionSpecification,
      'circle-color': [
        'match',
        ['get', 'role'],
        'valley',
        '#16a34a',
        '#dc2626',
      ] as unknown as ExpressionSpecification,
      'circle-stroke-color': '#ffffff',
      'circle-stroke-width': 2,
      'circle-opacity': 0.95,
    },
  }
  map.addLayer(markers)
}

/** Géométrie d'origine d'une trace et distances cumulées 2D. */
export interface TracePath {
  coordinates: [number, number][]
  cumulative: number[]
  totalM: number
}

/**
 * Construit le chemin d'origine de la trace (`trace.geojson`).
 *
 * Les distances suivent la même Haversine 2D que le pipeline Rust, ce qui
 * permet de découper les segments de pente sur les points exacts du GPX
 * nettoyé plutôt que sur le profil rééchantillonné.
 */
export function buildTracePath(
  geometry: GeoJSON.Feature | null,
): TracePath | null {
  const coordinates = geometryCoordinates(geometry)
  if (coordinates.length < 2) return null

  const cumulative: number[] = [0]
  let total = 0
  for (let i = 1; i < coordinates.length; i++) {
    const previous = coordinates[i - 1]
    const current = coordinates[i]
    total += haversineMeters(previous[1], previous[0], current[1], current[0])
    cumulative.push(total)
  }

  if (!(total > 0)) return null
  return { coordinates, cumulative, totalM: total }
}

/** Découpe le chemin d'origine entre deux distances (m), bornes interpolées. */
export function slicePathCoordinates(
  path: TracePath,
  startM: number,
  endM: number,
): [number, number][] {
  const start = Math.max(0, Math.min(path.totalM, startM))
  const end = Math.max(0, Math.min(path.totalM, endM))
  if (!(end > start)) return []

  const coordinates: [number, number][] = [coordinateAtDistance(path, start)]
  let index = lowerBoundNumbers(path.cumulative, start)

  // Évite de répéter le sommet exactement sur la borne de départ.
  if (
    index < path.cumulative.length &&
    path.cumulative[index] <= start + 1e-9
  ) {
    index++
  }

  for (; index < path.coordinates.length; index++) {
    if (path.cumulative[index] >= end - 1e-9) break
    const point = path.coordinates[index]
    coordinates.push([point[0], point[1]])
  }

  coordinates.push(coordinateAtDistance(path, end))
  return coordinates.length >= 2 ? coordinates : []
}

/** Coordonnée interpolée du chemin à une distance donnée (m). */
export function coordinateAtDistance(
  path: TracePath,
  targetM: number,
): [number, number] {
  const last = path.coordinates.length - 1
  if (targetM <= 0) return path.coordinates[0]
  if (targetM >= path.totalM) return path.coordinates[last]

  const index = lowerBoundNumbers(path.cumulative, targetM)
  const previous = Math.max(0, index - 1)
  const next = Math.min(last, index)
  const d1 = path.cumulative[previous]
  const d2 = path.cumulative[next]
  const ratio = d2 > d1 ? (targetM - d1) / (d2 - d1) : 0
  const from = path.coordinates[previous]
  const to = path.coordinates[next]

  return [
    from[0] + (to[0] - from[0]) * ratio,
    from[1] + (to[1] - from[1]) * ratio,
  ]
}

/** Points du profil compris entre deux distances (m). Repli sans géométrie. */
export function sliceProfileCoordinates(
  profile: AltimetrieProfilePoint[],
  startM: number,
  endM: number,
): [number, number][] {
  if (profile.length < 2 || endM < startM) return []

  const startIdx = lowerBound(profile, startM)
  const endIdx = lowerBound(profile, endM)
  const from = Math.max(0, startIdx)
  const to = Math.min(profile.length - 1, endIdx)
  if (to - from < 1) return []

  return profile.slice(from, to + 1).map(point => [point.lon, point.lat])
}

/** Premier point du profil dont la distance est ≥ `targetM`. */
export function profilePointAt(
  profile: AltimetrieProfilePoint[],
  targetM: number,
): AltimetrieProfilePoint | null {
  if (profile.length === 0) return null
  const index = lowerBound(profile, targetM)
  return profile[Math.min(index, profile.length - 1)] ?? null
}

/**
 * Segments de pente colorés, prêts pour la source GeoJSON.
 *
 * `path` porte la géométrie d'origine de la trace nettoyée ; sans lui (migration
 * ou géométrie indisponible), le découpage retombe sur le profil échantillonné.
 */
export function buildSegmentFeatures(
  file: AltimetrieFile,
  path: TracePath | null = null,
): GeoJSON.FeatureCollection {
  const features: GeoJSON.Feature<GeoJSON.LineString>[] = []

  for (const segment of file.segments) {
    const coordinates = sliceCoordinates(
      file,
      path,
      segment.startKm * 1000,
      segment.endKm * 1000,
    )
    if (coordinates.length < 2) continue

    features.push({
      type: 'Feature',
      properties: { color: colorForCategoryId(segment.categoryId) },
      geometry: { type: 'LineString', coordinates },
    })
  }

  return { type: 'FeatureCollection', features }
}

/** Bornes vallée/sommet de chaque montée. */
export function buildMarkerFeatures(
  file: AltimetrieFile,
  selectedClimbId: number | null,
  path: TracePath | null = null,
): GeoJSON.FeatureCollection {
  const features: GeoJSON.Feature<GeoJSON.Point>[] = []

  for (const climb of file.climbs) {
    const valley = coordinateAt(
      file,
      path,
      climb.startKm * 1000,
    )
    const summit = coordinateAt(
      file,
      path,
      climb.endKm * 1000,
    )
    const selected = climb.id === selectedClimbId

    if (valley) {
      features.push({
        type: 'Feature',
        properties: { role: 'valley', climbId: climb.id, selected },
        geometry: { type: 'Point', coordinates: valley },
      })
    }
    if (summit) {
      features.push({
        type: 'Feature',
        properties: { role: 'summit', climbId: climb.id, selected },
        geometry: { type: 'Point', coordinates: summit },
      })
    }
  }

  return { type: 'FeatureCollection', features }
}

/** Portion de trace de la montée sélectionnée, colorée par sa pente moyenne. */
export function buildSelectedClimbFeature(
  file: AltimetrieFile,
  selectedClimbId: number | null,
  path: TracePath | null = null,
): GeoJSON.FeatureCollection {
  if (selectedClimbId === null) return emptyFC()

  const climb = file.climbs.find(entry => entry.id === selectedClimbId)
  if (!climb) return emptyFC()

  const coordinates = sliceCoordinates(
    file,
    path,
    climb.startKm * 1000,
    climb.endKm * 1000,
  )
  if (coordinates.length < 2) return emptyFC()

  return {
    type: 'FeatureCollection',
    features: [
      {
        type: 'Feature',
        properties: { color: colorForSlope(climb.avgSlopePct) },
        geometry: { type: 'LineString', coordinates },
      },
    ],
  }
}

/** Découpe sur la géométrie d'origine, avec repli sur le profil échantillonné. */
function sliceCoordinates(
  file: AltimetrieFile,
  path: TracePath | null,
  startM: number,
  endM: number,
): [number, number][] {
  return path
    ? slicePathCoordinates(path, startM, endM)
    : sliceProfileCoordinates(file.profile, startM, endM)
}

/** Coordonnée d'une borne, sur la géométrie d'origine ou le profil. */
function coordinateAt(
  file: AltimetrieFile,
  path: TracePath | null,
  targetM: number,
): [number, number] | null {
  if (path) return coordinateAtDistance(path, targetM)
  const point = profilePointAt(file.profile, targetM)
  return point ? [point.lon, point.lat] : null
}

/** Coordonnées d'une feature LineString ou MultiLineString. */
function geometryCoordinates(
  geometry: GeoJSON.Feature | null,
): [number, number][] {
  const shape = geometry?.geometry
  if (!shape) return []

  if (shape.type === 'LineString') {
    return shape.coordinates.map(
      position => [position[0], position[1]] as [number, number],
    )
  }
  if (shape.type === 'MultiLineString') {
    return shape.coordinates.flatMap(line =>
      line.map(position => [position[0], position[1]] as [number, number]),
    )
  }
  return []
}

/** Premier index dont la valeur est ≥ `target`. */
function lowerBoundNumbers(values: number[], target: number): number {
  let lo = 0
  let hi = values.length - 1
  while (lo < hi) {
    const mid = (lo + hi) >> 1
    if (values[mid] < target) lo = mid + 1
    else hi = mid
  }
  return lo
}

/** Résout une source GeoJSON de la carte. */
export function geojsonSource(
  map: Map,
  sourceId: string,
): GeoJSONSource | undefined {
  return map.getSource(sourceId) as GeoJSONSource | undefined
}

function lowerBound(profile: AltimetrieProfilePoint[], targetM: number): number {
  let lo = 0
  let hi = profile.length - 1
  while (lo < hi) {
    const mid = (lo + hi) >> 1
    if (profile[mid].d < targetM) lo = mid + 1
    else hi = mid
  }
  return lo
}
