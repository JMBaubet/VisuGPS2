<template>
  <div
    v-show="isCurrentTraceLoaded"
    ref="graphContainer"
    class="progress-graph"
  >
    <!--
      Viewport scrollable (scroll DOM natif — fiable pour le rendu du
      contenu au-delà de la zone visible, contrairement à une translation
      SVG qui est cullée par le moteur de rendu sur les grandes largeurs).
      La scrollbar est masquée en CSS (scroll auto + molette conservés).
    -->
    <div
      ref="viewportEl"
      class="progress-viewport"
      @wheel.passive="onWheel"
    >
      <div class="progress-content" :style="{ width: timelineWidthPx + 'px' }">
        <svg
          ref="svgEl"
          :viewBox="`0 0 ${timelineWidthPx} ${totalHeight}`"
          :style="{ width: timelineWidthPx + 'px', height: totalHeight + 'px' }"
          class="progress-graph-svg"
          @click="onTimelineClick"
          @mousemove="onMouseMove"
          @mouseleave="onMouseLeave"
        >
          <!-- ZONE 1 : Points de RdV (keyframes) — ticks bleus (hauteur limitée, centrés) -->
          <g class="zone-rdv">
            <rect
              v-for="kf in keyframeTicks"
              :key="'rdv-' + kf.distance_from_start_m"
              :x="kf.x - RDV_WIDTH / 2"
              :y="rdvTickY"
              :width="RDV_WIDTH"
              :height="rdvTickHeight"
              fill="rgba(33, 150, 243, 0.7)"
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
              :x="0" :y="advanceBarY"
              :width="timelineWidthPx" :height="advanceBarHeight"
              fill="rgba(255,255,255,0.12)" rx="2"
            />
            <!-- Jauge jaune (avancement) — z-index inférieur -->
            <rect
              :x="0" :y="advanceBarY"
              :width="progressWidthPx" :height="advanceBarHeight"
              fill="#FFD600" rx="2"
            />
            <!-- Repères tous les 10 km — z-index supérieur (rendus après la jauge) -->
            <g v-for="mark in kmMarks" :key="'km-' + mark.distance" class="km-mark">
              <line
                :x1="mark.x" :y1="advanceBarY"
                :x2="mark.x" :y2="advanceBarY + advanceBarHeight"
                stroke="rgba(255,255,255,0.6)" stroke-width="1"
              />
              <!-- Zone de clic élargie autour du repère (invisible mais cliquable) -->
              <rect
                :x="mark.x - 5" :y="advanceBarY"
                :width="10" :height="advanceBarHeight"
                fill="transparent" class="km-mark-hit"
              />
            </g>
            <!-- Curseur orange (3px) — z-index le plus haut dans la zone -->
            <rect
              :x="cursorX - CURSOR_WIDTH / 2"
              :y="advanceBarY"
              :width="CURSOR_WIDTH"
              :height="advanceBarHeight"
              fill="#FF9800"
            />
          </g>

          <!-- Séparateur fin -->
          <line
            :x1="0" :y1="advanceZoneY + advanceZoneHeight"
            :x2="timelineWidthPx" :y2="advanceZoneY + advanceZoneHeight"
            stroke="rgba(255,255,255,0.1)" stroke-width="1"
          />

          <!-- ZONE 3 : Graduation (libellés « X km » tous les 10 km) -->
          <g class="zone-grad">
            <text
              v-for="mark in kmMarks"
              :key="'lbl-' + mark.distance"
              :x="mark.x"
              :y="gradZoneY + 12"
              fill="rgba(255,255,255,0.7)"
              font-size="10"
              font-family="monospace"
              text-anchor="middle"
            >{{ mark.label }}</text>
          </g>

          <!--
            Tooltip de survol, positionné au niveau de la zone graduation.
            Suivant la souris horizontalement. Texte blanc.
          -->
          <g v-if="tooltip.visible" :transform="`translate(${tooltip.x}, ${gradZoneY})`">
            <rect
              x="-58" y="2" width="116" height="16" rx="3"
              fill="rgba(0,0,0,0.9)" stroke="rgba(255,255,255,0.2)" stroke-width="1"
            />
            <text
              x="0" y="14" text-anchor="middle"
              fill="#FFFFFF" font-size="10" font-family="monospace"
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
 * **Scroll DOM natif** : un conteneur viewport (`overflow-x: auto`) contient
 * un content div de largeur `timelineWidthPx`, lui-même contenant le SVG
 * (width = timelineWidthPx = viewBox width → rendu 1:1, sans scaling). Le
 * navigateur gère nativement le rendu du contenu scrollé (fiable sur les
 * grandes largeurs, contrairement à une translation SVG qui est « cullée »
 * par le moteur de rendu). La scrollbar est masquée en CSS.
 *
 * Trois zones, de haut en bas :
 *   1. **Points de RdV** (keyframes) — ticks larges (3px) cliquables ;
 *   2. **Ligne d'avancement** — piste de fond + jauge jaune (progression) +
 *      repères verticaux tous les 10 km (cliquables pour seek direct) +
 *      curseur rouge (3px). z-index : jauge < repères < curseur ;
 *   3. **Graduation** — libellés « X km » tous les 10 km (non cliquable) +
 *      tooltip de survol (blanc) qui suit la souris horizontalement et affiche
 *      distance + altitude au point survolé.
 *
 * Hauteur compacte (~58px). Échelle : 3 px / 100 m.
 */
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { useEditionStore } from '../../stores/edition'

