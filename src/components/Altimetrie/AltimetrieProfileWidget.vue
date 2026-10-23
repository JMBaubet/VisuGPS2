<template>
  <div ref="rootRef" class="alt-profile">
    <div class="alt-profile__body">
      <div
        ref="yAxisHost"
        class="alt-profile__y-axis"
        :title="yAxisTitle"
        @click="cycleYScale"
        @wheel.prevent="onYAxisWheel"
      >
        <canvas ref="yAxisRef" class="alt-profile__axis-canvas" />
      </div>

      <div
        ref="scrollContainer"
        class="alt-profile__scroll"
        :style="{
          height: `${plotViewportHeight}px`,
          background: plotBackground,
        }"
        @scroll="onScroll"
        @wheel.prevent="onWheel"
      >
        <canvas
          ref="canvasRef"
          class="alt-profile__canvas"
          :class="{ 'alt-profile__canvas--dragging': dragState }"
          @pointerdown="onPointerDown"
          @pointermove="onPointerMove"
          @pointerup="onPointerUp"
          @pointercancel="onPointerCancel"
        />
      </div>

      <div
        ref="xAxisHost"
        class="alt-profile__x-axis"
        :title="xAxisTitle"
        @click="cycleXScale"
        @wheel.prevent="onXAxisWheel"
      >
        <canvas ref="xAxisRef" class="alt-profile__axis-canvas" />
      </div>
    </div>

    <div v-if="creationHint || tunnelCreationHint" class="alt-profile__hint">
      {{ creationHint || tunnelCreationHint }}
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Widget de profil altimétrique flottant.
 *
 * Disposition :
 * - colonne gauche fixe : altitudes, clic sur la colonne = échelle Y ;
 * - bande basse fixe : distances, clic sur la bande = échelle X ;
 * - profil défilant horizontalement entre les deux, et verticalement quand
 *   l'échelle 2,5 m/px dépasse la hauteur du widget.
 *
 * Les deux axes sont redessinés à chaque défilement pour rester alignés avec
 * le profil. Le canvas du profil n'est pas redessiné pendant le défilement.
 *
 * Échelles : X = 50 ou 12,5 m/px ; Y = 10, 5 ou 2,5 m/px. Les touches X et Y
 * font défiler ces valeurs. Un clic sur un triangle sélectionne la montée et
 * permet de glisser ses bornes, sans changer l'échelle courante.
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useAppStore } from '../../stores/app'
import type {
  AltimetrieClimb,
  AltimetrieFile,
  AltimetrieProfilePoint,
} from '../../stores/altimetrie'
import { colorForCategoryId, colorForSlope } from '../../utils/altimetrieCategories'
import {
  constrainClimbBound,
  minimalSegmentAt,
  MIN_CLIMB_LENGTH_KM,
  type ClimbHandle,
} from '../../utils/altimetrieClimbs'
import { formatDistance } from '../../utils/format'
import type { Tunnel } from '../../stores/tunnels'
import {
  constrainTunnelBound,
  isShortTunnelOnWidget,
  MIN_TUNNEL_LENGTH_KM,
  tunnelRangeOverlaps,
} from '../../utils/altimetrieTunnels'

type TunnelHandle = 'entrance' | 'exit'

const props = defineProps<{
  /** Onglet actif : montées ou tunnels. */
  mode: 'climbs' | 'tunnels'
  file: AltimetrieFile | null
  selectedClimbId: number | null
  /** Mode création manuelle : le prochain clic pose la vallée, le suivant le sommet. */
  creating: boolean
  /** Tunnels physiques de la trace (les groupes restent des tronçons distincts). */
  tunnels: Tunnel[]
  selectedTunnelIds: string[]
  /** Mode création manuelle de tunnel : entrée puis sortie. */
  creatingTunnel: boolean
}>()

const emit = defineEmits<{
  'select-climb': [id: number]
  recenter: [distanceM: number]
  /** Valeurs du segment minimal au point cliqué, pour le tooltip de la carte. */
  inspect: [
    payload: { distanceM: number; slopePct: number; altitudeM: number },
  ]
  'edit-bounds': [
    payload: { climbId: number; handle: ClimbHandle; km: number },
  ]
  /** Borne en cours de glissement : aperçu local sans appel backend. */
  'draft-bounds': [
    payload: { climbId: number; handle: ClimbHandle; km: number },
  ]
  /** Glissement abandonné : l'aperçu local doit être retiré. */
  'cancel-bounds': []
  'create-climb': [payload: { startKm: number; endKm: number }]
  'cancel-create': []
  'create-error': [message: string]
  'select-tunnel': [id: string]
  'draft-tunnel-bounds': [
    payload: { tunnelId: string; handle: TunnelHandle; km: number },
  ]
  'edit-tunnel-bounds': [
    payload: { tunnelId: string; handle: TunnelHandle; km: number },
  ]
  'cancel-tunnel-bounds': []
  'create-tunnel': [payload: { startKm: number; endKm: number }]
  'cancel-create-tunnel': []
  'create-tunnel-error': [message: string]
}>()

const X_SCALES: number[] = [50, 12.5]
const Y_SCALES: number[] = [10, 5, 2.5]

const TUNNEL_COLOR = '#2196F3'
const TUNNEL_ENTRANCE_COLOR = '#1565C0'
const TUNNEL_EXIT_COLOR = '#42A5F5'

const Y_AXIS_WIDTH = 54
const X_AXIS_HEIGHT = 26
const PLOT_LEFT = 10
const RIGHT_PAD = 10
const TOP_PAD = 24
const PLOT_BOTTOM_PAD = 6
const MIN_PLOT_HEIGHT = 100
const HANDLE_HIT_X = 14
const HANDLE_HIT_HEIGHT = 22
const HANDLE_TOP_GAP = 22

interface WidgetView {
  leftPad: number
  xScale: number
  axisY: number
  plotTop: number
  handleTop: number
}

interface DragState {
  climbId: number
  handle: ClimbHandle
  km: number
}

interface TunnelDragState {
  tunnelId: string
  handle: TunnelHandle
  km: number
}

interface TunnelHandleHit {
  tunnel: Tunnel
  handle: TunnelHandle
}

interface HandleHit {
  climb: AltimetrieClimb
  handle: ClimbHandle
}

interface YTick {
  alt: number
  y: number
}

interface ThemeColors {
  dark: boolean
  background: string
  grid: string
  text: string
  muted: string
  line: string
  outline: string
}

const appStore = useAppStore()

