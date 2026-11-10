<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <HermesToolbar
        :loading="hermesStore.loading"
        :server-state="hermesStore.server"
        @back="goHome"
        @refresh="refresh"
        @open-settings="toggleSettings"
      />

      <HermesPanel
        :selected-region-id="selectedRegionId"
        :selected-polygon-id="selectedPolygonId"
        :polygon-busy="polygonBusy"
        @select-region="onSelectRegion"
        @select-polygon="onSelectPolygon"
        @create-polygon="startCreatePolygon"
        @edit-polygon="startEditPolygon"
        @delete-polygon="requestDeletePolygon"
        @create-dataset="startCreateDataset"
        @delete-dataset="requestDeleteDataset"
      />

      <v-main>
        <HermesMap
          :regions="hermesStore.regions"
          :polygons="hermesStore.polygons"
          :selected-region-id="selectedRegionId"
          :selected-polygon-id="selectedPolygonId"
          :draw-mode="drawMode"
          :editing-polygon="editingPolygon"
          :draw-session="drawSession"
          :preview-region-ids="previewRegionIds"
          :preview-polygon-ids="previewPolygonIds"
          @select-region="onSelectRegion"
          @select-polygon="onSelectPolygon"
          @draw-create="onDrawCreate"
          @draw-update="onDrawUpdate"
        />
      </v-main>

      <SettingsDrawer :show-system="false" />

      <HermesPolygonForm
        :model-value="polygonDialogOpen"
        :mode="polygonFormMode"
        :polygon="editingPolygon"
        :geometry="drawnGeometry"
        :saving="polygonSaving"
        @submit="onPolygonSubmit"
        @cancel="onPolygonCancel"
      />

      <HermesDatasetForm
        :model-value="datasetFormOpen"
        :saving="datasetSaving"
        @submit="onDatasetSubmit"
        @cancel="onDatasetCancel"
        @preview="onDatasetPreview"
      />

      <v-dialog v-model="deleteDialogOpen" max-width="500" persistent>
        <v-card>
          <v-card-title class="text-h6">
            Supprimer le polygone ?
          </v-card-title>
          <v-card-text>
            Le polygone <b>« {{ deleteCandidate?.name ?? '' }} »</b> sera
            supprimé du serveur.
            <v-alert
              v-if="deleteImpact.length > 0"
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-3"
              :text="`Utilisé par les datasets : ${deleteImpact.join(
                ', ',
              )}. Ils seront marqués obsolètes au prochain chargement.`"
            />
          </v-card-text>
          <v-card-actions>
            <v-spacer />
            <v-btn variant="text" :disabled="polygonDeleting" @click="cancelDelete">
              Annuler
            </v-btn>
            <v-btn
              color="error"
              variant="flat"
              :loading="polygonDeleting"
              @click="confirmDeletePolygon"
            >
              Supprimer
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-dialog>

      <v-dialog v-model="deleteDatasetDialogOpen" max-width="500" persistent>
        <v-card>
          <v-card-title class="text-h6">
            Supprimer le dataset ?
          </v-card-title>
          <v-card-text>
            Le dataset <b>« {{ deleteDatasetCandidate?.id ?? '' }} »</b> et
            tout son contenu (graphes, cache, artefacts) seront supprimés du
            serveur. Cette action est irréversible.
            <v-alert
              v-if="
                deleteDatasetCandidate?.id === hermesStore.activeDatasetId
              "
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-3"
              text="C'est le dataset actif : il sera désélectionné."
            />
          </v-card-text>
          <v-card-actions>
            <v-spacer />
            <v-btn
              variant="text"
              :disabled="datasetDeleting"
              @click="cancelDeleteDataset"
            >
              Annuler
            </v-btn>
            <v-btn
              color="error"
              variant="flat"
              :loading="datasetDeleting"
              @click="confirmDeleteDataset"
            >
              Supprimer
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-dialog>
    </v-layout>
  </v-app>
</template>

<script setup lang="ts">
/**
 * Vue Hermes : serveur, datasets, régions et polygones custom.
 *
 * Le panneau gauche et la carte partagent la même sélection. La carte porte
 * l'outil Mapbox Draw : la création dessine un polygone puis ouvre le
 * formulaire ; l'édition charge le polygone dans l'outil et permet de déplacer
 * ses sommets. La suppression demande confirmation et signale les datasets
 * qui utilisent la zone.
 */
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import {
  useHermesStore,
  type HermesCreateDatasetPayload,
  type HermesDataset,
  type HermesPolygon,
  type HermesPolygonInput,
} from '../stores/hermes'
import { useUiStore } from '../stores/ui'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'
import HermesDatasetForm from '../components/Hermes/HermesDatasetForm.vue'
import HermesMap from '../components/Hermes/HermesMap.vue'
import HermesPanel from '../components/Hermes/HermesPanel.vue'
import HermesPolygonForm from '../components/Hermes/HermesPolygonForm.vue'
import HermesToolbar from '../components/Hermes/HermesToolbar.vue'
import { parseHermesError, type HermesDrawMode } from '../utils/hermes'

const router = useRouter()
const appStore = useAppStore()
const hermesStore = useHermesStore()
const ui = useUiStore()

/** Région sélectionnée (contour ou bbox), ou `null`. */
const selectedRegionId = ref<string | null>(null)
/** Polygone custom sélectionné, ou `null`. */
const selectedPolygonId = ref<string | null>(null)
/** Mode de l'outil de dessin Mapbox. */
const drawMode = ref<HermesDrawMode>('idle')
/** Compteur de session de dessin (ré-applique l'outil après une annulation). */
const drawSession = ref(0)
/** Polygone en cours d'édition. */
const editingPolygon = ref<HermesPolygon | null>(null)
/** Géométrie dessinée ou éditée, mise à jour à chaque `draw.update`. */
const drawnGeometry = ref<GeoJSON.Polygon | null>(null)
const polygonDialogOpen = ref(false)
const polygonFormMode = ref<'create' | 'edit'>('create')
const polygonSaving = ref(false)
const polygonDeleting = ref(false)
const deleteDialogOpen = ref(false)
const deleteCandidate = ref<HermesPolygon | null>(null)
/** IDs surlignés par le formulaire de dataset (sélection multiple). */
const previewRegionIds = ref<string[]>([])
const previewPolygonIds = ref<string[]>([])
const datasetFormOpen = ref(false)
const datasetSaving = ref(false)
const deleteDatasetDialogOpen = ref(false)
const deleteDatasetCandidate = ref<HermesDataset | null>(null)
const datasetDeleting = ref(false)

/** Une action polygone bloque les boutons du panneau et ceux du formulaire. */
const polygonBusy = computed(
  () =>
    polygonSaving.value || polygonDeleting.value || polygonDialogOpen.value,
)

/** Datasets qui référencent le polygone candidat à la suppression. */
const deleteImpact = computed(() => {
  const polygon = deleteCandidate.value
  if (!polygon) return []
  return hermesStore.datasets
    .filter(dataset => dataset.zones?.includes(polygon.id))
    .map(dataset => dataset.id)
})

function goHome(): void {
  void router.push({ name: 'accueil' })
}

function toggleSettings(): void {
  appStore.isSettingsDrawerOpen = !appStore.isSettingsDrawerOpen
}

async function refresh(): Promise<void> {
  await Promise.all([
    hermesStore.refreshServer(),
    hermesStore.loadRegions(),
    hermesStore.loadPolygons(),
  ])
}

function onSelectRegion(id: string | null): void {
  selectedRegionId.value = id
  if (id) selectedPolygonId.value = null
}

function onSelectPolygon(id: string | null): void {
  selectedPolygonId.value = id
  if (id) selectedRegionId.value = null
}

// ─── Création / édition ─────────────────────────────────────────────

function startCreatePolygon(): void {
  selectedRegionId.value = null
  selectedPolygonId.value = null
  editingPolygon.value = null
  drawnGeometry.value = null
  polygonFormMode.value = 'create'
  polygonSaving.value = false
  drawSession.value += 1
  drawMode.value = 'create'
}

function onDrawCreate(geometry: GeoJSON.Polygon): void {
  drawnGeometry.value = geometry
  polygonDialogOpen.value = true
}

function onDrawUpdate(geometry: GeoJSON.Polygon): void {
  if (polygonDialogOpen.value) {
    drawnGeometry.value = geometry
  }
}

function startEditPolygon(polygon: HermesPolygon): void {
  selectedRegionId.value = null
  selectedPolygonId.value = polygon.id
  editingPolygon.value = polygon
  drawnGeometry.value = polygon.geometry
  polygonFormMode.value = 'edit'
  polygonSaving.value = false
  polygonDialogOpen.value = true
  drawSession.value += 1
  drawMode.value = 'edit'
}

function onPolygonCancel(): void {
  polygonDialogOpen.value = false
  polygonSaving.value = false
  drawMode.value = 'idle'
  drawnGeometry.value = null
  editingPolygon.value = null
}

async function onPolygonSubmit(payload: HermesPolygonInput): Promise<void> {
  polygonSaving.value = true
  try {
    let saved: HermesPolygon
    if (polygonFormMode.value === 'edit' && editingPolygon.value) {
      saved = await hermesStore.updatePolygon(editingPolygon.value.id, {
        name: payload.name,
        description: payload.description ?? null,
        color: payload.color ?? null,
        geometry: payload.geometry,
      })
      ui.showSuccess(`Polygone « ${saved.name} » enregistré.`)
    } else {
      saved = await hermesStore.createPolygon({
        id: payload.id,
        name: payload.name,
        description: payload.description ?? null,
        color: payload.color ?? null,
        geometry: payload.geometry,
      })
      ui.showSuccess(`Polygone « ${saved.name} » créé.`)
    }

    selectedRegionId.value = null
    selectedPolygonId.value = saved.id
    polygonDialogOpen.value = false
    drawMode.value = 'idle'
    drawnGeometry.value = null
    editingPolygon.value = null
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  } finally {
    polygonSaving.value = false
  }
}

// ─── Suppression ────────────────────────────────────────────────────

function requestDeletePolygon(polygon: HermesPolygon): void {
  deleteCandidate.value = polygon
  deleteDialogOpen.value = true
}

function cancelDelete(): void {
  deleteDialogOpen.value = false
  deleteCandidate.value = null
}

async function confirmDeletePolygon(): Promise<void> {
  const polygon = deleteCandidate.value
  if (!polygon) return
  polygonDeleting.value = true
  try {
    await hermesStore.deletePolygon(polygon.id)
    if (selectedPolygonId.value === polygon.id) {
      selectedPolygonId.value = null
    }
    ui.showSuccess(`Polygone « ${polygon.name} » supprimé.`)
    deleteDialogOpen.value = false
    deleteCandidate.value = null
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  } finally {
    polygonDeleting.value = false
  }
}

// ─── Datasets ───────────────────────────────────────────────────────

function startCreateDataset(): void {
  datasetFormOpen.value = true
}

function onDatasetPreview(payload: {
  regionIds: string[]
  polygonIds: string[]
}): void {
  previewRegionIds.value = payload.regionIds
  previewPolygonIds.value = payload.polygonIds
}

function clearDatasetPreview(): void {
  previewRegionIds.value = []
  previewPolygonIds.value = []
}

function onDatasetCancel(): void {
  datasetFormOpen.value = false
  clearDatasetPreview()
}

async function onDatasetSubmit(
  payload: HermesCreateDatasetPayload,
): Promise<void> {
  datasetSaving.value = true
  try {
    const result = await hermesStore.createDataset(payload)
    ui.showSuccess(`Dataset ${result.id} en cours de préparation.`)
    datasetFormOpen.value = false
    clearDatasetPreview()
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  } finally {
    datasetSaving.value = false
  }
}

function requestDeleteDataset(dataset: HermesDataset): void {
  deleteDatasetCandidate.value = dataset
  deleteDatasetDialogOpen.value = true
}

function cancelDeleteDataset(): void {
  deleteDatasetDialogOpen.value = false
  deleteDatasetCandidate.value = null
}

async function confirmDeleteDataset(): Promise<void> {
  const dataset = deleteDatasetCandidate.value
  if (!dataset) return
  datasetDeleting.value = true
  try {
    await hermesStore.deleteDataset(dataset.id)
    ui.showSuccess(`Dataset ${dataset.id} supprimé.`)
    deleteDatasetDialogOpen.value = false
    deleteDatasetCandidate.value = null
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  } finally {
    datasetDeleting.value = false
  }
}

onMounted(() => {
  void refresh()
})

onUnmounted(() => {
  drawMode.value = 'idle'
  hermesStore.cancelPolling()
})
</script>
