<template>
  <div class="altitude-profile">
    <div class="text-caption text-medium-emphasis px-3 pt-1">
      Profil d'altitude
    </div>
    <svg
      ref="svgRef"
      class="profile-svg"
      :viewBox="`0 0 ${WIDTH} ${HEIGHT}`"
      preserveAspectRatio="none"
      @click="onClick"
      @mousemove="onHover"
      @mouseleave="hoverX = null"
    >
      <!-- Zone parcourue (dégradé sous la courbe) -->
      <defs>
        <linearGradient id="altGradient" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="rgb(var(--v-theme-primary))" stop-opacity="0.5" />
          <stop offset="100%" stop-color="rgb(var(--v-theme-primary))" stop-opacity="0.05" />
        </linearGradient>
      </defs>

      <!-- Aire sous la courbe -->
      <path v-if="areaPath" :d="areaPath" fill="url(#altGradient)" />

      <!-- Ligne de profil -->
      <path v-if="linePath" :d="linePath" fill="none"
        stroke="rgb(var(--v-theme-primary))" stroke-width="1.5" vector-effect="non-scaling-stroke" />

      <!-- Curseur vertical de progression -->
      <line
        v-if="cursorX !== null"
        :x1="cursorX" :y1="0" :x2="cursorX" :y2="HEIGHT"
        stroke="rgb(var(--v-theme-secondary))" stroke-width="1"
        vector-effect="non-scaling-stroke"
      />

      <!-- Curseur de survol (pointillé) -->
      <line
        v-if="hoverX !== null"
        :x1="hoverX" :y1="0" :x2="hoverX" :y2="HEIGHT"
        stroke="rgb(var(--v-theme-on-surface))" stroke-opacity="0.4"
        stroke-dasharray="3,3" stroke-width="1" vector-effect="non-scaling-stroke"
      />
    </svg>
  </div>
</template>

<script setup lang="ts">
/**
 * Profil d'altitude de la trace (§10.2).
 *
 * Mini-graphique SVG léger (pas de dépendance de charting) :
 *  - axe X : distance parcourue ;
 *  - axe Y : altitude ;
 *  - un curseur vertical synchronisé avec la timeline ;
 *  - clic pour se déplacer (seek) à la distance correspondante.
 *
 * Consomme les keyframes finaux (fusionnés) via le composable parent
 * `useLivePreview` exposé par la vue, pour rester cohérent avec le rendu.
 * Ici on lit `finalKeyframes` passé en prop (la vue fournit la ref calculée).
 */
import { computed, ref } from 'vue'
import { useKeyframesStore } from '../../stores/keyframes'
import type { RawKeyframesFile } from '../../utils/keyframes'

const props = defineProps<{
  /** Keyframes finaux (bruts fusionnés avec les overrides), ou null. */
  finalKeyframes: RawKeyframesFile | null
}>()

const keyframesStore = useKeyframesStore()

/** Dimensions du viewport SVG (coordonnées internes, mises à l'échelle par CSS). */
const WIDTH = 1000
const HEIGHT = 80

const svgRef = ref<SVGSVGElement | null>(null)
/** Position X (en coords SVG) du curseur de survol, ou null. */
const hoverX = ref<number | null>(null)

/** Points {x, y} normalisés dans le viewport SVG. */
const points = computed<{ x: number; y: number }[]>(() => {
  const kf = props.finalKeyframes?.keyframes
  if (!kf || kf.length === 0) return []

  // Calculer min/max altitude parmi les keyframes (traceur).
  let altMin = Infinity
  let altMax = -Infinity
  for (const k of kf) {
    if (k.traceur.altitude != null) {
      if (k.traceur.altitude < altMin) altMin = k.traceur.altitude
      if (k.traceur.altitude > altMax) altMax = k.traceur.altitude
    }
  }
  if (!isFinite(altMin) || !isFinite(altMax)) return []
  const altSpan = Math.max(altMax - altMin, 1)

  const total = keyframesStore.totalDuration || 1

  return kf.map((k) => ({
    x: (k.time / total) * WIDTH,
    y: HEIGHT - ((k.traceur.altitude ?? altMin) - altMin) / altSpan * HEIGHT,
  }))
})

/** Chemin de la ligne de profil (M + L…). */
const linePath = computed(() => buildPath(points.value, false))
/** Chemin de l'aire sous la courbe (fermé en bas). */
const areaPath = computed(() => buildPath(points.value, true))

/** Construit un path SVG à partir des points (ligne simple ou aire fermée). */
function buildPath(pts: { x: number; y: number }[], closed: boolean): string {
  if (pts.length === 0) return ''
  let d = `M ${pts[0].x.toFixed(2)} ${pts[0].y.toFixed(2)}`
  for (let i = 1; i < pts.length; i++) {
    d += ` L ${pts[i].x.toFixed(2)} ${pts[i].y.toFixed(2)}`
  }
  if (closed) {
    d += ` L ${pts[pts.length - 1].x.toFixed(2)} ${HEIGHT} L ${pts[0].x.toFixed(2)} ${HEIGHT} Z`
  }
  return d
}

/** Position X (coords SVG) du curseur de progression, ou null. */
const cursorX = computed(() => {
  if (points.value.length === 0) return null
  const fraction = keyframesStore.totalDuration > 0
    ? keyframesStore.currentTime / keyframesStore.totalDuration
    : 0
  return Math.max(0, Math.min(1, fraction)) * WIDTH
})

/** Convertit une abscisse client → fraction 0..1 de la largeur du SVG. */
function clientXToFraction(clientX: number): number {
  const svg = svgRef.value
  if (!svg) return 0
  const rect = svg.getBoundingClientRect()
  if (rect.width === 0) return 0
  return Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))
}

/** Clic → seek à la fraction correspondante. */
function onClick(e: MouseEvent) {
  const fraction = clientXToFraction(e.clientX)
  keyframesStore.seek(fraction * keyframesStore.totalDuration)
}

/** Survol → affiche le curseur vertical pointillé. */
function onHover(e: MouseEvent) {
  hoverX.value = clientXToFraction(e.clientX) * WIDTH
}
</script>

<style scoped>
.altitude-profile {
  width: 100%;
  height: 96px;
}

.profile-svg {
  width: 100%;
  height: 72px;
  cursor: pointer;
  display: block;
}
</style>
