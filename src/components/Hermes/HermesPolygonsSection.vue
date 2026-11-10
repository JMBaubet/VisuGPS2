<template>
  <div class="hermes-section">
    <div class="hermes-section__header">
      <v-icon icon="mdi-shape-polygon-plus" size="16" class="mr-1" />
      <span class="text-subtitle-2">Polygones</span>
      <v-chip size="x-small" variant="tonal" class="ml-2">
        {{ hermesStore.polygons.length }}
      </v-chip>
      <v-spacer />
      <v-btn
        size="x-small"
        variant="tonal"
        prepend-icon="mdi-plus"
        :disabled="busy"
        @click="emit('create-polygon')"
      >
        Nouveau
      </v-btn>
      <v-btn
        icon="mdi-refresh"
        size="x-small"
        variant="text"
        title="Recharger les polygones"
        :loading="hermesStore.polygonsLoading"
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
      placeholder="Filtrer les polygones…"
      class="mb-1 hermes-section__filter"
    />

    <div class="hermes-section__hint">
      <v-icon
        icon="mdi-information-outline"
        size="14"
        class="hermes-section__hint-icon"
      />
      <span>
        « Nouveau » dessine un polygone sur la carte ; les sommets restent
        modifiables.
      </span>
    </div>

    <div
      v-if="hermesStore.polygonsLoading && hermesStore.polygons.length === 0"
      class="d-flex justify-center py-6"
    >
      <v-progress-circular indeterminate size="24" />
    </div>

    <v-list
      v-else-if="filteredPolygons.length > 0"
      density="compact"
      nav
      class="hermes-list"
    >
      <v-list-item
        v-for="polygon in filteredPolygons"
        :key="polygon.id"
        :active="polygon.id === selectedPolygonId"
        @click="toggle(polygon.id)"
        @mouseenter="emit('hover-polygon', polygon.id)"
        @mouseleave="emit('hover-polygon', null)"
      >
        <template #prepend>
          <span
            class="hermes-polygon-swatch"
            :style="{ backgroundColor: polygon.color ?? '#3B82F6' }"
          />
        </template>
        <v-list-item-title>{{ polygon.name }}</v-list-item-title>
        <v-list-item-subtitle>
          {{ polygon.id }} · {{ areaLabel(polygon) }} · rév.
          {{ polygon.revision }}
        </v-list-item-subtitle>
        <template #append>
          <div class="d-flex align-center ga-1">
            <v-btn
              icon="mdi-pencil-outline"
              size="x-small"
              variant="text"
              title="Modifier le polygone"
              :disabled="busy"
              @click.stop="emit('edit-polygon', polygon)"
            />
            <v-btn
              icon="mdi-delete-outline"
              size="x-small"
              variant="text"
              color="error"
              title="Supprimer le polygone"
              :disabled="busy"
              @click.stop="emit('delete-polygon', polygon)"
            />
          </div>
        </template>
      </v-list-item>
    </v-list>

    <v-empty-state
      v-else
      icon="mdi-shape-polygon-plus"
      title="Aucun polygone"
      text="Aucun polygone custom, ou filtre trop restrictif."
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Onglet « Polygones » : liste, création, édition et suppression.
 *
 * Cliquer un polygone le sélectionne et cadre la carte ; un second clic
 * désélectionne. Le survol est remonté à la vue, qui le transmet à la carte.
 */
import { computed, onMounted, ref } from 'vue'
import { useHermesStore, type HermesPolygon } from '../../stores/hermes'
import { polygonAreaKm2 } from '../../utils/hermes'

const props = defineProps<{
  selectedPolygonId: string | null
  /** Une requête de création/édition/suppression est en cours. */
  busy: boolean
}>()

const emit = defineEmits<{
  'select-polygon': [id: string | null]
  'create-polygon': []
  'edit-polygon': [polygon: HermesPolygon]
  'delete-polygon': [polygon: HermesPolygon]
  'hover-polygon': [id: string | null]
}>()

const hermesStore = useHermesStore()
const search = ref('')

const filteredPolygons = computed(() => {
  const query = search.value.trim().toLowerCase()
  if (!query) return hermesStore.polygons
  return hermesStore.polygons.filter(polygon => {
    return (
      polygon.name.toLowerCase().includes(query) ||
      polygon.id.toLowerCase().includes(query)
    )
  })
})

function areaLabel(polygon: HermesPolygon): string {
  try {
    const area = polygonAreaKm2(polygon.geometry)
    return Number.isFinite(area) ? `${area.toFixed(1)} km²` : '—'
  } catch {
    return '—'
  }
}

function toggle(polygonId: string): void {
  emit(
    'select-polygon',
    polygonId === props.selectedPolygonId ? null : polygonId,
  )
}

async function reload(): Promise<void> {
  await hermesStore.loadPolygons()
}

onMounted(() => {
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

.hermes-polygon-swatch {
  display: inline-block;
  width: 14px;
  height: 14px;
  border-radius: 3px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.25);
}
</style>
