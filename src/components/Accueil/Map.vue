<template>
  <div ref="mapContainer" class="map-container"></div>
</template>

<script setup lang="ts">
/**
 * Composant carte Mapbox GL avec clustering des points de départ,
 * affichage des traces favorites et des traces affichées (dégradé).
 *
 * Couches (du bas vers le haut) :
 *   1. clusters / cluster-count / unclustered-point (points de départ)
 *   2. favorites-line          (LineString favoris, couleur favori, épaisseur 6)
 *   3. displayed-traces-line   (LineString dégradé, épaisseur 4) — au-dessus des favoris
 *
 * Conventions respectées :
 * - <script setup lang="ts">, imports relatifs
 * - Token Mapbox depuis settingsStore (Systeme.Key.mapBox)
 * - Données depuis tracesStore (pas de fetch direct)
 * - Debounce sur moveend pour limiter les recalculs
 * - Couleurs lues dans settingsStore (namespace Carte.*), fallback sur défauts
 */
import { ref, onMounted, onUnmounted, watch } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import { useSettingsStore } from '../../stores/settings'
import { useTracesStore } from '../../stores/traces'
import type { TraceMetadata } from '../../stores/traces'
import { useAppStore } from '../../stores/app'
import { hexToRgbaString } from '../../utils/materialColors'

// --- Stores ---

const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const appStore = useAppStore()

// --- Références ---

const mapContainer = ref<HTMLDivElement | null>(null)
let map: mapboxgl.Map | null = null
let moveEndTimer: ReturnType<typeof setTimeout> | null = null
const MOVE_END_DEBOUNCE_MS = 150
/** Observer du redimensionnement du conteneur (changement de thème, drawer…). */
let resizeObserver: ResizeObserver | null = null

// --- Lecture des paramètres (avec fallback sur les valeurs par défaut) ---

/** Valeur d'un paramètre via son chemin (ex. 'Carte.Favoris.couleurCluster'). */
function getParam(path: string): any {
  return settingsStore.getParamDef(path)?.value
}

/** Couleur d'un paramètre convertie en rgba(), avec fallback #RRGGBBAA. */
function getColor(path: string, fallback: string): string {
  return hexToRgbaString(getParam(path) ?? fallback)
}

// --- Construction GeoJSON : points de départ (source clusterisée 'traces') ---

/**
 * Construit un FeatureCollection GeoJSON à partir des traces du store.
 * Les coordonnées sont en [longitude, latitude] (convention Mapbox).
 * La propriété numérique `favorite` (0/1) permet de colorer les clusters
 * contenant au moins un favori.
 */
function buildTracesGeoJSON(): GeoJSON.FeatureCollection<GeoJSON.Geometry> {
  return {
    type: 'FeatureCollection',
    features: tracesStore.traces.map(t => ({
      type: 'Feature' as const,
      geometry: {
        type: 'Point' as const,
        coordinates: [t.stats.start_point.lon, t.stats.start_point.lat],
      },
      properties: {
        id: t.id,
        name: t.name,
        source: t.source,
        activity_type: t.activity_type,
        start_lat: t.stats.start_point.lat,
        start_lon: t.stats.start_point.lon,
        favorite: t.favorite ? 1 : 0,
      },
    })),
  }
}

// --- Expression de dégradé pour les traces affichées ---

/**
 * Construit l'expression Mapbox `line-gradient` à partir des paramètres.
 * Le dégradé s'appuie sur `line-progress` (0 → 1 le long de chaque segment).
 */
function buildGradientExpression(): mapboxgl.Expression {
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
    ]
  }
  return [
    'interpolate',
    ['linear'],
    ['line-progress'],
    0, debut,
    1, fin,
  ]
}

// --- Couleur conditionnelle des clusters (favori en priorité) ---

/** Expression de couleur des clusters : favori d'abord, puis paliers existants. */
function buildClusterColorExpression(): mapboxgl.Expression {
  const couleurFavori = getColor('Carte.Favoris.couleurCluster', '#FFEB3BFF')
  return [
    'case',
    ['==', ['get', 'hasFavorite'], 1], couleurFavori,
    [
      'step',
      ['get', 'point_count'],
      '#51bbd6',
      10, '#f1f075',
      100, '#f28cb1',
    ],
  ]
}

/**
 * Expression de couleur des points de départ non clusterisés :
 * couleur de la trace favorite si favori, sinon la couleur par défaut.
 */
