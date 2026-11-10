<template>
  <div class="hermes-section">
    <div class="hermes-section__header">
      <v-icon icon="mdi-map-outline" size="16" class="mr-1" />
      <span class="text-subtitle-2">Régions</span>
      <v-chip size="x-small" variant="tonal" class="ml-2">
        {{ hermesStore.regions.length }}
      </v-chip>
      <v-spacer />
      <v-btn
        icon="mdi-refresh"
        size="x-small"
        variant="text"
        title="Recharger les régions"
        :loading="hermesStore.regionsLoading"
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
      placeholder="Filtrer les régions…"
      class="mb-1 hermes-section__filter"
    />

    <div class="hermes-section__hint">
      <v-icon
        icon="mdi-information-outline"
        size="14"
        class="hermes-section__hint-icon"
      />
      <span>
        Les régions et les fichiers PBF sont gérés par l'administrateur.
      </span>
    </div>

    <div
      v-if="hermesStore.regionsLoading && hermesStore.regions.length === 0"
      class="d-flex justify-center py-6"
    >
      <v-progress-circular indeterminate size="24" />
    </div>

    <v-list
      v-else-if="filteredRegions.length > 0"
      density="compact"
      nav
      class="hermes-list"
    >
      <v-list-item
        v-for="region in filteredRegions"
        :key="region.id"
        :active="region.id === selectedRegionId"
        @click="toggle(region.id)"
        @mouseenter="emit('hover-region', region.id)"
        @mouseleave="emit('hover-region', null)"
      >
        <template #prepend>
          <v-icon icon="mdi-map-outline" size="18" />
        </template>
        <v-list-item-title>{{ region.id }}</v-list-item-title>
        <v-list-item-subtitle>
          {{ formatBbox(region.bbox) }}
          <span v-if="region.geofabrikId"> · {{ region.geofabrikId }}</span>
        </v-list-item-subtitle>
        <template #append>
          <v-chip
            size="x-small"
            variant="tonal"
            :color="region.hasPoly ? 'success' : 'warning'"
          >
            {{ region.hasPoly ? 'contour' : 'bbox' }}
          </v-chip>
        </template>
      </v-list-item>
    </v-list>

    <v-empty-state
      v-else
      icon="mdi-map-search-outline"
      title="Aucune région"
      text="Aucune région indexée sur le serveur, ou filtre trop restrictif."
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Onglet « Régions » : lecture seule.
 *
 * Les régions viennent de `GET /regions` ; leur CRUD est réservé à
 * l'administrateur (le contrat interdit `X-Admin-Token` à M1). Cliquer une
 * région la sélectionne et cadre la carte ; un second clic désélectionne.
 */
import { computed, onMounted, ref } from 'vue'
import { useHermesStore } from '../../stores/hermes'
import { formatBbox } from '../../utils/hermes'

const props = defineProps<{
  selectedRegionId: string | null
}>()

const emit = defineEmits<{
  'select-region': [id: string | null]
  'hover-region': [id: string | null]
}>()

const hermesStore = useHermesStore()
const search = ref('')

const filteredRegions = computed(() => {
  const query = search.value.trim().toLowerCase()
  if (!query) return hermesStore.regions
  return hermesStore.regions.filter(region => {
    const geofabrik = (region.geofabrikId ?? '').toLowerCase()
    return (
      region.id.toLowerCase().includes(query) || geofabrik.includes(query)
    )
  })
})

function toggle(regionId: string): void {
  emit(
    'select-region',
    regionId === props.selectedRegionId ? null : regionId,
  )
}

async function reload(): Promise<void> {
  await hermesStore.loadRegions()
}

onMounted(() => {
  if (hermesStore.regions.length === 0) {
    void hermesStore.loadRegions()
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