const rootRef = ref<HTMLDivElement | null>(null)
const canvasRef = ref<HTMLCanvasElement | null>(null)
const yAxisHost = ref<HTMLDivElement | null>(null)
const yAxisRef = ref<HTMLCanvasElement | null>(null)
const xAxisHost = ref<HTMLDivElement | null>(null)
const xAxisRef = ref<HTMLCanvasElement | null>(null)
const scrollContainer = ref<HTMLDivElement | null>(null)
const xScale = ref(50)
const yScale = ref(10)
const dragState = ref<DragState | null>(null)
const dragTunnelState = ref<TunnelDragState | null>(null)
/** Création manuelle en cours : bornes de vallée et aperçu de sommet. */
const creation = ref<{ valleyKm: number; summitKm: number } | null>(null)
/** Création manuelle de tunnel : entrée puis sortie. */
const tunnelCreation = ref<{ entranceKm: number; exitKm: number } | null>(null)
/** Hauteur maximale du profil visible, en pixels. */
const maxPlotHeight = ref(computeMaxPlotHeight())

let currentView: WidgetView | null = null
let drawFrame = 0
let axesFrame = 0
let dragPointerId: number | null = null
/** Dernière borne transmise pendant le drag, pour éviter les doublons. */
let lastDraftKm: number | null = null
let lastDraftTunnelKm: number | null = null
let resizeObserver: ResizeObserver | null = null

const profile = computed(() => props.file?.profile ?? [])
const climbs = computed(() => props.file?.climbs ?? [])
const segments = computed(() => props.file?.segments ?? [])

const selectedClimb = computed(
  () => climbs.value.find(climb => climb.id === props.selectedClimbId) ?? null,
)

const tunnels = computed(() => props.tunnels ?? [])
const selectedTunnels = computed(() =>
  tunnels.value.filter(tunnel => props.selectedTunnelIds.includes(tunnel.id)),
)
const selectedTunnel = computed(() => selectedTunnels.value[0] ?? null)
/** Un tunnel fusionné n'est pas étirable : il faut d'abord le séparer. */
const selectedTunnelEditable = computed(
  () =>
    selectedTunnels.value.length === 1 &&
    !selectedTunnels.value[0]?.groupId,
)

const totalDistanceM = computed(() => {
  const points = profile.value
  return points.length > 0 ? points[points.length - 1].d : 0
})

const totalKm = computed(() => totalDistanceM.value / 1000)

const altBounds = computed(() => {
  const points = profile.value
  if (points.length === 0) return { min: 0, max: 0 }

  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const point of points) {
    if (point.ele < min) min = point.ele
    if (point.ele > max) max = point.ele
  }
  return { min, max }
})

/**
 * Géométrie verticale du profil.
 *
 * `plotTop` peut être plus grand que `TOP_PAD` quand l'amplitude d'altitude
 * produit un profil plus court que `MIN_PLOT_HEIGHT` : l'espace excédentaire
 * est placé au-dessus, pour que le bas du profil reste sur l'axe X fixe.
 */
const plotGeometry = computed(() => {
  const altMin = altBounds.value.min
  const altMax = altBounds.value.max
  const range = Math.max(altMax - altMin, 1)
  const altitudeHeight = range / yScale.value
  const plotAreaHeight = Math.max(altitudeHeight, MIN_PLOT_HEIGHT)
  const plotTop = TOP_PAD + (plotAreaHeight - altitudeHeight)
  const axisY = plotTop + altitudeHeight
  const canvasHeight = axisY + PLOT_BOTTOM_PAD

  return {
    altMin,
    altMax,
    range,
    altitudeHeight,
    plotTop,
    axisY,
    canvasHeight,
  }
})

/** Hauteur visible du profil : le widget grandit, puis plafonne et défile. */
const plotViewportHeight = computed(() =>
  Math.min(plotGeometry.value.canvasHeight, maxPlotHeight.value),
)

/** Repères d'altitude, partagés par le canvas du profil et la colonne fixe. */
const yTicks = computed<YTick[]>(() => {
  const { altMin, altMax, plotTop } = plotGeometry.value
  const step = niceStep(yScale.value * 32)
  const ticks: YTick[] = []

  for (
    let alt = Math.ceil(altMin / step) * step;
    alt <= altMax + 1e-9;
    alt += step
  ) {
    ticks.push({ alt, y: plotTop + (altMax - alt) / yScale.value })
  }

  if (ticks.length === 0) {
    ticks.push({ alt: altMax, y: plotTop })
    ticks.push({
      alt: altMin,
      y: plotTop + (altMax - altMin) / yScale.value,
    })
  }

  return ticks
})

const plotBackground = computed(() =>
  appStore.isDarkMode ? '#0f172a' : '#f8fafc',
)

const yAxisTitle = computed(
  () =>
    `Échelle Y : ${formatScale(yScale.value)} m/px — cliquer pour changer (touche Y)`,
)

const xAxisTitle = computed(
  () =>
    `Échelle X : ${formatScale(xScale.value)} m/px — cliquer pour changer (touche X)`,
)

const creationHint = computed(() => {
  if (!props.creating) return ''
  return creation.value
    ? 'Cliquez pour poser le sommet — Échap pour annuler'
    : 'Cliquez pour poser la vallée — Échap pour annuler'
})

const tunnelCreationHint = computed(() => {
  if (!props.creatingTunnel) return ''
  return tunnelCreation.value
    ? 'Cliquez pour poser la sortie du tunnel — Échap pour annuler'
    : "Cliquez pour poser l'entrée du tunnel — Échap pour annuler"
})

function computeMaxPlotHeight(): number {
  const available = window.innerHeight * 0.45 - X_AXIS_HEIGHT
  return Math.max(MIN_PLOT_HEIGHT + TOP_PAD + PLOT_BOTTOM_PAD, available)
}

function formatScale(value: number): string {
  return Number.isInteger(value)
    ? String(value)
    : value.toFixed(1).replace('.', ',')
}

function niceStep(raw: number): number {
  if (!Number.isFinite(raw) || raw <= 0) return 1
  const exponent = Math.floor(Math.log10(raw))
  const fraction = raw / 10 ** exponent
  const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10
  return nice * 10 ** exponent
}

function lowerBound(
  profilePoints: AltimetrieProfilePoint[],
  targetM: number,
): number {
  let lo = 0
  let hi = profilePoints.length - 1
  while (lo < hi) {
    const mid = (lo + hi) >> 1
    if (profilePoints[mid].d < targetM) lo = mid + 1
    else hi = mid
  }
  return lo
}

function xOfDistance(distanceM: number, view: WidgetView): number {
  return view.leftPad + distanceM / view.xScale
}

function distanceAtX(x: number, view: WidgetView): number {
  return (x - view.leftPad) * view.xScale
}

function isInsideClimb(km: number): boolean {
  return climbs.value.some(
    climb => km >= climb.startKm && km <= climb.endKm,
  )
}

function isFreeRange(startKm: number, endKm: number): boolean {
  const lo = Math.min(startKm, endKm)
  const hi = Math.max(startKm, endKm)
  return !climbs.value.some(
    climb => lo < climb.endKm && hi > climb.startKm,
  )
}

function creationValid(valleyKm: number, summitKm: number): boolean {
  return (
    summitKm > valleyKm + MIN_CLIMB_LENGTH_KM &&
    isFreeRange(valleyKm, summitKm)
  )
}

