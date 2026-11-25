<template>
  <div class="hermes-section">
    <div class="hermes-section__header">
      <v-icon icon="mdi-shape-polygon-plus" size="16" class="mr-1" />
      <span class="text-subtitle-2">Secteurs</span>
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
        title="Recharger les secteurs"
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
      placeholder="Filtrer les secteurs…"
      class="mb-1 hermes-section__filter"
    />

    <div class="hermes-section__hint">
      <v-icon
        icon="mdi-information-outline"
        size="14"
        class="hermes-section__hint-icon"
      />
      <span>
        « Nouveau » dessine un secteur sur la carte ; les sommets restent
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
          <v-icon icon="mdi-vector-polygon" size="18" />
        </template>
        <v-list-item-title class="d-flex align-center flex-wrap">
          {{ polygon.name }}
          <v-chip
            v-if="isOversized(polygon)"
            size="x-small"
            variant="tonal"
            color="warning"
            class="ml-2"
          >
            surface &gt; max
          </v-chip>
        </v-list-item-title>
        <v-list-item-subtitle>
          {{ areaLabel(polygon) }} · modifié le
          {{ formatDateTime(polygon.updatedAt) }} · rév.
          {{ polygon.revision }}
        </v-list-item-subtitle>
        <v-list-item-subtitle v-if="appStore.isDebug">
          {{ polygon.id }}
        </v-list-item-subtitle>
        <template #append>
          <div class="d-flex align-center ga-1">
            <v-btn
              icon="mdi-pencil-outline"
              size="x-small"
              variant="text"
              title="Modifier le secteur"
              :disabled="busy"
              @click.stop="emit('edit-polygon', polygon)"
            />
            <v-tooltip
              v-if="usedByDataset(polygon.id)"
              location="left"
              max-width="300"
            >
              <template #activator="{ props }">
                <v-icon
                  v-bind="props"
                  icon="mdi-lock-outline"
                  size="16"
                  color="grey"
                />
              </template>
              <span>
                Secteur utilisé par au moins un jeu de données : suppression
                désactivée.
              </span>
            </v-tooltip>
            <v-btn
              v-else
              icon="mdi-delete-outline"
              size="x-small"
              variant="text"
              color="error"
              title="Supprimer le secteur"
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
      title="Aucun secteur"
      text="Aucun secteur custom, ou filtre trop restrictif."
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Onglet « Secteurs » : liste, création, édition et suppression.
 *
 * Cliquer un secteur le sélectionne et cadre la carte ; un second clic
 * désélectionne. Le survol est remonté à la vue, qui le transmet à la carte.
 * La suppression est masquée dès qu'un jeu de données utilise le secteur.
 */
import { computed, onMounted, ref } from 'vue'
import { useAppStore } from '../../stores/app'
import { useHermesStore, type HermesPolygon } from '../../stores/hermes'
import { useSettingsStore } from '../../stores/settings'
import { formatDateTime } from '../../utils/format'
import {
  HERMES_LIMIT_DEFAULTS,
  HERMES_LIMIT_PATHS,
  polygonAreaKm2,
} from '../../utils/hermes'

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
const settingsStore = useSettingsStore()
const appStore = useAppStore()
const search = ref('')

/** Surface maximale autorisée (km²), pour signaler les secteurs hérités. */
const maxAreaKm2 = computed(() => {
  const value = Number(
    settingsStore.getParamDef(HERMES_LIMIT_PATHS.maxPolygonAreaKm2)?.value,
  )
  return Number.isFinite(value) && value > 0
    ? value
    : HERMES_LIMIT_DEFAULTS.maxPolygonAreaKm2
})

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

/** Le secteur est-il assemblé dans au moins un jeu de données ? */
function usedByDataset(polygonId: string): boolean {
  return hermesStore.datasets.some(dataset =>
    dataset.zones?.includes(polygonId),
  )
}

function areaLabel(polygon: HermesPolygon): string {
  try {
    const area = polygonAreaKm2(polygon.geometry)
    return Number.isFinite(area) ? `${area.toFixed(1)} km²` : '—'
  } catch {
    return '—'
  }
}

function isOversized(polygon: HermesPolygon): boolean {
  try {
    return polygonAreaKm2(polygon.geometry) > maxAreaKm2.value
  } catch {
    return false
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
  // La poubelle dépend des jeux de données qui assemblent le secteur.
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
</style>
