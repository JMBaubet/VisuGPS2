<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <AltimetrieToolbar
        :mode="activeTab"
        :trace-name="traceName"
        :dirty="toolbarDirty"
        :saving="toolbarSaving"
        :validated="toolbarValidated"
        :ready="toolbarReady"
        :settings-status="toolbarSettingsStatus"
        :settings-causes="toolbarSettingsCauses"
        :params-outdated="toolbarParamsOutdated"
        @back="onBackClicked"
        @save="onSaveClicked"
        @recalculate="onRecalculateClicked"
        @open-settings="toggleSettings"
      />

      <AltimetrieClimbsPanel
        :active-tab="activeTab"
        :climbs="altimetrieStore.climbs"
        :profile="profile"
        :alt-min="altBounds.min"
        :alt-max="altBounds.max"
        :total-distance-m="totalDistanceM"
        :selected-id="altimetrieStore.selectedClimbId"
        :creating="creating"
        :tunnels="tunnelGroups"
        :selected-tunnel-ids="selectedTunnelIds"
        :tunnel-count="tunnelCount"
        :tunnel-creating="creatingTunnel"
        :tunnel-pending="tunnelPending"
        :tunnel-preparing="tunnelPreparing"
        :tunnel-progress="tunnelProgress"
        :tunnel-requirements="tunnelRequirements"
        :tunnel-has-file="tunnelHasFile"
        :can-create-tunnel="tunnelCreationAvailable"
        :merge-gap-m="mergeGapM"
        :tunnel-error="tunnelError"
        @update:active-tab="onTabChange"
        @select="altimetrieStore.selectClimb"
        @delete="onDeleteClimb"
        @split="onSplitClimb"
        @merge="onMergeClimb"
        @create="onCreateRequested"
        @select-tunnel="onSelectTunnel"
        @delete-tunnel="onDeleteTunnel"
        @merge-tunnel="onMergeTunnel"
        @split-tunnel="onSplitTunnel"
        @create-tunnel="onCreateTunnelRequested"
        @relaunch-tunnels="onRelaunchTunnels"
      />

      <v-main>
        <div class="altimetrie-center">
          <v-progress-circular
            v-if="isLoading"
            :size="60"
            :width="7"
            color="primary"
            indeterminate
          />

          <v-empty-state
            v-else-if="errorMessage"
            icon="mdi-alert-circle-outline"
            title="Analyse indisponible"
            :text="errorMessage"
          />

          <AltimetrieMap
            v-else
            ref="mapRef"
            :file="altimetrieStore.displayFile"
            :trace-geometry="traceGeometry"
            :selected-climb-id="
              activeTab === 'climbs' ? altimetrieStore.selectedClimbId : null
            "
          />

          <AltimetrieProfileWidget
            v-if="altimetrieStore.displayFile && activeTab === 'climbs'"
            :file="altimetrieStore.displayFile"
            :selected-climb-id="altimetrieStore.selectedClimbId"
            :creating="creating"
            @select-climb="altimetrieStore.selectClimb"
            @recenter="onRecenter"
            @inspect="onInspect"
            @edit-bounds="onEditBounds"
            @draft-bounds="onDraftBounds"
            @cancel-bounds="altimetrieStore.clearClimbDraft"
            @create-climb="onCreateClimb"
            @cancel-create="onCancelCreation"
            @create-error="onCreateError"
          />

          <div v-if="inspectPoint" class="altimetrie-inspect">
            <div class="altimetrie-inspect__row">
              <span class="altimetrie-inspect__label">Distance</span>
              <span class="altimetrie-inspect__value">
                {{ formatDistance(inspectPoint.distanceM) }}
              </span>
            </div>
            <div class="altimetrie-inspect__row">
              <span class="altimetrie-inspect__label">Pente</span>
              <span class="altimetrie-inspect__value">
                {{ inspectPoint.slopePct.toFixed(1) }} %
              </span>
            </div>
            <div class="altimetrie-inspect__row">
              <span class="altimetrie-inspect__label">Altitude</span>
              <span class="altimetrie-inspect__value">
                {{ formatAltitude(inspectPoint.altitudeM) }}
              </span>
            </div>
          </div>
        </div>
      </v-main>

      <!-- Panneau Paramètres (groupes Altimetrie.* de la vue active) -->
      <SettingsDrawer :show-system="false" />

      <ConfirmExitDialog
        v-model="dialogExitOpen"
        @confirm="onExitConfirmed"
        @cancel="dialogExitOpen = false"
      />

      <CreateClimbDialog
        v-model="createHelpOpen"
        @start="onStartCreation"
        @cancel="createHelpOpen = false"
      />

      <ConfirmRecalcDialog
        v-model="dialogRecalcOpen"
        @confirm="runRecalculation"
        @cancel="dialogRecalcOpen = false"
      />

      <CreateTunnelDialog
        v-model="tunnelCreateHelpOpen"
        @start="onStartTunnelCreation"
        @cancel="tunnelCreateHelpOpen = false"
      />

      <ConfirmTunnelDetectionDialog
        v-model="dialogTunnelDetectOpen"
        :existing-count="tunnelGroups.length"
        :dirty="tunnelsStore.dirty"
        @confirm="runTunnelDetection"
        @cancel="dialogTunnelDetectOpen = false"
      />
    </v-layout>
  </v-app>
