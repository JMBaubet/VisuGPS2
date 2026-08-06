<template>
  <div ref="mapContainer" class="edition-map-container"></div>
</template>

<script setup lang="ts">
/**
 * Carte Mapbox GL dédiée à la vue d'édition caméra.
 *
 * Contrairement à `Accueil/Map.vue` (carte de navigation avec clustering),
 * ce composant instancie une carte autonome orientée « rendu cinematic » :
 *   - style satellite (`mapbox://styles/mapbox/standard-satellite`)
 *   - activation du terrain/élévation (source raster-dem `mapbox-terrain-rgb`)
 *   - tracé de la trace sélectionnée (LineString)
 *   - curseur jaune (cercle GL, couche CircleLayer synchronisée avec le terrain)
 *
 * Le curseur est un **cercle rendu en WebGL** (CircleLayer) et non un Marker DOM.
 * Cela garantit qu'il est parfaitement synchronisé avec le terrain et la trace
 * pendant les transitions de caméra rapides (lacets, virages serrés).
 * Un Marker DOM 2D peut flotter au-dessus de la trace pendant les mouvements
 * de caméra car sa projection et le rendu du terrain WebGL sont désynchronisés.
 *
 * Après chargement de la trace, ce composant génère un jeu de keyframes
 * (`generateKeyframes`) et le pousse dans `editionStore`. Il assure ensuite
 * le rendu frame-by-frame piloté par l'état de lecture du store :
 *   - en lecture : une boucle `requestAnimationFrame` déclenche `tick()`
 *     puis applique l'état interpolé (caméra via `jumpTo`, curseur via
 *     mise à jour de la source GeoJSON) ;
 *   - en pause : l'état interpolé est appliqué uniquement sur changement
 *     du temps courant (l'utilisateur garde la main sur la carte).
 *
 * Conventions reprises de Accueil/Map.vue :
 * - <script setup lang="ts">, imports relatifs
 * - Token Mapbox depuis settingsStore (Systeme.Key.mapBox)
 * - Données depuis les stores (pas de fetch direct)
 * - ResizeObserver pour resynchroniser le canvas au redimensionnement
 */
import { ref, watch, onMounted, onUnmounted } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import { useSettingsStore } from '../../stores/settings'
import { useTracesStore } from '../../stores/traces'
import { useEditionStore } from '../../stores/edition'
import { useKeyframesStore } from '../../stores/keyframes'
import { generateKeyframes, KEYFRAME_STEP_M } from '../../algorithms/keyframeGenerator'

// --- Stores ---

const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const editionStore = useEditionStore()
const keyframesStore = useKeyframesStore()

// --- Références ---

const mapContainer = ref<HTMLDivElement | null>(null)
let map: mapboxgl.Map | null = null
/** Observer du redimensionnement du conteneur. */
let resizeObserver: ResizeObserver | null = null
/** Position courante du curseur [lng, lat], pour mise à jour source GeoJSON. */
let markerCoords: [number, number] = [0, 0]

// --- Animation (lecture) ---

/** Identifiant de la boucle requestAnimationFrame, ou null si inactive. */
let rafId: number | null = null
/** Horodatage de la frame précédente (ms), pour calculer le delta. */
let lastFrameTs: number | null = null
/**
 * `true` une fois la carte initialisée et la trace chargée. Garantit que
 * les watchers d'application caméra/marker ne s'exécutent pas avant que
 * la carte et le marker soient prêts.
 */
let editionReady = false

// --- Identifiants de couches / sources ---

const TRACE_SOURCE_ID = 'edition-trace'
const TRACE_LINE_LAYER_ID = 'edition-trace-line'
const MARKER_SOURCE_ID = 'edition-marker'
const MARKER_LAYER_ID = 'edition-marker-dot'
const TERRAIN_SOURCE_ID = 'edition-terrain'

// --- Initialisation ---

/**
 * Crée la carte, configure le style satellite + terrain, puis charge la
 * trace sélectionnée (LineString + marker + cadrage).
 */
