<template>
  <v-card class="hermes-server-card" variant="tonal">
    <v-card-title class="d-flex align-center text-subtitle-1 py-2">
      <v-icon
        :icon="hermesStore.statusDisplay.icon"
        :color="hermesStore.statusDisplay.color"
        size="18"
        class="mr-2"
      />
      Serveur hermes
      <v-spacer />
      <v-chip
        :color="hermesStore.statusDisplay.color"
        variant="tonal"
        size="x-small"
      >
        {{ hermesStore.statusDisplay.label }}
      </v-chip>
    </v-card-title>

    <v-card-text class="pt-1 pb-3">
      <div class="hermes-line">
        <span class="hermes-line__label">URL</span>
        <span class="hermes-line__value" :title="hermesStore.baseUrl">
          {{ hermesStore.baseUrl }}
        </span>
      </div>
      <div class="hermes-line">
        <span class="hermes-line__label">Dernier ping</span>
        <span class="hermes-line__value">{{ lastPingLabel }}</span>
      </div>

      <v-select
        :model-value="hermesStore.activeDatasetId"
        :items="hermesStore.datasets"
        item-title="id"
        item-value="id"
        label="Dataset actif"
        density="compact"
        variant="outlined"
        hide-details
        clearable
        class="mt-2"
        :loading="hermesStore.datasetsLoading"
        :disabled="hermesStore.datasetsLoading"
        @update:model-value="onSelectDataset"
      >
        <template #item="{ props: itemProps, item }">
          <v-list-item
            v-bind="itemProps"
            :disabled="item.raw.status !== 'ready'"
            :subtitle="
              `${item.raw.status}${item.raw.stale ? ' · obsolète' : ''}`
            "
          />
        </template>
      </v-select>

      <div v-if="hermesStore.activeDataset" class="hermes-line mt-2">
        <span class="hermes-line__label">Statut</span>
        <span class="hermes-line__value">
          {{ hermesStore.activeDataset.status
          }}{{ hermesStore.activeDataset.stale ? ' · obsolète' : '' }}
        </span>
      </div>

      <div v-if="auxChips.length" class="hermes-aux mt-2">
        <v-chip
          v-for="chip in auxChips"
          :key="chip.key"
          size="x-small"
          variant="tonal"
          :color="chip.exists ? 'success' : 'warning'"
          :prepend-icon="
            chip.exists ? 'mdi-check-circle-outline' : 'mdi-alert-circle-outline'
          "
          :title="`${chip.label} : ${
            chip.exists ? formatFileSize(chip.size) : 'absent'
          }`"
        >
          {{ chip.label }}
        </v-chip>
      </div>

      <div class="hermes-line mt-2">
        <span class="hermes-line__label">Instance ORS</span>
        <span class="hermes-line__value">{{ orsLabel }}</span>
      </div>

      <v-alert
        v-if="hermesStore.lastError"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-2"
        :text="hermesStore.lastError"
      />

      <div v-if="hermesStore.wakeProgress" class="mt-2 text-body-2">
        Réveil en cours — tentative
        {{ hermesStore.wakeProgress.attempts }},
        {{ hermesStore.wakeProgress.elapsedSec }} s.
      </div>

      <div class="d-flex ga-2 mt-3">
        <v-btn
          size="small"
          variant="tonal"
          prepend-icon="mdi-refresh"
          :loading="hermesStore.loading"
          @click="refresh"
        >
          Tester
        </v-btn>
        <v-btn
          v-if="hermesStore.server !== 'online'"
          size="small"
          color="primary"
          variant="flat"
          prepend-icon="mdi-power"
          :loading="hermesStore.server === 'waking'"
          :disabled="hermesStore.server === 'waking'"
          @click="wake"
        >
          Réveiller
        </v-btn>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
/**
 * Carte « Serveur » du panneau Hermes : état, URL, dataset actif, artefacts
 * auxiliaires, instance ORS, réveil WoL.
 *
 * Elle est toujours visible, quel que soit l'onglet actif. Le choix du dataset
 * est persisté dans `Hermes.Dataset.actif` ; seuls les datasets `ready` sont
 * sélectionnables.
 */
import { computed } from 'vue'
import { useHermesStore } from '../../stores/hermes'
import { useUiStore } from '../../stores/ui'
import { formatDateTime, formatFileSize } from '../../utils/format'
import { parseHermesError } from '../../utils/hermes'

const hermesStore = useHermesStore()
const ui = useUiStore()

const lastPingLabel = computed(() => formatDateTime(hermesStore.lastPingAt))

const auxChips = computed(() => {
  const aux = hermesStore.activeAux
  if (!aux) return []
  return [
    {
      key: 'communes',
      label: 'communes',
      exists: aux.communes.exists,
      size: aux.communes.sizeBytes,
    },
    {
      key: 'tunnels',
      label: 'tunnels',
      exists: aux.tunnels.exists,
      size: aux.tunnels.sizeBytes,
    },
    {
      key: 'mm',
      label: 'mm',
      exists: aux.mm.exists,
      size: aux.mm.sizeBytes,
    },
  ]
})

const orsLabel = computed(() => {
  const instance = hermesStore.activeInstance
  const health = hermesStore.orsHealth
  if (!instance) {
    return health ? `aucune instance (${health})` : 'aucune instance'
  }
  return `${instance.id} (${instance.status})${health ? ` · ${health}` : ''}`
})

async function onSelectDataset(value: string | null): Promise<void> {
  try {
    await hermesStore.setActiveDataset(value ?? null)
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  }
}

async function refresh(): Promise<void> {
  await hermesStore.refreshServer()
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

.hermes-aux {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
</style>
