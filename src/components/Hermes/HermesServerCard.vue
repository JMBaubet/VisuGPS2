<template>
  <v-card class="hermes-server-card" variant="tonal">
    <v-card-title class="d-flex align-center text-subtitle-1 py-2">
      <v-icon
        :icon="hermesStore.statusDisplay.icon"
        :color="hermesStore.statusDisplay.color"
        size="18"
        class="mr-2"
      />
      <div class="d-flex flex-column">
        <span>Serveur hermes</span>
        <span
          v-if="serverCaption"
          class="text-caption text-medium-emphasis"
        >
          {{ serverCaption }}
        </span>
      </div>
      <v-spacer />
      <v-chip
        :color="serviceDisplay.color"
        variant="tonal"
        size="small"
        title="État du service"
      >
        {{ serviceDisplay.label }}
      </v-chip>
    </v-card-title>

    <v-card-text class="pt-1 pb-3">
      <template v-if="hermesStore.activeDataset">
        <v-list density="compact" nav class="hermes-served">
          <HermesDatasetRow
            :dataset="hermesStore.activeDataset"
            active
            :interactive="false"
            :show-delete="false"
            @update-dataset="emit('update-dataset', $event)"
          />
        </v-list>
      </template>
      <template v-else>
        <div class="hermes-line">
          <span class="hermes-line__label">Jeu de données</span>
          <span
            class="hermes-line__value"
            :class="{ 'text-medium-emphasis': !servedName }"
            :title="servedName ?? undefined"
          >
            {{ servedName ?? 'Aucun' }}
          </span>
        </div>
        <div
          v-if="!hermesStore.activeDatasetId"
          class="text-caption text-medium-emphasis mt-1"
        >
          Choisissez un jeu de données dans la liste pour le servir.
        </div>
      </template>

      <div class="d-flex flex-wrap ga-2 mt-3">
        <v-btn
          v-if="showWake"
          size="small"
          color="primary"
          variant="tonal"
          prepend-icon="mdi-power"
          :loading="hermesStore.server === 'waking'"
          :disabled="hermesStore.server === 'waking'"
          @click="wake"
        >
          {{ wakeLabel }}
        </v-btn>
        <template v-else>
          <v-btn
            v-if="showStop"
            size="small"
            color="error"
            variant="tonal"
            prepend-icon="mdi-stop-circle-outline"
            :loading="hermesStore.servingBusy"
            :disabled="actionDisabled"
            @click="onStop"
          >
            Arrêter le service
          </v-btn>
          <v-btn
            v-else
            size="small"
            color="success"
            variant="tonal"
            prepend-icon="mdi-play-circle-outline"
            :loading="hermesStore.servingBusy"
            :disabled="
              actionDisabled ||
              !hermesStore.activeDatasetId ||
              !serviceReachable
            "
            @click="onStart"
          >
            Démarrer le service
          </v-btn>
          <span
            v-if="actionDisabled && hermesStore.constructionInProgress"
            class="text-caption text-blue align-self-center"
          >
            Construction en cours…
          </span>
        </template>
      </div>

      <v-alert
        v-if="showError"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-2"
        :text="hermesStore.lastError ?? undefined"
      />

      <div v-if="hermesStore.wakeProgress" class="mt-2 text-body-2">
        Réveil en cours — tentative
        {{ hermesStore.wakeProgress.attempts }},
        {{ hermesStore.wakeProgress.elapsedSec }} s.
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
/**
 * Carte « Serveur » du panneau Hermes.
 *
 * Trois niveaux distincts :
 * - **serveur** (machine + réseau Tailscale, ping système) : caption sous le
 *   titre uniquement dans les cas dégradés (injoignable, réveil en cours,
 *   perdu) ;
 * - **service** (API hermes + instance ORS) : chip en en-tête (Actif, Attente
 *   fin de construction, Démarrage en cours, Arrêté, Aucun service, Échec,
 *   Service arrêté, Inconnu) ; « Actif » n'apparaît qu'une fois la
 *   construction (build + aux) terminée ;
 * - **jeu de données servi** : même ligne que l'onglet « Jeu de données »
 *   (sans poubelle, coche colorée comme le chip d'état).
 *
 * Un ping en échec propose « Réveiller » ; après un réveil resté sans réponse,
 * le serveur est présumé perdu et le bouton devient « Réessayer le réveil ».
 * La sélection se fait depuis la liste ; « Arrêter le service » et « Démarrer
 * le service » agissent sur l'instance sans perdre la sélection
 * (`POST /instances/{id}/stop|start`).
 */
import { computed } from 'vue'
import { useHermesStore, type HermesDataset } from '../../stores/hermes'
import { useUiStore } from '../../stores/ui'
import { datasetDisplayName, parseHermesError } from '../../utils/hermes'
import HermesDatasetRow from './HermesDatasetRow.vue'

const emit = defineEmits<{
  'update-dataset': [dataset: HermesDataset]
}>()

const hermesStore = useHermesStore()
const ui = useUiStore()