async function initializeMap(token: string) {
  if (!mapContainer.value) return

  mapboxgl.accessToken = token || ''

  map = new mapboxgl.Map({
    container: mapContainer.value,
    style: 'mapbox://styles/mapbox/standard-satellite',
    zoom: 5,
    center: [2.0, 43.7], // [lon, lat] — France, recentré sur la trace ensuite
    pitch: 60, // pitch par défaut de la spec (§7)
  })

  map.on('load', async () => {
    if (!map) return

    // 1. Activer le terrain/élévation (source raster-dem Mapbox).
    //    exaggeration 1.5 pour souligner le relief (cohérent avec une
    //    visualisation « cinematic » de la trace).
    if (!map.getSource(TERRAIN_SOURCE_ID)) {
      map.addSource(TERRAIN_SOURCE_ID, {
        type: 'raster-dem',
        url: 'mapbox://mapbox.terrain-rgb',
        tileSize: 512,
        maxzoom: 14,
      })
    }
    map.setTerrain({ source: TERRAIN_SOURCE_ID, exaggeration: 1.5 })

    // 2. Source + couche de la trace (LineString blanche épaisse pour
    //    rester lisible sur fond satellite).
    if (!map.getSource(TRACE_SOURCE_ID)) {
      map.addSource(TRACE_SOURCE_ID, {
        type: 'geojson',
        data: { type: 'FeatureCollection', features: [] },
      })
    }
      if (!map.getLayer(TRACE_LINE_LAYER_ID)) {
      map.addLayer({
        id: TRACE_LINE_LAYER_ID,
        type: 'line',
        source: TRACE_SOURCE_ID,
        layout: {
          'line-cap': 'round',
          'line-join': 'round',
        },
        paint: {
          'line-color': '#FF0000',
          'line-width': 5,
          'line-opacity': 0.9,
        },
      })
    }

    // 2bis. Curseur (cercle jaune bordé de blanc, rendu WebGL).
    // Rendu via CircleLayer au lieu d'un Marker DOM pour rester synchronisé
    // avec le terrain et la trace pendant les transitions rapides de caméra.
    if (!map.getSource(MARKER_SOURCE_ID)) {
      map.addSource(MARKER_SOURCE_ID, {
        type: 'geojson',
        data: {
          type: 'Feature',
          geometry: { type: 'Point', coordinates: [0, 0] },
          properties: {},
        },
      })
    }
    if (!map.getLayer(MARKER_LAYER_ID)) {
      map.addLayer({
        id: MARKER_LAYER_ID,
        type: 'circle',
        source: MARKER_SOURCE_ID,
        paint: {
          'circle-radius': 9,
          'circle-color': '#FFD600',
          'circle-stroke-width': 2,
          'circle-stroke-color': '#FFFFFF',
          'circle-opacity': 1,
        },
      })
    }

    // 3. Charger la géométrie de la trace sélectionnée.
    await loadSelectedTrace()
  })
}

/**
 * Charge la géométrie de la trace sélectionnée depuis le store, l'affiche
 * dans la couche LineString, pose le marqueur jaune au départ et cadre la
 * carte sur l'emprise de la trace.
 *
 * Tente de charger les keyframes persistés sur disque ; si absents ou
 * invalides, les génère puis les sauvegarde pour les entrées futures
 * (spec §3.1 : déclenchement Phase 1).
 */
async function loadSelectedTrace() {
  const traceId = editionStore.selectedTraceId
  if (!map || !traceId) return

  let feature: GeoJSON.Feature
  try {
    feature = await tracesStore.getTraceGeometry(traceId)
  } catch (error) {
    console.error(`Géométrie introuvable pour la trace ${traceId} :`, error)
    return
  }
  if (!map) return // démontage pendant l'attente asynchrone

  const source = map.getSource(TRACE_SOURCE_ID) as mapboxgl.GeoJSONSource | undefined
  if (source) {
    source.setData({ type: 'FeatureCollection', features: [feature] })
  }

  // Cadrer la carte sur l'emprise de la trace (vue d'ensemble initiale).
  const bounds = computeBounds(feature)
  if (!bounds.isEmpty()) {
    map.fitBounds(bounds, { padding: 80, duration: 0 })
  }

  // Charger les points riches du backend (altitude + distance 3D).
  // Fait avant la génération des keyframes pour que l'altitude soit
  // disponible dans les TraceurPoint.
  let tracePoints: Awaited<ReturnType<typeof tracesStore.getTracePoints>> | null = null
  try {
    tracePoints = await tracesStore.getTracePoints(traceId)
  } catch (e) {
    console.warn(`[EditionMap] Chargement des points riches échoué :`, e)
  }

  // Charger les keyframes persistés ; sinon générer + sauvegarder.
  let kf = await keyframesStore.loadKeyframes(traceId)
  if (!kf) {
    const generated = generateKeyframes(traceId, feature, KEYFRAME_STEP_M, tracePoints)
    if (!generated) {
      console.error(`[EditionMap] Impossible de générer les keyframes pour ${traceId}`)
      return
    }
    kf = generated
    // Sauvegarder pour les entrées futures (best-effort, ne bloque pas).
    try {
      await keyframesStore.saveKeyframes(kf)
    } catch (e) {
      console.warn(`[EditionMap] Sauvegarde des keyframes échouée :`, e)
    }
  }

  editionStore.setKeyframeSet(kf, feature, tracePoints ?? null)

  // Curseur jaune (cercle GL). Position initiale au premier keyframe.
  if (kf && kf.keyframes.length > 0) {
    const start = kf.keyframes[0].traceur
    markerCoords = [start.lng, start.lat]
    updateMarkerSource()
    editionReady = true
    applyInterpolatedState()
  }
}

