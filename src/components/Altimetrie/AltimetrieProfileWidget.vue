<template>
  <div class="alt-profile">
    <div class="alt-profile__header">
      <div class="alt-profile__heading">
        <span class="alt-profile__title">{{ title }}</span>
        <span class="alt-profile__hint">{{ hint }}</span>
      </div>

      <div class="alt-profile__scales">
        <v-select
          v-model="xScale"
          :items="xScaleItems"
          label="X (m/px)"
          density="compact"
          variant="outlined"
          hide-details
          class="alt-profile__scale"
        />
        <v-select
          v-model="yScale"
          :items="yScaleItems"
          label="Y (m/px)"
          density="compact"
          variant="outlined"
          hide-details
          class="alt-profile__scale"
        />
        <v-btn
          v-if="creating"
          variant="text"
          size="small"
          color="warning"
          @click="emit('cancel-create')"
        >
          Annuler
        </v-btn>
        <v-btn
          v-else-if="mode === 'climb'"
          icon="mdi-close"
          variant="text"
          size="small"
          title="Revenir au profil complet"
          @click="returnToFull"
        />
      </div>
    </div>

    <div ref="scrollContainer" class="alt-profile__scroll" @wheel.prevent="onWheel">
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
  </div>
</template>

<script setup lang="ts">
/**
 * Widget de profil altimétrique flottant.
 *
 * Le canvas est dessiné à la main, comme le prototype validé :
 * - repères Y (lignes pointillées) et X (distances) ;
 * - profil coloré par segments de pente ;
 * - triangles vallée/sommet avec traits pointillés ;
 * - traits horizontaux des montées sur l'axe X.
 *
 * Interactions :
 * - molette : défilement horizontal ;
 * - clic sur le profil : recentrage de la carte ;
 * - clic sur un trait de montée : sélection et cadrage carte ;
 * - clic sur un triangle : sélection et passage en échelle détaillée 12,5 m/px ;
 * - en échelle détaillée, drag des triangles : aperçu des nouvelles bornes,
 *   envoyé au parent au relâchement.
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
  MIN_CLIMB_LENGTH_KM,
  type ClimbHandle,
} from '../../utils/altimetrieClimbs'
import { formatDistance } from '../../utils/format'

const props = defineProps<{
  file: AltimetrieFile | null
  selectedClimbId: number | null
  /** Mode création manuelle : le prochain clic pose la vallée, le suivant le sommet. */
  creating: boolean
}>()

const emit = defineEmits<{
  'select-climb': [id: number]
  recenter: [distanceM: number]
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
}>()

const LEFT_PAD = 46
const RIGHT_PAD = 10
const TOP_PAD = 24
const BOTTOM_PAD = 26
const MIN_HEIGHT = 140
const HANDLE_HIT_Y = 22
const HANDLE_HIT_X = 14

interface WidgetView {
  leftPad: number
  xScale: number
  axisY: number
}

interface DragState {
  climbId: number
  handle: ClimbHandle
  km: number
}

interface HandleHit {
  climb: AltimetrieClimb
  handle: ClimbHandle
}

const appStore = useAppStore()

const canvasRef = ref<HTMLCanvasElement | null>(null)
const scrollContainer = ref<HTMLDivElement | null>(null)
const mode = ref<'full' | 'climb'>('full')
const xScale = ref(50)
const yScale = ref(10)
const dragState = ref<DragState | null>(null)
/** Création manuelle en cours : bornes de vallée et aperçu de sommet. */
const creation = ref<{ valleyKm: number; summitKm: number } | null>(null)

let currentView: WidgetView | null = null
let drawFrame = 0
let dragPointerId: number | null = null
/** Dernière borne transmise pendant le drag, pour éviter les doublons. */
let lastDraftKm: number | null = null

const profile = computed(() => props.file?.profile ?? [])
const climbs = computed(() => props.file?.climbs ?? [])
const segments = computed(() => props.file?.segments ?? [])

