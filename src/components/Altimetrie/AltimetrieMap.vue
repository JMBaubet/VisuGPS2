<template>
  <div class="alt-map">
    <div ref="mapContainer" class="alt-map__canvas" />

    <v-card
      v-if="candidates.length > 0 && candidatePoint"
      class="alt-map__candidates"
      :style="candidateStyle"
      @click.stop
    >
      <v-list density="compact" nav>
        <v-list-subheader>Choisir le passage</v-list-subheader>
        <v-list-item
          v-for="(candidate, index) in candidates"
          :key="`${candidate.km}-${index}`"
          @click="chooseCandidate(candidate)"
        >
          <v-list-item-title>{{ formatPickKm(candidate.km) }}</v-list-item-title>
        </v-list-item>
      </v-list>
    </v-card>

    <v-progress-circular
      v-if="mapLoading"
      class="alt-map__loading"
      :size="48"
      :width="6"
      color="primary"
      indeterminate
    />

    <v-empty-state
      v-if="!hasMapboxKey"
      class="alt-map__placeholder"
      icon="mdi-map-outline"
      title="Clé Mapbox requise"
      text="Renseignez la clé dans les paramètres (Systeme.Key.mapBox) pour afficher la carte."
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Carte 3D de la vue Altimétrie.
 *
 * Elle superpose :
 * - les segments de pente colorés, découpés sur la géométrie d'origine de la
 *   trace nettoyée (`trace.geojson`) ;
 * - les bornes vallée/sommet des montées ;
 * - la montée sélectionnée, en halo sous les segments.
 *
 * En mode Tunnels, les bornes de montée sont masquées et un layer de tunnels
 * est affiché : ligne blanche bordée de noir sur la trace, halo bleu pour le
 * groupe sélectionné. Un groupe fusionné est peint en une seule bande continue,
 * du premier au dernier portail.
 *
 * En mode création, un clic près de la trace est projeté sur le segment le plus
 * proche et alimente la création d'une montée ou d'un tunnel ; les passages
 * superposés sont proposés dans un petit menu avec leur kilomètre.
 *
 * Le terrain Mapbox est activé (relief, pitch 60) et les couches sont drapées
 * sur le relief. Le composant ne connaît ni le store ni la vue : il reçoit ses
 * données en props, émet les créations et expose `centerOnDistance` au widget
 * de profil.
 */