</template>

<script setup lang="ts">
/**
 * Vue Altimétrie d'une trace propre (route `/altimetrie?traceId=…`).
 *
 * Elle charge l'analyse `altimetrie.json` — ou la génère si elle manque — et
 * présente les montées détectées dans un panneau latéral. La carte 3D et le
 * widget de profil seront branchés aux sous-étapes suivantes ; cette première
 * version pose le squelette de vue, le store et l'entrée depuis le circuit.
 *
 * La trace est désignée par la **query** de la route, comme Audit et Multiride.
 * À la sortie, le store est réinitialisé ; si des montées ont été éditées sans
 * être sauvegardées, un dialogue demande confirmation.
 */
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useAltimetrieStore, type AltimetrieParams } from '../stores/altimetrie'
import { useTunnelsStore } from '../stores/tunnels'
import { useSettingsStore } from '../stores/settings'
import { useTracesStore } from '../stores/traces'
import { useUiStore } from '../stores/ui'
import AltimetrieToolbar from '../components/Altimetrie/AltimetrieToolbar.vue'
import AltimetrieClimbsPanel from '../components/Altimetrie/AltimetrieClimbsPanel.vue'
import AltimetrieMap from '../components/Altimetrie/AltimetrieMap.vue'
import AltimetrieProfileWidget from '../components/Altimetrie/AltimetrieProfileWidget.vue'
import ConfirmExitDialog from '../components/Altimetrie/dialogs/ConfirmExitDialog.vue'
import CreateClimbDialog from '../components/Altimetrie/dialogs/CreateClimbDialog.vue'
import ConfirmRecalcDialog from '../components/Altimetrie/dialogs/ConfirmRecalcDialog.vue'
import CreateTunnelDialog from '../components/Altimetrie/dialogs/CreateTunnelDialog.vue'
import ConfirmTunnelDetectionDialog from '../components/Altimetrie/dialogs/ConfirmTunnelDetectionDialog.vue'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'
import {
  computeAltimetrieDiagnostics,
  type AltimetrieStatus,
} from '../utils/altimetrieDiagnostics'
import type { ClimbHandle } from '../utils/altimetrieClimbs'
import { formatAltitude, formatDistance } from '../utils/format'

/** Valeurs du segment minimal affichées dans le tooltip de la carte. */
interface InspectPoint {
  distanceM: number
  slopePct: number
  altitudeM: number
}

const route = useRoute()
const router = useRouter()
const appStore = useAppStore()
const altimetrieStore = useAltimetrieStore()
const tunnelsStore = useTunnelsStore()
const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const ui = useUiStore()

/** Chargement initial (réglages, traces, analyse). */
const initializing = ref(true)
/** Onglet actif du panneau gauche. */
const activeTab = ref<'climbs' | 'tunnels'>(
  route.query.tab === 'tunnels' ? 'tunnels' : 'climbs',
)
/** Message d'erreur de détection, affiché au centre. */
const errorMessage = ref<string | null>(null)
/** Confirmation de sortie avec des modifications non enregistrées. */
const dialogExitOpen = ref(false)
/** Création manuelle de montée en cours (deux clics dans le widget). */
const creating = ref(false)
/** Dialogue d'aide de création de montée ouvert. */
const createHelpOpen = ref(false)
/** Création manuelle de tunnel en cours (deux clics dans le widget). */
const creatingTunnel = ref(false)
/**
 * Le widget de profil accepte la création de tunnels. Activé à la sous-étape
 * « widget » ; le bouton reste sinon désactivé, sans flux inachevé.
 */
