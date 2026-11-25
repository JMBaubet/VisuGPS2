<template>
  <v-navigation-drawer
    v-if="modelValue"
    permanent
    location="right"
    width="420"
    class="hermes-polygon-form"
  >
    <v-card flat class="hermes-polygon-form__card">
      <v-card-title class="d-flex align-center">
        <v-icon icon="mdi-shape-polygon-plus" class="mr-2" />
        {{ mode === 'create' ? 'Nouveau secteur' : 'Modifier le secteur' }}
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

      <v-card-text class="hermes-polygon-form__body">
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

        <v-alert
          v-if="!previewGeometry"
          type="info"
          variant="tonal"
          density="compact"
          class="mt-2"
          text="Dessinez le secteur sur la carte : cliquez les sommets, puis double-cliquez pour terminer."
        />
        <v-alert
          v-else-if="isDraft"
          :type="draftAreaOver ? 'warning' : 'info'"
          variant="tonal"
          density="compact"
          class="mt-2"
          :text="draftInfo"
        />
        <v-alert
          v-else-if="!geometryValidation.ok"
          type="error"
          variant="tonal"
          density="compact"
          class="mt-2"
          :text="geometryValidation.message ?? 'Géométrie invalide.'"
        />
        <v-alert
          v-else-if="areaError"
          type="error"
          variant="tonal"
          density="compact"
          class="mt-2"
          :text="areaError"
        />
        <v-alert
          v-else
          type="info"
          variant="tonal"
          density="compact"
          class="mt-2"
          :text="geometryInfo"
        />

        <p class="text-caption text-medium-emphasis mt-3">
          Cliquez sur un point intermédiaire pour ajouter un sommet, glissez un
          sommet pour le déplacer, sélectionnez-le puis appuyez sur Suppr pour
          le retirer. Un secteur doit être fermé, sans trou, et comporter au
          plus 1000 sommets.
        </p>
        <p class="text-caption text-medium-emphasis mt-2">
          Le bouton « marqueur distance » de la carte affiche des cercles
          concentriques de 10 à 150 km autour d'un centre choisi. Il reste
          disponible en permanence sur les deux onglets.
        </p>
      </v-card-text>

      <v-divider />

      <v-card-actions>
        <v-spacer />
        <v-btn
          v-if="mode === 'create'"
          variant="text"
          prepend-icon="mdi-restart"
          :disabled="saving"
          @click="redraw"
        >
          Redessiner
        </v-btn>
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
  </v-navigation-drawer>
</template>

<script setup lang="ts">
/**
 * Formulaire de création / édition d'un polygone custom.
 *
 * Il est présenté dans un tiroir à droite, non modal : la carte reste
 * interactive pour manipuler les sommets et afficher les cercles de distance.
 * En création, le panneau s'ouvre **avant** le tracé : l'utilisateur peut
 * saisir le nom et suivre la surface/le nombre de sommets pendant qu'il dessine
 * (`draftGeometry`). Le tracé n'est pas saisi ici : il est dessiné sur la carte
 * (`HermesMap`) et transmis par la prop `geometry`, mise à jour à chaque
 * `draw.create`/`draw.update`. Le formulaire valide le nom, le slug, la
 * géométrie et la surface maximale (`Hermes.Limites.surfaceMaxPolygoneKm2`)
 * avant de déléguer la requête au parent.
 */
import { computed, ref, watch } from 'vue'
import type { HermesPolygon, HermesPolygonInput } from '../../stores/hermes'
import { useSettingsStore } from '../../stores/settings'
import {
  HERMES_LIMIT_DEFAULTS,
  HERMES_LIMIT_PATHS,
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
  /** Tracé en cours pendant la création, utilisé pour l'aperçu seulement. */
  draftGeometry: GeoJSON.Polygon | null
  saving: boolean
}>()

const emit = defineEmits<{
  submit: [payload: HermesPolygonInput]
  cancel: []
  redraw: []
}>()

const settingsStore = useSettingsStore()

const name = ref('')
const id = ref('')
const description = ref('')
const idTouched = ref(false)

