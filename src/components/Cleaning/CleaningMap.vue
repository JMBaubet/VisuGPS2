<template>
  <div class="cleaning-map-root">
    <div ref="mapContainer" class="cleaning-map-container"></div>
    <!-- Cadrage sur la trace complète (le cadrage par défaut suit la zone du cas) -->
    <v-btn
      class="fit-full-btn"
      variant="flat"
      size="small"
      prepend-icon="mdi-fit-to-page-outline"
      title="Cadrer sur la trace complète"
      @click="fitFullTrace"
    >
      Trace complète
    </v-btn>

    <!-- Sélecteur des points proches du curseur (points superposés) -->
    <div
      v-if="hoverCandidates.length > 0"
      class="hover-candidates"
      :style="{ left: hoverPos.x + 'px', top: hoverPos.y + 'px' }"
    >
      <div class="hover-candidates-title">Points proches :</div>
      <button
        v-for="c in hoverCandidates"
        :key="c.i"
        class="hover-candidate"
        :class="{ 'hover-candidate--start': c.i === cleaning.createStartIndex }"
        @click="onPickCandidate(c.i)"
      >
        #{{ c.label }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Carte Mapbox GL de la vue de nettoyage de trace.
 *
 * Affichage :
 * - la trace complète (ligne continue verte, contexte) ;
 * - le **segment courant** (zone du cas) surligné ;
 * - le **linestring corrigé** (ligne jaune à halo blanc) : le tracé réel après
 *   les suppressions, mis à jour en direct ;
 * - pour un aller-retour (`out_and_back`), les branches **aller** et **retour**
 *   décalées perpendiculairement (`line-offset`) et colorées différemment pour
 *   distinguer les passages superposés ;
 * - les points de la zone numérotés (index GPX), cliquables pour basculer leur
 *   suppression, avec anti-revouvrement des labels.
 *
 * Conventions reprises de EditionMap.vue / Accueil/Map.vue : token Mapbox
 * depuis `Systeme.Key.mapBox`, `ResizeObserver` pour le redimensionnement,
 * destruction de la carte au démontage.
 */
import { ref, watch, onMounted, onUnmounted } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import { useSettingsStore } from '../../stores/settings'
import { useCleaningStore } from '../../stores/cleaning'

const settingsStore = useSettingsStore()
const cleaning = useCleaningStore()

const mapContainer = ref<HTMLDivElement | null>(null)
let map: mapboxgl.Map | null = null
let resizeObserver: ResizeObserver | null = null
/** Index du point en cours de drag (déplacement), ou null. */
let dragPointIndex: number | null = null
/** true si la souris a bougé pendant le drag en cours. */
let dragMoved = false
/** true si le prochain clic doit être ignoré (suite à un vrai drag). */
let suppressNextClick = false

/** Rayon (px) de détection des points proches du curseur. */
const HOVER_RADIUS_PX = 14
/** Nombre maximal de candidats affichés. */
const HOVER_MAX_CANDIDATES = 8

/** Points proches du curseur (sélecteur de points superposés). */
const hoverCandidates = ref<{ i: number; label: string }[]>([])
/** Position du curseur (px) pour positionner le sélecteur. */
const hoverPos = ref({ x: 0, y: 0 })
/**
 * true quand le sélecteur est **verrouillé** (affiché suite à un clic ambigu) :
 * il reste affiché et cliquable même si la souris bouge ou quitte la carte,
 * jusqu'au choix d'un numéro ou à un clic ailleurs sur la carte.
 */
let candidatesLocked = false

// --- Identifiants de sources / couches ---

const FULL_SOURCE = 'clean-full'
const ZONE_SOURCE = 'clean-zone'
const BRANCH_A_SOURCE = 'clean-branch-a'
const BRANCH_B_SOURCE = 'clean-branch-b'
const CORRECTED_SOURCE = 'clean-corrected'
const CORRECTED_HALO_SOURCE = 'clean-corrected-halo'
const POINTS_SOURCE = 'clean-points'
const POINTS_LAYER = 'clean-points-layer'
const LABELS_LAYER = 'clean-labels-layer'
const CREATE_SOURCE = 'clean-create'

// --- Helpers GeoJSON ---