function buildPointColorExpression(): mapboxgl.Expression {
  const couleurFavori = getColor('Carte.Favoris.couleurTrace', '#FFEB3BFF')
  return [
    'case',
    ['==', ['get', 'favorite'], 1], couleurFavori,
    '#4264fb',
  ]
}

// --- Couches de clustering ---

/** Ajoute la source clusterisée et les 3 couches (clusters, compteur, points). */
function addClusterLayers() {
  if (!map) return

  map.addSource('traces', {
    type: 'geojson',
    data: buildTracesGeoJSON(),
    cluster: true,
    clusterRadius: 50,
    clusterMaxZoom: 14,
    // Propriété agrégée : 1 si le cluster contient au moins un favori.
    clusterProperties: { hasFavorite: ['max', ['get', 'favorite']] },
  })

  // Couche des clusters (favori en priorité, puis paliers par point_count)
  map.addLayer({
    id: 'clusters',
    type: 'circle',
    source: 'traces',
    filter: ['has', 'point_count'],
    paint: {
      'circle-color': buildClusterColorExpression(),
      'circle-radius': [
        'step',
        ['get', 'point_count'],
        20,
        30,
        40,
      ],
    },
  })

  // Compteur dans chaque cluster
  map.addLayer({
    id: 'cluster-count',
    type: 'symbol',
    source: 'traces',
    filter: ['has', 'point_count'],
    layout: {
      'text-field': '{point_count_abbreviated}',
      'text-size': 12,
    },
  })

  // Points individuels (non clusterisés) — favori coloré comme la trace favorite
  map.addLayer({
    id: 'unclustered-point',
    type: 'circle',
    source: 'traces',
    filter: ['!', ['has', 'point_count']],
    paint: {
      'circle-color': buildPointColorExpression(),
      'circle-radius': 8,
      'circle-stroke-width': 1,
      'circle-stroke-color': '#fff',
    },
  })
}

// --- Couches de LineString (favoris puis traces affichées) ---

/** Collection vide réutilisable pour initialiser les sources GeoJSON. */
function emptyCollection(): GeoJSON.FeatureCollection {
  return { type: 'FeatureCollection', features: [] }
}

/** Ajoute la couche des LineString favorites (couleur favori, épaisseur 6). */
function addFavoritesLayer() {
  if (!map) return

  const epaisseur = getParam('Carte.Favoris.epaisseur') ?? 6
  const couleurTrace = getColor('Carte.Favoris.couleurTrace', '#FFEB3BFF')

  map.addSource('favorites', { type: 'geojson', data: emptyCollection() })
  map.addLayer({
    id: 'favorites-line',
    type: 'line',
    source: 'favorites',
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-color': couleurTrace,
      'line-width': epaisseur,
    },
  })
}

/** Ajoute la couche des LineString affichées (dégradé, épaisseur 4). */
function addDisplayedTracesLayer() {
  if (!map) return

  const epaisseur = getParam('Carte.Traces.epaisseur') ?? 4

  // lineMetrics: true est OBLIGATOIRE pour que line-gradient soit rendu
  // (cf. documentation Mapbox GL JS — sans cette option, le dégradé est ignoré).
  map.addSource('displayed-traces', {
    type: 'geojson',
    data: emptyCollection(),
    lineMetrics: true,
  })
  map.addLayer({
    id: 'displayed-traces-line',
    type: 'line',
    source: 'displayed-traces',
    layout: { 'line-cap': 'round', 'line-join': 'round' },
    paint: {
      'line-width': epaisseur,
      'line-gradient': buildGradientExpression(),
    },
  })
}

/**
 * Ajoute la couche dédiée au « focus » (clic Info dans Circuit.vue).
 * Même représentation que les traces affichées (dégradé), mais initialement
 * masquée (visibility: 'none'). Elle n'affiche qu'une seule trace à la fois.
 */
function addFocusTracesLayer() {
  if (!map) return

  const epaisseur = getParam('Carte.Traces.epaisseur') ?? 4

  map.addSource('focus-traces', {
    type: 'geojson',
    data: emptyCollection(),
    lineMetrics: true,
  })
  map.addLayer({
    id: 'focus-traces-line',
    type: 'line',
    source: 'focus-traces',
    layout: {
      'line-cap': 'round',
      'line-join': 'round',
      'visibility': 'none', // masquée par défaut, activée lors d'un focus
    },
    paint: {
      'line-width': epaisseur,
      'line-gradient': buildGradientExpression(),
    },
  })
}

