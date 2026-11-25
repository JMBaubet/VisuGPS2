<template>
  <div class="hermes-map">
    <div ref="mapContainer" class="hermes-map__canvas"></div>

    <!-- Capture du prochain clic pour poser le centre des cercles. -->
    <div
      v-if="ringMode"
      class="hermes-map__capture"
      @click="onPlacementClick"
    />

    <v-tooltip
      v-if="hasMapboxKey && mapReady"
      location="right"
      :open-delay="300"
      max-width="280"
    >
      <template #activator="{ props }">
        <v-btn
          v-bind="props"
          class="hermes-map__distance-btn"
          :color="distanceToolActive ? HERMES_DISTANCE_COLOR : undefined"
          :variant="distanceToolActive ? 'flat' : 'elevated'"
          icon="mdi-map-marker-radius"
          size="small"
          title="Marqueur distance"
          @click="toggleDistanceTool"
        />
      </template>
      <span>{{ distanceToolHint }}</span>
    </v-tooltip>

    <div v-if="drawMode !== 'idle'" class="hermes-map__edit-hint">
      <v-icon icon="mdi-gesture-tap" size="14" class="mr-1" />
      Cliquez un point intermédiaire pour ajouter un sommet, glissez pour le
      déplacer, sélectionnez-le puis Suppr pour le retirer.
    </div>

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
 * Les régions sont affichées en permanence, en couche de fond jaune. Les
 * couches dépendent de l'onglet actif du panneau : dataset servi en vert
 * (contour sur l'onglet Polygones, remplissage sur l'onglet Datasets), autres
 * datasets en gris, polygones en orange. L'élément survolé dans la liste — ou
 * le polygone sélectionné au clic — passe en bleu. Les traces favorites et
 * affichées de l'Accueil restent visibles sur tous les onglets, avec la même
 * représentation que sur la carte principale.
 *
 * En création ou édition de polygone, Mapbox Draw affiche les sommets et les
 * points intermédiaires : cliquer un point intermédiaire ajoute un sommet, le
 * glisser le déplace, et Suppr retire le sommet sélectionné. Le bouton
 * « marqueur distance » est indépendant du dessin : il reste disponible en
 * permanence sur les deux onglets.
 *
 * Le style `outdoors-v12` est volontairement classique (pas le style Standard
 * de l'Accueil) : les couches GeoJSON et l'outil de dessin Mapbox s'y ajoutent
 * sans configuration de « slots ».
 *
 * La sélection est remontée au parent (`select-polygon`) ; le parent la renvoie
 * en props, ce qui recharge les FeatureCollections et cadre la carte sur le
 * polygone choisi.
 */
import { computed, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import type { ExpressionSpecification, MapMouseEvent } from 'mapbox-gl'
import MapboxDraw from '@mapbox/mapbox-gl-draw'
import '@mapbox/mapbox-gl-draw/dist/mapbox-gl-draw.css'
import { useSettingsStore } from '../../stores/settings'
import { useTracesStore } from '../../stores/traces'
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
  distanceRingLabels,
  distanceRingsFeatureCollection,
} from '../../utils/hermesGeometry'
import { hexToRgbaString } from '../../utils/materialColors'
import {
  addHermesDistanceLayers,
  addHermesLayers,
  addHermesTraceLayers,
  applyHermesVisibility,
  coverageFeatureCollection,
  datasetsFeatureCollection,
  emptyFC,
  HERMES_DISTANCE_COLOR,
  HERMES_DRAW_POLYGON_STYLE,
  HERMES_LAYER_IDS,
  HERMES_MAP_STYLE,
  HERMES_RING_SOURCE_IDS,
  HERMES_SOURCE_IDS,
  HERMES_TRACE_SOURCE_IDS,
  polygonsFeatureCollection,
  regionsFeatureCollection,
  setSourceData,
} from './hermesMapLayers'

const props = defineProps<{
  regions: HermesRegion[]
  polygons: HermesPolygon[]
  /** Tous les datasets : le dataset servi est en vert, les autres en gris. */
  datasets: HermesDataset[]
  /** Dataset actif, dont la couverture est dessinée sur tous les onglets. */
  activeDatasetId: string | null
  /** Onglet du panneau : détermine les couches visibles. */
  activeTab: HermesTab
  selectedPolygonId: string | null
  /** Identifiants survolés dans l'onglet courant. */
  hoveredDatasetId: string | null
  hoveredPolygonId: string | null
  /** Dataset prévisualisé avant confirmation : surligné et cadré. */
  previewDatasetId: string | null
  drawMode: HermesDrawMode
  editingPolygon: HermesPolygon | null
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
  'select-polygon': [id: string | null]
  'draw-create': [geometry: GeoJSON.Polygon]
  'draw-update': [geometry: GeoJSON.Polygon]
  'draw-draft': [geometry: GeoJSON.Polygon | null]
  'draw-clear': []
}>()

const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()

const mapContainer = ref<HTMLDivElement | null>(null)
const map = shallowRef<mapboxgl.Map | null>(null)
const mapReady = ref(false)
const hasMapboxKey = ref(false)
/** Le prochain clic sur la carte définit le centre des cercles. */
const ringMode = ref(false)
/** Centre des cercles de distance, ou `null` quand ils sont masqués. */
const ringCenter = ref<[number, number] | null>(null)
/** Marqueurs HTML des étiquettes 50/100/150 km. */
let ringMarkers: mapboxgl.Marker[] = []

let resizeObserver: ResizeObserver | null = null
let draw: MapboxDraw | null = null
/** Canvas de la carte, cible des raccourcis clavier de Mapbox Draw. */
let drawCanvas: HTMLCanvasElement | null = null
/** Neutralise `draw.delete` pendant que `applyDrawMode` nettoie l'outil. */
let applyingMode = false
/** Émet le tracé en cours pendant `draw_polygon`, jusqu'au `draw.create`. */
let draftActive = false
let lastDraftKey = ''
let lastDraftAt = 0
let hasFitted = false
/** Jeton d'invalidation des chargements de géométries de traces. */
let traceRefreshToken = 0

/** Dataset actif, résolu dans la liste complète pour dessiner sa couverture. */
const activeDataset = computed<HermesDataset | null>(() => {
  if (!props.activeDatasetId) return null
  return (
    props.datasets.find(dataset => dataset.id === props.activeDatasetId) ??
    null
  )
})

const distanceToolActive = computed(
  () => ringMode.value || ringCenter.value !== null,
)

const distanceToolHint = computed(() => {
  if (ringMode.value) {
    return 'Cliquez sur la carte pour définir le centre des cercles.'
  }
  if (ringCenter.value) {
    return 'Effacer les cercles de distance.'
  }
  return 'Afficher les cercles de distance (10 à 150 km, renforcés tous les 50 km). Disponible sur les deux onglets.'
})

/** Première géométrie `Polygon` d'un événement Mapbox Draw. */
function firstPolygon(features: GeoJSON.Feature[]): GeoJSON.Polygon | null {
  const geometry = features[0]?.geometry
  return geometry && geometry.type === 'Polygon' ? geometry : null
}

/** Polygone en cours de tracé, ou `null` tant qu'il n'a pas deux points. */
function currentDraftGeometry(): GeoJSON.Polygon | null {
  const drawControl = draw
  if (!drawControl) return null
  const feature = drawControl
    .getAll()
    .features.find(candidate => candidate.geometry.type === 'Polygon')
  if (!feature || feature.geometry.type !== 'Polygon') return null
  const ring = feature.geometry.coordinates[0]
  if (!ring || ring.length < 2) return null
  return feature.geometry
}

/**
 * Émet le brouillon pendant `draw_polygon`. `draw.render` est déjà lié aux
 * animation frames par Mapbox Draw ; on limite en plus à ~10 Hz pour ne pas
 * recalculer la surface à chaque pixel de souris.
 */
function emitDrawDraft(): void {
  if (!draftActive || props.drawMode !== 'create') return
  const now = performance.now()
  if (now - lastDraftAt < 100) return
  const geometry = currentDraftGeometry()
  const key = geometry ? JSON.stringify(geometry.coordinates) : ''
  if (key === lastDraftKey) return
  lastDraftKey = key
  lastDraftAt = now
  emit('draw-draft', geometry)
}

/** Donne le focus clavier au canvas pour que Suppr atteigne Mapbox Draw. */
function focusDrawCanvas(): void {
  const canvas = map.value?.getCanvas()
  if (!canvas) return
  canvas.tabIndex = 0
  canvas.focus({ preventScroll: true })
}

/**
 * Raccourci de suppression de sommet.
 *
 * Avec `displayControlsDefault: false` et `controls: {}`, Mapbox Draw met
 * `controls.trash` à `false` et désactive son propre traitement de
 * `Suppr`/`Backspace`. On le rétablit uniquement en `direct_select`, pour ne
 * jamais supprimer le polygone entier depuis `simple_select`.
 */
function onDrawKeyDown(event: KeyboardEvent): void {
  const drawControl = draw
  if (!drawControl || props.drawMode === 'idle') return
  if (event.key !== 'Delete' && event.key !== 'Backspace') return
  if (drawControl.getMode() !== 'direct_select') return
  event.preventDefault()
  drawControl.trash()
}

/**
 * Applique le mode de dessin demandé : nettoyage puis `draw_polygon` pour une
 * création, ajout du polygone édité puis `direct_select` pour une édition,
 * `simple_select` sinon.
 */
function applyDrawMode(): void {
  const drawControl = draw
  if (!drawControl) return
  applyingMode = true
  try {
    drawControl.deleteAll()

    if (props.drawMode === 'create') {
      draftActive = true
      lastDraftKey = ''
      lastDraftAt = 0
      drawControl.changeMode('draw_polygon')
      return
    }
    draftActive = false
    if (props.drawMode === 'edit' && props.editingPolygon) {
      const ids = drawControl.add({
        type: 'Feature',
        properties: {},
        geometry: props.editingPolygon.geometry,
      })
      const featureId = ids[0]
      if (featureId) {
        drawControl.changeMode('direct_select', { featureId })
        focusDrawCanvas()
      }
      return
    }
    drawControl.changeMode('simple_select')
  } finally {
    applyingMode = false
  }
}

/**
 * Couche du thème Mapbox Draw. Les types publiés décrivent une ancienne
 * nomenclature de couches ; le thème runtime de la version installée utilise
 * les identifiants `gl-draw-*`.
 */
interface DrawThemeLayer {
  id: string
  paint?: Record<string, unknown>
  [key: string]: unknown
}

/**
 * Construit le thème de Mapbox Draw avec les couleurs de
 * `HERMES_DRAW_POLYGON_STYLE`. Le thème est fourni au constructeur : Draw
 * applique les peintures au moment où il crée ses couches. Un
 * `setPaintProperty` immédiat serait ignoré, car Draw diffère `addLayers`
 * tant que la carte n'est pas complètement `loaded()`.
 */
function buildDrawStyles(): DrawThemeLayer[] {
  const drawStyle = HERMES_DRAW_POLYGON_STYLE
  const theme = MapboxDraw.lib.theme as unknown as DrawThemeLayer[]

  return theme.map(layer => {
    const paint = { ...(layer.paint ?? {}) }
    switch (layer.id) {
      case 'gl-draw-polygon-fill':
        paint['fill-color'] = drawStyle.fillColor
        paint['fill-opacity'] = drawStyle.fillOpacity
        break
      case 'gl-draw-lines':
        paint['line-color'] = drawStyle.lineColor
        paint['line-width'] = drawStyle.lineWidth
        paint['line-dasharray'] = drawStyle.lineDasharray
        break
      case 'gl-draw-vertex-outer':
        paint['circle-color'] = drawStyle.vertexOuterColor
        break
      case 'gl-draw-vertex-inner':
        paint['circle-color'] = drawStyle.vertexColor
        break
      case 'gl-draw-midpoint':
        paint['circle-color'] = drawStyle.midpointColor
        break
      default:
        return layer
    }
    return { ...layer, paint }
  })
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
      props.previewDatasetId ?? props.hoveredDatasetId,
    ),
  )

  // Les régions sont une couche de fond permanente : elles restent visibles
  // sur les deux onglets, en jaune, sans sélection ni survol.
  setSourceData(
    instance,
    HERMES_SOURCE_IDS.regions,
    regionsFeatureCollection(props.regions),
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
    props.previewPolygonIds.length > 0,
  )

  if (!hasFitted && (props.regions.length > 0 || props.polygons.length > 0)) {
    if (props.activeDatasetId) {
      fitDataset()
    } else {
      fitAll()
    }
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

/**
 * Cadre la carte sur le périmètre complet d'un jeu de données : union des bbox
 * de ses secteurs (zones) ou, à défaut, de ses régions.
 */
function fitDataset(dataset: HermesDataset | null = activeDataset.value): void {
  const instance = map.value
  if (!dataset || !instance) return
  const bboxes: [number, number, number, number][] = []
  for (const zoneId of dataset.zones ?? []) {
    const polygon = props.polygons.find(candidate => candidate.id === zoneId)
    if (polygon) bboxes.push(polygon.bbox)
  }
  for (const regionId of dataset.regions ?? []) {
    const region = props.regions.find(candidate => candidate.id === regionId)
    if (region) bboxes.push(region.bbox)
  }
  const bbox = unionBboxes(bboxes)
  if (!bbox) return
  instance.fitBounds(
    [
      [bbox[0], bbox[1]],
      [bbox[2], bbox[3]],
    ],
    { padding: 60, duration: 600, maxZoom: 10 },
  )
}

/** Cadre la carte sur un dataset arbitraire (prévisualisation). */
function fitDatasetById(datasetId: string | null): void {
  if (!datasetId) return
  fitDataset(
    props.datasets.find(dataset => dataset.id === datasetId) ?? null,
  )
}

function fitSelection(): void {
  const instance = map.value
  if (!instance) return
  const polygon = props.selectedPolygonId
    ? props.polygons.find(candidate => candidate.id === props.selectedPolygonId)
    : null
  const bbox = polygon?.bbox
  if (!bbox) return
  instance.fitBounds(
    [
      [bbox[0], bbox[1]],
      [bbox[2], bbox[3]],
    ],
    { padding: 80, duration: 500, maxZoom: 11 },
  )
}

/** Couches cliquables : seuls les polygones de l'onglet Polygones. */
function selectionLayers(): string[] {
  if (props.activeTab === 'polygons') return [HERMES_LAYER_IDS.polygonsFill]
  return []
}

/** Valeur d'un paramètre de la carte (undefined tant que les settings ne sont pas chargés). */
function getParam(path: string): any {
  return settingsStore.getParamDef(path)?.value
}

/** Couleur d'un paramètre convertie en rgba(), avec fallback #RRGGBBAA. */
function getColor(path: string, fallback: string): string {
  return hexToRgbaString(getParam(path) ?? fallback)
}

/** Dégradé des traces affichées, identique à celui de la carte de l'Accueil. */
function buildDisplayedGradient(): ExpressionSpecification {
  const debut = getColor('Carte.Traces.couleurDebut', '#2196F3FF')
  const fin = getColor('Carte.Traces.couleurFin', '#F44336FF')
  const activerMilieu = getParam('Carte.Traces.activerCouleurMilieu') ?? true
  const mid = getParam('Carte.Traces.positionCouleurMilieu') ?? 0.5

  if (activerMilieu) {
    const couleurMilieu = getColor('Carte.Traces.couleurMilieu', '#9C27B0FF')
    return [
      'interpolate',
      ['linear'],
      ['line-progress'],
      0, debut,
      mid, couleurMilieu,
      1, fin,
    ] as unknown as ExpressionSpecification
  }
  return [
    'interpolate',
    ['linear'],
    ['line-progress'],
    0, debut,
    1, fin,
  ] as unknown as ExpressionSpecification
}

/**
 * Recharge les géométries des traces favorites et affichées. Les deux sources
 * sont indépendantes : une trace à la fois favorite et affichée est dessinée
 * dans les deux, comme sur la carte de l'Accueil.
 */
async function refreshTraceLayers(): Promise<void> {
  const instance = map.value
  if (!instance || !mapReady.value) return

  const token = ++traceRefreshToken
  const favorites: GeoJSON.Feature[] = []
  const displayed: GeoJSON.Feature[] = []

  for (const trace of tracesStore.traces) {
    if (!trace.favorite && !trace.is_displayed) continue
    try {
      const geometry = await tracesStore.getTraceGeometry(trace.id)
      if (
        token !== traceRefreshToken ||
        map.value !== instance ||
        !mapReady.value
      ) {
        return
      }
      if (trace.favorite) favorites.push(geometry)
      if (trace.is_displayed) displayed.push(geometry)
    } catch (error) {
      console.error(
        `[Hermes] Géométrie introuvable pour la trace ${trace.id} :`,
        error,
      )
    }
  }

  setSourceData(instance, HERMES_TRACE_SOURCE_IDS.favorites, {
    type: 'FeatureCollection',
    features: favorites,
  })
  setSourceData(instance, HERMES_TRACE_SOURCE_IDS.displayed, {
    type: 'FeatureCollection',
    features: displayed,
  })
}

/** Affiche ou efface les anneaux et le point central. */
function renderDistanceRings(): void {
  const instance = map.value
  if (!instance || !mapReady.value) return

  const center = ringCenter.value
  if (!center) {
    setSourceData(instance, HERMES_RING_SOURCE_IDS.rings, emptyFC())
    setSourceData(instance, HERMES_RING_SOURCE_IDS.center, emptyFC())
    clearRingMarkers()
    return
  }

  setSourceData(
    instance,
    HERMES_RING_SOURCE_IDS.rings,
    distanceRingsFeatureCollection(center),
  )
  setSourceData(instance, HERMES_RING_SOURCE_IDS.center, {
    type: 'FeatureCollection',
    features: [
      {
        type: 'Feature',
        properties: {},
        geometry: { type: 'Point', coordinates: center },
      },
    ],
  })
  renderRingLabels(center)
}

/** Étiquettes « 50 km », « 100 km » et « 150 km » au nord des anneaux. */
function renderRingLabels(center: [number, number]): void {
  clearRingMarkers()
  const instance = map.value
  if (!instance) return

  for (const label of distanceRingLabels(center)) {
    const element = document.createElement('div')
    element.textContent = label.label
    element.style.cssText = `color:${HERMES_DISTANCE_COLOR};font-size:11px;font-weight:600;text-shadow:0 0 3px #fff,0 0 3px #fff;pointer-events:none;white-space:nowrap;`
    const marker = new mapboxgl.Marker({ element, anchor: 'bottom' })
      .setLngLat(label.position)
      .addTo(instance)
    ringMarkers.push(marker)
  }
}

function clearRingMarkers(): void {
  for (const marker of ringMarkers) marker.remove()
  ringMarkers = []
}

function clearDistanceRings(): void {
  ringMode.value = false
  ringCenter.value = null
  clearRingMarkers()
  const instance = map.value
  if (!instance || !mapReady.value) return
  setSourceData(instance, HERMES_RING_SOURCE_IDS.rings, emptyFC())
  setSourceData(instance, HERMES_RING_SOURCE_IDS.center, emptyFC())
}

/** Bouton de la carte : arme le placement, puis efface les cercles existants. */
function toggleDistanceTool(): void {
  if (ringCenter.value) {
    clearDistanceRings()
    return
  }
  ringMode.value = !ringMode.value
}

/** Clic sur la surface de capture : convertit le pixel en coordonnées. */
function onPlacementClick(event: MouseEvent): void {
  const instance = map.value
  const target = event.currentTarget as HTMLElement | null
  if (!instance || !target) return

  const rect = target.getBoundingClientRect()
  const point = new mapboxgl.Point(
    event.clientX - rect.left,
    event.clientY - rect.top,
  )
  const lngLat = instance.unproject(point)
  ringMode.value = false
  ringCenter.value = [lngLat.lng, lngLat.lat]
  renderDistanceRings()
}

function onMapClick(event: MapMouseEvent): void {
  const instance = map.value
  if (!instance || !mapReady.value || props.drawMode !== 'idle') return
  const layers = selectionLayers()
  if (layers.length === 0) return

  const features = instance.queryRenderedFeatures(event.point, { layers })
  if (features.length === 0) {
    emit('select-polygon', null)
    return
  }

  const feature = features[0]
  const id = feature.properties?.id
  if (typeof id !== 'string' || !id) return
  if (feature.layer?.id === HERMES_LAYER_IDS.polygonsFill) {
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
    addHermesTraceLayers(instance, {
      favoriteColor: getColor('Carte.Favoris.couleurTrace', '#FFEB3BFF'),
      favoriteWidth: Number(getParam('Carte.Favoris.epaisseur') ?? 6),
      displayedWidth: Number(getParam('Carte.Traces.epaisseur') ?? 4),
      displayedGradient: buildDisplayedGradient(),
    })
    addHermesDistanceLayers(instance)

    const drawControl = new MapboxDraw({
      displayControlsDefault: false,
      controls: {},
      defaultMode: 'simple_select',
      styles: buildDrawStyles(),
    })
    draw = drawControl
    instance.addControl(drawControl)

    drawCanvas = instance.getCanvas()
    drawCanvas.addEventListener('keydown', onDrawKeyDown)

    instance.on('draw.render', emitDrawDraft)

    instance.on('draw.create', (event: MapboxDraw.DrawCreateEvent) => {
      draftActive = false
      const feature = event.features[0]
      if (!feature || feature.geometry.type !== 'Polygon') return
      emit('draw-create', feature.geometry)

      // `onStop` de draw_polygon déclenche `draw.create` puis bascule lui-même
      // en simple_select : on diffère la sélection directe pour ne pas être
      // écrasé, et on rend les sommets immédiatement manipulables.
      const featureId =
        typeof feature.id === 'string' ? feature.id : null
      if (!featureId) return
      window.setTimeout(() => {
        if (!draw || props.drawMode === 'idle') return
        draw.changeMode('direct_select', { featureId })
        focusDrawCanvas()
      }, 0)
    })
    instance.on('draw.update', (event: MapboxDraw.DrawUpdateEvent) => {
      const geometry = firstPolygon(event.features)
      if (geometry) emit('draw-update', geometry)
    })
    instance.on('draw.delete', () => {
      draftActive = false
      // `deleteAll()` de `applyDrawMode` émet aussi `draw.delete` : ne pas
      // vider une géométrie d'édition à cause d'un nettoyage interne.
      if (applyingMode) return
      emit('draw-clear')
    })

    render()
    applyDrawMode()
    void refreshTraceLayers()
    renderDistanceRings()
  })
}

