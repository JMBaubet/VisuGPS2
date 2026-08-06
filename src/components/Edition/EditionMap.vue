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
 *   - marqueur jaune bordé de blanc
 *
 * Après chargement de la trace, ce composant génère un jeu de keyframes
 * (`generateKeyframes`) et le pousse dans `editionStore`. Il assure ensuite
 * le rendu frame-by-frame piloté par l'état de lecture du store :
 *   - en lecture : une boucle `requestAnimationFrame` déclenche `tick()`
 *     puis applique l'état interpolé (caméra via `jumpTo`, marker via
 *     `setLngLat`) ;
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
import { generateKeyframes } from '../../algorithms/keyframeGenerator'

// --- Stores ---

const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const editionStore = useEditionStore()

// --- Références ---

const mapContainer = ref<HTMLDivElement | null>(null)
let map: mapboxgl.Map | null = null
/** Observer du redimensionnement du conteneur. */
let resizeObserver: ResizeObserver | null = null
/** Marqueur jaune : créé une fois la trace chargée, détruit au démontage. */
let marker: mapboxgl.Marker | null = null

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

    // 3. Charger la géométrie de la trace sélectionnée.
    await loadSelectedTrace()
  })
}

/**
 * Charge la géométrie de la trace sélectionnée depuis le store, l'affiche
 * dans la couche LineString, pose le marqueur jaune au départ et cadre la
 * carte sur l'emprise de la trace.
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

  // Générer le jeu de keyframes et le pousser dans le store. Le watcher
  // sur `currentTimeMs` ci-dessous positionnera caméra + marker sur le
  // keyframe initial (currentTimeMs = 0 après setKeyframeSet).
  const kf = generateKeyframes(traceId, feature)
  editionStore.setKeyframeSet(kf, feature)

  // Marqueur jaune (cercle bordé de blanc, spec §1). On ne l'ajoute à la
  // carte qu'une fois sa position initiale connue (le premier keyframe),
  // sinon Mapbox tente de projeter un marqueur sans LngLat à chaque frame
  // et lève une erreur `LngLatLike`.
  if (kf && kf.keyframes.length > 0) {
    const start = kf.keyframes[0].traceur
    const el = document.createElement('div')
    el.className = 'edition-marker'
    marker = new mapboxgl.Marker({ element: el, anchor: 'center' })
      .setLngLat([start.lng, start.lat])
      .addTo(map)
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
 * Applique l'état caméra/marker interpolé à l'instant courant du store.
 *
 * En lecture, appelée à chaque frame par la boucle rAF ; en pause, appelée
 * par le watcher sur `currentTimeMs`. On utilise `jumpTo` (instantané) pour
 * un rendu frame-by-frame cohérent, avec `essential: true` afin que le
 * mouvement ne soit pas désactivé par `prefers-reduced-motion`.
 */
function applyInterpolatedState() {
  if (!map || !editionReady) return
  const cam = editionStore.interpolatedCam
  const tr = editionStore.interpolatedTraceur
  if (cam) {
    // jumpTo est instantané (sans animation), donc insensible à
    // prefers-reduced-motion : pas besoin du flag `essential`.
    map.jumpTo({
      center: [cam.lng, cam.lat],
      zoom: cam.zoom,
      bearing: cam.bearing,
      pitch: cam.pitch,
    })
  }
  if (tr && marker) {
    marker.setLngLat([tr.lng, tr.lat])
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
  if (marker) {
    marker.remove()
    marker = null
  }
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

/*
 * Marqueur jaune bordé de blanc (spec §1 : cercle jaune bordé de blanc).
 * L'élément est créé en JS et reçoit cette classe ; il n'est pas scoped
 * car il est attaché hors du DOM du composant (enfant du container Mapbox).
 */
.edition-marker {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background-color: #FFD600;
  border: 2px solid #FFFFFF;
  box-shadow: 0 0 4px rgba(0, 0, 0, 0.6);
}
</style>
