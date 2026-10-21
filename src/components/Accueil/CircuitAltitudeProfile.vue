<template>
  <div class="circuit-profile">
    <svg
      v-if="profile.length > 1"
      class="circuit-profile__sparkline"
      viewBox="0 0 100 26"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <!-- Bandes des montées, colorées par catégorie de pente. -->
      <rect
        v-for="bar in climbBars"
        :key="bar.key"
        :x="bar.x"
        y="2"
        :width="bar.width"
        height="22"
        :fill="bar.color"
        opacity="0.28"
      >
        <title>{{ bar.title }}</title>
      </rect>

      <!-- Profil d'altitude de la trace complète. -->
      <polyline
        :points="sparkline"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
        vector-effect="non-scaling-stroke"
      />
    </svg>

    <div v-else class="circuit-profile__empty text-caption">
      Profil d'altitude indisponible
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Sparkline du profil d'altitude complet, avec les montées surlignées.
 *
 * Même représentation que le mini-profil de `AltimetrieClimbCard` : la trace
 * est projetée dans un viewBox 100×26 et chaque montée est peinte dans la
 * couleur de sa catégorie de pente. Le profil reçu est déjà sous-échantillonné
 * par le backend (~120 points), la carte n'a donc pas à le réduire.
 */
import { computed } from 'vue'
import type {
  CircuitClimbInfo,
  CircuitProfilePoint,
} from '../../composables/useCircuitInfo'
import { colorForSlope } from '../../utils/altimetrieCategories'

const props = defineProps<{
  profile: CircuitProfilePoint[]
  climbs: CircuitClimbInfo[]
  altMinM: number
  altMaxM: number
  totalDistanceM: number
}>()

/** Distance totale du profil, jamais nulle (protection du ratio). */
const totalDistance = computed(() =>
  props.totalDistanceM > 0 ? props.totalDistanceM : 1,
)

/** Amplitude d'altitude, jamais nulle (protection du ratio). */
const altRange = computed(() => Math.max(props.altMaxM - props.altMinM, 1))

/** Profil projeté dans le viewBox 100×26. */
const sparkline = computed(() => {
  const profile = props.profile
  if (profile.length < 2) return ''

  return profile
    .map(point => {
      const x = Math.min(100, Math.max(0, (point.d / totalDistance.value) * 100))
      const y = 24 - ((point.ele - props.altMinM) / altRange.value) * 22
      return `${x.toFixed(2)},${y.toFixed(2)}`
    })
    .join(' ')
})

/** Bandes des montées, dans l'espace du viewBox. */
const climbBars = computed(() =>
  props.climbs.map(climb => {
    const startM = climb.startKm * 1000
    const endM = climb.endKm * 1000
    const x = Math.max(
      0,
      Math.min(100, (startM / totalDistance.value) * 100),
    )
    const width = Math.max(
      0.4,
      Math.min(100 - x, ((endM - startM) / totalDistance.value) * 100),
    )

    return {
      key: `${climb.index}-${climb.startKm}`,
      x,
      width,
      color: colorForSlope(climb.avgSlopePct),
      title: `Montée #${climb.index + 1} — ${(climb.endKm - climb.startKm).toFixed(2)} km à ${climb.avgSlopePct.toFixed(1)} %`,
    }
  }),
)
</script>

<style scoped>
.circuit-profile {
  width: 100%;
}

.circuit-profile__sparkline {
  display: block;
  width: 100%;
  height: 56px;
  color: rgba(var(--v-theme-on-surface), 0.68);
}

.circuit-profile__empty {
  color: rgba(var(--v-theme-on-surface), 0.55);
}
</style>
