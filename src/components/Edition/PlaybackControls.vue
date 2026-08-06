<template>
  <v-sheet class="playback-controls" color="rgba(0,0,0,0.75)" tile>
    <div class="d-flex align-center px-4 py-2 gap-3">
      <!-- Lecture / Pause -->
      <v-btn
        icon
        size="large"
        color="white"
        variant="text"
        :disabled="!editionStore.hasKeyframes"
        :title="editionStore.isPlaying ? 'Pause' : 'Lecture'"
        @click="editionStore.togglePlay()"
      >
        <v-icon>{{ editionStore.isPlaying ? 'mdi-pause' : 'mdi-play' }}</v-icon>
      </v-btn>

      <!-- Vitesse -->
      <v-btn-toggle
        :model-value="editionStore.speed"
        mandatory
        density="compact"
        color="primary"
        @update:model-value="onSpeedChange"
      >
        <v-btn value="0.5" size="small">0.5×</v-btn>
        <v-btn value="1" size="small">1×</v-btn>
        <v-btn value="2" size="small">2×</v-btn>
        <v-btn value="4" size="small">4×</v-btn>
      </v-btn-toggle>

      <v-spacer />

      <!-- Distance parcourue -->
      <div class="text-body-2 text-white text-no-wrap">
        Distance parcourue&nbsp;:
        <strong>{{ currentKm }}</strong> /
        <span class="opacity-70">{{ totalKm }}</span>
      </div>
    </div>
  </v-sheet>
</template>

<script setup lang="ts">
/**
 * Composant A — Contrôle de lecture (spec §4.3).
 *
 * Bandeau inférieur de la vue d'édition. Pour ce MVP il fournit :
 *   - un bouton Play / Pause ;
 *   - un sélecteur de vitesse (0.5× / 1× / 2× / 4×) ;
 *   - l'affichage « Distance parcourue : X.XX km / Y.YY km ».
 *
 * Tout l'état vit dans `editionStore` ; ce composant ne fait que lire et
 * déclencher des actions. Il sera enrichi lors de l'arrivée du graphe SVG
 * d'avancement (spec §4.6), qui prendra place dans ce même bandeau.
 */
import { computed } from 'vue'
import { useEditionStore } from '../../stores/edition'

const editionStore = useEditionStore()

const currentKm = computed(() => editionStore.currentDistanceKm.toFixed(2))
const totalKm = computed(() => editionStore.totalDistanceKm.toFixed(2))

/** Le v-btn-toggle émet des chaînes ; on convertit en nombre pour le store. */
function onSpeedChange(value: unknown) {
  const n = typeof value === 'string' ? parseFloat(value) : Number(value)
  if (!Number.isNaN(n)) editionStore.setSpeed(n)
}
</script>

<style scoped>
.playback-controls {
  /* Bandeau bas fixe, hauteur modeste (le graphe viendra l'agrandir). */
  min-height: 56px;
}

.gap-3 {
  gap: 12px;
}
</style>
