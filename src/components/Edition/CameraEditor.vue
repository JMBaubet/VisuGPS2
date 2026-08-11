<template>
  <div v-if="editionStore.hasKeyframes" class="camera-editor">
    <!-- Mode 1 : le curseur n'est PAS sur un point de RdV → bouton Ajouter. -->
    <div v-if="!isOnRdv" class="add-rdv">
      <v-btn
        size="small"
        color="primary"
        variant="flat"
        prepend-icon="mdi-plus"
        @click="addKeyframeHere"
      >
        Ajouter un point de RdV
      </v-btn>
    </div>

    <!-- Mode 2 : le curseur est sur un point de RdV → widgets + actions. -->
    <template v-else>
      <!-- Croix de visée bleue au centre de l'écran (visible en mode cible). -->
      <div v-if="targetingMode" class="crosshair" />

      <!-- Bloc droit : switch Cible (à gauche) + sliders Pitch/Zoom (à droite) -->
      <div class="right-stack">
        <!-- Switch mode visée (hors du bloc sliders pour ne pas être étiré) -->
        <div class="switch-col">
          <span class="slider-label">Cible</span>
          <v-switch
            v-model="targetingMode"
            color="primary"
            hide-details
            density="compact"
            class="target-switch"
          />
          <span class="slider-value">{{ targetingMode ? 'ON' : 'OFF' }}</span>
        </div>

        <div class="sliders-block">
          <div
            class="slider-col"
            @wheel.prevent="onSliderWheel($event, 'pitch')"
          >
            <span class="slider-label">Pitch</span>
            <div
              ref="pitchTrackEl"
              class="vs-track"
              @mousedown="onSliderStart($event, 'pitch')"
            >
              <div class="vs-fill" :style="{ height: pitchFillPct + '%' }" />
              <div class="vs-thumb" :style="{ bottom: pitchFillPct + '%' }" />
            </div>
            <span class="slider-value">{{ pitchModel?.toFixed(0) }}°</span>
          </div>
          <div
            class="slider-col"
            @wheel.prevent="onSliderWheel($event, 'zoom')"
          >
            <span class="slider-label">Zoom</span>
            <div
              ref="zoomTrackEl"
              class="vs-track"
              @mousedown="onSliderStart($event, 'zoom')"
            >
              <div class="vs-fill" :style="{ height: zoomFillPct + '%' }" />
              <div class="vs-thumb" :style="{ bottom: zoomFillPct + '%' }" />
            </div>
            <span class="slider-value">{{ zoomModel?.toFixed(1) }}</span>
          </div>
        </div>
      </div>

      <!-- CompassBandeau (coin inférieur droit) : cap ±90°, drag + molette -->
      <div
        ref="compassEl"
        class="compass-bandeau"
        @mousedown="onCompassDragStart"
        @wheel.prevent="onCompassWheel"
      >
        <div class="compass-tick" />
        <div
          class="compass-strip"
          :style="{ transform: `translateX(${compassTranslateX}px)` }"
        >
          <template v-for="mark in compassMarks" :key="mark.deg">
            <span
              class="compass-mark"
              :class="{ 'compass-mark-major': mark.major }"
              :style="{ left: mark.x + 'px' }"
            >
              {{ mark.label }}
            </span>
          </template>
        </div>
      </div>

      <!-- Barre d'actions du point de RdV -->
      <div class="kf-actions">
        <v-btn
          size="small"
          variant="text"
          color="white"
          :disabled="!dirty"
          title="Annuler les modifications"
          @click="undoKeyframe"
        >
          <v-icon>mdi-undo</v-icon>
          <span class="action-label">Undo</span>
        </v-btn>
        <v-btn
          size="small"
          color="error"
          variant="text"
          title="Supprimer ce point de RdV"
          @click="removeKeyframeHere"
        >
          <v-icon>mdi-delete</v-icon>
          <span class="action-label">Supprimer</span>
        </v-btn>
        <v-btn
          size="small"
          color="primary"
          variant="flat"
          :disabled="!dirty"
          title="Sauvegarder les modifications"
          @click="saveKeyframe"
        >
          <v-icon>mdi-content-save</v-icon>
          <span class="action-label">Sauvegarder</span>
        </v-btn>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * Composant B — Édition des keyframes (spec « Interface de contrôle MapBox »).
 *
 * L'affichage est **piloté par la position de lecture** (`currentKeyframe`) :
 *   - le curseur n'est **pas sur un point de RdV** → un seul bouton
 *     « Ajouter un point de RdV » ;
 *   - le curseur est **sur un point de RdV** → widgets d'édition (switch
 *     Cible, sliders Pitch/Zoom, CompassBandeau) + barre d'actions
 *     (Undo / Supprimer / Sauvegarder).
 *
 * Widgets de manipulation directe, pour un usage exclusif à la souris :
 *   - **Switch Cible** : passe le pitch à 0° (vue du dessus), affiche une
 *     croix bleue au centre et permet de déplacer le point observé
 *     (cam.lng/lat) en drag & drop sur toute la carte.
 *   - **PitchSlider** / **ZoomSlider** : sliders verticaux (bord droit).
 *   - **CompassBandeau** : bandeau horizontal (coin bas droit) — drag
 *     horizontal et molette (±1°) pour le cap. Le cap courant reste centré
 *     sur le repère central fixe (trait rouge) ; la bande défile en continu.
 *
 * La sauvegarde est **explicite** : toute modification marque l'état
 * `dirty` (bouton Sauvegarder dégrisé) ; le bouton Sauvegarder persiste
 * (`saveKeyframes`), le bouton Undo restaure l'état précédent.
 */
