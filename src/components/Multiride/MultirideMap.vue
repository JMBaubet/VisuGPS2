<script setup lang="ts">
/**
 * Quatrième instance Mapbox GL de l'application, dédiée à la restitution des
 * passages multiples.
 *
 * Elle porte le contrat de rendu de la spécification : la trace de fond, puis
 * les emprunts détectés — la **référence** en bleu épais et **sous** les autres,
 * les allers en orange et les retours en rouge — et les bornes de la trace. Un
 * segment écarté par l'utilisateur est rendu à faible opacité.
 *
 * Le fichier de description ne stocke que les **bornes** des emprunts : la
 * portion complète est reconstituée ici en joignant `pointEntree` /
 * `pointSortie` avec les points de la trace (`multirideMapFeatures`).
 *
 * Le composant ne connaît ni le store ni la vue : il reçoit ses données en props
 * et remonte les intentions. Les couches sont déclarées une fois au `load`, puis
 * seules les données sont mises à jour.
 */
import { ref, shallowRef, computed, onMounted, onUnmounted, watch } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import type { MultiridePassage } from '../../stores/multiride'
import type { TracePoint } from '../../stores/traces'
import { useSettingsStore } from '../../stores/settings'
import {
  MULTIRIDE_MAP_CENTER,
  MULTIRIDE_MAP_STYLE,
  MULTIRIDE_MAP_ZOOM,
  SOURCE_IDS,
  initMultirideLayers,
  setPassageData,
  setTraceData,
} from './multirideMapLayers'
import { buildMultirideRender, segmentBounds } from './multirideMapFeatures'

const props = defineProps<{
  /** Emprunts détectés, tous segments confondus. */
  passages: MultiridePassage[]
  /** Points de la trace (altitude et distance cumulée), pour découper les emprunts. */
  tracePoints: TracePoint[]
  /** Segment mis en avant (`null` : aucun). */
  selectedSegment: number | null
}>()

const emit = defineEmits<{
  'select-segment': [segment: number]
}>()

const settingsStore = useSettingsStore()

const mapContainer = ref<HTMLDivElement | null>(null)
const map = shallowRef<mapboxgl.Map | null>(null)
const mapReady = ref(false)
const hasMapboxKey = ref(false)

/** File d'attente des rendus demandés avant que la carte soit prête. */
const readyQueue: Array<() => void> = []

let currentPopup: mapboxgl.Popup | null = null
let resizeObserver: ResizeObserver | null = null
/** Emprunt actuellement survolé, pour ne remettre à zéro que celui-là. */
let hoveredId: string | null = null
/** Premier cadrage : la vue s'ouvre sur la trace entière, une seule fois. */
let framed = false

/** Exécute `fn` dès que la carte est prête, sinon met en file d'attente. */
function whenMapReady(fn: () => void): void {
  if (mapReady.value && map.value) fn()
  else readyQueue.push(fn)
}

const placeholderVisible = computed(() => !hasMapboxKey.value)

// ─── Popups ───────────────────────────────────────────────────────────

/** Ouvre le popup unique sur le HTML porté par la feature cliquée. */
function showPopupAt(e: mapboxgl.MapLayerMouseEvent): void {
  e.originalEvent.stopPropagation()
  const html = e.features?.[0]?.properties?.popupHtml as string | undefined
  if (currentPopup) {
    currentPopup.remove()
    currentPopup = null
  }
  if (!html || !map.value) return
  currentPopup = new mapboxgl.Popup({ offset: 10, maxWidth: '320px' })
    .setLngLat(e.lngLat)
    .setHTML(html)
    .addTo(map.value)
}

// ─── Interactions ─────────────────────────────────────────────────────

/** Clic sur un emprunt : popup, et sélection de son segment dans la liste. */
function onPassageClick(e: mapboxgl.MapLayerMouseEvent): void {
  showPopupAt(e)
  const segment = e.features?.[0]?.properties?.segment
  if (typeof segment === 'number') emit('select-segment', segment)
}

/** Survol d'un emprunt : épaississement, par l'état de feature. */
function onPassageEnter(e: mapboxgl.MapLayerMouseEvent): void {
  if (!map.value) return
  const feature = e.features?.[0]
  if (feature === undefined || feature.id === undefined) return
  if (hoveredId !== null && hoveredId !== feature.id) clearHover()
  hoveredId = String(feature.id)
  map.value.setFeatureState({ source: SOURCE_IDS.passages, id: feature.id }, { hover: true })
  if (mapContainer.value) mapContainer.value.style.cursor = 'pointer'
}

/** Sortie du pointeur : retour au trait nominal. */
function onPassageLeave(): void {
  clearHover()
  if (mapContainer.value) mapContainer.value.style.cursor = ''
}

/** Retire l'état de survol de l'emprunt pointé. */
function clearHover(): void {
  if (!map.value || hoveredId === null) return
  map.value.setFeatureState({ source: SOURCE_IDS.passages, id: hoveredId }, { hover: false })
  hoveredId = null
}

