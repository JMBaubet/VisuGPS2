<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <MultirideToolbar
        :trace-name="traceName"
        :status="multirideStore.status"
        :segment-count="multirideStore.segmentCount"
        :loading="multirideStore.loading || analyzing"
        :validated-at="archive?.updatedAt ?? null"
        @back="onBackClicked"
        @analyze="onAnalyzeClicked"
        @open-settings="appStore.isSettingsDrawerOpen = !appStore.isSettingsDrawerOpen"
      />

      <v-main class="multiride-main">
        <div class="multiride-center">
          <v-progress-circular
            v-if="!archive"
            :size="60"
            :width="7"
            color="primary"
            indeterminate
          />

          <v-card v-else class="multiride-summary" max-width="620">
            <v-card-title class="d-flex align-center">
              <v-icon
                :icon="multirideStore.hasPassages ? 'mdi-repeat' : 'mdi-check-circle-outline'"
                :color="multirideStore.hasPassages ? 'warning' : 'success'"
                class="mr-2"
              />
              {{
                multirideStore.hasPassages
                  ? 'Portions répétées détectées'
                  : 'Aucun passage multiple détecté'
              }}
            </v-card-title>

            <v-card-text>
              <p v-if="multirideStore.hasPassages" class="mb-4">
                {{ multirideStore.passages.length }} passage(s) sur
                {{ multirideStore.segmentCount }} segment(s), pour
                {{ formatDistance(multirideStore.repeatedKm * 1000) }} de trace
                parcourue plusieurs fois.
              </p>
              <p v-else class="mb-4">
                La trace ne repasse sur aucun tronçon : rien à valider, l'édition
                caméra est accessible.
              </p>

              <v-list density="compact" class="multiride-facts">
                <v-list-item title="Source" :subtitle="archive.source" />
                <v-list-item
                  title="Trace d'origine"
                  :subtitle="`${formatDistance(archive.traceLengthKm * 1000)} · ${archive.tracePointCount} points`"
                />
                <v-list-item
                  v-if="multirideStore.analysisDurationMs > 0"
                  title="Temps d'analyse"
                  :subtitle="`${multirideStore.analysisDurationMs} ms`"
                />
                <v-list-item title="Paramètres actifs" :subtitle="paramsSummary" />
              </v-list>

              <!-- Un pas d'échantillonnage relevé automatiquement change la
                   finesse de la détection : cela se sait. -->
              <v-alert
                v-if="archive.pasPlafonne"
                type="info"
                variant="tonal"
                density="compact"
                class="mt-4"
              >
                Le pas d'échantillonnage a été relevé automatiquement pour contenir
                le nombre de points analysés.
              </v-alert>
            </v-card-text>
          </v-card>
        </div>
      </v-main>

      <!-- Panneau Paramètres (groupes Multiride.* de la vue active) -->
      <SettingsDrawer :show-system="false" />
    </v-layout>
  </v-app>
</template>

<script setup lang="ts">
/**
 * Vue de détection des passages multiples (route `/multiride`, plein écran
 * style Accueil/Audit).
 *
 * Une trace valide peut contenir des portions parcourues plusieurs fois :
 * aller-retour sur un tronçon, reconnaissance repassant sur une section, boucle
 * locale. Cette vue les détecte — ou restitue une détection déjà écrite — et
 * les fait valider : **tant qu'un passage reste à valider, l'édition caméra est
 * inaccessible** (`multiride_status = "pending"`), au même titre qu'une trace
 * non auditée. La détection suit donc l'audit et précède l'édition caméra.
 *
 * Le `traceId` arrive par la query de la route (`/multiride?traceId=…`), posée
 * par le bouton Éditer de l'accueil ou par le garde-fou d'EditionCamera. À la
 * sortie, le store est réinitialisé — le fichier de description, lui, survit.
 *
 * La carte de restitution et le panneau détaillé des segments sont portés par
 * les sous-étapes suivantes de l'évolution ; cette vue expose d'ores et déjà
 * l'état, la synthèse et la relance de la détection.
 */
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter, onBeforeRouteLeave } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useMultirideStore, type MultirideParams } from '../stores/multiride'
import { useSettingsStore } from '../stores/settings'
import { useTracesStore } from '../stores/traces'
import { useUiStore } from '../stores/ui'
import MultirideToolbar from '../components/Multiride/MultirideToolbar.vue'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'
import { formatDistance } from '../utils/format'