const editionStore = useEditionStore()

// --- Références ---

const graphContainer = ref<HTMLDivElement | null>(null)
const viewportEl = ref<HTMLDivElement | null>(null)
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

// Hauteurs des zones (alignées avec les rangées de boutons de PlaybackControls).
// Rangée 1 (RdV) et rangée 2 (avancement) ont la même hauteur (28px),
// alignée sur les boutons (size="small"). La graduation fait 18px.
const rdvZoneHeight = 28
const advanceZoneHeight = 28
const advanceBarHeight = 16
const gradZoneHeight = 18

/** Ordonnées de chaque zone. */
const rdvZoneY = 0
const advanceZoneY = rdvZoneY + rdvZoneHeight
const gradZoneY = advanceZoneY + advanceZoneHeight

// Les ticks RdV ne font pas toute la hauteur de la zone : ils sont limités et
// centrés verticalement.
const rdvTickHeight = 20
const rdvTickY = rdvZoneY + (rdvZoneHeight - rdvTickHeight) / 2

// La barre d'avancement est centrée verticalement dans sa zone (réduit la
// zone neutre entre la barre et l'axe des abscisses).
const advanceBarY = advanceZoneY + (advanceZoneHeight - advanceBarHeight) / 2

/** Hauteur totale du SVG. */
const totalHeight = rdvZoneHeight + advanceZoneHeight + gradZoneHeight

// --- Dimensions ---

/** Largeur totale de la timeline (px, proportionnelle à la trace). */
const timelineWidthPx = computed(() =>
  Math.ceil(editionStore.totalDistanceM * PX_PER_METER),
)

// --- Auto-scroll (rAF continue pendant le playback) ---

/**
 * Indique si l'utilisateur est en train de défiler manuellement. Pendant ce
 * temps, l'auto-scroll est suspendu.
 */
let userScrolling = false
let userScrollTimer: ReturnType<typeof setTimeout> | null = null

/**
 * Dernière valeur de scrollLeft que NOUS avons assignée programmatiquement.
 * Permet de distinguer un event `scroll` déclenché par notre rAF (valeur
 * identique) d'un scroll utilisateur réel (valeur différente).
 *
 * On ne peut pas utiliser un simple flag booléen levé autour de l'assignation
 * car l'event `scroll` est différé (asynchrone) : il se déclenche APRÈS la
 * frame courante, quand le flag est déjà revenu à false.
 */