const serving = computed(() => hermesStore.serving)

/** Nom du jeu de données sélectionné (id en repli si le manifest est absent). */
const servedName = computed<string | null>(() => {
  const dataset = hermesStore.activeDataset
  if (dataset) return datasetDisplayName(dataset)
  return hermesStore.activeDatasetId
})

interface ServiceDisplay {
  label: string
  color: string
}

/**
 * Texte d'état du serveur, uniquement dans les cas dégradés : la situation
 * normale (« en ligne ») n'a pas besoin d'être répétée.
 */
const serverCaption = computed<string | null>(() => {
  switch (hermesStore.server) {
    case 'offline':
      return 'injoignable'
    case 'waking':
      return 'réveil en cours'
    case 'lost':
      return 'serveur perdu'
    default:
      return null
  }
})

/**
 * État du **service**, distinct de l'état du **serveur** :
 * - serveur muet : « Inconnu » (le service n'est pas interrogeable) ;
 * - serveur joignable mais API muette : « Service arrêté » (orange) ;
 * - API joignable : état du dataset servi (ORS).
 * « Aucun service » (rouge) est réservé au serveur joignable sans dataset servi.
 */
const serviceDisplay = computed<ServiceDisplay>(() => {
  if (hermesStore.server !== 'online') {
    return { label: 'Inconnu', color: 'grey' }
  }
  if (hermesStore.apiOnline === false) {
    return { label: 'Service arrêté', color: 'orange' }
  }
  if (hermesStore.apiOnline !== true) {
    return { label: 'Inconnu', color: 'grey' }
  }
  const state = serving.value?.state
  // « Attente fin de construction » prime sur les états transitoires : pendant
  // une construction (build ou aux), le serveur peut repasser brièvement en
  // `starting` ; on ne veut pas voir clignoter « Démarrage en cours ».
  if (
    hermesStore.constructionInProgress &&
    (state === 'active' || state === 'waiting' || state === 'starting')
  ) {
    return { label: 'Attente fin de construction', color: 'blue' }
  }
  switch (state) {
    case 'active':
      return { label: 'Actif', color: 'green' }
    case 'waiting':
      return { label: 'Attente fin de construction', color: 'blue' }
    case 'starting':
      return { label: 'Démarrage en cours', color: 'blue' }
    case 'stopped':
      return { label: 'Arrêté', color: 'orange' }
    case 'failed':
      return { label: 'Échec', color: 'red' }
    default:
      return { label: 'Aucun service', color: 'red' }
  }
})

/** États pendant lesquels ORS est engagé : l'action proposée est « Arrêter le service ». */
const RUNNING_STATES = new Set(['active', 'waiting', 'starting'])

const serviceReachable = computed(() => hermesStore.apiOnline === true)

const showStop = computed(
  () =>
    serviceReachable.value &&
    !!hermesStore.activeDatasetId &&
    RUNNING_STATES.has(serving.value?.state ?? ''),
)

/** Le ping a échoué : le réveil remplace Arrêter/Démarrer le service. */
const showWake = computed(
  () =>
    hermesStore.server === 'offline' ||
    hermesStore.server === 'waking' ||
    hermesStore.server === 'lost',
)

/** Après un réveil échoué, le bouton indique qu'il s'agit d'une nouvelle tentative. */
const wakeLabel = computed(() =>
  hermesStore.server === 'lost' ? 'Réessayer le réveil' : 'Réveiller',
)

const actionDisabled = computed(
  () => hermesStore.servingBusy || hermesStore.constructionInProgress,
)

/**
 * La perte du **service** est signalée par une popup, pas par un message dans
 * la carte : l'alerte est masquée tant que l'API est muette. Le message
 * « Perte du serveur » (`lost`) et les autres erreurs d'un serveur en ligne
 * restent visibles.
 */
const showError = computed(() => {
  if (!hermesStore.lastError) return false
  if (hermesStore.server === 'lost') return true
  if (hermesStore.server !== 'online') return false
  return hermesStore.apiOnline !== false
})

async function onStop(): Promise<void> {
  try {
    await hermesStore.stopServedDataset()
    ui.showSuccess('Instance ORS arrêtée, jeu de données conservé.')
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  }
}

async function onStart(): Promise<void> {
  try {
    await hermesStore.startServedDataset()
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  }
}

async function wake(): Promise<void> {
  const ok = await hermesStore.wake()
  if (ok) {
    ui.showSuccess('Serveur hermes réveillé.')
  } else {
    ui.showWarning("Le serveur hermes n'a pas répondu dans le délai imparti.")
  }
}
</script>

<style scoped>
.hermes-server-card {
  flex: 0 0 auto;
  margin: 8px;
}

.hermes-line {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 2px 0;
  font-size: 0.78rem;
}

.hermes-line__label {
  color: rgba(var(--v-theme-on-surface), 0.65);
  flex: 0 0 auto;
}

.hermes-line__value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: right;
}

.hermes-served {
  padding: 0;
  background: transparent;
}
</style>