function destroyMap(): void {
  traceRefreshToken += 1
  clearRingMarkers()
  if (drawCanvas) {
    drawCanvas.removeEventListener('keydown', onDrawKeyDown)
    drawCanvas = null
  }
  if (map.value) {
    try {
      map.value.remove()
    } catch {
      /* destruction best-effort */
    }
  }
  map.value = null
  draw = null
  applyingMode = false
  mapReady.value = false
  hasFitted = false
}

onMounted(async () => {
  if (tracesStore.traces.length === 0) {
    void tracesStore.loadTraces()
  }
  // Les couleurs et épaisseurs des traces viennent des paramètres Carte.* :
  // ils doivent être chargés avant la création des couches.
  await settingsStore.loadSettings()

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
      props.hoveredPolygonId,
      props.previewDatasetId,
      props.previewPolygonIds,
    ] as const,
  () => render(),
)

watch(
  () => props.selectedPolygonId,
  () => {
    render()
    if (props.selectedPolygonId) {
      fitSelection()
    }
  },
)

// Sélection d'un jeu de données : la carte cadre son périmètre complet, pour
// que sa couverture soit entièrement visible.
watch(
  () => props.activeDatasetId,
  datasetId => {
    render()
    if (datasetId) {
      fitDataset()
    }
  },
)

