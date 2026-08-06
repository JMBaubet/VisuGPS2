<template>
  <div v-if="editionStore.hasKeyframes" ref="graphContainer" class="progress-graph">
    <svg
      ref="svgEl"
      :viewBox="viewBox"
      preserveAspectRatio="none"
      class="progress-graph-svg"
      @mousemove="onMouseMove"
      @mouseleave="tooltipVisible = false"
      @click="onClick"
    >
      <!-- Zone cliquable (transparente, couvre toute la piste) -->
      <rect x="0" :y="trackY" :width="svgWidth" :height="trackHeight" fill="transparent" />

      <!-- Piste (fond) -->
      <rect
        :x="padL"
        :y="trackY"
        :width="trackWidth"
        :height="trackHeight"
        rx="3"
        fill="rgba(255,255,255,0.15)"
      />

      <!-- Progression (zone parcourue) -->
      <rect
        :x="padL"
        :y="trackY"
        :width="progressWidth"
        :height="trackHeight"
        rx="3"
        fill="#FFD600"
      />

      <!-- Ticks keyframes -->
      <line
        v-for="kf in keyframeTicks"
        :key="kf.distance_from_start_m"
        :x1="padL + kf.x"
        :y1="trackY"
        :x2="padL + kf.x"
        :y2="trackY + trackHeight"
        stroke="rgba(255,255,255,0.5)"
        stroke-width="1"
      />

      <!-- Curseur ▲ -->
      <polygon
        :points="cursorPoints"
        fill="#FFFFFF"
      />

      <!-- Tooltip -->
      <g v-if="tooltipVisible" :transform="tooltipTransform">
        <rect
          x="-60"
          y="6"
          width="120"
          height="24"
          rx="4"
          fill="rgba(0,0,0,0.85)"
        />
        <text
          x="0"
          y="22"
          text-anchor="middle"
          fill="#FFFFFF"
          font-size="11"
          font-family="monospace"
        >{{ tooltipText }}</text>
      </g>

      <!-- Labels distance (0, mi, fin) -->
      <text
        :x="padL"
        :y="trackY + trackHeight + 14"
        fill="rgba(255,255,255,0.6)"
        font-size="10"
        font-family="monospace"
        text-anchor="start"
      >{{ labelStart }}</text>
      <text
        :x="padL + trackWidth / 2"
        :y="trackY + trackHeight + 14"
        fill="rgba(255,255,255,0.6)"
        font-size="10"
        font-family="monospace"
        text-anchor="middle"
      >{{ labelMid }}</text>
      <text
        :x="padL + trackWidth"
        :y="trackY + trackHeight + 14"
        fill="rgba(255,255,255,0.6)"
        font-size="10"
        font-family="monospace"
        text-anchor="end"
      >{{ labelEnd }}</text>
    </svg>
  </div>
</template>

<script setup lang="ts">
/**
 * Graphe SVG d'avancement (spec §4.6).
 *
 * Timeline horizontale affichant :
 *   - une piste de progression (zone parcourue en jaune) ;
 *   - des ticks verticaux à chaque keyframe ;
 *   - un curseur ▲ blanc à la position courante ;
 *   - un tooltip au survol (distance + altitude) ;
 *   - des labels de distance (début, milieu, fin).
 *
 * Interaction : clic sur la piste → seek à la distance correspondante
 * via `editionStore.seekToDistance()`.
 *
 * Responsive : le viewBox SVG est recalculé via ResizeObserver quand la
 * largeur du conteneur change.
 */
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useEditionStore } from '../../stores/edition'

const editionStore = useEditionStore()

// --- Références ---

const graphContainer = ref<HTMLDivElement | null>(null)
const svgEl = ref<SVGSVGElement | null>(null)

let resizeObserver: ResizeObserver | null = null

// --- Dimensions ---

/** Largeur mesurée du conteneur (px). */
const containerWidth = ref(0)

/** Padding gauche/droite (px) — marge pour ne pas écraser les labels. */
const padL = 8
const padR = 8

/** Hauteur de la piste (px). */
const trackHeight = 10
/** Ordonnée de la piste (px) — laissée au-dessus pour le curseur ▲ au-dessus. */
const trackY = 10
/** Hauteur totale du SVG (px) — piste + labels en dessous. */
const svgHeight = trackY + trackHeight + 22

/** Largeur utile de la piste (px, sans padding). */
const trackWidth = computed(() =>
  Math.max(0, containerWidth.value - padL - padR),
)