const selectedClimb = computed(
  () => climbs.value.find(climb => climb.id === props.selectedClimbId) ?? null,
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

const xScaleItems = computed(() =>
  scaleValues(
    mode.value === 'climb' ? 2.5 : 5,
    mode.value === 'climb' ? 50 : 100,
    mode.value === 'climb' ? 2.5 : 5,
  ),
)

const yScaleItems = scaleValues(2.5, 50, 2.5)

const title = computed(() => {
  if (props.creating) {
    return creation.value
      ? 'Créer une montée — poser le sommet'
      : 'Créer une montée — poser la vallée'
  }
  return mode.value === 'climb' && selectedClimb.value
    ? `Montée #${selectedClimb.value.index + 1} — ajuster les bornes`
    : 'Profil altimétrique complet'
})

const hint = computed(() => {
  if (props.creating) {
    return creation.value
      ? 'Cliquez sur le profil pour poser le sommet · Échap pour annuler'
      : "Cliquez sur le profil pour poser la vallée, en dehors d'une montée existante"
  }
  return mode.value === 'climb'
    ? 'Glissez les triangles pour ajuster les bornes · molette : défilement horizontal'
    : 'Cliquez pour recentrer la carte · cliquez un triangle pour ajuster une montée'
})

function scaleValues(min: number, max: number, step: number): number[] {
  const values: number[] = []
  for (let value = min; value <= max + 1e-9; value += step) {
    values.push(Math.round(value * 10) / 10)
  }
  return values
}

function niceStep(raw: number): number {
  if (!Number.isFinite(raw) || raw <= 0) return 1
  const exponent = Math.floor(Math.log10(raw))
  const fraction = raw / 10 ** exponent
  const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10
  return nice * 10 ** exponent
}

function lowerBound(profilePoints: AltimetrieProfilePoint[], targetM: number): number {
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

function requestDraw(): void {
  if (drawFrame) return
  drawFrame = requestAnimationFrame(() => {
    drawFrame = 0
    draw()
  })
}

function draw(): void {
  const canvas = canvasRef.value
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return

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

  const dark = appStore.isDarkMode
  const background = dark ? '#0f172a' : '#f8fafc'
  const gridColor = dark ? 'rgba(148,163,184,0.38)' : 'rgba(100,116,139,0.28)'
  const textColor = dark ? 'rgba(226,232,240,0.78)' : 'rgba(51,65,85,0.82)'
  const lineColor = dark ? '#e2e8f0' : '#1e293b'
  const categoryOutline = dark
    ? 'rgba(255,255,255,0.22)'
    : 'rgba(0,0,0,0.14)'

  const altMin = altBounds.value.min
  const altMax = altBounds.value.max
  const range = Math.max(altMax - altMin, 1)
  const plotHeight = range / yScale.value
  const width = Math.ceil(totalM / xScale.value) + LEFT_PAD + RIGHT_PAD
  const height = Math.max(
    MIN_HEIGHT,
    Math.ceil(plotHeight) + TOP_PAD + BOTTOM_PAD,
  )
  const axisY = TOP_PAD + plotHeight
  const dpr = window.devicePixelRatio || 1

  canvas.width = Math.max(1, Math.floor(width * dpr))
  canvas.height = Math.max(1, Math.floor(height * dpr))
  canvas.style.width = `${width}px`
  canvas.style.height = `${height}px`
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, width, height)
  ctx.fillStyle = background
  ctx.fillRect(0, 0, width, height)

  const xOf = (distanceM: number) => LEFT_PAD + distanceM / xScale.value
  const yOf = (ele: number) => TOP_PAD + (altMax - ele) / yScale.value
  currentView = { leftPad: LEFT_PAD, xScale: xScale.value, axisY }

  // Repères Y
  ctx.save()
  ctx.font = '10px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
  ctx.textBaseline = 'middle'
  ctx.textAlign = 'left'
  ctx.lineWidth = 1
  ctx.strokeStyle = gridColor
  ctx.fillStyle = textColor
  ctx.setLineDash([3, 4])
  const yStep = niceStep(yScale.value * 32)
  let drewGrid = false
  for (
    let alt = Math.ceil(altMin / yStep) * yStep;
    alt <= altMax + 1e-9;
    alt += yStep
  ) {
    const y = yOf(alt)
    ctx.beginPath()
    ctx.moveTo(LEFT_PAD, y)
    ctx.lineTo(width - RIGHT_PAD, y)
    ctx.stroke()
    ctx.fillText(`${Math.round(alt)} m`, 4, y)
    drewGrid = true
  }
  if (!drewGrid) {
    ctx.fillText(`${Math.round(altMax)} m`, 4, yOf(altMax))
    ctx.fillText(`${Math.round(altMin)} m`, 4, yOf(altMin))
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
    ctx.strokeStyle = categoryOutline
    ctx.lineWidth = 0.8
    ctx.stroke()
  }

  // Bande de la montée sélectionnée (avec aperçu de drag)
  const selected = selectedClimb.value
  if (selected) {
    const drag =
      dragState.value?.climbId === selected.id ? dragState.value : null
    const startKm = drag?.handle === 'valley' ? drag.km : selected.startKm
    const endKm = drag?.handle === 'summit' ? drag.km : selected.endKm
    const x1 = xOf(Math.min(startKm, endKm) * 1000)
    const x2 = xOf(Math.max(startKm, endKm) * 1000)
    ctx.fillStyle = dark
      ? 'rgba(250,204,21,0.12)'
      : 'rgba(250,204,21,0.18)'
    ctx.fillRect(x1, TOP_PAD, Math.max(1, x2 - x1), axisY - TOP_PAD)
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
  ctx.strokeStyle = lineColor
  ctx.lineWidth = 1.4
  ctx.stroke()

  // Axe X
  ctx.beginPath()
  ctx.moveTo(LEFT_PAD, axisY)
  ctx.lineTo(width - RIGHT_PAD, axisY)
  ctx.strokeStyle = gridColor
  ctx.lineWidth = 1
  ctx.setLineDash([])
  ctx.stroke()

  // Graduations X
  ctx.save()
  ctx.font = '10px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'top'
  ctx.fillStyle = textColor
  const xStep = niceStep(xScale.value * 110)
  for (let distance = 0; distance <= totalM + 1e-9; distance += xStep) {
    const x = xOf(distance)
    ctx.beginPath()
    ctx.moveTo(x, axisY)
    ctx.lineTo(x, axisY + 4)
    ctx.strokeStyle = gridColor
    ctx.lineWidth = 1
    ctx.stroke()
    ctx.fillText(formatDistance(distance), x, axisY + 6)
  }
  ctx.restore()

  // Traits horizontaux des montées, au niveau de l'axe X
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
      ? dark
        ? '#ffffff'
        : '#1e293b'
      : categoryOutline
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
      axisY,
      dark,
    )
    drawProfileHandle(
      ctx,
      xOf(summitKm * 1000),
      '#dc2626',
      isSelected,
      axisY,
      dark,
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
      TOP_PAD,
      Math.max(1, Math.abs(summitX - valleyX)),
      axisY - TOP_PAD,
    )
    drawProfileHandle(ctx, valleyX, '#16a34a', true, axisY, dark)
    drawProfileHandle(
      ctx,
      summitX,
      valid ? '#dc2626' : '#ef4444',
      true,
      axisY,
      dark,
    )
  }

  // Centrage automatique sur la montée sélectionnée, hors drag.
  if (
    mode.value === 'climb' &&
    selected &&
    !dragState.value &&
    scrollContainer.value
  ) {
    const centerM = ((selected.startKm + selected.endKm) / 2) * 1000
    const centerX = xOf(centerM)
    const target = Math.max(
      0,
      centerX - scrollContainer.value.clientWidth / 2,
    )
    requestAnimationFrame(() => {
      if (scrollContainer.value) scrollContainer.value.scrollLeft = target
    })
  }
}

