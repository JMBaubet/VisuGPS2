<template>
  <div class="cleaning-map-root">
    <div ref="mapContainer" class="cleaning-map-container"></div>
    <!-- Cadrage sur la trace complète (le cadrage par défaut suit la zone du cas) -->
    <v-btn
      class="fit-full-btn"
      variant="flat"
      size="small"
      prepend-icon="mdi-fit-to-page-outline"
      title="Cadrer sur la trace complète"
      @click="fitFullTrace"
    >
      Trace complète
    </v-btn>
  </div>
</template>

<script setup lang="ts">
/**
 * Carte Mapbox GL de la vue de nettoyage de trace.
 *
 * Affichage :
 * - la trace complète en filigrane (gris, pointillés) ;
 * - le **segment courant** (zone du cas) surligné ;
 * - pour un aller-retour (`out_and_back`), les branches **aller** et **retour**
 *   décalées perpendiculairement (`line-offset`) et colorées différemment pour
 *   distinguer les passages superposés ;
 * - les points de la zone numérotés (index GPX), cliquables pour basculer
 *   leur suppression ;
 * - les points ajoutés (cas `parallel`) : déplaçables à la souris, retirés par
 *   simple clic.
 *
 * Le clic sur la carte, quand le cas courant est en mode ajout (`parallel`),
 * pose un nouveau point après le point de la zone le plus proche.
 *
 * Conventions reprises de EditionMap.vue / Accueil/Map.vue : token Mapbox
 * depuis `Systeme.Key.mapBox`, `ResizeObserver` pour le redimensionnement,
 * destruction de la carte au démontage.
 */
import { ref, watch, onMounted, onUnmounted } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import { useSettingsStore } from '../../stores/settings'
import { useCleaningStore } from '../../stores/cleaning'

const settingsStore = useSettingsStore()
const cleaning = useCleaningStore()

const mapContainer = ref<HTMLDivElement | null>(null)
let map: mapboxgl.Map | null = null
let resizeObserver: ResizeObserver | null = null

// --- Identifiants de sources / couches ---

const FULL_SOURCE = 'clean-full'
const ZONE_SOURCE = 'clean-zone'
const BRANCH_A_SOURCE = 'clean-branch-a'
const BRANCH_B_SOURCE = 'clean-branch-b'
const POINTS_SOURCE = 'clean-points'
const POINTS_LAYER = 'clean-points-layer'
const LABELS_LAYER = 'clean-labels-layer'
const INSERTED_SOURCE = 'clean-inserted'
const INSERTED_LAYER = 'clean-inserted-layer'

/** Drag en cours d'un point ajouté (index dans insert_points du cas courant). */
let dragInserted: { index: number; moved: boolean } | null = null

// --- Helpers GeoJSON ---

function lineFeature(coords: number[][]): GeoJSON.Feature {
  return {
    type: 'Feature',
    properties: {},
    geometry: { type: 'LineString', coordinates: coords },
  }
}

/** FeatureCollection vide (initiale, avant chargement des données). */
function emptyFC(): GeoJSON.FeatureCollection {
  return { type: 'FeatureCollection', features: [] }
}

function pointFeatures(): GeoJSON.FeatureCollection {
  const c = cleaning.currentCase
  const pts = cleaning.points
  if (!c || pts.length === 0) return { type: 'FeatureCollection', features: [] }

  const features: GeoJSON.Feature[] = []
  for (let i = c.start_index; i <= c.end_index && i < pts.length; i++) {
    const p = pts[i]
    const status = c.apex_indices.includes(i)
      ? 'apex'
      : cleaning.isDeleted(i)
        ? 'deleted'
        : 'normal'
    features.push({
      type: 'Feature',
      properties: { i, label: String(i + 1), status },
      geometry: { type: 'Point', coordinates: [p.lon, p.lat] },
    })
  }
  return { type: 'FeatureCollection', features }
}

function insertedFeatures(): GeoJSON.FeatureCollection {
  const c = cleaning.currentCase
  if (!c) return { type: 'FeatureCollection', features: [] }
  return {
    type: 'FeatureCollection',
    features: c.correction.insert_points.map((p, idx) => ({
      type: 'Feature',
      properties: { idx, label: `A${idx + 1}` },
      geometry: { type: 'Point', coordinates: [p.lon, p.lat] },
    })),
  }
}

// --- Rendu ---

