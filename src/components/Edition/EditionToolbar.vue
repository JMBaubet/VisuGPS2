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

    <!-- Seuil de détection des changements de cap brutaux (°/km) -->
    <v-text-field
      :model-value="String(editionStore.headingChangeThresholdDegPerKm)"
      density="compact"
      hide-details
      variant="outlined"
      type="number"
      min="10"
      max="1000"
      step="5"
      label="Seuil cap (°/km)"
      class="threshold-field"
      @change="onThresholdChange"
    />

    <!-- Cadre ViewPort : sélection du ratio d'écran + affichage/masquage -->
    <v-menu
      v-model="viewportMenuOpen"
      location="bottom end"
      :close-on-content-click="true"
    >
      <template #activator="{ props }">
        <v-btn
          icon
          v-bind="props"
          :color="editionStore.showViewportFrame ? 'primary' : ''"
          :title="
            editionStore.showViewportFrame
              ? `Cadre ViewPort · ${editionStore.viewportAspect}`
              : 'Cadre ViewPort masqué'
          "
        >
          <v-icon>{{ editionStore.showViewportFrame ? 'mdi-monitor' : 'mdi-monitor-off' }}</v-icon>
        </v-btn>
      </template>
      <v-list density="compact" class="viewport-menu">
        <v-list-item
          v-for="item in aspectItems"
          :key="item.value"
          :active="editionStore.viewportAspect === item.value"
          @click="onAspectSelect(item.value)"
        >
          <v-list-item-title>{{ item.title }}</v-list-item-title>
          <v-list-item-append-icon v-if="editionStore.viewportAspect === item.value">
            mdi-check
          </v-list-item-append-icon>
        </v-list-item>
        <v-divider />
        <v-list-item @click="editionStore.toggleViewportFrame()">
          <v-list-item-title>
            {{ editionStore.showViewportFrame ? 'Masquer le cadre' : 'Afficher le cadre' }}
          </v-list-item-title>
        </v-list-item>
      </v-list>
    </v-menu>

    <!-- Panneau « Changements de cap brutaux » (tableau à la demande) -->
    <v-btn
      icon
      :color="editionStore.showHeadingChangesPanel ? 'primary' : ''"
      title="Changements de cap brutaux"
      @click="editionStore.toggleHeadingChangesPanel()"
    >
      <v-icon>mdi-rotate-3d</v-icon>
      <v-badge
        v-if="brutalCount > 0"
        :content="String(brutalCount)"
        color="error"
        offset-x="-6"
        offset-y="-6"
      />
    </v-btn>

    <!-- Mode validation : verrouillage automatique des segments sans clic -->
    <v-btn
      icon
      :color="editionStore.validationMode ? 'primary' : ''"
      :title="
        editionStore.validationMode
          ? 'Mode validation actif — clic sur la carte = problème, segments sans clic verrouillés'
          : 'Mode validation (verrouille les segments sans clic)'
      "
      @click="editionStore.toggleValidationMode()"
    >
      <v-icon>mdi-camera-lock</v-icon>
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
 *   - le seuil de détection des **changements de cap brutaux** (°/km,
 *     10–1000, pas 5 — filtre d'affichage, sans régénération)
 *   - le menu **ViewPort** : sélection du ratio d'écran (16:9 / 4:3 — chaque
 *     ratio exploite son propre fichier keyframes) et affichage/masquage du
 *     cadre (overlay CSS)
 *   - le bouton **Cap brutaux** (badge du nombre de virages détectés) :
 *     affiche/masque le tableau des changements de cap brutaux
 *   - le bouton **Mode validation** (`mdi-camera-lock`) : pendant la lecture,
 *     un clic carte signale un problème (segment déverrouillé), les segments
 *     sans clic sont verrouillés
 */
import { computed, ref } from 'vue'
import { useEditionStore } from '../../stores/edition'
import { useTracesStore } from '../../stores/traces'
import {
  VIEWPORTS_BY_ASPECT,
  type ViewportAspect,
} from '../../algorithms/keyframeGenerator'

const editionStore = useEditionStore()
const tracesStore = useTracesStore()

/** Nom de la trace éditée, lu depuis le store des traces. */
const traceName = computed(() => {
  const id = editionStore.selectedTraceId
  if (!id) return 'Édition caméra'
  return tracesStore.traces.find(t => t.id === id)?.name ?? 'Édition caméra'
})

/** Nombre de changements de cap brutaux (badge du bouton). */
const brutalCount = computed(() => editionStore.brutalHeadingChanges.length)

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

/** Change le seuil de détection des changements de cap brutaux (°/km). */
function onThresholdChange(event: Event) {
  const input = (event.target as HTMLInputElement)?.value
  const n = input !== undefined && input !== '' ? Number(input) : NaN
  if (!Number.isNaN(n)) editionStore.setHeadingChangeThreshold(n)
}

// --- Menu ViewPort (ratio d'écran + visibilité du cadre) ---

/** Ouverture du menu ViewPort. */
const viewportMenuOpen = ref(false)

/** Ratios proposés, avec leurs dimensions de référence. */
const aspectItems = (['16:9', '4:3'] as ViewportAspect[]).map(a => ({
  value: a,
  title: `ViewPort ${a} · ${VIEWPORTS_BY_ASPECT[a].width}×${VIEWPORTS_BY_ASPECT[a].height}`,
}))

/** Sélectionne un ratio : affiche son cadre et exploite son fichier keyframes. */
function onAspectSelect(aspect: ViewportAspect) {
  viewportMenuOpen.value = false
  editionStore.setViewportAspect(aspect)
}
</script>

<style scoped>
/* Barre semi-transparente pour laisser deviner la carte en arrière-plan. */
.edition-toolbar {
  background-color: rgba(var(--v-theme-surface), 0.82);
  backdrop-filter: blur(4px);
}

/* Largeurs des champs Algorithme / Gap min / Seuil cap. */
.algo-select {
  max-width: 150px;
}
.gap-field {
  max-width: 130px;
}
.threshold-field {
  max-width: 130px;
}

/* Menu ViewPort (sélection du ratio d'écran). */
.viewport-menu {
  min-width: 200px;
}

</style>
