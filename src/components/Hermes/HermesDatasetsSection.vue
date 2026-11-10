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
        icon="mdi-refresh"
        size="x-small"
        variant="text"
        title="Recharger les datasets"
        :loading="hermesStore.datasetsLoading"
        @click="reload"
      />
    </div>

    <v-alert
      type="info"
      variant="tonal"
      density="compact"
      class="mb-2"
      text="La création, la suppression et la gestion ORS arrivent à la sous-étape 1.6."
    />

    <div
      v-if="hermesStore.datasetsLoading && hermesStore.datasets.length === 0"
      class="d-flex justify-center py-6"
    >
      <v-progress-circular indeterminate size="24" />
    </div>

    <v-list
      v-else-if="hermesStore.datasets.length > 0"
      density="compact"
      nav
      class="hermes-list"
    >
      <v-list-item
        v-for="dataset in hermesStore.datasets"
        :key="dataset.id"
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
        </v-list-item-subtitle>

        <template #append>
          <div class="d-flex align-center ga-1">
            <v-btn
              v-if="
                dataset.status === 'ready' &&
                dataset.id !== hermesStore.activeDatasetId
              "
              size="x-small"
              variant="text"
              prepend-icon="mdi-check-circle-outline"
              @click.stop="activate(dataset.id)"
            >
              Activer
            </v-btn>
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
          </div>
        </template>
      </v-list-item>
    </v-list>

    <v-empty-state
      v-else
      icon="mdi-database-off-outline"
      title="Aucun dataset"
      text="Aucun dataset sur le serveur."
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Onglet « Datasets » : liste et activation du dataset courant.
 *
 * La création (régions/zones), la suppression et la gestion des instances ORS
 * sont ajoutées en 1.6. Un dataset en préparation est suivi automatiquement
 * par le store (`loadDatasets` relance le polling).
 */
import { onMounted } from 'vue'
import {
  useHermesStore,
  type HermesDataset,
} from '../../stores/hermes'
import { useUiStore } from '../../stores/ui'
import { formatFileSize } from '../../utils/format'
import { parseHermesError } from '../../utils/hermes'

const hermesStore = useHermesStore()
const ui = useUiStore()

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

async function activate(datasetId: string): Promise<void> {
  try {
    await hermesStore.setActiveDataset(datasetId)
  } catch (error) {
    ui.showError(parseHermesError(error).message)
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