import { computed, ref, watch, onUnmounted } from 'vue'
import type { CamState } from '../../algorithms/keyframeGenerator'
import { useEditionStore } from '../../stores/edition'
import { useEditionMap } from '../../composables/useEditionMap'

const editionStore = useEditionStore()
const { map } = useEditionMap()

// --- Point de RdV courant (dépend de la position du curseur) ---

const currentKeyframe = computed(() => editionStore.currentKeyframe)
const isOnRdv = computed(() => !!currentKeyframe.value)

/** Distance (m) du point de RdV courant, ou null hors RdV. */
const kfDistance = computed(() => currentKeyframe.value?.distance_from_start_m ?? null)

// --- Sliders Pitch / Zoom ---

const pitchModel = ref(60)
const zoomModel = ref(16)

/** Sync des sliders quand on arrive sur un point de RdV. */
watch(
  currentKeyframe,
  () => {
    const kf = currentKeyframe.value
    if (kf) {
      pitchModel.value = kf.cam.pitch
      zoomModel.value = kf.cam.zoom
    }
  },
  { immediate: true },
)

// --- Sauvegarde explicite / dirty / undo ---

/**
 * Instantané du `cam` du point de RdV courant, pris à l'arrivée sur le RdV
 * ou après une sauvegarde. Sert de référence au bouton Undo.
 */
let baselineCam: CamState | null = null

/** `true` dès qu'un paramètre du point de RdV courant a été modifié. */
const dirty = ref(false)

/** Capture la baseline à l'arrivée sur un nouveau point de RdV. */
watch(
  currentKeyframe,
  () => {
    const kf = currentKeyframe.value
    baselineCam = kf ? { ...kf.cam } : null
    dirty.value = false
  },
  { immediate: true },
)

/** Restaure l'état précédent (baseline) sur la carte et dans le keyframe. */
function undoKeyframe() {
  const kf = currentKeyframe.value
  if (!kf || !baselineCam) return
  editionStore.updateKeyframe(kf.distance_from_start_m, { ...baselineCam })
  applyCamToMap(baselineCam)
  dirty.value = false
}

/** Sauvegarde explicitement le jeu de keyframes et rafraîchit la baseline. */
function saveKeyframe() {
  editionStore.saveKeyframes()
  const kf = currentKeyframe.value
  baselineCam = kf ? { ...kf.cam } : null
  dirty.value = false
}

/** Applique un état caméra à la carte (pour Undo). */
function applyCamToMap(cam: CamState) {
  const m = map.value
  if (!m) return
  m.jumpTo({ center: [cam.lng, cam.lat], zoom: cam.zoom, bearing: cam.bearing, pitch: cam.pitch })
}

/** Ratio 0-100 (height %) du remplissage pour le slider custom. */
const pitchFillPct = computed(() => (pitchModel.value / 85) * 100)
const zoomFillPct = computed(() => (zoomModel.value / 22) * 100)

// --- Sliders verticaux customs (Pitch / Zoom) ---

const pitchTrackEl = ref<HTMLDivElement | null>(null)
const zoomTrackEl = ref<HTMLDivElement | null>(null)

type SliderKind = 'pitch' | 'zoom'

let sliderDragging: SliderKind | null = null

/** Calcule la valeur normalisée d'un slider selon la position Y de la souris. */
function sliderValueFromY(
  trackEl: HTMLElement,
  kind: SliderKind,
  clientY: number,
): number {
  const rect = trackEl.getBoundingClientRect()
  const ratio = 1 - Math.min(1, Math.max(0, (clientY - rect.top) / rect.height))
  if (kind === 'pitch') return Math.round(ratio * 85)
  return Math.round(ratio * 22 * 2) / 2 // pas 0.5
}

