<template>
  <v-card flat border class="override-panel ma-2">
    <v-card-title class="text-subtitle-1 py-2">
      <v-icon icon="mdi-camera-control" class="mr-2" />
      {{ editingExisting ? `Édition : ${draft.name}` : 'Nouvel override caméra' }}
    </v-card-title>

    <v-card-text>
      <!-- Nom de la séquence -->
      <v-text-field
        v-model="draft.name"
        label="Nom de la séquence"
        density="compact"
        variant="outlined"
        hide-details
        class="mb-3"
      />

      <!-- Sélection de la plage temporelle -->
      <div class="text-caption text-medium-emphasis mb-1">Plage temporelle</div>
      <div class="d-flex align-center ga-2 mb-3">
        <v-text-field
          v-model="startFormatted"
          label="Début (mm:ss)"
          density="compact"
          variant="outlined"
          hide-details
          prepend-inner-icon="mdi-flag-outline"
          @blur="commitStart"
          @keydown.enter="commitStart"
        />
        <v-text-field
          v-model="endFormatted"
          label="Fin (mm:ss)"
          density="compact"
          variant="outlined"
          hide-details
          prepend-inner-icon="mdi-flag-checkered"
          @blur="commitEnd"
          @keydown.enter="commitEnd"
        />
      </div>

      <!-- Boutons rapides : marquer début/fin au curseur courant -->
      <div class="d-flex ga-2 mb-4">
        <v-btn
          size="small" variant="tonal" color="primary"
          prepend-icon="mdi-map-marker-left"
          @click="setStartToCursor"
        >
          Début au curseur
        </v-btn>
        <v-btn
          size="small" variant="tonal" color="primary"
          prepend-icon="mdi-map-marker-right"
          @click="setEndToCursor"
        >
          Fin au curseur
        </v-btn>
      </div>

      <!-- Sliders de paramètres caméra -->
      <div class="text-caption text-medium-emphasis mb-1">Paramètres caméra</div>

      <v-slider
        v-model="draft.params.zoom"
        :min="8" :max="20" :step="0.1"
        label="Zoom"
        thumb-label
        density="compact"
        class="mb-1"
      />
      <v-slider
        v-model="draft.params.pitch"
        :min="0" :max="85" :step="1"
        label="Pitch (°)"
        thumb-label
        density="compact"
        class="mb-1"
      />
      <v-slider
        v-model="draft.params.bearing"
        :min="0" :max="360" :step="1"
        label="Bearing (°)"
        thumb-label
        density="compact"
        class="mb-1"
      />

      <!-- Offsets relatifs (repliés) -->
      <v-expansion-panels class="mb-3">
        <v-expansion-panel title="Offsets relatifs (optionnel)">
          <template #text>
            <v-slider
              v-model="draft.params.zoom_offset"
              :min="-5" :max="5" :step="0.1"
              label="Δ Zoom"
              thumb-label density="compact" class="mb-1"
            />
            <v-slider
              v-model="draft.params.pitch_offset"
              :min="-30" :max="30" :step="1"
              label="Δ Pitch (°)"
              thumb-label density="compact" class="mb-1"
            />
            <v-slider
              v-model="draft.params.bearing_offset"
              :min="-180" :max="180" :step="1"
              label="Δ Bearing (°)"
              thumb-label density="compact"
            />
          </template>
        </v-expansion-panel>
      </v-expansion-panels>

      <!-- Easing + Damping -->
      <div class="text-caption text-medium-emphasis mb-1">Transition</div>
      <div class="d-flex align-center ga-2 mb-1">
        <v-select
          v-model="draft.easing"
          :items="easingChoices"
          label="Easing"
          density="compact" variant="outlined" hide-details
        />
        <v-slider
          v-model="draft.damping"
          :min="0" :max="1" :step="0.05"
          label="Damping"
          thumb-label density="compact" hide-details
          style="max-width: 180px"
        />
      </div>
    </v-card-text>

    <v-card-actions class="justify-end">
      <v-btn variant="text" @click="$emit('cancel')">Annuler</v-btn>
      <v-btn color="primary" variant="tonal" prepend-icon="mdi-check"
        @click="apply"
      >
        {{ editingExisting ? 'Mettre à jour' : 'Appliquer l\'override' }}
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
/**
 * Panneau d'édition d'un override caméra (§10.3).
 *
 * Permet de définir / modifier :
 *  - le nom de la séquence ;
 *  - la plage temporelle `[start_time, end_time]` (saisie mm:ss ou boutons
 *    « au curseur » pour utiliser la position courante de la timeline) ;
 *  - les paramètres caméra (zoom/pitch/bearing absolus + offsets relatifs) ;
 *  - la fonction d'easing et le damping (lissage de la transition).
 *
 * Chaque modification d'un slider met à jour `draft` ; la map est mise à jour
 * en live (via le watcher du store quand `apply` persiste l'override). À noter :
 * pendant l'édition, l'aperçu live de la plage se ferait via une override
 * temporaire — non implémenté en phase 1, l'utilisateur valide puis ajuste.
 */
