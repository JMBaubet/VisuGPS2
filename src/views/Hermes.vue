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
        :tab="activeTab"
        :selected-polygon-id="selectedPolygonId"
        :polygon-busy="polygonBusy"
        @update:tab="onTabUpdate"
        @select-polygon="onSelectPolygon"
        @create-polygon="startCreatePolygon"
        @edit-polygon="startEditPolygon"
        @delete-polygon="requestDeletePolygon"
        @create-dataset="startCreateDataset"
        @delete-dataset="requestDeleteDataset"
        @select-dataset="requestSelectDataset"
        @update-dataset="requestUpdateDataset"
        @hover-dataset="hoveredDatasetId = $event"
        @hover-polygon="hoveredPolygonId = $event"
      />

      <v-main>
        <HermesMap
          :regions="hermesStore.regions"
          :polygons="hermesStore.polygons"
          :datasets="hermesStore.datasets"
          :active-dataset-id="hermesStore.activeDatasetId"
          :active-tab="activeTab"
          :selected-polygon-id="selectedPolygonId"
          :hovered-dataset-id="hoveredDatasetId"
          :hovered-polygon-id="hoveredPolygonId"
          :preview-dataset-id="previewDatasetId"
          :draw-mode="drawMode"
          :editing-polygon="editingPolygon"
          :draw-session="drawSession"
          :preview-polygon-ids="previewPolygonIds"
          @select-polygon="onSelectPolygon"
          @draw-create="onDrawCreate"
          @draw-update="onDrawUpdate"
          @draw-draft="onDrawDraft"
          @draw-clear="onDrawClear"
        />
      </v-main>

      <SettingsDrawer :show-system="false" />

      <HermesPolygonForm
        :model-value="polygonFormOpen"
        :mode="polygonFormMode"
        :polygon="editingPolygon"
        :geometry="drawnGeometry"
        :draft-geometry="draftGeometry"
        :saving="polygonSaving"
        @submit="onPolygonSubmit"
        @cancel="onPolygonCancel"
        @redraw="onPolygonRedraw"
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
            Supprimer le secteur ?
          </v-card-title>
          <v-card-text>
            Le secteur <b>« {{ deleteCandidate?.name ?? '' }} »</b> sera
            supprimé du serveur.
            <v-alert
              v-if="deleteImpact.length > 0"
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-3"
              :text="`Utilisé par les jeux de données : ${deleteImpact.join(
                ', ',
              )}.`"
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
            Supprimer le jeu de données ?
          </v-card-title>
          <v-card-text>
            Le jeu de données
            <b>« {{ deleteDatasetCandidate?.id ?? '' }} »</b> et tout son
            contenu (graphes, cache, artefacts) seront supprimés du serveur.
            Cette action est irréversible.
            <v-alert
              v-if="
                deleteDatasetCandidate?.id === hermesStore.activeDatasetId
              "
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-3"
              text="C'est le jeu de données servi : ORS sera arrêté et la sélection sera conservée côté serveur (le temps de recréer un jeu de données du même id)."
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

      <v-dialog v-model="selectDatasetDialogOpen" max-width="520" persistent>
        <v-card class="hermes-confirm-card">
          <v-card-title class="text-h6">
            Servir ce jeu de données ?
          </v-card-title>
          <v-card-text>
            Le jeu de données
            <b>« {{ selectDatasetCandidate ? displayName(selectDatasetCandidate) : '' }} »</b>
            sera sélectionné et ORS démarrera. La carte cadrera son périmètre
            complet.
            <div v-if="selectDatasetSectors" class="text-caption mt-2">
              Secteurs : {{ selectDatasetSectors }}
            </div>
            <v-alert
              v-if="selectDatasetNeedsUpdate"
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-3"
              text="Un secteur utilisé a été modifié depuis la construction : ce jeu de données est à mettre à jour."
            />
            <v-alert
              v-else-if="selectDatasetCandidate?.stale"
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-3"
              text="Ce jeu de données est marqué obsolète : une région source a changé depuis sa construction."
            />
            <v-alert
              v-if="selectDatasetAuxIssues.length > 0"
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-3"
              :text="`Aux dégradés : ${selectDatasetAuxIssues.join(', ')}.`"
            />
          </v-card-text>
          <v-card-actions>
            <v-spacer />
            <v-btn
              variant="text"
              :disabled="datasetSelecting"
              @click="cancelSelectDataset"
            >
              Annuler
            </v-btn>
            <v-btn
              color="success"
              variant="flat"
              :loading="datasetSelecting"
              @click="confirmSelectDataset"
            >
              Sélectionner
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-dialog>

      <v-dialog v-model="updateDatasetDialogOpen" max-width="560" persistent>
        <v-card class="hermes-confirm-card">
          <v-card-title class="text-h6">
            Mettre à jour le jeu de données ?
          </v-card-title>
          <v-card-text>
            Le jeu de données
            <b>« {{ updateDatasetCandidate ? displayName(updateDatasetCandidate) : '' }} »</b>
            sera supprimé puis reconstruit à partir des secteurs actuels.
            <v-alert
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-3"
              :text="
                updateDatasetIsServed
                  ? 'Ce jeu de données est actuellement servi : le service sera interrompu pendant la reconstruction (ORS arrêté), puis redémarré automatiquement à la fin du build. L’opération peut prendre plusieurs minutes.'
                  : 'Le service sera interrompu pendant la reconstruction : ORS est arrêté, le jeu de données est reconstruit, puis le service reprend. L’opération peut prendre plusieurs minutes.'
              "
            />
            <div v-if="updateDatasetSectors" class="text-caption mt-2">
              Secteurs : {{ updateDatasetSectors }}
            </div>
          </v-card-text>
          <v-card-actions>
            <v-spacer />
            <v-btn
              variant="text"
              :disabled="datasetRebuilding"
              @click="cancelUpdateDataset"
            >
              Annuler
            </v-btn>
            <v-btn
              color="warning"
              variant="flat"
              :loading="datasetRebuilding"
              @click="confirmUpdateDataset"
            >
              Mettre à jour
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-dialog>

      <!-- Alertes globales rendues uniquement dans la vue Hermes. -->
      <v-dialog v-model="hermesStore.systemAlertOpen" max-width="560">
        <v-card>
          <v-card-title class="d-flex align-center">
            <v-icon icon="mdi-server-off" color="red" class="mr-2" />
            Serveur hermes injoignable
          </v-card-title>
          <v-card-text>
            <p class="mb-3">
              Le ping système du serveur hermes ne répond pas. Cible :
              <code>{{ hermesStore.baseUrl }}</code>
            </p>
            <v-alert
              type="warning"
              variant="tonal"
              density="compact"
              class="mb-3"
              text="Vérifiez d'abord le service Tailscale sur ce poste."
            />
            <ul class="hermes-alert-list">
              <li>Le service <b>Tailscale</b> est-il démarré et connecté ?</li>
              <li>La connexion Internet fonctionne-t-elle ?</li>
              <li>
                Le serveur est peut-être en veille : cliquez sur « Réveiller ».
              </li>
              <li>
                Vous n'avez pas la main sur le serveur : si le problème
                persiste, contactez l'administrateur.
              </li>
            </ul>
          </v-card-text>
          <v-card-actions>
            <v-spacer />
            <v-btn variant="text" @click="hermesStore.dismissSystemAlert()">
              Fermer
            </v-btn>
            <v-btn
              color="primary"
              variant="flat"
              :loading="hermesStore.server === 'waking'"
              :disabled="hermesStore.server === 'waking'"
              @click="wakeFromAlert"
            >
              Réveiller
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-dialog>

      <v-dialog v-model="hermesStore.serviceAlertOpen" max-width="520">
        <v-card>
          <v-card-title class="d-flex align-center">
            <v-icon icon="mdi-alert-circle-outline" color="orange" class="mr-2" />
            Service hermes arrêté
          </v-card-title>
          <v-card-text>
            <p class="mb-3">
              Le serveur répond, mais l'API hermes ne répond pas : le service
              est arrêté ou indisponible. Le routage et les jeux de données sont
              inaccessibles.
            </p>
            <ul class="hermes-alert-list">
              <li>
                Vous n'avez pas la main sur le service : contactez
                l'administrateur.
              </li>
              <li>
                La surveillance réessaie automatiquement ; cette alerte ne se
                rouvrira qu'après un rétablissement puis une nouvelle panne.
              </li>
            </ul>
          </v-card-text>
          <v-card-actions>
            <v-spacer />
            <v-btn variant="text" @click="hermesStore.dismissServiceAlert()">
              Fermer
            </v-btn>
            <v-btn
              color="primary"
              variant="flat"
              :loading="hermesStore.loading"
              @click="refresh"
            >
              Actualiser
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-dialog>
    </v-layout>
  </v-app>
</template>

<script setup lang="ts">
/**
 * Vue Hermes : serveur, jeux de données et secteurs custom.
 *
 * Le panneau gauche et la carte partagent la même sélection. Les régions
 * indexées sont dessinées en permanence sur la carte, en couche de fond. La
 * carte porte l'outil Mapbox Draw : la création dessine un secteur puis ouvre
 * le formulaire ; l'édition charge le secteur dans l'outil et permet d'ajouter,
 * déplacer ou supprimer ses sommets. La sélection d'un jeu de données passe par
 * une confirmation, puis la carte cadre son périmètre.
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
import { useSettingsStore } from '../stores/settings'
import { useUiStore } from '../stores/ui'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'
import HermesDatasetForm from '../components/Hermes/HermesDatasetForm.vue'
import HermesMap from '../components/Hermes/HermesMap.vue'
import HermesPanel from '../components/Hermes/HermesPanel.vue'
import HermesPolygonForm from '../components/Hermes/HermesPolygonForm.vue'
import HermesToolbar from '../components/Hermes/HermesToolbar.vue'
import {
  datasetAuxIssues,
  datasetDisplayName,
  datasetNeedsUpdate,
  datasetSectorNames as sectorNamesOf,
  parseHermesError,
  type HermesDrawMode,
  type HermesTab,
} from '../utils/hermes'

const router = useRouter()
const appStore = useAppStore()
const hermesStore = useHermesStore()
const settingsStore = useSettingsStore()
const ui = useUiStore()

/** Polygone custom sélectionné, ou `null`. */
const selectedPolygonId = ref<string | null>(null)
/** Onglet actif du panneau : la carte adapte ses couches. */
const activeTab = ref<HermesTab>('datasets')
/** Élément survolé dans la liste de l'onglet courant. */
const hoveredDatasetId = ref<string | null>(null)
const hoveredPolygonId = ref<string | null>(null)
/** Mode de l'outil de dessin Mapbox. */
const drawMode = ref<HermesDrawMode>('idle')
/** Compteur de session de dessin (ré-applique l'outil après une annulation). */
const drawSession = ref(0)
/** Polygone en cours d'édition. */
const editingPolygon = ref<HermesPolygon | null>(null)
/** Géométrie dessinée ou éditée, mise à jour à chaque `draw.update`. */
const drawnGeometry = ref<GeoJSON.Polygon | null>(null)
/** Tracé en cours pendant la création, pour l'aperçu du formulaire. */
const draftGeometry = ref<GeoJSON.Polygon | null>(null)
/** Formulaire polygone en tiroir droit : ouvert après le tracé ou en édition. */
const polygonFormOpen = ref(false)
const polygonFormMode = ref<'create' | 'edit'>('create')
const polygonSaving = ref(false)
const polygonDeleting = ref(false)
const deleteDialogOpen = ref(false)
const deleteCandidate = ref<HermesPolygon | null>(null)
/** IDs de polygones surlignés par le formulaire de dataset. */
const previewPolygonIds = ref<string[]>([])
const datasetFormOpen = ref(false)
const datasetSaving = ref(false)
const deleteDatasetDialogOpen = ref(false)
const deleteDatasetCandidate = ref<HermesDataset | null>(null)
const datasetDeleting = ref(false)
/** Confirmation de sélection d'un jeu de données (icône de la liste). */
const selectDatasetDialogOpen = ref(false)
const selectDatasetCandidate = ref<HermesDataset | null>(null)
const datasetSelecting = ref(false)
/** Confirmation de reconstruction d'un jeu de données « à mettre à jour ». */
const updateDatasetDialogOpen = ref(false)
const updateDatasetCandidate = ref<HermesDataset | null>(null)
const datasetRebuilding = ref(false)
/** Dataset prévisualisé sur la carte avant d'ouvrir la confirmation. */
const previewDatasetId = ref<string | null>(null)
/** Laisse le cadrage de la carte s'afficher avant la confirmation. */
const PREVIEW_DELAY_MS = 500
let selectDialogTimer: number | null = null

/** Une action polygone bloque les boutons du panneau et ceux du formulaire. */
const polygonBusy = computed(
  () =>
    polygonSaving.value || polygonDeleting.value || polygonFormOpen.value,
)

/** Jeux de données qui référencent le secteur candidat à la suppression. */
const deleteImpact = computed(() => {
  const polygon = deleteCandidate.value
  if (!polygon) return []
  return hermesStore.datasets
    .filter(dataset => dataset.zones?.includes(polygon.id))
    .map(dataset => datasetDisplayName(dataset))
})

/** Noms des secteurs assemblés par un jeu de données. */
function datasetSectorNames(dataset: HermesDataset | null): string {
  return sectorNamesOf(dataset, hermesStore.polygons).join(', ')
}

const selectDatasetSectors = computed(() =>
  datasetSectorNames(selectDatasetCandidate.value),
)

const updateDatasetSectors = computed(() =>
  datasetSectorNames(updateDatasetCandidate.value),
)

const updateDatasetIsServed = computed(
  () =>
    !!updateDatasetCandidate.value &&
    updateDatasetCandidate.value.id === hermesStore.activeDatasetId,
)

const selectDatasetNeedsUpdate = computed(() => {
  const dataset = selectDatasetCandidate.value
  return dataset ? datasetNeedsUpdate(dataset, hermesStore.polygons) : false
})

const selectDatasetAuxIssues = computed(() => {
  const dataset = selectDatasetCandidate.value
  return dataset ? datasetAuxIssues(dataset) : []
})

function displayName(dataset: HermesDataset): string {
  return datasetDisplayName(dataset)
}

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

function onSelectPolygon(id: string | null): void {
  selectedPolygonId.value = id
}

/**
 * Change d'onglet, efface le survol précédent et réactualise les données
 * (dont les régions, couche de fond permanente).
 */
function onTabUpdate(tab: HermesTab): void {
  if (tab !== activeTab.value) {
    activeTab.value = tab
    hoveredDatasetId.value = null
    hoveredPolygonId.value = null
    void refresh()
  }
}

// ─── Création / édition ─────────────────────────────────────────────

function startCreatePolygon(): void {
  selectedPolygonId.value = null
  editingPolygon.value = null
  drawnGeometry.value = null
  draftGeometry.value = null
  polygonFormMode.value = 'create'
  polygonSaving.value = false
  // Le panneau s'ouvre avant le tracé : l'utilisateur peut saisir le nom et
  // suivre la surface/le nombre de sommets pendant qu'il dessine.
  polygonFormOpen.value = true
  drawSession.value += 1
  drawMode.value = 'create'
}

function onDrawCreate(geometry: GeoJSON.Polygon): void {
  draftGeometry.value = null
  drawnGeometry.value = geometry
  polygonFormOpen.value = true
}

/** Aperçu du tracé en cours (création uniquement). */
function onDrawDraft(geometry: GeoJSON.Polygon | null): void {
  if (polygonFormOpen.value && polygonFormMode.value === 'create') {
    draftGeometry.value = geometry
  }
}

function onDrawUpdate(geometry: GeoJSON.Polygon): void {
  if (polygonFormOpen.value) {
    drawnGeometry.value = geometry
  }
}

/** Le polygone a été supprimé sur la carte (dernier sommet retiré). */
function onDrawClear(): void {
  if (polygonFormOpen.value) {
    drawnGeometry.value = null
    draftGeometry.value = null
  }
}

/** Repart d'un tracé vierge sans fermer le formulaire ni perdre le nom saisi. */
function onPolygonRedraw(): void {
  if (polygonFormMode.value !== 'create') return
  drawnGeometry.value = null
  draftGeometry.value = null
  drawSession.value += 1
  drawMode.value = 'create'
}

function startEditPolygon(polygon: HermesPolygon): void {
  selectedPolygonId.value = polygon.id
  editingPolygon.value = polygon
  drawnGeometry.value = polygon.geometry
  draftGeometry.value = null
  polygonFormMode.value = 'edit'
  polygonSaving.value = false
  polygonFormOpen.value = true
  drawSession.value += 1
  drawMode.value = 'edit'
}

function onPolygonCancel(): void {
  polygonFormOpen.value = false
  polygonSaving.value = false
  drawMode.value = 'idle'
  drawnGeometry.value = null
  draftGeometry.value = null
  editingPolygon.value = null
}

async function onPolygonSubmit(payload: HermesPolygonInput): Promise<void> {
  polygonSaving.value = true
  try {
    let saved: HermesPolygon
    if (polygonFormMode.value === 'edit' && editingPolygon.value) {
      const polygonId = editingPolygon.value.id
      // Les datasets assemblés doivent être reconstruits après une modification.
      const affected = hermesStore.datasets
        .filter(dataset => dataset.zones?.includes(polygonId))
        .map(dataset => datasetDisplayName(dataset))
      saved = await hermesStore.updatePolygon(polygonId, {
        name: payload.name,
        description: payload.description ?? null,
        geometry: payload.geometry,
      })
      const suffix =
        affected.length > 0
          ? ` Jeux de données à mettre à jour : ${affected.join(', ')}.`
          : ''
      ui.showSuccess(`Secteur « ${saved.name} » enregistré.${suffix}`)
    } else {
      saved = await hermesStore.createPolygon({
        id: payload.id,
        name: payload.name,
        description: payload.description ?? null,
        geometry: payload.geometry,
      })
      ui.showSuccess(`Secteur « ${saved.name} » créé.`)
    }

    selectedPolygonId.value = saved.id
    polygonFormOpen.value = false
    drawMode.value = 'idle'
    drawnGeometry.value = null
    draftGeometry.value = null
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
    ui.showSuccess(`Secteur « ${polygon.name} » supprimé.`)
    deleteDialogOpen.value = false
    deleteCandidate.value = null
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  } finally {
    polygonDeleting.value = false
  }
}

// ─── Jeux de données ────────────────────────────────────────────────

function startCreateDataset(): void {
  datasetFormOpen.value = true
}

function onDatasetPreview(payload: { polygonIds: string[] }): void {
  previewPolygonIds.value = payload.polygonIds
}

function clearDatasetPreview(): void {
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
    const label = payload.name?.trim() || result.id
    ui.showSuccess(`Jeu de données « ${label} » en cours de préparation.`)
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
    ui.showSuccess(`Jeu de données « ${datasetDisplayName(dataset)} » supprimé.`)
    deleteDatasetDialogOpen.value = false
    deleteDatasetCandidate.value = null
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  } finally {
    datasetDeleting.value = false
  }
}

// ─── Sélection du jeu de données servi ──────────────────────────────

/**
 * Prévisualise le dataset sur la carte (surbrillance + cadrage) puis ouvre la
 * confirmation après un court délai, pour que l'utilisateur voie son périmètre
 * avant de décider.
 */
function requestSelectDataset(dataset: HermesDataset): void {
  selectDatasetCandidate.value = dataset
  previewDatasetId.value = dataset.id
  if (selectDialogTimer !== null) {
    window.clearTimeout(selectDialogTimer)
  }
  selectDialogTimer = window.setTimeout(() => {
    selectDialogTimer = null
    selectDatasetDialogOpen.value = true
  }, PREVIEW_DELAY_MS)
}

function clearSelectPreview(): void {
  previewDatasetId.value = null
  if (selectDialogTimer !== null) {
    window.clearTimeout(selectDialogTimer)
    selectDialogTimer = null
  }
}

function cancelSelectDataset(): void {
  selectDatasetDialogOpen.value = false
  selectDatasetCandidate.value = null
  clearSelectPreview()
}

async function confirmSelectDataset(): Promise<void> {
  const dataset = selectDatasetCandidate.value
  if (!dataset) return
  datasetSelecting.value = true
  try {
    await hermesStore.selectServingDataset(dataset.id)
    ui.showSuccess(
      `Jeu de données « ${datasetDisplayName(dataset)} » sélectionné.`,
    )
    selectDatasetDialogOpen.value = false
    selectDatasetCandidate.value = null
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  } finally {
    datasetSelecting.value = false
    clearSelectPreview()
  }
}

// ─── Reconstruction d'un jeu de données « à mettre à jour » ─────────

function requestUpdateDataset(dataset: HermesDataset): void {
  updateDatasetCandidate.value = dataset
  updateDatasetDialogOpen.value = true
}

function cancelUpdateDataset(): void {
  updateDatasetDialogOpen.value = false
  updateDatasetCandidate.value = null
}

async function confirmUpdateDataset(): Promise<void> {
  const dataset = updateDatasetCandidate.value
  if (!dataset) return
  datasetRebuilding.value = true
  try {
    await hermesStore.rebuildDataset(dataset.id)
    ui.showSuccess(
      `Jeu de données « ${datasetDisplayName(dataset)} » en cours de reconstruction.`,
    )
    updateDatasetDialogOpen.value = false
    updateDatasetCandidate.value = null
  } catch (error) {
    ui.showError(parseHermesError(error).message)
  } finally {
    datasetRebuilding.value = false
  }
}

// ─── Alertes service / serveur ──────────────────────────────────────

/** Réveil lancé depuis la popup « serveur injoignable ». */
async function wakeFromAlert(): Promise<void> {
  hermesStore.dismissSystemAlert()
  const ok = await hermesStore.wake()
  if (ok) {
    ui.showSuccess('Serveur hermes réveillé.')
  } else {
    ui.showWarning("Le serveur hermes n'a pas répondu dans le délai imparti.")
  }
}

onMounted(() => {
  // Les limites de création (surface, nombre de polygones) viennent des
  // paramètres `Hermes.Limites.*` : elles doivent être disponibles avant
  // l'ouverture des formulaires.
  void settingsStore.loadSettings()
  void refresh()
})

onUnmounted(() => {
  drawMode.value = 'idle'
  clearSelectPreview()
  hermesStore.cancelPolling()
  hermesStore.cancelServingPolling()
})
</script>

<style scoped>
/* Fenêtres de confirmation : 50 % d'opacité, pour laisser voir la carte
   (périmètre du dataset prévisualisé) derrière la boîte de dialogue. */
.hermes-confirm-card {
  background: rgba(var(--v-theme-surface), 0.5) !important;
}

.hermes-alert-list {
  padding-left: 20px;
  line-height: 1.6;
}
</style>