function onSliderStart(event: MouseEvent, kind: SliderKind) {
  if (!map.value) return
  const trackEl = kind === 'pitch' ? pitchTrackEl.value : zoomTrackEl.value
  if (!trackEl) return
  sliderDragging = kind
  map.value.dragPan.disable()
  applySliderValue(kind, sliderValueFromY(trackEl, kind, event.clientY))
  window.addEventListener('mousemove', onSliderMove)
  window.addEventListener('mouseup', onSliderEnd)
}

function onSliderMove(event: MouseEvent) {
  if (!sliderDragging || !map.value) return
  const trackEl = sliderDragging === 'pitch' ? pitchTrackEl.value : zoomTrackEl.value
  if (!trackEl) return
  applySliderValue(sliderDragging, sliderValueFromY(trackEl, sliderDragging, event.clientY))
}

function onSliderEnd() {
  sliderDragging = null
  window.removeEventListener('mousemove', onSliderMove)
  window.removeEventListener('mouseup', onSliderEnd)
  syncMapInteractions()
}

/** Applique la valeur à la carte + au store (pitch ou zoom). */
function applySliderValue(kind: SliderKind, v: number) {
  const kfDist = kfDistance.value
  if (kfDist === null) return
  if (kind === 'pitch') {
    const clamped = Math.min(85, Math.max(0, v))
    pitchModel.value = clamped
    map.value?.setPitch(clamped)
    editionStore.updateKeyframe(kfDist, { pitch: clamped })
  } else {
    const clamped = Math.min(22, Math.max(0, v))
    zoomModel.value = clamped
    map.value?.setZoom(clamped)
    editionStore.updateKeyframe(kfDist, { zoom: clamped })
  }
  dirty.value = true
}

/**
 * Molette sur un slider (spec §6.2) : ±1 pas par cran
 * (pitch 1°, zoom 0.5). `prevent` pour ne pas zoomer la carte.
 */
function onSliderWheel(event: WheelEvent, kind: SliderKind) {
  const delta = event.deltaY > 0 ? -1 : 1
  if (kind === 'pitch') {
    applySliderValue(kind, (pitchModel.value ?? 0) + delta)
  } else {
    applySliderValue(kind, (zoomModel.value ?? 0) + delta * 0.5)
  }
}

// --- Mode visée (v-switch Cible) ---

const targetingMode = ref(false)
/** Handler moveend Mapbox pour le mode visée (sauve le centre vers le keyframe). */
function onTargetingMoveEnd() {
  const m = map.value
  const kfDist = kfDistance.value
  if (!m || kfDist === null) return
  const c = m.getCenter()
  editionStore.updateKeyframe(kfDist, { lng: c.lng, lat: c.lat })
  dirty.value = true
}

/**
 * Active ou désactive **toutes** les interactions souris de la carte
 * (drag, molette, double-clic, rotation, boîte). Quand on est sur un point
 * de RdV, la carte est verrouillée tant que le bouton Cible n'est pas actif :
 * le seul moyen de la déplacer est alors le mode visée.
 */
function setMapInteractions(enabled: boolean) {
  const m = map.value
  if (!m) return
  const controls = [
    m.dragPan,
    m.scrollZoom,
    m.doubleClickZoom,
    m.dragRotate,
    m.boxZoom,
    m.touchZoomRotate,
    m.touchPitch,
  ]
  for (const ctrl of controls) {
    if (!ctrl) continue
    if (enabled) ctrl.enable()
    else ctrl.disable()
  }
}

/**
 * Réapplique l'état des interactions selon le mode courant :
 * carte libre hors RdV, verrouillée sur un RdV si Cible est OFF.
 */
function syncMapInteractions() {
  setMapInteractions(!currentKeyframe.value || targetingMode.value)
}

watch(
  [currentKeyframe, targetingMode],
  ([kf, target]) => {
    const m = map.value
    if (!m) return
    if (!kf) {
      // Hors RdV (mode ajout) : navigation libre.
      m.off('moveend', onTargetingMoveEnd)
      targetingMode.value = false
      setMapInteractions(true)
      return
    }
    if (target) {
      // Mode visée : vue du dessus + déplacement libre (drag → moveend).
      m.easeTo({ pitch: 0 })
      m.on('moveend', onTargetingMoveEnd)
      setMapInteractions(true)
    } else {
      // Édition verrouillée : interactions off, pitch du keyframe restauré.
      m.off('moveend', onTargetingMoveEnd)
      m.easeTo({ pitch: kf.cam.pitch })
      setMapInteractions(false)
    }
  },
  { immediate: true },
)

