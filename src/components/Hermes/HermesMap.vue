<template>
  <div class="hermes-map">
    <div ref="mapContainer" class="hermes-map__canvas"></div>

    <v-empty-state
      v-if="!hasMapboxKey"
      icon="mdi-map-marker-off-outline"
      title="Clé Mapbox absente"
      text="Renseignez la clé dans les paramètres (Systeme.Key.mapBox) pour afficher la carte."
      class="hermes-map__empty"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Carte de la vue Hermes : datasets, régions indexées et polygones custom.
 *
 * Les couches dépendent de l'onglet actif du panneau : dataset actif en vert
 * (contour sur les onglets Régions/Polygones, remplissage sur l'onglet
 * Datasets), autres datasets en gris, régions en jaune, polygones en orange.
 * L'élément survolé dans la liste — ou sélectionné au clic — passe en bleu.
 *
 * Le style `outdoors-v12` est volontairement classique (pas le style Standard
 * de l'Accueil) : les couches GeoJSON et l'outil de dessin Mapbox s'y ajoutent
 * sans configuration de « slots ».
 *
 * La sélection est remontée au parent (`select-region` / `select-polygon`) ;
 * le parent la renvoie en props, ce qui recharge les FeatureCollections et
 * cadre la carte sur l'objet choisi.
 */
import { computed, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import type { MapMouseEvent } from 'mapbox-gl'
import MapboxDraw from '@mapbox/mapbox-gl-draw'
import '@mapbox/mapbox-gl-draw/dist/mapbox-gl-draw.css'
import { useSettingsStore } from '../../stores/settings'
import type {
  HermesDataset,
  HermesPolygon,
  HermesRegion,
} from '../../stores/hermes'
import {
  unionBboxes,
  type HermesDrawMode,
  type HermesTab,
} from '../../utils/hermes'
import {
  addHermesLayers,
  applyHermesVisibility,
  coverageFeatureCollection,
  datasetsFeatureCollection,
  emptyFC,
  HERMES_LAYER_IDS,
  HERMES_MAP_STYLE,
  HERMES_SOURCE_IDS,
  polygonsFeatureCollection,
  regionsFeatureCollection,
  setSourceData,
} from './hermesMapLayers'

const props = defineProps<{
  regions: HermesRegion[]
  polygons: HermesPolygon[]
  /** Tous les datasets : l'actif est en vert, les autres en gris. */
  datasets: HermesDataset[]
  /** Dataset actif, dont la couverture est dessinée sur tous les onglets. */
  activeDatasetId: string | null
  /** Onglet du panneau : détermine les couches visibles. */
  activeTab: HermesTab
  selectedRegionId: string | null
  selectedPolygonId: string | null
  /** Identifiants survolés dans l'onglet courant. */
  hoveredDatasetId: string | null
  hoveredRegionId: string | null
  hoveredPolygonId: string | null
  drawMode: HermesDrawMode
  editingPolygon: HermesPolygon | null
  /** Régions surlignées par le formulaire de dataset (sélection multiple). */
  previewRegionIds: string[]
  /** Polygones surlignés par le formulaire de dataset (sélection multiple). */
  previewPolygonIds: string[]
  /**
   * Compteur de session : incrémenté à chaque demande de création/édition,
   * même si le mode est déjà le bon. Il force la ré-application de l'outil
   * après une annulation par Échap, sans quoi le bouton « Nouveau » resterait
   * sans effet.
   */
  drawSession: number
}>()

const emit = defineEmits<{
  'select-region': [id: string | null]
  'select-polygon': [id: string | null]
  'draw-create': [geometry: GeoJSON.Polygon]
  'draw-update': [geometry: GeoJSON.Polygon]
}>()

const settingsStore = useSettingsStore()

const mapContainer = ref<HTMLDivElement | null>(null)
const map = shallowRef<mapboxgl.Map | null>(null)
const mapReady = ref(false)
const hasMapboxKey = ref(false)

let resizeObserver: ResizeObserver | null = null
let draw: MapboxDraw | null = null
let hasFitted = false

/** Dataset actif, résolu dans la liste complète pour dessiner sa couverture. */
const activeDataset = computed<HermesDataset | null>(() => {
  if (!props.activeDatasetId) return null
  return (
    props.datasets.find(dataset => dataset.id === props.activeDatasetId) ??
    null
  )
})

/** Première géométrie `Polygon` d'un événement Mapbox Draw. */
function firstPolygon(features: GeoJSON.Feature[]): GeoJSON.Polygon | null {
  const geometry = features[0]?.geometry
  return geometry && geometry.type === 'Polygon' ? geometry : null
}

/**
 * Applique le mode de dessin demandé : nettoyage puis `draw_polygon` pour une
 * création, ajout du polygone édité puis `direct_select` pour une édition,
 * `simple_select` sinon.
 */
function applyDrawMode(): void {
  const drawControl = draw
  if (!drawControl) return
  drawControl.deleteAll()

  if (props.drawMode === 'create') {
    drawControl.changeMode('draw_polygon')
    return
  }
  if (props.drawMode === 'edit' && props.editingPolygon) {
    const ids = drawControl.add({
      type: 'Feature',
      properties: {},
      geometry: props.editingPolygon.geometry,
    })
    const featureId = ids[0]
    if (featureId) {
      drawControl.changeMode('direct_select', { featureId })
    }
    return
  }
  drawControl.changeMode('simple_select')
}

function render(): void {
  const instance = map.value
  if (!instance || !mapReady.value) return

  const active = activeDataset.value
  setSourceData(
    instance,
    HERMES_SOURCE_IDS.coverage,
    active
      ? coverageFeatureCollection(
          active,
          props.regions,
          props.polygons,
          props.hoveredDatasetId === active.id,
        )
      : emptyFC(),
  )

  setSourceData(
    instance,
    HERMES_SOURCE_IDS.datasets,
    datasetsFeatureCollection(
      props.datasets.filter(dataset => dataset.id !== active?.id),
      props.regions,
      props.polygons,
      props.hoveredDatasetId,
    ),
  )

  // Hors de l'onglet Régions, seuls les aperçus du formulaire de dataset
  // doivent rester visibles (le formulaire s'ouvre depuis l'onglet Datasets).
  const previewRegions = new Set(props.previewRegionIds)
  const drawnRegions =
    props.activeTab === 'regions'
      ? props.regions
      : props.regions.filter(region => previewRegions.has(region.id))
  setSourceData(
    instance,
    HERMES_SOURCE_IDS.regions,
    regionsFeatureCollection(
      drawnRegions,
      props.selectedRegionId,
      props.previewRegionIds,
      props.hoveredRegionId,
    ),
  )

  const previewPolygons = new Set(props.previewPolygonIds)
  const drawnPolygons =
    props.activeTab === 'polygons'
      ? props.polygons
      : props.polygons.filter(polygon => previewPolygons.has(polygon.id))
  const editingId =
    props.drawMode === 'edit' ? props.editingPolygon?.id ?? null : null
  setSourceData(
    instance,
    HERMES_SOURCE_IDS.polygons,
    polygonsFeatureCollection(
      drawnPolygons,
      props.selectedPolygonId,
      props.previewPolygonIds,
      props.hoveredPolygonId,
      editingId,
    ),
  )

  applyHermesVisibility(
    instance,
    props.activeTab,
    props.previewRegionIds.length > 0,
    props.previewPolygonIds.length > 0,
  )

  if (!hasFitted && (props.regions.length > 0 || props.polygons.length > 0)) {
    fitAll()
    hasFitted = true
  }
}

function fitAll(): void {
  const instance = map.value
  if (!instance) return
  const bbox = unionBboxes([
    ...props.regions.map(region => region.bbox),
    ...props.polygons.map(polygon => polygon.bbox),
  ])
  if (!bbox) return
  instance.fitBounds(
    [
      [bbox[0], bbox[1]],
      [bbox[2], bbox[3]],
    ],
    { padding: 60, duration: 600, maxZoom: 9 },
  )
}

function fitSelection(): void {
  const instance = map.value
  if (!instance) return
  const region = props.selectedRegionId
    ? props.regions.find(candidate => candidate.id === props.selectedRegionId)
    : null
  const polygon = props.selectedPolygonId
    ? props.polygons.find(candidate => candidate.id === props.selectedPolygonId)
    : null
  const bbox = region?.bbox ?? polygon?.bbox
  if (!bbox) return
  instance.fitBounds(
    [
      [bbox[0], bbox[1]],
      [bbox[2], bbox[3]],
    ],
    { padding: 80, duration: 500, maxZoom: 11 },
  )
}

/** Couches cliquables : seules celles de l'onglet courant. */
function selectionLayers(): string[] {
  if (props.activeTab === 'regions') return [HERMES_LAYER_IDS.regionsFill]
  if (props.activeTab === 'polygons') return [HERMES_LAYER_IDS.polygonsFill]
  return []
}

function onMapClick(event: MapMouseEvent): void {
  const instance = map.value
  if (!instance || !mapReady.value || props.drawMode !== 'idle') return
  const layers = selectionLayers()
  if (layers.length === 0) return

  const features = instance.queryRenderedFeatures(event.point, { layers })
  if (features.length === 0) {
    if (props.activeTab === 'regions') {
      emit('select-region', null)
    } else {
      emit('select-polygon', null)
    }
    return
  }

  const feature = features[0]
  const id = feature.properties?.id
  if (typeof id !== 'string' || !id) return
  if (feature.layer?.id === HERMES_LAYER_IDS.regionsFill) {
    emit('select-region', id)
  } else if (feature.layer?.id === HERMES_LAYER_IDS.polygonsFill) {
    emit('select-polygon', id)
  }
}

function onMouseMove(event: MapMouseEvent): void {
  const instance = map.value
  if (!instance || !mapReady.value || props.drawMode !== 'idle') return
  const layers = selectionLayers()
  if (layers.length === 0) {
    instance.getCanvas().style.cursor = ''
    return
  }
  const features = instance.queryRenderedFeatures(event.point, { layers })
  instance.getCanvas().style.cursor = features.length > 0 ? 'pointer' : ''
}

async function initializeMap(token: string): Promise<void> {
  if (!mapContainer.value) return

  mapboxgl.accessToken = token
  const instance = new mapboxgl.Map({
    container: mapContainer.value,
    style: HERMES_MAP_STYLE,
    center: [2.6, 46.6],
    zoom: 5,
    antialias: true,
  })
  map.value = instance

  instance.addControl(
    new mapboxgl.NavigationControl({ showCompass: false }),
    'top-right',
  )
  instance.addControl(
    new mapboxgl.ScaleControl({ unit: 'metric', maxWidth: 120 }),
    'bottom-left',
  )

  instance.on('click', onMapClick)
  instance.on('mousemove', onMouseMove)
  instance.on('error', event => {
    const message = event?.error?.message ?? ''
    if (/access token|401|403|token/i.test(message)) {
      console.warn('[Hermes] clé Mapbox refusée :', message)
    } else if (message) {
      console.warn('[Mapbox]', message)
    }
  })

  instance.on('load', () => {
    mapReady.value = true
    addHermesLayers(instance)

    const drawControl = new MapboxDraw({
      displayControlsDefault: false,
      controls: {},
      defaultMode: 'simple_select',
    })
    draw = drawControl
    instance.addControl(drawControl)

    instance.on('draw.create', (event: MapboxDraw.DrawCreateEvent) => {
      const geometry = firstPolygon(event.features)
      if (geometry) emit('draw-create', geometry)
    })
    instance.on('draw.update', (event: MapboxDraw.DrawUpdateEvent) => {
      const geometry = firstPolygon(event.features)
      if (geometry) emit('draw-update', geometry)
    })

    render()
    applyDrawMode()
  })
}

function destroyMap(): void {
  if (map.value) {
    try {
      map.value.remove()
    } catch {
      /* destruction best-effort */
    }
  }
  map.value = null
  draw = null
  mapReady.value = false
  hasFitted = false
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
    resizeObserver = new ResizeObserver(() => map.value?.resize())
    resizeObserver.observe(mapContainer.value)
  }
})

onUnmounted(() => {
  resizeObserver?.disconnect()
  resizeObserver = null
  destroyMap()
})

watch(
  () =>
    [
      props.regions,
      props.polygons,
      props.datasets,
      props.activeDatasetId,
      props.activeTab,
      props.hoveredDatasetId,
      props.hoveredRegionId,
      props.hoveredPolygonId,
      props.previewRegionIds,
      props.previewPolygonIds,
    ] as const,
  () => render(),
)

watch(
  () => [props.selectedRegionId, props.selectedPolygonId] as const,
  () => {
    render()
    if (props.selectedRegionId || props.selectedPolygonId) {
      fitSelection()
    }
  },
)

watch(
  () => [props.drawMode, props.editingPolygon, props.drawSession] as const,
  () => {
    applyDrawMode()
    render()
  },
)
</script>

<style scoped>
.hermes-map {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.hermes-map__canvas {
  position: absolute;
  inset: 0;
}

.hermes-map__empty {
  position: absolute;
  inset: 0;
  background: rgba(var(--v-theme-surface), 0.92);
}
</style>
