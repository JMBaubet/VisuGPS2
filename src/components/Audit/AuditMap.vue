<script setup lang="ts">
/**
 * Troisième instance Mapbox GL de l'application, dédiée à l'audit GPX.
 *
 * Elle porte le rendu de l'anomalie (IHM §3.4 à §3.7) :
 * - **trace de travail** et marqueurs Départ/Arrivée ;
 * - **traits et points des anomalies** dans deux couches data-driven (un layer
 *   unique porte des features hétérogènes, chacune décrivant sa couleur et sa
 *   taille) ;
 * - **croix Kåsa** (centre du cercle ajusté) des boucles giratoires, en
 *   marqueurs HTML ;
 * - **étiquettes draguables** des points de l'anomalie sélectionnée, reliées à
 *   leur point par un trait de liaison mis à jour à chaque mouvement de carte.
 *
 * Cinq **registres de couches** remplacent les `layerGroup` de la référence. Un
 * seul popup est actif à la fois : les features portent leur HTML dans
 * `properties.popupHtml`, aucune n'a de popup en propre.
 *
 * Le placement des étiquettes et les ancres de routage sont calculés côté Rust
 * (`audit_map_overlay`) : le composant ne fait que créer les marqueurs DOM aux
 * positions reçues.
 */
import { ref, shallowRef, computed, onMounted, onUnmounted, watch } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import {
  useAuditStore,
  type DeletePreview,
  type Finding,
  type LabelItem,
  type PreviewCursor,
} from '../../stores/audit'
import { useSettingsStore } from '../../stores/settings'
import {
  AUDIT_COLORS,
  AUDIT_MAP_CENTER,
  AUDIT_MAP_STYLE,
  AUDIT_MAP_ZOOM,
  LAYER_IDS,
  attachRegistries,
  createRegistries,
  detachRegistries,
  emptyFC,
  initAuditLayers,
  publishLabelLeaders,
} from './auditMapLayers'
import { buildFindingRender } from './auditMapFeatures'

const auditStore = useAuditStore()
const settingsStore = useSettingsStore()

const mapContainer = ref<HTMLDivElement | null>(null)
const map = shallowRef<mapboxgl.Map | null>(null)
const mapReady = ref(false)
const hasMapboxKey = ref(false)

/** File d'attente des rendus demandés avant que la carte soit prête. */
const readyQueue: Array<() => void> = []
/** Les cinq registres fonctionnels (trace, appliqué, aperçu, anomalies, suppression). */
const regs = createRegistries()

let currentPopup: mapboxgl.Popup | null = null
let resizeObserver: ResizeObserver | null = null

/** Étiquette affichée : marqueur DOM, offset courant et trait de liaison. */
interface LabelMarker {
  marker: mapboxgl.Marker
  el: HTMLElement
  span: HTMLElement
  /** Indice du point dans la trace de travail (masquage en aperçu AR). */
  index: number
  lngLat: [number, number]
  dx: number
  dy: number
  /** Offset de référence, restauré au double-clic. */
  baseDx: number
  baseDy: number
  color: string
  lineRef: GeoJSON.Feature | null
}
let labelMarkers: LabelMarker[] = []
let labelLeaderFeatures: GeoJSON.Feature[] = []
/** Croix Kåsa des boucles (marqueurs HTML). */
let crossMarkers: mapboxgl.Marker[] = []
/** Marqueurs des curseurs de sliders (aperçu de suppression). */
let sliderMarkers: mapboxgl.Marker[] = []
/** Marqueurs des curseurs de la vue de routage (jaune / orange). */
let routeSliderMarkers: mapboxgl.Marker[] = []
/**
 * Features brutes des points, par anomalie. L'aperçu de suppression en dérive
 * des copies restylées : la sortie de vue restaure l'état sans re-détection.
 */
const basePointsByFinding = new Map<string, GeoJSON.Feature[]>()
/** Coordonnées couvertes par le rendu de chaque anomalie (centrage). */
const boundsListByFinding = new Map<string, [number, number][]>()