const tunnelCreationAvailable = ref(false)
/** Dialogue d'aide de création de tunnel ouvert. */
const tunnelCreateHelpOpen = ref(false)
/** Confirmation du recalcul de l'altimétrie lorsque des éditions sont en cours. */
const dialogRecalcOpen = ref(false)
/** Confirmation de relance de la détection des tunnels. */
const dialogTunnelDetectOpen = ref(false)
/** Géométrie complète de la trace, pour le trait neutre de la carte. */
const traceGeometry = ref<GeoJSON.Feature | null>(null)
/** Instance de la carte, exposée pour le widget de profil. */
const mapRef = ref<InstanceType<typeof AltimetrieMap> | null>(null)
/** Valeurs du dernier clic sur le profil, affichées 5 s sur la carte. */
const inspectPoint = ref<InspectPoint | null>(null)
let inspectTimer: ReturnType<typeof setTimeout> | null = null

const traceId = computed(() => (route.query.traceId as string | null) ?? null)
const traceName = computed(
  () => tracesStore.traces.find(trace => trace.id === traceId.value)?.name ?? 'Trace',
)
const profile = computed(() => altimetrieStore.file?.profile ?? [])
const isLoading = computed(
  () =>
    initializing.value ||
    altimetrieStore.loading ||
    tunnelsStore.loading,
)

// ─── Onglet Tunnels ──────────────────────────────────────────────────

const tunnelGroups = computed(() => tunnelsStore.groups)
const selectedTunnelIds = computed(() => tunnelsStore.selectedTunnelIds)
const tunnelCount = computed(() => tunnelsStore.tunnelCount)
const mergeGapM = computed(() =>
  settingNumber('Tunnels.Detection.mergeGap', 100),
)
const tunnelPending = computed(() =>
  traceId.value ? tunnelsStore.isPending(traceId.value) : false,
)
const tunnelPreparing = computed(() =>
  traceId.value ? tunnelsStore.isPreparing(traceId.value) : false,
)
const tunnelProgress = computed(() =>
  traceId.value ? tunnelsStore.progressFor(traceId.value) : null,
)
const tunnelRequirements = computed(() => tunnelsStore.requirements)
const tunnelHasFile = computed(() => tunnelsStore.fileExists)
const tunnelError = computed(() =>
  tunnelsStore.lastErrorTraceId === traceId.value
    ? tunnelsStore.lastError
    : null,
)

/** Paramètres de détection courants, pour signaler un fichier périmé. */
const currentTunnelParams = computed(() => ({
  distanceToleranceM: settingNumber('Tunnels.Detection.distanceTolerance', 20),
  angleToleranceDeg: settingNumber('Tunnels.Detection.angleTolerance', 25),
  bboxMarginM: settingNumber('Tunnels.Detection.bboxMargin', 500),
  bearingWindowM: settingNumber('Tunnels.Detection.bearingWindow', 30),
  minRunLength: settingNumber('Tunnels.Detection.minRunLength', 2),
}))

const tunnelParamsOutdated = computed(() => {
  const currentFile = tunnelsStore.file
  if (!currentFile || !tunnelsStore.fileExists) return false
  const current = currentTunnelParams.value
  return (Object.keys(current) as (keyof typeof current)[]).some(
    key => currentFile.params[key] !== current[key],
  )
})

// ─── Toolbar adaptée à l'onglet ──────────────────────────────────────

const toolbarDirty = computed(() =>
  activeTab.value === 'tunnels'
    ? tunnelsStore.dirty
    : altimetrieStore.dirty,
)
const toolbarSaving = computed(() =>
  activeTab.value === 'tunnels'
    ? tunnelsStore.saving
    : altimetrieStore.saving,
)
const toolbarValidated = computed(() =>
  activeTab.value === 'tunnels'
    ? tunnelsStore.validated
    : altimetrieStore.validated,
)
const toolbarReady = computed(() =>
  activeTab.value === 'tunnels'
    ? !!altimetrieStore.file && !tunnelPending.value
    : !!altimetrieStore.file,
)
const toolbarSettingsStatus = computed<AltimetrieStatus>(() =>
  activeTab.value === 'tunnels'
    ? tunnelParamsOutdated.value
      ? 'warning'
      : 'success'
    : settingsStatus.value,
)
const toolbarSettingsCauses = computed(() =>
  activeTab.value === 'tunnels'
    ? tunnelParamsOutdated.value
      ? ['Paramètres de détection modifiés — relancez la détection']
      : []
    : settingsCauses.value,
)
const toolbarParamsOutdated = computed(
  () => activeTab.value === 'climbs' && paramsOutdated.value,
)