function setData(sourceId: string, data: GeoJSON.Feature | GeoJSON.FeatureCollection) {
  const source = map?.getSource(sourceId) as mapboxgl.GeoJSONSource | undefined
  if (source) source.setData(data)
}

function setLayerVisibility(layerId: string, visible: boolean) {
  if (map?.getLayer(layerId)) {
    map.setLayoutProperty(layerId, 'visibility', visible ? 'visible' : 'none')
  }
}

function renderFullTrace() {
  const coords = cleaning.points.map(p => [p.lon, p.lat] as number[])
  setData(FULL_SOURCE, lineFeature(coords))
}

function renderZone() {
  const c = cleaning.currentCase
  const pts = cleaning.points
  if (!c || pts.length === 0) {
    setData(ZONE_SOURCE, lineFeature([]))
    setData(BRANCH_A_SOURCE, lineFeature([]))
    setData(BRANCH_B_SOURCE, lineFeature([]))
    return
  }  const zone = pts
    .slice(c.start_index, c.end_index + 1)
    .map(p => [p.lon, p.lat] as number[])
  setData(ZONE_SOURCE, lineFeature(zone))

  // Branches aller / retour pour les aller-retours (décalage latéral visuel).
  const hasBranches = c.kind === 'out_and_back' || c.kind === 'parallel'
  if (hasBranches) {
    const apex = c.apex_indices[0] ?? Math.floor((c.start_index + c.end_index) / 2)
    const a = pts
      .slice(c.start_index, Math.min(apex, c.end_index) + 1)
      .map(p => [p.lon, p.lat] as number[])
    const b = pts
      .slice(Math.min(apex, c.end_index), c.end_index + 1)
      .map(p => [p.lon, p.lat] as number[])
    setData(BRANCH_A_SOURCE, lineFeature(a))
    setData(BRANCH_B_SOURCE, lineFeature(b))
    setLayerVisibility(BRANCH_A_SOURCE, true)
    setLayerVisibility(BRANCH_B_SOURCE, true)
  } else {
    setData(BRANCH_A_SOURCE, lineFeature([]))
    setData(BRANCH_B_SOURCE, lineFeature([]))
    setLayerVisibility(BRANCH_A_SOURCE, false)
    setLayerVisibility(BRANCH_B_SOURCE, false)
  }
}

function renderPoints() {
  setData(POINTS_SOURCE, pointFeatures())
  setData(INSERTED_SOURCE, insertedFeatures())
}

function renderAll() {
  if (!map) return
  renderFullTrace()
  renderZone()
  renderPoints()
}

function fitZone() {
  const c = cleaning.currentCase
  const pts = cleaning.points
  if (!map || !c || pts.length === 0) return
  const bounds = new mapboxgl.LngLatBounds()
  for (let i = c.start_index; i <= c.end_index && i < pts.length; i++) {
    bounds.extend([pts[i].lon, pts[i].lat])
  }
  map.fitBounds(bounds, { padding: 80, maxZoom: 18, duration: 600 })
}

/** Cadre la carte sur la **trace complète** (contexte global). */
function fitFullTrace() {
  const pts = cleaning.points
  if (!map || pts.length === 0) return
  const bounds = new mapboxgl.LngLatBounds()
  for (const p of pts) bounds.extend([p.lon, p.lat])
  map.fitBounds(bounds, { padding: 50, duration: 600 })
}

/** Index du point de la zone le plus proche du clic (pour l'insertion). */
function nearestIndexInZone(lngLat: mapboxgl.LngLat): number {
  const c = cleaning.currentCase
  const pts = cleaning.points
  if (!c) return -1
  let best = -1
  let bestDist = Infinity
  for (let i = c.start_index; i <= c.end_index && i < pts.length; i++) {
    const p = pts[i]
    const d = (p.lon - lngLat.lng) ** 2 + (p.lat - lngLat.lat) ** 2
    if (d < bestDist) {
      bestDist = d
      best = i
    }
  }
  return best
}

// --- Initialisation de la carte ---

