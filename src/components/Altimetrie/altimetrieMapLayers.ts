// Contrat de rendu de la carte Altimétrie : sources, couches et découpage du
// profil en segments colorés.
//
// La carte affiche :
// - la trace complète en trait neutre, pour conserver la géométrie exacte ;
// - les segments de pente colorés, construits à partir du profil échantillonné
//   du fichier `altimetrie.json` ;
// - les bornes vallée/sommet de chaque montée ;
// - la montée sélectionnée, en surépaisseur.
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

export const ALTIMETRIE_MAP_STYLE = 'mapbox://styles/mapbox/outdoors-v12'
export const ALTIMETRIE_TERRAIN_SOURCE = 'altimetrie-terrain'

export const ALTIMETRIE_SOURCE_IDS = {
  trace: 'altimetrie-trace',
  segments: 'altimetrie-segments',
  markers: 'altimetrie-markers',
  selected: 'altimetrie-selected',
} as const

export const ALTIMETRIE_LAYER_IDS = {
  traceCasing: 'altimetrie-trace-casing',
  trace: 'altimetrie-trace-line',
  segmentsCasing: 'altimetrie-segments-casing',
  segments: 'altimetrie-segments-line',
  selectedCasing: 'altimetrie-selected-casing',
  selected: 'altimetrie-selected-line',
  markers: 'altimetrie-climb-markers',
} as const

export function emptyFC(): GeoJSON.FeatureCollection {
  return { type: 'FeatureCollection', features: [] }
}

/** Ajoute les sources et les couches, une seule fois, après le `load`. */
export function addAltimetrieLayers(map: Map): void {
  const empty = emptyFC()

  map.addSource(ALTIMETRIE_SOURCE_IDS.trace, { type: 'geojson', data: empty })
  map.addSource(ALTIMETRIE_SOURCE_IDS.segments, { type: 'geojson', data: empty })
  map.addSource(ALTIMETRIE_SOURCE_IDS.markers, { type: 'geojson', data: empty })
  map.addSource(ALTIMETRIE_SOURCE_IDS.selected, { type: 'geojson', data: empty })

  const traceCasing: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.traceCasing,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.trace,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': '#ffffff',
      'line-width': 5,
      'line-opacity': 0.55,
    },
  }
  map.addLayer(traceCasing)

  const trace: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.trace,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.trace,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': '#1f2937',
      'line-width': 2,
      'line-opacity': 0.55,
    },
  }
  map.addLayer(trace)

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

  const selectedCasing: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.selectedCasing,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.selected,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': '#ffffff',
      'line-width': 9,
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
      'line-width': 6,
      'line-opacity': 0.95,
    },
  }
  map.addLayer(selected)

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

/** Points du profil compris entre deux distances (m). */
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

/** Segments de pente colorés, prêts pour la source GeoJSON. */
export function buildSegmentFeatures(
  file: AltimetrieFile,
): GeoJSON.FeatureCollection {
  const features: GeoJSON.Feature<GeoJSON.LineString>[] = []

  for (const segment of file.segments) {
    const coordinates = sliceProfileCoordinates(
      file.profile,
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
): GeoJSON.FeatureCollection {
  const features: GeoJSON.Feature<GeoJSON.Point>[] = []

  for (const climb of file.climbs) {
    const valley = profilePointAt(file.profile, climb.startKm * 1000)
    const summit = profilePointAt(file.profile, climb.endKm * 1000)
    const selected = climb.id === selectedClimbId

    if (valley) {
      features.push({
        type: 'Feature',
        properties: { role: 'valley', climbId: climb.id, selected },
        geometry: { type: 'Point', coordinates: [valley.lon, valley.lat] },
      })
    }
    if (summit) {
      features.push({
        type: 'Feature',
        properties: { role: 'summit', climbId: climb.id, selected },
        geometry: { type: 'Point', coordinates: [summit.lon, summit.lat] },
      })
    }
  }

  return { type: 'FeatureCollection', features }
}

/** Portion de trace de la montée sélectionnée, colorée par sa pente moyenne. */
export function buildSelectedClimbFeature(
  file: AltimetrieFile,
  selectedClimbId: number | null,
): GeoJSON.FeatureCollection {
  if (selectedClimbId === null) return emptyFC()

  const climb = file.climbs.find(entry => entry.id === selectedClimbId)
  if (!climb) return emptyFC()

  const coordinates = sliceProfileCoordinates(
    file.profile,
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
