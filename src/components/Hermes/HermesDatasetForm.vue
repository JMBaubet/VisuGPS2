<template>
  <v-dialog :model-value="modelValue" max-width="760" persistent>
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon icon="mdi-database-plus" class="mr-2" />
        Nouveau dataset
        <v-spacer />
        <v-btn
          icon="mdi-close"
          variant="text"
          title="Annuler"
          :disabled="saving"
          @click="cancel"
        />
      </v-card-title>

      <v-divider />

      <v-card-text>
        <v-text-field
          v-model="name"
          label="Nom (optionnel)"
          density="compact"
          variant="outlined"
          hint="Généré par le serveur si vide."
          persistent-hint
          class="mb-3"
        />

        <div class="d-flex align-center ga-2 mb-3 flex-wrap">
          <v-btn-toggle
            v-model="mode"
            mandatory
            density="compact"
            variant="outlined"
            @update:model-value="onModeChange"
          >
            <v-btn value="regions" prepend-icon="mdi-map-outline">
              Régions
            </v-btn>
            <v-btn value="zones" prepend-icon="mdi-shape-polygon-plus">
              Zones
            </v-btn>
          </v-btn-toggle>
          <v-spacer />
          <v-chip size="small" variant="tonal">
            {{ selectedIds.length }} sélectionné{{
              selectedIds.length > 1 ? 's' : ''
            }}
          </v-chip>
        </div>

        <v-text-field
          v-model="search"
          density="compact"
          variant="outlined"
          hide-details
          clearable
          prepend-inner-icon="mdi-magnify"
          :placeholder="
            mode === 'regions'
              ? 'Filtrer les régions…'
              : 'Filtrer les polygones…'
          "
          class="mb-2"
        />

        <v-alert
          v-if="limitWarning"
          type="warning"
          variant="tonal"
          density="compact"
          class="mb-2"
          :text="limitWarning"
        />

        <template v-if="mode === 'regions'">
          <div
            v-if="hermesStore.regionsLoading && hermesStore.regions.length === 0"
            class="d-flex justify-center py-6"
          >
            <v-progress-circular indeterminate size="24" />
          </div>
          <v-list
            v-else-if="filteredRegions.length > 0"
            density="compact"
            class="hermes-dataset-form__list"
          >
            <v-list-item
              v-for="region in filteredRegions"
              :key="region.id"
              @click="toggleId(region.id)"
            >
              <template #prepend>
                <v-checkbox-btn
                  :model-value="selectedIds.includes(region.id)"
                  @click.stop="toggleId(region.id)"
                />
              </template>
              <v-list-item-title>{{ region.id }}</v-list-item-title>
              <v-list-item-subtitle>
                {{ formatBbox(region.bbox) }}
              </v-list-item-subtitle>
            </v-list-item>
          </v-list>
          <v-empty-state
            v-else
            icon="mdi-map-search-outline"
            title="Aucune région"
            text="Aucune région indexée, ou filtre trop restrictif."
          />
        </template>

        <template v-else>
          <div
            v-if="
              hermesStore.polygonsLoading && hermesStore.polygons.length === 0
            "
            class="d-flex justify-center py-6"
          >
            <v-progress-circular indeterminate size="24" />
          </div>
          <v-list
            v-else-if="filteredPolygons.length > 0"
            density="compact"
            class="hermes-dataset-form__list"
          >
            <v-list-item
              v-for="polygon in filteredPolygons"
              :key="polygon.id"
              @click="toggleId(polygon.id)"
            >
              <template #prepend>
                <v-checkbox-btn
                  :model-value="selectedIds.includes(polygon.id)"
                  @click.stop="toggleId(polygon.id)"
                />
              </template>
              <v-list-item-title>
                <span
                  class="hermes-dataset-form__swatch"
                  :style="{ backgroundColor: polygon.color ?? '#3B82F6' }"
                />
                {{ polygon.name }}
              </v-list-item-title>
              <v-list-item-subtitle>
                {{ polygon.id }} · {{ areaLabel(polygon) }}
              </v-list-item-subtitle>
            </v-list-item>
          </v-list>
          <v-empty-state
            v-else
            icon="mdi-shape-polygon-plus"
            title="Aucun polygone"
            text="Créez d'abord un polygone dans l'onglet Polygones ; un dataset assemble des zones existantes."
          />
        </template>

        <div
          v-if="mode === 'zones' && selectedIds.length > 0"
          class="text-caption text-medium-emphasis mt-2"
        >
          Surface estimée : {{ selectedAreaKm2.toFixed(1) }} km²
        </div>

        <v-alert
          type="info"
          variant="tonal"
          density="compact"
          class="mt-2"
          text="Le serveur applique ses propres limites (régions, zones, surface) et vérifie la présence des PBF ; une erreur 400 est affichée telle quelle."
        />
      </v-card-text>

      <v-divider />

      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" :disabled="saving" @click="cancel">
          Annuler
        </v-btn>
        <v-btn
          color="primary"
          variant="flat"
          :loading="saving"
          :disabled="!canSubmit"
          @click="submit"
        >
          Créer le dataset
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Formulaire de création d'un dataset.
 *
 * Un dataset **assemble des objets existants** : des régions indexées ou des
 * polygones custom déjà créés dans l'onglet Polygones. Ce formulaire ne
 * dessine jamais de polygone.
 *
 * Les limites (4 régions, 100 zones, 50 000 km²) sont des **avertissements
 * souples** : le serveur reste l'autorité et ses erreurs 400 sont affichées
 * telles quelles. La sélection est remontée au parent (`preview`) pour
 * surligner les objets sur la carte.
 */
