<template>
  <v-app-bar flat class="edition-toolbar" density="compact">
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

    <!--
      Cadre ViewPort : flip-flop 16:9 / 4:3.
      Icône = ratio (monitor / monitor-small) ; couleur = taux de verrouillage
      (vert 100 %, jaune > 50 %, orange ≥ 10 %, rouge sinon).
    -->
    <v-btn
      icon
      :color="viewportColor"
      :title="
        `ViewPort ${editionStore.viewportAspect} · ${lockPct} % des segments verrouillés — cliquer pour basculer`
      "
      @click="onViewportFlip"
    >
      <v-icon>
        {{ editionStore.viewportAspect === '16:9' ? 'mdi-monitor' : 'mdi-monitor-small' }}
      </v-icon>
    </v-btn>

    <!-- Panneau « Changements de cap brutaux » (tableau à la demande) -->
    <v-btn
      icon
      :color="editionStore.showHeadingChangesPanel ? 'primary' : ''"
      title="Changements de cap brutaux"
      @click="editionStore.toggleHeadingChangesPanel()"
    >
      <v-icon>mdi-rotate-3d-variant</v-icon>
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

    <!-- Paramètres de la vue (icône seule pour l'instant, comme la toolbar Accueil) -->
    <v-btn icon="mdi-cog-outline" slim title="Paramètres de la vue Edition"></v-btn>

    <!-- Quitter l'édition (retour à l'accueil), complètement à droite -->
    <v-btn icon :to="{ name: 'accueil' }" title="Quitter l'édition — retour à l'accueil">
      <v-icon>mdi-location-exit</v-icon>
    </v-btn>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre d'outils supérieure de la vue d'édition caméra.
 *
 * Semi-transparente par-dessus la carte, elle expose :
 *   - le titre de la trace éditée
 *   - le sélecteur de l'algorithme de génération des keyframes
 *     (`'frustum'` — placement par visibilité — ou `'simple'` — MVP)
 *   - la distance minimale entre keyframes (frustum, 200–5000 m, pas 50)
 *   - le bouton **ViewPort** : **flip-flop** entre 16:9 et 4:3 — icône
 *     `mdi-monitor` (16:9) / `mdi-monitor-small` (4:3), chaque ratio exploite
 *     son propre fichier keyframes et son cadre ; **couleur** = avancement du
 *     verrouillage (vert 100 %, jaune > 50 %, orange ≥ 10 %, rouge sinon)
 *   - le bouton **Cap brutaux** : affiche/masque le tableau des changements de
 *     cap brutaux
 *   - le bouton **Mode validation** (`mdi-camera-lock`) : pendant la lecture,
 *     un clic carte signale un problème (segment déverrouillé), les segments
 *     sans clic sont verrouillés
 *   - le bouton **Paramètres** (`mdi-cog-outline`, comme la toolbar Accueil) :
 *     icône seule pour l'instant, pas encore câblée
 *   - le bouton **Quitter** (`mdi-location-exit`, complètement à droite de la
 *     barre) : retour à l'accueil
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

// --- Bouton ViewPort : ratio (icône) + avancement du verrouillage (couleur) ---

/** Ratio (0..1) de segments verrouillés sur l'ensemble des segments. */
const lockRatio = computed(() => {
  const all = editionStore.headingChanges.length
  if (all === 0) return 0
  const locked = editionStore.headingChanges.filter(h =>
    editionStore.isSegmentLocked(h.fromDistanceM),
  ).length
  return locked / all
})

/** Pourcentage de segments verrouillés (pour le tooltip). */
const lockPct = computed(() => Math.round(lockRatio.value * 100))

/**
 * Couleur de l'icône ViewPort selon l'avancement du verrouillage :
 * vert si **tous** les segments sont verrouillés, jaune si **> 50 %**, orange
 * si **≥ 10 %**, rouge sinon (validation pas encore avancée).
 */
const viewportColor = computed(() => {
  const r = lockRatio.value
  if (r >= 1) return '#4CAF50' // vert
  if (r > 0.5) return '#FFEB3B' // jaune
  if (r >= 0.1) return '#FF9800' // orange
  return '#F44336' // rouge
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

// --- Bouton ViewPort (flip-flop 16:9 / 4:3) ---

/**
 * Bascule (flip-flop) du ratio d'écran entre 16:9 et 4:3. Affiche le cadre et
 * charge/génère le fichier keyframes du ratio sélectionné.
 */
function onViewportFlip() {
  editionStore.setViewportAspect(editionStore.viewportAspect === '16:9' ? '4:3' : '16:9')
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