// ─── Rendu ────────────────────────────────────────────────────────────

/** Pousse la trace, ses bornes et les emprunts dans les sources de la carte. */
function render(fit = false): void {
  whenMapReady(() => {
    const instance = map.value
    if (!instance) return

    const built = buildMultirideRender(props.passages, props.tracePoints)
    setTraceData(instance, built.trace, built.ends)
    setPassageData(instance, built.passages)

    if (fit && !framed && built.bounds) {
      frameOn(built.bounds)
      framed = true
    }
  })
}

/** Cadre la carte sur une emprise. */
function frameOn(coords: [number, number][]): void {
  const instance = map.value
  if (!instance || coords.length < 2) return
  const bounds = new mapboxgl.LngLatBounds(coords[0], coords[0])
  for (const c of coords) bounds.extend(c)
  instance.fitBounds(bounds, { padding: 48, duration: 600, maxZoom: 16 })
}

/** Cadre la carte sur l'étendue d'un segment (clic dans la liste ou la carte). */
function frameOnSegment(segment: number): void {
  whenMapReady(() => {
    const coords = segmentBounds(props.passages, props.tracePoints, segment)
    if (coords) frameOn(coords)
  })
}

// ─── Cycle de vie de la carte ─────────────────────────────────────────

async function initializeMap(token: string): Promise<void> {
  if (!mapContainer.value) return

  mapboxgl.accessToken = token
  const instance = new mapboxgl.Map({
    container: mapContainer.value,
    style: MULTIRIDE_MAP_STYLE,
    center: MULTIRIDE_MAP_CENTER,
    zoom: MULTIRIDE_MAP_ZOOM,
  })
  map.value = instance

  instance.addControl(
    new mapboxgl.NavigationControl({ showCompass: false }),
    'bottom-right',
  )

  instance.on('error', (e) => {
    const message = e?.error?.message ?? ''
    if (/access token|401|403|token/i.test(message)) {
      console.warn('[Multiride] clé Mapbox refusée :', message)
    } else if (message) {
      console.warn('[Mapbox]', message)
    }
  })

  instance.on('load', () => {
    mapReady.value = true
    initMultirideLayers(instance, {
      onClick: onPassageClick,
      onEnter: onPassageEnter,
      onLeave: onPassageLeave,
    })

    const queue = readyQueue.splice(0)
    for (const fn of queue) {
      try {
        fn()
      } catch (err) {
        console.error(err)
      }
    }
  })
}

function destroyMap(): void {
  if (currentPopup) {
    currentPopup.remove()
    currentPopup = null
  }
  hoveredId = null
  framed = false
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
    token = String((await settingsStore.getSettingValue('Systeme.Key.mapBox')) ?? '')
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

  // Premier rendu : la carte se cadre sur la trace dès que ses données sont là.
  render(true)
})

onUnmounted(() => {
  resizeObserver?.disconnect()
  resizeObserver = null
  destroyMap()
})

// Réactivité : les données reçues sont repoussées dans les sources.
watch(() => props.tracePoints, () => render(true))
watch(() => props.passages, () => render(!framed))

// Le suivi d'un segment cadre la carte sur son étendue — c'est aussi ce que
// produit un clic sur un emprunt, qui remonte la sélection au parent.
watch(
  () => props.selectedSegment,
  (segment) => {
    if (segment !== null) frameOnSegment(segment)
  },
)

// La clé Mapbox peut être saisie après le montage : la carte se crée alors.
watch(
  () => settingsStore.settings,
  async () => {
    let token = ''
    try {
      token = String(
        (await settingsStore.getSettingValue('Systeme.Key.mapBox')) ?? '',
      )
    } catch {
      token = ''
    }
    const wasEmpty = !hasMapboxKey.value
    hasMapboxKey.value = !!token

    if (!token && map.value) {
      destroyMap()
      return
    }
    if (token && wasEmpty && !map.value) {
      await initializeMap(token)
      render(true)
    }
  },
)
</script>

<template>
  <div class="mrl-map-wrapper">
    <div ref="mapContainer" class="mrl-map-canvas" />

    <v-empty-state
      v-if="placeholderVisible"
      class="mrl-map-placeholder"
      icon="mdi-map-outline"
      title="Clé Mapbox requise"
      text="Renseignez la clé dans les paramètres (Systeme.Key.mapBox) pour afficher la carte."
    />
  </div>
</template>

<style scoped>
.mrl-map-wrapper {
  position: relative;
  width: 100%;
  height: 100%;
}

.mrl-map-canvas {
  width: 100%;
  height: 100%;
}

.mrl-map-placeholder {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.35);
}

/* Le contenu du popup est créé par Mapbox et inséré sous le conteneur, donc
   hors de l'arbre du composant : `:deep` est nécessaire pour l'atteindre. */
:deep(.mrl-popup) {
  font-size: 12px;
  line-height: 1.5;
}
</style>