function isInsideTunnel(km: number): boolean {
  return tunnels.value.some(
    tunnel => km >= tunnel.startKm && km <= tunnel.endKm,
  )
}

function tunnelCreationValid(entranceKm: number, exitKm: number): boolean {
  return (
    exitKm > entranceKm + MIN_TUNNEL_LENGTH_KM &&
    !tunnelRangeOverlaps(tunnels.value, entranceKm, exitKm)
  )
}

/**
 * Change l'échelle X en conservant le centre visible.
 *
 * Le redessin est demandé avant le repositionnement : la largeur du canvas est
 * recalculée, puis le navigateur borne le défilement avant la nouvelle valeur.
 */
function setXScale(next: number): void {
  if (next === xScale.value) return

  const container = scrollContainer.value
  const previous = xScale.value
  const centerDistance = container
    ? (container.scrollLeft + container.clientWidth / 2) * previous
    : 0

  xScale.value = next
  requestDraw()

  requestAnimationFrame(() => {
    const element = scrollContainer.value
    if (!element) return
    element.scrollLeft = Math.max(
      0,
      centerDistance / next - element.clientWidth / 2,
    )
    requestAxesDraw()
  })
}

function cycleXScale(): void {
  const index = X_SCALES.indexOf(xScale.value)
  setXScale(X_SCALES[(index + 1) % X_SCALES.length])
}

function setYScale(next: number): void {
  if (next === yScale.value) return
  yScale.value = next
  requestDraw()
}

function cycleYScale(): void {
  const index = Y_SCALES.indexOf(yScale.value)
  setYScale(Y_SCALES[(index + 1) % Y_SCALES.length])
}

/**
 * Recentre la fenêtre de défilement sur une plage kilométrique, sans changer
 * l'échelle X choisie par l'opérateur.
 */
function scrollToRange(startKm: number, endKm: number): void {
  const container = scrollContainer.value
  if (!container) return

  const centerM = ((startKm + endKm) / 2) * 1000
  const centerX = PLOT_LEFT + centerM / xScale.value
  container.scrollLeft = Math.max(
    0,
    centerX - container.clientWidth / 2,
  )
  requestAxesDraw()
}

function requestDraw(): void {
  if (drawFrame) return
  drawFrame = requestAnimationFrame(() => {
    drawFrame = 0
    draw()
  })
}

/** Redessine uniquement les axes fixes, après un défilement. */
function requestAxesDraw(): void {
  if (axesFrame) return
  axesFrame = requestAnimationFrame(() => {
    axesFrame = 0
    drawYAxis()
    drawXAxis()
  })
}

function getTheme(): ThemeColors {
  const dark = appStore.isDarkMode
  return {
    dark,
    background: dark ? '#0f172a' : '#f8fafc',
    grid: dark ? 'rgba(148,163,184,0.38)' : 'rgba(100,116,139,0.28)',
    text: dark ? 'rgba(226,232,240,0.78)' : 'rgba(51,65,85,0.82)',
    muted: dark ? 'rgba(148,163,184,0.68)' : 'rgba(100,116,139,0.72)',
    line: dark ? '#e2e8f0' : '#1e293b',
    outline: dark ? 'rgba(255,255,255,0.22)' : 'rgba(0,0,0,0.14)',
  }
}

/** Prépare un canvas haute densité et retourne son contexte. */
function prepareCanvas(
  canvas: HTMLCanvasElement,
  width: number,
  height: number,
): CanvasRenderingContext2D | null {
  const dpr = window.devicePixelRatio || 1
  const cssWidth = Math.max(1, Math.floor(width))
  const cssHeight = Math.max(1, Math.floor(height))
  const pixelWidth = Math.max(1, Math.floor(cssWidth * dpr))
  const pixelHeight = Math.max(1, Math.floor(cssHeight * dpr))

  if (canvas.width !== pixelWidth) canvas.width = pixelWidth
  if (canvas.height !== pixelHeight) canvas.height = pixelHeight
  canvas.style.width = `${cssWidth}px`
  canvas.style.height = `${cssHeight}px`

  const ctx = canvas.getContext('2d')
  if (!ctx) return null
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  return ctx
}

function draw(): void {
  drawPlot()
  drawYAxis()
  drawXAxis()
}

