// Contrat de rendu de la carte Altimétrie : sources, couches et découpage du
// profil en segments colorés.
//
// La carte affiche :
// - les segments de pente colorés, découpés sur la géométrie d'origine de la
//   trace nettoyée (`trace.geojson`) ;
// - les bornes vallée/sommet de chaque montée ;
// - la montée sélectionnée, en halo sous les segments ;
// - en mode Tunnels, les tunnels en blanc bordé de noir, avec un halo bleu et
//   une paire entrée/sortie pour le groupe sélectionné. Un groupe fusionné est
//   peint comme une seule bande continue : l'écart n'est pas dessiné comme du
//   tunnel dans `tunnels.json`, mais il l'est à l'écran pour donner une
//   représentation unique de la fusion.
//
// Les couleurs viennent de `altimetrieCategories` : elles sont identiques dans
// la liste, le graphe et la carte.

import type {
  CircleLayerSpecification,
  ExpressionSpecification,
  GeoJSONSource,
  LineLayerSpecification,
  Map,
  SymbolLayerSpecification,
} from 'mapbox-gl'
import type {
  AltimetrieFile,
  AltimetrieProfilePoint,
} from '../../stores/altimetrie'
import type { Tunnel } from '../../stores/tunnels'
import { groupTunnels } from '../../utils/altimetrieTunnels'
import { colorForCategoryId } from '../../utils/altimetrieCategories'
import { haversineMeters } from '../../utils/geo'

export const ALTIMETRIE_MAP_STYLE = 'mapbox://styles/mapbox/standard-satellite'
export const ALTIMETRIE_TERRAIN_SOURCE = 'altimetrie-terrain'

export const ALTIMETRIE_SOURCE_IDS = {
  segments: 'altimetrie-segments',
  markers: 'altimetrie-markers',
  selected: 'altimetrie-selected',
  tunnels: 'altimetrie-tunnels',
  tunnelEndpoints: 'altimetrie-tunnel-endpoints',
  creationPick: 'altimetrie-creation-pick',
} as const

export const ALTIMETRIE_LAYER_IDS = {
  segmentsCasing: 'altimetrie-segments-casing',
  selectedCasing: 'altimetrie-selected-casing',
  selected: 'altimetrie-selected-line',
  segments: 'altimetrie-segments-line',
  selectedTunnels: 'altimetrie-tunnels-selected',
  tunnelsCasing: 'altimetrie-tunnels-casing',
  tunnels: 'altimetrie-tunnels-line',
  markers: 'altimetrie-climb-markers',
  tunnelEndpointCircles: 'altimetrie-tunnel-endpoint-circles',
  tunnelEndpointPins: 'altimetrie-tunnel-endpoint-pins',
  creationPick: 'altimetrie-creation-pick',
} as const

const TUNNEL_ENTRY_PIN_IMAGE = 'altimetrie-tunnel-entry-pin'
const TUNNEL_EXIT_PIN_IMAGE = 'altimetrie-tunnel-exit-pin'

export function emptyFC(): GeoJSON.FeatureCollection {
  return { type: 'FeatureCollection', features: [] }
}

/**
 * Dessine un pin dans un canvas et le retourne en `ImageData`.
 *
 * Le canvas est produit en 2× pour rester net sur les écrans Retina ; les
 * couleurs sont celles de Material Design, avec un liseré blanc pour tenir sur
 * le fond satellite.
 */
function createTunnelPinImage(color: string): ImageData | null {
  const canvas = document.createElement('canvas')
  canvas.width = 48
  canvas.height = 60
  const context = canvas.getContext('2d')
  if (!context) return null

  context.clearRect(0, 0, canvas.width, canvas.height)
  context.beginPath()
  context.moveTo(24, 58)
  context.bezierCurveTo(20, 50, 6, 38, 6, 22)
  context.bezierCurveTo(6, 12, 14, 4, 24, 4)
  context.bezierCurveTo(34, 4, 42, 12, 42, 22)
  context.bezierCurveTo(42, 38, 28, 50, 24, 58)
  context.closePath()
  context.fillStyle = color
  context.fill()
  context.lineWidth = 4
  context.strokeStyle = '#FFFFFF'
  context.stroke()

  context.beginPath()
  context.arc(24, 22, 7, 0, Math.PI * 2)
  context.fillStyle = '#FFFFFF'
  context.fill()

  return context.getImageData(0, 0, canvas.width, canvas.height)
}

