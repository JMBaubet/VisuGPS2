<script setup lang="ts">
import { computed } from 'vue'
import type { OsmFileStatus, OsmProgress } from '../../stores/osm'
import { formatDate, formatFileSize } from '../../utils/format'
import {
  daysLabel,
  fileStateColor,
  fileStateLabel,
} from '../../utils/osmFiles'
import OsmFlag from './OsmFlag.vue'

const props = defineProps<{
  file: OsmFileStatus
  renewing: boolean
  progress: OsmProgress | null
  disabled: boolean
}>()

const emit = defineEmits<{
  (e: 'renew'): void
  (e: 'delete'): void
}>()

const dateLabel = computed(() => {
  if (!props.file.createdAt) return 'Date inconnue'
  const prefix =
    props.file.dateSource === 'pbf'
      ? 'Généré le'
      : props.file.dateSource === 'modified'
        ? 'Modifié le'
        : 'Créé le'
  return `${prefix} ${formatDate(props.file.createdAt)}`
})

const daysClass = computed(() =>
  props.file.daysRemaining < 0 ? 'text-warning' : 'text-medium-emphasis',
)

const stateTooltip = computed(() => {
  if (props.file.state === 'orange') {
    const days = props.file.validityDaysRemaining ?? 0
    return days >= 0
      ? `Fin de vie : ${days} j restants.`
      : `Validité maximale dépassée de ${Math.abs(days)} j.`
  }
  if (props.file.state === 'yellow') {
    const origin =
      props.file.dateSource === 'pbf'
        ? 'la génération'
        : props.file.dateSource === 'modified'
          ? 'la modification'
          : 'la création'
    return `Renouvellement dû (${props.file.renewalDays} j depuis ${origin}).`
  }
  return `À jour (renouvellement dans ${props.file.daysRemaining} j).`
})

const renewTitle = computed(() =>
  props.file.regionId
    ? 'Renouveler ce fichier'
    : 'Région inconnue : mettez à jour le catalogue pour pouvoir la renouveler.',
)

const progressPercent = computed(() =>
  props.progress && props.progress.bytesTotal > 0
    ? props.progress.percent
    : 0,
)
</script>

<template>
  <div class="osm-file-row">
    <OsmFlag
      :code="file.countryCode"
      :name="file.countryName"
      class="osm-file-row__flag"
    />
    <div class="osm-file-row__main">
      <div
        class="osm-file-row__title"
        :title="file.regionName ?? file.filename"
      >
        {{ file.regionName ?? file.filename }}
      </div>
      <div class="osm-file-row__subtitle" :title="file.filename">
        {{ file.continentName ?? 'Continent inconnu' }} ·
        {{ file.countryName ?? 'Pays inconnu' }} · {{ file.filename }}
      </div>
      <div class="osm-file-row__meta">
        {{ formatFileSize(file.sizeBytes) }} · {{ dateLabel }} ·
        <span :class="daysClass">{{ daysLabel(file) }}</span>
      </div>
      <v-progress-linear
        v-if="renewing"
        :model-value="progressPercent"
        :indeterminate="!progress || progress.bytesTotal === 0"
        color="info"
        height="3"
        class="mt-1"
      />
    </div>
    <v-chip
      :color="fileStateColor(file.state)"
      size="x-small"
      variant="tonal"
      :title="stateTooltip"
    >
      {{ fileStateLabel(file.state) }}
    </v-chip>
    <v-btn
      icon="mdi-refresh"
      variant="text"
      density="comfortable"
      size="small"
      :disabled="disabled || renewing || !file.regionId"
      :title="renewTitle"
      @click="emit('renew')"
    />
    <v-btn
      icon="mdi-delete-outline"
      variant="text"
      density="comfortable"
      size="small"
      :disabled="disabled || renewing"
      title="Supprimer ce fichier"
      @click="emit('delete')"
    />
  </div>
</template>

<style scoped>
.osm-file-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 4px;
}

.osm-file-row__flag {
  flex: 0 0 26px;
}

.osm-file-row__main {
  flex: 1 1 auto;
  min-width: 0;
}

.osm-file-row__title {
  font-size: 0.82rem;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.osm-file-row__subtitle {
  font-size: 0.68rem;
  color: rgba(var(--v-theme-on-surface), 0.6);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.osm-file-row__meta {
  font-size: 0.68rem;
  color: rgba(var(--v-theme-on-surface), 0.7);
  margin-top: 2px;
}
</style>
