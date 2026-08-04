<template>
  <v-overlay
    :model-value="visible"
    contained
    persistent
    scroll-strategy="none"
    class="align-center justify-center"
  >
    <v-card
      width="420"
      class="pa-2"
      :title="title"
      subtitle="Pré-calcul des keyframes"
    >
      <template #prepend>
        <v-icon color="primary" icon="mdi-progress-clock" />
      </template>

      <v-card-text>
        <!-- Libellé de l'étape courante -->
        <div class="text-body-2 mb-2 text-medium-emphasis">
          {{ stepLabel || 'Initialisation…' }}
        </div>

        <!-- Barre de progression -->
        <v-progress-linear
          :model-value="progressPercent"
          color="primary"
          height="10"
          rounded
          striped
        />

        <div class="text-caption text-right text-medium-emphasis mt-1">
          {{ progressPercent }} %
        </div>
      </v-card-text>
    </v-card>
  </v-overlay>
</template>

<script setup lang="ts">
/**
 * Overlay de progression du pré-calcul (Mode 1).
 *
 * Affiché par dessus la map 3D pendant que `useKeyframeEngine.precomputeKeyframes`
 * s'exécute. Consomme l'état réactif du store `keyframes` (`precomputeStatus`,
 * `precomputeProgress`, `precomputeStep`) ; aucun prop à maintenir.
 */
import { computed } from 'vue'
import { useKeyframesStore } from '../../stores/keyframes'

const keyframesStore = useKeyframesStore()

/** Visible uniquement pendant l'exécution du pré-calcul. */
const visible = computed(() => keyframesStore.precomputeStatus === 'running')

/** Titre de l'overlay, dépendant du succès/échec (reste visible un court instant). */
const title = computed(() => {
  switch (keyframesStore.precomputeStatus) {
    case 'running': return 'Calcul en cours'
    case 'done': return 'Calcul terminé'
    case 'error': return 'Échec du calcul'
    default: return 'Pré-calcul'
  }
})

const stepLabel = computed(() => keyframesStore.precomputeStep)

const progressPercent = computed(() =>
  Math.round((keyframesStore.precomputeProgress ?? 0) * 100),
)
</script>
