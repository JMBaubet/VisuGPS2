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
import { useEditionMap } from '../../composables/useEditionMap'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import { useSettingsStore } from '../../stores/settings'
import { useTracesStore } from '../../stores/traces'
import { useEditionStore } from '../../stores/edition'
import { useKeyframesStore } from '../../stores/keyframes'
import { generateKeyframes, KEYFRAME_STEP_M, type KeyframeSet } from '../../algorithms/keyframeGenerator'

// --- Stores ---

const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const editionStore = useEditionStore()
const keyframesStore = useKeyframesStore()

// Instance Mapbox partagée avec les widgets d'édition (CameraEditor).
const { map: mapRef } = useEditionMap()

// --- Références ---

const mapContainer = ref<HTMLDivElement | null>(null)
let map: mapboxgl.Map | null = null
/** Observer du redimensionnement du conteneur. */
let resizeObserver: ResizeObserver | null = null
/** Position courante du curseur [lng, lat], pour mise à jour source GeoJSON. */
let markerCoords: [number, number] = [0, 0]

// Géométrie et points riches de la trace chargée, mémorisés pour permettre
// la régénération des keyframes quand l'algorithme ou le gap min change.
let loadedFeature: GeoJSON.Feature | null = null
let loadedTracePoints: Awaited<ReturnType<typeof tracesStore.getTracePoints>> | null = null

// --- Échantillonnage du terrain (DEM Mapbox) pour l'occlusion par le relief ---

/**
 * Cache des altitudes terrain interrogées via `queryTerrainElevation` (avant
 * que la grille fine soit construite). Les lignes de visée de l'algorithme se
 * chevauchent fortement : le cache évite de re-interroger Mapbox pour des
 * points déjà demandés. Clé arrondie à ~11 m.
 */
const terrainElevCache = new Map<string, number | null>()

/**
 * Grille d'altitude terrain **fine** (~100 m) couvrant l'emprise de la trace.
 * La carte est à zoom ~10 (fitBounds) au moment de la génération :
 * `queryTerrainElevation` y renvoie un terrain ~60 m/px, trop grossier pour les
 * buttes côtières étroites qui masquent le curseur. On construit donc une
 * grille fine une seule fois en téléchargeant les tuiles `mapbox.terrain-rgb`
 * à zoom 13 (~7 m/px), puis on la restaure — elle persiste en mémoire et est
 * indépendante du zoom courant.
 */
const TERRAIN_GRID_STEP_DEG = 0.001 // ~100 m à ces latitudes
/** Zoom de téléchargement des tuiles terrain-rgb (≈7 m/px, nb de tuiles réduit). */
const TERRAIN_TILE_ZOOM = 13
const terrainGrid = new Map<string, number>()
let terrainGridReady = false

/** Clé de cellule de la grille fine pour un point (lng, lat). */
function gridKey(lng: number, lat: number): string {
  const step = TERRAIN_GRID_STEP_DEG
  return `${Math.round(lng / step)},${Math.round(lat / step)}`
}

/**
 * Renvoie l'altitude du terrain (m, exagération appliquée pour coller au
 * rendu) au point (lng, lat), ou `null` si indisponible. Fourni à
 * `generateKeyframes` comme `terrainSampler`. Utilise la grille fine dès
 * qu'elle est prête, sinon interroge Mapbox directement.
 */
function sampleTerrain(lng: number, lat: number): number | null {
  if (terrainGridReady) return terrainGrid.get(gridKey(lng, lat)) ?? null
  if (!map) return null
  const key = `${lng.toFixed(4)},${lat.toFixed(4)}`
  const cached = terrainElevCache.get(key)
  if (cached != null) return cached
  let value: number | null = null
  try {
    value = map.queryTerrainElevation([lng, lat], { exaggerated: true }) ?? null
  } catch {
    value = null
  }
  if (value != null) terrainElevCache.set(key, value)
  return value
}

/** Latitude (degrés) → radians. */
function toRad(deg: number): number {
  return (deg * Math.PI) / 180
}

/** Web Mercator : index X de la tuile contenant une longitude. */
function lngToTileX(lng: number, z: number): number {
  return Math.floor(((lng + 180) / 360) * Math.pow(2, z))
}

/** Web Mercator : index Y de la tuile contenant une latitude. */
function latToTileY(lat: number, z: number): number {
  const r = toRad(lat)
  return Math.floor(
    ((1 - Math.log(Math.tan(r) + 1 / Math.cos(r)) / Math.PI) / 2) * Math.pow(2, z),
  )
}