function drawPlot(): void {
  const canvas = canvasRef.value
  if (!canvas) return

  const points = profile.value
  const totalM = totalDistanceM.value
  if (!props.file || points.length < 2 || totalM <= 0) {
    currentView = null
    canvas.width = 1
    canvas.height = 1
    canvas.style.width = '1px'
    canvas.style.height = '1px'
    return
  }

  const theme = getTheme()
  const { altMax, plotTop, axisY, canvasHeight } = plotGeometry.value
  const width = Math.ceil(PLOT_LEFT + totalM / xScale.value + RIGHT_PAD)
  const ctx = prepareCanvas(canvas, width, canvasHeight)
  if (!ctx) return

  ctx.fillStyle = theme.background
  ctx.fillRect(0, 0, width, canvasHeight)

  const xOf = (distanceM: number) => PLOT_LEFT + distanceM / xScale.value
  const yOf = (ele: number) => plotTop + (altMax - ele) / yScale.value
  const handleTop = plotTop - HANDLE_TOP_GAP

  currentView = {
    leftPad: PLOT_LEFT,
    xScale: xScale.value,
    axisY,
    plotTop,
    handleTop,
  }

  // Repères Y (les libellés sont dans la colonne fixe)
  ctx.save()
  ctx.lineWidth = 1
  ctx.strokeStyle = theme.grid
  ctx.setLineDash([3, 4])
  for (const tick of yTicks.value) {
    ctx.beginPath()
    ctx.moveTo(PLOT_LEFT, tick.y)
    ctx.lineTo(width - RIGHT_PAD, tick.y)
    ctx.stroke()
  }
  ctx.restore()

  // Remplissage par segments de pente
  for (const segment of segments.value) {
    const startIdx = lowerBound(points, segment.startKm * 1000)
    const endIdx = lowerBound(points, segment.endKm * 1000)
    if (endIdx <= startIdx) continue

    const firstPoint = points[startIdx]
    const lastPoint = points[endIdx]
    if (!firstPoint || !lastPoint) continue

    ctx.beginPath()
    ctx.moveTo(xOf(firstPoint.d), yOf(firstPoint.ele))
    for (let i = startIdx + 1; i <= endIdx; i++) {
      const point = points[i]
      if (point) ctx.lineTo(xOf(point.d), yOf(point.ele))
    }
    ctx.lineTo(xOf(lastPoint.d), axisY)
    ctx.lineTo(xOf(firstPoint.d), axisY)
    ctx.closePath()

    ctx.globalAlpha = 0.55
    ctx.fillStyle = colorForCategoryId(segment.categoryId)
    ctx.fill()
    ctx.globalAlpha = 1
    ctx.strokeStyle = theme.outline
    ctx.lineWidth = 0.8
    ctx.stroke()
  }

  // Bande de la montée sélectionnée (mode Montées)
  const selected = selectedClimb.value
  if (props.mode === 'climbs' && selected) {
    const drag =
      dragState.value?.climbId === selected.id ? dragState.value : null
    const startKm = drag?.handle === 'valley' ? drag.km : selected.startKm
    const endKm = drag?.handle === 'summit' ? drag.km : selected.endKm
    const x1 = xOf(Math.min(startKm, endKm) * 1000)
    const x2 = xOf(Math.max(startKm, endKm) * 1000)
    ctx.fillStyle = theme.dark
      ? 'rgba(250,204,21,0.12)'
      : 'rgba(250,204,21,0.18)'
    ctx.fillRect(x1, plotTop, Math.max(1, x2 - x1), axisY - plotTop)
  }

  // Bandes des tunnels sélectionnés (mode Tunnels). Le groupe fusionné est
  // signalé par une bande très légère sur son étendue, chaque tronçon par une
  // bande plus marquée : l'écart n'est jamais peint comme du tunnel.
  if (props.mode === 'tunnels' && selectedTunnels.value.length > 0) {
    const groupStart = Math.min(
      ...selectedTunnels.value.map(tunnel => tunnel.startKm),
    )
    const groupEnd = Math.max(
      ...selectedTunnels.value.map(tunnel => tunnel.endKm),
    )
    const groupX1 = xOf(groupStart * 1000)
    const groupX2 = xOf(groupEnd * 1000)
    ctx.fillStyle = theme.dark
      ? 'rgba(33,150,243,0.10)'
      : 'rgba(33,150,243,0.07)'
    ctx.fillRect(
      groupX1,
      plotTop,
      Math.max(1, groupX2 - groupX1),
      axisY - plotTop,
    )
    ctx.fillStyle = theme.dark
      ? 'rgba(33,150,243,0.20)'
      : 'rgba(33,150,243,0.15)'
    for (const tunnel of selectedTunnels.value) {
      const x1 = xOf(tunnel.startKm * 1000)
      const x2 = xOf(tunnel.endKm * 1000)
      ctx.fillRect(x1, plotTop, Math.max(1, x2 - x1), axisY - plotTop)
    }
  }

  // Ligne de profil
  ctx.beginPath()
  for (let i = 0; i < points.length; i++) {
    const point = points[i]
    if (!point) continue
    const x = xOf(point.d)
    const y = yOf(point.ele)
    if (i === 0) ctx.moveTo(x, y)
    else ctx.lineTo(x, y)
  }
  ctx.strokeStyle = theme.line
  ctx.lineWidth = 1.4
  ctx.stroke()

  if (props.mode === 'climbs') {
    // Traits horizontaux des montées, au-dessus de l'axe X
    for (const climb of climbs.value) {
      const x1 = xOf(climb.startKm * 1000)
      const x2 = xOf(climb.endKm * 1000)
      const barWidth = Math.max(1, x2 - x1)
      const isSelected = climb.id === props.selectedClimbId
      const barHeight = isSelected ? 5 : 3
      const barY = axisY - 3 - (barHeight - 3)

      ctx.fillStyle = colorForSlope(climb.avgSlopePct)
      ctx.globalAlpha = isSelected ? 1 : 0.85
      ctx.fillRect(x1, barY, barWidth, barHeight)
      ctx.globalAlpha = 1
      ctx.strokeStyle = isSelected
        ? theme.dark
          ? '#ffffff'
          : '#1e293b'
        : theme.outline
      ctx.lineWidth = isSelected ? 1.2 : 0.6
      ctx.strokeRect(x1, barY, barWidth, barHeight)
    }

    // Triangles vallée/sommet et traits pointillés
    for (const climb of climbs.value) {
      const isSelected = climb.id === props.selectedClimbId
      const drag =
        dragState.value?.climbId === climb.id ? dragState.value : null
      const valleyKm = drag?.handle === 'valley' ? drag.km : climb.startKm
      const summitKm = drag?.handle === 'summit' ? drag.km : climb.endKm

      drawProfileHandle(
        ctx,
        xOf(valleyKm * 1000),
        '#16a34a',
        isSelected,
        handleTop,
        axisY,
        theme.dark,
      )
      drawProfileHandle(
        ctx,
        xOf(summitKm * 1000),
        '#dc2626',
        isSelected,
        handleTop,
        axisY,
        theme.dark,
      )
    }

    // Aperçu de la création manuelle
    const creating = creation.value
    if (creating) {
      const valid = creationValid(creating.valleyKm, creating.summitKm)
      const valleyX = xOf(creating.valleyKm * 1000)
      const summitX = xOf(creating.summitKm * 1000)
      ctx.fillStyle = valid
        ? 'rgba(34,197,94,0.14)'
        : 'rgba(239,68,68,0.16)'
      ctx.fillRect(
        Math.min(valleyX, summitX),
        plotTop,
        Math.max(1, Math.abs(summitX - valleyX)),
        axisY - plotTop,
      )
      drawProfileHandle(
        ctx,
        valleyX,
        '#16a34a',
        true,
        handleTop,
        axisY,
        theme.dark,
      )
      drawProfileHandle(
        ctx,
        summitX,
        valid ? '#dc2626' : '#ef4444',
        true,
        handleTop,
        axisY,
        theme.dark,
      )
    }
  }

  if (props.mode === 'tunnels') {
    drawTunnelsOverlay(ctx, xOf, plotTop, axisY, handleTop, theme)
  }
}

/** Colonne fixe des altitudes. */
function drawYAxis(): void {
  const canvas = yAxisRef.value
  const host = yAxisHost.value
  if (!canvas || !host) return

  const width = host.clientWidth || Y_AXIS_WIDTH
  const height = plotViewportHeight.value + X_AXIS_HEIGHT
  const ctx = prepareCanvas(canvas, width, height)
  if (!ctx) return

  const theme = getTheme()
  ctx.fillStyle = theme.background
  ctx.fillRect(0, 0, width, height)

  ctx.beginPath()
  ctx.moveTo(width - 0.5, 0)
  ctx.lineTo(width - 0.5, height)
  ctx.strokeStyle = theme.grid
  ctx.lineWidth = 1
  ctx.stroke()

  if (props.file && profile.value.length >= 2) {
    const scrollTop = scrollContainer.value?.scrollTop ?? 0
    const viewportHeight = plotViewportHeight.value

    ctx.save()
    ctx.font =
      '10px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    ctx.textAlign = 'left'
    ctx.textBaseline = 'middle'
    ctx.fillStyle = theme.text

    for (const tick of yTicks.value) {
      const y = tick.y - scrollTop
      if (y < -6 || y > viewportHeight + 6) continue
      ctx.fillText(`${Math.round(tick.alt)} m`, 4, y)
    }

    ctx.restore()
  }

  // Rappel de l'échelle Y, dans le coin bas-gauche
  ctx.save()
  ctx.font = '9px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillStyle = theme.muted
  ctx.fillText(
    `Y ${formatScale(yScale.value)}`,
    width / 2,
    plotViewportHeight.value + X_AXIS_HEIGHT / 2,
  )
  ctx.restore()
}

