<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  useOsmStore,
  type OsmFileStatus,
  type OsmProgress,
} from '../../stores/osm'
import { useUiStore } from '../../stores/ui'
import { formatFileSize } from '../../utils/format'
import { fileStateColor, fileStateLabel } from '../../utils/osmFiles'
import OsmDeleteDialog from './OsmDeleteDialog.vue'
import OsmFileRow from './OsmFileRow.vue'
import OsmRegionDownloadPanel from './OsmRegionDownloadPanel.vue'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'open-settings'): void
}>()

const osmStore = useOsmStore()
const ui = useUiStore()

const deleteTarget = ref<OsmFileStatus | null>(null)

const files = computed<OsmFileStatus[]>(() => osmStore.overview?.files ?? [])
const totalSize = computed(() => osmStore.overview?.totalSizeBytes ?? 0)
const overviewState = computed(() => osmStore.overview?.state ?? '')
const supportUnavailable = computed(() => {
  const overview = osmStore.overview
  return (
    !overview ||
    overview.state === 'not_configured' ||
    overview.state === 'usb_missing'
  )
})
const overviewLabel = computed(() =>
  overviewState.value === 'empty'
    ? 'Aucun fichier'
    : fileStateLabel(overviewState.value),
)

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function close(): void {
  deleteTarget.value = null
  osmStore.clearPreview()
  osmStore.clearPresentRegions()
  emit('update:modelValue', false)
}

function onOpenSettings(): void {
  close()
  emit('open-settings')
}

async function refreshAll(): Promise<void> {
  await Promise.all([osmStore.refreshOverview(), osmStore.loadCatalog()])
}

function isRenewing(file: OsmFileStatus): boolean {
  if (!osmStore.downloading) return false
  if (file.regionId) {
    return (
      osmStore.downloadingRegionId === file.regionId ||
      osmStore.progress?.regionId === file.regionId
    )
  }
  return osmStore.progress?.filename === file.filename
}

function progressFor(file: OsmFileStatus): OsmProgress | null {
  return osmStore.progress?.filename === file.filename
    ? osmStore.progress
    : null
}

async function onRenew(file: OsmFileStatus): Promise<void> {
  if (!file.regionId) return
  try {
    const result = await osmStore.downloadRegion(file.regionId, true)
    ui.showSuccess(
      `${file.regionName ?? file.filename} renouvelé (${formatFileSize(
        result.sizeBytes,
      )}).`,
    )
  } catch (error) {
    ui.showError(errorMessage(error))
  }
}

function onDelete(file: OsmFileStatus): void {
  deleteTarget.value = file
}

async function confirmDelete(): Promise<void> {
  const target = deleteTarget.value
  if (!target) return
  try {
    await osmStore.deleteFile(target.filename)
    ui.showSuccess(`${target.regionName ?? target.filename} supprimé.`)
    deleteTarget.value = null
  } catch (error) {
    ui.showError(errorMessage(error))
  }
}

watch(
  () => props.modelValue,
  open => {
    if (!open) {
      deleteTarget.value = null
      osmStore.clearPreview()
      osmStore.clearPresentRegions()
      return
    }
    void osmStore.refreshOverview()
    void osmStore.loadCatalog()
    void osmStore.loadPresentRegions()
  },
  { immediate: true },
)

watch(
  () =>
    osmStore.overview?.files
      .map(file => `${file.filename}:${file.state}`)
      .join('|') ?? '',
  () => {
    if (props.modelValue) void osmStore.loadPresentRegions()
  },
)
</script>