import { computed, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import mapboxgl from 'mapbox-gl'
import type { MapMouseEvent } from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import type { AltimetrieFile } from '../../stores/altimetrie'
import type { Tunnel } from '../../stores/tunnels'
import { useSettingsStore } from '../../stores/settings'
import { MIN_CLIMB_LENGTH_KM } from '../../utils/altimetrieClimbs'
import {
  pickTraceCandidates,
  type TracePickCandidate,
} from '../../utils/altimetrieMapPick'
import {
  MIN_TUNNEL_LENGTH_KM,
  tunnelRangeOverlaps,
} from '../../utils/altimetrieTunnels'
import { hexToRgbaString } from '../../utils/materialColors'
import {
  ALTIMETRIE_LAYER_IDS,
  ALTIMETRIE_MAP_STYLE,
  ALTIMETRIE_SOURCE_IDS,
  ALTIMETRIE_TERRAIN_SOURCE,
  addAltimetrieLayers,
  buildCreationPickFeature,
  buildMarkerFeatures,
  buildSegmentFeatures,
  buildSelectedClimbFeature,
  buildTunnelEndpointFeatures,
  buildTunnelFeatures,
  buildTracePath,
  coordinateAtDistance,
  emptyFC,
  geojsonSource,
  profilePointAt,
  slicePathCoordinates,
  sliceProfileCoordinates,
  type TracePath,
} from './altimetrieMapLayers'

const props = defineProps<{
  /** Onglet actif : montées ou tunnels. */
  mode: 'climbs' | 'tunnels'
  file: AltimetrieFile | null
  /** Géométrie complète de la trace (`trace.geojson`). */
  traceGeometry: GeoJSON.Feature | null
  selectedClimbId: number | null
  /** Tunnels physiques ; un groupe fusionné est peint comme une bande unique. */
  tunnels: Tunnel[]
  selectedTunnelIds: string[]
  /** Création en cours : montée, tunnel, ou aucune. */
  creationKind: 'climb' | 'tunnel' | null
}>()

const emit = defineEmits<{
  'create-climb': [payload: { startKm: number; endKm: number }]
  'create-tunnel': [payload: { startKm: number; endKm: number }]
  'create-error': [message: string]
  'create-tunnel-error': [message: string]
}>()

const settingsStore = useSettingsStore()

const mapContainer = ref<HTMLDivElement | null>(null)
const map = shallowRef<mapboxgl.Map | null>(null)
const mapReady = ref(false)
const hasMapboxKey = ref(false)

/** Rayon écran de sélection d'un passage autour du clic (pixels). */
const PICK_RADIUS_PX = 24

/** Premier point de la création en cours, en kilomètres. */
const creationKm = ref<number | null>(null)
/** Passages proposés quand plusieurs points se superposent. */
const candidates = ref<TracePickCandidate[]>([])
/** Position écran du menu de candidats. */
const candidatePoint = ref<{ x: number; y: number } | null>(null)

const candidateStyle = computed(() => {
  const point = candidatePoint.value
  const container = mapContainer.value
  if (!point || !container) return {}

  const width = 220
  const height = Math.min(48 + candidates.value.length * 36, 280)
  const left = Math.max(
    8,
    Math.min(point.x + 8, container.clientWidth - width - 8),
  )
  const top = Math.max(
    8,
    Math.min(point.y + 8, container.clientHeight - height - 8),
  )
  return { left: `${left}px`, top: `${top}px`, width: `${width}px` }
})

/** File d'attente des rendus demandés avant que la carte soit prête. */
const readyQueue: Array<() => void> = []

let resizeObserver: ResizeObserver | null = null
/** Premier cadrage : la carte s'ouvre sur la trace entière, une seule fois. */
let framed = false

const mapLoading = computed(() => hasMapboxKey.value && !mapReady.value)

/** Chemin d'origine de la trace, avec distances cumulées 2D. */
const tracePath = computed(() => buildTracePath(props.traceGeometry))

/** Dernier contenu poussé dans la source des segments. */
let lastSegmentsSource: AltimetrieFile['segments'] | null = null
let lastSegmentsPath: TracePath | null = null

/** Valeur d'un paramètre via son chemin. */
function getParam(path: string): unknown {
  return settingsStore.getParamDef(path)?.value
}

/** Couleur d'un paramètre convertie en rgba(), avec fallback #RRGGBBAA. */
function getColor(path: string, fallback: string): string {
  const value = getParam(path)
  return hexToRgbaString(typeof value === 'string' ? value : fallback)
}

/**
 * Applique les réglages `Altimetrie.Visualisation` aux couches Mapbox.
 *
 * Les largeurs sont utilisées telles que configurées ; les liserés blancs
 * suivent avec 2 px de plus. Le halo sélectionné reste sous les segments.
 */
function applyLayerStyle(): void {
  const instance = map.value
  if (!instance || !mapReady.value) return
  if (!instance.getLayer(ALTIMETRIE_LAYER_IDS.segments)) return

  const largeurBrute = Number(
    getParam('Altimetrie.Visualisation.largeurTrace') ?? 5,
  )
  const selectionBrute = Number(
    getParam('Altimetrie.Visualisation.largeurTraceSelectionnee') ?? 8,
  )
  const largeurTrace = Number.isFinite(largeurBrute) ? largeurBrute : 5
  const largeurSelection = Number.isFinite(selectionBrute) ? selectionBrute : 8
  const couleurSelection = getColor(
    'Altimetrie.Visualisation.couleurTraceSelectionnee',
    '#607D8BFF',
  )

  instance.setPaintProperty(
    ALTIMETRIE_LAYER_IDS.segments,
    'line-width',
    largeurTrace,
  )
  instance.setPaintProperty(
    ALTIMETRIE_LAYER_IDS.segmentsCasing,
    'line-width',
    largeurTrace + 2,
  )
  instance.setPaintProperty(
    ALTIMETRIE_LAYER_IDS.selected,
    'line-width',
    largeurSelection,
  )
  instance.setPaintProperty(
    ALTIMETRIE_LAYER_IDS.selected,
    'line-color',
    couleurSelection,
  )
  instance.setPaintProperty(
    ALTIMETRIE_LAYER_IDS.selectedCasing,
    'line-width',
    largeurSelection + 2,
  )
}

function whenMapReady(fn: () => void): void {
  if (mapReady.value && map.value) fn()
  else readyQueue.push(fn)
}

/**
 * Chemin utilisable pour le découpage.
 *
 * La géométrie d'origine et le profil partagent la même base de distances 2D.
 * Un écart trop important (points sans altitude ignorés par l'analyse) ferait
 * glisser les segments : on retombe alors sur le profil échantillonné.
 */
function usableTracePath(): TracePath | null {
  const file = props.file
  const path = tracePath.value
  if (!file || !path) return null

  const expected = file.trace.totalDistanceM
  if (expected > 0 && Math.abs(path.totalM - expected) / expected > 0.02) {
    return null
  }
  return path
}

/** Chemin utilisé pour la sélection, avec repli sur le profil échantillonné. */
function creationPath(): {
  coordinates: [number, number][]
  cumulative: number[]
} | null {
  const path = usableTracePath()
  if (path) {
    return { coordinates: path.coordinates, cumulative: path.cumulative }
  }

  const profile = props.file?.profile
  if (!profile || profile.length < 2) return null
  return {
    coordinates: profile.map(point => [point.lon, point.lat]),
    cumulative: profile.map(point => point.d),
  }
}

/** Kilomètre affiché dans le menu de choix d'un passage. */
function formatPickKm(km: number): string {
  return `${km.toFixed(3).replace('.', ',')} km`
}

/** Curseur de création sur la carte. */
function updateCreationCursor(): void {
  const canvas = map.value?.getCanvas()
  if (canvas) canvas.style.cursor = props.creationKind ? 'crosshair' : ''
}

/** Ferme le menu de candidats sans changer le premier point. */
function closeCandidates(): void {
  candidates.value = []
  candidatePoint.value = null
}

/** Signale une erreur au parent, avec le bon événement selon le mode. */
function emitCreationError(message: string): void {
  if (props.creationKind === 'tunnel') {
    emit('create-tunnel-error', message)
  } else if (props.creationKind === 'climb') {
    emit('create-error', message)
  }
}

/** Le point tombe-t-il déjà dans un tunnel ou une montée existante ? */
function isInsideExisting(km: number): boolean {
  if (props.creationKind === 'tunnel') {
    return props.tunnels.some(
      tunnel => km >= tunnel.startKm && km <= tunnel.endKm,
    )
  }
  if (props.creationKind === 'climb') {
    return (props.file?.climbs ?? []).some(
      climb => km >= climb.startKm && km <= climb.endKm,
    )
  }
  return false
}

/** La plage sélectionnée chevauche-t-elle un élément existant ? */
function hasCreationOverlap(startKm: number, endKm: number): boolean {
  if (props.creationKind === 'tunnel') {
    return tunnelRangeOverlaps(props.tunnels, startKm, endKm)
  }
  if (props.creationKind === 'climb') {
    return (props.file?.climbs ?? []).some(
      climb => startKm < climb.endKm && endKm > climb.startKm,
    )
  }
  return false
}

/** Enregistre un point de création, puis émet la création au second. */
function selectCreationPoint(candidate: TracePickCandidate): void {
  const kind = props.creationKind
  if (!kind) return

  if (creationKm.value === null) {
    if (isInsideExisting(candidate.km)) {
      emitCreationError(
        kind === 'climb'
          ? "Le point de vallée doit être en dehors d'une montée existante."
          : "Le point d'entrée doit être en dehors d'un tunnel existant.",
      )
      return
    }
    creationKm.value = candidate.km
    render(false)
    return
  }

  const startKm = Math.min(creationKm.value, candidate.km)
  const endKm = Math.max(creationKm.value, candidate.km)
  const minimum = kind === 'climb' ? MIN_CLIMB_LENGTH_KM : MIN_TUNNEL_LENGTH_KM

  if (endKm - startKm < minimum) {
    emitCreationError(
      kind === 'climb'
        ? 'La montée doit couvrir au moins 50 m.'
        : 'Le tunnel doit couvrir au moins 10 m.',
    )
    return
  }
  if (hasCreationOverlap(startKm, endKm)) {
    emitCreationError(
      kind === 'climb'
        ? 'La montée chevauche une montée existante.'
        : 'Le tunnel chevauche un tunnel existant.',
    )
    return
  }

  if (kind === 'climb') {
    emit('create-climb', { startKm, endKm })
  } else {
    emit('create-tunnel', { startKm, endKm })
  }
}

/** Retient le passage choisi dans le menu. */
function chooseCandidate(candidate: TracePickCandidate): void {
  closeCandidates()
  selectCreationPoint(candidate)
}

/** Clic sur la carte en mode création. */
function onMapClick(event: MapMouseEvent): void {
  if (!props.creationKind) return
  if (candidates.value.length > 0) {
    closeCandidates()
    return
  }

  const instance = map.value
  const path = creationPath()
  if (!instance || !path) return

  const screenCoordinates = path.coordinates.map(coordinate => {
    const point = instance.project(coordinate)
    return [point.x, point.y] as [number, number]
  })
  const picks = pickTraceCandidates(
    path.coordinates,
    screenCoordinates,
    path.cumulative,
    { x: event.point.x, y: event.point.y },
    PICK_RADIUS_PX,
  )

  if (picks.length === 0) {
    emitCreationError('Cliquez plus près de la trace.')
    return
  }
  if (picks.length === 1) {
    selectCreationPoint(picks[0])
    return
  }

  candidates.value = picks
  candidatePoint.value = { x: event.point.x, y: event.point.y }
}

/** Réinitialise la sélection de création en cours. */
function resetCreation(): void {
  creationKm.value = null
  candidates.value = []
  candidatePoint.value = null
  render(false)
}

/** Pousse les données courantes dans les sources de la carte. */
function render(fit = false): void {
  whenMapReady(() => {
    const instance = map.value
    if (!instance) return

    const file = props.file
    const path = usableTracePath()
    const segmentsSource = geojsonSource(
      instance,
      ALTIMETRIE_SOURCE_IDS.segments,
    )
    const markersSource = geojsonSource(instance, ALTIMETRIE_SOURCE_IDS.markers)
    const selectedSource = geojsonSource(instance, ALTIMETRIE_SOURCE_IDS.selected)
    const tunnelsSource = geojsonSource(instance, ALTIMETRIE_SOURCE_IDS.tunnels)
    const tunnelEndpointsSource = geojsonSource(
      instance,
      ALTIMETRIE_SOURCE_IDS.tunnelEndpoints,
    )
    const creationSource = geojsonSource(
      instance,
      ALTIMETRIE_SOURCE_IDS.creationPick,
    )

    // Les segments ne changent pas pendant un glissement : on ne reconstruit
    // la géométrie complète que lorsque le fichier ou le chemin change.
    if (
      file &&
      (file.segments !== lastSegmentsSource || path !== lastSegmentsPath)
    ) {
      segmentsSource?.setData(buildSegmentFeatures(file, path))
      lastSegmentsSource = file.segments
      lastSegmentsPath = path
    } else if (!file) {
      segmentsSource?.setData(emptyFC())
      lastSegmentsSource = null
      lastSegmentsPath = null
    }

    if (props.mode === 'tunnels') {
      tunnelsSource?.setData(
        file
          ? buildTunnelFeatures(
              file,
              props.tunnels,
              props.selectedTunnelIds,
              path,
            )
          : emptyFC(),
      )
      tunnelEndpointsSource?.setData(
        file
          ? buildTunnelEndpointFeatures(
              file,
              props.tunnels,
              props.selectedTunnelIds,
              path,
            )
          : emptyFC(),
      )
      markersSource?.setData(emptyFC())
      selectedSource?.setData(emptyFC())
    } else {
      tunnelsSource?.setData(emptyFC())
      tunnelEndpointsSource?.setData(emptyFC())
      if (file) {
        markersSource?.setData(
          buildMarkerFeatures(file, props.selectedClimbId, path),
        )
        selectedSource?.setData(
          buildSelectedClimbFeature(file, props.selectedClimbId, path),
        )
      } else {
        markersSource?.setData(emptyFC())
        selectedSource?.setData(emptyFC())
      }
    }

    creationSource?.setData(
      file && creationKm.value !== null
        ? buildCreationPickFeature(file, creationKm.value, path)
        : emptyFC(),
    )

    if (fit && !framed && fitTrace()) {
      framed = true
    }
  })
}

/** Coordonnées de la trace : géométrie complète, sinon profil échantillonné. */
function traceCoordinates(): [number, number][] {
  const path = tracePath.value
  if (path) return path.coordinates
  return (
    props.file?.profile.map(
      point => [point.lon, point.lat] as [number, number],
    ) ?? []
  )
}

/** Cadre la carte sur la trace entière. Retourne `true` si un cadrage a eu lieu. */
function fitTrace(): boolean {
  const instance = map.value
  if (!instance) return false

  const coordinates = traceCoordinates()
  const first = coordinates[0]
  if (!first || coordinates.length < 2) return false

  const bounds = new mapboxgl.LngLatBounds(first, first)
  for (const coordinate of coordinates) bounds.extend(coordinate)
  instance.fitBounds(bounds, {
    padding: { top: 40, bottom: 300, left: 40, right: 40 },
    duration: 0,
    maxZoom: 14,
  })
  return true
}

/** Cadre la carte sur la montée sélectionnée. */
function fitSelectedClimb(id: number): void {
  whenMapReady(() => {
    const instance = map.value
    if (!instance || !props.file || props.selectedClimbId !== id) return

    const climb = props.file.climbs.find(entry => entry.id === id)
    if (!climb) return

    const path = usableTracePath()
    const coordinates = path
      ? slicePathCoordinates(path, climb.startKm * 1000, climb.endKm * 1000)
      : sliceProfileCoordinates(
          props.file.profile,
          climb.startKm * 1000,
          climb.endKm * 1000,
        )
    const first = coordinates[0]
    if (!first || coordinates.length < 2) return

    const bounds = new mapboxgl.LngLatBounds(first, first)
    for (const coordinate of coordinates) bounds.extend(coordinate)
    instance.fitBounds(bounds, {
      padding: { top: 60, bottom: 320, left: 60, right: 60 },
      duration: 600,
      maxZoom: 15,
    })
  })
}

/** Cadre la carte sur le groupe de tunnels sélectionné. */
function fitSelectedTunnels(): void {
  whenMapReady(() => {
    const instance = map.value
    if (!instance || !props.file || props.selectedTunnelIds.length === 0) {
      return
    }

    const selected = props.tunnels.filter(tunnel =>
      props.selectedTunnelIds.includes(tunnel.id),
    )
    if (selected.length === 0) return

    const startM = Math.min(...selected.map(tunnel => tunnel.startKm)) * 1000
    const endM = Math.max(...selected.map(tunnel => tunnel.endKm)) * 1000
    const path = usableTracePath()
    const coordinates = path
      ? slicePathCoordinates(path, startM, endM)
      : sliceProfileCoordinates(props.file.profile, startM, endM)
    const first = coordinates[0]
    if (!first || coordinates.length < 2) return

    const bounds = new mapboxgl.LngLatBounds(first, first)
    for (const coordinate of coordinates) bounds.extend(coordinate)
    instance.fitBounds(bounds, {
      padding: { top: 60, bottom: 320, left: 60, right: 60 },
      duration: 600,
      maxZoom: 15,
    })
  })
}

/** Recentre la carte sur un point du profil (clic dans le widget). */
function centerOnDistance(distanceM: number): void {
  const instance = map.value
  if (!instance || !props.file) return

  const path = usableTracePath()
  if (path) {
    instance.easeTo({
      center: coordinateAtDistance(path, distanceM),
      duration: 500,
    })
    return
  }

  const point = profilePointAt(props.file.profile, distanceM)
  if (!point) return
  instance.easeTo({ center: [point.lon, point.lat], duration: 500 })
}

async function initializeMap(token: string): Promise<void> {
  if (!mapContainer.value) return

  mapboxgl.accessToken = token
  const firstProfilePoint = props.file?.profile[0]
  const center: [number, number] = firstProfilePoint
    ? [firstProfilePoint.lon, firstProfilePoint.lat]
    : [2.6, 46.6]

  const instance = new mapboxgl.Map({
    container: mapContainer.value,
    style: ALTIMETRIE_MAP_STYLE,
    center,
    zoom: 11,
    pitch: 60,
    bearing: 0,
    antialias: true,
  })
  map.value = instance

  instance.on('click', onMapClick)
  instance.on('movestart', closeCandidates)

  instance.addControl(
    new mapboxgl.NavigationControl({ showCompass: true, visualizePitch: true }),
    'top-right',
  )

  instance.on('error', event => {
    const message = event?.error?.message ?? ''
    if (/access token|401|403|token/i.test(message)) {
      console.warn('[Altimetrie] clé Mapbox refusée :', message)
    } else if (message) {
      console.warn('[Mapbox]', message)
    }
  })

  instance.on('load', () => {
    mapReady.value = true

    if (!instance.getSource(ALTIMETRIE_TERRAIN_SOURCE)) {
      instance.addSource(ALTIMETRIE_TERRAIN_SOURCE, {
        type: 'raster-dem',
        url: 'mapbox://mapbox.terrain-rgb',
        tileSize: 512,
        maxzoom: 14,
      })
    }
    instance.setTerrain({
      source: ALTIMETRIE_TERRAIN_SOURCE,
      exaggeration: 1.5,
    })

    addAltimetrieLayers(instance)
    applyLayerStyle()
    updateCreationCursor()

    const queue = readyQueue.splice(0)
    for (const fn of queue) {
      try {
        fn()
      } catch (error) {
        console.error(error)
      }
    }
    render(true)
  })
}

function destroyMap(): void {
  framed = false
  lastSegmentsSource = null
  lastSegmentsPath = null
  readyQueue.length = 0
  creationKm.value = null
  candidates.value = []
  candidatePoint.value = null
  if (map.value) {
    try {
      map.value.remove()
    } catch {
      /* destruction best-effort */
    }
  }
  map.value = null
  mapReady.value = false
}

onMounted(async () => {
  let token = ''
  try {
    token = String(
      (await settingsStore.getSettingValue('Systeme.Key.mapBox')) ?? '',
    )
  } catch {
    token = ''
  }
  hasMapboxKey.value = !!token
  if (!hasMapboxKey.value) return

  await initializeMap(token)

  if (mapContainer.value) {
    resizeObserver = new ResizeObserver(() => {
      map.value?.resize()
    })
    resizeObserver.observe(mapContainer.value)
  }
})

onUnmounted(() => {
  resizeObserver?.disconnect()
  resizeObserver = null
  destroyMap()
})

watch(
  () => props.file,
  () => render(false),
)

watch(
  () => props.traceGeometry,
  () => render(!framed),
)

watch(
  () => props.selectedClimbId,
  id => {
    render(false)
    if (props.mode === 'climbs' && id !== null) fitSelectedClimb(id)
  },
)

watch(
  () => props.creationKind,
  () => {
    resetCreation()
    updateCreationCursor()
  },
)

watch(
  () => props.mode,
  () => {
    render(false)
    if (props.mode === 'tunnels' && props.selectedTunnelIds.length > 0) {
      fitSelectedTunnels()
    }
  },
)

watch(
  () => props.tunnels,
  () => render(false),
)

watch(
  () => props.selectedTunnelIds,
  ids => {
    render(false)
    if (props.mode === 'tunnels' && ids.length > 0) fitSelectedTunnels()
  },
)

watch(
  () => settingsStore.settings,
  () => applyLayerStyle(),
  { deep: true },
)

defineExpose({
  centerOnDistance,
  fitTrace,
})
</script>

<style scoped>
.alt-map {
  position: relative;
  width: 100%;
  height: 100%;
}

.alt-map__canvas {
  position: absolute;
  inset: 0;
}

.alt-map__candidates {
  position: absolute;
  z-index: 5;
  max-height: 280px;
  overflow-y: auto;
}

.alt-map__loading {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
}

.alt-map__placeholder {
  position: absolute;
  inset: 0;
  background: rgb(var(--v-theme-surface));
}
</style>
