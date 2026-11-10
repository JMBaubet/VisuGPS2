<template>
  <div class="hermes-section">
    <div class="hermes-section__header">
      <v-icon icon="mdi-database" size="16" class="mr-1" />
      <span class="text-subtitle-2">Datasets</span>
      <v-chip size="x-small" variant="tonal" class="ml-2">
        {{ hermesStore.datasets.length }}
      </v-chip>
      <v-spacer />
      <v-btn
        size="x-small"
        variant="tonal"
        prepend-icon="mdi-plus"
        :disabled="hermesStore.orsStarting"
        @click="emit('create-dataset')"
      >
        Nouveau
      </v-btn>
      <v-btn
        icon="mdi-refresh"
        size="x-small"
        variant="text"
        title="Recharger les datasets"
        :loading="hermesStore.datasetsLoading"
        @click="reload"
      />
    </div>

    <v-text-field
      v-model="search"
      density="compact"
      variant="outlined"
      hide-details
      clearable
      prepend-inner-icon="mdi-magnify"
      placeholder="Filtrer les datasets…"
      class="mb-1 hermes-section__filter"
    />

    <div class="hermes-section__hint">
      <v-icon
        icon="mdi-information-outline"
        size="14"
        class="hermes-section__hint-icon"
      />
      <span>
        Un dataset assemble des régions ou des zones existantes ; la création
        ne dessine rien.
      </span>
    </div>

    <div
      v-if="hermesStore.datasetsLoading && hermesStore.datasets.length === 0"
      class="d-flex justify-center py-6"
    >
      <v-progress-circular indeterminate size="24" />
    </div>

    <v-list
      v-else-if="filteredDatasets.length > 0"
      density="compact"
      nav
      class="hermes-list"
    >
      <v-list-item
        v-for="dataset in filteredDatasets"
        :key="dataset.id"
        @mouseenter="emit('hover-dataset', dataset.id)"
        @mouseleave="emit('hover-dataset', null)"
      >
        <template #prepend>
          <v-icon
            :icon="
              dataset.id === hermesStore.activeDatasetId
                ? 'mdi-check-circle'
                : 'mdi-database-outline'
            "
            :color="
              dataset.id === hermesStore.activeDatasetId
                ? 'success'
                : undefined
            "
            size="18"
          />
        </template>

        <v-list-item-title class="d-flex align-center flex-wrap">
          {{ dataset.id }}
          <v-chip
            size="x-small"
            variant="tonal"
            :color="statusColor(dataset.status)"
            class="ml-2"
          >
            {{ dataset.status }}
          </v-chip>
          <v-chip
            v-if="dataset.stale"
            size="x-small"
            variant="tonal"
            color="warning"
            class="ml-1"
          >
            obsolète
          </v-chip>
          <v-progress-circular
            v-if="hermesStore.pollingDatasetId === dataset.id"
            indeterminate
            size="12"
            class="ml-2"
          />
        </v-list-item-title>

        <v-list-item-subtitle>{{ scope(dataset) }}</v-list-item-subtitle>
        <v-list-item-subtitle>
          {{ formatFileSize(dataset.sizeBytes) }} · ORS
          {{ dataset.orsVersion || '—' }}
          <span v-if="isActiveInstance(dataset)">
            · instance active
          </span>
        </v-list-item-subtitle>

        <template #append>
          <div class="d-flex align-center ga-1">
            <v-tooltip
              v-if="dataset.status === 'failed' && dataset.lastLogTail"
              location="left"
              max-width="420"
            >
              <template #activator="{ props }">
                <v-icon
                  v-bind="props"
                  icon="mdi-alert-circle-outline"
                  color="error"
                  size="18"
                />
              </template>
              <div class="hermes-log-tail">{{ dataset.lastLogTail }}</div>
            </v-tooltip>

            <v-menu location="bottom end">
              <template #activator="{ props }">
                <v-btn
                  v-bind="props"
                  icon="mdi-dots-vertical"
                  size="small"
                  variant="text"
                  :title="`Actions pour ${dataset.id}`"
                  :disabled="hermesStore.orsStarting"
                />
              </template>
              <v-list density="compact">
                <v-list-item
                  v-if="
                    dataset.status === 'ready' &&
                    dataset.id !== hermesStore.activeDatasetId
                  "
                  prepend-icon="mdi-check-circle-outline"
                  title="Définir comme actif"
                  @click="activate(dataset.id)"
                />
                <v-list-item
                  v-if="dataset.status === 'ready'"
                  :prepend-icon="
                    isActiveInstance(dataset)
                      ? 'mdi-stop-circle-outline'
                      : 'mdi-play-circle-outline'
                  "
                  :title="
                    isActiveInstance(dataset)
                      ? 'Arrêter ORS'
                      : 'Démarrer ORS'
                  "
                  :disabled="hermesStore.orsStarting"
                  @click="
                    isActiveInstance(dataset)
                      ? stopOrs(dataset)
                      : requestStartOrs(dataset)
                  "
                />
                <v-divider />
                <v-list-item
                  prepend-icon="mdi-delete-outline"
                  title="Supprimer"
                  class="text-error"
                  @click="emit('delete-dataset', dataset)"
                />
              </v-list>
            </v-menu>
          </div>
        </template>
      </v-list-item>
    </v-list>

    <v-empty-state
      v-else
      icon="mdi-database-off-outline"
      :title="
        hermesStore.datasets.length > 0
          ? 'Aucun dataset correspondant'
          : 'Aucun dataset'
      "
      :text="
        hermesStore.datasets.length > 0
          ? 'Le filtre ne correspond à aucun dataset.'
          : 'Aucun dataset sur le serveur.'
      "
    />

    <v-dialog v-model="orsDialogOpen" max-width="480" persistent>
      <v-card>
        <v-card-title class="text-h6">Démarrer ORS ?</v-card-title>
        <v-card-text>
          Une instance ORS est déjà active pour
          <b>{{ hermesStore.activeInstance?.id }}</b>. Démarrer
          <b>{{ orsCandidate?.id }}</b> arrêtera la précédente : le serveur ne
          maintient qu'une instance à la fois.
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" @click="cancelStartOrs">Annuler</v-btn>
          <v-btn color="primary" variant="flat" @click="confirmStartOrs">
            Démarrer
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
/**
 * Onglet « Datasets » : liste, activation, suppression et gestion ORS.
 *
 * La création ouvre `HermesDatasetForm`, qui assemble des régions ou des
 * polygones existants. Un build en préparation est suivi par le store
 * (`loadDatasets` relance le polling). ORS ne maintient qu'une instance :
 * démarrer un autre dataset demande confirmation.
 */