/**
 * Construit la grille fine du terrain sur l'emprise de la trace (pad ~2,5 km
 * pour couvrir la ligne de visée, la caméra étant jusqu'à ~2,4 km en arrière du
 * centre).
 *
 * On **télécharge les tuiles `mapbox.terrain-rgb` à zoom 13** (~7 m/px) et on
 * décode l'altitude RGB→m : cela contourne la limite de `queryTerrainElevation`,
 * qui ne lit que les tuiles chargées pour la caméra courante (zoom ~10 = ~60
 * m/px au moment de la génération, trop grossier pour les buttes côtières).
 * Le résultat est stocké dans `terrainGrid` (clé = cellule ~100 m), indépendant
 * du zoom de la carte. `onDone` est appelé une fois la grille complète.
 */
async function buildTerrainGrid(onDone: () => void): Promise<void> {
  if (!map || !loadedFeature) { onDone(); return }
  const coords = (loadedFeature.geometry as GeoJSON.LineString)?.coordinates
  if (!coords || coords.length === 0) { onDone(); return }

  let minLng = 180, maxLng = -180, minLat = 90, maxLat = -90
  for (const c of coords) {
    const [lng, lat] = c as [number, number]
    if (lng < minLng) minLng = lng
    if (lng > maxLng) maxLng = lng
    if (lat < minLat) minLat = lat
    if (lat > maxLat) maxLat = lat
  }
  const pad = 0.025 // ~2,5 km
  minLng -= pad; maxLng += pad; minLat -= pad; maxLat += pad

  const token = mapboxgl.accessToken
  if (!token) {
    console.warn('[EditionMap] pas de token Mapbox pour la grille terrain')
    onDone()
    return
  }

  const z = TERRAIN_TILE_ZOOM
  const x0 = lngToTileX(minLng, z)
  const x1 = lngToTileX(maxLng, z)
  const y0 = latToTileY(maxLat, z)
  const y1 = latToTileY(minLat, z)

  // 1. Télécharger et décoder toutes les tuiles DEM en parallèle.
  const tileImages = new Map<string, ImageData>()
  const requests: Promise<void>[] = []
  for (let x = x0; x <= x1; x++) {
    for (let y = y0; y <= y1; y++) {
      const key = `${z}/${x}/${y}`
      requests.push(
        (async () => {
          try {
            const resp = await fetch(
              `https://api.mapbox.com/v4/mapbox.terrain-rgb/${key}@2x.pngraw?access_token=${token}`,
            )
            if (!resp.ok) return
            const bmp = await createImageBitmap(await resp.blob())
            const canvas = document.createElement('canvas')
            canvas.width = bmp.width
            canvas.height = bmp.height
            const ctx = canvas.getContext('2d', { willReadFrequently: true })
            if (!ctx) { bmp.close(); return }
            ctx.drawImage(bmp, 0, 0)
            bmp.close()
            tileImages.set(key, ctx.getImageData(0, 0, canvas.width, canvas.height))
          } catch {
            // tuile indisponible → simplement ignorée
          }
        })(),
      )
    }
  }
  await Promise.all(requests)
  if (tileImages.size === 0) {
    console.warn('[EditionMap] aucune tuile terrain téléchargée')
    onDone()
    return
  }

  // 2. Échantillonner la grille (~100 m) depuis les tuiles décodées.
  terrainGrid.clear()
  const n = Math.pow(2, z)
  for (let lat = minLat; lat <= maxLat; lat += TERRAIN_GRID_STEP_DEG) {
    for (let lng = minLng; lng <= maxLng; lng += TERRAIN_GRID_STEP_DEG) {
      const x = lngToTileX(lng, z)
      const y = latToTileY(lat, z)
      const img = tileImages.get(`${z}/${x}/${y}`)
      if (!img) continue
      const worldX = ((lng + 180) / 360) * n - x
      const r = toRad(lat)
      const worldY =
        ((1 - Math.log(Math.tan(r) + 1 / Math.cos(r)) / Math.PI) / 2) * n - y
      const px = Math.min(img.width - 1, Math.floor(worldX * img.width))
      const py = Math.min(img.height - 1, Math.floor(worldY * img.height))
      const i = (py * img.width + px) * 4
      const red = img.data[i]
      const green = img.data[i + 1]
      const blue = img.data[i + 2]
      const elev = -10000 + ((red * 256 * 256 + green * 256 + blue) * 0.1)
      if (elev > -5000) terrainGrid.set(gridKey(lng, lat), elev * 1.5)
    }
  }
  if (terrainGrid.size > 0) {
    terrainGridReady = true
  } else {
    console.warn('[EditionMap] grille terrain vide — occlusion par relief désactivée')
  }
  onDone()
}

/** Mémorise si la dernière génération a bénéficié du DEM (pour ne pas
 * régénérer inutilement, ni ignorer un jeu persisté d'une version antérieure). */
let lastGenerationHadTerrain = false

/**
 * Régénère les keyframes dès que la grille fine du terrain est disponible, si ce
 * n'était pas le cas lors de la dernière génération.
 *
 * La grille est construite une seule fois (téléchargement des tuiles
 * `terrain-rgb`, indépendant du zoom de la carte) ; les régénérations suivantes
 * (changement d'algorithme / gap) l'utilisent en mémoire.
 */