/** Bande fixe des distances. */
function drawXAxis(): void {
  const canvas = xAxisRef.value
  const host = xAxisHost.value
  if (!canvas || !host) return

  const width = host.clientWidth
  const height = X_AXIS_HEIGHT
  if (width <= 0) return

  const ctx = prepareCanvas(canvas, width, height)
  if (!ctx) return

  const theme = getTheme()
  ctx.fillStyle = theme.background
  ctx.fillRect(0, 0, width, height)

  ctx.beginPath()
  ctx.moveTo(0, 0.5)
  ctx.lineTo(width, 0.5)
  ctx.strokeStyle = theme.grid
  ctx.lineWidth = 1
  ctx.stroke()

  const totalM = totalDistanceM.value
  if (!props.file || totalM <= 0) return

  const scrollLeft = scrollContainer.value?.scrollLeft ?? 0
  const step = niceStep(xScale.value * 110)

  ctx.save()
  ctx.font =
    '10px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
  ctx.textBaseline = 'top'
  ctx.fillStyle = theme.text

  for (
    let distance = 0;
    distance <= totalM + 1e-9;
    distance += step
  ) {
    const x = PLOT_LEFT + distance / xScale.value - scrollLeft
    if (x < -40 || x > width + 40) continue

    ctx.beginPath()
    ctx.moveTo(x, 0)
    ctx.lineTo(x, 4)
    ctx.strokeStyle = theme.grid
    ctx.lineWidth = 1
    ctx.stroke()

    // Le km 0 est aligné sur le bord gauche pour rester entièrement visible.
    const leftAligned = distance === 0 && x <= PLOT_LEFT + 2
    ctx.textAlign = leftAligned ? 'left' : 'center'
    ctx.fillText(
      formatDistance(distance),
      leftAligned ? x + 2 : x,
      6,
    )
  }

  ctx.restore()
}

function drawTunnelHandle(
  ctx: CanvasRenderingContext2D,
  x: number,
  color: string,
  selected: boolean,
  top: number,
  axisY: number,
  dark: boolean,
): void {
  const size = selected ? 6 : 4
  const centerY = top + 4

  ctx.save()
  ctx.globalAlpha = selected ? 0.95 : 0.5
  ctx.strokeStyle = color
  ctx.lineWidth = selected ? 1.5 : 1
  ctx.setLineDash([4, 3])
  ctx.beginPath()
  ctx.moveTo(x, centerY + size)
  ctx.lineTo(x, axisY)
  ctx.stroke()
  ctx.restore()

  ctx.save()
  ctx.beginPath()
  ctx.moveTo(x, centerY - size)
  ctx.lineTo(x + size, centerY)
  ctx.lineTo(x, centerY + size)
  ctx.lineTo(x - size, centerY)
  ctx.closePath()
  ctx.globalAlpha = selected ? 1 : 0.7
  ctx.fillStyle = color
  ctx.fill()
  ctx.globalAlpha = 1
  ctx.strokeStyle = dark ? '#ffffff' : '#1e293b'
  ctx.lineWidth = 1
  ctx.stroke()
  ctx.restore()
}

/**
 * Tunnels dans le profil : trait vertical pointillé bleu pour un tunnel court,
 * bande bleue pour un tunnel long, poignées entrée/sortie du tunnel
 * sélectionné non fusionné, et aperçu de création en deux clics.
 */
function drawTunnelsOverlay(
  ctx: CanvasRenderingContext2D,
  xOf: (distanceM: number) => number,
  plotTop: number,
  axisY: number,
  handleTop: number,
  theme: ThemeColors,
): void {
  for (const tunnel of tunnels.value) {
    const x1 = xOf(tunnel.startKm * 1000)
    const x2 = xOf(tunnel.endKm * 1000)
    const selected = props.selectedTunnelIds.includes(tunnel.id)
    const short = isShortTunnelOnWidget(
      tunnel.startKm,
      tunnel.endKm,
      xScale.value,
    )

    // Le repère vertical reste visible pour les tunnels non sélectionnés même
    // lorsqu'ils deviennent assez larges pour être dessinés en bande : à
    // l'échelle X étendue, la bande seule se confond avec l'axe.
    if (short || !selected) {
      const mid = (x1 + x2) / 2
      ctx.save()
      ctx.strokeStyle = TUNNEL_COLOR
      ctx.lineWidth = selected ? 3 : 2.5
      ctx.globalAlpha = selected ? 1 : 0.9
      ctx.setLineDash([4, 3])
      ctx.beginPath()
      ctx.moveTo(mid, handleTop)
      ctx.lineTo(mid, axisY)
      ctx.stroke()
      ctx.restore()
    }

    if (!short) {
      const barHeight = selected ? 6 : 4
      const barY = axisY - barHeight
      const barWidth = Math.max(1, x2 - x1)
      ctx.save()
      ctx.fillStyle = TUNNEL_COLOR
      ctx.globalAlpha = selected ? 1 : 0.9
      ctx.fillRect(x1, barY, barWidth, barHeight)
      ctx.globalAlpha = 1
      ctx.strokeStyle = selected
        ? theme.dark
          ? '#ffffff'
          : '#1e293b'
        : theme.outline
      ctx.lineWidth = selected ? 1.2 : 0.6
      ctx.strokeRect(x1, barY, barWidth, barHeight)
      ctx.restore()
    }
  }

  // Poignées du tunnel sélectionné : uniquement s'il est isolé. Un groupe
  // fusionné se sépare d'abord dans la liste.
  if (selectedTunnelEditable.value && selectedTunnel.value) {
    const tunnel = selectedTunnel.value
    const drag =
      dragTunnelState.value?.tunnelId === tunnel.id
        ? dragTunnelState.value
        : null
    const entranceKm = drag?.handle === 'entrance' ? drag.km : tunnel.startKm
    const exitKm = drag?.handle === 'exit' ? drag.km : tunnel.endKm

    drawTunnelHandle(
      ctx,
      xOf(entranceKm * 1000),
      TUNNEL_ENTRANCE_COLOR,
      true,
      handleTop,
      axisY,
      theme.dark,
    )
    drawTunnelHandle(
      ctx,
      xOf(exitKm * 1000),
      TUNNEL_EXIT_COLOR,
      true,
      handleTop,
      axisY,
      theme.dark,
    )
  }

  // Aperçu de la création manuelle : entrée puis sortie.
  const creating = tunnelCreation.value
  if (creating) {
    const valid = tunnelCreationValid(creating.entranceKm, creating.exitKm)
    const entranceX = xOf(creating.entranceKm * 1000)
    const exitX = xOf(creating.exitKm * 1000)

    ctx.save()
    ctx.fillStyle = valid
      ? 'rgba(33,150,243,0.16)'
      : 'rgba(239,68,68,0.16)'
    ctx.fillRect(
      Math.min(entranceX, exitX),
      plotTop,
      Math.max(1, Math.abs(exitX - entranceX)),
      axisY - plotTop,
    )
    ctx.restore()

    drawTunnelHandle(
      ctx,
      entranceX,
      TUNNEL_ENTRANCE_COLOR,
      true,
      handleTop,
      axisY,
      theme.dark,
    )
    drawTunnelHandle(
      ctx,
      exitX,
      valid ? TUNNEL_EXIT_COLOR : '#ef4444',
      true,
      handleTop,
      axisY,
      theme.dark,
    )
  }
}