async function initializeMap() {
  if (!mapContainer.value) return
  let token = ''
  try {
    token = (await settingsStore.getSettingValue('Systeme.Key.mapBox')) || ''
  } catch {
    token = ''
  }
  mapboxgl.accessToken = token

  map = new mapboxgl.Map({
    container: mapContainer.value,
    style: 'mapbox://styles/mapbox/standard',
    zoom: 9,
    center: [2.5, 43.0],
    attributionControl: false,
  })
  map.addControl(new mapboxgl.AttributionControl({ compact: true }), 'top-left')
  map.addControl(new mapboxgl.NavigationControl({ showCompass: false }), 'top-right')

  map.on('load', () => {
    setupLayers()
    renderAll()
    fitZone()
  })

  // Interactions : sélection de points, ajout, déplacement des points insérés.
  map.on('click', (e) => {
    if (!map) return
    const c = cleaning.currentCase
    if (!c) return

    // Clic sur un point de la zone → bascule sa suppression.
    const hit = map.queryRenderedFeatures(e.point, {
      layers: [POINTS_LAYER, LABELS_LAYER],
    })
    if (hit.length > 0) {
      const i = Number(hit[0].properties?.i)
      if (Number.isInteger(i)) cleaning.toggleDeletePoint(i)
      return
    }
    // Clic sur un point inséré → traité par le drag (mouseup sans déplacement).
    const hitInserted = map.queryRenderedFeatures(e.point, { layers: [INSERTED_LAYER] })
    if (hitInserted.length > 0) return

    // Mode ajout (cas `parallel`) : poser un nouveau point.
    if (c.kind === 'parallel') {
      const idx = nearestIndexInZone(e.lngLat)
      if (idx >= 0) cleaning.addInsertPoint(idx, e.lngLat.lat, e.lngLat.lng)
    }
  })

  // Déplacement des points ajoutés (drag) ; un simple clic les retire.
  map.on('mousedown', (e) => {
    if (!map) return
    const feats = map.queryRenderedFeatures(e.point, { layers: [INSERTED_LAYER] })
    if (feats.length > 0) {
      const idx = Number(feats[0].properties?.idx)
      if (Number.isInteger(idx)) {
        dragInserted = { index: idx, moved: false }
        map.dragPan.disable()
        map.getCanvas().style.cursor = 'grabbing'
      }
    }
  })
  map.on('mousemove', (e) => {
    if (!dragInserted) return
    dragInserted.moved = true
    cleaning.setInsertPointPosition(dragInserted.index, e.lngLat.lat, e.lngLat.lng)
  })
  map.on('mouseup', () => {
    if (!dragInserted) return
    const { index, moved } = dragInserted
    dragInserted = null
    if (map) {
      map.dragPan.enable()
      map.getCanvas().style.cursor = ''
    }
    if (!moved) cleaning.removeInsertPoint(index)
  })
  // Curseur adaptatif sur les éléments interactifs.
  map.on('mousemove', (e) => {
    if (dragInserted || !map) return
    const hover = map.queryRenderedFeatures(e.point, {
      layers: [POINTS_LAYER, LABELS_LAYER, INSERTED_LAYER],
    })
    map.getCanvas().style.cursor = hover.length > 0 ? 'pointer' : ''
  })

  resizeObserver = new ResizeObserver(() => map?.resize())
  resizeObserver.observe(mapContainer.value)
}