function lineFeature(coords: number[][]): GeoJSON.Feature {
  return {
    type: 'Feature',
    properties: {},
    geometry: { type: 'LineString', coordinates: coords },
  }
}

/** FeatureCollection vide (initiale, avant chargement des données). */
function emptyFC(): GeoJSON.FeatureCollection {
  return { type: 'FeatureCollection', features: [] }
}

/** Coordonnées `[lon, lat]` effectives d'un point de la trace (déplacement
 * appliqué s'il existe) pour le cas courant. */
function effectivePoint(i: number): { lat: number; lon: number } | null {
  const pts = cleaning.points
  if (i < 0 || i >= pts.length) return null
  return cleaning.isMoved(i) ?? pts[i]
}

function pointFeatures(): GeoJSON.FeatureCollection {
  const c = cleaning.currentCase
  const pts = cleaning.points
  if (!c || pts.length === 0) return { type: 'FeatureCollection', features: [] }

  const features: GeoJSON.Feature[] = []
  for (let i = c.start_index; i <= c.end_index && i < pts.length; i++) {
    const p = effectivePoint(i)
    if (!p) continue
    const status = cleaning.movePointIndex === i
      ? 'moving'
      : c.apex_indices.includes(i)
        ? 'apex'
        : cleaning.isDeleted(i)
          ? 'deleted'
          : 'normal'
    features.push({
      type: 'Feature',
      properties: {
        i,
        label: String(i + 1),
        status,
        // Décalage du label en em (mis à jour par `layoutPointLabels`).
        offset: [0, -0.9],
        // 1 = label placé sans chevauchement, 0 = masqué (reste le cercle).
        placed: 1,
      },
      geometry: { type: 'Point', coordinates: [p.lon, p.lat] },
    })
  }
  return { type: 'FeatureCollection', features }
}

/** Marqueur(s) de la création manuelle en cours (point de début désigné). */
function createMarkersFeatures(): GeoJSON.FeatureCollection {
  const idx = cleaning.createStartIndex
  const p = idx !== null ? effectivePoint(idx) : null
  if (!p) return { type: 'FeatureCollection', features: [] }
  return {
    type: 'FeatureCollection',
    features: [
      {
        type: 'Feature',
        properties: { role: 'start' },
        geometry: { type: 'Point', coordinates: [p.lon, p.lat] },
      },
    ],
  }
}

// --- Anti-revouvrement des labels (placement greedy) ---

/** Largeur/hauteur approx. d'un label à l'écran (px). */
const LABEL_W = 22
const LABEL_H = 12

/** Taille de police des labels (px) — 1 em = 1 caractère de hauteur. */
const LABEL_EM = 10

/** Priorité d'affichage : point en cours de déplacement, apex, supprimés, reste. */
function labelPriority(status: string): number {
  if (status === 'moving') return 0
  if (status === 'apex') return 0
  if (status === 'deleted') return 1
  return 2
}

/**
 * Candidats de décalage (px) pour un rayon donné. r=0 place le label au-dessus
 * du point ; les rayons suivants écartent le label dans 8 directions.
 */
function labelCandidates(radius: number): [number, number][] {
  if (radius === 0) return [[0, -14]]
  const d = radius * 12
  return [
    [0, -d],
    [d, 0],
    [0, d],
    [-d, 0],
    [d, -d],
    [d, d],
    [-d, d],
    [-d, -d],
  ]
}

function rectsCollide(
  a: { x: number; y: number; w: number; h: number },
  b: { x: number; y: number; w: number; h: number },
): boolean {
  return (
    a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y
  )
}

/**
 * Offsets des labels **mémorisés par cas** (clé `id:start:end` → index →
 * offset en px) : une fois calculés, les labels ne sont plus déplacés — ils
 * suivent leur point (position écran = position du point + offset fixe en
 * pixels), même pendant un pan/zoom ou un déplacement de point. Le clic reste
 * donc fiable. Chaque cas conserve ses propres offsets.
 */
const labelOffsetsCache = new Map<string, Map<number, [number, number]>>()

