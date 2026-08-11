<script setup lang="ts">
/**
 * Vue d'édition caméra (Phase 2 de la spec « Visualisation GPX sur MapBox »).
 *
 * Mise en page (spec §4.1) :
 *   - barre d'outils supérieure semi-transparente (EditionToolbar) ;
 *   - zone centrale : carte MapBox satellite + terrain (EditionMap) avec,
 *     en overlays absolus, le cadre ViewPort 16:9 (ViewportFrame) et le HUD
 *     de télémétrie (TelemetryHud) ;
 *   - bandeau inférieur fixe : contrôle de lecture (PlaybackControls) avec
 *     graphe SVG d'avancement (spec §4.6), bouton Play/Pause, sélecteur
 *     de vitesse et distance parcourue.
 *
 * Garde-fou : si aucune trace n'est sélectionnée (par exemple après un
 * rechargement direct de /edition-camera), on redirige vers l'accueil.
 */
import { onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useSettingsStore } from '../stores/settings'
import { useTracesStore } from '../stores/traces'
import { useEditionStore } from '../stores/edition'
import EditionMap from '../components/Edition/EditionMap.vue'
import EditionToolbar from '../components/Edition/EditionToolbar.vue'
import ViewportFrame from '../components/Edition/ViewportFrame.vue'
import PlaybackControls from '../components/Edition/PlaybackControls.vue'
import TelemetryHud from '../components/Edition/TelemetryHud.vue'
import DistanceHud from '../components/Edition/DistanceHud.vue'
import CameraEditor from '../components/Edition/CameraEditor.vue'

const router = useRouter()
const appStore = useAppStore()
const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const editionStore = useEditionStore()

onMounted(async () => {
  // Garde-fou : pas de trace sélectionnée → retour à l'accueil.
  if (!editionStore.selectedTraceId) {
    router.replace({ name: 'accueil' })
    return
  }

  // Précharger le nécessaire au rendu de la vue.
  await appStore.loadExecutionEnv()
  await appStore.loadModes()
  await settingsStore.loadSettings()
  await tracesStore.loadTraces()
})

// --- Raccourcis clavier ---
//   Espace        → Play/Pause ;
//   Flèche droite → pause (si lecture) + point de RdV suivant ;
//   Flèche gauche → pause (si lecture) + point de RdV précédent.

/**
 * `true` si le focus est dans un champ de saisie (on laisse alors le clavier
 * faire son travail, sans interférer avec la frappe).
 */
function isEditableTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null
  return (
    el?.tagName === 'INPUT' ||
    el?.tagName === 'TEXTAREA' ||
    !!el?.isContentEditable
  )
}

function onEditionKeydown(event: KeyboardEvent) {
  if (isEditableTarget(event.target)) return

  switch (event.code) {
    case 'Space':
      event.preventDefault()
      if (editionStore.hasKeyframes) editionStore.togglePlay()
      break
    case 'ArrowRight':
      event.preventDefault()
      editionStore.goToNextRdv()
      break
    case 'ArrowLeft':
      event.preventDefault()
      editionStore.goToPrevRdv()
      break
  }
}

// Phase de capture : intercepte les touches avant les handlers du canvas
// Mapbox (qui, lui, capte les flèches en premier sinon).
window.addEventListener('keydown', onEditionKeydown, true)

onUnmounted(() => {
  window.removeEventListener('keydown', onEditionKeydown, true)
})
</script>

<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <EditionToolbar />

      <!--
        v-main en colonne flex : la zone carte occupe toute la place
        restante au-dessus du bandeau de lecture. Le wrapper de la carte
        est positionné en relatif pour servir de repère d'ancrage aux
        overlays absolus (ViewportFrame, TelemetryHud).
      -->
      <v-main fill-height class="edition-main">
        <div class="edition-map-wrapper">
          <EditionMap />
          <ViewportFrame />
          <TelemetryHud />
          <DistanceHud />
          <CameraEditor />
        </div>
        <PlaybackControls />
      </v-main>
    </v-layout>
  </v-app>
</template>

<style scoped>
.edition-main {
  display: flex;
  flex-direction: column;
}

/* La zone carte prend toute la hauteur disponible ; repère d'ancrage des overlays. */
.edition-map-wrapper {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
}
</style>
