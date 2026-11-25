<template>
  <div class="hermes-section">
    <div class="hermes-section__header">
      <v-icon icon="mdi-database" size="16" class="mr-1" />
      <span class="text-subtitle-2">Jeu de données</span>
      <v-chip size="x-small" variant="tonal" class="ml-2">
        {{ hermesStore.datasets.length }}
      </v-chip>
      <v-spacer />
      <v-btn
        size="x-small"
        variant="tonal"
        prepend-icon="mdi-plus"
        :disabled="hermesStore.datasetsLoading"
        @click="emit('create-dataset')"
      >
        Nouveau
      </v-btn>
      <v-btn
        icon="mdi-refresh"
        size="x-small"
        variant="text"
        title="Recharger les jeux de données"
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
      placeholder="Filtrer les jeux de données…"
      class="mb-1 hermes-section__filter"
    />

    <div class="hermes-section__hint">
      <v-icon
        icon="mdi-information-outline"
        size="14"
        class="hermes-section__hint-icon"
      />
      <span>
        Un jeu de données assemble des secteurs existants. Cliquez l'icône
        d'une ligne pour le servir ; l'arrêt et le démarrage du service se font
        dans la carte Serveur.
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
      <HermesDatasetRow
        v-for="dataset in filteredDatasets"
        :key="dataset.id"
        :dataset="dataset"
        :active="dataset.id === hermesStore.activeDatasetId"
        interactive
        show-delete
        @select-dataset="emit('select-dataset', $event)"
        @update-dataset="emit('update-dataset', $event)"
        @delete-dataset="emit('delete-dataset', $event)"
        @hover-dataset="emit('hover-dataset', $event)"
      />
    </v-list>

    <v-empty-state
      v-else
      icon="mdi-database-off-outline"
      :title="
        hermesStore.datasets.length > 0
          ? 'Aucun jeu de données correspondant'
          : 'Aucun jeu de données'
      "
      :text="
        hermesStore.datasets.length > 0
          ? 'Le filtre ne correspond à aucun jeu de données.'
          : 'Aucun jeu de données sur le serveur.'
      "
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Onglet « Jeu de données » : liste, création, suppression et sélection.
 *
 * Chaque ligne est rendue par `HermesDatasetRow` (partagée avec la carte
 * Serveur). L'icône de tête sélectionne le dataset servi (`PUT /serving/{id}`)
 * après confirmation dans la vue.
 */
import { computed, onMounted, ref } from 'vue'
import { useHermesStore, type HermesDataset } from '../../stores/hermes'
import { datasetDisplayName, datasetSectorNames } from '../../utils/hermes'
import HermesDatasetRow from './HermesDatasetRow.vue'

const emit = defineEmits<{
  'create-dataset': []
  'delete-dataset': [dataset: HermesDataset]
  'select-dataset': [dataset: HermesDataset]
  'update-dataset': [dataset: HermesDataset]
  'hover-dataset': [id: string | null]
}>()

const hermesStore = useHermesStore()
const search = ref('')

/** Filtre sur le nom, l'identifiant, les régions et les secteurs. */
const filteredDatasets = computed(() => {
  const query = search.value.trim().toLowerCase()
  if (!query) return hermesStore.datasets
  return hermesStore.datasets.filter(dataset => {
    const regions = (dataset.regions ?? []).join(' ').toLowerCase()
    const sectors = datasetSectorNames(dataset, hermesStore.polygons)
      .join(' ')
      .toLowerCase()
    return (
      datasetDisplayName(dataset).toLowerCase().includes(query) ||
      dataset.id.toLowerCase().includes(query) ||
      regions.includes(query) ||
      sectors.includes(query)
    )
  })
})

async function reload(): Promise<void> {
  await hermesStore.loadDatasets()
}

onMounted(() => {
  if (hermesStore.datasets.length === 0) {
    void hermesStore.loadDatasets()
  }
  // Les noms des secteurs et l'état « à mettre à jour » dépendent de cette liste.
  if (hermesStore.polygons.length === 0) {
    void hermesStore.loadPolygons()
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
</style>
