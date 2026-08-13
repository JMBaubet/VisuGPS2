<template>
  <v-app-bar flat class="edition-toolbar" density="compact">
    <!-- Retour à l'accueil -->
    <v-btn icon :to="{ name: 'accueil' }" title="Accueil">
      <v-icon>mdi-home</v-icon>
    </v-btn>

    <v-app-bar-title class="text-truncate">
      {{ traceName }}
    </v-app-bar-title>

    <v-spacer />

    <!-- Sélecteur de l'algorithme de génération des keyframes -->
    <v-select
      :model-value="editionStore.keyframeAlgorithm"
      :items="algorithmItems"
      density="compact"
      hide-details
      variant="outlined"
      class="algo-select"
      label="Algorithme"
      @update:model-value="onAlgorithmChange"
    />

    <!-- Distance minimale entre keyframes (frustum) -->
    <v-text-field
      :model-value="String(editionStore.minKeyframeGapM)"
      density="compact"
      hide-details
      variant="outlined"
      type="number"
      min="200"
      max="5000"
      step="50"
      label="Gap min (m)"
      class="gap-field"
      @change="onGapChange"
    />

    <!-- Bascule du cadre ViewPort 16:9 -->
    <v-btn
      icon
      :color="editionStore.showViewportFrame ? 'primary' : ''"
      title="Afficher / masquer le cadre ViewPort 16:9"
      @click="editionStore.toggleViewportFrame()"
    >
      <v-icon>{{ editionStore.showViewportFrame ? 'mdi-monitor' : 'mdi-monitor-off' }}</v-icon>
    </v-btn>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre d'outils supérieure de la vue d'édition caméra.
 *
 * Semi-transparente par-dessus la carte, elle expose :
 *   - un bouton Home (retour à l'accueil)
 *   - le titre de la trace éditée
 *   - le sélecteur de l'algorithme de génération des keyframes
 *     (`'frustum'` — placement par visibilité — ou `'simple'` — MVP)
 *   - la distance minimale entre keyframes (frustum, 200–5000 m, pas 50)
 *   - un toggle d'affichage du cadre ViewPort 16:9 (overlay CSS)
 */
import { computed } from 'vue'
import { useEditionStore } from '../../stores/edition'
import { useTracesStore } from '../../stores/traces'

const editionStore = useEditionStore()
const tracesStore = useTracesStore()

/** Nom de la trace éditée, lu depuis le store des traces. */
const traceName = computed(() => {
  const id = editionStore.selectedTraceId
  if (!id) return 'Édition caméra'
  return tracesStore.traces.find(t => t.id === id)?.name ?? 'Édition caméra'
})

/** Options du sélecteur d'algorithme. */
const algorithmItems = [
  { title: 'Frustum', value: 'frustum' },
  { title: 'Simple', value: 'simple' },
]

function onAlgorithmChange(value: unknown) {
  if (value === 'frustum' || value === 'simple') {
    editionStore.setKeyframeAlgorithm(value)
  }
}

function onGapChange(event: Event) {
  const input = (event.target as HTMLInputElement)?.value
  const n = input !== undefined && input !== '' ? Number(input) : NaN
  if (!Number.isNaN(n)) editionStore.setMinKeyframeGapM(n)
}
</script>

<style scoped>
/* Barre semi-transparente pour laisser deviner la carte en arrière-plan. */
.edition-toolbar {
  background-color: rgba(var(--v-theme-surface), 0.82);
  backdrop-filter: blur(4px);
}

/* Largeurs des champs Algorithme / Gap min. */
.algo-select {
  max-width: 150px;
}
.gap-field {
  max-width: 130px;
}

</style>
