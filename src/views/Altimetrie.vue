<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <AltimetrieToolbar
        :trace-name="traceName"
        :dirty="altimetrieStore.dirty"
        :saving="altimetrieStore.saving"
        @back="onBackClicked"
        @open-settings="toggleSettings"
      />

      <AltimetrieClimbsPanel
        :climbs="altimetrieStore.climbs"
        :profile="profile"
        :alt-min="altBounds.min"
        :alt-max="altBounds.max"
        :total-distance-m="totalDistanceM"
        :selected-id="altimetrieStore.selectedClimbId"
        @select="altimetrieStore.selectClimb"
      />

      <v-main class="altimetrie-main">
        <div class="altimetrie-center">
          <v-progress-circular
            v-if="isLoading"
            :size="60"
            :width="7"
            color="primary"
            indeterminate
          />

          <v-empty-state
            v-else-if="errorMessage"
            icon="mdi-alert-circle-outline"
            title="Analyse indisponible"
            :text="errorMessage"
          />

          <div v-else class="altimetrie-placeholder">
            <v-icon icon="mdi-map" size="64" />
            <p class="mt-3">La carte 3D sera intégrée à l'étape suivante.</p>
          </div>
        </div>
      </v-main>

      <!-- Panneau Paramètres (groupes Altimetrie.* de la vue active) -->
      <SettingsDrawer :show-system="false" />

      <ConfirmExitDialog
        v-model="dialogExitOpen"
        @confirm="onExitConfirmed"
        @cancel="dialogExitOpen = false"
      />
    </v-layout>
  </v-app>
</template>

<script setup lang="ts">
/**
 * Vue Altimétrie d'une trace propre (route `/altimetrie?traceId=…`).
 *
 * Elle charge l'analyse `altimetrie.json` — ou la génère si elle manque — et
 * présente les montées détectées dans un panneau latéral. La carte 3D et le
 * widget de profil seront branchés aux sous-étapes suivantes ; cette première
 * version pose le squelette de vue, le store et l'entrée depuis le circuit.
 *
 * La trace est désignée par la **query** de la route, comme Audit et Multiride.
 * À la sortie, le store est réinitialisé ; si des montées ont été éditées sans
 * être sauvegardées, un dialogue demande confirmation.
 */
import { computed, onMounted, ref } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useAltimetrieStore, type AltimetrieParams } from '../stores/altimetrie'
import { useSettingsStore } from '../stores/settings'
import { useTracesStore } from '../stores/traces'
import { useUiStore } from '../stores/ui'
import AltimetrieToolbar from '../components/Altimetrie/AltimetrieToolbar.vue'
import AltimetrieClimbsPanel from '../components/Altimetrie/AltimetrieClimbsPanel.vue'
import ConfirmExitDialog from '../components/Altimetrie/dialogs/ConfirmExitDialog.vue'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'

const route = useRoute()
const router = useRouter()
const appStore = useAppStore()
const altimetrieStore = useAltimetrieStore()
const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const ui = useUiStore()

/** Chargement initial (réglages, traces, analyse). */
const initializing = ref(true)
/** Message d'erreur de détection, affiché au centre. */
const errorMessage = ref<string | null>(null)
/** Confirmation de sortie avec des modifications non enregistrées. */
const dialogExitOpen = ref(false)

const traceId = computed(() => (route.query.traceId as string | null) ?? null)
const traceName = computed(
  () => tracesStore.traces.find(trace => trace.id === traceId.value)?.name ?? 'Trace',
)
const profile = computed(() => altimetrieStore.file?.profile ?? [])
const isLoading = computed(() => initializing.value || altimetrieStore.loading)

const totalDistanceM = computed(() => {
  const points = profile.value
  if (points.length === 0) return 0
  return points[points.length - 1].d
})

const altBounds = computed(() => {
  const points = profile.value
  if (points.length === 0) return { min: 0, max: 0 }

  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const point of points) {
    if (point.ele < min) min = point.ele
    if (point.ele > max) max = point.ele
  }
  return { min, max }
})