const totalDistanceM = computed(() => {
  const points = profile.value
  if (points.length === 0) return 0
  return points[points.length - 1].d
})

const altBounds = computed(() => {
  const points = profile.value
  if (points.length === 0) return { min: 0, max: 0 }

  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const point of points) {
    if (point.ele < min) min = point.ele
    if (point.ele > max) max = point.ele
  }
  return { min, max }
})

/** Paramètres courants, recomposés à chaque changement du drawer. */
const currentParams = computed<AltimetrieParams>(() => buildParams())

/** Diagnostic de cohérence des paramètres courants. */
const diagnostics = computed(() =>
  computeAltimetrieDiagnostics(currentParams.value),
)

/** Le fichier a été généré avec d'autres paramètres que les réglages courants. */
const paramsOutdated = computed(() => {
  const file = altimetrieStore.file
  if (!file) return false
  return paramsKey(file.params) !== paramsKey(currentParams.value)
})

/** Statut global affiché dans la toolbar : un fichier périmé est au moins orange. */
const settingsStatus = computed<AltimetrieStatus>(() =>
  paramsOutdated.value && diagnostics.value.status === 'success'
    ? 'warning'
    : diagnostics.value.status,
)

/** Causes du statut, complétées par l'écart entre fichier et réglages. */
const settingsCauses = computed(() => {
  const causes = [...diagnostics.value.causes]
  if (paramsOutdated.value) {
    causes.push('Paramètres modifiés depuis la génération')
  }
  return causes
})

/** Valeur d'un paramètre numérique, avec repli sur la valeur par défaut. */
function settingNumber(path: string, fallback: number): number {
  const definition = settingsStore.settings.find(setting => setting.path === path)
  const value = definition?.value
  return typeof value === 'number' ? value : fallback
}

/** Valeur d'un paramètre booléen, avec repli sur la valeur par défaut. */
function settingBool(path: string, fallback: boolean): boolean {
  const definition = settingsStore.settings.find(setting => setting.path === path)
  const value = definition?.value
  return typeof value === 'boolean' ? value : fallback
}

/** Assemble les paramètres du pipeline depuis les réglages `Altimetrie.*`. */
function buildParams(): AltimetrieParams {
  return {
    pasEchantillonnage: settingNumber('Altimetrie.Preparation.pasEchantillonnage', 50),
    correctionPlateaux: settingNumber('Altimetrie.Preparation.correctionPlateaux', 6),
    fenetreAntiPic: settingNumber('Altimetrie.Preparation.fenetreAntiPic', 200),
    seuilAntiPic: settingNumber('Altimetrie.Preparation.seuilAntiPic', 15),
    fenetreAltitude: settingNumber('Altimetrie.Lissage.fenetreAltitude', 500),
    fenetrePente: settingNumber('Altimetrie.Lissage.fenetrePente', 150),
    sigmaGaussien: settingNumber('Altimetrie.Lissage.sigmaGaussien', 75),
    fenetreMediane: settingNumber('Altimetrie.Lissage.fenetreMediane', 0),
    lissageRenforce: settingBool('Altimetrie.Lissage.lissageRenforce', true),
    seuilMontee: settingNumber('Altimetrie.Detection.seuilMontee', 0.5),
    ecartFusion: settingNumber('Altimetrie.Detection.ecartFusion', 250),
    rechercheExtrema: settingNumber('Altimetrie.Detection.rechercheExtrema', 60),
    fenetreCalcul: settingNumber('Altimetrie.Segments.fenetreCalcul', 100),
    longueurMin: settingNumber('Altimetrie.Segments.longueurMin', 200),
  }
}

function toggleSettings(): void {
  appStore.isSettingsDrawerOpen = !appStore.isSettingsDrawerOpen
}

function onBackClicked(): void {
  router.push({ name: 'accueil' })
}