/** Exécute `fn` dès que la carte est prête, sinon met en file d'attente. */
function whenMapReady(fn: () => void): void {
  if (mapReady.value && map.value) fn()
  else readyQueue.push(fn)
}

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

// ─── Cycle de vie de la carte ─────────────────────────────────────────

async function initializeMap(token: string): Promise<void> {
  if (!mapContainer.value) return

  mapboxgl.accessToken = token
  const instance = new mapboxgl.Map({
    container: mapContainer.value,
    style: AUDIT_MAP_STYLE,
    center: AUDIT_MAP_CENTER,
    zoom: AUDIT_MAP_ZOOM,
  })
  map.value = instance

  instance.addControl(
    new mapboxgl.NavigationControl({ showCompass: false }),
    'bottom-right',
  )

  instance.on('error', (e) => {
    const message = e?.error?.message ?? ''
    if (/access token|401|403|token/i.test(message)) {
      console.warn('[Audit] clé Mapbox refusée :', message)
    } else if (message) {
      console.warn('[Mapbox]', message)
    }
  })

  instance.on('load', () => {
    mapReady.value = true
    attachRegistries(regs, instance)
    initAuditLayers(instance, regs, showPopupAt)

    const queue = readyQueue.splice(0)
    for (const fn of queue) {
      try {
        fn()
      } catch (err) {
        console.error(err)
      }
    }
    replayCurrentState()

    instance.on('move', updateLabelLines)
    instance.on('zoom', updateLabelLines)
    instance.on('resize', updateLabelLines)
  })
}

/** Rejoue l'état courant après (re)chargement de la carte. */
function replayCurrentState(): void {
  if (!map.value || !mapReady.value) return
  if (auditStore.working.length === 0) return
  renderBaseTrace(true)
  renderAnalysis()
  renderLabels()
  renderRoutePreview()
  renderRouteSliders()
}