/**
 * Calcule les bounds (LngLatBounds) d'une Feature LineString en parcourant
 * ses coordonnées. Utilisé par map.fitBounds lors du focus sur un circuit.
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

// --- Rechargement des géométries (favoris + traces affichées) ---

/**
 * Recharge les géométries des traces via le store (avec cache) et met à jour
 * les sources `favorites` et `displayed-traces`.
 */
async function refreshLineLayers() {
  if (!map) return

  const favoriteFeatures: GeoJSON.Feature[] = []
  const displayedFeatures: GeoJSON.Feature[] = []

  for (const t of tracesStore.traces) {
    if (!t.favorite && !t.is_displayed) continue
    try {
      const geometry = await tracesStore.getTraceGeometry(t.id)
      if (t.favorite) favoriteFeatures.push(geometry)
      if (t.is_displayed) displayedFeatures.push(geometry)
    } catch (error) {
      console.error(`Géométrie introuvable pour la trace ${t.id} :`, error)
    }
  }

  const favoritesSource = map.getSource('favorites') as mapboxgl.GeoJSONSource | undefined
  if (favoritesSource) {
    favoritesSource.setData({ type: 'FeatureCollection', features: favoriteFeatures })
  }

  const displayedSource = map.getSource('displayed-traces') as mapboxgl.GeoJSONSource | undefined
  if (displayedSource) {
    displayedSource.setData({ type: 'FeatureCollection', features: displayedFeatures })
  }
}

// --- Gestion des clics ---

/** Zoom d'expansion vers le centre d'un cluster (comportement par défaut). */
function expandCluster(feature: mapboxgl.MapboxGeoJSONFeature) {
  if (!map) return
  const clusterId = feature.properties?.cluster_id as number
  const source = map.getSource('traces') as mapboxgl.GeoJSONSource
  source.getClusterExpansionZoom(clusterId, (err, zoom) => {
    if (err || !map) return
    map.easeTo({
      center: (feature.geometry as GeoJSON.Point).coordinates as [number, number],
      zoom: zoom ?? map.getZoom(),
    })
  })
}

/**
 * Seuil de circuits en-deçà duquel un clic sur un cluster affiche le popup
 * listant les circuits ; au-delà, le cluster s'étend (zoom d'expansion).
 */
function getClusterPopupThreshold(): number {
  return getParam('Carte.Clusters.seuilPopupCircuits') ?? 10
}

/**
 * Clic sur un cluster :
 * - si le nombre de circuits est ≤ seuil → popup listant les circuits ;
 * - sinon → zoom d'expansion (comportement par défaut).
 */
function handleClusterClick(e: mapboxgl.MapMouseEvent) {
  if (!map) return
  const feature = e.features?.[0]
  if (!feature || !feature.properties || feature.properties.cluster_id == null) return

  const pointCount = feature.properties.point_count as number
  const threshold = getClusterPopupThreshold()

  // Au-delà du seuil : on étend le cluster (pas de popup, trop de circuits).
  if (pointCount > threshold) {
    expandCluster(feature)
    return
  }

  // En-deçà du seuil : on récupère les circuits du cluster et on affiche le popup.
  const clusterId = feature.properties.cluster_id as number
  const source = map.getSource('traces') as mapboxgl.GeoJSONSource
  // Capturer l'époque courante pour invalider cette ouverture si un zoom /
  // double-clic survient avant la fin de getClusterLeaves (asynchrone).
  const epoch = popupEpoch
  source.getClusterLeaves(clusterId, pointCount, 0, (err, leaves) => {
    if (err || !map) return
    // Annuler si une fermeture (zoom, double-clic…) a eu lieu entre-temps.
    if (epoch !== popupEpoch) return
    if (!leaves || leaves.length === 0) {
      expandCluster(feature)
      return
    }

    // Faire correspondre chaque feuille (point) à sa TraceMetadata via l'id.
    const ids = leaves
      .map(l => l.properties?.id as string | undefined)
      .filter((id): id is string => !!id)
    const traces = ids
      .map(id => tracesStore.traces.find(t => t.id === id))
      .filter((t): t is TraceMetadata => !!t)

    if (traces.length === 0) {
      expandCluster(feature)
      return
    }

    // Fermer un éventuel popup précédent.
    closeCurrentPopup()
    currentPopup_kind = 'cluster'
    currentPopup_traceIds = traces.map(t => t.id)

    const coords = (feature.geometry as GeoJSON.Point).coordinates as [number, number]
    const popup = new mapboxgl.Popup({ offset: 12, closeButton: true })
      .setLngLat(coords)
      .setDOMContent(buildClusterPopupContent(traces))
      .addTo(map)

    popup.on('close', onPopupClose)
    currentPopup = popup
  })
}