/** Valeur d'un paramètre numérique, avec repli sur la valeur par défaut. */
function settingNumber(path: string, fallback: number): number {
  const definition = settingsStore.settings.find(setting => setting.path === path)
  const value = definition?.value
  return typeof value === 'number' ? value : fallback
}

/** Valeur d'un paramètre booléen, avec repli sur la valeur par défaut. */
function settingBool(path: string, fallback: boolean): boolean {
  const definition = settingsStore.settings.find(setting => setting.path === path)
  const value = definition?.value
  return typeof value === 'boolean' ? value : fallback
}

/** Assemble les paramètres du pipeline depuis les réglages `Altimetrie.*`. */
function buildParams(): AltimetrieParams {
  return {
    pasEchantillonnage: settingNumber('Altimetrie.Preparation.pasEchantillonnage', 50),
    correctionPlateaux: settingNumber('Altimetrie.Preparation.correctionPlateaux', 6),
    fenetreAntiPic: settingNumber('Altimetrie.Preparation.fenetreAntiPic', 200),
    seuilAntiPic: settingNumber('Altimetrie.Preparation.seuilAntiPic', 15),
    fenetreAltitude: settingNumber('Altimetrie.Lissage.fenetreAltitude', 500),
    fenetrePente: settingNumber('Altimetrie.Lissage.fenetrePente', 150),
    sigmaGaussien: settingNumber('Altimetrie.Lissage.sigmaGaussien', 75),
    fenetreMediane: settingNumber('Altimetrie.Lissage.fenetreMediane', 0),
    lissageRenforce: settingBool('Altimetrie.Lissage.lissageRenforce', true),
    seuilMontee: settingNumber('Altimetrie.Detection.seuilMontee', 0.5),
    ecartFusion: settingNumber('Altimetrie.Detection.ecartFusion', 250),
    rechercheExtrema: settingNumber('Altimetrie.Detection.rechercheExtrema', 60),
    fenetreCalcul: settingNumber('Altimetrie.Segments.fenetreCalcul', 100),
    longueurMin: settingNumber('Altimetrie.Segments.longueurMin', 200),
  }
}

function toggleSettings(): void {
  appStore.isSettingsDrawerOpen = !appStore.isSettingsDrawerOpen
}

function onBackClicked(): void {
  router.push({ name: 'accueil' })
}

function onExitConfirmed(): void {
  dialogExitOpen.value = false
  altimetrieStore.reset()
  router.push({ name: 'accueil' })
}

onMounted(async () => {
  if (!traceId.value) {
    ui.showError('Aucune trace à analyser.')
    router.replace({ name: 'accueil' })
    initializing.value = false
    return
  }

  try {
    await settingsStore.loadSettings()
    await tracesStore.loadTraces()

    const trace = tracesStore.traces.find(entry => entry.id === traceId.value)
    if (!trace) {
      ui.showError('Trace introuvable.')
      router.replace({ name: 'accueil' })
      return
    }
    if (trace.audit_status !== 'clean') {
      ui.showWarning(
        "L'altimétrie n'est disponible qu'après nettoyage de la trace.",
      )
      router.replace({ name: 'accueil' })
      return
    }

    await altimetrieStore.loadOrDetect(traceId.value, buildParams())
  } catch (error) {
    const message =
      typeof error === 'string' ? error : 'Analyse altimétrique impossible.'
    errorMessage.value = message
    ui.showError(message)
  } finally {
    initializing.value = false
  }
})

onBeforeRouteLeave(() => {
  if (!altimetrieStore.dirty) {
    altimetrieStore.reset()
    return true
  }
  if (!dialogExitOpen.value) {
    dialogExitOpen.value = true
    return false
  }
  altimetrieStore.reset()
  return true
})
</script>

<style scoped>
.altimetrie-main {
  height: 100vh;
}

.altimetrie-center {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.altimetrie-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  color: rgba(var(--v-theme-on-surface), 0.55);
  font-size: 0.9rem;
}
</style>