import { computed, ref, watch } from 'vue'
import {
  useHermesStore,
  type HermesCreateDatasetPayload,
  type HermesPolygon,
} from '../../stores/hermes'
import {
  DATASET_LIMITS,
  formatBbox,
  polygonAreaKm2,
} from '../../utils/hermes'

const props = defineProps<{
  modelValue: boolean
  saving: boolean
}>()

const emit = defineEmits<{
  submit: [payload: HermesCreateDatasetPayload]
  cancel: []
  preview: [payload: { regionIds: string[]; polygonIds: string[] }]
}>()

const hermesStore = useHermesStore()

const name = ref('')
const mode = ref<'regions' | 'zones'>('regions')
const selectedIds = ref<string[]>([])
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

function safeArea(polygon: HermesPolygon): number {
  try {
    const area = polygonAreaKm2(polygon.geometry)
    return Number.isFinite(area) ? area : 0
  } catch {
    return 0
  }
}

function areaLabel(polygon: HermesPolygon): string {
  return `${safeArea(polygon).toFixed(1)} km²`
}

const selectedAreaKm2 = computed(() => {
  if (mode.value !== 'zones') return 0
  const selected = new Set(selectedIds.value)
  return hermesStore.polygons
    .filter(polygon => selected.has(polygon.id))
    .reduce((sum, polygon) => sum + safeArea(polygon), 0)
})

const limitWarning = computed(() => {
  if (
    mode.value === 'regions' &&
    selectedIds.value.length > DATASET_LIMITS.maxRegions
  ) {
    return `Le serveur accepte ${DATASET_LIMITS.maxRegions} régions par défaut ; au-delà, la création peut être refusée.`
  }
  if (mode.value === 'zones') {
    if (selectedIds.value.length > DATASET_LIMITS.maxZones) {
      return `Le serveur accepte ${DATASET_LIMITS.maxZones} zones par défaut ; au-delà, la création peut être refusée.`
    }
    if (selectedAreaKm2.value > DATASET_LIMITS.maxAreaKm2) {
      return `Surface estimée ${selectedAreaKm2.value.toFixed(
        1,
      )} km² : au-delà de ${DATASET_LIMITS.maxAreaKm2} km², le serveur peut refuser.`
    }
  }
  return null
})

const canSubmit = computed(
  () => !props.saving && selectedIds.value.length > 0,
)

function toggleId(id: string): void {
  selectedIds.value = selectedIds.value.includes(id)
    ? selectedIds.value.filter(candidate => candidate !== id)
    : [...selectedIds.value, id]
}

function onModeChange(): void {
  selectedIds.value = []
  search.value = ''
}

function emitPreview(): void {
  emit('preview', {
    regionIds: mode.value === 'regions' ? [...selectedIds.value] : [],
    polygonIds: mode.value === 'zones' ? [...selectedIds.value] : [],
  })
}

function reset(): void {
  name.value = ''
  mode.value = 'regions'
  selectedIds.value = []
  search.value = ''
  emitPreview()
}

function cancel(): void {
  emit('cancel')
}

function submit(): void {
  if (!canSubmit.value) return
  emit('submit', {
    name: name.value.trim() || null,
    regions: mode.value === 'regions' ? [...selectedIds.value] : null,
    zones: mode.value === 'zones' ? [...selectedIds.value] : null,
  })
}

watch(
  () => props.modelValue,
  open => {
    if (!open) {
      selectedIds.value = []
      emitPreview()
      return
    }
    reset()
    if (hermesStore.regions.length === 0) {
      void hermesStore.loadRegions()
    }
    if (hermesStore.polygons.length === 0) {
      void hermesStore.loadPolygons()
    }
  },
  { immediate: true },
)

watch([mode, selectedIds], () => emitPreview())
</script>

<style scoped>
.hermes-dataset-form__list {
  max-height: 260px;
  overflow-y: auto;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  border-radius: 4px;
}

.hermes-dataset-form__swatch {
  display: inline-block;
  width: 12px;
  height: 12px;
  margin-right: 6px;
  border-radius: 3px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.25);
  vertical-align: baseline;
}
</style>