/**
 * Calcule les bounds (LngLatBounds) d'une Feature LineString en
 * parcourant ses coordonnées. Utilisé par map.fitBounds.
 */
function computeBounds(feature: GeoJSON.Feature): mapboxgl.LngLatBounds {
  const bounds = new mapboxgl.LngLatBounds()
  const geometry = feature.geometry
  if (geometry && geometry.type === 'LineString') {
    for (const coord of geometry.coordinates) {
      bounds.extend(coord as [number, number])
    }
  }
  return bounds
}

// --- Application de l'état interpolé (caméra + marker) ---

/**
 * Applique l'état caméra/curseur interpolé à l'instant courant du store.
 *
 * En lecture, appelée à chaque frame par la boucle rAF ; en pause, appelée
 * par le watcher sur `currentTimeMs`. On utilise `jumpTo` (instantané) pour
 * la caméra et mise à jour de la source GeoJSON pour le curseur.
 */
function applyInterpolatedState() {
  if (!map || !editionReady) return
  const cam = editionStore.interpolatedCam
  const tr = editionStore.interpolatedTraceur
  if (cam) {
    map.jumpTo({
      center: [cam.lng, cam.lat],
      zoom: cam.zoom,
      bearing: cam.bearing,
      pitch: cam.pitch,
    })
  }
  if (tr) {
    markerCoords = [tr.lng, tr.lat]
    updateMarkerSource()
  }
}

/** Met à jour la position du curseur via la source GeoJSON. */
function updateMarkerSource() {
  if (!map) return
  const source = map.getSource(MARKER_SOURCE_ID) as mapboxgl.GeoJSONSource | undefined
  if (source) {
    source.setData({
      type: 'Feature',
      geometry: { type: 'Point', coordinates: markerCoords },
      properties: {},
    })
  }
}

// --- Boucle d'animation (lecture) ---

/** Callback d'une frame : calcule le delta, déclenche `tick`, applique l'état. */
function onAnimationFrame(ts: number) {
  if (!editionStore.isPlaying) {
    stopAnimation()
    return
  }
  const delta = lastFrameTs === null ? 0 : ts - lastFrameTs
  lastFrameTs = ts

  editionStore.tick(delta)
  applyInterpolatedState()

  if (editionStore.isPlaying) {
    rafId = requestAnimationFrame(onAnimationFrame)
  } else {
    // La lecture s'est terminée pendant le tick (pause auto en fin de course).
    stopAnimation()
  }
}

/** Démarre la boucle d'animation. */
function startAnimation() {
  if (rafId !== null) return
  lastFrameTs = null
  rafId = requestAnimationFrame(onAnimationFrame)
}

/** Arrête la boucle d'animation. */
function stopAnimation() {
  if (rafId !== null) {
    cancelAnimationFrame(rafId)
    rafId = null
  }
  lastFrameTs = null
}

// Démarrage / arrêt de la boucle selon l'état de lecture du store.
watch(
  () => editionStore.isPlaying,
  (playing) => {
    if (playing) startAnimation()
    else stopAnimation()
  },
)

// En pause : repositionner la caméra et le marker quand le temps change
// (positionnement initial, seek futur). On évite la double-application
// pendant la lecture (la boucle rAF s'en charge déjà).
watch(
  () => editionStore.currentTimeMs,
  () => {
    if (!editionStore.isPlaying) applyInterpolatedState()
  },
)

// --- Cycle de vie ---

onMounted(async () => {
  // Token Mapbox depuis les paramètres (cf. Accueil/Map.vue).
  let token = ''
  try {
    token = await settingsStore.getSettingValue('Systeme.Key.mapBox')
  } catch (error) {
    console.error('Failed to retrieve MapBox token from settings:', error)
  }

  await initializeMap(token)

  // Surveiller le redimensionnement du conteneur pour resynchroniser le canvas.
  if (mapContainer.value) {
    resizeObserver = new ResizeObserver(() => {
      map?.resize()
    })
    resizeObserver.observe(mapContainer.value)
  }
})

onUnmounted(() => {
  stopAnimation()
  editionReady = false
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
  if (map) {
    map.remove()
    map = null
  }
})
</script>

<style>
/* Le conteneur de carte remplit son parent (v-main). */
.edition-map-container {
  width: 100%;
  height: 100%;
}
</style>