/**
 * Calcule les décalages des labels (anti-revouvrement greedy) en réutilisant
 * les offsets déjà mémorisés pour ce cas. Les labels dont l'offset est connu
 * restent **stables** ; seuls les labels sans offset (premier affichage du
 * cas) sont placés autour des boîtes déjà posées.
 */
function layoutPointLabels(features: GeoJSON.Feature[], offsets: Map<number, [number, number]>): void {
  if (!map) return
  const placed: { x: number; y: number; w: number; h: number }[] = []
  const pending: GeoJSON.Feature[] = []

  const ordered = [...features].sort((a, b) => {
    const pa = labelPriority(String(a.properties?.status ?? ''))
    const pb = labelPriority(String(b.properties?.status ?? ''))
    if (pa !== pb) return pa - pb
    return Number(a.properties?.i) - Number(b.properties?.i)
  })

  for (const f of ordered) {
    const geom = f.geometry as GeoJSON.Point | undefined
    const props = f.properties as Record<string, unknown>
    if (!geom || geom.type !== 'Point' || !props) continue

    const i = Number(props.i)
    const [lon, lat] = geom.coordinates as [number, number]
    const px = map.project([lon, lat])
    const label = String(props.label ?? '')
    const w = Math.max(LABEL_W, label.length * 7 + 4)
    const h = LABEL_H

    const cached = offsets.get(i)
    if (cached) {
      // Label déjà positionné : on garde l'offset, on place sa boîte pour que
      // les nouveaux labels ne le recouvrent pas.
      const box = { x: px.x + cached[0] - w / 2, y: px.y + cached[1] - h / 2, w, h }
      placed.push(box)
      props.offset = [cached[0] / LABEL_EM, cached[1] / LABEL_EM]
      props.placed = 1
      continue
    }
    pending.push(f)
  }

  for (const f of pending) {
    const geom = f.geometry as GeoJSON.Point
    const props = f.properties as Record<string, unknown>
    const i = Number(props.i)
    const [lon, lat] = geom.coordinates as [number, number]
    const px = map.project([lon, lat])
    const label = String(props.label ?? '')
    const w = Math.max(LABEL_W, label.length * 7 + 4)
    const h = LABEL_H

    let placedPos: [number, number] | null = null
    search: for (let r = 0; r < 6; r++) {
      for (const [dx, dy] of labelCandidates(r)) {
        const box = { x: px.x + dx - w / 2, y: px.y + dy - h / 2, w, h }
        if (!placed.some(p => rectsCollide(p, box))) {
          placed.push(box)
          placedPos = [dx, dy]
          break search
        }
      }
    }

    props.offset = placedPos ? [placedPos[0] / LABEL_EM, placedPos[1] / LABEL_EM] : [0, -0.9]
    props.placed = placedPos ? 1 : 0
    if (placedPos) offsets.set(i, placedPos)
  }
}

function renderPoints() {
  const fc = pointFeatures()
  // Anti-revouvrement : offsets mémorisés par cas (stables ensuite).
  const c = cleaning.currentCase
  const key = c ? `${c.id}:${c.start_index}:${c.end_index}` : ''
  let offsets = labelOffsetsCache.get(key)
  if (!offsets) {
    offsets = new Map()
    labelOffsetsCache.set(key, offsets)
  }
  layoutPointLabels(fc.features, offsets)
  setData(POINTS_SOURCE, fc)
}

function setData(sourceId: string, data: GeoJSON.Feature | GeoJSON.FeatureCollection) {
  const source = map?.getSource(sourceId) as mapboxgl.GeoJSONSource | undefined
  if (source) source.setData(data)
}

function setLayerVisibility(layerId: string, visible: boolean) {
  if (map?.getLayer(layerId)) {
    map.setLayoutProperty(layerId, 'visibility', visible ? 'visible' : 'none')
  }
}

function renderFullTrace() {
  const coords = cleaning.points.map(p => [p.lon, p.lat] as number[])
  setData(FULL_SOURCE, lineFeature(coords))
}