/** Recentre la carte sur un point du profil (clic dans le widget). */
function onRecenter(distanceM: number): void {
  mapRef.value?.centerOnDistance(distanceM)
}

/**
 * Affiche les valeurs du segment minimal cliqué, pendant 5 secondes.
 *
 * Chaque clic remplace le contenu précédent et relance le minuteur.
 */
function onInspect(payload: InspectPoint): void {
  inspectPoint.value = payload

  if (inspectTimer) clearTimeout(inspectTimer)
  inspectTimer = setTimeout(() => {
    inspectPoint.value = null
    inspectTimer = null
  }, 5000)
}

/** Projette la borne en cours de glissement, sans appel backend. */
function onDraftBounds(payload: {
  climbId: number
  handle: ClimbHandle
  km: number
}): void {
  altimetrieStore.draftClimbBound(
    payload.climbId,
    payload.handle,
    payload.km,
  )
}

/** Applique une borne déplacée dans le widget, via un aperçu backend. */
async function onEditBounds(payload: {
  climbId: number
  handle: ClimbHandle
  km: number
}): Promise<void> {
  if (!traceId.value) return
  try {
    await altimetrieStore.updateClimbBound(
      traceId.value,
      payload.climbId,
      payload.handle,
      payload.km,
    )
  } catch (error) {
    const message =
      typeof error === 'string'
        ? error
        : "Modification de la montée impossible."
    ui.showError(message)
  }
}

/**
 * Enregistre l'analyse. La première sauvegarde vaut **validation** de
 * l'altimétrie par l'opérateur, qu'elle passe par « Valider » ou par
 * « Sauvegarder » après une modification.
 */
async function onSaveClicked(): Promise<void> {
  if (activeTab.value === 'tunnels') {
    await saveTunnels()
    return
  }
  if (!traceId.value) return
  const wasValidated = altimetrieStore.validated
  try {
    await altimetrieStore.save(traceId.value)
    ui.showSuccess(
      wasValidated ? 'Altimétrie enregistrée.' : 'Altimétrie validée.',
    )
  } catch (error) {
    const message =
      typeof error === 'string'
        ? error
        : "Enregistrement de l'altimétrie impossible."
    ui.showError(message)
  }
}

/** Persiste les tunnels édités ; la sauvegarde vaut validation. */
async function saveTunnels(): Promise<void> {
  if (!traceId.value) return
  const wasValidated = tunnelsStore.validated
  try {
    await tunnelsStore.save(traceId.value)
    ui.showSuccess(
      wasValidated ? 'Tunnels enregistrés.' : 'Tunnels validés.',
    )
  } catch (error) {
    const message =
      typeof error === 'string'
        ? error
        : "Enregistrement des tunnels impossible."
    ui.showError(message)
  }
}

/** Clé comparable des 14 paramètres, dans un ordre stable. */
function paramsKey(params: AltimetrieParams): string {
  return [
    params.pasEchantillonnage,
    params.correctionPlateaux,
    params.fenetreAntiPic,
    params.seuilAntiPic,
    params.fenetreAltitude,
    params.fenetrePente,
    params.sigmaGaussien,
    params.fenetreMediane,
    params.lissageRenforce ? 1 : 0,
    params.seuilMontee,
    params.ecartFusion,
    params.rechercheExtrema,
    params.fenetreCalcul,
    params.longueurMin,
  ].join('|')
}

/** Ouvre la confirmation de recalcul si des éditions sont en cours. */
function onRecalculateClicked(): void {
  if (creating.value) {
    ui.showWarning(
      "Terminez ou annulez la création de montée avant de recalculer.",
    )
    return
  }
  if (altimetrieStore.dirty) {
    dialogRecalcOpen.value = true
    return
  }
  void runRecalculation()
}

/** Rejoue la détection avec les paramètres courants (action explicite). */
async function runRecalculation(): Promise<void> {
  if (!traceId.value) return
  dialogRecalcOpen.value = false
  try {
    await altimetrieStore.detect(traceId.value, currentParams.value)
    ui.showSuccess('Détection altimétrique recalculée.')
  } catch (error) {
    const message =
      typeof error === 'string'
        ? error
        : 'Recalcul de la détection impossible.'
    ui.showError(message)
  }
}

function onCreateRequested(): void {
  createHelpOpen.value = true
}

function onStartCreation(): void {
  createHelpOpen.value = false
  creating.value = true
}