function destroyMap(): void {
  if (currentPopup) {
    currentPopup.remove()
    currentPopup = null
  }
  for (const it of labelMarkers) it.marker.remove()
  labelMarkers = []
  labelLeaderFeatures = []
  for (const m of crossMarkers) m.remove()
  crossMarkers = []
  clearSliderMarkers()
  clearRouteSliderMarkers()
  detachRegistries(regs)
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

// ─── Trace de base ────────────────────────────────────────────────────

/** Collection d'une polyligne avec sa couleur, son épaisseur et son opacité. */
function fcLine(
  coords: [number, number][],
  color: string,
  width: number,
  opacity: number,
): GeoJSON.FeatureCollection {
  if (coords.length < 2) return emptyFC()
  return {
    type: 'FeatureCollection',
    features: [
      {
        type: 'Feature',
        properties: { color, width, opacity },
        geometry: { type: 'LineString', coordinates: coords },
      },
    ],
  }
}

function renderBaseTrace(fit = false): void {
  whenMapReady(() => {
    const points = auditStore.working
    if (points.length < 2) {
      regs.trace.update(LAYER_IDS.trace, emptyFC())
      regs.trace.update(LAYER_IDS.traceEnds, emptyFC())
      return
    }

    const coords: [number, number][] = points.map((p) => [p.lon, p.lat])
    regs.trace.update(
      LAYER_IDS.trace,
      fcLine(coords, AUDIT_COLORS.trace, 3, 0.95),
    )

    const start = coords[0]
    const end = coords[coords.length - 1]
    regs.trace.update(LAYER_IDS.traceEnds, {
      type: 'FeatureCollection',
      features: [
        {
          type: 'Feature',
          properties: {
            kind: 'start',
            popupHtml: '<div><strong>Départ</strong></div>',
          },
          geometry: { type: 'Point', coordinates: start },
        },
        {
          type: 'Feature',
          properties: {
            kind: 'end',
            popupHtml: '<div><strong>Arrivée</strong></div>',
          },
          geometry: { type: 'Point', coordinates: end },
        },
      ],
    })

    if (fit && map.value) {
      const b = new mapboxgl.LngLatBounds(coords[0], coords[0])
      for (const c of coords) b.extend(c)
      map.value.fitBounds(b, { padding: 40, duration: 0 })
    }
  })
}

// ─── Anomalies ────────────────────────────────────────────────────────

/** Retire les croix Kåsa et vide les couches d'anomalies. */
function clearAnalysis(): void {
  for (const m of crossMarkers) m.remove()
  crossMarkers = []
  clearSliderMarkers()
  if (map.value) {
    regs.anomaly.update(LAYER_IDS.anomalyLines, emptyFC())
    regs.anomaly.update(LAYER_IDS.anomalyPoints, emptyFC())
  }
  regs.del.update(LAYER_IDS.delJoin, emptyFC())
  regs.del.update(LAYER_IDS.delRed, emptyFC())
  if (currentPopup) {
    currentPopup.remove()
    currentPopup = null
  }
}

/** Crée la croix Kåsa d'une boucle (centre du cercle ajusté). */
function createKasaCross(finding: Finding, center: { lat: number; lon: number }, radiusM: number | null): void {
  if (!map.value) return
  const el = document.createElement('div')
  el.textContent = '✕'
  el.style.font = '700 16px/1 monospace'
  el.style.color = AUDIT_COLORS.kasasCross
  el.style.textShadow = '0 0 3px #000, 0 1px 2px #000'
  el.style.whiteSpace = 'nowrap'
  el.style.userSelect = 'none'
  el.style.cursor = 'pointer'

  const marker = new mapboxgl.Marker({ element: el, anchor: 'center' })
    .setLngLat([center.lon, center.lat])
    .addTo(map.value)
  marker.setPopup(
    new mapboxgl.Popup({ offset: 10 }).setHTML(
      `<div><strong>${finding.label}</strong></div>` +
        `<div>Centre de l'anneau (ajustement de cercle)</div>` +
        `<div>rayon médian ≈ <strong>${
          radiusM !== null ? radiusM.toFixed(0) : '?'
        } m</strong></div>` +
        `<div>lat/lon : <strong>${center.lat.toFixed(5)}, ${center.lon.toFixed(
          5,
        )}</strong></div>`,
    ),
  )
  crossMarkers.push(marker)
}

function renderAnalysis(): void {
  whenMapReady(() => {
    clearAnalysis()
    const points = auditStore.working
    if (points.length === 0) return

    const lineFeatures: GeoJSON.Feature[] = []
    const appliedFeatures: GeoJSON.Feature[] = []
    boundsListByFinding.clear()
    basePointsByFinding.clear()

    for (const finding of auditStore.findings) {
      const overlay = auditStore.overlayOf(finding.id)
      const render = buildFindingRender(finding, points, overlay)

      lineFeatures.push(...render.lines)
      basePointsByFinding.set(finding.id, render.points)
      boundsListByFinding.set(finding.id, render.bounds)

      // Tracé appliqué : les routages des anomalies corrigées sont **cumulés**
      // (IHM §5.1 — chaque anomalie routée porte son tracé). La référence
      // écrasait la source à chaque passage, ce qui faisait disparaître le
      // tracé d'un routage dès qu'une suppression corrigée suivait dans la
      // liste : l'ordre des anomalies décidait de l'affichage.
      appliedFeatures.push(...render.applied.features)

      const anchors = overlay?.anchors
      if (
        finding.kind === 'rp' &&
        finding.status !== 'corrected' &&
        anchors?.center
      ) {
        createKasaCross(finding, anchors.center, anchors.radiusM)
      }
    }

    regs.anomaly.update(LAYER_IDS.anomalyLines, {
      type: 'FeatureCollection',
      features: lineFeatures,
    })
    regs.applied.update(LAYER_IDS.applied, {
      type: 'FeatureCollection',
      features: appliedFeatures,
    })
    publishAnomalyPoints()
    // L'aperçu en cours est rejoué : un re-rendu ne doit pas l'effacer.
    renderDeletePreview()
  })
}

/**
 * Publie la couche des points d'anomalies.
 *
 * En aperçu de suppression, les points de l'emprise réglée sont restylés — bleu
 * « sera conservé », gris clair « sera supprimé » — et les ancres de routage
 * d'une boucle sont **destylisées** (IHM §8.2, §9.2). Les features brutes ne
 * sont jamais mutées : sortir de l'aperçu restaure l'état par simple recalcul.
 */
function publishAnomalyPoints(): void {
  const preview = auditStore.deletePreview
  const finding = auditStore.selectedFinding
  const selectedId = auditStore.selectedFindingId
  const features: GeoJSON.Feature[] = []

  for (const [id, base] of basePointsByFinding) {
    if (!preview || !finding || id !== selectedId) {
      features.push(...base)
      continue
    }
    for (const feature of base) {
      features.push(applyPreviewStyle(feature, preview, finding))
    }
  }

  regs.anomaly.update(LAYER_IDS.anomalyPoints, {
    type: 'FeatureCollection',
    features,
  })
}

/** Dérive d'une feature de point sa version « en aperçu » (copie). */
function applyPreviewStyle(
  feature: GeoJSON.Feature,
  preview: DeletePreview,
  finding: Finding,
): GeoJSON.Feature {
  const index = feature.properties?._idx as number | undefined
  if (index === undefined) return feature
  const style = preview.points.find((p) => p.index === index)
  if (!style) return feature

  const properties: Record<string, unknown> = {
    ...feature.properties,
    fill: style.kept ? AUDIT_COLORS.delKeep : AUDIT_COLORS.delDrop,
  }
  if (style.isAnchor) {
    // Pendant la vue, les ancres perdent leur style : le tableau visuel devient
    // exactement gris = supprimé, bleu = conservé, vert = sain.
    properties.radius = 2.5
    properties.strokeWidth = 1
    properties.popupHtml =
      `<div><strong>${finding.label}</strong></div>` +
      `<div>pt <strong>${index + 1}</strong> · ${
        style.kept ? 'conservé' : 'à supprimer'
      }</div>`
  }
  return { ...feature, properties }
}

/** Retire les marqueurs des curseurs de sliders. */
function clearSliderMarkers(): void {
  for (const m of sliderMarkers) m.remove()
  sliderMarkers = []
}

/** Pose un curseur de la vue de suppression (marqueur bleu). */
function createSliderMarker(cursor: PreviewCursor): void {
  const marker = circleMarker(
    cursor.lat,
    cursor.lon,
    AUDIT_COLORS.delKeep,
    `${cursor.title} · pt ${cursor.index + 1}`,
  )
  if (marker) sliderMarkers.push(marker)
}

/** Retire les marqueurs des curseurs de la vue de routage. */
function clearRouteSliderMarkers(): void {
  for (const m of routeSliderMarkers) m.remove()
  routeSliderMarkers = []
}

/** Marqueur circulaire coloré (curseur de slider sur la carte). */
function circleMarker(
  lat: number,
  lon: number,
  color: string,
  title: string,
): mapboxgl.Marker | null {
  const m = map.value
  if (!m) return null
  const el = document.createElement('div')
  el.style.width = '14px'
  el.style.height = '14px'
  el.style.borderRadius = '50%'
  el.style.background = color
  el.style.border = '2px solid #ffffff'
  el.style.boxShadow = '0 0 0 1px rgba(0,0,0,.35), 0 2px 6px rgba(0,0,0,.4)'
  el.style.pointerEvents = 'auto'
  el.title = title
  return new mapboxgl.Marker({ element: el, anchor: 'center' })
    .setLngLat([lon, lat])
    .addTo(m)
}

/**
 * Pose les curseurs Début/Fin de la vue de routage (IHM §6).
 *
 * Jaune pour l'ancre amont, orange pour l'ancre aval — les deux points
 * **mobiles** de la zone remplacée par le tracé routier.
 */
function renderRouteSliders(): void {
  clearRouteSliderMarkers()
  const range = auditStore.routeRange
  if (!range) return

  const specs: Array<[number, string, string]> = [
    [range[0], AUDIT_COLORS.selA, 'Début'],
    [range[1], AUDIT_COLORS.selB, 'Fin'],
  ]
  for (const [index, color, label] of specs) {
    const p = auditStore.working[index]
    if (!p) continue
    const marker = circleMarker(p.lat, p.lon, color, `${label} · pt ${index + 1}`)
    if (marker) routeSliderMarkers.push(marker)
  }
}

/** Vide les quatre couches d'aperçu de routage. */
function clearRoutePreviewLayers(): void {
  for (const id of [
    LAYER_IDS.previewCar,
    LAYER_IDS.previewCarPts,
    LAYER_IDS.previewBike,
    LAYER_IDS.previewBikePts,
  ]) {
    regs.preview.update(id, emptyFC())
  }
}

/**
 * Publie l'aperçu de routage (IHM §7.5).
 *
 * Quand les deux tracés sont identiques, un seul tracé jaune est dessiné
 * (largeur 9) ; sinon la voiture (largeur 10) et le vélo (largeur 6) sont
 * superposés, et la carte cadre l'union des deux.
 */
function renderRoutePreview(): void {
  whenMapReady(() => {
    const preview = auditStore.routePreview
    if (!preview) {
      clearRoutePreviewLayers()
      return
    }

    let bounds: mapboxgl.LngLatBounds | null = null
    const extend = (coords: [number, number][]) => {
      if (coords.length === 0) return
      if (!bounds) bounds = new mapboxgl.LngLatBounds(coords[0], coords[0])
      for (const c of coords) bounds?.extend(c)
    }
    const putLine = (
      id: string,
      route: { coords: [number, number][] } | null,
      color: string,
      width: number,
    ) => {
      const coords = route?.coords ?? []
      regs.preview.update(id, fcLine(coords, color, width, 0.9))
      extend(coords)
    }
    const putPoints = (
      id: string,
      route: { coords: [number, number][] } | null,
      color: string,
      radius: number,
    ) => {
      // Seuls les points intermédiaires sont marqués (les extrémités sont les
      // ancres, déjà matérialisées par les curseurs).
      const inner = (route?.coords ?? []).slice(1, -1)
      regs.preview.update(id, fcPoints(inner, color, radius))
    }

    const asCoords = (route: { coords: Array<{ lat: number; lon: number }> } | null) =>
      route
        ? {
            coords: route.coords.map(
              (p) => [p.lon, p.lat] as [number, number],
            ),
          }
        : null

    const car = asCoords(preview.car)
    const bike = asCoords(preview.bike)

    if (preview.identical) {
      putLine(LAYER_IDS.previewCar, car, AUDIT_COLORS.routeCar, 9)
      putPoints(LAYER_IDS.previewCarPts, car, AUDIT_COLORS.routeCar, 4)
      putLine(LAYER_IDS.previewBike, null, AUDIT_COLORS.routeBike, 6)
      putPoints(LAYER_IDS.previewBikePts, null, AUDIT_COLORS.routeBike, 3)
    } else {
      putLine(LAYER_IDS.previewCar, car, AUDIT_COLORS.routeCar, 10)
      putPoints(LAYER_IDS.previewCarPts, car, AUDIT_COLORS.routeCar, 4)
      putLine(LAYER_IDS.previewBike, bike, AUDIT_COLORS.routeBike, 6)
      putPoints(LAYER_IDS.previewBikePts, bike, AUDIT_COLORS.routeBike, 3)
    }

    if (bounds && map.value) {
      map.value.fitBounds(bounds, { padding: 60, maxZoom: 18, duration: 500 })
    }
  })
}

/**
 * Collection de points intermédiaires d'un aperçu de routage.
 */
function fcPoints(
  coords: [number, number][],
  color: string,
  radius: number,
): GeoJSON.FeatureCollection {
  return {
    type: 'FeatureCollection',
    features: coords.map((c) => ({
      type: 'Feature' as const,
      properties: {
        radius,
        fill: color,
        stroke: '#ffffff',
        strokeWidth: 1.2,
      },
      geometry: { type: 'Point' as const, coordinates: c },
    })),
  }
}

/**
 * Publie l'aperçu de suppression : chemin bleu des points conservés, trait rouge
 * de la plage supprimée (RP) et curseurs de sliders (IHM §8.2, §9.2).
 */
function renderDeletePreview(): void {
  whenMapReady(() => {
    clearSliderMarkers()
    const preview = auditStore.deletePreview
    if (!preview) {
      regs.del.update(LAYER_IDS.delJoin, emptyFC())
      regs.del.update(LAYER_IDS.delRed, emptyFC())
      return
    }

    const join = preview.join.map((p) => [p.lon, p.lat] as [number, number])
    const red = preview.red.map((p) => [p.lon, p.lat] as [number, number])
    regs.del.update(
      LAYER_IDS.delJoin,
      fcLine(join, AUDIT_COLORS.delJoin, 4, 0.95),
    )
    regs.del.update(LAYER_IDS.delRed, fcLine(red, AUDIT_COLORS.warn, 5, 0.95))

    for (const cursor of preview.cursors) createSliderMarker(cursor)
  })
}

// ─── Étiquettes ───────────────────────────────────────────────────────

/** Couleur du trait de liaison selon la classe d'étiquette (IHM §5.4). */
function leaderColor(cls: string): string {
  switch (cls) {
    case 'pk':
      return AUDIT_COLORS.warn
    case 'ctx':
      return AUDIT_COLORS.ctx
    case 'del':
      return AUDIT_COLORS.orig
    case 'keep':
      return AUDIT_COLORS.delKeep
    case 'fpl':
      return AUDIT_COLORS.fp
    default:
      return AUDIT_COLORS.labelLeader
  }
}

/**
 * Applique le style d'une étiquette selon sa classe (IHM §5.4).
 *
 * Les styles sont posés en ligne, depuis la table chromatique : la vue n'a pas
 * de feuille de style propre pour ces marqueurs.
 */
function applyLabelStyle(span: HTMLElement, cls: string): void {
  span.style.display = 'inline-block'
  span.style.whiteSpace = 'nowrap'
  span.style.background = '#10131a'
  span.style.border = '1px solid #323a46'
  span.style.color = '#e6e9ee'
  span.style.font = '600 10px/1.6 monospace'
  span.style.padding = '0 5px'
  span.style.borderRadius = '4px'
  span.style.boxShadow = '0 2px 8px rgba(0,0,0,.45)'
  span.style.cursor = 'grab'
  span.style.touchAction = 'none'
  span.style.userSelect = 'none'

  switch (cls) {
    case 'pk':
      span.style.borderColor = AUDIT_COLORS.warn
      span.style.color = '#ffb3ad'
      span.style.font = '600 11px/1.6 monospace'
      break
    case 'ctx':
      span.style.borderColor = AUDIT_COLORS.ctx
      span.style.color = '#7fe0b4'
      break
    case 'del':
      span.style.background = '#3a414d'
      span.style.borderColor = '#565f6d'
      span.style.color = '#d6dbe3'
      break
    case 'fpl':
      span.style.background = '#17304f'
      span.style.borderColor = '#3b5e8f'
      span.style.color = '#a8ccff'
      break
    case 'keep':
      span.style.borderColor = AUDIT_COLORS.delKeep
      span.style.color = '#cfe6ff'
      break
    default:
      break
  }
}

/** Positionne l'étiquette à son point, décalée de son offset courant. */
function applyLabelOffset(it: LabelMarker): void {
  it.span.style.transform =
    `translate(calc(-50% + ${it.dx}px), calc(-50% + ${it.dy}px))`
}

/** Recalcule le trait de liaison d'une étiquette (projection écran). */
function updateLabelLine(it: LabelMarker): void {
  const m = map.value
  if (!m || !it.lineRef) return
  const p0 = m.project(it.lngLat)
  const end = m.unproject(
    new mapboxgl.Point(p0.x + it.dx, p0.y + it.dy),
  )
  const geometry = it.lineRef.geometry as GeoJSON.LineString
  geometry.coordinates = [it.lngLat, [end.lng, end.lat]]
}

/** Recalcule tous les traits de liaison puis republie la source. */
function updateLabelLines(): void {
  if (!mapReady.value) return
  for (const it of labelMarkers) updateLabelLine(it)
  publishLabelLeaders(map.value, labelLeaderFeatures)
}

/** Rend une étiquette déplaçable ; le double-clic restaure son offset. */
function makeLabelDraggable(it: LabelMarker): void {
  let pointerId: number | null = null
  let startX = 0
  let startY = 0
  let dx0 = 0
  let dy0 = 0

  it.el.addEventListener('pointerdown', (e: PointerEvent) => {
    if (e.pointerType === 'mouse' && e.button !== 0) return
    e.preventDefault()
    e.stopPropagation()
    pointerId = e.pointerId
    try {
      it.el.setPointerCapture(pointerId)
    } catch {
      /* capture best-effort */
    }
    startX = e.clientX
    startY = e.clientY
    dx0 = it.dx
    dy0 = it.dy
    it.el.style.zIndex = '2000'
    it.span.style.cursor = 'grabbing'
    // Le déplacement de carte est suspendu pendant le drag de l'étiquette.
    map.value?.dragPan.disable()
  })

  it.el.addEventListener('pointermove', (e: PointerEvent) => {
    if (pointerId === null || e.pointerId !== pointerId) return
    e.preventDefault()
    it.dx = dx0 + (e.clientX - startX)
    it.dy = dy0 + (e.clientY - startY)
    applyLabelOffset(it)
    updateLabelLine(it)
    publishLabelLeaders(map.value, labelLeaderFeatures)
  })

  const end = () => {
    if (pointerId === null) return
    pointerId = null
    it.span.style.cursor = 'grab'
    map.value?.dragPan.enable()
  }
  it.el.addEventListener('pointerup', end)
  it.el.addEventListener('pointercancel', end)
  it.el.addEventListener('lostpointercapture', end)

  it.el.addEventListener('dblclick', (e: MouseEvent) => {
    e.stopPropagation()
    it.dx = it.baseDx
    it.dy = it.baseDy
    applyLabelOffset(it)
    updateLabelLine(it)
    publishLabelLeaders(map.value, labelLeaderFeatures)
  })
}

/** Retire les étiquettes et vide la source des traits de liaison. */
function clearLabels(): void {
  for (const it of labelMarkers) it.marker.remove()
  labelMarkers = []
  labelLeaderFeatures = []
  publishLabelLeaders(map.value, labelLeaderFeatures)
}

/** Affiche les étiquettes de l'anomalie sélectionnée. */
function renderLabels(): void {
  clearLabels()
  if (!mapReady.value || !map.value) return

  const finding = auditStore.selectedFinding
  const preview = auditStore.deletePreview

  // Aperçu RP : les étiquettes des points conservés remplacent les standards.
  if (preview && finding?.kind === 'rp') {
    for (const item of preview.labels) createLabel(item)
    updateLabelLines()
    return
  }

  const overlay = auditStore.selectedOverlay
  if (!overlay) return

  for (const item of overlay.labels) {
    createLabel(item)
  }
  // Aperçu AR : les étiquettes de la plage supprimée sont masquées.
  if (preview && finding?.kind === 'ar') {
    hideLabelsInRange(preview.start, preview.end)
  }
  updateLabelLines()
}

/** Masque les étiquettes de la plage supprimée et leurs traits de liaison. */
function hideLabelsInRange(start: number, end: number): void {
  for (const it of labelMarkers) {
    const hidden = it.index >= start && it.index <= end
    it.el.style.display = hidden ? 'none' : ''
    if (it.lineRef) {
      it.lineRef.properties = {
        ...it.lineRef.properties,
        opacity: hidden ? 0 : 0.9,
      }
    }
  }
}

/** Crée une étiquette (marqueur HTML) et son trait de liaison. */
function createLabel(item: LabelItem): void {
  const m = map.value
  if (!m) return

  const color = leaderColor(item.cls)
  const lineRef: GeoJSON.Feature = {
    type: 'Feature',
    properties: { color, opacity: 0.9 },
    geometry: {
      type: 'LineString',
      coordinates: [
        [item.lon, item.lat],
        [item.lon, item.lat],
      ],
    },
  }
  labelLeaderFeatures.push(lineRef)

  const wrap = document.createElement('div')
  wrap.style.pointerEvents = 'auto'
  const span = document.createElement('span')
  span.textContent = String(item.no)
  applyLabelStyle(span, item.cls)
  wrap.appendChild(span)

  const lngLat: [number, number] = [item.lon, item.lat]
  const marker = new mapboxgl.Marker({ element: wrap, anchor: 'center' })
    .setLngLat(lngLat)
    .addTo(m)

  const entry: LabelMarker = {
    marker,
    el: wrap,
    span,
    index: item.index,
    lngLat,
    dx: item.dx,
    dy: item.dy,
    baseDx: item.dx,
    baseDy: item.dy,
    color,
    lineRef,
  }
  applyLabelOffset(entry)
  makeLabelDraggable(entry)
  labelMarkers.push(entry)
}

// ─── Centrage ─────────────────────────────────────────────────────────

/** Cadre la carte sur l'emprise de l'anomalie sélectionnée. */
function centerOnSelected(): void {
  const id = auditStore.selectedFindingId
  const m = map.value
  if (!id || !m) return
  const coords = boundsListByFinding.get(id)
  if (!coords || coords.length === 0) return

  const b = new mapboxgl.LngLatBounds(coords[0], coords[0])
  for (const c of coords) b.extend(c)
  m.fitBounds(b, { padding: 80, maxZoom: 18, duration: 500 })
}

// ─── Réactivité ───────────────────────────────────────────────────────

// La trace de travail est remplacée en bloc par chaque commande : un watch
// superficiel suffit et évite de parcourir la trace à chaque frappe.
watch(
  () => auditStore.working,
  () => {
    renderBaseTrace(false)
    renderAnalysis()
    renderLabels()
  },
)

watch(
  () => auditStore.findings,
  () => {
    renderAnalysis()
    renderLabels()
  },
)

// Les ancres de routage arrivent avec le rendu : elles colorent les points
// d'ancre des boucles, donc un nouveau passage est nécessaire.
watch(
  () => auditStore.overlays,
  () => {
    renderAnalysis()
    renderLabels()
  },
)

watch(
  () => auditStore.selectedFindingId,
  () => {
    renderLabels()
    centerOnSelected()
  },
)

// Aperçu de routage : les quatre couches d'aperçu et le cadrage d'union.
watch(
  () => auditStore.routePreview,
  () => {
    renderRoutePreview()
  },
)

// Curseurs Début/Fin de la vue de routage.
watch(
  () => auditStore.routeRange,
  () => {
    renderRouteSliders()
  },
)

// Aperçu de suppression : les couches de suppression, les points restylés et,
// en RP, les étiquettes des points conservés.
watch(
  () => auditStore.deletePreview,
  () => {
    renderDeletePreview()
    publishAnomalyPoints()
    renderLabels()
  },
)

// ─── Montage ──────────────────────────────────────────────────────────

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
})

onUnmounted(() => {
  resizeObserver?.disconnect()
  resizeObserver = null
  destroyMap()
})

// Cycle de vie de la clé (IHM §3.7) : la carte est créée dès qu'une clé est
// disponible, détruite si la clé est vidée. La vue courante n'est pas
// mémorisée/restaurée (le cas ne se présente qu'en saisie manuelle).
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
      replayCurrentState()
    }
  },
)

const placeholderVisible = computed(() => !hasMapboxKey.value)
</script>

<template>
  <div class="audit-map-wrapper">
    <div ref="mapContainer" class="audit-map-canvas" />

    <v-empty-state
      v-if="placeholderVisible"
      class="audit-map-placeholder"
      icon="mdi-map-outline"
      title="Clé Mapbox requise"
      text="Renseignez la clé dans les paramètres (Systeme.Key.mapBox) pour afficher la carte."
    />
  </div>
</template>

<style scoped>
.audit-map-wrapper {
  position: relative;
  width: 100%;
  height: 100%;
}

.audit-map-canvas {
  width: 100%;
  height: 100%;
}

.audit-map-placeholder {
  position: absolute;
  inset: 0;
  background: #dfe3e8;
}
</style>