// Prévisualisation avant confirmation : surbrillance puis cadrage.
watch(
  () => props.previewDatasetId,
  datasetId => {
    render()
    fitDatasetById(datasetId)
  },
)

watch(
  () => [props.drawMode, props.editingPolygon, props.drawSession] as const,
  () => {
    applyDrawMode()
    render()
  },
)

watch(
  () => tracesStore.traces,
  () => {
    void refreshTraceLayers()
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

.hermes-map__capture {
  position: absolute;
  inset: 0;
  z-index: 3;
  cursor: crosshair;
}

.hermes-map__distance-btn {
  position: absolute;
  top: 10px;
  left: 10px;
  z-index: 4;
}

.hermes-map__edit-hint {
  position: absolute;
  bottom: 12px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 4;
  display: flex;
  align-items: center;
  max-width: min(620px, calc(100% - 24px));
  padding: 6px 10px;
  border-radius: 6px;
  background: rgba(var(--v-theme-surface), 0.92);
  color: rgba(var(--v-theme-on-surface), 0.85);
  font-size: 0.72rem;
  line-height: 1.3;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
  pointer-events: none;
}

.hermes-map :deep(.mapboxgl-canvas:focus) {
  outline: none;
}

.hermes-map__empty {
  position: absolute;
  inset: 0;
  background: rgba(var(--v-theme-surface), 0.92);
}
</style>
