<template>
  <div ref="mapContainer" class="map-container"></div>
</template>

<script setup lang="ts">
/**
 * Composant carte Mapbox GL avec clustering des points de départ.
 *
 * Affiche tous les points de départ (stats.start_point) des traces importées
 * sous forme de clusters (regroupements automatiques selon le niveau de zoom).
 * Synchronise le centre de la carte avec le store pour le tri par distance.
 *
 * Conventions respectées :
 * - <script setup lang="ts">, imports relatifs
 * - Token Mapbox depuis settingsStore (Systeme.Key.mapBox)
 * - Données depuis tracesStore (pas de fetch direct)
 * - Debounce sur moveend pour limiter les recalculs
 */
import { ref, onMounted, onUnmounted, watch } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import { useSettingsStore } from '../../stores/settings'
import { useTracesStore } from '../../stores/traces'
import { formatCoordinate } from '../../utils/format'

// --- Stores ---

const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()

// --- Références ---

const mapContainer = ref<HTMLDivElement | null>(null)
let map: mapboxgl.Map | null = null
let moveEndTimer: ReturnType<typeof setTimeout> | null = null
const MOVE_END_DEBOUNCE_MS = 150

// --- Construction GeoJSON ---

/**
 * Construit un FeatureCollection GeoJSON à partir des traces du store.
 * Les coordonnées sont en [longitude, latitude] (convention Mapbox).
 */
function buildTracesGeoJSON(): GeoJSON.FeatureCollection<GeoJSON.Geometry> {
  return {
    type: 'FeatureCollection',
    features: tracesStore.traces.map(t => ({
      type: 'Feature' as const,
      geometry: {
        type: 'Point' as const,
        coordinates: [t.stats.start_point.lon, t.stats.start_point.lat],
      },
      properties: {
        id: t.id,
        name: t.name,
        source: t.source,
        activity_type: t.activity_type,
        start_lat: t.stats.start_point.lat,
        start_lon: t.stats.start_point.lon,
      },
    })),
  }
}

// --- Couches de clustering ---

/** Ajoute la source clusterisée et les 3 couches (clusters, compteur, points). */
function addClusterLayers() {
  if (!map) return

  map.addSource('traces', {
    type: 'geojson',
    data: buildTracesGeoJSON(),
    cluster: true,
    clusterRadius: 50,
    clusterMaxZoom: 14,
  })

  // Couche des clusters (cercles colorés par paliers)
  map.addLayer({
    id: 'clusters',
    type: 'circle',
    source: 'traces',
    filter: ['has', 'point_count'],
    paint: {
      'circle-color': [
        'step',
        ['get', 'point_count'],
        '#51bbd6',
        10,
        '#f1f075',
        100,
        '#f28cb1',
      ],
      'circle-radius': [
        'step',
        ['get', 'point_count'],
        20,
        30,
        40,
      ],
    },
  })

  // Compteur dans chaque cluster
  map.addLayer({
    id: 'cluster-count',
    type: 'symbol',
    source: 'traces',
    filter: ['has', 'point_count'],
    layout: {
      'text-field': '{point_count_abbreviated}',
      'text-size': 12,
    },
  })

  // Points individuels (non clusterés)
  map.addLayer({
    id: 'unclustered-point',
    type: 'circle',
    source: 'traces',
    filter: ['!', ['has', 'point_count']],
    paint: {
      'circle-color': '#4264fb',
      'circle-radius': 8,
      'circle-stroke-width': 1,
      'circle-stroke-color': '#fff',
    },
  })
}

// --- Gestion des clics ---

/** Clic sur un cluster : zoom vers le centre du cluster au niveau d'expansion. */
function handleClusterClick(e: mapboxgl.MapMouseEvent) {
  if (!map) return
  const feature = e.features?.[0]
  if (!feature || !feature.properties || feature.properties.cluster_id == null) return

  const clusterId = feature.properties.cluster_id as number
  const source = map.getSource('traces') as mapboxgl.GeoJSONSource
  source.getClusterExpansionZoom(clusterId, (err, zoom) => {
    if (err || !map) return
    map.easeTo({
      center: (feature.geometry as GeoJSON.Point).coordinates as [number, number],
      zoom: zoom ?? map.getZoom(),
    })
  })
}

