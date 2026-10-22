<template>
  <v-expansion-panels v-model="opened" variant="accordion" class="osm-panel">
    <v-expansion-panel value="osm">
      <v-expansion-panel-title class="osm-panel__title">
        <v-icon icon="mdi-database" size="16" class="mr-2" />
        <span class="osm-panel__title-text">Données OSM</span>
        <v-chip
          v-if="total > 0"
          size="x-small"
          variant="tonal"
          :color="missingCount > 0 ? 'warning' : 'success'"
          class="ml-auto"
        >
          {{ presentCount }}/{{ total }}
        </v-chip>
        <v-chip
          v-else-if="requirements"
          size="x-small"
          variant="tonal"
          color="grey"
          class="ml-auto"
        >
          {{ readinessLabel }}
        </v-chip>
      </v-expansion-panel-title>

      <v-expansion-panel-text>
        <v-alert
          v-if="error"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-2"
          :text="error"
        />

        <v-alert
          v-if="readinessMessage"
          :type="readinessType"
          variant="tonal"
          density="compact"
          class="mb-2"
          :text="readinessMessage"
        />

        <v-alert
          v-if="!hasFile && !pending && requirements?.readiness === 'ready'"
          type="info"
          variant="tonal"
          density="compact"
          class="mb-2"
          text="Aucune analyse de tunnels enregistrée. Utilisez « Relancer la détection »."
        />

        <div v-if="globalLabel" class="osm-panel__progress">
          <div class="osm-panel__progress-label">{{ globalLabel }}</div>
          <v-progress-linear
            :model-value="globalPercent"
            :indeterminate="globalIndeterminate"
            color="info"
            height="4"
            rounded
          />
        </div>

        <div
          v-for="region in requirements?.regions ?? []"
          :key="region.id"
          class="osm-panel__row"
        >
          <v-icon
            :icon="statusFor(region).icon"
            :color="statusFor(region).color"
            size="16"
          />
          <div class="osm-panel__text">
            <div class="osm-panel__name" :title="region.name">
              {{ region.name }}
            </div>
            <div class="osm-panel__file" :title="region.pbfFilename">
              {{ region.pbfFilename }}
              <span v-if="region.sizeMb != null">
                — {{ formatSize(region.sizeMb) }}
              </span>
            </div>
            <v-progress-linear
              v-if="statusFor(region).percent != null"
              :model-value="statusFor(region).percent ?? 0"
              color="info"
              height="3"
              class="mt-1"
            />
          </div>
          <v-chip
            size="x-small"
            variant="tonal"
            :color="statusFor(region).color"
            class="osm-panel__status"
          >
            {{ statusFor(region).label }}
          </v-chip>
        </div>

        <v-btn
          v-if="tooLargeCount > 0 && !pending"
          block
          color="warning"
          variant="tonal"
          prepend-icon="mdi-download"
          class="mt-2"
          @click="emit('force-download')"
        >
          Télécharger quand même ({{ tooLargeCount }})
        </v-btn>
      </v-expansion-panel-text>
    </v-expansion-panel>
  </v-expansion-panels>
</template>

<script setup lang="ts">
/**
 * Panneau « Données OSM » de l'onglet Tunnels.
 *
 * Il rend visible le téléchargement **silencieux** demandé : `regions.json`,
 * PBF déjà chargés, PBF en cours avec leur pourcentage, PBF manquants. Il se
 * replie de lui-même quand tout est prêt, et s'ouvre dès qu'une ressource
 * manque, qu'une préparation est en cours ou qu'une erreur est remontée.
 */
import { computed, ref, watch } from 'vue'
import type {
  TunnelProgress,
  TunnelRequirementRegion,
  TunnelRequirements,
} from '../../stores/tunnels'

const props = defineProps<{
  requirements: TunnelRequirements | null
  progress: TunnelProgress | null
  pending: boolean
  preparing: boolean
  hasFile: boolean
  error: string | null
}>()

const emit = defineEmits<{
  (e: 'force-download'): void
}>()

const opened = ref<string | undefined>('osm')

const total = computed(() => props.requirements?.regions.length ?? 0)
const presentCount = computed(
  () => props.requirements?.regions.filter(region => region.present).length ?? 0,
)
const missingCount = computed(() => props.requirements?.missingCount ?? 0)
const tooLargeCount = computed(
  () =>
    props.requirements?.regions.filter(region => region.tooLarge).length ?? 0,
)

function formatSize(sizeMb: number | null | undefined): string {
  if (sizeMb == null) return '—'
  if (sizeMb >= 1024) return `${(sizeMb / 1024).toFixed(2)} Go`
  return `${Math.round(sizeMb)} Mo`
}

const readinessLabel = computed(() => {
  switch (props.requirements?.readiness) {
    case 'disabled':
      return 'désactivé'
    case 'not_configured':
      return 'non configuré'
    case 'needs_regions':
      return 'à préparer'
    case 'no_region':
      return 'hors couverture'
    case 'missing_pbf':
      return 'PBF manquants'
    case 'ready':
      return 'prêt'
    default:
      return '—'
  }
})

