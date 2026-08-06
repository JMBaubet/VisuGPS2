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
 * Semi-transparente par-dessus la carte, elle expose pour le MVP :
 *   - un bouton Home (retour à l'accueil)
 *   - le titre de la trace éditée
 *   - un toggle d'affichage du cadre ViewPort 16:9 (overlay CSS)
 *
 * Elle s'enrichira dans les prochaines itérations : bouton « + » d'ajout
 * de keyframe (spec §4.7), toggles globaux, etc.
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
</script>

<style scoped>
/* Barre semi-transparente pour laisser deviner la carte en arrière-plan. */
.edition-toolbar {
  background-color: rgba(var(--v-theme-surface), 0.82);
  backdrop-filter: blur(4px);
}
</style>
