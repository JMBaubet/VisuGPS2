<template>
  <div
    v-show="isCurrentTraceLoaded"
    ref="graphContainer"
    class="progress-graph"
  >
    <!--
      SVG unique, dimensions = viewport (conteneur). Tout le contenu de la
      timeline est enveloppé dans un <g> translaté de -scrollOffset : c'est
      le SVG qui gère le scroll (translation), pas le DOM. Plus de scrollbar,
      plus de race condition clientWidth=0.
    -->
    <svg
      ref="svgEl"
      :viewBox="`0 0 ${viewWidth} ${totalHeight}`"
      preserveAspectRatio="none"
      class="progress-graph-svg"
      @click="onTimelineClick"
      @mousemove="onMouseMove"
      @mouseleave="onMouseLeave"
    >
      <!-- Clip pour que le contenu translaté ne déborde pas -->
      <defs>
        <clipPath id="timeline-clip">
          <rect :x="0" :y="0" :width="viewWidth" :height="totalHeight" />
        </clipPath>
      </defs>

      <!-- Contenu timeline (translaté horizontalement par scrollOffset) -->
      <g :transform="`translate(${-scrollOffset}, 0)`" clip-path="url(#timeline-clip)">
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
      </g>
    </svg>
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
 * **Scroll géré par le SVG** (et non par le DOM) : le viewBox du SVG est fixe
 * (= dimensions du conteneur), et tout le contenu timeline est enveloppé dans
 * un <g :transform="translate(-scrollOffset, 0)">. Le scroll devient un simple
 * décalage de coordonnées — plus de scrollbar, plus de problème clientWidth=0
 * quand le graphe est masqué.
 *
 * Trois zones distinctes, de haut en bas :
 *   1. **Points de RdV** (keyframes) — ticks larges (3px) cliquables ;
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
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useEditionStore } from '../../stores/edition'

const editionStore = useEditionStore()

// --- Références ---

const graphContainer = ref<HTMLDivElement | null>(null)
const svgEl = ref<SVGSVGElement | null>(null)

let resizeObserver: ResizeObserver | null = null

/**
 * `true` quand les keyframes affichés correspondent à la trace sélectionnée.
 * Évite le flash de l'ancienne timeline : tant que keyframeSet.trace_id ne
 * correspond pas à selectedTraceId (chargement en cours), le graphe est
 * masqué.
 */
const isCurrentTraceLoaded = computed(
  () =>
    !!editionStore.keyframeSet &&
    editionStore.keyframeSet.trace_id === editionStore.selectedTraceId,
)

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

// --- Dimensions du viewport ---

/** Largeur mesurée du conteneur (px) = largeur visible du SVG. */
const viewWidth = ref(0)
/** Largeur totale de la timeline (px, proportionnelle à la trace). */
const timelineWidthPx = computed(() =>
  Math.ceil(editionStore.totalDistanceM * PX_PER_METER),
)

// --- Scroll (offset de translation du <g> SVG) ---

/**
 * Offset de scroll en px. Appliqué comme `translate(-scrollOffset, 0)` sur
 * le <g> contenant la timeline. 0 = timeline collée à gauche.
 */
const scrollOffset = ref(0)

/**
 * Indique si l'utilisateur est en train de défiler manuellement (roulette /
 * glisser). Pendant ce temps, l'auto-scroll est suspendu.
 */
let userScrolling = false
let userScrollTimer: ReturnType<typeof setTimeout> | null = null

function suspendAutoScroll() {
  userScrolling = true
  if (userScrollTimer) clearTimeout(userScrollTimer)
  userScrollTimer = setTimeout(() => {
    userScrolling = false
  }, 1500)
}

/**
 * Offset de scroll maximum (= largeur timeline - largeur viewport), au-delà
 * duquel on ne peut plus scroller vers la droite.
 */
const maxScrollOffset = computed(() =>
  Math.max(0, timelineWidthPx.value - viewWidth.value),
)

/**
 * Recentre le viewport sur le curseur rouge si la timeline dépasse le
 * viewport. Le curseur est maintenu à ~30% du viewport depuis le bord
 * gauche, de sorte qu'on voit une longueur de trace devant lui.
 */