/** Géométrie affichée : le tracé en cours, sinon la géométrie validée. */
const previewGeometry = computed<GeoJSON.Polygon | null>(
  () => props.geometry ?? props.draftGeometry,
)

/** Vrai tant que le polygone n'est pas terminé par un double-clic. */
const isDraft = computed(() => !props.geometry && !!props.draftGeometry)

/** Validation de soumission : ne porte que sur la géométrie terminée. */
const geometryValidation = computed(() =>
  validatePolygonGeometry(props.geometry),
)

/** Surface maximale autorisée, lue dans les paramètres (km²). */
const maxAreaKm2 = computed(() => {
  const value = Number(
    settingsStore.getParamDef(HERMES_LIMIT_PATHS.maxPolygonAreaKm2)?.value,
  )
  return Number.isFinite(value) && value > 0
    ? value
    : HERMES_LIMIT_DEFAULTS.maxPolygonAreaKm2
})

const vertices = computed(() => {
  const ring = previewGeometry.value?.coordinates[0]
  if (!ring) return 0
  // Pendant le tracé, Mapbox Draw ajoute le curseur et referme l'anneau :
  // on retire ces deux points pour compter les sommets réellement posés.
  return Math.max(0, ring.length - (isDraft.value ? 2 : 1))
})

const areaKm2 = computed(() => {
  const geometry = previewGeometry.value
  if (!geometry) return 0
  const ring = geometry.coordinates[0]
  if (!ring || ring.length < 4) return 0
  return polygonAreaKm2(geometry)
})

const areaError = computed(() => {
  if (!props.geometry || !geometryValidation.value.ok) return null
  if (areaKm2.value <= maxAreaKm2.value) return null
  return `Surface ${areaKm2.value.toFixed(
    1,
  )} km² supérieure au maximum autorisé (${maxAreaKm2.value} km²). Glissez ou supprimez des sommets pour la réduire, ou cliquez « Redessiner ».`
})

const draftAreaOver = computed(
  () => isDraft.value && areaKm2.value > maxAreaKm2.value,
)

const geometryInfo = computed(
  () =>
    `${vertices.value} sommets · ${areaKm2.value.toFixed(
      1,
    )} km² / max ${maxAreaKm2.value} km²`,
)

const draftInfo = computed(() => {
  const base = `Brouillon : ${vertices.value} sommets · ${areaKm2.value.toFixed(
    1,
  )} km² / max ${maxAreaKm2.value} km².`
  if (draftAreaOver.value) {
    return `${base} Surface déjà au-dessus de la limite : réduisez le tracé, ou double-cliquez puis ajustez les sommets.`
  }
  return `${base} Double-cliquez sur le dernier sommet pour terminer.`
})

const nameError = computed(() =>
  name.value.trim() ? null : 'Le nom est obligatoire.',
)

const idError = computed(() => {
  if (props.mode !== 'create') return null
  return isValidPolygonSlug(id.value.trim())
    ? null
    : 'Identifiant invalide : slug ASCII de 1 à 64 caractères, « index » réservé.'
})

const canSubmit = computed(
  () =>
    !props.saving &&
    !nameError.value &&
    !idError.value &&
    !areaError.value &&
    geometryValidation.value.ok,
)

function initialize(): void {
  if (props.mode === 'edit' && props.polygon) {
    name.value = props.polygon.name
    id.value = props.polygon.id
    description.value = props.polygon.description ?? ''
    idTouched.value = true
    return
  }
  name.value = ''
  id.value = ''
  description.value = ''
  idTouched.value = false
}

function cancel(): void {
  emit('cancel')
}

/** Efface le tracé courant et relance le dessin, sans fermer le panneau. */
function redraw(): void {
  emit('redraw')
}

function submit(): void {
  if (!canSubmit.value || !props.geometry) return
  emit('submit', {
    id: id.value.trim(),
    name: name.value.trim(),
    description: description.value.trim() || null,
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
.hermes-polygon-form__card {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.hermes-polygon-form__body {
  flex: 1 1 auto;
  overflow-y: auto;
}
</style>
