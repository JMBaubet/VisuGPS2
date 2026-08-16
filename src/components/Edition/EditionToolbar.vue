<template>
  <v-app-bar flat class="edition-toolbar" density="compact">
    <v-app-bar-title class="text-truncate">
      {{ traceName }}
    </v-app-bar-title>

    <v-spacer />

    <!--
      Cadre ViewPort : flip-flop 16:9 / 4:3.
      Icône = ratio (monitor / monitor-small) ; couleur = taux de verrouillage
      (vert 100 %, jaune > 50 %, orange ≥ 10 %, rouge sinon).
    -->
    <v-btn
      icon
      :color="viewportColor"
      :title="viewportTitle"
      @click="onViewportFlip"
    >
      <v-icon>
        {{ editionStore.viewportAspect === '16:9' ? 'mdi-monitor' : 'mdi-monitor-small' }}
      </v-icon>
    </v-btn>

    <!-- Mode validation : verrouillage automatique des segments sans clic -->
    <v-btn
      icon
      :color="editionStore.validationMode ? 'primary' : ''"
      :title="
        editionStore.validationMode
          ? 'Mode validation actif — clic carte ou Entrée = problème, segments sans clic verrouillés'
          : 'Mode validation (verrouille les segments sans clic)'
      "
      @click="editionStore.toggleValidationMode()"
    >
      <v-icon>mdi-camera-lock</v-icon>
    </v-btn>

    <!-- Paramètres de la vue (panneau Paramètres, comme la toolbar Accueil) -->
    <v-btn
      icon="mdi-cog-outline"
      slim
      :color="appStore.isSettingsDrawerOpen ? 'primary' : ''"
      :title="appStore.isSettingsDrawerOpen ? 'Fermer les paramètres' : 'Paramètres de la vue Edition'"
      @click="emit('open-settings')"
    ></v-btn>

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
 *   - le bouton **ViewPort** : **flip-flop** entre 16:9 et 4:3 — icône
 *     `mdi-monitor` (16:9) / `mdi-monitor-small` (4:3), chaque ratio exploite
 *     son propre fichier keyframes et son cadre ; **couleur** = avancement du
 *     verrouillage (vert 100 %, jaune > 50 %, orange ≥ 10 %, rouge sinon)
 *   - le bouton **Mode validation** (`mdi-camera-lock`) : pendant la lecture,
 *     un clic carte signale un problème (segment déverrouillé), les segments
 *     sans clic sont verrouillés
 *   - le bouton **Paramètres** (`mdi-cog-outline`, comme la toolbar Accueil) :
 *     ouvre le panneau Paramètres de la vue (algorithme, gap min, zoom/pitch,
 *     viewport et couleurs y sont gérés)
 *   - le bouton **Quitter** (`mdi-location-exit`, complètement à droite de la
 *     barre) : retour à l'accueil
 */
import { computed } from 'vue'
import { useAppStore } from '../../stores/app'
import { useEditionStore } from '../../stores/edition'
import { useTracesStore } from '../../stores/traces'

/** Ouvre le panneau Paramètres de la vue (reçu par EditionCamera). */
const emit = defineEmits<{ (e: 'open-settings'): void }>()

const appStore = useAppStore()
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
 *
 * **Neutre au lancement** : tant que la vue n'est pas prête
 * (`editionViewReady` false), l'icône n'affiche pas la couleur de la session
 * précédente — elle n'est colorée qu'à la révélation, avec le ratio de la
 * trace courante.
 */
const viewportColor = computed(() => {
  if (!editionStore.editionViewReady) return ''
  const r = lockRatio.value
  if (r >= 1) return '#4CAF50' // vert
  if (r > 0.5) return '#FFEB3B' // jaune
  if (r >= 0.1) return '#FF9800' // orange
  return '#F44336' // rouge
})

/**
 * Tooltip du bouton ViewPort : neutre pendant le chargement, sinon ratio actif
 * + pourcentage de segments verrouillés.
 */
const viewportTitle = computed(() => {
  if (!editionStore.editionViewReady) return 'ViewPort — chargement de la trace…'
  return `ViewPort ${editionStore.viewportAspect} · ${lockPct} % des segments verrouillés — cliquer pour basculer`
})

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

</style>
