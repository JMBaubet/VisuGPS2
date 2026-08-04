<template>
  <div class="map3d-wrapper">
    <!-- Conteneur Mapbox : le terrain 3D y est rendu. Sa taille est forcée
         à la résolution canonique pendant le pré-calcul (via precompute),
         puis restaurée à la taille réelle pour le live preview. -->
    <div ref="mapContainer" class="map-container"></div>
  </div>
</template>

<script setup lang="ts">
/**
 * Carte Mapbox 3D dédiée à l'atelier d'édition (Mode 2) et au pré-calcul (Mode 1).
 *
 * Différences avec `Accueil/Map.vue` (carte 2D d'accueil) :
 *  - Active le terrain Mapbox (`mapbox-dem`, `exaggeration: 1`) pour pouvoir
 *    interroger `queryTerrainElevation` (source d'altitude primaire du doc).
 *  - N'affiche aucune couche de cluster / favoris (pas son rôle).
 *  - Expose une méthode `getMap()` pour que le parent déclenche le pré-calcul
 *    via le composable `useKeyframeEngine`.
 *
 * Le live preview (déplacement caméra selon le curseur de timeline) sera
 * branché en M3/M5 : il consommera la même instance via `getMap()`.
 */
import { ref, shallowRef, onMounted, onUnmounted } from 'vue'
import mapboxgl from 'mapbox-gl'
import { useSettingsStore } from '../../stores/settings'

const settingsStore = useSettingsStore()

// Émet 'map-ready' dès que l'instance Mapbox est créée (pour que la vue parent
// la branche au live preview), et 'terrain-ready' quand le DEM est chargé.
const emit = defineEmits<{
  (e: 'map-ready'): void
  (e: 'terrain-ready'): void
}>()

/** Couche/source de la trace parcourue ( LineString dynamique, M5). */
const TRAIL_SOURCE = 'edition-trail'
const TRAIL_LAYER = 'edition-trail-line'

/** Couche/source de la trace GPX complète (LineString statique de la trace). */
const TRACE_SOURCE = 'edition-trace'
const TRACE_LAYER = 'edition-trace-line'

/** Conteneur DOM de la carte (ref template). */
const mapContainer = ref<HTMLDivElement | null>(null)

/**
 * Instance Mapbox. `shallowRef` : on ne veut pas que Vue rende l'objet map
 * réactif (problème de perf + Mapbox gère son propre état interne). Pattern
 * identique à `Accueil/Map.vue`.
 */
const map = shallowRef<mapboxgl.Map | null>(null)

/** true une fois que le style + le DEM sont chargés (prêt à précalculer). */
let terrainReady = false
/** Résout à `true` dès que `terrainReady` passe à vrai (attendu par le parent). */
let resolveTerrainReady: () => void = () => {}
const terrainReadyPromise = new Promise<boolean>((resolve) => {
  resolveTerrainReady = () => resolve(true)
})

/** ResizeObserver pour resynchroniser le canvas au redimensionnement. */
let resizeObserver: ResizeObserver | null = null

// --- Marqueur traceur + trace parcourue (M5) ---

/** Marqueur Mapbox représentant le traceur (point rouge). */
let traceurMarker: mapboxgl.Marker | null = null
/** true si la source/couche de trace parcourue a été ajoutée. */
let trailReady = false
/** Altitude de décalage du marqueur (paramètre `markerAltitudeOffset`). */
let markerAltitudeOffset = 2

/** Crée le marqueur traceur (élément DOM circulaire rouge). */
function createTraceurMarker(map: mapboxgl.Map, color: string, size: number) {
  const el = document.createElement('div')
  el.style.width = `${size}px`
  el.style.height = `${size}px`
  el.style.borderRadius = '50%'
  el.style.backgroundColor = color
  el.style.border = '2px solid white'
  el.style.boxShadow = '0 0 6px rgba(0,0,0,0.6)'
  traceurMarker = new mapboxgl.Marker({ element: el })
    .setLngLat(map.getCenter())
    .addTo(map)
}