function renderZone() {
  const c = cleaning.currentCase
  const pts = cleaning.points
  if (!c || pts.length === 0) {
    setData(ZONE_SOURCE, lineFeature([]))
    setData(BRANCH_A_SOURCE, lineFeature([]))
    setData(BRANCH_B_SOURCE, lineFeature([]))
    return
  }
  const zone = pts
    .slice(c.start_index, c.end_index + 1)
    .map(p => [p.lon, p.lat] as number[])
  setData(ZONE_SOURCE, lineFeature(zone))

  // Branches aller / retour pour les aller-retours (décalage latéral visuel).
  const hasBranches = c.kind === 'out_and_back'
  if (hasBranches) {
    const apex = c.apex_indices[0] ?? Math.floor((c.start_index + c.end_index) / 2)
    const a = pts
      .slice(c.start_index, Math.min(apex, c.end_index) + 1)
      .map(p => [p.lon, p.lat] as number[])
    const b = pts
      .slice(Math.min(apex, c.end_index), c.end_index + 1)
      .map(p => [p.lon, p.lat] as number[])
    setData(BRANCH_A_SOURCE, lineFeature(a))
    setData(BRANCH_B_SOURCE, lineFeature(b))
    setLayerVisibility(BRANCH_A_SOURCE, true)
    setLayerVisibility(BRANCH_B_SOURCE, true)
  } else {
    setData(BRANCH_A_SOURCE, lineFeature([]))
    setData(BRANCH_B_SOURCE, lineFeature([]))
    setLayerVisibility(BRANCH_A_SOURCE, false)
    setLayerVisibility(BRANCH_B_SOURCE, false)
  }
}

/** Linestring du segment courant **après corrections** (points supprimés
 * retirés, points déplacés repositionnés) — le retour visuel de l'impact réel. */
function renderCorrected() {
  const coords = cleaning.correctedZoneCoords
  setData(CORRECTED_SOURCE, lineFeature(coords))
  setData(CORRECTED_HALO_SOURCE, lineFeature(coords))
}

/** Marqueurs du mode « création d'anomalie » (point de début désigné). */
function renderCreate() {
  setData(CREATE_SOURCE, createMarkersFeatures())
}

function renderAll() {
  if (!map) return
  renderFullTrace()
  renderZone()
  renderCorrected()
  renderPoints()
  renderCreate()
}

/**
 * Index du point de la **trace entière** le plus proche du clic — utilisé pour
 * le snap lors de la création manuelle d'un cas (début / fin de segment).
 */
function nearestIndexInTrace(lngLat: mapboxgl.LngLat): number {
  const pts = cleaning.points
  if (pts.length === 0) return -1
  let best = -1
  let bestDist = Infinity
  for (let i = 0; i < pts.length; i++) {
    const d = (pts[i].lon - lngLat.lng) ** 2 + (pts[i].lat - lngLat.lat) ** 2
    if (d < bestDist) {
      bestDist = d
      best = i
    }
  }
  return best
}

// --- Sélecteur des points proches du curseur (points superposés) ---

/**
 * Points de la trace situés dans le rayon (px) autour d'une position écran,
 * triés par distance. En mode création, la recherche porte sur **toute** la
 * trace ; sinon sur la zone du cas courant. Utilise les positions effectives
 * (déplacements appliqués).
 */
function computeNearbyPoints(px: { x: number; y: number }): { i: number; label: string }[] {
  if (!map) return []
  const pts = cleaning.points
  if (pts.length === 0) return []

  // Bbox en degrés autour de la position (conversion du rayon pixel).
  const r = HOVER_RADIUS_PX
  const tl = map.unproject([px.x - r, px.y - r])
  const br = map.unproject([px.x + r, px.y + r])
  const minLon = Math.min(tl.lng, br.lng)
  const maxLon = Math.max(tl.lng, br.lng)
  const minLat = Math.min(tl.lat, br.lat)
  const maxLat = Math.max(tl.lat, br.lat)

  // Plage d'index à parcourir.
  const c = cleaning.currentCase
  let start = 0
  let end = pts.length - 1
  if (!cleaning.createMode && c) {
    start = c.start_index
    end = Math.min(c.end_index, pts.length - 1)
  }

  const candidates: { i: number; label: string; dist: number }[] = []
  for (let i = start; i <= end; i++) {
    const p = effectivePoint(i)
    if (!p) continue
    if (p.lat < minLat || p.lat > maxLat || p.lon < minLon || p.lon > maxLon) continue
    const proj = map.project([p.lon, p.lat])
    const dist = Math.hypot(proj.x - px.x, proj.y - px.y)
    if (dist <= r) {
      candidates.push({ i, label: String(i + 1), dist })
    }
  }
  candidates.sort((a, b) => a.dist - b.dist)
  return candidates.slice(0, HOVER_MAX_CANDIDATES).map(({ i, label }) => ({ i, label }))
}

