/**
 * Store Pinia du client `hermes`.
 *
 * Il porte l'état du serveur (connexion, réveil), les datasets, les régions,
 * les polygones custom, les instances ORS et les artefacts auxiliaires. Toutes
 * les E/S passent par les commandes Tauri `hermes_*` : le store ne fait aucun
 * `fetch` et ne connaît ni URL ni jeton.
 *
 * Deux responsabilités transverses :
 * - **keep-alive** : pendant un travail actif (polling de build, démarrage
 *   d'ORS, file de détection), un ping est envoyé à la période
 *   `Hermes.Connexion.keepAliveSec`. Il ne tourne jamais en permanence, pour ne
 *   pas empêcher la veille du serveur.
 * - **dataset actif** : choisi dans la vue Hermes, persisté dans le paramètre
 *   caché `Hermes.Dataset.actif`. Les stores Communes/Tunnels le liront via
 *   `ensureReady`.
 */

import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

import {
  describeHermesStatus,
  parseHermesError,
  type HermesServerState,
  type HermesStatusDisplay,
} from '../utils/hermes'
import { useSettingsStore } from './settings'
import { useUiStore } from './ui'

// ─── Types (miroir des contrats IPC Rust, camelCase) ──────────────────

export interface HermesStatusResponse {
  status: string
}

export interface HermesDataset {
  id: string
  regions: string[] | null
  zones: string[] | null
  createdAt: string
  status: string
  sizeBytes: number
  orsVersion: string
  stale: boolean
  lastLogTail: string | null
}

export interface HermesIdStatus {
  id: string
  status: string
}

export interface HermesRegion {
  id: string
  bbox: [number, number, number, number]
  hasPoly: boolean
  geojson: GeoJSON.Geometry | null
  geofabrikId: string | null
  updatedAt: string | null
}

export interface HermesPolygon {
  id: string
  name: string
  description: string | null
  color: string | null
  geometry: GeoJSON.Polygon
  bbox: [number, number, number, number]
  createdAt: string
  updatedAt: string
  revision: number
}

export interface HermesAuxFile {
  name: string
  exists: boolean
  sizeBytes: number | null
  modifiedAt: string | null
}

export interface HermesAuxStatus {
  datasetId: string
  communes: HermesAuxFile
  tunnels: HermesAuxFile
  mm: HermesAuxFile
  preparing: boolean
  startedAt: string | null
  lastError: string | null
  loadedInMemory: boolean
  activeDataset: string | null
}

export interface HermesWakeResult {
  sent: boolean
  address: string
  mac: string
  port: number
}

export interface HermesPolygonInput {
  id: string
  name: string
  description?: string | null
  color?: string | null
  geometry: GeoJSON.Polygon
}

export interface HermesPolygonUpdate {
  name: string
  description?: string | null
  color?: string | null
  geometry: GeoJSON.Polygon
}

export interface HermesCreateDatasetPayload {
  name?: string | null
  regions?: string[] | null
  zones?: string[] | null
}

/** Réponse de `ensureReady`, consommée par les détections. */
export interface HermesReadiness {
  ok: boolean
  code: string | null
  message: string | null
}

export interface HermesWakeProgress {
  attempts: number
  elapsedSec: number
}

// ─── Constantes ───────────────────────────────────────────────────────

const DEFAULT_BASE_URL = 'http://10.15.58.131:9000/api/v1'
const DEFAULT_KEEP_ALIVE_SEC = 30
const DEFAULT_WAKE_TIMEOUT_SEC = 120
const BUILD_POLL_INTERVAL_MS = 5000
const WAKE_POLL_INTERVAL_MS = 3000
const ORS_POLL_INTERVAL_MS = 3000
const BUILD_BUDGET_MS = 45 * 60 * 1000
const ORS_READY_TIMEOUT_SEC = 120

function delay(ms: number): Promise<void> {
  return new Promise(resolve => window.setTimeout(resolve, ms))
}