const readinessMessage = computed(() => {
  if (props.error) return null
  switch (props.requirements?.readiness) {
    case 'disabled':
      return 'La détection des tunnels est désactivée dans les paramètres.'
    case 'not_configured':
      return "Support OSM non configuré. Ouvrez les paramètres (icône cog) pour choisir le dossier de la clé."
    case 'needs_regions':
      return 'La liste des régions OSM sera téléchargée automatiquement (environ 50 Mo, une seule fois).'
    case 'no_region':
      return (
        props.requirements.message ??
        'Aucune région Geofabrik ne couvre la zone de cette trace.'
      )
    case 'missing_pbf':
      if (
        props.requirements?.regions.some(region => region.tooLarge)
      ) {
        return "Certaines régions dépassent le plafond de téléchargement. Utilisez « Télécharger quand même » ou augmentez Tunnels.Support.tailleMaxPbf."
      }
      return props.pending
        ? null
        : (props.requirements.message ??
            'Des fichiers PBF sont manquants ; la relance les téléchargera.')
    default:
      return null
  }
})

const readinessType = computed<'warning' | 'info'>(() =>
  props.requirements?.readiness === 'needs_regions' ||
  props.requirements?.readiness === 'disabled'
    ? 'info'
    : 'warning',
)

const globalLabel = computed(() => {
  if (!props.pending) return null
  if (!props.preparing) return 'En attente dans la file de préparation…'
  const progress = props.progress
  if (!progress) return 'Préparation des tunnels…'

  const percent =
    progress.bytesTotal > 0 ? ` ${Math.round(progress.percent)} %` : ''
  switch (progress.stage) {
    case 'regions':
      return `Préparation des régions OSM…${percent}`
    case 'pbf':
      return `Téléchargement de ${
        progress.region ?? progress.filename ?? 'PBF'
      }…${percent}`
    case 'detect':
      return 'Analyse des PBF…'
    default:
      return 'Préparation des tunnels…'
  }
})

const globalPercent = computed(() =>
  props.progress && props.progress.bytesTotal > 0 ? props.progress.percent : 0,
)

const globalIndeterminate = computed(
  () =>
    !props.preparing ||
    !props.progress ||
    props.progress.bytesTotal === 0 ||
    props.progress.stage === 'detect',
)

function statusFor(region: TunnelRequirementRegion): {
  label: string
  color: string
  icon: string
  percent: number | null
} {
  if (region.present) {
    return {
      label: 'chargé',
      color: 'success',
      icon: 'mdi-check-circle-outline',
      percent: null,
    }
  }

  if (region.tooLarge) {
    return {
      label: `bloqué (${formatSize(region.sizeMb)})`,
      color: 'error',
      icon: 'mdi-download-off',
      percent: null,
    }
  }

  const progress = props.progress
  if (
    props.preparing &&
    progress &&
    progress.stage === 'pbf' &&
    (progress.filename === region.pbfFilename || progress.region === region.name)
  ) {
    return {
      label:
        progress.bytesTotal > 0
          ? `en cours ${Math.round(progress.percent)} %`
          : 'en cours',
      color: 'info',
      icon: 'mdi-download',
      percent: progress.bytesTotal > 0 ? progress.percent : null,
    }
  }

  if (props.pending) {
    return {
      label: 'en attente',
      color: 'warning',
      icon: 'mdi-clock-outline',
      percent: null,
    }
  }

  return {
    label: 'manquant',
    color: 'warning',
    icon: 'mdi-alert-circle-outline',
    percent: null,
  }
}

watch(
  () => [props.pending, props.error, props.requirements?.readiness] as const,
  () => {
    const needsAttention =
      props.pending ||
      !!props.error ||
      (props.requirements != null && props.requirements.readiness !== 'ready')
    opened.value = needsAttention ? 'osm' : undefined
  },
  { immediate: true },
)
</script>

<style scoped>
.osm-panel {
  flex: 0 0 auto;
}

.osm-panel__title {
  min-height: 40px;
  font-size: 0.8rem;
}

.osm-panel__title-text {
  font-weight: 600;
}

.osm-panel__progress {
  margin-bottom: 8px;
}

.osm-panel__progress-label {
  font-size: 0.72rem;
  color: rgba(var(--v-theme-on-surface), 0.7);
  margin-bottom: 4px;
}

.osm-panel__row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
}

.osm-panel__text {
  flex: 1 1 auto;
  min-width: 0;
}

.osm-panel__name {
  font-size: 0.76rem;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.osm-panel__file {
  font-size: 0.64rem;
  color: rgba(var(--v-theme-on-surface), 0.55);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.osm-panel__status {
  flex: 0 0 auto;
}
</style>