function autoScroll() {
  if (userScrolling) return
  if (viewWidth.value <= 0) return
  if (timelineWidthPx.value <= viewWidth.value) {
    // Timeline plus courte que le viewport : pas de scroll.
    scrollOffset.value = 0
    return
  }
  const target = cursorX.value - viewWidth.value * 0.3
  scrollOffset.value = Math.max(0, Math.min(maxScrollOffset.value, target))
}

/** Défilement manuel (delta en px, positif = vers la droite). */
function manualScroll(deltaPx: number) {
  suspendAutoScroll()
  scrollOffset.value = Math.max(
    0,
    Math.min(maxScrollOffset.value, scrollOffset.value + deltaPx),
  )
}

/** Molette : défiler horizontalement (deltaY converti en delta X). */
function onWheel(event: WheelEvent) {
  // On intercepte uniquement la molette verticale pour la convertir en
  // scroll horizontal.
  if (event.deltaY !== 0) {
    event.preventDefault()
    manualScroll(event.deltaY)
  } else if (event.deltaX !== 0) {
    event.preventDefault()
    manualScroll(event.deltaX)
  }
}

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

/**
 * Convertit un clientX (écran) en coordonnée X de la timeline (viewBox du
 * SVG + décalage du scrollOffset). Utilise getScreenCTM pour être robuste
 * face au scaling responsive.
 */
function clientXToTimelineX(clientX: number): number {
  const svg = svgEl.value
  if (!svg) return 0
  const ctm = svg.getScreenCTM()
  if (!ctm) return 0
  const pt = svg.createSVGPoint()
  pt.x = clientX
  pt.y = 0
  const transformed = pt.matrixTransform(ctm.inverse())
  // Le viewBox est [0..viewWidth], mais le contenu est translaté de
  // -scrollOffset : il faut donc ajouter scrollOffset pour obtenir la
  // coordonnée "timeline" réelle.
  const tlX = transformed.x + scrollOffset.value
  return Math.max(0, Math.min(timelineWidthPx.value, tlX))
}

function timelineXToDistance(x: number): number {
  return Math.max(0, Math.min(totalDistanceM.value, x / PX_PER_METER))
}

function onMouseMove(event: MouseEvent) {
  const x = clientXToTimelineX(event.clientX)
  const dist = timelineXToDistance(x)
  // Position du tooltip en coordonnée timeline (relative au <g> translaté).
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
 */
function onTimelineClick(event: MouseEvent) {
  const x = clientXToTimelineX(event.clientX)
  const dist = timelineXToDistance(x)
  editionStore.seekToDistance(dist)
}

// --- Watchers ---

// Auto-scroll quand le curseur avance.
watch(cursorX, () => {
  autoScroll()
})

// --- Cycle de vie ---

function measure() {
  if (graphContainer.value) {
    viewWidth.value = graphContainer.value.clientWidth
  }
}

onMounted(() => {
  // Brancher la molette sur le conteneur pour le scroll horizontal.
  if (graphContainer.value) {
    graphContainer.value.addEventListener('wheel', onWheel, { passive: false })
  }

  measure()
  if (graphContainer.value) {
    resizeObserver = new ResizeObserver(() => measure())
    resizeObserver.observe(graphContainer.value)
  }

  // Quand la trace courante est chargée, réinitialiser le scroll et
  // re-mesurer. Gère aussi le flash : le graphe est masqué tant que
  // isCurrentTraceLoaded est false.
  watch(
    isCurrentTraceLoaded,
    async (loaded) => {
      if (loaded) {
        scrollOffset.value = 0
        await Promise.resolve()
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
  if (graphContainer.value) {
    graphContainer.value.removeEventListener('wheel', onWheel)
  }
  if (userScrollTimer) clearTimeout(userScrollTimer)
})
</script>

<style scoped>
.progress-graph {
  width: 100%;
  padding: 0 12px;
  height: 70px;
  overflow: hidden;
}

.progress-graph-svg {
  display: block;
  width: 100%;
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