<template>
  <v-dialog
    :model-value="props.modelValue"
    max-width="1120"
    :scrim="false"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card
      class="osm-dialog"
      style="background: rgba(var(--v-theme-surface), 0.5);"
    >
      <v-card-title class="d-flex align-center px-5 py-3">
        <v-icon icon="mdi-map-legend" class="mr-2" />
        <span class="text-h6">Données OSM</span>
        <v-chip
          v-if="overviewState && !supportUnavailable"
          :color="fileStateColor(overviewState)"
          size="x-small"
          variant="tonal"
          class="ml-3"
        >
          {{ overviewLabel }}
        </v-chip>
        <v-spacer />
        <v-btn
          icon="mdi-close"
          variant="text"
          density="comfortable"
          title="Fermer"
          @click="close"
        />
      </v-card-title>
      <v-divider />

      <div
        v-if="osmStore.overview?.folder"
        class="osm-dialog__folder px-5 py-2"
      >
        <v-icon icon="mdi-folder-open" size="16" class="mr-1" />
        <span class="text-caption osm-dialog__path">
          {{ osmStore.overview?.folder }}
        </span>
        <v-spacer />
        <span class="text-caption text-medium-emphasis">
          Espace libre : {{ formatFileSize(osmStore.overview?.freeBytes) }}
          <template v-if="osmStore.overview?.totalBytes != null">
            / {{ formatFileSize(osmStore.overview?.totalBytes) }}
          </template>
        </span>
      </div>
      <v-divider v-if="osmStore.overview?.folder" />

      <v-card-text class="osm-dialog__body pa-0">
        <v-alert
          v-if="!osmStore.overview && osmStore.lastError"
          type="error"
          variant="tonal"
          class="ma-4"
          :text="osmStore.lastError"
        >
          <template #append>
            <v-btn
              size="small"
              variant="flat"
              color="error"
              @click="refreshAll"
            >
              Réessayer
            </v-btn>
          </template>
        </v-alert>

        <div v-else-if="!osmStore.overview" class="osm-dialog__loading">
          <v-progress-circular indeterminate color="primary" />
          <span class="text-body-2 mt-3">Chargement des données OSM…</span>
        </div>

        <v-alert
          v-else-if="supportUnavailable"
          :type="
            osmStore.overview?.state === 'usb_missing' ? 'warning' : 'info'
          "
          variant="tonal"
          class="ma-4"
          :text="osmStore.overview?.message ?? 'Support OSM indisponible.'"
        >
          <template #append>
            <v-btn
              size="small"
              variant="flat"
              :color="
                osmStore.overview?.state === 'usb_missing'
                  ? 'warning'
                  : 'info'
              "
              @click="onOpenSettings"
            >
              Choisir un dossier
            </v-btn>
          </template>
        </v-alert>

        <v-row v-else class="osm-dialog__columns ma-0">
          <v-col cols="12" md="7" class="osm-dialog__left pa-4">
            <div class="d-flex align-center mb-2">
              <div class="text-subtitle-1">Fichiers présents</div>
              <v-chip
                size="x-small"
                variant="tonal"
                color="grey"
                class="ml-2"
              >
                {{ files.length }}
              </v-chip>
              <v-chip
                v-if="
                  osmStore.presentRegions.some(region => region.approximate)
                "
                size="x-small"
                variant="tonal"
                color="warning"
                class="ml-2"
              >
                géométries approximatives
              </v-chip>
              <v-spacer />
              <span class="text-caption text-medium-emphasis">
                {{ formatFileSize(totalSize) }}
              </span>
            </div>
            <v-alert
              v-if="files.length === 0"
              type="info"
              variant="tonal"
              density="compact"
              text="Aucun fichier PBF présent. Téléchargez une région dans la zone de droite."
            />
            <div v-else class="osm-dialog__file-list">
              <template v-for="(file, index) in files" :key="file.filename">
                <OsmFileRow
                  :file="file"
                  :renewing="isRenewing(file)"
                  :progress="progressFor(file)"
                  :disabled="osmStore.isBusy"
                  @renew="onRenew(file)"
                  @delete="onDelete(file)"
                />
                <v-divider v-if="index < files.length - 1" />
              </template>
            </div>
          </v-col>
          <v-col cols="12" md="5" class="osm-dialog__right pa-4">
            <OsmRegionDownloadPanel />
          </v-col>
        </v-row>
      </v-card-text>

      <v-divider />
      <v-card-actions class="px-5">
        <v-btn
          variant="text"
          prepend-icon="mdi-refresh"
          :disabled="osmStore.isBusy"
          @click="refreshAll"
        >
          Actualiser
        </v-btn>
        <v-spacer />
        <v-btn variant="text" @click="close">Fermer</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>

  <OsmDeleteDialog
    :model-value="deleteTarget !== null"
    :filename="deleteTarget?.filename ?? ''"
    :region-name="deleteTarget?.regionName ?? null"
    @update:model-value="deleteTarget = null"
    @confirm="confirmDelete"
  />
</template>

<style scoped>
.osm-dialog {
  display: flex;
  flex-direction: column;
  max-height: 90vh;
}

.osm-dialog__folder {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}

.osm-dialog__path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.osm-dialog__body {
  flex: 1 1 auto;
  min-height: 0;
  height: 68vh;
  display: flex;
  flex-direction: column;
}

.osm-dialog__loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 40vh;
}

.osm-dialog__columns {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}

.osm-dialog__left,
.osm-dialog__right {
  min-height: 0;
}

.osm-dialog__right {
  background: rgba(var(--v-theme-surface-variant), 0.35);
  border-inline-start: 1px solid
    rgba(var(--v-border-color), var(--v-border-opacity));
}

.osm-dialog__file-list {
  display: flex;
  flex-direction: column;
}

@media (min-width: 960px) {
  .osm-dialog__columns {
    overflow-y: hidden;
  }

  .osm-dialog__left,
  .osm-dialog__right {
    height: 100%;
    overflow-y: auto;
  }
}
</style>