/** Affiche le sélecteur de candidats (au survol). */
function updateHoverCandidates(e: mapboxgl.MapMouseEvent) {
  if (!map || dragPointIndex !== null) return
  // Liste verrouillée par un clic ambigu : le survol ne la modifie pas.
  if (candidatesLocked) return
  const found = computeNearbyPoints(e.point)
  // La liste n'apparaît que pour **trancher une ambiguïté** (≥ 2 points
  // superposés) : avec un seul candidat, on clique directement sur le label.
  if (found.length >= 2) {
    hoverCandidates.value = found
    hoverPos.value = {
      x: Math.min(e.point.x + 14, Math.max(0, (map.getContainer().clientWidth || 0) - 170)),
      y: e.point.y + 14,
    }
  } else {
    hoverCandidates.value = []
  }
}

/** Sélection d'un point dans la liste des candidats. */
function onPickCandidate(index: number) {
  candidatesLocked = false
  hoverCandidates.value = []
  if (cleaning.createMode) {
    // Création manuelle : 1er clic = début, 2e clic = fin du segment.
    if (cleaning.createStartIndex === null) {
      cleaning.createStartIndex = index
      renderCreate()
    } else {
      cleaning.createManualCase(cleaning.createStartIndex, index)
    }
    return
  }
  // Sinon : bascule la suppression du point choisi.
  cleaning.toggleDeletePoint(index)
}

function fitZone() {
  const c = cleaning.currentCase
  const pts = cleaning.points
  if (!map || !c || pts.length === 0) return
  const bounds = new mapboxgl.LngLatBounds()
  for (let i = c.start_index; i <= c.end_index && i < pts.length; i++) {
    bounds.extend([pts[i].lon, pts[i].lat])
  }
  map.fitBounds(bounds, { padding: 80, maxZoom: 18, duration: 600 })
}

/** Cadre la carte sur la **trace complète** (contexte global). */
function fitFullTrace() {
  const pts = cleaning.points
  if (!map || pts.length === 0) return
  const bounds = new mapboxgl.LngLatBounds()
  for (const p of pts) bounds.extend([p.lon, p.lat])
  map.fitBounds(bounds, { padding: 50, duration: 600 })
}

// --- Initialisation de la carte ---

