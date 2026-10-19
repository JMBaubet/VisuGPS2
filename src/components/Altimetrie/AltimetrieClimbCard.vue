<template>
  <v-card
    class="alt-climb-card"
    :class="{ 'alt-climb-card--selected': selected }"
    variant="outlined"
    @click="emit('click')"
  >
    <div class="alt-climb-card__header">
      <span class="alt-climb-card__index">#{{ climb.index + 1 }}</span>
      <span class="alt-climb-card__category-dot" :style="{ background: categoryColor }" />
      <span class="alt-climb-card__category-label">{{ categoryLabel }}</span>
      <v-chip
        v-if="climb.isMerged"
        size="x-small"
        variant="tonal"
        color="info"
        class="ml-1"
      >
        fusionnée
      </v-chip>
      <v-chip
        v-if="climb.isSeparated"
        size="x-small"
        variant="tonal"
        color="secondary"
        class="ml-1"
      >
        séparée
      </v-chip>
    </div>

    <!-- Position de la montée sur la trace complète : profil miniature,
         bande de la montée et bornes. -->
    <svg
      class="alt-climb-card__sparkline"
      viewBox="0 0 100 26"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <rect
        :x="highlightX"
        y="2"
        :width="highlightWidth"
        height="22"
        :fill="categoryColor"
        opacity="0.22"
      />
      <polyline
        :points="sparkline"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
        vector-effect="non-scaling-stroke"
      />
      <line
        :x1="highlightX"
        y1="2"
        :x2="highlightX"
        y2="24"
        :stroke="categoryColor"
        stroke-width="1"
        stroke-dasharray="2 2"
        vector-effect="non-scaling-stroke"
      />
      <line
        :x1="highlightX + highlightWidth"
        y1="2"
        :x2="highlightX + highlightWidth"
        y2="24"
        :stroke="categoryColor"
        stroke-width="1"
        stroke-dasharray="2 2"
        vector-effect="non-scaling-stroke"
      />
    </svg>

    <div class="alt-climb-card__metrics">
      <div class="alt-climb-card__metric">
        <span class="alt-climb-card__metric-label">Vallée</span>
        <span class="alt-climb-card__metric-value">
          {{ formatDistance(climb.startKm * 1000) }}
        </span>
      </div>
      <div class="alt-climb-card__metric">
        <span class="alt-climb-card__metric-label">Longueur</span>
        <span class="alt-climb-card__metric-value">
          {{ formatDistance((climb.endKm - climb.startKm) * 1000) }}
        </span>
      </div>
      <div class="alt-climb-card__metric">
        <span class="alt-climb-card__metric-label">Dénivelé</span>
        <span class="alt-climb-card__metric-value">
          {{ formatElevation(climb.dPlusM) }}
        </span>
      </div>
      <div class="alt-climb-card__metric">
        <span class="alt-climb-card__metric-label">Pente moy.</span>
        <span class="alt-climb-card__metric-value">
          {{ climb.avgSlopePct.toFixed(1) }} %
        </span>
      </div>
    </div>
  </v-card>
</template>

<script setup lang="ts">
/**
 * Carte d'une montée : mini-profil de la trace complète, bornes surlignées et
 * métriques essentielles (vallée, longueur, dénivelé, pente moyenne).
 *
 * La représentation graphique est un SVG léger : la trace est sous-échantillonnée
 * à ~90 points, la montée est peinte dans la couleur de sa catégorie de pente.
 */
import { computed } from 'vue'
import type { AltimetrieClimb, AltimetrieProfilePoint } from '../../stores/altimetrie'
import { colorForSlope, labelForSlope } from '../../utils/altimetrieCategories'
import { formatDistance, formatElevation } from '../../utils/format'

const props = defineProps<{
  climb: AltimetrieClimb
  profile: AltimetrieProfilePoint[]
  altMin: number
  altMax: number
  totalDistanceM: number
  selected: boolean
}>()

const emit = defineEmits<{
  click: []
}>()

/** Distance totale du profil, jamais nulle (protection du ratio). */
const totalDistance = computed(() =>
  props.totalDistanceM > 0 ? props.totalDistanceM : 1,
)

const categoryColor = computed(() => colorForSlope(props.climb.avgSlopePct))
const categoryLabel = computed(() => labelForSlope(props.climb.avgSlopePct))

/** Trace complète sous-échantillonnée, en coordonnées du viewBox 100×26. */
const sparkline = computed(() => {
  const profile = props.profile
  if (profile.length < 2) return ''

  const altRange = Math.max(props.altMax - props.altMin, 1)
  const step = Math.max(1, Math.ceil(profile.length / 90))
  const points: string[] = []

  const pushPoint = (point: AltimetrieProfilePoint) => {
    const x = Math.min(100, Math.max(0, (point.d / totalDistance.value) * 100))
    const y = 24 - ((point.ele - props.altMin) / altRange) * 22
    points.push(`${x.toFixed(2)},${y.toFixed(2)}`)
  }

  for (let i = 0; i < profile.length; i += step) {
    pushPoint(profile[i])
  }
  if ((profile.length - 1) % step !== 0) {
    pushPoint(profile[profile.length - 1])
  }

  return points.join(' ')
})

const highlightX = computed(() =>
  Math.max(
    0,
    Math.min(100, (props.climb.startKm * 1000 / totalDistance.value) * 100),
  ),
)

const highlightWidth = computed(() => {
  const startM = props.climb.startKm * 1000
  const endM = props.climb.endKm * 1000
  const width = ((endM - startM) / totalDistance.value) * 100
  return Math.max(0.4, Math.min(100 - highlightX.value, width))
})
</script>

<style scoped>
.alt-climb-card {
  cursor: pointer;
  transition:
    border-color 0.15s ease,
    background-color 0.15s ease;
}

.alt-climb-card:hover {
  border-color: rgba(var(--v-theme-primary), 0.45);
}

.alt-climb-card--selected {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.06);
}

.alt-climb-card__header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 12px 6px;
}

.alt-climb-card__index {
  font-size: 0.8rem;
  font-weight: 600;
  color: rgba(var(--v-theme-on-surface), 0.86);
}

.alt-climb-card__category-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex: 0 0 auto;
  /* La catégorie up4 est noire : un liseré la garde visible sur fond sombre. */
  box-shadow: 0 0 0 1px rgba(var(--v-theme-on-surface), 0.28);
}

.alt-climb-card__category-label {
  font-size: 0.72rem;
  color: rgba(var(--v-theme-on-surface), 0.7);
}

.alt-climb-card__sparkline {
  display: block;
  width: 100%;
  height: 42px;
  color: rgba(var(--v-theme-on-surface), 0.68);
}

.alt-climb-card__metrics {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px 12px;
  padding: 6px 12px 12px;
}

.alt-climb-card__metric {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.alt-climb-card__metric-label {
  font-size: 0.65rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: rgba(var(--v-theme-on-surface), 0.55);
}

.alt-climb-card__metric-value {
  font-size: 0.83rem;
  font-weight: 600;
  color: rgba(var(--v-theme-on-surface), 0.92);
}
</style>