// --- Popups interactifs sur les points de départ et les clusters ---

/** Popup courant (point individuel OU cluster), pour le fermer/rafraîchir. */
let currentPopup: mapboxgl.Popup | null = null
/** Nature du popup courant : 'point' (un circuit) ou 'cluster' (liste de circuits). */
let currentPopup_kind: 'point' | 'cluster' | null = null
/** Id de la trace attachée au popup courant en mode 'point'. */
let currentPopup_traceId = ''
/** Ids des traces attachées au popup courant en mode 'cluster'. */
let currentPopup_traceIds: string[] = []
/**
 * Jeton d'invalidation des ouvertures de popup en cours.
 * Chaque fermeture (zoom, double-clic, fermeture manuelle) incrémente ce compteur ;
 * les ouvertures asynchrones capturent la valeur au démarrage et ne s'appliquent
 * que si elle n'a pas changé depuis. Cela évite qu'un popup ne se rouvre après
 * un zoom/double-clic (race condition sur getClusterLeaves).
 */
let popupEpoch = 0

/** Ferme et réinitialise le popup courant et invalide les ouvertures en cours. */
function closeCurrentPopup() {
  popupEpoch++ // invalide toute ouverture asynchrone en cours
  if (currentPopup) {
    currentPopup.remove()
    currentPopup = null
  }
  currentPopup_kind = null
  currentPopup_traceId = ''
  currentPopup_traceIds = []
}

/** Nettoyeur branché sur l'événement 'close' du popup. */
function onPopupClose() {
  popupEpoch++ // invalide les ouvertures en cours
  currentPopup = null
  currentPopup_kind = null
  currentPopup_traceId = ''
  currentPopup_traceIds = []
}

/**
 * Crée un bouton d'action du popup (icône MDI + titre) attaché à un id de trace.
 *
 * @param iconName nom d'icône MDI sans le préfixe (ex. 'mdi-pencil')
 * @param title    infobulle du bouton
 * @param onClick  rappel de clic (reçoit l'id de la trace)
 * @param traceId  identifiant de la trace concernée
 * @param color    couleur optionnelle de l'icône
 */
function createActionButton(
  iconName: string,
  title: string,
  onClick: (traceId: string) => void,
  traceId: string,
  color?: string,
): HTMLButtonElement {
  const btn = document.createElement('button')
  btn.title = title
  btn.style.cssText = [
    'border: none',
    'background: transparent',
    'cursor: pointer',
    'padding: 4px 6px',
    'font-size: 20px',
    'line-height: 1',
    'border-radius: 4px',
    'display: inline-flex',
    'align-items: center',
  ].join(';')
  if (color) btn.style.color = color

  const icon = document.createElement('i')
  icon.className = `mdi ${iconName}`
  btn.appendChild(icon)

  btn.addEventListener('mouseenter', () => { btn.style.background = 'rgba(0,0,0,0.08)' })
  btn.addEventListener('mouseleave', () => { btn.style.background = 'transparent' })
  btn.addEventListener('click', () => onClick(traceId))

  return btn
}

/**
 * Bascule le favori d'une trace via le store.
 * Le rafraîchissement du popup est pris en charge par le watch(traces).
 */
async function onPopupToggleFavorite(traceId: string) {
  const trace = tracesStore.traces.find(t => t.id === traceId)
  if (!trace) return
  await tracesStore.updateTrace(traceId, { favorite: !trace.favorite })
}

/**
 * Bascule l'affichage d'une trace via le store.
 * Le rafraîchissement du popup est pris en charge par le watch(traces).
 */
async function onPopupToggleDisplay(traceId: string) {
  const trace = tracesStore.traces.find(t => t.id === traceId)
  if (!trace) return
  await tracesStore.updateTrace(traceId, { is_displayed: !trace.is_displayed })
}

/** Placeholders pour les actions non câblées (éditer, groupes, météo). */
function onPopupEdit(_traceId: string) { /* TODO : édition de la trace */ }
function onPopupManageGroups(_traceId: string) { /* TODO : gestion des groupes */ }
function onPopupManageWeather(_traceId: string) { /* TODO : gestion de la météo */ }