export const useHermesStore = defineStore('hermes', () => {
  const settingsStore = useSettingsStore()
  const ui = useUiStore()

  // ─── State ─────────────────────────────────────────────────────────
  const server = ref<HermesServerState>('unknown')
  const baseUrl = ref<string>(DEFAULT_BASE_URL)
  const lastPingAt = ref<string | null>(null)
  const lastError = ref<string | null>(null)
  const lastErrorCode = ref<string | null>(null)
  const datasets = ref<HermesDataset[]>([])
  const regions = ref<HermesRegion[]>([])
  const polygons = ref<HermesPolygon[]>([])
  const instances = ref<HermesIdStatus[]>([])
  const activeDatasetId = ref<string | null>(null)
  const activeAux = ref<HermesAuxStatus | null>(null)
  const loading = ref(false)
  const datasetsLoading = ref(false)
  const regionsLoading = ref(false)
  const polygonsLoading = ref(false)
  const instancesLoading = ref(false)
  const auxLoading = ref(false)
  const wakeProgress = ref<HermesWakeProgress | null>(null)
  const pollingDatasetId = ref<string | null>(null)
  const keepAliveActive = ref(false)
  const orsHealth = ref<string | null>(null)
  const orsStarting = ref(false)

  let activityCount = 0
  let activityGeneration = 0
  let keepAliveTimer: number | null = null
  let pollToken = 0
  let pollingActive = false
  let auxToken = 0

  // ─── Getters ───────────────────────────────────────────────────────

  const activeDataset = computed<HermesDataset | null>(
    () =>
      datasets.value.find(dataset => dataset.id === activeDatasetId.value) ??
      null,
  )

  const readyDatasets = computed<HermesDataset[]>(() =>
    datasets.value.filter(dataset => dataset.status === 'ready'),
  )

  const activeInstance = computed<HermesIdStatus | null>(
    () => instances.value[0] ?? null,
  )

  const statusDisplay = computed<HermesStatusDisplay>(() =>
    describeHermesStatus(
      server.value,
      activeDataset.value,
      lastError.value,
      baseUrl.value,
    ),
  )

  /**
   * Le dataset actif est-il prêt pour la détection demandée ? Suppose que les
   * artefacts aux ont été chargés (`ensureReady` les rafraîchit).
   */
  function isReady(kind: 'communes' | 'tunnels'): boolean {
    const dataset = activeDataset.value
    const aux = activeAux.value
    if (
      server.value !== 'online' ||
      !dataset ||
      dataset.status !== 'ready' ||
      !aux ||
      aux.datasetId !== dataset.id
    ) {
      return false
    }
    if (kind === 'communes') return aux.communes.exists
    return aux.tunnels.exists && aux.mm.exists
  }

  // ─── Helpers internes ──────────────────────────────────────────────

  function recordError(error: unknown): void {
    const parsed = parseHermesError(error)
    lastError.value = parsed.message
    lastErrorCode.value = parsed.code
  }

  async function readSettingNumber(
    path: string,
    fallback: number,
  ): Promise<number> {
    try {
      const value = Number(await settingsStore.getSettingValue(path))
      return Number.isFinite(value) ? value : fallback
    } catch {
      return fallback
    }
  }

  async function readSettingString(
    path: string,
    fallback: string,
  ): Promise<string> {
    try {
      const value = await settingsStore.getSettingValue(path)
      return typeof value === 'string' && value.trim() ? value.trim() : fallback
    } catch {
      return fallback
    }
  }

  function upsertDataset(dataset: HermesDataset): void {
    const index = datasets.value.findIndex(
      candidate => candidate.id === dataset.id,
    )
    if (index >= 0) {
      datasets.value[index] = dataset
    } else {
      datasets.value.push(dataset)
    }
  }

  // ─── Activité et keep-alive ────────────────────────────────────────

  /**
   * Démarre (ou rejoint) une période d'activité. Le ping de maintien n'est
   * lancé qu'au premier appel, et seulement si `keepAliveSec > 0`.
   */
  async function beginActivity(): Promise<void> {
    activityCount += 1
    if (activityCount !== 1) return

    const generation = ++activityGeneration
    const seconds = await readSettingNumber(
      'Hermes.Connexion.keepAliveSec',
      DEFAULT_KEEP_ALIVE_SEC,
    )
    if (generation !== activityGeneration || seconds <= 0) {
      return
    }

    keepAliveActive.value = true
    keepAliveTimer = window.setInterval(() => {
      void invoke('hermes_ping').catch(() => {
        /* un ping manqué n'interrompt pas le travail en cours */
      })
    }, seconds * 1000)
  }

  function endActivity(): void {
    activityCount = Math.max(0, activityCount - 1)
    if (activityCount > 0) return

    activityGeneration += 1
    if (keepAliveTimer !== null) {
      window.clearInterval(keepAliveTimer)
      keepAliveTimer = null
    }
    keepAliveActive.value = false
  }

  // ─── Serveur ───────────────────────────────────────────────────────

  async function ping(): Promise<boolean> {
    try {
      await invoke<HermesStatusResponse>('hermes_ping')
      server.value = 'online'
      lastPingAt.value = new Date().toISOString()
      lastError.value = null
      lastErrorCode.value = null
      return true
    } catch (error) {
      recordError(error)
      server.value = 'offline'
      return false
    }
  }

  async function loadBaseUrl(): Promise<void> {
    baseUrl.value = await readSettingString(
      'Hermes.Connexion.baseUrl',
      DEFAULT_BASE_URL,
    )
  }

  async function loadActiveDataset(): Promise<void> {
    const value = await readSettingString('Hermes.Dataset.actif', '')
    activeDatasetId.value = value || null
  }

  /** Ping, listes de datasets/instances, artefacts du dataset actif. */
  async function refreshServer(): Promise<void> {
    if (loading.value) return
    loading.value = true
    try {
      await Promise.all([loadBaseUrl(), loadActiveDataset()])
      const online = await ping()
      if (!online) return
      await Promise.all([loadDatasets(), loadInstances()])
      if (activeDatasetId.value) {
        await loadAux(activeDatasetId.value)
      }
    } finally {
      loading.value = false
    }
  }

  /**
   * Envoie le paquet WoL puis interroge `hermes_ping` toutes les 3 s jusqu'à
   * `Hermes.Reveil.delaiMaxSec`. Le réveil est manuel : aucune autre action ne
   * l'appelle automatiquement.
   */
  async function wake(): Promise<boolean> {
    const timeoutSec = await readSettingNumber(
      'Hermes.Reveil.delaiMaxSec',
      DEFAULT_WAKE_TIMEOUT_SEC,
    )
    server.value = 'waking'
    lastError.value = null
    lastErrorCode.value = null
    wakeProgress.value = { attempts: 0, elapsedSec: 0 }

    try {
      await invoke<HermesWakeResult>('hermes_wake')
    } catch (error) {
      recordError(error)
      server.value = 'offline'
      wakeProgress.value = null
      return false
    }

    const startedAt = Date.now()
    const deadline = startedAt + timeoutSec * 1000
    let attempts = 0
    while (Date.now() < deadline) {
      attempts += 1
      wakeProgress.value = {
        attempts,
        elapsedSec: Math.round((Date.now() - startedAt) / 1000),
      }
      try {
        await invoke<HermesStatusResponse>('hermes_ping')
        server.value = 'online'
        lastPingAt.value = new Date().toISOString()
        wakeProgress.value = null
        await refreshServer()
        return true
      } catch {
        // Serveur encore absent : nouvelle tentative après 3 s.
      }
      await delay(WAKE_POLL_INTERVAL_MS)
    }

    server.value = 'offline'
    lastError.value = `Serveur hermes indisponible après ${timeoutSec} s.`
    lastErrorCode.value = 'HERMES_TIMEOUT'
    wakeProgress.value = null
    return false
  }

  // ─── Datasets ──────────────────────────────────────────────────────

  async function loadDatasets(): Promise<void> {
    datasetsLoading.value = true
    try {
      const list = await invoke<HermesDataset[]>('hermes_list_datasets')
      datasets.value = list

      if (
        activeDatasetId.value &&
        !list.some(dataset => dataset.id === activeDatasetId.value)
      ) {
        // Le dataset actif a été supprimé côté serveur : on nettoie le choix.
        await setActiveDataset(null)
        return
      }
      if (!activeDatasetId.value) {
        const ready = list.filter(dataset => dataset.status === 'ready')
        // Un seul dataset prêt : choix automatique, sans ambiguïté.
        if (ready.length === 1) {
          await setActiveDataset(ready[0].id)
        }
      }

      // Un build peut être en cours au moment de l'ouverture de la vue : on
      // reprend son suivi sans attendre une action utilisateur.
      const preparing = list.find(dataset => dataset.status === 'preparing')
      if (preparing && pollingDatasetId.value === null) {
        void pollDataset(preparing.id)
      }
    } catch (error) {
      recordError(error)
    } finally {
      datasetsLoading.value = false
    }
  }

  async function setActiveDataset(datasetId: string | null): Promise<void> {
    const previous = activeDatasetId.value
    activeDatasetId.value = datasetId
    try {
      await settingsStore.updateSetting('Hermes.Dataset.actif', datasetId ?? '')
    } catch (error) {
      activeDatasetId.value = previous
      recordError(error)
      throw error
    }

    if (datasetId) {
      await loadAux(datasetId)
    } else {
      auxToken += 1
      activeAux.value = null
    }
  }

  async function createDataset(
    payload: HermesCreateDatasetPayload,
  ): Promise<HermesIdStatus> {
    try {
      const result = await invoke<HermesIdStatus>('hermes_create_dataset', {
        name: payload.name ?? null,
        regions: payload.regions ?? null,
        zones: payload.zones ?? null,
      })
      await loadDatasets()
      if (
        result.status === 'preparing' &&
        pollingDatasetId.value !== result.id
      ) {
        void pollDataset(result.id)
      }
      return result
    } catch (error) {
      recordError(error)
      throw error
    }
  }

  async function deleteDataset(datasetId: string): Promise<void> {
    try {
      await invoke('hermes_delete_dataset', { datasetId })
      if (pollingDatasetId.value === datasetId) {
        cancelPolling()
      }
      if (activeDatasetId.value === datasetId) {
        await setActiveDataset(null)
      }
      await Promise.all([loadDatasets(), loadInstances()])
    } catch (error) {
      recordError(error)
      throw error
    }
  }

  /**
   * Suit un build de dataset jusqu'à `ready` ou `failed`. Une erreur ponctuelle
   * n'interrompt pas le suivi : le build continue côté serveur. Le budget est
   * de 45 min (contrat).
   */
  async function pollDataset(datasetId: string): Promise<void> {
    const token = ++pollToken
    pollingActive = true
    pollingDatasetId.value = datasetId
    void beginActivity()

    const deadline = Date.now() + BUILD_BUDGET_MS
    try {
      while (token === pollToken && Date.now() < deadline) {
        await delay(BUILD_POLL_INTERVAL_MS)
        if (token !== pollToken) return

        try {
          const dataset = await invoke<HermesDataset>('hermes_get_dataset', {
            datasetId,
          })
          upsertDataset(dataset)
          // Une réponse valide efface l'erreur transitoire d'un polling
          // précédent : le build a continué côté serveur.
          lastError.value = null
          lastErrorCode.value = null
          if (dataset.status === 'ready') {
            ui.showSuccess(`Dataset ${dataset.id} prêt.`)
            return
          }
          if (dataset.status === 'failed') {
            const tail = dataset.lastLogTail
              ? ` : ${dataset.lastLogTail}`
              : ''
            ui.showError(`Build du dataset ${dataset.id} échoué${tail}`)
            return
          }
        } catch (error) {
          recordError(error)
        }
      }

      if (token === pollToken) {
        ui.showWarning(
          `Build du dataset ${datasetId} toujours en cours après 45 min.`,
        )
      }
    } finally {
      endActivity()
      if (token === pollToken) {
        pollingActive = false
        pollingDatasetId.value = null
      }
    }
  }

  /** Stoppe le suivi du build courant ; le build continue côté serveur. */
  function cancelPolling(): void {
    if (!pollingActive) return
    pollToken += 1
    pollingActive = false
    pollingDatasetId.value = null
  }

  // ─── Artefacts auxiliaires ─────────────────────────────────────────

  async function loadAux(datasetId: string): Promise<void> {
    const token = ++auxToken
    auxLoading.value = true
    try {
      const aux = await invoke<HermesAuxStatus>('hermes_aux_status', {
        datasetId,
      })
      if (token === auxToken) {
        activeAux.value = aux
      }
    } catch (error) {
      if (token === auxToken) {
        activeAux.value = null
        recordError(error)
      }
    } finally {
      if (token === auxToken) {
        auxLoading.value = false
      }
    }
  }

  /**
   * Vérifie que le serveur, le dataset actif et les artefacts requis sont
   * disponibles **avant** une détection. Utilisé par les stores Communes et
   * Tunnels (étapes 2 et 3).
   */
  async function ensureReady(
    kind: 'communes' | 'tunnels',
  ): Promise<HermesReadiness> {
    if (server.value !== 'online') {
      const online = await ping()
      if (!online) {
        return {
          ok: false,
          code: 'HERMES_OFFLINE',
          message: 'Serveur hermes indisponible (ZeroTier ou veille).',
        }
      }
    }

    // Le statut d'un build peut avoir changé depuis le dernier chargement :
    // on relit la liste avant de conclure à un dataset non prêt.
    await loadDatasets()
    if (!activeDatasetId.value) {
      await loadActiveDataset()
    }
    if (!activeDatasetId.value) {
      return {
        ok: false,
        code: 'HERMES_NO_DATASET',
        message: 'Aucun dataset actif. Choisissez-en un dans la vue Hermes.',
      }
    }

    const dataset = activeDataset.value
    if (!dataset) {
      return {
        ok: false,
        code: 'HERMES_NO_DATASET',
        message: `Dataset actif introuvable (${activeDatasetId.value}).`,
      }
    }
    if (dataset.status !== 'ready') {
      return {
        ok: false,
        code: 'HERMES_CONFLICT',
        message: `Dataset ${dataset.id} non prêt (${dataset.status}).`,
      }
    }

    await loadAux(dataset.id)
    const aux = activeAux.value
    if (!aux || aux.datasetId !== dataset.id) {
      return {
        ok: false,
        code: 'HERMES_NOT_PREPARED',
        message: `Artefacts illisibles pour le dataset ${dataset.id}.`,
      }
    }
    if (kind === 'communes' && !aux.communes.exists) {
      return {
        ok: false,
        code: 'HERMES_NOT_PREPARED',
        message: `Communes non préparées pour ${dataset.id}. Contactez l'administrateur.`,
      }
    }
    if (kind === 'tunnels' && !(aux.tunnels.exists && aux.mm.exists)) {
      return {
        ok: false,
        code: 'HERMES_NOT_PREPARED',
        message: `Tunnels non préparés pour ${dataset.id}. Contactez l'administrateur.`,
      }
    }

    return { ok: true, code: null, message: null }
  }

  // ─── Régions et polygones ──────────────────────────────────────────

  async function loadRegions(): Promise<void> {
    regionsLoading.value = true
    try {
      regions.value = await invoke<HermesRegion[]>('hermes_list_regions')
    } catch (error) {
      recordError(error)
    } finally {
      regionsLoading.value = false
    }
  }

  async function loadPolygons(): Promise<void> {
    polygonsLoading.value = true
    try {
      polygons.value = await invoke<HermesPolygon[]>('hermes_list_polygons')
    } catch (error) {
      recordError(error)
    } finally {
      polygonsLoading.value = false
    }
  }

  /**
   * Charge les données nécessaires au calcul de la couverture du dataset
   * actif sur la carte de l'Accueil : datasets, régions et polygones.
   *
   * `loadDatasets` est appelé même sans dataset actif : il peut sélectionner
   * automatiquement l'unique dataset prêt. Sans dataset actif ensuite, aucun
   * chargement de géométries n'est fait. Les erreurs sont absorbées par les
   * chargements unitaires : la carte reste simplement sans couverture si le
   * serveur est indisponible.
   */
  async function ensureCoverageData(): Promise<void> {
    await loadActiveDataset()
    await loadDatasets()
    if (!activeDatasetId.value) return
    await Promise.all([loadRegions(), loadPolygons()])
  }

  async function createPolygon(
    input: HermesPolygonInput,
  ): Promise<HermesPolygon> {
    try {
      const polygon = await invoke<HermesPolygon>('hermes_create_polygon', {
        id: input.id,
        name: input.name,
        description: input.description ?? null,
        color: input.color ?? null,
        geometry: input.geometry,
      })
      await loadPolygons()
      return polygon
    } catch (error) {
      recordError(error)
      throw error
    }
  }

  async function updatePolygon(
    polygonId: string,
    input: HermesPolygonUpdate,
  ): Promise<HermesPolygon> {
    try {
      const polygon = await invoke<HermesPolygon>('hermes_update_polygon', {
        polygonId,
        name: input.name,
        description: input.description ?? null,
        color: input.color ?? null,
        geometry: input.geometry,
      })
      await loadPolygons()
      return polygon
    } catch (error) {
      recordError(error)
      throw error
    }
  }

  async function deletePolygon(polygonId: string): Promise<void> {
    try {
      await invoke('hermes_delete_polygon', { polygonId })
      await Promise.all([loadPolygons(), loadDatasets()])
    } catch (error) {
      recordError(error)
      throw error
    }
  }

  // ─── Instances ORS ─────────────────────────────────────────────────

  async function loadInstances(): Promise<void> {
    instancesLoading.value = true
    try {
      instances.value = await invoke<HermesIdStatus[]>('hermes_list_instances')
    } catch (error) {
      instances.value = []
      console.warn('[hermes] Instances ORS illisibles :', error)
    } finally {
      instancesLoading.value = false
    }
  }

  /** Attend `status=ready` sur `/ors/v2/health` (poll 3 s, timeout 120 s). */
  async function pollOrsHealth(timeoutSec: number): Promise<boolean> {
    void beginActivity()
    const deadline = Date.now() + timeoutSec * 1000
    try {
      while (Date.now() < deadline) {
        try {
          const response = await invoke<HermesStatusResponse>(
            'hermes_ors_health',
          )
          orsHealth.value = response.status
          if (response.status === 'ready') return true
        } catch {
          orsHealth.value = 'unreachable'
        }
        await delay(ORS_POLL_INTERVAL_MS)
      }
      return false
    } finally {
      endActivity()
    }
  }

  async function startInstance(datasetId: string): Promise<boolean> {
    orsStarting.value = true
    orsHealth.value = null
    try {
      await invoke<HermesIdStatus>('hermes_start_instance', { datasetId })
      await loadInstances()
      const ready = await pollOrsHealth(ORS_READY_TIMEOUT_SEC)
      if (ready) {
        ui.showSuccess(`ORS prêt pour ${datasetId}.`)
      } else {
        ui.showWarning(
          `ORS n'a pas confirmé « ready » après ${ORS_READY_TIMEOUT_SEC} s.`,
        )
      }
      return ready
    } catch (error) {
      recordError(error)
      throw error
    } finally {
      orsStarting.value = false
    }
  }

  async function stopInstance(datasetId: string): Promise<void> {
    try {
      await invoke<HermesIdStatus>('hermes_stop_instance', { datasetId })
      orsHealth.value = null
      await loadInstances()
      // Le serveur décharge les index communes/tunnels sur stop : on relit
      // l'état aux du dataset actif pour que le diagnostic reste exact.
      if (activeDatasetId.value) {
        await loadAux(activeDatasetId.value)
      }
    } catch (error) {
      recordError(error)
      throw error
    }
  }

  // ─── Return ────────────────────────────────────────────────────────

  return {
    // State
    server,
    baseUrl,
    lastPingAt,
    lastError,
    lastErrorCode,
    datasets,
    regions,
    polygons,
    instances,
    activeDatasetId,
    activeAux,
    loading,
    datasetsLoading,
    regionsLoading,
    polygonsLoading,
    instancesLoading,
    auxLoading,
    wakeProgress,
    pollingDatasetId,
    keepAliveActive,
    orsHealth,
    orsStarting,
    // Getters
    activeDataset,
    readyDatasets,
    activeInstance,
    statusDisplay,
    isReady,
    // Actions
    refreshServer,
    ping,
    wake,
    loadDatasets,
    loadActiveDataset,
    loadRegions,
    loadPolygons,
    loadInstances,
    loadAux,
    ensureCoverageData,
    createDataset,
    deleteDataset,
    pollDataset,
    cancelPolling,
    startInstance,
    stopInstance,
    pollOrsHealth,
    createPolygon,
    updatePolygon,
    deletePolygon,
    setActiveDataset,
    ensureReady,
    beginActivity,
    endActivity,
  }
})
