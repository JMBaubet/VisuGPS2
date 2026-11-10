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
 * Carte de la vue Hermes : régions indexées et polygones custom.
 *
 * Le style `outdoors-v12` est volontairement classique (pas le style Standard
 * de l'Accueil) : les couches GeoJSON et, à la sous-étape 1.5, l'outil de
 * dessin Mapbox s'y ajoutent sans configuration de « slots ».
 *
 * La sélection est remontée au parent (`select-region` / `select-polygon`) ;
 * le parent la renvoie en props, ce qui recharge les FeatureCollections et
 * cadre la carte sur l'objet choisi.
 */
import { onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import type { MapMouseEvent } from 'mapbox-gl'
import MapboxDraw from '@mapbox/mapbox-gl-draw'
import '@mapbox/mapbox-gl-draw/dist/mapbox-gl-draw.css'
import { useSettingsStore } from '../../stores/settings'
import type { HermesPolygon, HermesRegion } from '../../stores/hermes'
import { unionBboxes, type HermesDrawMode } from '../../utils/hermes'
import {
  addHermesLayers,
  HERMES_LAYER_IDS,
  HERMES_MAP_STYLE,
  polygonsFeatureCollection,
  regionsFeatureCollection,
  setPolygonsData,
  setRegionsData,
} from './hermesMapLayers'

const props = defineProps<{
  regions: HermesRegion[]
  polygons: HermesPolygon[]
  selectedRegionId: string | null
  selectedPolygonId: string | null
  drawMode: HermesDrawMode
  editingPolygon: HermesPolygon | null
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

  setRegionsData(
    instance,
    regionsFeatureCollection(props.regions, props.selectedRegionId),
  )
  setPolygonsData(
    instance,
    polygonsFeatureCollection(props.polygons, props.selectedPolygonId),
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

function onMapClick(event: MapMouseEvent): void {
  const instance = map.value
  if (!instance || !mapReady.value || props.drawMode !== 'idle') return
  const features = instance.queryRenderedFeatures(event.point, {
    layers: [HERMES_LAYER_IDS.regionsFill, HERMES_LAYER_IDS.polygonsFill],
  })
  if (features.length === 0) {
    emit('select-region', null)
    emit('select-polygon', null)
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
  const features = instance.queryRenderedFeatures(event.point, {
    layers: [HERMES_LAYER_IDS.regionsFill, HERMES_LAYER_IDS.polygonsFill],
  })
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
      props.selectedRegionId,
      props.selectedPolygonId,
    ] as const,
  () => {
    render()
    if (props.selectedRegionId || props.selectedPolygonId) {
      fitSelection()
    }
  },
)

watch(
  () => [props.drawMode, props.editingPolygon, props.drawSession] as const,
  () => applyDrawMode(),
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
