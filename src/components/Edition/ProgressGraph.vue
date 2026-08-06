<template>
  <div
    v-show="editionStore.hasKeyframes && !traceLoading"
    ref="graphContainer"
    class="progress-graph"
  >
    <!-- Fenêtre viewport (clipping) avec scroll horizontal -->
    <div
      class="progress-viewport"
      ref="viewportEl"
      @scroll.passive="onManualScroll"
    >
      <div class="progress-content" :style="{ width: timelineWidthPx + 'px' }">
        <svg
          ref="svgEl"
          :viewBox="`0 0 ${timelineWidthPx} ${totalHeight}`"
          :style="{ width: timelineWidthPx + 'px' }"
          class="progress-graph-svg"
          @click="onTimelineClick"
          @mousemove="onMouseMove"
          @mouseleave="onMouseLeave"
        >
          <!-- ZONE 1 : Points de RdV (keyframes) — ticks larges cliquables -->
          <g class="zone-rdv">
            <rect
              v-for="kf in keyframeTicks"
              :key="'rdv-' + kf.distance_from_start_m"
              :x="kf.x - RDV_WIDTH / 2"
              :y="rdvZoneY"
              :width="RDV_WIDTH"
              :height="rdvZoneHeight"
              fill="rgba(255, 214, 0, 0.7)"
              class="rdv-tick"
            />
          </g>

          <!-- Séparateur fin -->
          <line
            :x1="0" :y1="rdvZoneY + rdvZoneHeight"
            :x2="timelineWidthPx" :y2="rdvZoneY + rdvZoneHeight"
            stroke="rgba(255,255,255,0.1)" stroke-width="1"
          />

          <!-- ZONE 2 : Avancement -->
          <g class="zone-advance">
            <!-- Piste (fond) -->
            <rect
              :x="0" :y="advanceZoneY"
              :width="timelineWidthPx" :height="advanceBarHeight"
              fill="rgba(255,255,255,0.12)" rx="2"
            />
            <!-- Jauge jaune (avancement) — z-index inférieur -->
            <rect
              :x="0" :y="advanceZoneY"
              :width="progressWidthPx" :height="advanceBarHeight"
              fill="#FFD600" rx="2"
            />
            <!-- Repères tous les 10 km — z-index supérieur (rendus après la jauge) -->
            <g v-for="mark in kmMarks" :key="'km-' + mark.distance" class="km-mark">
              <line
                :x1="mark.x" :y1="advanceZoneY"
                :x2="mark.x" :y2="advanceZoneY + advanceBarHeight"
                stroke="rgba(255,255,255,0.6)" stroke-width="1"
              />
              <!-- Zone de clic élargie autour du repère (invisible mais cliquable) -->
              <rect
                :x="mark.x - 5" :y="advanceZoneY"
                :width="10" :height="advanceBarHeight"
                fill="transparent" class="km-mark-hit"
              />
            </g>
            <!-- Curseur rouge (3px) — z-index le plus haut dans la zone -->
            <rect
              :x="cursorX - CURSOR_WIDTH / 2"
              :y="advanceZoneY"
              :width="CURSOR_WIDTH"
              :height="advanceBarHeight"
              fill="#FF0000"
            />
          </g>

          <!-- Séparateur fin -->
          <line
            :x1="0" :y1="advanceZoneY + advanceZoneHeight"
            :x2="timelineWidthPx" :y2="advanceZoneY + advanceZoneHeight"
            stroke="rgba(255,255,255,0.1)" stroke-width="1"
          />

          <!-- ZONE 3 : Graduation (libellés tous les 10km, non cliquable) -->
          <g class="zone-grad">
            <text
              v-for="mark in kmMarks"
              :key="'lbl-' + mark.distance"
              :x="mark.x"
              :y="gradZoneY + 14"
              fill="rgba(255,255,255,0.7)"
              font-size="10"
              font-family="monospace"
              text-anchor="middle"
            >{{ mark.label }}</text>
          </g>

          <!-- Tooltip au survol -->
          <g v-if="tooltip.visible" :transform="`translate(${tooltip.x}, ${advanceZoneY + advanceBarHeight})`">
            <rect
              x="-65" y="4" width="130" height="22" rx="4"
              fill="rgba(0,0,0,0.9)" stroke="rgba(255,255,255,0.2)" stroke-width="1"
            />
            <text
              x="0" y="19" text-anchor="middle"
              fill="#FFFFFF" font-size="11" font-family="monospace"
            >{{ tooltip.text }}</text>
          </g>
        </svg>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Graphe SVG d'avancement (spec §4.6).
 *
 * Timeline horizontale dont la **longueur est proportionnelle à la longueur
 * de la trace** : 3 px pour 100 m (soit 30 px/km). Si la timeline dépasse la
 * largeur de la fenêtre, un mécanisme de **scroll automatique centré sur le
 * curseur** entre en jeu.
 *
 * Trois zones distinctes, de haut en bas :
 *   1. **Points de RdV** (keyframes) — ticks larges (3px) cliquables (clic =
 *      seek pour cette itération ; CRUD à venir avec le Composant B) ;
 *   2. **Ligne d'avancement** — piste de fond + jauge jaune (progression) +
 *      repères verticaux tous les 10 km (cliquables pour seek direct) +
 *      curseur rouge (3px). z-index : jauge < repères < curseur ;
 *   3. **Graduation** — libellés « X km » tous les 10 km (non cliquable).
 *
 * Hauteur confortable (~70px) :
 *   - Zone RdV : 22px
 *   - Zone avancement : 28px
 *   - Zone graduation : 20px
 *
 * Échelle : 3 px / 100 m.
 */
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { useEditionStore } from '../../stores/edition'

