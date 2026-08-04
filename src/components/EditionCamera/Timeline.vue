<template>
  <div class="timeline-panel">
    <!-- Ligne de contrôles de lecture -->
    <div class="d-flex align-center px-3 pt-2 pb-1">
      <v-btn
        :icon="keyframesStore.isPlaying ? 'mdi-pause' : 'mdi-play'"
        variant="text"
        color="primary"
        :disabled="!keyframesStore.hasRawKeyframes"
        :title="keyframesStore.isPlaying ? 'Pause' : 'Lecture'"
        @click="keyframesStore.togglePlay"
      />

      <!-- Sélecteur de vitesse (M5 l'utilisera dans la boucle de lecture) -->
      <v-select
        v-model="speedModel"
        :items="speedOptions"
        density="compact"
        variant="outlined"
        hide-details
        label="Vitesse"
        class="speed-select ml-2"
      />

      <v-spacer />

      <!-- Compteurs temps / distance courants -->
      <div class="text-caption text-medium-emphasis d-flex ga-3">
        <span>{{ formattedCurrentTime }} / {{ formattedTotalTime }}</span>
        <span>{{ formattedCurrentDistance }} / {{ formattedTotalDistance }}</span>
      </div>
    </div>

    <!-- Curseur de timeline avec repères d'override -->
    <div class="timeline-track-wrapper px-3 pb-2">
      <!-- Repères visuels des overrides (zones colorées sous le curseur) -->
      <div class="override-markers">
        <div
          v-for="ov in keyframesStore.overrides.overrides"
          :key="ov.id"
          class="override-marker"
          :class="{ 'override-marker--disabled': ov.disabled }"
          :style="overrideStyle(ov)"
          :title="ov.name"
        />
      </div>

      <!-- Curseur (v-slider horizontal plein écran) -->
      <v-slider
        :model-value="progressValue"
        :min="0"
        :max="1"
        :step="stepValue"
        :disabled="!keyframesStore.hasRawKeyframes"
        hide-details
        color="primary"
        track-color="surface-variant"
        class="timeline-slider"
        @update:model-value="onSeekFraction"
      >
        <template #prepend>
          <span class="text-caption">{{ formattedTotalTime }}</span>
        </template>
        <template #append>
          <span class="text-caption">{{ formattedTotalDistance }}</span>
        </template>
      </v-slider>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Timeline interactive de l'atelier d'édition (§10.2).
 *
 * Affiche :
 *  - un curseur de progression (v-slider) couvrant toute la durée ;
 *  - les contrôles de lecture (play/pause + sélecteur de vitesse) ;
 *  - des compteurs temps (HH:MM:SS) et distance (km) ;
 *  - des repères visuels pour chaque override (zones colorées).
 *
 * La timeline ne dépend pas d'une instance Mapbox : elle lit/écrit uniquement
 * le store `keyframes`. Le moteur de lecture (boucle RAF) vit dans le composable
 * `usePlayback` (M5) ; ce composant ne fait que déclencher play/pause et seek.
 */
import { computed } from 'vue'
import { useKeyframesStore } from '../../stores/keyframes'
import { formatDuration, formatDistance } from '../../utils/format'
import type { Override } from '../../utils/keyframes'

const keyframesStore = useKeyframesStore()

/** Vitesse de lecture disponibles. */
const speedOptions = [0.5, 1, 2]

/** Modèle bidirectionnel pour le sélecteur de vitesse. */
const speedModel = computed<number>({
  get: () => keyframesStore.playbackSpeed,
  set: (v: number) => { keyframesStore.playbackSpeed = v },
})

/** Progression normalisée 0 → 1 (pour le v-slider). */
const progressValue = computed(() => {
  const total = keyframesStore.totalDuration
  if (total <= 0) return 0
  return keyframesStore.currentTime / total
})

/**
 * Pas du slider : un pas de `sampleRate` ms donne une précision suffisante
 * sans saturer l'UI (ex. 100 ms → 1/totalDuration × 100 pas).
 */
const stepValue = computed(() => {
  const total = keyframesStore.totalDuration
  if (total <= 0) return 0.001
  return Math.max(0.0005, 100 / total)
})

/** Gestion du seek : convertit la fraction 0→1 en millisecondes. */
function onSeekFraction(fraction: number): void {
  keyframesStore.seek(fraction * keyframesStore.totalDuration)
}

// --- Formatage ---

const formattedCurrentTime = computed(() =>
  formatDuration(keyframesStore.currentTime / 1000),
)
const formattedTotalTime = computed(() =>
  formatDuration(keyframesStore.totalDuration / 1000),
)
const formattedCurrentDistance = computed(() => {
  const fraction = progressValue.value
  return formatDistance(fraction * keyframesStore.totalDistance)
})
const formattedTotalDistance = computed(() =>
  formatDistance(keyframesStore.totalDistance),
)

/**
 * Style inline positionnant un marqueur d'override sur la timeline.
 * La largeur et le `left` sont des fractions 0→100 % de la durée totale.
 */
function overrideStyle(ov: Override): Record<string, string> {
  const total = keyframesStore.totalDuration || 1
  const left = (ov.start_time / total) * 100
  const width = Math.max(0.5, ((ov.end_time - ov.start_time) / total) * 100)
  return {
    left: `${left}%`,
    width: `${width}%`,
  }
}
</script>

<style scoped>
.timeline-panel {
  /* Bande basse de l'atelier, sous la map. */
  background: rgb(var(--v-theme-surface));
  border-top: thin solid rgba(var(--v-theme-on-surface), 0.12);
}

.speed-select {
  max-width: 110px;
}

.timeline-track-wrapper {
  position: relative;
}

/* Conteneur des marqueurs d'override, aligné sur la piste du slider. */
.override-markers {
  position: absolute;
  left: 16px;
  right: 16px;
  top: 6px;
  height: 6px;
  pointer-events: none;
}

.override-marker {
  position: absolute;
  top: 0;
  height: 6px;
  background-color: rgb(var(--v-theme-warning));
  border-radius: 2px;
  opacity: 0.85;
}

.override-marker--disabled {
  opacity: 0.35;
  background-color: rgb(var(--v-theme-on-surface));
}

/* Le slider occupe toute la largeur disponible. */
.timeline-slider {
  width: 100%;
}
</style>