/** Reconstruit le contenu du popup courant selon son mode (point ou cluster). */
function rebuildCurrentPopup() {
  if (!currentPopup || !map) return

  if (currentPopup_kind === 'point') {
    const trace = tracesStore.traces.find(t => t.id === currentPopup_traceId)
    if (!trace) return
    currentPopup.setDOMContent(buildPopupContent(trace))
  } else if (currentPopup_kind === 'cluster') {
    const traces = currentPopup_traceIds
      .map(id => tracesStore.traces.find(t => t.id === id))
      .filter((t): t is TraceMetadata => !!t)
    if (traces.length === 0) return
    currentPopup.setDOMContent(buildClusterPopupContent(traces))
  }
}

/**
 * Construit une ligne du popup : nom du circuit + icônes favori et affichage.
 * Utilisée par le popup de cluster (une ligne par circuit).
 */
function buildClusterRow(trace: TraceMetadata): HTMLElement {
  const row = document.createElement('div')
  row.style.cssText = 'display: flex; align-items: center; gap: 4px; padding: 2px 0;'

  const name = document.createElement('span')
  name.textContent = trace.name
  name.style.cssText = 'flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;'
  row.appendChild(name)

  const icons = document.createElement('div')
  icons.style.cssText = 'display: flex; align-items: center; gap: 0; flex-shrink: 0;'

  // Favori (orange si sélectionné)
  icons.appendChild(createActionButton(
    trace.favorite ? 'mdi-star' : 'mdi-star-outline',
    trace.favorite ? 'Retirer des favoris' : 'Ajouter aux favoris',
    onPopupToggleFavorite,
    trace.id,
    trace.favorite ? '#FB8C00' : undefined, // orange darken-1
  ))

  // Afficher (bleu si affiché)
  icons.appendChild(createActionButton(
    trace.is_displayed ? 'mdi-map-check' : 'mdi-map-check-outline',
    trace.is_displayed ? 'Masquer' : 'Afficher',
    onPopupToggleDisplay,
    trace.id,
    trace.is_displayed ? '#1976D2' : undefined, // blue
  ))

  row.appendChild(icons)
  return row
}

/**
 * Construit le contenu DOM du popup d'un cluster : une ligne par circuit,
 * chacune avec son nom et les icônes favori / affichage.
 */
function buildClusterPopupContent(traces: TraceMetadata[]): HTMLElement {
  const container = document.createElement('div')
  container.style.cssText = 'font-family: sans-serif; padding: 4px 0; min-width: 220px; max-width: 280px;'

  for (const trace of traces) {
    container.appendChild(buildClusterRow(trace))
  }
  return container
}

/**
 * Construit le contenu DOM du popup d'une trace : nom + 5 boutons d'action.
 * Les boutons Favoris et Afficher reflètent et basculent l'état de la trace ;
 * les boutons Éditer / Groupes / Météo sont des placeholders (icônes de Circuit.vue).
 */
function buildPopupContent(trace: TraceMetadata): HTMLElement {
  const container = document.createElement('div')
  container.style.cssText = 'font-family: sans-serif; padding: 4px 0; min-width: 180px;'

  const title = document.createElement('strong')
  title.textContent = trace.name
  title.style.cssText = 'display: block; margin-bottom: 6px;'
  container.appendChild(title)

  const actions = document.createElement('div')
  actions.style.cssText = 'display: flex; align-items: center; gap: 2px;'

  // Favori (orange si sélectionné, comme dans Circuit.vue)
  actions.appendChild(createActionButton(
    trace.favorite ? 'mdi-star' : 'mdi-star-outline',
    trace.favorite ? 'Retirer des favoris' : 'Ajouter aux favoris',
    onPopupToggleFavorite,
    trace.id,
    trace.favorite ? '#FB8C00' : undefined, // orange darken-1
  ))

  // Afficher (bleu si affiché)
  actions.appendChild(createActionButton(
    trace.is_displayed ? 'mdi-map-check' : 'mdi-map-check-outline',
    trace.is_displayed ? 'Masquer' : 'Afficher',
    onPopupToggleDisplay,
    trace.id,
    trace.is_displayed ? '#1976D2' : undefined, // blue
  ))

  actions.appendChild(createActionButton('mdi-pencil', 'Éditer', onPopupEdit, trace.id))
  actions.appendChild(createActionButton('mdi-account-group', 'Gérer les groupes…', onPopupManageGroups, trace.id))
  actions.appendChild(createActionButton('mdi-sun-thermometer-outline', 'Gérer la météo…', onPopupManageWeather, trace.id))

  container.appendChild(actions)
  return container
}

