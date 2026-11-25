<template>
  <v-dialog :model-value="modelValue" max-width="760" persistent>
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon icon="mdi-database-plus" class="mr-2" />
        Nouveau jeu de données
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
          hint="Généré automatiquement si vide. Lettres, chiffres, - et _ uniquement (64 max)."
          persistent-hint
          :error-messages="nameError"
          class="mb-3"
        />

        <div class="d-flex align-center ga-2 mb-3 flex-wrap">
          <v-chip size="small" variant="tonal">
            {{ selectedIds.length }} / {{ maxPolygons }} secteur{{
              maxPolygons > 1 ? 's' : ''
            }}
          </v-chip>
          <span class="text-caption text-medium-emphasis">
            Maximum {{ maxPolygons }} secteurs et
            {{ maxDatasetAreaKm2 }} km² par jeu de données
            (<code>Hermes.Limites.*</code>).
          </span>
        </div>

        <v-text-field
          v-model="search"
          density="compact"
          variant="outlined"
          hide-details
          clearable
          prepend-inner-icon="mdi-magnify"
          placeholder="Filtrer les secteurs…"
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
            :disabled="!isSelectable(polygon)"
            @click="toggleId(polygon.id)"
          >
            <template #prepend>
              <v-checkbox-btn
                :model-value="selectedIds.includes(polygon.id)"
                :disabled="!isSelectable(polygon)"
                @click.stop="toggleId(polygon.id)"
              />
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
              {{ polygon.id }} · {{ areaLabel(polygon) }}
            </v-list-item-subtitle>
          </v-list-item>
        </v-list>
        <v-empty-state
          v-else
          icon="mdi-shape-polygon-plus"
          title="Aucun secteur"
          text="Créez d'abord un secteur dans l'onglet Secteurs ; un jeu de données assemble des secteurs existants."
        />

        <div
          v-if="selectedIds.length > 0"
          class="text-caption text-medium-emphasis mt-2"
        >
          Surface du jeu de données : {{ selectedAreaKm2.toFixed(1) }} km² /
          max {{ maxDatasetAreaKm2 }} km²
        </div>

        <v-alert
          type="info"
          variant="tonal"
          density="compact"
          class="mt-2"
          text="Le serveur applique aussi ses propres limites (surface, présence des PBF) ; une erreur 400 est affichée telle quelle."
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
          Créer le jeu de données
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Formulaire de création d'un dataset.
 *
 * Un dataset **assemble des polygones (zones) existants** déjà créés dans
 * l'onglet Polygones. Ce formulaire ne dessine jamais de polygone et n'accepte
 * plus de régions, pour des raisons de performance côté serveur.
 *
 * Les limites locales viennent des paramètres `Hermes.Limites.*` : nombre de
 * polygones par dataset, surface maximale d'un polygone et surface maximale du
 * dataset (somme des zones, distincte de la limite par polygone). Les limites
 * du serveur restent l'autorité ; ses erreurs 400 sont affichées telles
 * quelles. La sélection est remontée au parent (`preview`) pour surligner les
 * polygones sur la carte.
 */
import { computed, ref, watch } from 'vue'
import {
  useHermesStore,
  type HermesCreateDatasetPayload,
  type HermesPolygon,
} from '../../stores/hermes'
import { useSettingsStore } from '../../stores/settings'
import {
  HERMES_LIMIT_DEFAULTS,
  HERMES_LIMIT_PATHS,
  polygonAreaKm2,
} from '../../utils/hermes'

const props = defineProps<{
  modelValue: boolean
  saving: boolean
}>()

const emit = defineEmits<{
  submit: [payload: HermesCreateDatasetPayload]
  cancel: []
  preview: [payload: { polygonIds: string[] }]
}>()

const hermesStore = useHermesStore()
const settingsStore = useSettingsStore()

const name = ref('')
const selectedIds = ref<string[]>([])
const search = ref('')

/** Même contrainte que le serveur : regex `^[a-zA-Z0-9_-]+$`, 64 max. */
const nameError = computed(() => {
  const value = name.value.trim()
  if (!value) return null
  if (value.length > 64) {
    return 'Le nom est limité à 64 caractères.'
  }
  return /^[a-zA-Z0-9_-]+$/.test(value)
    ? null
    : 'Nom invalide : lettres, chiffres, - et _ uniquement.'
})