function setupLayers() {
  if (!map) return

  // 1. Trace complète (ligne continue verte, pour le contexte).
  if (!map.getSource(FULL_SOURCE)) {
    map.addSource(FULL_SOURCE, { type: 'geojson', data: emptyFC() })
  }
  if (!map.getLayer(FULL_SOURCE)) {
    map.addLayer({
      id: FULL_SOURCE,
      type: 'line',
      source: FULL_SOURCE,
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: {
        'line-color': '#4CAF50', // vert — trace complète bien visible
        'line-width': 5,
        'line-opacity': 0.85,
      },
    })
  }

  // 2. Zone du cas courant.
  if (!map.getSource(ZONE_SOURCE)) {
    map.addSource(ZONE_SOURCE, { type: 'geojson', data: emptyFC() })
  }
  if (!map.getLayer(ZONE_SOURCE)) {
    map.addLayer({
      id: ZONE_SOURCE,
      type: 'line',
      source: ZONE_SOURCE,
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: {
        'line-color': '#1565c0',
        'line-width': 6,
        'line-opacity': 0.85,
      },
    })
  }

  // 3. Branches aller / retour (décalage latéral pour distinguer les passages).
  const addBranchLayer = (id: string, color: string, offset: number) => {
    if (!map) return
    if (!map.getSource(id)) map.addSource(id, { type: 'geojson', data: emptyFC() })
    if (!map.getLayer(id)) {
      map.addLayer({
        id,
        type: 'line',
        source: id,
        layout: { 'line-cap': 'round', 'line-join': 'round' },
        paint: {
          'line-color': color,
          'line-width': 3,
          'line-offset': offset,
          'line-opacity': 0.9,
        },
      })
    }
  }
  addBranchLayer(BRANCH_A_SOURCE, '#2e7d32', -7)
  addBranchLayer(BRANCH_B_SOURCE, '#ef6c00', 7)

  // 4. Points de la zone (cercles + numéros d'index GPX).
  if (!map.getSource(POINTS_SOURCE)) {
    map.addSource(POINTS_SOURCE, { type: 'geojson', data: pointFeatures() })
  }
  if (!map.getLayer(POINTS_LAYER)) {
    map.addLayer({
      id: POINTS_LAYER,
      type: 'circle',
      source: POINTS_SOURCE,
      paint: {
        'circle-radius': 5,
        'circle-stroke-width': 1,
        'circle-stroke-color': '#ffffff',
        'circle-color': [
          'match',
          ['get', 'status'],
          'apex',
          '#e53935',
          'deleted',
          '#9e9e9e',
          '#1e88e5',
        ],
        'circle-opacity': ['case', ['==', ['get', 'status'], 'deleted'], 0.35, 1],
      },
    })
  }
  if (!map.getLayer(LABELS_LAYER)) {
    map.addLayer({
      id: LABELS_LAYER,
      type: 'symbol',
      source: POINTS_SOURCE,
      layout: {
        'text-field': ['get', 'label'],
        'text-size': 10,
        'text-allow-overlap': true,
        'text-offset': [0, -0.9],
      },
      paint: {
        'text-color': [
          'match',
          ['get', 'status'],
          'apex',
          '#e53935',
          'deleted',
          '#9e9e9e',
          '#1565c0',
        ],
        'text-opacity': ['case', ['==', ['get', 'status'], 'deleted'], 0.3, 1],
        'text-halo-color': '#ffffff',
        'text-halo-width': 1.5,
      },
    })
  }

  // 5. Points ajoutés (rouge, déplaçables).
  if (!map.getSource(INSERTED_SOURCE)) {
    map.addSource(INSERTED_SOURCE, { type: 'geojson', data: insertedFeatures() })
  }
  if (!map.getLayer(INSERTED_LAYER)) {
    map.addLayer({
      id: INSERTED_LAYER,
      type: 'circle',
      source: INSERTED_SOURCE,
      paint: {
        'circle-radius': 6,
        'circle-stroke-width': 2,
        'circle-stroke-color': '#ffffff',
        'circle-color': '#d81b60',
      },
    })
  }
  if (!map.getLayer('clean-inserted-labels')) {
    map.addLayer({
      id: 'clean-inserted-labels',
      type: 'symbol',
      source: INSERTED_SOURCE,
      layout: {
        'text-field': ['get', 'label'],
        'text-size': 11,
        'text-allow-overlap': true,
        'text-offset': [0, -1],
      },
      paint: {
        'text-color': '#d81b60',
        'text-halo-color': '#ffffff',
        'text-halo-width': 1.5,
      },
    })
  }
}

// --- Réactivité ---

// Re-rendu à chaque changement du cas courant (navigation) ou des corrections.
watch(
  () => cleaning.currentCase?.id,
  () => {
    renderAll()
    fitZone()
  },
)

watch(
  () => cleaning.currentCase?.correction,
  () => renderPoints(),
  { deep: true },
)

// Rendu quand l'état est chargé (carte montée avant le chargement du store).
watch(
  () => cleaning.state,
  () => {
    if (cleaning.state) {
      renderAll()
      fitZone()
    }
  },
)

onMounted(() => {
  initializeMap()
})

onUnmounted(() => {
  resizeObserver?.disconnect()
  map?.remove()
  map = null
})
</script>

<style scoped>
.cleaning-map-root {
  position: absolute;
  inset: 0;
}

.cleaning-map-container {
  position: absolute;
  inset: 0;
}

/* Bouton flottant de cadrage global, en bas à droite (au-dessus de la carte). */
.fit-full-btn {
  position: absolute;
  right: 12px;
  bottom: 12px;
  z-index: 2;
  background: rgba(255, 255, 255, 0.92);
  color: rgba(0, 0, 0, 0.87);
}
</style>