/** Ajoute les images des pins d'entrée et de sortie, une seule fois. */
function addTunnelEndpointImages(map: Map): void {
  const entry = createTunnelPinImage('#4CAF50')
  const exit = createTunnelPinImage('#F44336')

  if (entry && !map.hasImage(TUNNEL_ENTRY_PIN_IMAGE)) {
    map.addImage(TUNNEL_ENTRY_PIN_IMAGE, entry, { pixelRatio: 2 })
  }
  if (exit && !map.hasImage(TUNNEL_EXIT_PIN_IMAGE)) {
    map.addImage(TUNNEL_EXIT_PIN_IMAGE, exit, { pixelRatio: 2 })
  }
}

/** Ajoute les sources et les couches, une seule fois, après le `load`. */
export function addAltimetrieLayers(map: Map): void {
  const empty = emptyFC()

  map.addSource(ALTIMETRIE_SOURCE_IDS.segments, { type: 'geojson', data: empty })
  map.addSource(ALTIMETRIE_SOURCE_IDS.markers, { type: 'geojson', data: empty })
  map.addSource(ALTIMETRIE_SOURCE_IDS.selected, { type: 'geojson', data: empty })
  map.addSource(ALTIMETRIE_SOURCE_IDS.tunnels, { type: 'geojson', data: empty })
  map.addSource(ALTIMETRIE_SOURCE_IDS.tunnelEndpoints, {
    type: 'geojson',
    data: empty,
  })
  map.addSource(ALTIMETRIE_SOURCE_IDS.creationPick, {
    type: 'geojson',
    data: empty,
  })

  addTunnelEndpointImages(map)

  // Ordre d'empilement (bas → haut) : les segments de pente restent au-dessus
  // de la montée sélectionnée, qui ne sert plus que de halo coloré.
  const segmentsCasing: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.segmentsCasing,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.segments,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': '#ffffff',
      'line-width': 7,
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
      'line-width': 10,
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
      // Couleur grise par défaut ; `AltimetrieMap` applique le réglage
      // `Altimetrie.Visualisation.couleurTraceSelectionnee` après le `load`.
      'line-color': '#607D8B',
      'line-width': 8,
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

  // Couche Tunnels : halo bleu de sélection, liseré noir puis ligne blanche.
  // Elle est au-dessus des segments de pente et sous les bornes de montée
  // (vides en mode Tunnels). Les tronçons d'un groupe fusionné restent des
  // LineString distinctes : l'écart n'est jamais peint.
  const selectedTunnels: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.selectedTunnels,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.tunnels,
    filter: ['==', ['get', 'selected'], true] as unknown as ExpressionSpecification,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': '#2196F3',
      'line-width': 9,
      'line-opacity': 0.8,
    },
  }
  map.addLayer(selectedTunnels)

  const tunnelsCasing: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.tunnelsCasing,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.tunnels,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': '#000000',
      'line-width': 6,
      'line-opacity': 0.95,
    },
  }
  map.addLayer(tunnelsCasing)

  const tunnels: LineLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.tunnels,
    type: 'line',
    source: ALTIMETRIE_SOURCE_IDS.tunnels,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': '#ffffff',
      'line-width': 3.2,
      'line-opacity': 1,
    },
  }
  map.addLayer(tunnels)

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

  // Portails non sélectionnés : cercles blanc (entrée) / noir (sortie), au
  // même diamètre que les bornes de montée (4 px, 6 px lorsque sélectionnée).
  const tunnelEndpointCircles: CircleLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.tunnelEndpointCircles,
    type: 'circle',
    source: ALTIMETRIE_SOURCE_IDS.tunnelEndpoints,
    filter: [
      '==',
      ['get', 'selected'],
      false,
    ] as unknown as ExpressionSpecification,
    paint: {
      'circle-radius': [
        'case',
        ['get', 'selected'],
        6,
        4,
      ] as unknown as ExpressionSpecification,
      'circle-color': [
        'match',
        ['get', 'role'],
        'entry',
        '#FFFFFF',
        '#000000',
      ] as unknown as ExpressionSpecification,
      'circle-stroke-color': [
        'match',
        ['get', 'role'],
        'entry',
        '#000000',
        '#FFFFFF',
      ] as unknown as ExpressionSpecification,
      'circle-stroke-width': 2,
      'circle-opacity': 1,
    },
  }
  map.addLayer(tunnelEndpointCircles)

  // Portails du groupe sélectionné : pins Material Design, ancrés au portail.
  const tunnelEndpointPins: SymbolLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.tunnelEndpointPins,
    type: 'symbol',
    source: ALTIMETRIE_SOURCE_IDS.tunnelEndpoints,
    filter: [
      '==',
      ['get', 'selected'],
      true,
    ] as unknown as ExpressionSpecification,
    layout: {
      'icon-image': [
        'match',
        ['get', 'role'],
        'entry',
        TUNNEL_ENTRY_PIN_IMAGE,
        TUNNEL_EXIT_PIN_IMAGE,
      ] as unknown as ExpressionSpecification,
      'icon-size': 1,
      'icon-anchor': 'bottom',
      'icon-allow-overlap': true,
      'icon-ignore-placement': true,
      'icon-pitch-alignment': 'viewport',
      'icon-rotation-alignment': 'viewport',
    },
  }
  map.addLayer(tunnelEndpointPins)

  // Premier point de la création en cours : simple repère bleu, au-dessus des
  // couches de données pour rester visible pendant la sélection.
  const creationPick: CircleLayerSpecification = {
    id: ALTIMETRIE_LAYER_IDS.creationPick,
    type: 'circle',
    source: ALTIMETRIE_SOURCE_IDS.creationPick,
    paint: {
      'circle-radius': 6,
      'circle-color': '#1976D2',
      'circle-stroke-color': '#FFFFFF',
      'circle-stroke-width': 2,
      'circle-opacity': 1,
    },
  }
  map.addLayer(creationPick)
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