let lastProgrammaticScrollLeft = -1

function suspendAutoScroll() {
  userScrolling = true
  if (userScrollTimer) clearTimeout(userScrollTimer)
  userScrollTimer = setTimeout(() => {
    userScrolling = false
  }, 1500)
}

/**
 * Défilement natif détecté sur le viewport. Ignore les scrolls que NOUS
 * déclenchons en comparant scrollLeft à la dernière valeur programmatique.
 * Seul un scroll utilisateur réel (valeur différente) suspend l'auto-scroll.
 */
function onManualScroll() {
  const vp = viewportEl.value
  if (vp && Math.abs(vp.scrollLeft - lastProgrammaticScrollLeft) < 1) return
  suspendAutoScroll()
}

/**
 * Molette : convertit le défilement vertical en défilement horizontal (la
 * timeline n'a pas de contenu vertical). passive car on ne fait que lire
 * deltaY et modifier scrollLeft (pas de preventDefault).
 */
function onWheel(event: WheelEvent) {
  if (event.deltaY !== 0 && viewportEl.value) {
    viewportEl.value.scrollLeft += event.deltaY
    suspendAutoScroll()
  }
}

/**
 * Identifiant de la boucle requestAnimationFrame de scroll, ou null si
 * inactive. Une seule boucle tourne à la fois, démarrée/arrêtée selon
 * l'état de lecture du store.
 */
let scrollRafId: number | null = null

/**
 * Applique le scroll directement (sans interpolation) : scrollLeft =
 * max(0, cursorX - viewport*0.3). Le curseur rouge est maintenu à ~30% du
 * viewport depuis le bord gauche.
 *
 * Sans lerp ni epsilon : le scroll suit le curseur **exactement** à chaque
 * frame, exactement comme applyInterpolatedState fait un jumpTo. C'est le
 * playback lui-même (rAF du store, ~60 fps) qui pilote la fluidité.
 */
function applyAutoScroll() {
  const vp = viewportEl.value
  if (!vp || userScrolling) return
  if (vp.clientWidth <= 0) return
  let target: number
  if (timelineWidthPx.value <= vp.clientWidth) {
    target = 0
  } else {
    target = Math.max(0, cursorX.value - vp.clientWidth * 0.3)
  }
  // Comparaison avant affectation pour éviter des events scroll inutiles
  // quand la position n'a pas bougé.
  if (Math.abs(vp.scrollLeft - target) >= 1) {
    vp.scrollLeft = target
    // Mémoriser la valeur assignée pour que l'event `scroll` différé qui
    // en découle soit reconnu comme programmatique par onManualScroll.
    lastProgrammaticScrollLeft = target
  }
}

/** Callback d'une frame de la boucle de scroll continue. */
function onScrollRaf() {
  scrollRafId = null
  applyAutoScroll()
  // Relancer tant que la lecture est en cours.
  if (editionStore.isPlaying) {
    scrollRafId = requestAnimationFrame(onScrollRaf)
  }
}

/** Démarre la boucle de scroll continue (si inactive). */
function startScrollLoop() {
  if (scrollRafId !== null) return
  scrollRafId = requestAnimationFrame(onScrollRaf)
}