// --- CompassBandeau ---

/** Largeur du bandeau (px) — champ visible de ±90°. */
const COMPASS_WIDTH_PX = 360
/** Échelle de défilement : 2 px par degré (bande totale 720 px pour 360°). */
const PX_PER_DEG = 2
/** Position (px dans la bande) du repère central. */
const CENTER_PX = COMPASS_WIDTH_PX / 2

/** Génère une graduation tous les 15° (majeure tous les 45°), absolue 0..345°. */
const compassMarks = computed(() => {
  const marks: { deg: number; x: number; label: string; major: boolean }[] = []
  for (let deg = 0; deg < 360; deg += 15) {
    marks.push({
      deg,
      x: deg * PX_PER_DEG,
      label: compassLabel(deg),
      major: deg % 45 === 0,
    })
  }
  return marks
})

/** Libellé d'une graduation : point cardinal si multiple de 45°, sinon « deg° ». */
function compassLabel(deg: number): string {
  const cardinals: Record<number, string> = {
    0: 'N', 45: 'NE', 90: 'E', 135: 'SE',
    180: 'S', 225: 'SO', 270: 'O', 315: 'NO',
  }
  return cardinals[deg] ?? `${deg}°`
}

/**
 * Translation X de la bande pour garder le cap courant sur le repère central :
 * la graduation absolue `deg = cap` doit tomber à `CENTER_PX` px.
 */
const compassTranslateX = computed(() => {
  const kf = currentKeyframe.value
  if (!kf) return 0
  const bearing = ((kf.cam.bearing % 360) + 360) % 360
  return CENTER_PX - bearing * PX_PER_DEG
})

let compassDragging = false
let compassStartX = 0
let compassStartBearing = 0

function onCompassDragStart(event: MouseEvent) {
  if (!map.value) return
  compassDragging = true
  compassStartX = event.clientX
  compassStartBearing = map.value.getBearing()
  map.value.dragPan.disable()
  window.addEventListener('mousemove', onCompassDragMove)
  window.addEventListener('mouseup', onCompassDragEnd)
}

function onCompassDragMove(event: MouseEvent) {
  if (!compassDragging || !map.value) return
  const dx = event.clientX - compassStartX // cumulé depuis le début du drag
  applyBearing(compassStartBearing + dx)
}

function onCompassDragEnd() {
  compassDragging = false
  window.removeEventListener('mousemove', onCompassDragMove)
  window.removeEventListener('mouseup', onCompassDragEnd)
  syncMapInteractions()
}

function onCompassWheel(event: WheelEvent) {
  const delta = event.deltaY > 0 ? 1 : -1
  applyBearing((map.value?.getBearing() ?? 0) + delta)
}

/** Applique un bearing (modulo 360) à la carte et au keyframe sélectionné. */
function applyBearing(raw: number) {
  const m = map.value
  const kfDist = kfDistance.value
  if (!m || kfDist === null) return
  const bearingVal = ((raw % 360) + 360) % 360
  m.setBearing(bearingVal)
  editionStore.updateKeyframe(kfDist, { bearing: bearingVal })
  dirty.value = true
}

// --- Ajouter / Supprimer un point de RdV ---

/** Ajoute un point de RdV à la position courante du curseur. */
function addKeyframeHere() {
  editionStore.addKeyframe(editionStore.currentDistanceM)
}

/** Supprime le point de RdV courant (le curseur est dessus). */
function removeKeyframeHere() {
  const dist = kfDistance.value
  if (dist !== null) editionStore.removeKeyframe(dist)
}

// --- Lifecycle : nettoyage des listeners globaux au démontage ---

onUnmounted(() => {
  if (compassDragging) onCompassDragEnd()
  if (sliderDragging) onSliderEnd()
  map.value?.off('moveend', onTargetingMoveEnd)
  // Toujours réactiver les interactions au démontage (ne jamais laisser
  // la carte verrouillée après avoir quitté l'éditeur).
  setMapInteractions(true)
})
</script>

<style scoped>
.camera-editor {
  position: absolute;
  inset: 0;
  z-index: 7;
  pointer-events: none;
}

/* --- Mode hors RdV : bouton « Ajouter un point de RdV » (sous le compas) --- */
.add-rdv {
  position: absolute;
  right: 12px;
  bottom: 12px; /* sous le compas, à la place des boutons d'action */
  pointer-events: auto;
}

