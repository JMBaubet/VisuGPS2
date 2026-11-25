<template>
  <v-list-item
    @mouseenter="emit('hover-dataset', dataset.id)"
    @mouseleave="emit('hover-dataset', null)"
  >
    <template #prepend>
      <v-tooltip v-if="interactive" location="right" max-width="300">
        <template #activator="{ props: activatorProps }">
          <span v-bind="activatorProps" class="hermes-select-icon">
            <v-btn
              icon
              size="x-small"
              variant="text"
              :disabled="isUnavailable(dataset)"
              @click.stop="onSelectIcon"
            >
              <v-icon
                :icon="selectionIcon"
                :color="selectionColor"
                size="18"
              />
            </v-btn>
          </span>
        </template>
        <span>{{ selectionHint }}</span>
      </v-tooltip>
      <v-icon
        v-else
        icon="mdi-check-circle"
        :color="datasetStateColor(dataset)"
        size="18"
      />
    </template>

    <v-list-item-title class="d-flex align-center flex-wrap">
      {{ datasetDisplayName(dataset) }}
      <v-chip
        size="x-small"
        variant="tonal"
        :color="datasetStateColor(dataset)"
        class="ml-2"
      >
        {{ datasetStateLabel(dataset) }}
      </v-chip>
      <v-chip
        v-if="dataset.stale"
        size="x-small"
        variant="tonal"
        color="warning"
        class="ml-1"
      >
        obsolète
      </v-chip>
      <v-chip
        v-if="needsUpdate"
        size="x-small"
        variant="tonal"
        color="deep-orange"
        class="ml-1"
        title="Un secteur utilisé a été modifié depuis la construction."
      >
        à mettre à jour
      </v-chip>
      <v-progress-circular
        v-if="hermesStore.pollingDatasetId === dataset.id"
        indeterminate
        size="12"
        class="ml-2"
      />
    </v-list-item-title>

    <v-list-item-subtitle>{{ scope }}</v-list-item-subtitle>

    <v-list-item-subtitle
      v-if="auxIssues.length > 0 || appStore.isDebug"
      class="d-flex align-center flex-wrap ga-1 mt-1"
    >
      <v-chip
        v-for="issue in auxIssues"
        :key="issue"
        size="x-small"
        variant="tonal"
        color="orange"
      >
        {{ issue }}
      </v-chip>
      <span v-if="appStore.isDebug" class="text-caption text-medium-emphasis">
        {{ formatFileSize(dataset.sizeBytes) }} · ORS
        {{ dataset.orsVersion || '—' }}
      </span>
    </v-list-item-subtitle>

    <template #append>
      <div class="d-flex align-center ga-1">
        <v-tooltip v-if="needsUpdate" location="left" max-width="320">
          <template #activator="{ props }">
            <v-btn
              v-bind="props"
              icon="mdi-database-refresh"
              size="small"
              variant="text"
              color="deep-orange"
              title="Mettre à jour le jeu de données"
              :loading="hermesStore.rebuildingDatasetId === dataset.id"
              :disabled="
                hermesStore.rebuildingDatasetId !== null ||
                hermesStore.constructionInProgress
              "
              @click.stop="emit('update-dataset', dataset)"
            />
          </template>
          <span>
            Supprimer puis reconstruire ce jeu de données avec les secteurs
            actuels (service interrompu pendant l'opération).
          </span>
        </v-tooltip>

        <v-tooltip v-if="auxIssues.length > 0" location="left" max-width="320">
          <template #activator="{ props }">
            <v-btn
              v-bind="props"
              icon="mdi-hammer-wrench"
              size="small"
              variant="text"
              color="orange"
              title="Relancer la préparation des aux"
              :loading="hermesStore.auxPreparingDatasetId === dataset.id"
              :disabled="
                hermesStore.constructionInProgress ||
                !isDatasetGraphReady(dataset)
              "
              @click.stop="onReprepareAux"
            />
          </template>
          <span>
            Relancer la préparation des aux de ce jeu de données (force=true).
          </span>
        </v-tooltip>

        <v-tooltip v-if="dataset.lastLogTail" location="left" max-width="420">
          <template #activator="{ props }">
            <v-icon
              v-bind="props"
              icon="mdi-alert-circle-outline"
              color="error"
              size="18"
            />
          </template>
          <div class="hermes-log-tail">{{ dataset.lastLogTail }}</div>
        </v-tooltip>

        <v-btn
          v-if="showDelete"
          icon="mdi-delete-outline"
          size="small"
          variant="text"
          color="error"
          :title="`Supprimer ${datasetDisplayName(dataset)}`"
          @click.stop="emit('delete-dataset', dataset)"
        />
      </div>
    </template>
  </v-list-item>
</template>

<script setup lang="ts">
/**
 * Ligne d'un jeu de données, partagée par l'onglet « Jeu de données » et la
 * carte Serveur (dataset servi).
 *
 * La liste rend l'icône de tête interactive (sélection après confirmation) ;
 * la carte affiche une coche statique colorée comme le chip d'état. Seule la
 * liste affiche la poubelle.
 */
import { computed } from 'vue'
import { useAppStore } from '../../stores/app'
import { useHermesStore, type HermesDataset } from '../../stores/hermes'
import { formatFileSize } from '../../utils/format'
import {
  datasetAuxIssues,
  datasetDisplayName,
  datasetNeedsUpdate,
  datasetSectorNames,
  datasetStateColor,
  datasetStateLabel,
  isDatasetGraphReady,
} from '../../utils/hermes'

const props = defineProps<{
  dataset: HermesDataset
  /** Dataset servi. */
  active: boolean
  /** L'icône de tête permet de sélectionner le dataset (liste). */
  interactive: boolean
  /** Affiche l'action de suppression (liste uniquement). */
  showDelete: boolean
}>()

const emit = defineEmits<{
  'select-dataset': [dataset: HermesDataset]
  'update-dataset': [dataset: HermesDataset]
  'delete-dataset': [dataset: HermesDataset]
  'hover-dataset': [id: string | null]
}>()

const hermesStore = useHermesStore()
const appStore = useAppStore()

const auxIssues = computed(() => datasetAuxIssues(props.dataset))
const needsUpdate = computed(() =>
  datasetNeedsUpdate(props.dataset, hermesStore.polygons),
)

/** Périmètre affiché : noms des secteurs, ou régions pour les datasets legacy. */
const scope = computed(() => {
  if (props.dataset.zones && props.dataset.zones.length > 0) {
    return `secteurs : ${datasetSectorNames(
      props.dataset,
      hermesStore.polygons,
    ).join(', ')}`
  }
  if (props.dataset.regions && props.dataset.regions.length > 0) {
    return `régions : ${props.dataset.regions.join(', ')}`
  }
  return 'périmètre inconnu'
})

/** Dataset non servable : graphe absent (construction en échec ou indisponible). */
function isUnavailable(dataset: HermesDataset): boolean {
  return dataset.state === 'unavailable' || dataset.status === 'failed'
}

const selectionIcon = computed(() => {
  if (props.active) return 'mdi-check-circle'
  return isUnavailable(props.dataset)
    ? 'mdi-lock-outline'
    : 'mdi-play-circle-outline'
})

const selectionColor = computed<string | undefined>(() => {
  if (props.active) return datasetStateColor(props.dataset)
  return isUnavailable(props.dataset) ? undefined : 'primary'
})

const selectionHint = computed(() => {
  if (props.active) return 'Jeu de données servi.'
  if (isUnavailable(props.dataset)) {
    return 'Jeu de données indisponible : graphe absent ou construction en échec.'
  }
  return `Sélectionner et servir « ${datasetDisplayName(props.dataset)} ».`
})

function onSelectIcon(): void {
  if (
    props.active ||
    isUnavailable(props.dataset) ||
    hermesStore.servingBusy ||
    hermesStore.rebuildingDatasetId !== null
  ) {
    return
  }
  emit('select-dataset', props.dataset)
}

async function onReprepareAux(): Promise<void> {
  await hermesStore.prepareAux(props.dataset.id, true)
}
</script>

<style scoped>
.hermes-select-icon {
  display: inline-flex;
}

.hermes-log-tail {
  font-size: 0.75rem;
  line-height: 1.35;
  white-space: pre-wrap;
  max-height: 240px;
  overflow-y: auto;
}
</style>