function scheduleTerrainRegeneration(): void {
  if (!map || !loadedFeature || editionStore.keyframeAlgorithm !== 'frustum') return
  if (lastGenerationHadTerrain) return // déjà généré avec la grille
  if (terrainGridReady) {
    void generateAndSetKeyframes()
    return
  }
  void buildTerrainGrid(() => {
    if (editionStore.keyframeAlgorithm === 'frustum') {
      void generateAndSetKeyframes()
    }
  })
}

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
    attributionControl: false, // ajouté manuellement plus bas (position top-left)
  })
  // Copyright Mapbox repositionné en haut à gauche (au lieu du bas droit).
  map.addControl(new mapboxgl.AttributionControl({ compact: true }), 'top-left')
  // Les flèches clavier sont réservées à la navigation entre RdV (vue) :
  // on désactive le pan clavier natif de Mapbox pour éviter le conflit.
  map.keyboard.disable()
  // Partager l'instance pour les widgets d'édition (CameraEditor).
  mapRef.value = map

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

  // Réinitialiser l'état terrain : la grille fine est propre à chaque trace.
  terrainGrid.clear()
  terrainGridReady = false
  lastGenerationHadTerrain = false

  let feature: GeoJSON.Feature
  try {
    feature = await tracesStore.getTraceGeometry(traceId)
  } catch (error) {
    console.error(`Géométrie introuvable pour la trace ${traceId} :`, error)
    return
  }
  if (!map) return // démontage pendant l'attente asynchrone
  loadedFeature = feature

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
  loadedTracePoints = null
  try {
    loadedTracePoints = await tracesStore.getTracePoints(traceId)
  } catch (e) {
    console.warn(`[EditionMap] Chargement des points riches échoué :`, e)
  }

  // Charger les keyframes persistés ; sinon générer + sauvegarder.
  let kf = await keyframesStore.loadKeyframes(traceId)
  if (!kf) {
    kf = await generateAndSetKeyframes()
    if (!kf) return
  } else {
    editionStore.setKeyframeSet(kf, feature, loadedTracePoints ?? null)
    // Jeu persisté (éventuellement d'une version antérieure sans occlusion
    // par le relief) : on force une régénération dès que la grille est prête.
    lastGenerationHadTerrain = terrainGridReady
  }

  // Curseur jaune (cercle GL). Position initiale au premier keyframe.
  if (kf && kf.keyframes.length > 0) {
    const start = kf.keyframes[0].traceur
    markerCoords = [start.lng, start.lat]
    updateMarkerSource()
    editionReady = true
    applyInterpolatedState()
  }

  // L'occlusion par le relief (frustum) dépend du DEM Mapbox : régénérer si
  // la dernière génération n'a pas encore bénéficié du terrain.
  scheduleTerrainRegeneration()
}

/**
 * Génère les keyframes selon l'algorithme sélectionné (frustum ou simple),
 * les sauvegarde sur disque (best-effort) et les pousse dans le store.
 *
 * Réutilise la géométrie et les points riches déjà chargés par
 * `loadSelectedTrace` (`loadedFeature` / `loadedTracePoints`).
 *
 * @returns Le jeu généré, ou `null` en cas d'échec.
 */
async function generateAndSetKeyframes(): Promise<KeyframeSet | null> {
  const traceId = editionStore.selectedTraceId
  if (!map || !traceId || !loadedFeature) return null

  // Ne fournir le sampler terrain que si la grille fine est prête : sinon
  // l'algorithme retombe sur les points de trace, et `scheduleTerrainRegeneration`
  // régénèrera avec la grille dès qu'elle sera construite.
  const sampler = terrainGridReady ? sampleTerrain : null

  const generated = generateKeyframes(
    traceId,
    loadedFeature,
    KEYFRAME_STEP_M,
    loadedTracePoints ?? null,
    editionStore.keyframeAlgorithm,
    editionStore.minKeyframeGapM,
    sampler,
  )
  if (!generated) {
    console.error(`[EditionMap] Impossible de générer les keyframes pour ${traceId}`)
    return null
  }

  // Sauvegarder pour les entrées futures (best-effort, ne bloque pas).
  try {
    await keyframesStore.saveKeyframes(generated)
  } catch (e) {
    console.warn(`[EditionMap] Sauvegarde des keyframes échouée :`, e)
  }

  editionStore.setKeyframeSet(generated, loadedFeature, loadedTracePoints ?? null)
  lastGenerationHadTerrain = terrainGridReady
  return generated
}

/**
 * Régénère les keyframes quand l'algorithme ou la distance minimale change
 * (sélecteur « Algorithme » / « Gap min » de la toolbar).
 */
watch(
  [() => editionStore.keyframeAlgorithm, () => editionStore.minKeyframeGapM],
  () => {
    if (!editionReady || !loadedFeature) return
    void generateAndSetKeyframes()
  },
)

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
    mapRef.value = null
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
