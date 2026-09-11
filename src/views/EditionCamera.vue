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
import { onMounted, onUnmounted, watch } from 'vue'
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
import HeadingChangesPanel from '../components/Edition/HeadingChangesPanel.vue'
import CameraEditor from '../components/Edition/CameraEditor.vue'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'

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

  // Garde-fou « validité » : une trace non « clean » (anomalies détectées, audit
  // non appliqué) n'est pas candidate à l'édition caméra — on redirige vers la
  // vue d'audit, qui reçoit la trace par la query de la route.
  const trace = tracesStore.traces.find(t => t.id === editionStore.selectedTraceId)
  if (trace && trace.audit_status !== 'clean') {
    router.replace({ name: 'audit', query: { traceId: trace.id } })
    return
  }

  // (Le seeding initial des paramètres d'édition est assuré par EditionMap
  // avant la génération des keyframes : `loadSettings` + `applySettings(true)`.)
})

// Réactivité live : toute sauvegarde d'un paramètre dans le panneau Paramètres
// (rechargement de `settings`) est répercutée sur le store édition — sans
// toucher au viewport actif (le flip-flop de session n'est pas écrasé).
watch(
  () => settingsStore.settings,
  () => editionStore.applySettings(false),
)

// Mise à jour live du **viewport** : quand c'est **ce paramètre** qui change
// dans le panneau Paramètres, on bascule le ratio actif (cadre ViewPort +
// fichier keyframes du ratio). Les autres paramètres ne déclenchent pas ce
// watch (la valeur lue ne change pas) → le flip-flop de session est préservé.
watch(
  () => settingsStore.settings.find(s => s.path === 'Edition.Camera.viewportDefaut')?.value,
  vp => {
    if (vp === '16:9' || vp === '4:3') editionStore.setViewportAspect(vp)
  },
)

// Mise à jour live de l'affichage du panneau des changements de cap brutaux
// (paramètre comme le HUD de télémétrie) : n'applique que quand **ce paramètre**
// change, pour ne pas écraser la bascule manuelle de la toolbar.
watch(
  () =>
    settingsStore.settings.find(s => s.path === 'Edition.Camera.afficherCapBrutaux')?.value,
  visible => {
    if (typeof visible === 'boolean') editionStore.setHeadingChangesPanel(visible)
  },
)

// Mise à jour live du HUD de télémétrie : idem, pour ne pas réafficher le HUD
// après une fermeture manuelle (bouton Fermer) quand un autre paramètre change.
watch(
  () =>
    settingsStore.settings.find(s => s.path === 'Edition.Camera.afficherTelemetrie')?.value,
  visible => {
    if (typeof visible === 'boolean') editionStore.setTelemetryHud(visible)
  },
)

// --- Raccourcis clavier ---
//   Espace        → Play/Pause ;
//   Entrée        → en mode validation : déverrouille le segment courant dans
//                   tous les cas (équivalent au clic carte, y compris s'il
//                   était déjà verrouillé) ;
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
    case 'Enter':
    case 'NumpadEnter':
      // En mode validation : Entrée = clic carte — le segment courant est
      // déverrouillé dans tous les cas (mémorisé + déverrouillé s'il était
      // verrouillé). Sans effet hors validation.
      if (!editionStore.validationMode) break
      event.preventDefault()
      editionStore.markValidationClick()
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
      <EditionToolbar @open-settings="appStore.isSettingsDrawerOpen = !appStore.isSettingsDrawerOpen" />

      <!--
        v-main en colonne flex : la zone carte occupe toute la place
        restante au-dessus du bandeau de lecture. Le wrapper de la carte
        est positionné en relatif pour servir de repère d'ancrage aux
        overlays absolus (ViewportFrame, TelemetryHud).
      -->
      <v-main fill-height class="edition-main">
        <div class="edition-map-wrapper">
          <!-- La carte se monte immédiatement (chargement des tuiles) mais reste
               masquée tant que `editionViewReady` est false ; un spinner centré
               est affiché à la place. Les overlays et le bandeau de lecture ne
               sont montés qu'à la révélation. -->
          <EditionMap />
          <div v-if="!editionStore.editionViewReady" class="edition-loading">
            <div class="edition-loading-inner">
              <v-progress-circular :size="70" :width="7" color="primary" indeterminate />
              <span class="edition-loading-text">Chargement en cours…</span>
            </div>
          </div>
          <template v-if="editionStore.editionViewReady">
            <ViewportFrame />
            <TelemetryHud />
            <HeadingChangesPanel />
            <CameraEditor />
          </template>
        </div>
        <PlaybackControls v-if="editionStore.editionViewReady" />
      </v-main>

      <!-- Panneau Paramètres (comme sur l'Accueil) : catégories de la vue active
           uniquement — pas de sections système (prop `show-system`). -->
      <SettingsDrawer :show-system="false" />
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

/* Spinner centré pendant le chargement initial de la vue (écran vierge avant
   la révélation de la carte et des composants). */
.edition-loading {
  position: absolute;
  inset: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.25);
  pointer-events: none;
}

/* Colonne spinner + message. */
.edition-loading-inner {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.edition-loading-text {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.92);
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
  letter-spacing: 0.03em;
}
</style>