const editionStore = useEditionStore()

// --- Références ---

const graphContainer = ref<HTMLDivElement | null>(null)
const viewportEl = ref<HTMLDivElement | null>(null)
const svgEl = ref<SVGSVGElement | null>(null)

/**
 * `true` pendant le chargement d'une nouvelle trace (entre selectedTraceId
 * change et keyframeSet chargé). Masque le graphe pour éviter le flash de
 * l'ancienne timeline.
 */
const traceLoading = ref(true)

let resizeObserver: ResizeObserver | null = null

// --- Constantes de mise en page ---

/** Échelle : 3 px pour 100 m (30 px/km). */
const PX_PER_METER = 3 / 100

/** Largeur des ticks Points de RdV (px, cliquables). */
const RDV_WIDTH = 3
/** Largeur du curseur rouge (px). */
const CURSOR_WIDTH = 3
/** Pas des repères / graduation (m). */
const KM_MARK_STEP_M = 10000 // 10 km

// Hauteurs des zones
const rdvZoneHeight = 22
const advanceZoneHeight = 28
const advanceBarHeight = 16
const gradZoneHeight = 20

/** Ordonnées de chaque zone. */
const rdvZoneY = 0
const advanceZoneY = rdvZoneY + rdvZoneHeight
const gradZoneY = advanceZoneY + advanceZoneHeight

/** Hauteur totale du SVG. */
const totalHeight = rdvZoneHeight + advanceZoneHeight + gradZoneHeight

// --- Dimensions du conteneur (viewport) ---

const viewportWidth = ref(0)
/** Largeur totale de la timeline (px, proportionnelle à la trace). */
const timelineWidthPx = computed(() =>
  Math.ceil(editionStore.totalDistanceM * PX_PER_METER),
)

// --- Données dérivées du store ---

const totalDistanceM = computed(() => editionStore.totalDistanceM)
const currentDistanceM = computed(() => editionStore.currentDistanceM)
const progressRatio = computed(() => editionStore.progressRatio)

/** Largeur de la jauge jaune (px). */
const progressWidthPx = computed(() =>
  Math.max(0, timelineWidthPx.value * progressRatio.value),
)

/** Position X du curseur rouge (px). */
const cursorX = computed(() => currentDistanceM.value * PX_PER_METER)

/** Ticks des Points de RdV (keyframes) avec X pré-calculé. */
const keyframeTicks = computed(() => {
  const kf = editionStore.keyframeSet?.keyframes ?? []
  return kf.map(k => ({
    distance_from_start_m: k.distance_from_start_m,
    x: k.distance_from_start_m * PX_PER_METER,
  }))
})

/**
 * Repères tous les 10 km (pour la zone avancement + la graduation).
 * Chaque repère est cliquable pour seek direct.
 */
const kmMarks = computed(() => {
  const total = totalDistanceM.value
  if (total <= 0) return []
  const marks: { distance: number; x: number; label: string }[] = []
  for (let d = 0; d <= total; d += KM_MARK_STEP_M) {
    marks.push({
      distance: d,
      x: d * PX_PER_METER,
      label: `${Math.round(d / 1000)} km`,
    })
  }
  // Toujours inclure la distance finale si elle ne tombe pas sur un pas.
  const last = marks[marks.length - 1]
  if (last && last.distance < total) {
    marks.push({
      distance: total,
      x: total * PX_PER_METER,
      label: `${(total / 1000).toFixed(1)} km`,
    })
  }
  return marks
})

// --- Tooltip ---

const tooltip = ref({ visible: false, x: 0, text: '' })

function clientXToTimelineX(clientX: number): number {
  const svg = svgEl.value
  if (!svg) return 0
  // Utilisation de la matrice de transformation native du SVG : robuste
  // face au scroll, au scaling, au viewBox et au padding. Convertit un
  // point écran (clientX/Y) en coordonnée viewBox (timeline X).
  const ctm = svg.getScreenCTM()
  if (!ctm) return 0
  const pt = svg.createSVGPoint()
  pt.x = clientX
  pt.y = 0
  const transformed = pt.matrixTransform(ctm.inverse())
  return Math.max(0, Math.min(timelineWidthPx.value, transformed.x))
}