function drawProfileHandle(
  ctx: CanvasRenderingContext2D,
  x: number,
  color: string,
  selected: boolean,
  top: number,
  axisY: number,
  dark: boolean,
): void {
  const size = selected ? 8 : 5
  const apex = top + size * 1.6

  ctx.save()
  ctx.globalAlpha = selected ? 0.9 : 0.45
  ctx.strokeStyle = color
  ctx.lineWidth = selected ? 1.5 : 1
  ctx.setLineDash([4, 3])
  ctx.beginPath()
  ctx.moveTo(x, apex)
  ctx.lineTo(x, axisY)
  ctx.stroke()
  ctx.restore()

  ctx.save()
  ctx.beginPath()
  ctx.moveTo(x, apex)
  ctx.lineTo(x - size, top)
  ctx.lineTo(x + size, top)
  ctx.closePath()
  ctx.globalAlpha = selected ? 1 : 0.7
  ctx.fillStyle = color
  ctx.fill()
  ctx.globalAlpha = 1
  ctx.strokeStyle = dark ? '#ffffff' : '#1e293b'
  ctx.lineWidth = 1
  ctx.stroke()
  ctx.restore()
}

function hitHandle(x: number, y: number): HandleHit | null {
  if (!currentView || y > currentView.handleTop + HANDLE_HIT_HEIGHT) {
    return null
  }

  let best: (HandleHit & { distance: number }) | null = null
  for (const climb of climbs.value) {
    const valleyX = xOfDistance(climb.startKm * 1000, currentView)
    const summitX = xOfDistance(climb.endKm * 1000, currentView)
    const valleyDistance = Math.abs(x - valleyX)
    const summitDistance = Math.abs(x - summitX)

    if (
      valleyDistance <= HANDLE_HIT_X &&
      (!best || valleyDistance < best.distance)
    ) {
      best = { climb, handle: 'valley', distance: valleyDistance }
    }
    if (
      summitDistance <= HANDLE_HIT_X &&
      (!best || summitDistance < best.distance)
    ) {
      best = { climb, handle: 'summit', distance: summitDistance }
    }
  }

  return best ? { climb: best.climb, handle: best.handle } : null
}

function hitBar(x: number, y: number): AltimetrieClimb | null {
  if (!currentView) return null
  if (y < currentView.axisY - 10 || y > currentView.axisY + 2) return null

  for (const climb of climbs.value) {
    const x1 = xOfDistance(climb.startKm * 1000, currentView)
    const x2 = xOfDistance(climb.endKm * 1000, currentView)
    if (x >= x1 - 2 && x <= x2 + 2) return climb
  }
  return null
}

function hitTunnelHandle(x: number, y: number): TunnelHandleHit | null {
  if (!currentView || !selectedTunnelEditable.value || !selectedTunnel.value) {
    return null
  }
  if (
    y < currentView.handleTop - 8 ||
    y > currentView.handleTop + HANDLE_HIT_HEIGHT
  ) {
    return null
  }

  const tunnel = selectedTunnel.value
  const drag =
    dragTunnelState.value?.tunnelId === tunnel.id
      ? dragTunnelState.value
      : null
  const entranceKm = drag?.handle === 'entrance' ? drag.km : tunnel.startKm
  const exitKm = drag?.handle === 'exit' ? drag.km : tunnel.endKm
  const entranceX = xOfDistance(entranceKm * 1000, currentView)
  const exitX = xOfDistance(exitKm * 1000, currentView)
  const entranceDistance = Math.abs(x - entranceX)
  const exitDistance = Math.abs(x - exitX)

  if (entranceDistance <= HANDLE_HIT_X && entranceDistance <= exitDistance) {
    return { tunnel, handle: 'entrance' }
  }
  if (exitDistance <= HANDLE_HIT_X) {
    return { tunnel, handle: 'exit' }
  }
  return null
}

function hitTunnelMark(x: number, y: number): Tunnel | null {
  if (!currentView) return null

  for (const tunnel of tunnels.value) {
    const x1 = xOfDistance(tunnel.startKm * 1000, currentView)
    const x2 = xOfDistance(tunnel.endKm * 1000, currentView)

    if (isShortTunnelOnWidget(tunnel.startKm, tunnel.endKm, xScale.value)) {
      const mid = (x1 + x2) / 2
      if (
        Math.abs(x - mid) <= 6 &&
        y >= currentView.handleTop &&
        y <= currentView.axisY + 2
      ) {
        return tunnel
      }
    } else if (
      y >= currentView.axisY - 12 &&
      y <= currentView.axisY + 2 &&
      x >= x1 - 2 &&
      x <= x2 + 2
    ) {
      return tunnel
    }
  }
  return null
}

function handleTunnelCreationClick(x: number, y: number): void {
  if (!currentView) return
  if (y < currentView.plotTop || y > currentView.axisY) return

  const km = distanceAtX(x, currentView) / 1000
  if (km < 0 || km > totalKm.value) return

  if (!tunnelCreation.value) {
    if (isInsideTunnel(km)) {
      emit(
        'create-tunnel-error',
        "Le point d'entrée doit être en dehors d'un tunnel existant.",
      )
      return
    }
    tunnelCreation.value = {
      entranceKm: km,
      exitKm: Math.min(totalKm.value, km + MIN_TUNNEL_LENGTH_KM),
    }
    requestDraw()
    return
  }

  const entranceKm = tunnelCreation.value.entranceKm
  if (km <= entranceKm + MIN_TUNNEL_LENGTH_KM) {
    emit(
      'create-tunnel-error',
      "La sortie doit être placée après l'entrée (10 m minimum).",
    )
    return
  }
  if (tunnelRangeOverlaps(tunnels.value, entranceKm, km)) {
    emit('create-tunnel-error', 'Le tunnel chevauche un tunnel existant.')
    return
  }

  emit('create-tunnel', { startKm: entranceKm, endKm: km })
}