import { reactive, computed, ref, watch } from 'vue'
import { useKeyframesStore } from '../../stores/keyframes'
import {
  createEmptyOverride,
  type Override,
  type EasingType,
} from '../../utils/keyframes'

const props = defineProps<{
  /** Override à éditer, ou null pour en créer un nouveau. */
  override?: Override | null
}>()

const emit = defineEmits<{
  (e: 'cancel'): void
  (e: 'applied', override: Override): void
}>()

const keyframesStore = useKeyframesStore()

/** true si on édite un override existant (vs création). */
const editingExisting = computed(() => !!props.override)

/**
 * Brouillon local éditable. Initialisé depuis l'override fourni, ou depuis un
 * override vide couvrant la position courante du curseur ± un intervalle par
 * défaut (5 s de part et d'autre).
 */
function makeDraft(): Override {
  if (props.override) return { ...props.override, params: { ...props.override.params } }
  const cursor = keyframesStore.currentTime
  const span = 5000
  const start = Math.max(0, cursor - span)
  const end = Math.min(keyframesStore.totalDuration, cursor + span)
  return createEmptyOverride(start, end)
}

const draft = reactive<Override>(makeDraft())

// S'assurer que les champs de params existent (v-slider a besoin de valeurs
// définies, pas undefined). On les initialise à une valeur neutre si absents.
function ensureParamDefaults() {
  const p = draft.params
  if (p.zoom === undefined) p.zoom = keyframesStore.rawKeyframes?.keyframes[0]?.cam.zoom ?? 16
  if (p.pitch === undefined) p.pitch = keyframesStore.rawKeyframes?.keyframes[0]?.cam.pitch ?? 60
  if (p.bearing === undefined) p.bearing = keyframesStore.rawKeyframes?.keyframes[0]?.cam.bearing ?? 0
  if (p.zoom_offset === undefined) p.zoom_offset = 0
  if (p.pitch_offset === undefined) p.pitch_offset = 0
  if (p.bearing_offset === undefined) p.bearing_offset = 0
  if (draft.damping === undefined) draft.damping = 1
}
ensureParamDefaults()
// Recharger les valeurs si l'override prop change (changement de sélection).
watch(() => props.override, () => {
  Object.assign(draft, makeDraft())
  ensureParamDefaults()
})

// --- Formatage mm:ss pour la saisie des bornes temporelles ---
// Champs libres (ref locaux), synchronisés depuis le draft via watch, puis
// commités au blur (tolère une frappe partielle avant validation).

const startFormatted = ref('')
const endFormatted = ref('')

function syncFormattedFields() {
  startFormatted.value = msToMmSs(draft.start_time)
  endFormatted.value = msToMmSs(draft.end_time)
}
syncFormattedFields()
watch(() => [draft.start_time, draft.end_time], syncFormattedFields)

/** Convertit ms → "mm:ss". */
function msToMmSs(ms: number): string {
  const total = Math.round(ms / 1000)
  const m = Math.floor(total / 60)
  const s = total % 60
  return `${m}:${s.toString().padStart(2, '0')}`
}

/** Convertit "mm:ss" → ms. Tolère aussi "ss" ou un nombre de secondes brut. */
function mmSsToMs(s: string): number | null {
  const trimmed = s.trim()
  if (!trimmed) return null
  if (trimmed.includes(':')) {
    const [mm, ss] = trimmed.split(':')
    const m = parseInt(mm, 10)
    const sec = parseInt(ss, 10)
    if (isNaN(m) || isNaN(sec)) return null
    return (m * 60 + sec) * 1000
  }
  const sec = parseFloat(trimmed)
  return isNaN(sec) ? null : sec * 1000
}

function commitStart() {
  const v = mmSsToMs(startFormatted.value)
  if (v !== null) draft.start_time = Math.max(0, Math.min(v, draft.end_time))
  else syncFormattedFields()
}
function commitEnd() {
  const v = mmSsToMs(endFormatted.value)
  if (v !== null) draft.end_time = Math.min(keyframesStore.totalDuration, Math.max(v, draft.start_time))
  else syncFormattedFields()
}

function setStartToCursor() {
  draft.start_time = Math.max(0, Math.min(keyframesStore.currentTime, draft.end_time))
}
function setEndToCursor() {
  draft.end_time = Math.min(keyframesStore.totalDuration, Math.max(keyframesStore.currentTime, draft.start_time))
}

const easingChoices: EasingType[] = ['linear', 'smoothstep', 'easeInOut']

/** Valide l'override : persiste dans le store puis émet `applied`. */
async function apply() {
  const override: Override = {
    ...draft,
    params: { ...draft.params },
  }
  // Nettoyer les offsets nuls pour ne pas polluer le JSON.
  for (const k of ['zoom_offset', 'pitch_offset', 'bearing_offset'] as const) {
    if (override.params[k] === 0) override.params[k] = undefined
  }
  if (editingExisting.value && props.override) {
    await keyframesStore.updateOverride(props.override.id, override)
  } else {
    await keyframesStore.addOverride(override)
  }
  emit('applied', override)
}
</script>

<style scoped>
.override-panel {
  /* Panneau latéral droit, sous la liste des overrides. */
}
</style>