function timelineXToDistance(x: number): number {
  return Math.max(0, Math.min(totalDistanceM.value, x / PX_PER_METER))
}

function onMouseMove(event: MouseEvent) {
  const x = clientXToTimelineX(event.clientX)
  const dist = timelineXToDistance(x)
  tooltip.value = {
    visible: true,
    x,
    text: formatTooltip(dist),
  }
}

function onMouseLeave() {
  tooltip.value.visible = false
}

function formatTooltip(distanceM: number): string {
  const km = (distanceM / 1000).toFixed(2)
  const alt = editionStore.altitudeAtDistance(distanceM)
  if (alt !== null) {
    return `${km} km · ${Math.round(alt).toLocaleString('fr-FR')} m`
  }
  return `${km} km`
}

// --- Interactions clic ---

/**
 * Clic sur la timeline → seek à la distance.
 * (Les ticks Points de RdV sont dans le même SVG : un clic sur un tick
 * tombe sur sa distance exacte.)
 */
function onTimelineClick(event: MouseEvent) {
  const x = clientXToTimelineX(event.clientX)
  const dist = timelineXToDistance(x)
  editionStore.seekToDistance(dist)
}

// --- Auto-scroll centré sur le curseur ---

/**
 * Indique si l'utilisateur est en train de scroller manuellement.
 * Pendant ce temps, l'auto-scroll est suspendu.
 */
let userScrolling = false
let userScrollTimer: ReturnType<typeof setTimeout> | null = null

function onManualScroll() {
  userScrolling = true
  if (userScrollTimer) clearTimeout(userScrollTimer)
  // Réactiver l'auto-scroll après 1.5s sans action utilisateur.
  userScrollTimer = setTimeout(() => {
    userScrolling = false
  }, 1500)
}

/**
 * Recentre le viewport sur le curseur rouge si :
 *   - la timeline dépasse le viewport (scroll nécessaire) ;
 *   - l'utilisateur ne scrolle pas manuellement.
 *
 * Le curseur est maintenu à ~30% du viewport depuis le bord gauche, de
 * sorte qu'on voit une longueur de trace devant lui (et non juste le passé).
 */
function autoScroll() {
  const vp = viewportEl.value
  if (!vp || userScrolling) return
  if (timelineWidthPx.value <= vp.clientWidth) return // pas de scroll nécessaire

  const target = cursorX.value - vp.clientWidth * 0.3
  vp.scrollLeft = Math.max(0, target)
}

// Surveille la position du curseur pour déclencher l'auto-scroll.
watch(cursorX, () => {
  autoScroll()
})

// --- Mesure responsive ---

function measure() {
  if (viewportEl.value) {
    viewportWidth.value = viewportEl.value.clientWidth
  }
}

onMounted(() => {
  measure()
  if (viewportEl.value) {
    resizeObserver = new ResizeObserver(() => measure())
    resizeObserver.observe(viewportEl.value)
  }

  // Masquer le graphe + réinitialiser le scroll quand la trace sélectionnée
  // change (évite le flash de l'ancienne timeline pendant le chargement).
  watch(
    () => editionStore.selectedTraceId,
    () => {
      traceLoading.value = true
      if (viewportEl.value) viewportEl.value.scrollLeft = 0
    },
  )

  // Quand les keyframes arrivent (trace chargée), révéler le graphe.
  watch(
    () => editionStore.hasKeyframes,
    async (visible) => {
      if (visible) {
        traceLoading.value = false
        await nextTick()
        measure()
        autoScroll()
      }
    },
    { immediate: true },
  )
})

onUnmounted(() => {
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
  if (userScrollTimer) clearTimeout(userScrollTimer)
})
</script>

<style scoped>
.progress-graph {
  width: 100%;
  padding: 4px 12px;
  height: 70px;
}

.progress-viewport {
  width: 100%;
  height: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  /* Scrollbar masquée (scroll auto conservé) */
  scrollbar-width: none;
  -ms-overflow-style: none;
}

/* Scrollbar masquée (WebKit) */
.progress-viewport::-webkit-scrollbar {
  display: none;
}

.progress-content {
  height: 100%;
  position: relative;
}

.progress-graph-svg {
  display: block;
  /* width fixée via :style (timelineWidthPx) pour cohérence viewBox/rendu */
  height: 100%;
  cursor: pointer;
}

/* Curseur pointeur sur les ticks RdV pour signaler qu'ils sont cliquables */
.rdv-tick {
  cursor: pointer;
}
.rdv-tick:hover {
  fill: #FFD600;
}

/* Repères 10 km : zone de clic élargie + curseur pointer */
.km-mark-hit {
  cursor: pointer;
}
</style>
