<template>
  <div class="edition-camera fill-height d-flex flex-column">
    <!-- Barre d'outils : retour accueil + nom de la trace + statut pré-calcul -->
    <v-toolbar density="compact" flat border>
      <v-btn icon :to="{ name: 'accueil' }" title="Retour à l'accueil">
        <v-icon>mdi-home</v-icon>
      </v-btn>
      <v-divider vertical class="mx-2" />
      <v-icon class="mr-2">mdi-video-vintage</v-icon>
      <v-toolbar-title class="text-body-1 font-weight-bold">
        {{ traceName }}
      </v-toolbar-title>
      <v-spacer />

      <!-- Indicateur de statut du pré-calcul -->
      <v-chip
        v-if="precomputeStatusLabel"
        :color="precomputeStatusColor"
        size="small"
        variant="tonal"
        class="mr-2"
      >
        <v-icon start :icon="precomputeStatusIcon" />
        {{ precomputeStatusLabel }}
      </v-chip>

      <!-- Bouton d'ouverture du panneau de paramètres dédié -->
      <EditionSettingsPanel v-model="settingsOpen" />
    </v-toolbar>

    <!-- Corps : map 3D + overlay de progression + panneaux d'édition.
         Disposition : map (flex-grow) | panneau latéral droit (liste/édition),
         avec une bande basse contenant le profile d'altitude et la timeline. -->
    <div class="flex-grow-1 d-flex flex-column position-relative">
      <div class="flex-grow-1 d-flex min-h-0">
        <!-- Map 3D (pré-calcul + live preview) -->
        <div class="flex-grow-1 position-relative">
          <Map3D ref="map3dRef" @map-ready="onMapReady" />
          <PrecomputeOverlay />
        </div>

        <!-- Panneau latéral droit : liste + édition d'overrides -->
        <div class="side-panel d-flex flex-column" v-if="keyframesStore.hasRawKeyframes">
          <OverrideList
            :selected-id="selectedOverrideId"
            @create="onCreateOverride"
            @select="onSelectOverride"
            @edit="onEditOverride"
          />
          <OverridePanel
            v-if="overridePanelOpen"
            :override="editingOverride"
            @cancel="onCloseOverridePanel"
            @applied="onOverrideApplied"
          />
        </div>
      </div>

      <!-- Bande basse : profile d'altitude + timeline -->
      <div v-if="keyframesStore.hasRawKeyframes" class="bottom-band">
        <AltitudeProfile :final-keyframes="finalKeyframes" />
        <Timeline />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Atelier d'édition / montage (Mode 2).
 *
 * Accessible via le bouton « Éditer » d'un circuit (Circuit.vue) avec
 * `router.push({ name: 'editionCamera', params: { traceId } })`.
 *
 * Au chargement :
 *  - retrouve la trace (pour son nom) via le store `traces` ;
 *  - charge l'état d'édition via le store `keyframes` (cache + overrides) ;
 *  - si aucun cache de keyframes n'existe, déclenche automatiquement le
 *    pré-calcul (Mode 1, Variante A') dans la map 3D dès que le terrain DEM
 *    est prêt.
 *
 * M4 y ajoutera la timeline, le profile d'altitude et le panneau d'overrides.
 * M5 y ajoutera la lecture (boucle requestAnimationFrame + marqueur traceur).
 */
import { ref, shallowRef, computed, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import type { Map as MbMap } from 'mapbox-gl'
import { useTracesStore } from '../stores/traces'
import { useKeyframesStore, type PrecomputeStatus } from '../stores/keyframes'
import { useSettingsStore } from '../stores/settings'
import { useUiStore } from '../stores/ui'
import Map3D from '../components/EditionCamera/Map3D.vue'
import PrecomputeOverlay from '../components/EditionCamera/PrecomputeOverlay.vue'
import EditionSettingsPanel from '../components/EditionCamera/EditionSettingsPanel.vue'
import Timeline from '../components/EditionCamera/Timeline.vue'
import AltitudeProfile from '../components/EditionCamera/AltitudeProfile.vue'
import OverrideList from '../components/EditionCamera/OverrideList.vue'
import OverridePanel from '../components/EditionCamera/OverridePanel.vue'
import { precomputeKeyframes, loadPrecomputeOptions } from '../composables/useKeyframeEngine'
import { useLivePreview } from '../composables/useLivePreview'
import { usePlayback } from '../composables/usePlayback'
import type { Override } from '../utils/keyframes'

const route = useRoute()
const tracesStore = useTracesStore()
const keyframesStore = useKeyframesStore()
const settingsStore = useSettingsStore()
const uiStore = useUiStore()

/** Réf vers le composant Map3D (pour appeler getMap / waitForTerrain). */
const map3dRef = ref<InstanceType<typeof Map3D> | null>(null)

/**
 * Réf réactive vers l'instance Mapbox. Alimentée par l'événement `map-ready`
 * de Map3D, consommée par le composable `useLivePreview`. `shallowRef` car
 * Mapbox gère son propre état interne (pas de réactivité profonde voulue).
 */
const mapInstance = shallowRef<MbMap | null>(null)

// Branche le live preview (refusion automatique + jumpTo sur changement de
// curseur/overrides). La map peut être null au début ; le composable est sans
// effet tant qu'elle n'est pas disponible.
// `finalKeyframes` alimente le profile d'altitude (reflète les overrides).
const { finalKeyframes } = useLivePreview(mapInstance)

// Branche la lecture (boucle RAF + déplacement du marqueur traceur + trace
// parcourue). Les callbacks délèguent au composant Map3D exposé.
usePlayback({
  onMarkerMove: (lng, lat, altitude) => {
    map3dRef.value?.moveTraceurMarker(lng, lat, altitude)
  },
  onTrailUpdate: (coords) => {
    map3dRef.value?.updateTrail(coords)
  },
})

/** Handler de l'événement `map-ready` de Map3D : alimente la réf réactive. */
function onMapReady() {
  mapInstance.value = map3dRef.value?.getMap() ?? null
}

/** État d'ouverture du panneau de paramètres. */
const settingsOpen = ref(false)

// --- État UI du panneau d'édition d'override ---

/** true si le panneau OverridePanel est ouvert (édition ou création). */
const overridePanelOpen = ref(false)
/** Override en cours d'édition (null = création d'un nouvel override). */
const editingOverride = ref<Override | null>(null)
/** id de l'override sélectionné dans la liste (surlignage), ou null. */
const selectedOverrideId = ref<string | null>(null)

/** Ouvre le panneau pour créer un nouvel override. */
function onCreateOverride() {
  editingOverride.value = null
  overridePanelOpen.value = true
}

/** Ouvre le panneau pour éditer un override existant et place le curseur. */
function onEditOverride(ov: Override) {
  editingOverride.value = ov
  selectedOverrideId.value = ov.id
  // Placer le curseur au milieu de la plage pour un aperçu immédiat.
  keyframesStore.seek((ov.start_time + ov.end_time) / 2)
  overridePanelOpen.value = true
}

/** Sélectionne un override dans la liste (surlignage) sans l'éditer. */
function onSelectOverride(ov: Override) {
  selectedOverrideId.value = ov.id
}

/** Ferme le panneau d'édition (annulation ou après application). */
function onCloseOverridePanel() {
  overridePanelOpen.value = false
  editingOverride.value = null
}

/** Après application d'un override : fermer le panneau et le sélectionner. */
function onOverrideApplied(ov: Override) {
  selectedOverrideId.value = ov.id
  overridePanelOpen.value = false
  editingOverride.value = null
}

/** Identifiant de la trace à éditer, lu depuis l'URL. */
const traceId = computed(() => String(route.params.traceId ?? ''))

/** Métadonnées de la trace courante (pour le titre), ou null. */
const trace = computed(() =>
  tracesStore.traces.find(t => t.id === traceId.value) ?? null,
)

const traceName = computed(() => trace.value?.name ?? 'Édition')

// --- Libellés de statut du pré-calcul (toolbar) ---

const precomputeStatusLabel = computed<string>(() => {
  switch (keyframesStore.precomputeStatus) {
    case 'needed': return 'À calculer'
    case 'running': return 'Calcul…'
    case 'done': return 'Prêt'
    case 'error': return 'Erreur'
    default: return ''
  }
})

const precomputeStatusColor = computed(() => {
  const map: Record<PrecomputeStatus, string> = {
    idle: 'grey',
    needed: 'warning',
    running: 'info',
    done: 'success',
    error: 'error',
  }
  return map[keyframesStore.precomputeStatus] ?? 'grey'
})

const precomputeStatusIcon = computed(() => {
  switch (keyframesStore.precomputeStatus) {
    case 'needed': return 'mdi-alert-circle-outline'
    case 'running': return 'mdi-progress-clock'
    case 'done': return 'mdi-check-circle-outline'
    case 'error': return 'mdi-alert-octagon-outline'
    default: return 'mdi-circle-outline'
  }
})

// --- Pré-calcul automatique (Mode 1, Variante A') ---

/**
 * Affiche la LineString de la trace GPX sur la map et la cadre.
 *
 * Récupère la géométrie depuis le store `traces` et la couleur du marqueur
 * depuis les paramètres (`EditionCamera.Traceur.markerColor`), puis appelle
 * `Map3D.displayTrace`. Appelé dans les deux chemins : après un pré-calcul
 * et au chargement depuis le cache.
 *
 * Sans effet si la map 3D n'est pas encore prête (le caller doit avoir attendu
 * `waitForTerrain` au préalable pour que le style soit chargé).
 */
async function showTraceOnMap(): Promise<void> {
  if (!map3dRef.value) {
    console.warn('[showTraceOnMap] Map3D non monté.')
    return
  }
  let color = '#FF0000FF'
  try {
    color = await settingsStore.getSettingValue('EditionCamera.Traceur.markerColor') ?? color
  } catch (e) {
    console.warn('[showTraceOnMap] Couleur markerColor illisible, repli rouge :', e)
  }
  try {
    const feature = await tracesStore.getTraceGeometry(traceId.value)
    const coords = (feature.geometry as GeoJSON.LineString).coordinates
    console.info(`[showTraceOnMap] Géométrie récupérée : ${coords.length} points.`)
    map3dRef.value.displayTrace(feature, color)
  } catch (e) {
    console.error('[showTraceOnMap] Impossible de récupérer/afficher la géométrie :', e)
  }
}

/**
 * Déclenche le pré-calcul des keyframes si aucun cache n'existe.
 *
 * Attend que :
 *  - la map 3D soit montée et expose son instance Mapbox ;
 *  - le terrain DEM soit chargé (queryTerrainElevation fiable).
 * Puis lance `precomputeKeyframes` avec les options lues depuis les settings,
 * persiste le résultat via le store et restaure la taille réelle de la map.
 *
 * En cas d'erreur, le statut passe à 'error' et une snackbar l'explique ; le
 * bouton « Éditer » reste cliquable pour un éventuel rechargement de la vue.
 */
async function runPrecomputeIfNeeded(): Promise<void> {
  if (keyframesStore.precomputeStatus !== 'needed') return

  // Attendre que la map soit disponible.
  while (!map3dRef.value?.getMap()) {
    await new Promise((r) => setTimeout(r, 50))
  }
  const map = map3dRef.value!.getMap()!

  keyframesStore.precomputeStatus = 'running'
  keyframesStore.precomputeProgress = 0

  try {
    // Attendre que le terrain DEM soit chargé (queryTerrainElevation fiable).
    await map3dRef.value!.waitForTerrain()

    // Récupérer la géométrie de la trace (LineString [lon, lat][]).
    const feature = await tracesStore.getTraceGeometry(traceId.value)
    const coords = (feature.geometry as GeoJSON.LineString).coordinates as [number, number][]
    if (!coords || coords.length < 2) {
      throw new Error('La trace contient moins de 2 points.')
    }

    // Lire les options depuis les paramètres backend.
    const opts = await loadPrecomputeOptions((p) => settingsStore.getSettingValue(p))

    // Lancer le pré-calcul (zone morte + altitudes).
    const raw = await precomputeKeyframes(map, coords, traceId.value, opts, {
      onPhase: (_phase, label) => {
        keyframesStore.precomputeStep = label
      },
      onProgress: (fraction) => {
        keyframesStore.precomputeProgress = fraction
      },
    })

    // Persister le résultat.
    await keyframesStore.saveRawKeyframes(raw)

    // Restaurer la taille réelle de la map (le pré-calcul l'a forcée au
    // viewport canonique 1920×1080) pour un live preview correct.
    map.getContainer().style.width = ''
    map.getContainer().style.height = ''
    map.resize()

    // Afficher la trace GPX en fond et la cadrer.
    await showTraceOnMap()

    // Initialiser les couches de lecture (marqueur traceur + trace parcourue).
    await map3dRef.value!.setupPlaybackLayers()

    // Amener le curseur au départ pour un aperçu immédiat.
    keyframesStore.seek(0)

    uiStore.showSuccess('Pré-calcul terminé. L’édition est prête.')
  } catch (e) {
    console.error('Pré-calcul des keyframes échoué :', e)
    keyframesStore.precomputeStatus = 'error'
    keyframesStore.precomputeStep = ''
    uiStore.showError(
      `Échec du pré-calcul : ${e instanceof Error ? e.message : String(e)}`,
    )
  }
}

onMounted(async () => {
  // S'assurer que la liste des traces est chargée (pour retrouver le nom).
  if (tracesStore.traces.length === 0) {
    await tracesStore.loadTraces()
  }
  if (!traceId.value) return

  await keyframesStore.loadForTrace(traceId.value)

  // Si la trace a déjà un cache de keyframes, attendre que la map 3D soit
  // prête puis initialiser les couches de lecture (marqueur + trace).
  if (keyframesStore.hasRawKeyframes) {
    void setupPlaybackWhenReady()
  } else {
    // Si la trace n'a pas encore de keyframes, lancer le pré-calcul.
    // Léger différé pour laisser la map 3D s'initialiser.
    void runPrecomputeIfNeeded()
  }
})

/**
 * Attend que la map 3D est prête puis initialise les couches de lecture.
 * Utilisé quand un cache de keyframes existait déjà au chargement (pas de
 * pré-calcul à refaire, mais le marqueur traceur et la trace doivent être
 * créés pour la lecture).
 */
async function setupPlaybackWhenReady(): Promise<void> {
  while (!map3dRef.value?.getMap()) {
    await new Promise((r) => setTimeout(r, 50))
  }
  // Attendre que le style/terrain soit chargé (les couches de trace dépendent
  // du style Mapbox). L'événement 'terrain-ready' n'est pas strictement requis
  // pour la trace 2D, mais on attend au moins le style.
  await map3dRef.value!.waitForTerrain()
  // Afficher la trace GPX en fond et la cadrer (avant les couches de lecture).
  await showTraceOnMap()
  await map3dRef.value!.setupPlaybackLayers()
  keyframesStore.seek(0)
}

onUnmounted(() => {
  keyframesStore.reset()
})
</script>

<style scoped>
.edition-camera {
  /* Remplit la zone d'affichage fournie par App.vue (v-main). */
  width: 100%;
  height: 100%;
}

.position-relative {
  position: relative;
}

/* Panneau latéral droit : largeur fixe, fond surface. */
.side-panel {
  width: 380px;
  flex-shrink: 0;
  background: rgb(var(--v-theme-surface));
  border-left: thin solid rgba(var(--v-theme-on-surface), 0.12);
  overflow-y: auto;
}

/* Bande basse : profile + timeline, sous la map. */
.bottom-band {
  background: rgb(var(--v-theme-surface));
  border-top: thin solid rgba(var(--v-theme-on-surface), 0.12);
  flex-shrink: 0;
}

/* min-h-0 : nécessaire pour que les enfants flex puissent défiler. */
.min-h-0 {
  min-height: 0;
}
</style>