/** Clic sur un point individuel : popup interactif (nom + actions). */
function handlePointClick(e: mapboxgl.MapMouseEvent) {
  if (!map) return
  const feature = e.features?.[0]
  if (!feature || !feature.properties) return

  const coords = (feature.geometry as GeoJSON.Point).coordinates.slice() as [number, number]
  const traceId = feature.properties.id as string
  const trace = tracesStore.traces.find(t => t.id === traceId)
  if (!trace) return

  // Fermer un éventuel popup précédent.
  closeCurrentPopup()

  currentPopup_kind = 'point'
  currentPopup_traceId = trace.id
  const popup = new mapboxgl.Popup({ offset: 12, closeButton: true })
    .setLngLat(coords)
    .setDOMContent(buildPopupContent(trace))
    .addTo(map)

  // Nettoyage de la référence globale quand le popup est fermé.
  popup.on('close', onPopupClose)
  currentPopup = popup
}

// --- Gestion du curseur ---

/** Change le curseur en pointer au survol des clusters et points. */
function addCursorHandlers() {
  if (!map) return
  const canvas = map.getCanvas()

  // Clusters
  map.on('mouseenter', 'clusters', () => { canvas.style.cursor = 'pointer' })
  map.on('mouseleave', 'clusters', () => { canvas.style.cursor = '' })

  // Points individuels
  map.on('mouseenter', 'unclustered-point', () => { canvas.style.cursor = 'pointer' })
  map.on('mouseleave', 'unclustered-point', () => { canvas.style.cursor = '' })
}

// --- Synchronisation centre → store ---

/** Met à jour le centre dans le store (avec debounce). */
function onMapMoveEnd() {
  if (!map) return
  if (moveEndTimer) clearTimeout(moveEndTimer)
  moveEndTimer = setTimeout(() => {
    // Pendant un focus, ne pas mettre à jour le centre : sinon le tri des
    // circuits par distance (sortedTracesByDistance) se recalcule, ce qui
    // réordonne la liste CircuitsDrawer au lieu de la garder stable.
    if (tracesStore.focusedTraceId) return
    const c = map!.getCenter()
    tracesStore.updateMapCenter(c.lat, c.lng)
  }, MOVE_END_DEBOUNCE_MS)
}

// --- Initialisation ---

function initializeMap() {
  if (!mapContainer.value) return

  map = new mapboxgl.Map({
    container: mapContainer.value,
    style: 'mapbox://styles/mapbox/standard',
    zoom: 5.15,
    center: [2.0, 43.7], // [lon, lat] — France
  })

  map.on('load', () => {
    // Ordre d'ajout des couches respecte le z-order (cf. spec §8) :
    // 1. points de départ (clusters), 2. favoris, 3. traces affichées (au-dessus).
    addClusterLayers()
    addFavoritesLayer()
    addDisplayedTracesLayer()
    addFocusTracesLayer()
    addCursorHandlers()

    // Les couches personnalisées sont en place : on autorise les watchers
    // à appliquer setPaintProperty / setData sans erreur « layer does not exist ».
    mapReady = true

    // Recharger les géométries des favoris et traces affichées.
    refreshLineLayers()

    // Synchroniser le centre initial
    const c = map!.getCenter()
    tracesStore.updateMapCenter(c.lat, c.lng)
  })

  map.on('moveend', onMapMoveEnd)
  // Fermer le popup courant dès le début d'un zoom/dézoom (le cluster se
  // décompose et le popup ne correspondrait plus à son point d'ancrage).
  map.on('zoomstart', () => closeCurrentPopup())
  // Fermer le popup au double-clic (sur cluster ou point de départ) :
  // le double-clic déclenche aussi un zoom, mais dblclick garantit la
  // fermeture immédiate et invalide les ouvertures asynchrones en cours.
  map.on('dblclick', () => closeCurrentPopup())
  map.on('click', 'clusters', handleClusterClick)
  map.on('click', 'unclustered-point', handlePointClick)
  // Fermer le drawer de paramètres au clic sur la carte.
  map.on('click', () => {
    appStore.isSettingsDrawerOpen = false
  })
}

// --- Réactivité : mise à jour des données sans recréer la carte ---

/** true une fois que les couches personnalisées ont été ajoutées au style. */
let mapReady = false

// --- État du focus temporaire (clic Info dans Circuit.vue) ---

/** Vue (centre + zoom) sauvegardée avant un focus, pour restauration à la fermeture. */
let savedCenter: mapboxgl.LngLat | null = null
let savedZoom = 0
/** Compteur d'epoch pour annuler un focus en cours de chargement asynchrone. */
let focusEpoch = 0