/** Arrête la boucle de scroll continue (si active). */
function stopScrollLoop() {
  if (scrollRafId !== null) {
    cancelAnimationFrame(scrollRafId)
    scrollRafId = null
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
 * Repères tous les 10 km (ligne verticale dans la barre + libellé graduation).
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

// --- Tooltip de survol + clic ---

const tooltip = ref({ visible: false, x: 0, text: '' })

/**
 * Convertit un clientX (écran) en coordonnée X de la timeline via la matrice
 * native du SVG (getScreenCTM). Robuste face au scroll DOM, au scaling et
 * au positionnement, quel que soit l'état du scroll.
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
  return Math.max(0, Math.min(timelineWidthPx.value, transformed.x))
}

function timelineXToDistance(x: number): number {
  return Math.max(0, Math.min(totalDistanceM.value, x / PX_PER_METER))
}

function onMouseMove(event: MouseEvent) {
  const x = clientXToTimelineX(event.clientX)
  const dist = timelineXToDistance(x)
  tooltip.value = { visible: true, x, text: formatTooltip(dist) }
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
 * Distance en dessous de laquelle un clic est considéré comme « sur » un
 * keyframe (moitié du pas d'échantillonnage). Au-delà, simple seek.
 */
const KEYFRAME_HIT_TOLERANCE_M = 500

function onTimelineClick(event: MouseEvent) {
  const x = clientXToTimelineX(event.clientX)
  const dist = timelineXToDistance(x)
  editionStore.seekToDistance(dist)

  // Sélectionner le keyframe le plus proche si le clic tombe dessus.
  const kfs = editionStore.keyframeSet?.keyframes ?? []
  let nearest: (typeof kfs)[number] | null = null
  let nearestGap = Infinity
  for (const kf of kfs) {
    const gap = Math.abs(kf.distance_from_start_m - dist)
    if (gap < nearestGap) {
      nearestGap = gap
      nearest = kf
    }
  }
  if (nearest && nearestGap <= KEYFRAME_HIT_TOLERANCE_M) {
    editionStore.selectKeyframe(nearest.distance_from_start_m)
  }
}

// --- Watchers ---

// Boucle de scroll : démarre en lecture, s'arrête en pause (comme la boucle
// applyInterpolatedState du EditionMap). Pendant la pause, un scroll
// ponctuel est appliqué via le watcher currentTimeMs ci-dessous.
watch(
  () => editionStore.isPlaying,
  (playing) => {
    if (playing) startScrollLoop()
    else stopScrollLoop()
  },
)

// En pause (seek, positionnement initial) : appliquer le scroll une fois
// quand le temps change. Pendant la lecture, la boucle rAF s'en charge déjà.
watch(
  () => editionStore.currentTimeMs,
  () => {
    if (!editionStore.isPlaying) applyAutoScroll()
  },
)

// --- Cycle de vie ---

onMounted(() => {
  if (viewportEl.value) {
    viewportEl.value.addEventListener('scroll', onManualScroll, { passive: true })
  }

  if (viewportEl.value) {
    resizeObserver = new ResizeObserver(() => {})
    resizeObserver.observe(viewportEl.value)
  }

  watch(
    isCurrentTraceLoaded,
    async (loaded) => {
      if (loaded) {
        // Nouvelle trace : stopper la boucle et réinitialiser le scroll.
        stopScrollLoop()
        if (viewportEl.value) viewportEl.value.scrollLeft = 0
        await nextTick()
        applyAutoScroll()
        // Si déjà en lecture, redémarrer la boucle.
        if (editionStore.isPlaying) startScrollLoop()
      }
    },
    { immediate: true },
  )
})

onUnmounted(() => {
  stopScrollLoop()
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
  if (viewportEl.value) {
    viewportEl.value.removeEventListener('scroll', onManualScroll)
  }
  if (userScrollTimer) clearTimeout(userScrollTimer)
})
</script>

<style scoped>
.progress-graph {
  width: 100%;
  padding: 0 12px;
  /* Hauteur = SVG (78px) + petit padding vertical. */
  height: 82px;
}

.progress-viewport {
  width: 100%;
  height: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  /* Scrollbar masquée (scroll auto + molette conservés) */
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
  cursor: pointer;
}

/* Curseur pointeur sur les ticks RdV pour signaler qu'ils sont cliquables */
.rdv-tick {
  cursor: pointer;
}
.rdv-tick:hover {
  fill: #2196F3;
}

/* Repères 10 km : zone de clic élargie + curseur pointer */
.km-mark-hit {
  cursor: pointer;
}
</style>
