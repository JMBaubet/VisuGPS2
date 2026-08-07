<template>
  <v-sheet class="playback-controls" color="rgba(0,0,0,0.75)" tile>
    <!--
      Layout horizontal : colonne de boutons à gauche + graphe à droite.
      La colonne fait 2 rangées :
        - rangée 1 (alignée avec la zone RdV) : RdV précédent / suivant ;
        - rangée 2 (alignée avec la zone avancement) : km0 / Play-Pause / dernier point.
    -->
    <div class="controls-col">
      <!-- Rangée 1 : navigation entre Points de RdV (icônes bleues) -->
      <div class="rdv-row">
        <v-btn
          icon
          size="small"
          variant="text"
          color="info"
          :disabled="!canGoPrev"
          title="Point de RdV précédent"
          @click="goToPrevRdv"
        >
          <v-icon>mdi-chevron-left</v-icon>
        </v-btn>
        <v-btn
          icon
          size="small"
          variant="text"
          color="info"
          :disabled="!canGoNext"
          title="Point de RdV suivant"
          @click="goToNextRdv"
        >
          <v-icon>mdi-chevron-right</v-icon>
        </v-btn>
      </div>

      <!-- Rangée 2 : navigation principale (km0 / Play-Pause / dernier point) -->
      <div class="play-row">
        <v-btn
          icon
          size="small"
          variant="text"
          color="white"
          :disabled="!canGoStart"
          title="Aller au km 0"
          @click="editionStore.seekToDistance(0)"
        >
          <v-icon>mdi-skip-backward</v-icon>
        </v-btn>
        <v-btn
          icon
          size="small"
          color="white"
          variant="text"
          :disabled="!editionStore.hasKeyframes"
          :title="editionStore.isPlaying ? 'Pause' : 'Lecture'"
          @click="editionStore.togglePlay()"
        >
          <v-icon>{{ editionStore.isPlaying ? 'mdi-pause' : 'mdi-play' }}</v-icon>
        </v-btn>
        <v-btn
          icon
          size="small"
          variant="text"
          color="white"
          :disabled="!canGoEnd"
          title="Aller au dernier point"
          @click="editionStore.seekToDistance(editionStore.totalDistanceM)"
        >
          <v-icon>mdi-skip-forward</v-icon>
        </v-btn>
      </div>
    </div>

    <!-- Graphe SVG d'avancement (occupe le reste de la largeur) -->
    <ProgressGraph class="graph-flex" />
  </v-sheet>
</template>

<script setup lang="ts">
/**
 * Composant A — Contrôle de lecture (spec §4.3).
 *
 * Bandeau inférieur de la vue d'édition, en **layout horizontal compact** :
 *   - à gauche, une colonne de boutons sur 2 rangées :
 *     • rangée 1 (alignée zone RdV) : Points de RdV précédent / suivant ;
 *     • rangée 2 (alignée zone avancement) : km0 / Play-Pause / dernier point ;
 *   - à droite, le graphe SVG d'avancement (spec §4.6).
 *
 * La distance parcourue est affichée en permanence sous le curseur rouge du
 * graphe (tooltip jaune), complétée par le tooltip de survol (distance +
 * altitude au point survolé).
 *
 * Vitesse fixe à 1× (le contrôle de vitesse a été retiré).
 */
import { computed } from 'vue'
import { useEditionStore } from '../../stores/edition'
import ProgressGraph from './ProgressGraph.vue'

const editionStore = useEditionStore()

/** Distance en dessous de laquelle on considère qu'on est sur un RdV (m). */
const ROLLOVER_EPSILON_M = 0.5

const keyframes = computed(() => editionStore.keyframeSet?.keyframes ?? [])

/** Index du prochain RdV strictement après la position courante, ou -1. */
function nextKeyframeIndex(): number {
  const cur = editionStore.currentDistanceM
  return keyframes.value.findIndex(k => k.distance_from_start_m > cur + ROLLOVER_EPSILON_M)
}

/** Index du dernier RdV strictement avant la position courante, ou -1. */
function prevKeyframeIndex(): number {
  const cur = editionStore.currentDistanceM
  for (let i = keyframes.value.length - 1; i >= 0; i--) {
    if (keyframes.value[i].distance_from_start_m < cur - ROLLOVER_EPSILON_M) return i
  }
  return -1
}

function goToPrevRdv() {
  const i = prevKeyframeIndex()
  if (i >= 0) editionStore.seekToDistance(keyframes.value[i].distance_from_start_m)
}

function goToNextRdv() {
  const i = nextKeyframeIndex()
  if (i >= 0) editionStore.seekToDistance(keyframes.value[i].distance_from_start_m)
}

const canGoPrev = computed(() => prevKeyframeIndex() >= 0)
const canGoNext = computed(() => nextKeyframeIndex() >= 0)
const canGoStart = computed(() => editionStore.currentDistanceM > ROLLOVER_EPSILON_M)
const canGoEnd = computed(
  () => editionStore.currentDistanceM < editionStore.totalDistanceM - ROLLOVER_EPSILON_M,
)
</script>

<style scoped>
.playback-controls {
  /* Bandeau : colonne boutons (2 rangées) + graphe (~82px). */
  height: 82px;
  display: flex;
  align-items: stretch;
}

.controls-col {
  /* Colonne fixe à gauche : 2 rangées collées, centrées verticalement. */
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0;
  padding: 0 4px;
  flex-shrink: 0;
}

.rdv-row,
.play-row {
  display: flex;
  align-items: center;
  gap: 2px;
}

.graph-flex {
  flex: 1;
  min-width: 0;
}
</style>
