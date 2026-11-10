<template>
  <v-dialog :model-value="modelValue" max-width="600" persistent>
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon icon="mdi-shape-polygon-plus" class="mr-2" />
        {{ mode === 'create' ? 'Nouveau polygone' : 'Modifier le polygone' }}
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
          label="Nom"
          density="compact"
          variant="outlined"
          :error-messages="nameError"
          class="mb-2"
        />

        <v-text-field
          v-if="mode === 'create'"
          v-model="id"
          label="Identifiant"
          density="compact"
          variant="outlined"
          hint="Slug ASCII : lettres, chiffres, - et _ (64 max). « index » est réservé."
          persistent-hint
          :error-messages="idError"
          class="mb-2"
          @update:model-value="idTouched = true"
        />
        <v-text-field
          v-else
          :model-value="polygon?.id ?? ''"
          label="Identifiant"
          density="compact"
          variant="outlined"
          readonly
          class="mb-2"
        />

        <v-textarea
          v-model="description"
          label="Description (optionnel)"
          density="compact"
          variant="outlined"
          rows="2"
          auto-grow
          class="mb-2"
        />

        <v-text-field
          v-model="color"
          label="Couleur"
          density="compact"
          variant="outlined"
          :error-messages="colorError"
          class="mb-2"
        >
          <template #append-inner>
            <input v-model="color" type="color" class="hermes-color-input" />
          </template>
        </v-text-field>

        <v-alert
          v-if="geometryValidation.ok"
          type="info"
          variant="tonal"
          density="compact"
          class="mt-2"
          :text="geometryInfo"
        />
        <v-alert
          v-else
          type="error"
          variant="tonal"
          density="compact"
          class="mt-2"
          :text="geometryValidation.message ?? 'Géométrie invalide.'"
        />

        <p class="text-caption text-medium-emphasis mt-3">
          Les sommets se déplacent directement sur la carte. Un polygone doit
          être fermé, sans trou, et comporter au plus 1000 sommets.
        </p>
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
          {{ mode === 'create' ? 'Créer' : 'Enregistrer' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Formulaire de création / édition d'un polygone custom.
 *
 * Le tracé n'est pas saisi ici : il est dessiné sur la carte (`HermesMap`) et
 * transmis par la prop `geometry`, mise à jour à chaque `draw.update`. Le
 * formulaire valide le nom, le slug, la couleur et la géométrie avant de
 * déléguer la requête au parent.
 */
import { computed, ref, watch } from 'vue'
import type { HermesPolygon, HermesPolygonInput } from '../../stores/hermes'
import {
  isValidPolygonSlug,
  polygonAreaKm2,
  slugifyPolygonName,
  validatePolygonGeometry,
} from '../../utils/hermes'

const props = defineProps<{
  modelValue: boolean
  mode: 'create' | 'edit'
  polygon: HermesPolygon | null
  geometry: GeoJSON.Polygon | null
  saving: boolean
}>()

const emit = defineEmits<{
  submit: [payload: HermesPolygonInput]
  cancel: []
}>()

const name = ref('')
const id = ref('')
const description = ref('')
const color = ref('#3B82F6')
const idTouched = ref(false)

const geometryValidation = computed(() =>
  validatePolygonGeometry(props.geometry),
)

const vertices = computed(() => {
  const ring = props.geometry?.coordinates[0]
  return ring ? Math.max(0, ring.length - 1) : 0
})

const areaKm2 = computed(() => {
  if (!props.geometry || !geometryValidation.value.ok) return 0
  return polygonAreaKm2(props.geometry)
})

const geometryInfo = computed(
  () => `${vertices.value} sommets · ${areaKm2.value.toFixed(1)} km²`,
)

const nameError = computed(() =>
  name.value.trim() ? null : 'Le nom est obligatoire.',
)

const idError = computed(() => {
  if (props.mode !== 'create') return null
  return isValidPolygonSlug(id.value.trim())
    ? null
    : 'Identifiant invalide : slug ASCII de 1 à 64 caractères, « index » réservé.'
})

const colorError = computed(() =>
  /^#[0-9a-fA-F]{6}$/.test(color.value.trim())
    ? null
    : 'Couleur au format #RRGGBB attendue.',
)

const canSubmit = computed(
  () =>
    !props.saving &&
    !nameError.value &&
    !idError.value &&
    !colorError.value &&
    geometryValidation.value.ok,
)

function initialize(): void {
  if (props.mode === 'edit' && props.polygon) {
    name.value = props.polygon.name
    id.value = props.polygon.id
    description.value = props.polygon.description ?? ''
    color.value = props.polygon.color ?? '#3B82F6'
    idTouched.value = true
    return
  }
  name.value = ''
  id.value = ''
  description.value = ''
  color.value = '#3B82F6'
  idTouched.value = false
}

function cancel(): void {
  emit('cancel')
}

function submit(): void {
  if (!canSubmit.value || !props.geometry) return
  emit('submit', {
    id: id.value.trim(),
    name: name.value.trim(),
    description: description.value.trim() || null,
    color: color.value.trim(),
    geometry: props.geometry,
  })
}

watch(
  () => props.modelValue,
  open => {
    if (open) initialize()
  },
  { immediate: true },
)

watch(name, value => {
  if (props.mode === 'create' && !idTouched.value) {
    id.value = slugifyPolygonName(value)
  }
})
</script>

<style scoped>
.hermes-color-input {
  width: 32px;
  height: 24px;
  padding: 0;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.25);
  border-radius: 4px;
  background: transparent;
  cursor: pointer;
}
</style>