/** Clic sur un point individuel : popup avec nom et coordonnées. */
function handlePointClick(e: mapboxgl.MapMouseEvent) {
  if (!map) return
  const feature = e.features?.[0]
  if (!feature || !feature.properties) return

  const coords = (feature.geometry as GeoJSON.Point).coordinates.slice() as [number, number]
  const props = feature.properties

  new mapboxgl.Popup({ offset: 12, closeButton: true })
    .setLngLat(coords)
    .setHTML(
      `<div style="font-family: sans-serif; padding: 4px 0;">
        <strong>${props.name ?? 'Point'}</strong><br/>
        <span style="color: #666; font-size: 0.85em;">
          ${props.source ?? ''}${props.activity_type ? ' · ' + props.activity_type : ''}
        </span><br/>
        <span style="font-size: 0.85em;">
          ${formatCoordinate(props.start_lat as number)}, ${formatCoordinate(props.start_lon as number)}
        </span>
      </div>`,
    )
    .addTo(map)
}

// --- Gestion du curseur ---

/** Change le curseur en pointer au survol des clusters et points. */
function addCursorHandlers() {
  if (!map) return
  const canvas = map.getCanvas()

  // Clusters
  map.on('mouseenter', 'clusters', () => { canvas.style.cursor = 'pointer' })
  map.on('mouseleave', 'clusters', () => { canvas.style.cursor = '' })

  // Points individuels
  map.on('mouseenter', 'unclustered-point', () => { canvas.style.cursor = 'pointer' })
  map.on('mouseleave', 'unclustered-point', () => { canvas.style.cursor = '' })
}

// --- Synchronisation centre → store ---

/** Met à jour le centre dans le store (avec debounce). */
function onMapMoveEnd() {
  if (!map) return
  if (moveEndTimer) clearTimeout(moveEndTimer)
  moveEndTimer = setTimeout(() => {
    const c = map!.getCenter()
    tracesStore.updateMapCenter(c.lat, c.lng)
  }, MOVE_END_DEBOUNCE_MS)
}

// --- Initialisation ---

function initializeMap() {
  if (!mapContainer.value) return

  map = new mapboxgl.Map({
    container: mapContainer.value,
    style: 'mapbox://styles/mapbox/standard',
    zoom: 5.15,
    center: [2.0, 43.7], // [lon, lat] — France
  })

  map.on('load', () => {
    addClusterLayers()
    addCursorHandlers()

    // Synchroniser le centre initial
    const c = map!.getCenter()
    tracesStore.updateMapCenter(c.lat, c.lng)
  })

  map.on('moveend', onMapMoveEnd)
  map.on('click', 'clusters', handleClusterClick)
  map.on('click', 'unclustered-point', handlePointClick)
}

// --- Réactivité : mise à jour des données sans recréer la carte ---

watch(
  () => tracesStore.traces,
  () => {
    if (!map) return
    const source = map.getSource('traces') as mapboxgl.GeoJSONSource | undefined
    if (source) {
      source.setData(buildTracesGeoJSON())
    }
  },
)

// --- Cycle de vie ---

onMounted(async () => {
  // Charger le token Mapbox depuis les paramètres
  let token = ''
  try {
    token = await settingsStore.getSettingValue('Systeme.Key.mapBox')
  } catch (error) {
    console.error('Failed to retrieve MapBox token from settings:', error)
  }

  mapboxgl.accessToken = token || ''

  // Initialiser la carte
  initializeMap()
})

onUnmounted(() => {
  if (moveEndTimer) clearTimeout(moveEndTimer)
  if (map) {
    map.remove()
    map = null
  }
})
</script>

<style>
/* make the map container fill its parent */
.map-container {
  width: 100%;
  height: 100%;
}
</style>