/* --- Barre d'actions du point de RdV (bas droite, à gauche du compas) --- */
.kf-actions {
  position: absolute;
  right: 12px;
  bottom: 12px; /* tout en bas, sous le compas */
  display: flex;
  gap: 8px;
  padding: 6px 10px;
  background: rgba(0, 0, 0, 0.7);
  border-radius: 8px;
  pointer-events: auto;
}
.action-label {
  margin-left: 4px;
  font-size: 12px;
}

/* --- Croix de visée (centre écran, mode visée) --- */
.crosshair {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 20px;
  height: 20px;
  pointer-events: none;
}
.crosshair::before,
.crosshair::after {
  content: '';
  position: absolute;
  background: rgb(var(--v-theme-primary));
}
.crosshair::before {
  left: 50%;
  top: 0;
  width: 2px;
  height: 100%;
  transform: translateX(-50%);
}
.crosshair::after {
  top: 50%;
  left: 0;
  height: 2px;
  width: 100%;
  transform: translateY(-50%);
}

/* --- Bloc droit : switch Cible + sliders Pitch/Zoom (au-dessus du compas) --- */
.right-stack {
  position: absolute;
  right: 12px;
  bottom: 112px; /* au-dessus du compas (44px) + boutons (~36px) + gaps */
  display: flex;
  align-items: flex-end; /* le switch et les sliders alignés par le bas */
  gap: 12px;
  pointer-events: auto;
}
.sliders-block {
  display: flex;
  gap: 12px;
}
.slider-col {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  background: rgba(0, 0, 0, 0.6);
  border-radius: 6px;
  padding: 8px 4px;
}
.slider-label {
  color: rgba(255, 255, 255, 0.85);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
.slider-value {
  color: #ffffff;
  font-family: monospace;
  font-size: 11px;
}
/* Colonne du switch Cible : même fond/police que les sliders, hauteur propre. */
.switch-col {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-end;
  gap: 4px;
  background: rgba(0, 0, 0, 0.6);
  border-radius: 6px;
  padding: 8px 6px;
}
/* Le v-switch : hauteur strictement minimale (piste fine, pouce compact). */
.target-switch {
  margin: 0;
  height: 20px;
}
.target-switch :deep(.v-input__control) {
  min-height: 0;
  height: 20px;
}
.target-switch :deep(.v-selection-control) {
  min-height: 0;
  height: 20px;
  justify-content: center;
  padding: 0;
}
.target-switch :deep(.v-selection-control__input) {
  height: 20px;
  width: 32px;
  margin: 0;
}
.target-switch :deep(.v-switch__track) {
  width: 28px;
  height: 14px;
}
.target-switch :deep(.v-switch__thumb) {
  width: 12px;
  height: 12px;
}
/* Sliders verticaux customs (piste + remplissage + pouce). */
.vs-track {
  position: relative;
  width: 8px;
  height: 200px;
  background: rgba(255, 255, 255, 0.15);
  border-radius: 4px;
  cursor: pointer;
  flex-shrink: 0;
}
.vs-fill {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  background: rgb(var(--v-theme-primary));
  border-radius: 4px;
}
.vs-thumb {
  position: absolute;
  left: 50%;
  width: 16px;
  height: 16px;
  margin-left: -8px;
  margin-bottom: -8px;
  background: rgb(var(--v-theme-primary));
  border: 2px solid #ffffff;
  border-radius: 50%;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
}

/* --- CompassBandeau (coin inférieur droit) --- */
.compass-bandeau {
  position: absolute;
  right: 12px;
  bottom: 56px; /* au-dessus des boutons d'action (~36px) + marge */
  width: 360px;
  height: 44px;
  overflow: hidden;
  background: rgba(0, 0, 0, 0.6);
  border-radius: 6px;
  pointer-events: auto;
  cursor: ew-resize;
}
/* Repère central fixe (trait rouge). */
.compass-tick {
  position: absolute;
  top: 0;
  left: 50%;
  width: 2px;
  height: 100%;
  background: #ff5252;
  transform: translateX(-50%);
  z-index: 2;
}
/* Bande défilante des graduations. */
.compass-strip {
  position: absolute;
  top: 0;
  left: 0;
  width: 720px;
  height: 100%;
}
.compass-mark {
  position: absolute;
  top: 26px;
  transform: translateX(-50%);
  color: rgba(255, 255, 255, 0.6);
  font-size: 9px;
  font-family: monospace;
  white-space: nowrap;
}
/* Graduation majeure (45°) : plus haute + ligne. */
.compass-mark-major {
  top: 18px;
  color: #ffffff;
  font-size: 11px;
  font-weight: 600;
}
</style>