const route = useRoute()
const router = useRouter()
const appStore = useAppStore()
const multirideStore = useMultirideStore()
const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const ui = useUiStore()

/** Une détection est en cours de lancement depuis cette vue. */
const analyzing = ref(false)

const traceId = computed(() => (route.query.traceId as string | null) ?? null)
const traceName = computed(
  () => tracesStore.traces.find((t) => t.id === traceId.value)?.name ?? 'Trace',
)

/** L'état de la détection courante (`null` avant la première restitution). */
const archive = computed(() => multirideStore.archive)

/** Paramètres actifs, tels qu'ils ont produit la détection affichée. */
const paramsSummary = computed(() => {
  const params = archive.value?.params
  if (!params) return ''
  return [
    `tolérance ${params.toleranceM} m`,
    `longueur min ${params.longueurMinM} m`,
    `pas ${params.pasEchantillonnageM} m`,
    `fusion ${params.fusionReferencesM} m`,
  ].join(' · ')
})

/** Valeur d'un paramètre numérique, avec repli sur la valeur par défaut. */
function settingNumber(path: string, fallback: number): number {
  const def = settingsStore.settings.find((s) => s.path === path)
  const value = def?.value
  return typeof value === 'number' ? value : fallback
}

/** Assemble les paramètres du détecteur depuis les réglages `Multiride.*`. */
function buildParams(): MultirideParams {
  return {
    toleranceM: settingNumber('Multiride.Detection.tolerance', 10),
    longueurMinM: settingNumber('Multiride.Detection.longueurMin', 100),
    pasEchantillonnageM: settingNumber('Multiride.Detection.pasEchantillonnage', 4),
    fusionReferencesM: settingNumber('Multiride.Detection.fusionReferences', 100),
  }
}

/** Lance la détection sur la trace courante. */
async function runDetection(): Promise<void> {
  if (!traceId.value) return
  await multirideStore.runDetection(traceId.value, buildParams())
}

/** Relance la détection à la demande (paramètres `Multiride.*` courants). */
async function onAnalyzeClicked(): Promise<void> {
  analyzing.value = true
  try {
    await runDetection()
    ui.showSuccess('Détection des passages multiples terminée.')
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Échec de la détection.'
    ui.showError(msg)
  } finally {
    analyzing.value = false
  }
}

function onBackClicked() {
  router.push({ name: 'accueil' })
}

onMounted(async () => {
  // Garde-fou : sans trace en query, la vue n'a rien à analyser.
  if (!traceId.value) {
    ui.showError('Aucune trace à analyser.')
    router.replace({ name: 'accueil' })
    return
  }

  try {
    await settingsStore.loadSettings()
    await tracesStore.loadTraces()
    // La détection déjà écrite fait foi — ajustements compris ; elle n'est
    // rejouée que si le fichier est absent ou inexploitable.
    const restored = await multirideStore.restore(traceId.value)
    if (!restored) await runDetection()
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Erreur de détection.'
    ui.showError(msg)
    router.replace({ name: 'accueil' })
  }
})

onBeforeRouteLeave(() => {
  // Le fichier de description porte l'état : quitter la vue ne perd rien, une
  // reprise restitue la détection et ses ajustements.
  multirideStore.reset()
  return true
})
</script>

<style scoped>
.multiride-center {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.multiride-summary {
  width: 100%;
}

.multiride-facts {
  background: transparent;
}
</style>