function drawProfileHandle(
  ctx: CanvasRenderingContext2D,
  x: number,
  color: string,
  selected: boolean,
  axisY: number,
  dark: boolean,
): void {
  const size = selected ? 8 : 5
  const top = 2
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
  if (!currentView || y > HANDLE_HIT_Y) return null

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

function handleCreationClick(x: number, y: number): void {
  if (!currentView || y < TOP_PAD || y > currentView.axisY) return

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

  if (props.creating) {
    handleCreationClick(x, y)
    return
  }

  const handleHit = hitHandle(x, y)
  if (handleHit) {
    emit('select-climb', handleHit.climb.id)

    if (mode.value === 'full') {
      mode.value = 'climb'
      xScale.value = 12.5
      requestDraw()
      return
    }

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

  const distanceM = distanceAtX(x, currentView)
  if (distanceM >= 0 && distanceM <= totalDistanceM.value) {
    emit('recenter', distanceM)
  }
}

function onPointerMove(event: PointerEvent): void {
  const canvas = canvasRef.value
  if (canvas && props.creating && creation.value && currentView) {
    const rect = canvas.getBoundingClientRect()
    const x = event.clientX - rect.left
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
  if (
    !canvas ||
    !drag ||
    !currentView ||
    dragPointerId !== event.pointerId
  ) {
    return
  }

  const rect = canvas.getBoundingClientRect()
  const x = event.clientX - rect.left
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
  const drag = dragState.value
  if (!canvas || !drag || dragPointerId !== event.pointerId) return

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
  if (dragState.value) emit('cancel-bounds')
  dragState.value = null
  dragPointerId = null
  lastDraftKm = null
  requestDraw()
}

function onWheel(event: WheelEvent): void {
  const container = scrollContainer.value
  if (!container) return
  container.scrollLeft += event.deltaY + event.deltaX
}

function returnToFull(): void {
  mode.value = 'full'
  xScale.value = 50
  const container = scrollContainer.value
  if (container) container.scrollLeft = 0
  requestDraw()
}

function onKeyDown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && props.creating) {
    emit('cancel-create')
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown)
  requestDraw()
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeyDown)
  if (drawFrame) cancelAnimationFrame(drawFrame)
  drawFrame = 0
})