function recenterAndInspect(x: number): void {
  if (!currentView) return
  const distanceM = distanceAtX(x, currentView)
  if (distanceM < 0 || distanceM > totalDistanceM.value) return

  emit('recenter', distanceM)
  const segment = minimalSegmentAt(profile.value, distanceM)
  if (segment) {
    emit('inspect', {
      distanceM: segment.distanceM,
      slopePct: segment.slopePct,
      altitudeM: segment.altitudeM,
    })
  }
}

function handleCreationClick(x: number, y: number): void {
  if (!currentView) return
  if (y < currentView.plotTop || y > currentView.axisY) return

  const km = distanceAtX(x, currentView) / 1000
  if (km < 0 || km > totalKm.value) return

  if (!creation.value) {
    if (isInsideClimb(km)) {
      emit(
        'create-error',
        "Le point de vallée doit être en dehors d'une montée existante.",
      )
      return
    }
    creation.value = {
      valleyKm: km,
      summitKm: Math.min(totalKm.value, km + MIN_CLIMB_LENGTH_KM),
    }
    requestDraw()
    return
  }

  const valleyKm = creation.value.valleyKm
  if (km <= valleyKm + MIN_CLIMB_LENGTH_KM) {
    emit(
      'create-error',
      'Le sommet doit être placé après la vallée (50 m minimum).',
    )
    return
  }
  if (!isFreeRange(valleyKm, km)) {
    emit('create-error', 'La montée chevauche une montée existante.')
    return
  }

  emit('create-climb', { startKm: valleyKm, endKm: km })
}

function onPointerDown(event: PointerEvent): void {
  const canvas = canvasRef.value
  if (!canvas || !currentView) return

  const rect = canvas.getBoundingClientRect()
  const x = event.clientX - rect.left
  const y = event.clientY - rect.top

  if (props.mode === 'tunnels') {
    if (props.creatingTunnel) {
      handleTunnelCreationClick(x, y)
      return
    }

    const tunnelHandle = hitTunnelHandle(x, y)
    if (tunnelHandle) {
      emit('select-tunnel', tunnelHandle.tunnel.id)
      dragState.value = null
      dragTunnelState.value = {
        tunnelId: tunnelHandle.tunnel.id,
        handle: tunnelHandle.handle,
        km:
          tunnelHandle.handle === 'entrance'
            ? tunnelHandle.tunnel.startKm
            : tunnelHandle.tunnel.endKm,
      }
      lastDraftTunnelKm = null
      dragPointerId = event.pointerId
      canvas.setPointerCapture(event.pointerId)
      event.preventDefault()
      requestDraw()
      return
    }

    const tunnelHit = hitTunnelMark(x, y)
    if (tunnelHit) {
      emit('select-tunnel', tunnelHit.id)
      return
    }

    recenterAndInspect(x)
    return
  }

  if (props.creating) {
    handleCreationClick(x, y)
    return
  }

  const handleHit = hitHandle(x, y)
  if (handleHit) {
    emit('select-climb', handleHit.climb.id)
    dragTunnelState.value = null

    dragState.value = {
      climbId: handleHit.climb.id,
      handle: handleHit.handle,
      km:
        handleHit.handle === 'valley'
          ? handleHit.climb.startKm
          : handleHit.climb.endKm,
    }
    lastDraftKm = null
    dragPointerId = event.pointerId
    canvas.setPointerCapture(event.pointerId)
    event.preventDefault()
    requestDraw()
    return
  }

  const barHit = hitBar(x, y)
  if (barHit) {
    emit('select-climb', barHit.id)
    return
  }

  recenterAndInspect(x)
}

function onPointerMove(event: PointerEvent): void {
  const canvas = canvasRef.value
  if (!canvas || !currentView) return

  const rect = canvas.getBoundingClientRect()
  const x = event.clientX - rect.left

  if (props.mode === 'tunnels') {
    if (props.creatingTunnel && tunnelCreation.value) {
      const distanceM = distanceAtX(x, currentView)
      if (distanceM >= 0 && distanceM <= totalDistanceM.value) {
        const entranceKm = tunnelCreation.value.entranceKm
        const exitKm = Math.max(
          entranceKm + MIN_TUNNEL_LENGTH_KM,
          Math.min(totalKm.value, distanceM / 1000),
        )
        tunnelCreation.value = { entranceKm, exitKm }
        requestDraw()
      }
      return
    }

    const tunnelDrag = dragTunnelState.value
    if (!tunnelDrag || dragPointerId !== event.pointerId) return

    const rawKm = distanceAtX(x, currentView) / 1000
    const tunnel = tunnels.value.find(
      entry => entry.id === tunnelDrag.tunnelId,
    )
    if (!tunnel) return

    const bounds = constrainTunnelBound(
      tunnel,
      tunnelDrag.handle,
      rawKm,
      tunnels.value,
      totalKm.value,
    )
    const km =
      tunnelDrag.handle === 'entrance' ? bounds.startKm : bounds.endKm
    dragTunnelState.value = { ...tunnelDrag, km }

    if (
      lastDraftTunnelKm === null ||
      Math.abs(lastDraftTunnelKm - km) > 1e-6
    ) {
      lastDraftTunnelKm = km
      emit('draft-tunnel-bounds', {
        tunnelId: tunnelDrag.tunnelId,
        handle: tunnelDrag.handle,
        km,
      })
    }

    event.preventDefault()
    requestDraw()
    return
  }

  if (props.creating && creation.value) {
    const distanceM = distanceAtX(x, currentView)
    if (distanceM >= 0 && distanceM <= totalDistanceM.value) {
      const valleyKm = creation.value.valleyKm
      const summitKm = Math.max(
        valleyKm + MIN_CLIMB_LENGTH_KM,
        Math.min(totalKm.value, distanceM / 1000),
      )
      creation.value = { valleyKm, summitKm }
      requestDraw()
    }
    return
  }

  const drag = dragState.value
  if (!drag || dragPointerId !== event.pointerId) {
    return
  }

  const rawKm = distanceAtX(x, currentView) / 1000
  const climb = climbs.value.find(entry => entry.id === drag.climbId)
  if (!climb) return

  const km = constrainClimbBound(
    climbs.value,
    climb.id,
    drag.handle,
    rawKm,
    totalKm.value,
  )
  dragState.value = { ...drag, km }

  // Aperçu temps réel : le parent projette la nouvelle borne sur la carte et
  // dans les cartes de montée, sans attendre le backend.
  if (lastDraftKm === null || Math.abs(lastDraftKm - km) > 1e-6) {
    lastDraftKm = km
    emit('draft-bounds', { climbId: drag.climbId, handle: drag.handle, km })
  }

  event.preventDefault()
  requestDraw()
}