function onCancelCreation(): void {
  creating.value = false
}

function onCreateError(message: string): void {
  ui.showWarning(message)
}

async function onCreateClimb(payload: {
  startKm: number
  endKm: number
}): Promise<void> {
  if (!traceId.value) return
  try {
    await altimetrieStore.createClimb(
      traceId.value,
      payload.startKm,
      payload.endKm,
    )
    creating.value = false
    ui.showSuccess("Montée ajoutée. Sauvegardez pour l'enregistrer.")
  } catch (error) {
    const message =
      typeof error === 'string'
        ? error
        : 'Création de la montée impossible.'
    ui.showError(message)
  }
}

// ─── Actions de l'onglet Tunnels ─────────────────────────────────────

/** Bascule d'onglet : annule les créations et vide les sélections croisées. */
function onTabChange(tab: 'climbs' | 'tunnels'): void {
  if (tab === activeTab.value) return
  creating.value = false
  creatingTunnel.value = false
  createHelpOpen.value = false
  tunnelCreateHelpOpen.value = false
  altimetrieStore.selectClimb(null)
  tunnelsStore.selectTunnel(null)
  activeTab.value = tab
  void router.replace({ query: { ...route.query, tab } })
  if (tab === 'tunnels') {
    void tunnelsStore.refreshSupport()
    if (traceId.value) void tunnelsStore.refreshRequirements(traceId.value)
  }
}

function onCreateTunnelRequested(): void {
  if (!tunnelCreationAvailable.value) return
  if (tunnelPending.value) {
    ui.showWarning('La préparation des tunnels est déjà en cours.')
    return
  }
  tunnelCreateHelpOpen.value = true
}

function onStartTunnelCreation(): void {
  tunnelCreateHelpOpen.value = false
  creating.value = false
  creatingTunnel.value = true
}

function onSelectTunnel(id: string): void {
  tunnelsStore.selectTunnel(id)
}

function onDeleteTunnel(id: string): void {
  if (creatingTunnel.value) {
    ui.showWarning(
      'Terminez ou annulez la création de tunnel avant de modifier la liste.',
    )
    return
  }
  tunnelsStore.deleteGroup(id)
}

function onMergeTunnel(id: string): void {
  if (creatingTunnel.value) {
    ui.showWarning(
      'Terminez ou annulez la création de tunnel avant de modifier la liste.',
    )
    return
  }
  if (!tunnelsStore.mergeWithPrevious(id, mergeGapM.value)) {
    ui.showWarning(
      "Fusion impossible : écart trop grand ou tunnel sans précédent.",
    )
    return
  }
  ui.showSuccess("Tunnels fusionnés. Sauvegardez pour l'enregistrer.")
}

function onSplitTunnel(id: string): void {
  if (creatingTunnel.value) {
    ui.showWarning(
      'Terminez ou annulez la création de tunnel avant de modifier la liste.',
    )
    return
  }
  if (tunnelsStore.splitGroup(id)) {
    ui.showSuccess("Tunnels séparés. Sauvegardez pour l'enregistrer.")
  }
}

/** Relance explicite : confirmation si des tunnels ou des éditions existent. */
function onRelaunchTunnels(): void {
  if (tunnelPending.value) {
    ui.showWarning('La préparation des tunnels est déjà en cours.')
    return
  }
  if (tunnelsStore.dirty || tunnelGroups.value.length > 0) {
    dialogTunnelDetectOpen.value = true
    return
  }
  runTunnelDetection()
}

/** Enfile la préparation silencieuse de la trace courante. */
function runTunnelDetection(): void {
  dialogTunnelDetectOpen.value = false
  if (!traceId.value) return
  tunnelsStore.request(traceId.value)
}

async function onDeleteClimb(id: number): Promise<void> {
  if (!traceId.value) return
  if (creating.value) {
    ui.showWarning(
      "Terminez ou annulez la création de montée avant de modifier la liste.",
    )
    return
  }
  try {
    await altimetrieStore.deleteClimb(traceId.value, id)
  } catch (error) {
    const message =
      typeof error === 'string'
        ? error
        : 'Suppression de la montée impossible.'
    ui.showError(message)
  }
}

