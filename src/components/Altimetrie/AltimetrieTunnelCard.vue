<template>
  <v-card
    class="alt-tunnel-card"
    :class="{ 'alt-tunnel-card--selected': selected }"
    variant="outlined"
    @click="emit('click')"
  >
    <div class="alt-tunnel-card__header">
      <v-icon icon="mdi-tunnel" size="16" :color="TUNNEL_COLOR" class="mr-1" />
      <span class="alt-tunnel-card__index">#{{ index + 1 }}</span>
      <span v-if="name" class="alt-tunnel-card__name" :title="name">
        {{ name }}
      </span>

      <v-chip
        v-if="group.merged"
        size="x-small"
        variant="tonal"
        color="info"
        class="ml-1"
      >
        fusionné ({{ group.tunnels.length }})
      </v-chip>
      <v-btn
        v-if="group.merged"
        icon="mdi-call-split"
        size="x-small"
        variant="text"
        color="info"
        class="ml-1"
        title="Séparer les tronçons fusionnés"
        @click.stop="emit('split')"
      />
      <v-btn
        v-if="canMerge"
        icon="mdi-call-merge"
        size="x-small"
        variant="text"
        color="orange"
        class="ml-1"
        :title="mergeTitle"
        @click.stop="emit('merge')"
      />

      <v-spacer />
      <v-btn
        icon="mdi-delete-outline"
        size="x-small"
        variant="text"
        color="error"
        title="Supprimer le tunnel"
        @click.stop="emit('delete')"
      />
    </div>

    <!-- Position des tronçons sur la trace complète : trait pointillé bleu
         pour un tunnel court, bande bleue pour un tunnel long. -->
    <svg
      class="alt-tunnel-card__sparkline"
      viewBox="0 0 100 26"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <rect
        v-if="selected"
        :x="groupX"
        y="0"
        :width="groupWidth"
        height="26"
        :fill="TUNNEL_COLOR"
        opacity="0.08"
      />
      <polyline
        :points="sparkline"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
        vector-effect="non-scaling-stroke"
      />
      <template v-for="span in spans" :key="span.id">
        <line
          v-if="span.short"
          :x1="span.mid"
          y1="2"
          :x2="span.mid"
          y2="24"
          :stroke="TUNNEL_COLOR"
          stroke-width="0.8"
          stroke-dasharray="1.2 1.2"
          vector-effect="non-scaling-stroke"
        />
        <rect
          v-else
          :x="span.x1"
          y="2"
          :width="span.width"
          height="22"
          :fill="TUNNEL_COLOR"
          opacity="0.28"
        />
      </template>
    </svg>

    <div class="alt-tunnel-card__metrics">
      <div class="alt-tunnel-card__metric">
        <span class="alt-tunnel-card__metric-label">Entrée :</span>
        <span class="alt-tunnel-card__metric-value">
          {{ formatDistance(group.startKm * 1000) }}
        </span>
      </div>
      <div class="alt-tunnel-card__metric">
        <span class="alt-tunnel-card__metric-label">Longueur :</span>
        <span class="alt-tunnel-card__metric-value">
          {{ formatTunnelLength(group.lengthM) }}
        </span>
      </div>
    </div>

    <div v-if="group.merged" class="alt-tunnel-card__sub">
      {{ group.tunnels.length }} tronçons distincts
    </div>
  </v-card>
</template>

<script setup lang="ts">
/**
 * Carte d'un tunnel, ou d'un groupe fusionné : mini-profil de la trace
 * complète, position des tronçons et métriques demandées (distance d'entrée
 * depuis le km 0, longueur). Un tunnel court est un trait vertical bleu
 * pointillé ; un tunnel long est une bande bleue, comme les montées.
 *
 * Les tronçons d'un groupe fusionné restent **disjoints** : la carte dessine
 * chacun d'eux séparément, l'écart n'est jamais traité comme du tunnel.
 */
import { computed } from 'vue'
import type { AltimetrieProfilePoint } from '../../stores/altimetrie'
import type { TunnelGroup } from '../../stores/tunnels'
import {
  formatTunnelLength,
  isShortTunnelOnProfile,
} from '../../utils/altimetrieTunnels'
import { formatDistance } from '../../utils/format'

const TUNNEL_COLOR = '#2196F3'

const props = defineProps<{
  group: TunnelGroup
  /** Rang du groupe dans la liste (0-based), pour l'affichage `#n`. */
  index: number
  profile: AltimetrieProfilePoint[]
  altMin: number
  altMax: number
  totalDistanceM: number
  selected: boolean
  canMerge: boolean
  previousGapM: number | null
}>()

const emit = defineEmits<{
  click: []
  delete: []
  merge: []
  split: []
}>()

const name = computed(() => props.group.tunnels[0]?.name ?? null)

const mergeTitle = computed(
  () =>
    `Fusionner avec le tunnel précédent (écart ${Math.round(
      props.previousGapM ?? 0,
    )} m)`,
)

const totalDistance = computed(() =>
  props.totalDistanceM > 0 ? props.totalDistanceM : 1,
)

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

function xOfKm(km: number): number {
  return Math.max(
    0,
    Math.min(100, ((km * 1000) / totalDistance.value) * 100),
  )
}

const groupX = computed(() => xOfKm(props.group.startKm))
const groupWidth = computed(() =>
  Math.max(0.4, xOfKm(props.group.endKm) - groupX.value),
)

const spans = computed(() =>
  props.group.tunnels.map(tunnel => {
    const x1 = xOfKm(tunnel.startKm)
    const x2 = xOfKm(tunnel.endKm)
    return {
      id: tunnel.id,
      x1,
      width: Math.max(0.4, x2 - x1),
      mid: (x1 + x2) / 2,
      short: isShortTunnelOnProfile(
        tunnel.startKm,
        tunnel.endKm,
        props.totalDistanceM,
      ),
    }
  }),
)
</script>

<style scoped>
.alt-tunnel-card {
  cursor: pointer;
  transition:
    border-color 0.15s ease,
    background-color 0.15s ease;
}

.alt-tunnel-card:hover {
  border-color: rgba(33, 150, 243, 0.55);
}

.alt-tunnel-card--selected {
  border-color: #2196f3;
  background: rgba(33, 150, 243, 0.06);
}

.alt-tunnel-card__header {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 12px;
  min-width: 0;
}

.alt-tunnel-card__index {
  font-size: 0.8rem;
  font-weight: 600;
  color: rgba(var(--v-theme-on-surface), 0.86);
  flex: 0 0 auto;
}

.alt-tunnel-card__name {
  font-size: 0.72rem;
  color: rgba(var(--v-theme-on-surface), 0.7);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.alt-tunnel-card__sparkline {
  display: block;
  width: 100%;
  height: 42px;
  color: rgba(var(--v-theme-on-surface), 0.68);
}

.alt-tunnel-card__metrics {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 4px 12px;
  padding: 2px 12px 4px;
}

.alt-tunnel-card__sub {
  padding: 0 12px 8px;
  font-size: 0.68rem;
  color: rgba(var(--v-theme-on-surface), 0.55);
}

.alt-tunnel-card__metric {
  display: flex;
  align-items: baseline;
  gap: 4px;
  min-width: 0;
}

.alt-tunnel-card__metric-label {
  font-size: 0.65rem;
  font-weight: 400;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: rgba(var(--v-theme-on-surface), 0.55);
  white-space: nowrap;
}

.alt-tunnel-card__metric-value {
  font-size: 0.83rem;
  font-weight: 600;
  color: rgba(var(--v-theme-on-surface), 0.92);
  white-space: nowrap;
}
</style>