// Bascule favori / affichage / import / suppression : rafraîchir les sources.
watch(
  () => tracesStore.traces,
  () => {
    if (!map || !mapReady) return
    // 1. Mettre à jour les points de départ (avec la propriété favorite).
    const source = map.getSource('traces') as mapboxgl.GeoJSONSource | undefined
    if (source) {
      source.setData(buildTracesGeoJSON())
    }
    // 2. Recharger les LineStrings favorites / affichées.
    refreshLineLayers()
    // 3. Rafraîchir le popup courant (si ouvert) pour rester cohérent avec
    //    l'état modifié depuis n'importe où (Circuit.vue, popup, etc.).
    rebuildCurrentPopup()
  },
  { deep: true },
)

// Focus temporaire sur un circuit (clic Info) : isoler la trace et la cadrer.
watch(
  () => tracesStore.focusedTraceId,
  async (newId) => {
    if (!map || !mapReady) return
    const epoch = ++focusEpoch
    const duration = getParam('Carte.Traces.dureeFlyTo') ?? 500

    if (newId) {
      // --- Ouverture du focus ---
      // 1. Sauvegarder la vue courante pour pouvoir la restaurer à la fermeture.
      savedCenter = map.getCenter()
      savedZoom = map.getZoom()
      // 2. Masquer les autres traces (favoris + traces affichées forcées).
      if (map.getLayer('favorites-line')) {
        map.setLayoutProperty('favorites-line', 'visibility', 'none')
      }
      if (map.getLayer('displayed-traces-line')) {
        map.setLayoutProperty('displayed-traces-line', 'visibility', 'none')
      }
      // 3. Charger la géométrie de la trace ciblée et l'afficher dans la couche dédiée.
      try {
        const geom = await tracesStore.getTraceGeometry(newId)
        if (epoch !== focusEpoch) return // un autre focus (ou fermeture) a pris le dessus
        const source = map.getSource('focus-traces') as mapboxgl.GeoJSONSource | undefined
        if (source) {
          source.setData({ type: 'FeatureCollection', features: [geom] })
        }
        if (map.getLayer('focus-traces-line')) {
          map.setLayoutProperty('focus-traces-line', 'visibility', 'visible')
        }
        // 4. Cadrer la carte sur la trace (fitBounds calcule centre + zoom optimaux).
        map.fitBounds(computeBounds(geom), { padding: 60, duration, essential: true })
      } catch (error) {
        console.error(`Géométrie introuvable pour le focus sur la trace ${newId} :`, error)
      }
    } else {
      // --- Fermeture du focus ---
      // 1. Masquer la couche dédiée.
      if (map.getLayer('focus-traces-line')) {
        map.setLayoutProperty('focus-traces-line', 'visibility', 'none')
      }
      // 2. Réafficher les favoris et traces affichées (leurs données sont toujours en source).
      if (map.getLayer('favorites-line')) {
        map.setLayoutProperty('favorites-line', 'visibility', 'visible')
      }
      if (map.getLayer('displayed-traces-line')) {
        map.setLayoutProperty('displayed-traces-line', 'visibility', 'visible')
      }
      // 3. Restaurer la vue sauvegardée.
      if (savedCenter) {
        map.flyTo({ center: savedCenter, zoom: savedZoom, duration, essential: true })
      }
    }
  },
)
watch(
  () => settingsStore.settings,
  () => {
    if (!map || !mapReady) return
    // Les couches attendues doivent exister avant tout setPaintProperty.
    if (!map.getLayer('clusters')
      || !map.getLayer('unclustered-point')
      || !map.getLayer('favorites-line')
      || !map.getLayer('displayed-traces-line')) {
      return
    }

    // Couleur des clusters (favori en priorité).
    map.setPaintProperty('clusters', 'circle-color', buildClusterColorExpression())

    // Couleur des points de départ non clusterisés (favori → couleur trace favorite).
    map.setPaintProperty('unclustered-point', 'circle-color', buildPointColorExpression())

    // Couche favoris : couleur + épaisseur.
    const couleurTrace = getColor('Carte.Favoris.couleurTrace', '#FFEB3BFF')
    const epaisseurFavori = getParam('Carte.Favoris.epaisseur') ?? 6
    map.setPaintProperty('favorites-line', 'line-color', couleurTrace)
    map.setPaintProperty('favorites-line', 'line-width', epaisseurFavori)

    // Couche traces affichées : épaisseur + dégradé.
    const epaisseurTrace = getParam('Carte.Traces.epaisseur') ?? 4
    map.setPaintProperty('displayed-traces-line', 'line-width', epaisseurTrace)
    map.setPaintProperty('displayed-traces-line', 'line-gradient', buildGradientExpression())

    // Couche focus (même représentation que les traces affichées).
    if (map.getLayer('focus-traces-line')) {
      map.setPaintProperty('focus-traces-line', 'line-width', epaisseurTrace)
      map.setPaintProperty('focus-traces-line', 'line-gradient', buildGradientExpression())
    }
  },
  { deep: true },
)