async function initializeMap() {
  if (!mapContainer.value) return
  let token = ''
  try {
    token = (await settingsStore.getSettingValue('Systeme.Key.mapBox')) || ''
  } catch {
    token = ''
  }
  mapboxgl.accessToken = token

  map = new mapboxgl.Map({
    container: mapContainer.value,
    style: 'mapbox://styles/mapbox/standard',
    zoom: 9,
    center: [2.5, 43.0],
    attributionControl: false,
  })
  map.addControl(new mapboxgl.AttributionControl({ compact: true }), 'top-left')
  map.addControl(new mapboxgl.NavigationControl({ showCompass: false }), 'top-right')

  map.on('load', () => {
    setupLayers()
    renderAll()
    fitZone()
  })

  // Clic : en mode « création d'anomalie », désigne le début puis la fin du
  // segment (snap au point de trace le plus proche) ; sinon, un clic sur un
  // point de la zone bascule sa suppression.
  //
  // Si **plusieurs points sont superposés** sous le curseur, on n'applique
  // aucune action : le sélecteur des N° d'ordre s'affiche et l'utilisateur
  // clique sur le numéro voulu (l'action se fait alors via `onPickCandidate`).
  map.on('click', (e) => {
    if (!map) return
    // Suite à un vrai drag (déplacement d'un point), on ignore le clic qui suit.
    if (suppressNextClick) {
      suppressNextClick = false
      return
    }

    candidatesLocked = false
    const nearby = computeNearbyPoints(e.point)
    if (nearby.length >= 2) {
      // Ambiguïté : verrouiller la liste et attendre le choix de l'utilisateur.
      candidatesLocked = true
      hoverCandidates.value = nearby
      hoverPos.value = {
        x: Math.min(e.point.x + 14, Math.max(0, (map.getContainer().clientWidth || 0) - 170)),
        y: e.point.y + 14,
      }
      return
    }
    hoverCandidates.value = []
    const snap = nearby.length === 1 ? nearby[0].i : -1

    if (cleaning.createMode) {
      // Snap au point de trace le plus proche (le seul candidat, ou le plus
      // proche de toute la trace si aucun n'est dans le rayon).
      const idx = snap >= 0 ? snap : nearestIndexInTrace(e.lngLat)
      if (idx < 0) return
      if (cleaning.createStartIndex === null) {
        cleaning.createStartIndex = idx
        renderCreate()
      } else {
        cleaning.createManualCase(cleaning.createStartIndex, idx)
      }
      return
    }

    if (snap >= 0) {
      cleaning.toggleDeletePoint(snap)
      return
    }

    // Aucun point dans le rayon : on cherche les éléments rendus sous le
    // curseur (label décalé par l'anti-revouvrement, point déplacé…).
    const hit = map.queryRenderedFeatures(e.point, {
      layers: [POINTS_LAYER, LABELS_LAYER],
    })
    if (hit.length > 0) {
      const i = Number(hit[0].properties?.i)
      if (Number.isInteger(i)) cleaning.toggleDeletePoint(i)
    }
  })

  // Déplacement **direct** des points : réservé aux cas manuels, tous les
  // points du segment sont déplaçables à la souris (cliquez-glissez). Un
  // simple clic sans déplacement reste une bascule de suppression (gérée par
  // le handler `click` ci-dessus).
  map.on('mousedown', (e) => {
    if (!map || cleaning.createMode) return
    const c = cleaning.currentCase
    if (!c || c.kind !== 'manual') return
    const hit = map.queryRenderedFeatures(e.point, { layers: [POINTS_LAYER] })
    const i = hit.length > 0 ? Number(hit[0].properties?.i) : -1
    // Attention : -1 est un entier valide pour `Number.isInteger` — il faut
    // aussi exclure les index négatifs (clic sur la carte vide).
    if (!Number.isInteger(i) || i < 0) return
    dragPointIndex = i
    dragMoved = false
    cleaning.startMovePoint(i)
    map.dragPan.disable()
    map.getCanvas().style.cursor = 'grabbing'
  })
  map.on('mousemove', (e) => {
    if (dragPointIndex === null) return
    dragMoved = true
    cleaning.setMovedPoint(dragPointIndex, e.lngLat.lat, e.lngLat.lng)
  })
  map.on('mouseup', () => {
    if (dragPointIndex === null) return
    dragPointIndex = null
    cleaning.stopMovePoint()
    // Un vrai drag (souris bougée) : on ignore le clic qui suit (pas de
    // bascule de suppression après un déplacement).
    suppressNextClick = dragMoved
    if (map) {
      map.dragPan.enable()
      map.getCanvas().style.cursor = ''
    }
  })

  // Curseur adaptatif : réticule en mode création, main sur les points des
  // cas manuels (déplaçables), pointeur sur un point cliquable. Met à jour le
  // sélecteur des points proches (superposés).
  map.on('mousemove', (e) => {
    if (!map || dragPointIndex !== null) return
    updateHoverCandidates(e)
    const hover = map.queryRenderedFeatures(e.point, {
      layers: [POINTS_LAYER, LABELS_LAYER],
    })
    if (cleaning.currentCase?.kind === 'manual' && hover.length > 0) {
      map.getCanvas().style.cursor = 'move'
    } else if (cleaning.createMode) {
      map.getCanvas().style.cursor = 'crosshair'
    } else {
      map.getCanvas().style.cursor = hover.length > 0 ? 'pointer' : ''
    }
  })

  // Sortie du curseur de la carte : masque le sélecteur, sauf s'il est
  // verrouillé (choix en cours après un clic ambigu).
  map.on('mouseout', () => {
    if (!candidatesLocked) hoverCandidates.value = []
  })

  resizeObserver = new ResizeObserver(() => {
    map?.resize()
    // Re-placer les labels après un redimensionnement (moveend n'est pas
    // garanti d'être émis après un resize).
    window.setTimeout(() => renderPoints(), 60)
  })
  resizeObserver.observe(mapContainer.value)

  // Anti-revouvrement : recalcule les décalages des labels à la fin de chaque
  // mouvement de caméra (pan / zoom) — les labels sont stables pendant le
  // mouvement, repositionnés au repos.
  map.on('moveend', () => renderPoints())
}