watch(
  [() => props.file, xScale, yScale, mode, () => appStore.isDarkMode],
  () => requestDraw(),
)

watch(
  () => props.selectedClimbId,
  id => {
    if (id === null) {
      mode.value = 'full'
      xScale.value = 50
      const container = scrollContainer.value
      if (container) container.scrollLeft = 0
    }
    requestDraw()
  },
)

watch(
  () => props.creating,
  creating => {
    if (creating) {
      mode.value = 'full'
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
  max-height: 45vh;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  border-radius: 10px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  overflow: hidden;
}

.alt-profile__header {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 0 0 auto;
  flex-wrap: wrap;
  padding: 8px 12px;
  border-bottom: 1px solid rgba(var(--v-theme-on-surface), 0.1);
}

.alt-profile__heading {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1 1 260px;
}

.alt-profile__title {
  font-size: 0.85rem;
  font-weight: 600;
}

.alt-profile__hint {
  font-size: 0.68rem;
  color: rgba(var(--v-theme-on-surface), 0.6);
}

.alt-profile__scales {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
}

.alt-profile__scale {
  width: 108px;
}

.alt-profile__scroll {
  overflow: auto;
  max-height: calc(45vh - 64px);
}

.alt-profile__canvas {
  display: block;
  cursor: crosshair;
  touch-action: none;
}

.alt-profile__canvas--dragging {
  cursor: ew-resize;
}
</style>