/** Ajoute la source + couche de la trace parcourue (LineString dynamique). */
function addTrailLayers(map: mapboxgl.Map, color: string) {
  if (trailReady) return
  map.addSource(TRAIL_SOURCE, {
    type: 'geojson',
    data: { type: 'Feature', geometry: { type: 'LineString', coordinates: [] }, properties: {} },
  })
  map.addLayer({
    id: TRAIL_LAYER,
    type: 'line',
    source: TRAIL_SOURCE,
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': color,
      'line-width': 4,
    },
  })
  trailReady = true
}

/**
 * Affiche la LineString complète de la trace GPX et cadre la carte dessus.
 * À appeler par la vue parent dès que la géométrie est disponible (au chargement
 * depuis le cache comme après un pré-calcul), afin que la trace soit visible
 * en fond, sous le traceur et la trace parcourue dynamique.
 *
 * @param geometry - Feature GeoJSON LineString de la trace (coords [lon, lat][]).
 * @param color    - couleur de tracé (paramètre `EditionCamera.Traceur.markerColor`).
 */
function displayTrace(geometry: GeoJSON.Feature, color: string) {
  const mapInstance = map.value
  if (!mapInstance) {
    console.warn('[displayTrace] Instance Mapbox indisponible.')
    return
  }

  // Vérifier que le style est chargé : sinon différer à l'événement 'idle'.
  if (!mapInstance.isStyleLoaded()) {
    console.warn('[displayTrace] Style non chargé, nouvel essai au prochain idle.')
    mapInstance.once('idle', () => displayTrace(geometry, color))
    return
  }

  try {
    // Couleur de fond sans canal alpha (compatibilité maximale mapbox-gl).
    const lineColor = (color || '#FF0000').slice(0, 7)

    // Ajouter la source/couche une seule fois (idempotent).
    if (!mapInstance.getSource(TRACE_SOURCE)) {
      mapInstance.addSource(TRACE_SOURCE, {
        type: 'geojson',
        data: geometry as GeoJSON.Feature,
      })
      mapInstance.addLayer({
        id: TRACE_LAYER,
        type: 'line',
        source: TRACE_SOURCE,
        layout: { 'line-cap': 'round', 'line-join': 'round' },
        paint: {
          'line-color': lineColor,
          'line-width': 6,
          'line-opacity': 1.0,
        },
      })
      console.info('[displayTrace] Couche de trace ajoutée.')
    }

    // Cadrer la carte sur l'emprise de la trace (fitBounds calcule centre + zoom).
    const coords = (geometry.geometry as GeoJSON.LineString).coordinates as [number, number][]
    if (coords.length === 0) {
      console.warn('[displayTrace] Aucune coordonnée dans la géométrie.')
      return
    }
    const bounds = new mapboxgl.LngLatBounds()
    for (const c of coords) bounds.extend(c as [number, number])
    mapInstance.fitBounds(bounds, { padding: 60, duration: 0 })
    console.info(`[displayTrace] ${coords.length} points, trace cadrée.`)
  } catch (e) {
    console.error('[displayTrace] Erreur lors de l\'ajout de la couche de trace :', e)
  }
}

/**
 * Met à jour la position du marqueur traceur (callback du composable usePlayback).
 * L'altitude est décalée de `markerAltitudeOffset` pour faire flotter le
 * marqueur au-dessus du relief (lisibilité sur terrain accidenté, §9.4).
 */
function moveTraceurMarker(lng: number, lat: number, altitude: number | null) {
  if (!traceurMarker) return
  traceurMarker.setLngLat([lng, lat])
  // markerAltitudeOffset est appliqué via setOffset ou l'altitude si supporté.
  // Mapbox Marker ne gère pas nativement l'altitude 3D sur un DOM element ;
  // le décalage est donc visuel (le terrain fait le reste). Gardé pour usage futur.
  void altitude
  void markerAltitudeOffset
}

/** Met à jour la LineString de trace parcourue (callback usePlayback). */
function updateTrail(coords: [number, number][]) {
  const mapInstance = map.value
  if (!mapInstance || !trailReady) return
  const source = mapInstance.getSource(TRAIL_SOURCE) as mapboxgl.GeoJSONSource | undefined
  if (!source) return
  source.setData({
    type: 'Feature',
    geometry: { type: 'LineString', coordinates: coords },
    properties: {},
  })
}