function setupLayers() {
  if (!map) return

  // 1. Trace complète (ligne continue verte, pour le contexte).
  if (!map.getSource(FULL_SOURCE)) {
    map.addSource(FULL_SOURCE, { type: 'geojson', data: emptyFC() })
  }
  if (!map.getLayer(FULL_SOURCE)) {
    map.addLayer({
      id: FULL_SOURCE,
      type: 'line',
      source: FULL_SOURCE,
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: {
        'line-color': '#4CAF50', // vert — trace complète bien visible
        'line-width': 5,
        'line-opacity': 0.85,
      },
    })
  }

  // 2. Zone du cas courant.
  if (!map.getSource(ZONE_SOURCE)) {
    map.addSource(ZONE_SOURCE, { type: 'geojson', data: emptyFC() })
  }
  if (!map.getLayer(ZONE_SOURCE)) {
    map.addLayer({
      id: ZONE_SOURCE,
      type: 'line',
      source: ZONE_SOURCE,
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: {
        'line-color': '#1565c0',
        'line-width': 6,
        'line-opacity': 0.85,
      },
    })
  }

  // 3. Branches aller / retour (décalage latéral pour distinguer les passages).
  const addBranchLayer = (id: string, color: string, offset: number) => {
    if (!map) return
    if (!map.getSource(id)) map.addSource(id, { type: 'geojson', data: emptyFC() })
    if (!map.getLayer(id)) {
      map.addLayer({
        id,
        type: 'line',
        source: id,
        layout: { 'line-cap': 'round', 'line-join': 'round' },
        paint: {
          'line-color': color,
          'line-width': 3,
          'line-offset': offset,
          'line-opacity': 0.9,
        },
      })
    }
  }
  addBranchLayer(BRANCH_A_SOURCE, '#2e7d32', -7)
  addBranchLayer(BRANCH_B_SOURCE, '#ef6c00', 7)

  // 3bis. Linestring corrigé (résultat des suppressions/insertions) : halo
  //       blanc en dessous, ligne jaune épaisse au-dessus de la zone.
  for (const [id, color, width, opacity] of [
    [CORRECTED_HALO_SOURCE, '#ffffff', 9, 0.8],
    [CORRECTED_SOURCE, '#FDD835', 4, 0.95],
  ] as const) {
    if (!map.getSource(id)) map.addSource(id, { type: 'geojson', data: emptyFC() })
    if (!map.getLayer(id)) {
      map.addLayer({
        id,
        type: 'line',
        source: id,
        layout: { 'line-cap': 'round', 'line-join': 'round' },
        paint: {
          'line-color': color,
          'line-width': width,
          'line-opacity': opacity,
        },
      })
    }
  }

  // 4. Points de la zone (cercles + numéros d'index GPX).
  if (!map.getSource(POINTS_SOURCE)) {
    map.addSource(POINTS_SOURCE, { type: 'geojson', data: pointFeatures() })
  }
  if (!map.getLayer(POINTS_LAYER)) {
    map.addLayer({
      id: POINTS_LAYER,
      type: 'circle',
      source: POINTS_SOURCE,
      paint: {
        'circle-radius': 5,
        'circle-stroke-width': 1,
        'circle-stroke-color': '#ffffff',
        'circle-color': [
          'match',
          ['get', 'status'],
          'apex',
          '#b71c1c',
          'deleted',
          '#f44336',
          'moving',
          '#00c853',
          '#1e88e5',
        ],
        'circle-opacity': ['case', ['==', ['get', 'status'], 'deleted'], 0.9, 1],
      },
    })
  }
  if (!map.getLayer(LABELS_LAYER)) {
    map.addLayer({
      id: LABELS_LAYER,
      type: 'symbol',
      source: POINTS_SOURCE,
      layout: {
        'text-field': ['get', 'label'],
        'text-size': 10,
        // Le chevauchement est contrôlé par `layoutPointLabels` (anti-revouvrement
        // custom) : `text-allow-overlap` doit rester à true pour que Mapbox
        // n'écrase pas nos décalages par son propre placement.
        'text-allow-overlap': true,
        'text-offset': ['get', 'offset'],
      },
      paint: {
        'text-color': [
          'match',
          ['get', 'status'],
          'apex',
          '#b71c1c',
          'deleted',
          '#f44336',
          'moving',
          '#00c853',
          '#1565c0',
        ],
        // Les points supprimés sont affichés en rouge bien visible ; seuls les
        // labels sans emplacement libre (anti-revouvrement) sont masqués.
        'text-opacity': ['case', ['==', ['get', 'placed'], 0], 0, 1],
        'text-halo-color': '#ffffff',
        'text-halo-width': 1.5,
      },
    })
  }

  // 6. Marqueur(s) de la création manuelle d'un cas (point de début désigné).
  if (!map.getSource(CREATE_SOURCE)) {
    map.addSource(CREATE_SOURCE, { type: 'geojson', data: emptyFC() })
  }
  if (!map.getLayer(CREATE_SOURCE)) {
    map.addLayer({
      id: CREATE_SOURCE,
      type: 'circle',
      source: CREATE_SOURCE,
      paint: {
        'circle-radius': 9,
        'circle-stroke-width': 2,
        'circle-stroke-color': '#ffffff',
        'circle-color': '#00c853',
        'circle-opacity': 0.95,
      },
    })
  }
}