/** Largeur totale du SVG (px, = conteneur). */
const svgWidth = computed(() => containerWidth.value)

/** ViewBox recalculé au resize. */
const viewBox = computed(() =>
  `0 0 ${svgWidth.value} ${svgHeight}`,
)

// --- Données du store ---

const keyframes = computed(() => editionStore.keyframeSet?.keyframes ?? [])
const totalDistanceM = computed(() => editionStore.totalDistanceM)
const currentDistanceM = computed(() => editionStore.currentDistanceM)
const progressRatio = computed(() => editionStore.progressRatio)

/** Largeur de la zone remplie (progression). */
const progressWidth = computed(() =>
  Math.max(0, trackWidth.value * progressRatio.value),
)

/** Ticks keyframes avec position X pré-calculée. */
const keyframeTicks = computed(() => {
  const kf = keyframes.value
  const total = totalDistanceM.value
  if (total <= 0 || kf.length === 0) return []
  return kf.map(k => ({
    distance_from_start_m: k.distance_from_start_m,
    x: (k.distance_from_start_m / total) * trackWidth.value,
  }))
})

/** Points du curseur ▲ (triangle pointant vers le bas, centré sur la position). */
const cursorPoints = computed(() => {
  const total = totalDistanceM.value
  if (total <= 0) return ''
  const cx = padL + (currentDistanceM.value / total) * trackWidth.value
  const top = 0
  const halfBase = 5
  // Triangle : sommet en bas (trackY), base en haut
  return `${cx - halfBase},${top} ${cx + halfBase},${top} ${cx},${trackY}`
})

// --- Labels distance ---

function fmtKm(m: number): string {
  return (m / 1000).toFixed(2) + ' km'
}

const labelStart = computed(() => fmtKm(0))
const labelMid = computed(() => fmtKm(totalDistanceM.value / 2))
const labelEnd = computed(() => fmtKm(totalDistanceM.value))

// --- Tooltip ---

const tooltipVisible = ref(false)
const tooltipX = ref(0)
const tooltipDistanceM = ref(0)

/** Texte affiché dans le tooltip. */
const tooltipText = computed(() => {
  const dist = tooltipDistanceM.value
  const alt = editionStore.altitudeAtDistance(dist)
  const km = (dist / 1000).toFixed(2)
  if (alt !== null) {
    return `${km} km · ${Math.round(alt).toLocaleString('fr-FR')} m`
  }
  return `${km} km`
})

/** Transform SVG du tooltip (translate X). */
const tooltipTransform = computed(() => `translate(${tooltipX.value}, ${trackY})`)

/**
 * Convertit un clientX en position X dans le SVG (coordonnées viewBox).
 * Tient compte du ratio viewBox/réel si nécessaire.
 */
function clientXToSvgX(clientX: number): number {
  if (!svgEl.value) return 0
  const rect = svgEl.value.getBoundingClientRect()
  const svgW = svgWidth.value
  if (svgW <= 0) return 0
  return ((clientX - rect.left) / rect.width) * svgW
}

/** Convertit une position X (dans le SVG) en distance (m). */
function svgXToDistance(x: number): number {
  const tw = trackWidth.value
  if (tw <= 0) return 0
  const ratio = Math.max(0, Math.min(1, (x - padL) / tw))
  return ratio * totalDistanceM.value
}

function onMouseMove(event: MouseEvent) {
  tooltipX.value = clientXToSvgX(event.clientX)
  tooltipDistanceM.value = svgXToDistance(tooltipX.value)
  tooltipVisible.value = true
}

function onClick(event: MouseEvent) {
  const svgX = clientXToSvgX(event.clientX)
  const dist = svgXToDistance(svgX)
  editionStore.seekToDistance(dist)
}

// --- Cycle de vie ---

function measure() {
  if (graphContainer.value) {
    containerWidth.value = graphContainer.value.clientWidth
  }
}

onMounted(() => {
  measure()
  if (graphContainer.value) {
    resizeObserver = new ResizeObserver(() => measure())
    resizeObserver.observe(graphContainer.value)
  }
})

onUnmounted(() => {
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
})
</script>

<style scoped>
.progress-graph {
  width: 100%;
  padding: 0;
  /* Hauteur fixe : curseur + piste + labels */
  height: 48px;
}

.progress-graph-svg {
  display: block;
  width: 100%;
  height: 100%;
  cursor: pointer;
}
</style>