async function onSplitClimb(id: number): Promise<void> {
  if (!traceId.value) return
  if (creating.value) {
    ui.showWarning(
      "Terminez ou annulez la création de montée avant de modifier la liste.",
    )
    return
  }
  try {
    await altimetrieStore.splitClimb(traceId.value, id)
  } catch (error) {
    const message =
      typeof error === 'string' ? error : 'Séparation de la montée impossible.'
    ui.showError(message)
  }
}

async function onMergeClimb(id: number): Promise<void> {
  if (!traceId.value) return
  if (creating.value) {
    ui.showWarning(
      "Terminez ou annulez la création de montée avant de modifier la liste.",
    )
    return
  }
  try {
    await altimetrieStore.mergeClimb(traceId.value, id)
  } catch (error) {
    const message =
      typeof error === 'string' ? error : 'Fusion des montées impossible.'
    ui.showError(message)
  }
}

function onExitConfirmed(): void {
  dialogExitOpen.value = false
  creating.value = false
  creatingTunnel.value = false
  altimetrieStore.reset()
  tunnelsStore.reset()
  router.push({ name: 'accueil' })
}

onMounted(async () => {
  if (!traceId.value) {
    ui.showError('Aucune trace à analyser.')
    router.replace({ name: 'accueil' })
    initializing.value = false
    return
  }

  try {
    await settingsStore.loadSettings()
    await tracesStore.loadTraces()

    const trace = tracesStore.traces.find(entry => entry.id === traceId.value)
    if (!trace) {
      ui.showError('Trace introuvable.')
      router.replace({ name: 'accueil' })
      return
    }
    if (trace.audit_status !== 'clean') {
      ui.showWarning(
        "L'altimétrie n'est disponible qu'après nettoyage de la trace.",
      )
      router.replace({ name: 'accueil' })
      return
    }

    try {
      // Le cache a pu être rempli avant un nettoyage qui réécrit le GPX et
      // `trace.geojson` : on force une relecture pour colorer les segments
      // sur la géométrie réellement nettoyée.
      tracesStore.invalidateGeometry(traceId.value)
      traceGeometry.value = await tracesStore.getTraceGeometry(traceId.value)
    } catch (error) {
      console.warn(
        `[Altimetrie] Géométrie indisponible pour ${traceId.value} :`,
        error,
      )
    }

    await altimetrieStore.loadOrDetect(
      traceId.value,
      buildParams(),
      trace.altimetrie_validated,
    )

    // Onglet Tunnels : état du support OSM, fichier éventuel (ou état local
    // vide pour permettre la création manuelle), puis écoute de la
    // progression des téléchargements.
    await tunnelsStore.refreshSupport()
    await tunnelsStore.loadOrCreate(
      traceId.value,
      trace.tunnels_status === 'success',
    )
    void tunnelsStore.bindProgress()
  } catch (error) {
    const message =
      typeof error === 'string' ? error : 'Analyse altimétrique impossible.'
    errorMessage.value = message
    ui.showError(message)
  } finally {
    initializing.value = false
  }
})

onUnmounted(() => {
  if (inspectTimer) clearTimeout(inspectTimer)
  inspectTimer = null
  tunnelsStore.reset()
})

onBeforeRouteLeave(() => {
  if (
    !altimetrieStore.dirty &&
    !tunnelsStore.dirty &&
    !creating.value &&
    !creatingTunnel.value
  ) {
    altimetrieStore.reset()
    tunnelsStore.reset()
    return true
  }
  if (!dialogExitOpen.value) {
    dialogExitOpen.value = true
    return false
  }
  creating.value = false
  creatingTunnel.value = false
  altimetrieStore.reset()
  tunnelsStore.reset()
  return true
})
</script>

<style scoped>
.altimetrie-center {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.altimetrie-inspect {
  position: absolute;
  top: 16px;
  left: 16px;
  z-index: 6;
  min-width: 150px;
  padding: 8px 10px;
  border-radius: 8px;
  background: rgba(var(--v-theme-surface), 0.95);
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.18);
  font-size: 0.75rem;
  pointer-events: none;
}

.altimetrie-inspect__row {
  display: flex;
  justify-content: space-between;
  gap: 16px;
}

.altimetrie-inspect__row + .altimetrie-inspect__row {
  margin-top: 2px;
}

.altimetrie-inspect__label {
  color: rgba(var(--v-theme-on-surface), 0.6);
}

.altimetrie-inspect__value {
  font-weight: 600;
  color: rgba(var(--v-theme-on-surface), 0.92);
}
</style>