// --- Réactivité ---

// Re-rendu à chaque changement du cas courant (navigation) ou des corrections.
watch(
  () => cleaning.currentCase?.id,
  () => {
    renderAll()
    fitZone()
  },
)

watch(
  () => cleaning.currentCase?.correction,
  () => {
    renderPoints()
    renderCorrected()
  },
  { deep: true },
)

// Re-rendu quand le point à déplacer change (coloration « moving »).
watch(
  () => cleaning.movePointIndex,
  () => renderPoints(),
)

// Re-rendu des marqueurs de création manuelle.
watch(
  () => cleaning.createStartIndex,
  () => renderCreate(),
)

// Rendu quand l'état est chargé (carte montée avant le chargement du store).
watch(
  () => cleaning.state,
  () => {
    if (cleaning.state) {
      renderAll()
      fitZone()
    }
  },
)

onMounted(() => {
  initializeMap()
})

onUnmounted(() => {
  resizeObserver?.disconnect()
  map?.remove()
  map = null
})
</script>

<style scoped>
.cleaning-map-root {
  position: absolute;
  inset: 0;
}

.cleaning-map-container {
  position: absolute;
  inset: 0;
}

/* Bouton flottant de cadrage global, en bas à droite (au-dessus de la carte). */
.fit-full-btn {
  position: absolute;
  right: 12px;
  bottom: 12px;
  z-index: 2;
  background: rgba(255, 255, 255, 0.92);
  color: rgba(0, 0, 0, 0.87);
}

/* Sélecteur des points proches du curseur (points superposés). */
.hover-candidates {
  position: absolute;
  z-index: 3;
  min-width: 120px;
  max-width: 200px;
  background: rgba(255, 255, 255, 0.96);
  border: 0.5px solid rgba(0, 0, 0, 0.2);
  border-radius: 6px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  padding: 4px;
  pointer-events: auto;
}

.hover-candidates-title {
  font-size: 11px;
  color: rgba(0, 0, 0, 0.6);
  padding: 2px 6px 4px;
  white-space: nowrap;
}

.hover-candidate {
  display: block;
  width: 100%;
  text-align: left;
  font-size: 12px;
  font-weight: 600;
  color: #1565c0;
  background: transparent;
  border: none;
  border-radius: 4px;
  padding: 3px 8px;
  cursor: pointer;
}

.hover-candidate:hover {
  background: rgba(21, 101, 192, 0.12);
}

.hover-candidate--start {
  color: #00c853;
}
</style>