// Changement de thème (clair ↔ sombre) : appliquer la classe sombre et
// resynchroniser le canvas. On évite de binder isDarkMode dans le template
// (un re-render du template par Vue désynchronise le canvas Mapbox) ; on
// bascule la classe CSS à la main puis on force un resize au frame suivant.
watch(
  () => appStore.isDarkMode,
  (dark) => {
    if (!mapContainer.value) return
    mapContainer.value.classList.toggle('map-dark', dark)
    // Le changement de thème recalcule la mise en page Vuetify : attendre le
    // prochain frame pour que les dimensions soient stabilisées avant resize().
    requestAnimationFrame(() => map?.resize())
  },
)

// --- Cycle de vie ---

onMounted(async () => {
  // Appliquer la classe sombre dès le montage si le thème actif est sombre.
  if (mapContainer.value && appStore.isDarkMode) {
    mapContainer.value.classList.add('map-dark')
  }

  // Charger le token Mapbox depuis les paramètres
  let token = ''
  try {
    token = await settingsStore.getSettingValue('Systeme.Key.mapBox')
  } catch (error) {
    console.error('Failed to retrieve MapBox token from settings:', error)
  }

  mapboxgl.accessToken = token || ''

  // Initialiser la carte
  initializeMap()

  // Surveiller le redimensionnement du conteneur pour recalculer le canvas.
  // Un changement de thème recalcule la mise en page Vuetify (variables CSS),
  // ce qui modifie les dimensions du conteneur sans que Mapbox ne le détecte ;
  // map.resize() resynchronise le canvas avec la nouvelle taille.
  if (mapContainer.value) {
    resizeObserver = new ResizeObserver(() => {
      map?.resize()
    })
    resizeObserver.observe(mapContainer.value)
  }
})

onUnmounted(() => {
  mapReady = false
  if (moveEndTimer) clearTimeout(moveEndTimer)
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
  closeCurrentPopup()
  if (map) {
    map.remove()
    map = null
  }
})
</script>

<style>
/* make the map container fill its parent */
.map-container {
  width: 100%;
  height: 100%;
}

/*
 * Thème sombre des popups Mapbox.
 * Par défaut, .mapboxgl-popup-content a un fond blanc (#fff) codé en dur.
 * En mode sombre, on bascule le fond, le texte et la pointe (tip) du popup.
 * Les popups étant des enfants du conteneur de la carte, le sélecteur
 * .map-dark .mapboxgl-popup-* les cible uniquement quand le thème est sombre.
 */
.map-dark .mapboxgl-popup-content {
  background: #1e1e1e; /* grey-darken-4 (cohérent avec le thème Vuetify dark) */
  color: #ffffff;
}

/* Bouton de fermeture du popup : couleur claire en mode sombre */
.map-dark .mapboxgl-popup-close-button {
  color: #ffffff;
}

/* Pointe (tip) du popup : couleur du fond sombre */
.map-dark.mapboxgl-popup-anchor-top .mapboxgl-popup-tip,
.map-dark .mapboxgl-popup-anchor-top .mapboxgl-popup-tip {
  border-bottom-color: #1e1e1e;
}
.map-dark.mapboxgl-popup-anchor-bottom .mapboxgl-popup-tip,
.map-dark .mapboxgl-popup-anchor-bottom .mapboxgl-popup-tip {
  border-top-color: #1e1e1e;
}
.map-dark.mapboxgl-popup-anchor-left .mapboxgl-popup-tip,
.map-dark .mapboxgl-popup-anchor-left .mapboxgl-popup-tip {
  border-right-color: #1e1e1e;
}
.map-dark.mapboxgl-popup-anchor-right .mapboxgl-popup-tip,
.map-dark .mapboxgl-popup-anchor-right .mapboxgl-popup-tip {
  border-left-color: #1e1e1e;
}
</style>