import { computed, onMounted, ref } from 'vue'
import {
  useHermesStore,
  type HermesDataset,
} from '../../stores/hermes'
import { useUiStore } from '../../stores/ui'
import { formatFileSize } from '../../utils/format'
import { parseHermesError } from '../../utils/hermes'

const emit = defineEmits<{
  'create-dataset': []
  'delete-dataset': [dataset: HermesDataset]
  'hover-dataset': [id: string | null]
}>()

const hermesStore = useHermesStore()
const ui = useUiStore()

const orsDialogOpen = ref(false)
const orsCandidate = ref<HermesDataset | null>(null)
const search = ref('')

/** Filtre sur l'identifiant, les régions et les zones assemblées. */
const filteredDatasets = computed(() => {
  const query = search.value.trim().toLowerCase()
  if (!query) return hermesStore.datasets
  return hermesStore.datasets.filter(dataset => {
    const regions = (dataset.regions ?? []).join(' ').toLowerCase()
    const zones = (dataset.zones ?? []).join(' ').toLowerCase()
    return (
      dataset.id.toLowerCase().includes(query) ||
      regions.includes(query) ||
      zones.includes(query)
    )
  })
})

function statusColor(status: string): string {
  switch (status) {
    case 'ready':
      return 'success'
    case 'preparing':
      return 'warning'
    case 'failed':
      return 'error'
    default:
      return 'grey'
  }
}

function scope(dataset: HermesDataset): string {
  if (dataset.regions && dataset.regions.length > 0) {
    return `régions : ${dataset.regions.join(', ')}`
  }
  if (dataset.zones && dataset.zones.length > 0) {
    return `zones : ${dataset.zones.join(', ')}`
  }
  return 'périmètre inconnu'
}

function isActiveInstance(dataset: HermesDataset): boolean {
  return hermesStore.activeInstance?.id === dataset.id
}

async function activate(datasetId: string): Promise<void> {
  try {
    await hermesStore.setActiveDataset(datasetId)
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  }
}

function requestStartOrs(dataset: HermesDataset): void {
  const current = hermesStore.activeInstance
  if (current && current.id !== dataset.id) {
    orsCandidate.value = dataset
    orsDialogOpen.value = true
    return
  }
  void startOrs(dataset)
}

async function startOrs(dataset: HermesDataset): Promise<void> {
  try {
    await hermesStore.startInstance(dataset.id)
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  }
}

async function stopOrs(dataset: HermesDataset): Promise<void> {
  try {
    await hermesStore.stopInstance(dataset.id)
    ui.showSuccess(`ORS arrêté pour ${dataset.id}.`)
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  }
}

function cancelStartOrs(): void {
  orsDialogOpen.value = false
  orsCandidate.value = null
}

function confirmStartOrs(): void {
  const dataset = orsCandidate.value
  orsDialogOpen.value = false
  orsCandidate.value = null
  if (dataset) {
    void startOrs(dataset)
  }
}

async function reload(): Promise<void> {
  await hermesStore.loadDatasets()
}

onMounted(() => {
  if (hermesStore.datasets.length === 0) {
    void hermesStore.loadDatasets()
  }
})
</script>

<style scoped>
.hermes-section {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  padding: 8px;
}

.hermes-section__header {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-bottom: 8px;
}

.hermes-section__filter {
  --v-input-control-height: 32px;
  flex: 0 0 auto;
  min-height: 32px;
}

.hermes-section__filter :deep(.v-field) {
  height: 32px;
  min-height: 32px;
  padding-top: 0;
  padding-bottom: 0;
}

.hermes-section__filter :deep(.v-field__input) {
  height: 32px;
  min-height: 32px;
  padding-top: 0;
  padding-bottom: 0;
  font-size: 0.8125rem;
}

.hermes-section__hint {
  display: flex;
  align-items: flex-start;
  gap: 4px;
  margin-bottom: 8px;
  color: rgba(var(--v-theme-on-surface), 0.65);
  font-size: 0.72rem;
  line-height: 1.3;
}

.hermes-section__hint-icon {
  flex: 0 0 auto;
  margin-top: 1px;
}

/* L'état vide de Vuetify se centre et déborde du panneau : on l'aligne en
   haut, juste sous le filtre et l'encart d'information. */
.hermes-section :deep(.v-empty-state) {
  min-height: 0;
  justify-content: flex-start;
  padding-top: 16px;
}

.hermes-list {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}

.hermes-log-tail {
  font-size: 0.75rem;
  line-height: 1.35;
  white-space: pre-wrap;
  max-height: 240px;
  overflow-y: auto;
}
</style>
