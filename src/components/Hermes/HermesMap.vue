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
import { useSettingsStore } from '../../stores/settings'
import type { HermesPolygon, HermesRegion } from '../../stores/hermes'
import { unionBboxes } from '../../utils/hermes'
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
}>()

const emit = defineEmits<{
  'select-region': [id: string | null]
  'select-polygon': [id: string | null]
}>()

const settingsStore = useSettingsStore()

const mapContainer = ref<HTMLDivElement | null>(null)
const map = shallowRef<mapboxgl.Map | null>(null)
const mapReady = ref(false)
const hasMapboxKey = ref(false)

let resizeObserver: ResizeObserver | null = null
let hasFitted = false

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
  if (!instance || !mapReady.value) return
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
  if (!instance || !mapReady.value) return
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
    render()
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