function onPointerUp(event: PointerEvent): void {
  const canvas = canvasRef.value
  if (!canvas) return

  const tunnelDrag = dragTunnelState.value
  if (tunnelDrag && dragPointerId === event.pointerId) {
    dragTunnelState.value = null
    dragPointerId = null
    lastDraftTunnelKm = null
    if (canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId)
    }

    const tunnel = tunnels.value.find(
      entry => entry.id === tunnelDrag.tunnelId,
    )
    if (tunnel) {
      emit('edit-tunnel-bounds', {
        tunnelId: tunnel.id,
        handle: tunnelDrag.handle,
        km: tunnelDrag.km,
      })
    } else {
      emit('cancel-tunnel-bounds')
    }
    requestDraw()
    return
  }

  const drag = dragState.value
  if (!drag || dragPointerId !== event.pointerId) return

  dragState.value = null
  dragPointerId = null
  lastDraftKm = null
  if (canvas.hasPointerCapture(event.pointerId)) {
    canvas.releasePointerCapture(event.pointerId)
  }

  const climb = climbs.value.find(entry => entry.id === drag.climbId)
  if (climb) {
    // Le fichier affiché porte déjà la borne provisoire : on transmet toujours
    // la position finale, le store ignore un déplacement nul.
    emit('edit-bounds', {
      climbId: climb.id,
      handle: drag.handle,
      km: drag.km,
    })
  } else {
    emit('cancel-bounds')
  }
  requestDraw()
}

function onPointerCancel(event: PointerEvent): void {
  const canvas = canvasRef.value
  if (
    canvas &&
    dragPointerId === event.pointerId &&
    canvas.hasPointerCapture(event.pointerId)
  ) {
    canvas.releasePointerCapture(event.pointerId)
  }
  if (dragTunnelState.value) emit('cancel-tunnel-bounds')
  if (dragState.value) emit('cancel-bounds')
  dragTunnelState.value = null
  dragState.value = null
  dragPointerId = null
  lastDraftKm = null
  lastDraftTunnelKm = null
  requestDraw()
}

function onScroll(): void {
  requestAxesDraw()
}

function onWheel(event: WheelEvent): void {
  const container = scrollContainer.value
  if (!container) return
  container.scrollLeft += event.deltaY + event.deltaX
}

function onYAxisWheel(event: WheelEvent): void {
  const container = scrollContainer.value
  if (!container) return
  container.scrollTop += event.deltaY + event.deltaX
}

function onXAxisWheel(event: WheelEvent): void {
  const container = scrollContainer.value
  if (!container) return
  container.scrollLeft += event.deltaY + event.deltaX
}

function isEditableTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null
  if (!element) return false
  const tag = element.tagName
  return (
    tag === 'INPUT' ||
    tag === 'TEXTAREA' ||
    tag === 'SELECT' ||
    element.isContentEditable
  )
}

function onKeyDown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    if (props.creating) {
      emit('cancel-create')
      return
    }
    if (props.creatingTunnel) {
      emit('cancel-create-tunnel')
      return
    }
  }
  if (event.ctrlKey || event.metaKey || event.altKey) return
  if (isEditableTarget(event.target)) return

  if (event.key === 'x' || event.key === 'X') {
    event.preventDefault()
    cycleXScale()
  } else if (event.key === 'y' || event.key === 'Y') {
    event.preventDefault()
    cycleYScale()
  }
}

function onWindowResize(): void {
  maxPlotHeight.value = computeMaxPlotHeight()
  requestDraw()
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown)
  window.addEventListener('resize', onWindowResize)

  if (rootRef.value) {
    resizeObserver = new ResizeObserver(() => requestDraw())
    resizeObserver.observe(rootRef.value)
  }

  requestDraw()
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeyDown)
  window.removeEventListener('resize', onWindowResize)
  resizeObserver?.disconnect()
  resizeObserver = null

  if (drawFrame) cancelAnimationFrame(drawFrame)
  if (axesFrame) cancelAnimationFrame(axesFrame)
  drawFrame = 0
  axesFrame = 0
})

watch(
  [() => props.file, xScale, yScale, () => appStore.isDarkMode],
  () => requestDraw(),
)

watch(
  () => props.selectedClimbId,
  () => requestDraw(),
)

watch(
  () => props.creating,
  creating => {
    if (creating) {
      xScale.value = 50
      const container = scrollContainer.value
      if (container) container.scrollLeft = 0
      creation.value = null
    } else {
      creation.value = null
    }
    requestDraw()
  },
)

watch(
  () => props.creatingTunnel,
  creating => {
    if (creating) {
      xScale.value = 50
      const container = scrollContainer.value
      if (container) container.scrollLeft = 0
      tunnelCreation.value = null
    } else {
      tunnelCreation.value = null
    }
    requestDraw()
  },
)

watch(
  [() => props.mode, () => props.tunnels, () => props.selectedTunnelIds],
  () => requestDraw(),
)

watch(
  () => props.mode,
  () => {
    dragState.value = null
    dragTunnelState.value = null
    dragPointerId = null
    lastDraftKm = null
    lastDraftTunnelKm = null
    creation.value = null
    tunnelCreation.value = null
    requestDraw()
  },
)

defineExpose({
  scrollToRange,
})
</script>

<style scoped>
.alt-profile {
  position: absolute;
  left: 16px;
  right: 16px;
  bottom: 16px;
  z-index: 5;
  display: flex;
  flex-direction: column;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  border-radius: 10px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  overflow: hidden;
}

.alt-profile__body {
  display: grid;
  grid-template-columns: 54px minmax(0, 1fr);
  grid-template-rows: auto 26px;
  min-height: 0;
}

.alt-profile__y-axis {
  grid-column: 1;
  grid-row: 1 / span 2;
  position: relative;
  overflow: hidden;
  cursor: ns-resize;
  user-select: none;
}

.alt-profile__scroll {
  grid-column: 2;
  grid-row: 1;
  min-height: 0;
  overflow: auto;
  overscroll-behavior: contain;
  scrollbar-width: thin;
}

/* Le défilement horizontal se fait à la molette ; la barre verticale reste. */
.alt-profile__scroll::-webkit-scrollbar {
  width: 8px;
  height: 0;
}

.alt-profile__scroll::-webkit-scrollbar-thumb {
  background: rgba(var(--v-theme-on-surface), 0.22);
  border-radius: 4px;
}

.alt-profile__x-axis {
  grid-column: 2;
  grid-row: 2;
  position: relative;
  overflow: hidden;
  cursor: ew-resize;
  user-select: none;
}

.alt-profile__axis-canvas {
  display: block;
}

.alt-profile__canvas {
  display: block;
  cursor: crosshair;
  touch-action: none;
}

.alt-profile__canvas--dragging {
  cursor: ew-resize;
}

.alt-profile__hint {
  position: absolute;
  top: 6px;
  left: 62px;
  z-index: 3;
  padding: 4px 8px;
  border-radius: 6px;
  background: rgba(var(--v-theme-surface), 0.92);
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  color: rgba(var(--v-theme-on-surface), 0.78);
  font-size: 0.7rem;
  pointer-events: none;
}
</style>