/**
 * Initialise les couches de lecture (marqueur + trace) une fois les keyframes
 * disponibles. À appeler par la vue parent après le pré-calcul. Lit les
 * paramètres `EditionCamera.Traceur.*` depuis le backend.
 */
async function setupPlaybackLayers() {
  const mapInstance = map.value
  if (!mapInstance) return

  const color = await settingsStore
    .getSettingValue('EditionCamera.Traceur.markerColor')
    .catch(() => '#FF0000FF') as string
  const size = await settingsStore
    .getSettingValue('EditionCamera.Traceur.markerSize')
    .catch(() => 12) as number
  markerAltitudeOffset = await settingsStore
    .getSettingValue('EditionCamera.Traceur.markerAltitudeOffset')
    .catch(() => 2) as number

  createTraceurMarker(mapInstance, color, size)
  addTrailLayers(mapInstance, color)
}

/** Crée et initialise la carte Mapbox avec le terrain 3D. */
function initMap(token: string) {
  if (!mapContainer.value) return

  mapboxgl.accessToken = token

  const instance = new mapboxgl.Map({
    container: mapContainer.value,
    style: 'mapbox://styles/mapbox/standard-satellite',
    zoom:16,
    center: [2.0, 43.7], // sera recentré sur la trace par le pré-calcul
    pitch: 60,
    bearing: 0,
    // Le terrain sera activé après le chargement du style (ci-dessous).
  })

  instance.on('style.load', () => {
    // Ajouter la source DEM et activer le terrain. On utilise le standard
    // DEM Mapbox (précision verticale ~0,1 m, cf. note §0 du document).
    instance.addSource('mapbox-dem', {
      type: 'raster-dem',
      url: 'mapbox://mapbox.terrain-rgb',
      tileSize: 514,
      maxzoom: 16,
    })
    instance.setTerrain({ source: 'mapbox-dem', exaggeration: 1 })

    // Le terrain est considéré prêt une fois qu'au moins une tuile DEM est
    // chargée : on écoute 'sourcedata' puis 'idle' pour s'assurer que
    // queryTerrainElevation renverra des valeurs fiables.
    const onSource = (e: mapboxgl.MapSourceDataEvent) => {
      if (e.sourceId === 'mapbox-dem' && e.isSourceLoaded && !terrainReady) {
        instance.once('idle', () => {
          terrainReady = true
          resolveTerrainReady()
          emit('terrain-ready')
        })
        instance.off('sourcedata', onSource)
      }
    }
    instance.on('sourcedata', onSource)
  })

  map.value = instance
  emit('map-ready')

  if (mapContainer.value) {
    resizeObserver = new ResizeObserver(() => instance.resize())
    resizeObserver.observe(mapContainer.value)
  }
}

/**
 * Retourne l'instance Mapbox (ou null). Le parent (vue ÉditionCamera) l'utilise
 * pour appeler `precomputeKeyframes(map, ...)` puis le live preview.
 */
function getMap(): mapboxgl.Map | null {
  return map.value
}

/**
 * Promesse résolue quand le terrain DEM est chargé et que
 * `queryTerrainElevation` renvoie des valeurs fiables. Le parent l'attend
 * avant de lancer le pré-calcul.
 */
function waitForTerrain(): Promise<boolean> {
  return terrainReadyPromise
}

defineExpose({
  getMap,
  waitForTerrain,
  setupPlaybackLayers,
  moveTraceurMarker,
  updateTrail,
  displayTrace,
})

onMounted(async () => {
  // Token Mapbox : même pattern que Accueil/Map.vue (paramètre `secret`).
  let token = ''
  try {
    token = await settingsStore.getSettingValue('Systeme.Key.mapBox')
  } catch (e) {
    console.error("Récupération du token Mapbox impossible :", e)
  }
  initMap(token || '')
})

onUnmounted(() => {
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
  if (traceurMarker) {
    traceurMarker.remove()
    traceurMarker = null
  }
  if (map.value) {
    map.value.remove()
    map.value = null
  }
  trailReady = false
})
</script>

<style scoped>
.map3d-wrapper {
  position: relative;
  width: 100%;
  height: 100%;
}

.map-container {
  width: 100%;
  height: 100%;
}
</style>