/** Nombre maximal de polygones par dataset. */
const maxPolygons = computed(() => {
  const value = Number(
    settingsStore.getParamDef(HERMES_LIMIT_PATHS.maxPolygonsPerDataset)?.value,
  )
  return Number.isFinite(value) && value >= 1
    ? value
    : HERMES_LIMIT_DEFAULTS.maxPolygonsPerDataset
})

/** Surface maximale d'un polygone pris isolément (km²). */
const maxPolygonAreaKm2 = computed(() => {
  const value = Number(
    settingsStore.getParamDef(HERMES_LIMIT_PATHS.maxPolygonAreaKm2)?.value,
  )
  return Number.isFinite(value) && value > 0
    ? value
    : HERMES_LIMIT_DEFAULTS.maxPolygonAreaKm2
})

/** Surface maximale du dataset complet, somme des zones (km²). */
const maxDatasetAreaKm2 = computed(() => {
  const value = Number(
    settingsStore.getParamDef(HERMES_LIMIT_PATHS.maxDatasetAreaKm2)?.value,
  )
  return Number.isFinite(value) && value > 0
    ? value
    : HERMES_LIMIT_DEFAULTS.maxDatasetAreaKm2
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

function isOversized(polygon: HermesPolygon): boolean {
  return safeArea(polygon) > maxPolygonAreaKm2.value
}

/** Un polygone reste décochable même s'il est hors limite (paramètre abaissé). */
function isSelectable(polygon: HermesPolygon): boolean {
  if (selectedIds.value.includes(polygon.id)) return true
  if (isOversized(polygon)) return false
  return selectedIds.value.length < maxPolygons.value
}

const selectedAreaKm2 = computed(() => {
  const selected = new Set(selectedIds.value)
  return hermesStore.polygons
    .filter(polygon => selected.has(polygon.id))
    .reduce((sum, polygon) => sum + safeArea(polygon), 0)
})

const hasOversizedSelection = computed(() =>
  selectedIds.value.some(id => {
    const polygon = hermesStore.polygons.find(candidate => candidate.id === id)
    return polygon ? isOversized(polygon) : false
  }),
)

const limitWarning = computed(() => {
  if (selectedIds.value.length > maxPolygons.value) {
    return `Sélectionnez au plus ${maxPolygons.value} secteurs par jeu de données.`
  }
  if (hasOversizedSelection.value) {
    return `Un secteur sélectionné dépasse la surface maximale de ${maxPolygonAreaKm2.value} km².`
  }
  if (selectedAreaKm2.value > maxDatasetAreaKm2.value) {
    return `Surface du jeu de données ${selectedAreaKm2.value.toFixed(
      1,
    )} km² supérieure au maximum autorisé (${maxDatasetAreaKm2.value} km²).`
  }
  return null
})

const canSubmit = computed(() => {
  if (props.saving || nameError.value) return false
  const count = selectedIds.value.length
  if (count === 0 || count > maxPolygons.value) return false
  if (hasOversizedSelection.value) return false
  return selectedAreaKm2.value <= maxDatasetAreaKm2.value
})

function toggleId(id: string): void {
  if (selectedIds.value.includes(id)) {
    selectedIds.value = selectedIds.value.filter(candidate => candidate !== id)
    return
  }
  const polygon = hermesStore.polygons.find(candidate => candidate.id === id)
  if (!polygon || !isSelectable(polygon)) return
  selectedIds.value = [...selectedIds.value, id]
}

function emitPreview(): void {
  emit('preview', { polygonIds: [...selectedIds.value] })
}

function reset(): void {
  name.value = ''
  selectedIds.value = []
  search.value = ''
  emitPreview()
}

function cancel(): void {
  emit('cancel')
}

function submit(): void {
  if (!canSubmit.value) return
  const trimmedName = name.value.trim()
  emit('submit', {
    // Toujours envoyer un nom valide : évite de dépendre du chemin « name
    // absent » côté serveur pour les versions où il est fragile.
    name: trimmedName || `ds-${Date.now()}`,
    zones: [...selectedIds.value],
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
    if (hermesStore.polygons.length === 0) {
      void hermesStore.loadPolygons()
    }
  },
  { immediate: true },
)

watch(selectedIds, () => emitPreview())
</script>

<style scoped>
.hermes-dataset-form__list {
  max-height: 260px;
  overflow-y: auto;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  border-radius: 4px;
}
</style>