/** Portion de trace de la montée sélectionnée (halo sous les segments). */
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
        properties: { climbId: climb.id },
        geometry: { type: 'LineString', coordinates },
      },
    ],
  }
}

/**
 * Tunnels, prêts pour la source GeoJSON.
 *
 * Un tunnel isolé est une LineString découpée sur la géométrie d'origine. Un
 * groupe fusionné est peint comme **une seule bande continue** du début du
 * premier tronçon à la fin du dernier : l'écart est visuellement absorbé pour
 * donner une représentation unique de la fusion. Les tronçons individuels
 * restent dans `tunnels.json`, seule source de vérité pour la séparation.
 */
export function buildTunnelFeatures(
  file: AltimetrieFile,
  tunnels: Tunnel[],
  selectedIds: string[],
  path: TracePath | null = null,
): GeoJSON.FeatureCollection {
  const selected = new Set(selectedIds)
  const features: GeoJSON.Feature<GeoJSON.LineString>[] = []

  for (const group of groupTunnels(tunnels)) {
    const coordinates = sliceCoordinates(
      file,
      path,
      group.startKm * 1000,
      group.endKm * 1000,
    )
    if (coordinates.length < 2) continue

    features.push({
      type: 'Feature',
      properties: {
        tunnelId: group.id,
        groupId: group.groupId,
        selected: group.tunnels.some(tunnel => selected.has(tunnel.id)),
      },
      geometry: { type: 'LineString', coordinates },
    })
  }

  return { type: 'FeatureCollection', features }
}

/**
 * Portails des tunnels : une entrée et une sortie par tunnel logique.
 *
 * Un groupe fusionné ne produit qu'une paire, placée aux bornes extérieures
 * (`group.startKm` / `group.endKm`). `selected` porte sur le groupe entier.
 */
export function buildTunnelEndpointFeatures(
  file: AltimetrieFile,
  tunnels: Tunnel[],
  selectedIds: string[],
  path: TracePath | null = null,
): GeoJSON.FeatureCollection {
  const selected = new Set(selectedIds)
  const features: GeoJSON.Feature<GeoJSON.Point>[] = []

  for (const group of groupTunnels(tunnels)) {
    const isSelected = group.tunnels.some(tunnel => selected.has(tunnel.id))
    const endpoints: Array<{
      role: 'entry' | 'exit'
      km: number
      tunnelId: string
    }> = [
      {
        role: 'entry',
        km: group.startKm,
        tunnelId: group.tunnels[0]?.id ?? group.id,
      },
      {
        role: 'exit',
        km: group.endKm,
        tunnelId: group.tunnels[group.tunnels.length - 1]?.id ?? group.id,
      },
    ]

    for (const endpoint of endpoints) {
      const coordinates = coordinateAt(file, path, endpoint.km * 1000)
      if (!coordinates) continue

      features.push({
        type: 'Feature',
        properties: {
          role: endpoint.role,
          groupId: group.groupId,
          tunnelId: endpoint.tunnelId,
          selected: isSelected,
          draggable: !group.merged,
        },
        geometry: { type: 'Point', coordinates },
      })
    }
  }

  return { type: 'FeatureCollection', features }
}

/** Repère du premier point d'une création en cours, ou collection vide. */
export function buildCreationPickFeature(
  file: AltimetrieFile,
  km: number | null,
  path: TracePath | null = null,
): GeoJSON.FeatureCollection {
  if (km === null) return emptyFC()
  const coordinate = coordinateAt(file, path, km * 1000)
  if (!coordinate) return emptyFC()

  return {
    type: 'FeatureCollection',
    features: [
      {
        type: 'Feature',
        properties: {},
        geometry: { type: 'Point', coordinates: coordinate },
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
