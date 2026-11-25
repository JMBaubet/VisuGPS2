/**
 * Store Pinia du client `hermes`.
 *
 * Il porte l'état du serveur (connexion, réveil), les datasets, les régions,
 * les polygones custom, le dataset servi et les artefacts auxiliaires. Toutes
 * les E/S passent par les commandes Tauri `hermes_*` : le store ne fait aucun
 * `fetch` et ne connaît ni URL ni jeton.
 *
 * Trois responsabilités transverses :
 * - **moniteur de santé** : `GET /ping` est envoyé en permanence
 *   (`Hermes.Connexion.pingApiSec`, 3–60 s) pour détecter l'arrêt du service.
 *   Quand l'API ne répond pas, un ping **système** (ICMP) unique départage
 *   « service arrêté » et « serveur injoignable » (Tailscale, veille).
 * - **keep-alive** : pendant un travail actif (polling de build, de serving ou
 *   d'aux), un `GET /ping` complémentaire est envoyé à la période
 *   `Hermes.Connexion.keepAliveSec`.
 * - **dataset servi** : la sélection est persistée **côté serveur** via
 *   `PUT /serving/{id}` ; `GET /serving` fournit l'état (`none`, `waiting`,
 *   `starting`, `active`, `stopped`, `failed`) et `DELETE /serving` arrête ORS
 *   et efface la sélection. Les stores Communes/Tunnels lisent l'état via
 *   `ensureReady`.
 */

import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

import {
  auxFileReady,
  describeHermesStatus,
  isDatasetGraphReady,
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
  /** Nom d'affichage fourni par le serveur (repli sur `id` s'il est absent). */
  name: string | null
  regions: string[] | null
  zones: string[] | null
  createdAt: string
  /** Champ historique : `preparing`, `ready` ou `failed`. */
  status: string
  /** État de référence v1.9 (initializing, building, ready, degraded*, …). */
  state: string | null
  stateLabel: string | null
  phase: string | null
  /** Présence des artefacts `pbf`, `ors`, `communes`, `tunnels`, `mm`. */
  artifacts: Record<string, unknown> | null
  runId: string | null
  sizeBytes: number
  orsVersion: string
  stale: boolean
  lastLogTail: string | null
}

/** État du dataset servi (`GET /serving`). */
export interface HermesServingStatus {
  datasetId: string | null
  state: string
  stateLabel: string | null
  datasetState: string | null
  since: string | null
  reason: string | null
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
  /** Champ serveur hérité : plus traité par l'IHM (sera retiré côté API). */
  color: string | null
  geometry: GeoJSON.Polygon
  bbox: [number, number, number, number]
  createdAt: string
  updatedAt: string
  revision: number
}

export interface HermesAuxFile {
  name: string
  /** `pending`, `building`, `ready` ou `failed` (contrat v1.10). */
  state: string
  exists: boolean
  sizeBytes: number | null
  modifiedAt: string | null
  completedAt: string | null
  error: string | null
  runId: string | null
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

/** Réponse de `POST /datasets/{id}/aux/prepare` (202, tâche de fond). */
export interface HermesAuxPrepareResponse {
  datasetId: string
  status: string
  startedAt: string
  message: string | null
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
  geometry: GeoJSON.Polygon
}

export interface HermesPolygonUpdate {
  name: string
  description?: string | null
  geometry: GeoJSON.Polygon
}

/** Création d'un dataset : uniquement des polygones (zones) existants. */
export interface HermesCreateDatasetPayload {
  name?: string | null
  zones: string[]
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

/**
 * Repli d'affichage uniquement : l'adresse réellement utilisée est lue dans
 * `Hermes.Connexion.baseUrl` (par mode d'exécution). Le défaut du schéma est
 * `http://100.118.62.10:9000/api/v1` ; un mode de test peut pointer vers
 * `100.118.62.12` sans changement de code.
 */
const DEFAULT_BASE_URL = 'http://100.118.62.10:9000/api/v1'
const DEFAULT_KEEP_ALIVE_SEC = 30
const DEFAULT_WAKE_TIMEOUT_SEC = 120
/** Période du ping API (service) : définie par `Hermes.Connexion.pingApiSec`. */
const API_PING_DEFAULT_SEC = 10
const API_PING_MIN_SEC = 3
const API_PING_MAX_SEC = 60
const BUILD_POLL_INTERVAL_MS = 5000
const WAKE_POLL_INTERVAL_MS = 3000
const AUX_POLL_INTERVAL_MS = 3000
const SERVING_POLL_INTERVAL_MS = 3000
const BUILD_BUDGET_MS = 45 * 60 * 1000
const AUX_BUDGET_MS = 45 * 60 * 1000
const SERVING_BUDGET_MS = 45 * 60 * 1000
/** États de serving qui nécessitent un suivi par polling. */
const SERVING_TRANSITIONAL_STATES = new Set(['waiting', 'starting'])

function delay(ms: number): Promise<void> {
  return new Promise(resolve => window.setTimeout(resolve, ms))
}

export const useHermesStore = defineStore('hermes', () => {
  const settingsStore = useSettingsStore()
  const ui = useUiStore()

  // ─── State ─────────────────────────────────────────────────────────
  /** Machine + réseau (ping système), indépendant du service `hermes-api`. */
  const server = ref<HermesServerState>('unknown')
  /** Santé de l'API hermes (le service), `null` tant qu'elle n'a pas été testée. */
  const apiOnline = ref<boolean | null>(null)
  const baseUrl = ref<string>(DEFAULT_BASE_URL)
  const lastPingAt = ref<string | null>(null)
  /** Dernier ping système réussi (diagnostic). */
  const systemPingAt = ref<string | null>(null)
  /** Popup d'alerte « serveur injoignable », une fois par panne. */
  const systemAlertOpen = ref(false)
  /** Popup d'alerte « service arrêté » (machine joignable, API muette). */
  const serviceAlertOpen = ref(false)
  const lastError = ref<string | null>(null)
  const lastErrorCode = ref<string | null>(null)
  const datasets = ref<HermesDataset[]>([])
  const regions = ref<HermesRegion[]>([])
  const polygons = ref<HermesPolygon[]>([])
  /** Dataset servi et état de l'instance ORS (source de vérité serveur). */
  const serving = ref<HermesServingStatus | null>(null)
  const activeAux = ref<HermesAuxStatus | null>(null)
  const loading = ref(false)
  const datasetsLoading = ref(false)
  const regionsLoading = ref(false)
  const polygonsLoading = ref(false)
  const auxLoading = ref(false)
  const wakeProgress = ref<HermesWakeProgress | null>(null)
  const pollingDatasetId = ref<string | null>(null)
  const auxPreparing = ref(false)
  const auxPreparingDatasetId = ref<string | null>(null)
  const keepAliveActive = ref(false)
  const servingPolling = ref(false)
  const servingBusy = ref(false)
  /** Dataset en cours de reconstruction (DELETE puis POST), ou `null`. */
  const rebuildingDatasetId = ref<string | null>(null)

  let activityCount = 0
  let activityGeneration = 0
  let keepAliveTimer: number | null = null
  let pollToken = 0
  let pollingActive = false
  let auxToken = 0
  let auxPrepareToken = 0
  let servingToken = 0
  let healthMonitorActive = false
  let healthMonitorToken = 0
  /** Une alerte a déjà été émise pour la panne courante. */
  let systemAlertArmed = true
  let serviceAlertArmed = true

  // ─── Getters ───────────────────────────────────────────────────────

  /** Identifiant du dataset servi, persisté côté serveur (`/serving`). */
  const activeDatasetId = computed<string | null>(
    () => serving.value?.datasetId ?? null,
  )

  const activeDataset = computed<HermesDataset | null>(
    () =>
      datasets.value.find(dataset => dataset.id === activeDatasetId.value) ??
      null,
  )

  const readyDatasets = computed<HermesDataset[]>(() =>
    datasets.value.filter(dataset => dataset.status === 'ready'),
  )

  /**
   * Une construction est en cours (build d'un dataset ou chaîne aux) : la carte
   * Serveur désactive Arrêter/Démarrer le service et affiche l'attente de fin
   * de construction, le serveur arrêtant ORS pendant un build.
   */
  const constructionInProgress = computed<boolean>(() => {
    if (
      pollingDatasetId.value !== null ||
      auxPreparing.value ||
      activeAux.value?.preparing === true ||
      rebuildingDatasetId.value !== null
    ) {
      return true
    }
    return datasets.value.some(
      dataset =>
        dataset.status === 'preparing' ||
        dataset.state === 'initializing' ||
        dataset.state === 'building',
    )
  })

  const statusDisplay = computed<HermesStatusDisplay>(() =>
    describeHermesStatus(
      server.value,
      activeDataset.value,
      lastError.value,
      baseUrl.value,
      apiOnline.value,
    ),
  )

  /**
   * Le dataset servi est-il prêt pour la détection demandée ? Suppose que les
   * artefacts aux ont été chargés (`ensureReady` les rafraîchit). Le service
   * (API) doit répondre, indépendamment de la machine.
   */
  function isReady(kind: 'communes' | 'tunnels'): boolean {
    const dataset = activeDataset.value
    const aux = activeAux.value
    if (
      apiOnline.value !== true ||
      !dataset ||
      !isDatasetGraphReady(dataset) ||
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
    const networkError =
      parsed.code === 'HERMES_OFFLINE' || parsed.code === 'HERMES_TIMEOUT'
    // Après un réveil échoué, le message « Perte du serveur » prime sur le
    // bruit réseau des chargements concurrents (polygones, régions…).
    if (server.value === 'lost' && networkError) {
      return
    }
    // Machine joignable (ou pas encore jugée) et API muette : c'est le service
    // qui est arrêté, pas nécessairement le serveur.
    if (
      (server.value === 'online' || server.value === 'unknown') &&
      networkError
    ) {
      lastError.value = "Service hermes arrêté : l'API ne répond pas."
      lastErrorCode.value = parsed.code
      return
    }
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

  /**
   * Relit `GET /datasets/{id}` et met à jour la liste. Utilisé après la fin des
   * aux : le serveur ne fait passer `state` à `ready`/`degraded*` qu'à ce
   * moment, et la carte doit refléter ce changement sans quitter la vue.
   */
  async function refreshDatasetState(datasetId: string): Promise<void> {
    try {
      const dataset = await invoke<HermesDataset>('hermes_get_dataset', {
        datasetId,
      })
      upsertDataset(dataset)
    } catch {
      // On conserve l'état précédent : un rafraîchissement ultérieur corrigera.
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

  // ─── Serveur (machine) et service (API) ───────────────────────────

  /**
   * `GET /ping` : teste l'**API** (le service), pas la machine. Une réponse
   * prouve aussi que la machine est joignable, même si l'ICMP est filtré.
   */
  async function ping(): Promise<boolean> {
    try {
      await invoke<HermesStatusResponse>('hermes_ping')
      apiOnline.value = true
      lastPingAt.value = new Date().toISOString()
      lastError.value = null
      lastErrorCode.value = null
      // Service et machine répondent : plus aucune alerte, réarmement.
      systemAlertOpen.value = false
      serviceAlertOpen.value = false
      serviceAlertArmed = true
      if (server.value !== 'waking') {
        server.value = 'online'
        systemAlertArmed = true
      }
      return true
    } catch (error) {
      apiOnline.value = false
      recordError(error)
      return false
    }
  }

  /** Période du ping API, bornée 3–60 s (défaut 10 s). */
  function healthPingIntervalMs(): number {
    const raw = Number(
      settingsStore.getParamDef('Hermes.Connexion.pingApiSec')?.value,
    )
    const seconds = Number.isFinite(raw)
      ? Math.min(API_PING_MAX_SEC, Math.max(API_PING_MIN_SEC, raw))
      : API_PING_DEFAULT_SEC
    return seconds * 1000
  }

  /**
   * Ping **système** (ICMP) : teste la machine et Tailscale. Il n'est appelé
   * que lorsque le ping API échoue, pour lever le doute entre un service
   * arrêté (machine joignable) et un serveur injoignable.
   *
   * L'alerte n'est ouverte qu'une fois par panne ; elle se réarme au prochain
   * succès (API ou système).
   */
  async function systemPing(): Promise<boolean> {
    try {
      await invoke<HermesStatusResponse>('hermes_system_ping')
      systemPingAt.value = new Date().toISOString()
      if (server.value !== 'waking') {
        server.value = 'online'
      }
      // Machine joignable alors que l'API vient d'échouer : service arrêté.
      systemAlertOpen.value = false
      systemAlertArmed = true
      if (apiOnline.value === false && serviceAlertArmed) {
        serviceAlertOpen.value = true
        serviceAlertArmed = false
      }
      return true
    } catch {
      if (server.value === 'waking') {
        return false
      }
      if (server.value !== 'lost') {
        server.value = 'offline'
      }
      // Le diagnostic est « serveur », pas « service ».
      serviceAlertOpen.value = false
      serviceAlertArmed = true
      if (systemAlertArmed) {
        systemAlertOpen.value = true
        systemAlertArmed = false
      }
      return false
    }
  }

  /**
   * Moniteur de santé permanent : `GET /ping` à chaque période. Si l'API ne
   * répond pas, un ping système départage « service arrêté » et « serveur
   * injoignable ». Sur retour de l'API, l'état du service (`/serving`) est
   * relu.
   */
  async function runHealthMonitor(token: number): Promise<void> {
    while (healthMonitorActive && token === healthMonitorToken) {
      const startedAt = Date.now()
      const wasApiDown = apiOnline.value === false
      const apiOk = await ping()
      if (apiOk) {
        if (wasApiDown) {
          await loadServing()
        }
      } else {
        await systemPing()
      }
      if (!healthMonitorActive || token !== healthMonitorToken) return
      // Cadence fixe : le temps du ping ne s'ajoute pas à la période.
      const wait = Math.max(
        1000,
        healthPingIntervalMs() - (Date.now() - startedAt),
      )
      await delay(wait)
    }
  }

  /** Démarre le ping API permanent (toute la durée de l'application). */
  function startHealthMonitor(): void {
    if (healthMonitorActive) return
    healthMonitorActive = true
    const token = ++healthMonitorToken
    void runHealthMonitor(token)
  }

  function stopHealthMonitor(): void {
    healthMonitorActive = false
    healthMonitorToken += 1
  }

  function dismissSystemAlert(): void {
    systemAlertOpen.value = false
  }

  function dismissServiceAlert(): void {
    serviceAlertOpen.value = false
  }

  async function loadBaseUrl(): Promise<void> {
    baseUrl.value = await readSettingString(
      'Hermes.Connexion.baseUrl',
      DEFAULT_BASE_URL,
    )
  }

  /**
   * Lit le dataset servi (`GET /serving`). Un 404 est traduit en état `none`
   * sans erreur globale : il signale un serveur antérieur à la route.
   */
  async function loadServing(): Promise<void> {
    try {
      serving.value = await invoke<HermesServingStatus>('hermes_get_serving')
    } catch (error) {
      const parsed = parseHermesError(error)
      if (parsed.code === 'HERMES_NOT_FOUND') {
        serving.value = {
          datasetId: null,
          state: 'none',
          stateLabel: 'Service indisponible',
          datasetState: null,
          since: null,
          reason: 'Route /serving absente du serveur (version antérieure à v1.9).',
        }
        return
      }
      recordError(error)
    }
  }

  /** Ping, dataset servi, datasets et artefacts aux du dataset servi. */
  async function refreshServer(): Promise<void> {
    if (loading.value) return
    loading.value = true
    try {
      await loadBaseUrl()
      const online = await ping()
      if (!online) return
      await Promise.all([loadServing(), loadDatasets()])
      if (activeDatasetId.value) {
        await loadAux(activeDatasetId.value)
      }
      if (
        serving.value &&
        SERVING_TRANSITIONAL_STATES.has(serving.value.state)
      ) {
        void pollServing()
      }
    } finally {
      loading.value = false
    }
  }

  /**
   * Envoie le paquet WoL puis interroge le **ping système** toutes les 3 s
   * jusqu'à `Hermes.Reveil.delaiMaxSec` (repli sur l'API si l'ICMP est filtré).
   * Appelé **systématiquement au lancement de l'application** (le serveur se
   * met en veille par défaut), puis manuellement par les boutons Réveiller.
   */
  async function wake(): Promise<boolean> {
    // Une trame est déjà en cours : on ne lance pas de seconde boucle.
    if (server.value === 'waking') return false
    // L'état est posé avant le premier await : le moniteur de santé ne déclare
    // pas le serveur injoignable pendant le démarrage.
    server.value = 'waking'
    systemAlertOpen.value = false
    serviceAlertOpen.value = false
    lastError.value = null
    lastErrorCode.value = null
    wakeProgress.value = { attempts: 0, elapsedSec: 0 }

    const timeoutSec = await readSettingNumber(
      'Hermes.Reveil.delaiMaxSec',
      DEFAULT_WAKE_TIMEOUT_SEC,
    )

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
      let reachable = false
      try {
        await invoke<HermesStatusResponse>('hermes_system_ping')
        reachable = true
      } catch {
        try {
          await invoke<HermesStatusResponse>('hermes_ping')
          apiOnline.value = true
          reachable = true
        } catch {
          apiOnline.value = false
        }
      }
      if (reachable) {
        server.value = 'online'
        systemPingAt.value = new Date().toISOString()
        wakeProgress.value = null
        systemAlertOpen.value = false
        serviceAlertOpen.value = false
        systemAlertArmed = true
        serviceAlertArmed = true
        // La machine répond ; on relit l'état du service (l'API peut démarrer
        // quelques secondes après la machine).
        await refreshServer()
        return true
      }
      await delay(WAKE_POLL_INTERVAL_MS)
    }

    server.value = 'lost'
    lastError.value = `Perte du serveur hermes : le réveil a échoué après ${timeoutSec} s. Vérifiez l'alimentation, le service Tailscale et le service hermes-api.`
    lastErrorCode.value = 'HERMES_SERVER_LOST'
    wakeProgress.value = null
    systemAlertOpen.value = true
    systemAlertArmed = false
    serviceAlertOpen.value = false
    serviceAlertArmed = true
    return false
  }

  // ─── Datasets ──────────────────────────────────────────────────────

  async function loadDatasets(): Promise<void> {
    datasetsLoading.value = true
    try {
      const list = await invoke<HermesDataset[]>('hermes_list_datasets')
      datasets.value = list

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

  /**
   * Sélectionne le dataset servi côté serveur (`PUT /serving/{id}`). Le serveur
   * démarre ORS s'il est exploitable ; le store suit ensuite `GET /serving`.
   */
  async function selectServingDataset(datasetId: string): Promise<void> {
    if (!datasetId) return
    servingBusy.value = true
    try {
      await invoke('hermes_set_serving', { datasetId })
      await loadServing()
      if (activeDatasetId.value) {
        await loadAux(activeDatasetId.value)
      }
      if (
        serving.value &&
        SERVING_TRANSITIONAL_STATES.has(serving.value.state)
      ) {
        void pollServing()
      }
    } catch (error) {
      recordError(error)
      throw error
    } finally {
      servingBusy.value = false
    }
  }

  /** Arrête ORS et efface la sélection côté serveur (`DELETE /serving`). */
  async function stopServing(): Promise<void> {
    servingBusy.value = true
    try {
      await invoke('hermes_clear_serving')
      await loadServing()
      cancelServingPolling()
      auxToken += 1
      activeAux.value = null
    } catch (error) {
      recordError(error)
      throw error
    } finally {
      servingBusy.value = false
    }
  }

  /**
   * Démarre ORS pour le dataset sélectionné sans changer la sélection
   * (`POST /instances/{id}/start`). Utilisé par « Lancer » après un arrêt
   * volontaire, qui conserve la sélection côté serveur.
   */
  async function startServedDataset(): Promise<void> {
    const datasetId = activeDatasetId.value
    if (!datasetId) return
    servingBusy.value = true
    try {
      const result = await invoke<HermesIdStatus>('hermes_start_instance', {
        datasetId,
      })
      if (serving.value) {
        serving.value = {
          ...serving.value,
          state: result.status,
          stateLabel:
            result.status === 'starting'
              ? 'Démarrage en cours'
              : serving.value.stateLabel,
          reason: null,
          since: new Date().toISOString(),
        }
      }
      if (SERVING_TRANSITIONAL_STATES.has(result.status)) {
        void pollServing()
      } else {
        await loadServing()
      }
    } catch (error) {
      recordError(error)
      throw error
    } finally {
      servingBusy.value = false
    }
  }

  /**
   * Arrête ORS en conservant la sélection (`POST /instances/{id}/stop`) : la
   * carte garde le nom du dataset et « Lancer » peut le redémarrer.
   */
  async function stopServedDataset(): Promise<void> {
    const datasetId = activeDatasetId.value
    if (!datasetId) return
    servingBusy.value = true
    try {
      const result = await invoke<HermesIdStatus>('hermes_stop_instance', {
        datasetId,
      })
      cancelServingPolling()
      if (serving.value) {
        serving.value = {
          ...serving.value,
          state: result.status,
          stateLabel: 'Arrêté',
          reason: null,
          since: new Date().toISOString(),
        }
      }
      await loadServing()
    } catch (error) {
      recordError(error)
      throw error
    } finally {
      servingBusy.value = false
    }
  }

  /**
   * Un build arrête ORS côté serveur et expose le service en `waiting` : on
   * reflète cet arrêt immédiatement pour ne pas laisser « Actif » à l'écran
   * pendant le délai du premier polling (le serveur reste l'autorité et
   * corrigera l'état au prochain `GET /serving`).
   */
  function markServiceWaitingForBuild(): void {
    const current = serving.value
    if (!current?.datasetId) return
    if (
      current.state !== 'active' &&
      !SERVING_TRANSITIONAL_STATES.has(current.state)
    ) {
      return
    }
    serving.value = {
      ...current,
      state: 'waiting',
      stateLabel: 'Attente fin de construction',
      reason: 'construction en cours',
      since: new Date().toISOString(),
    }
  }

  async function createDataset(
    payload: HermesCreateDatasetPayload,
  ): Promise<HermesIdStatus> {
    try {
      const result = await invoke<HermesIdStatus>('hermes_create_dataset', {
        name: payload.name ?? null,
        zones: payload.zones,
      })
      // Un build arrête ORS côté serveur : on relit aussitôt `/serving` pour
      // afficher « En attente » plutôt que l'ancien état « Actif ».
      await Promise.all([loadDatasets(), loadServing()])
      markServiceWaitingForBuild()
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
      // Le contrat conserve la sélection par id ; on relit simplement l'état.
      await Promise.all([loadDatasets(), loadServing()])
    } catch (error) {
      recordError(error)
      throw error
    }
  }

  /**
   * Reconstruit un dataset en mode zones à partir de ses secteurs actuels.
   *
   * L'API n'expose pas de route de reconstruction : le flux documenté est
   * `DELETE /datasets/{id}` puis `POST /datasets` avec le **même nom**, ce qui
   * recrée le même identifiant. ORS est arrêté pendant l'opération ; si le
   * dataset était servi, le serveur conserve la sélection par id et le
   * redémarre à la fin du build.
   *
   * Refusé si une construction est déjà en cours : un `POST` concurrent
   * renverrait 409 alors que l'ancien dataset est déjà supprimé.
   */
  async function rebuildDataset(datasetId: string): Promise<HermesIdStatus> {
    const dataset = datasets.value.find(
      candidate => candidate.id === datasetId,
    )
    if (!dataset) {
      throw new Error(`Jeu de données ${datasetId} introuvable.`)
    }
    const zones = [...(dataset.zones ?? [])]
    if (zones.length === 0) {
      throw new Error(
        'Seuls les jeux de données composés de secteurs peuvent être reconstruits.',
      )
    }
    if (rebuildingDatasetId.value !== null) {
      throw new Error('Une reconstruction est déjà en cours.')
    }
    if (constructionInProgress.value) {
      throw new Error(
        'Une construction est en cours : réessayez quand elle sera terminée.',
      )
    }

    const name = dataset.name?.trim() || dataset.id
    rebuildingDatasetId.value = datasetId
    try {
      await deleteDataset(datasetId)
      const result = await createDataset({ name, zones })
      if (result.id !== datasetId) {
        ui.showWarning(
          `Jeu de données reconstruit sous l'identifiant « ${result.id} » ; l'ancien « ${datasetId} » a été supprimé.`,
        )
      }
      await loadServing()
      if (
        serving.value &&
        SERVING_TRANSITIONAL_STATES.has(serving.value.state)
      ) {
        void pollServing()
      }
      return result
    } catch (error) {
      recordError(error)
      throw error
    } finally {
      rebuildingDatasetId.value = null
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
          // Le build arrête ORS et maintient le service en « waiting » : on
          // relit `/serving` à chaque passage pour que le chip suive l'état
          // réel (En attente pendant la construction, puis reprise).
          await loadServing()
          if (isDatasetGraphReady(dataset)) {
            // v1.9 : `status` peut rester `preparing` pendant les aux. C'est
            // l'artefact `ors` qui marque la fin du graphe et le déclenchement
            // de la chaîne aux (précontrôle `GET /aux` implicite côté serveur).
            ui.showSuccess(
              dataset.status === 'ready'
                ? `Dataset ${dataset.id} prêt.`
                : `Dataset ${dataset.id} construit : préparation des artefacts aux…`,
            )
            void prepareAux(dataset.id, false)
            // Si ce dataset est celui servi, le build peut débloquer ORS.
            if (
              serving.value &&
              SERVING_TRANSITIONAL_STATES.has(serving.value.state)
            ) {
              void pollServing()
            }
            return
          }
          if (dataset.status === 'failed' || dataset.state === 'unavailable') {
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

  /**
   * Suit l'état du dataset servi jusqu'à un état terminal. Poll 3 s, budget
   * 45 min, keep-alive actif pendant l'attente (`waiting` puis `starting`).
   */
  async function pollServing(): Promise<void> {
    if (servingPolling.value) return
    const token = ++servingToken
    servingPolling.value = true
    void beginActivity()

    const deadline = Date.now() + SERVING_BUDGET_MS
    try {
      while (token === servingToken && Date.now() < deadline) {
        await delay(SERVING_POLL_INTERVAL_MS)
        if (token !== servingToken) return

        const previous = serving.value?.state ?? ''
        await loadServing()
        const current = serving.value?.state ?? 'none'
        if (SERVING_TRANSITIONAL_STATES.has(current)) continue

        const datasetId = serving.value?.datasetId ?? ''
        if (current === 'active') {
          if (activeDatasetId.value) {
            await loadAux(activeDatasetId.value)
          }
          if (previous !== 'active') {
            ui.showSuccess(`Dataset ${datasetId} servi : ORS actif.`)
          }
        } else if (current === 'failed') {
          const reason = serving.value?.reason
            ? ` : ${serving.value.reason}`
            : ''
          ui.showError(`Dataset ${datasetId} non servi${reason}`)
        } else if (current === 'stopped') {
          ui.showWarning('Instance ORS arrêtée.')
        }
        return
      }

      if (token === servingToken) {
        ui.showWarning(
          "Le démarrage du dataset servi n'a pas abouti après 45 min.",
        )
      }
    } finally {
      endActivity()
      if (token === servingToken) {
        servingPolling.value = false
      }
    }
  }

  /** Stoppe le suivi de l'état servi ; le serveur continue son travail. */
  function cancelServingPolling(): void {
    servingToken += 1
    servingPolling.value = false
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
        // Un dataset encore en build n'a pas encore d'artefacts aux : le 404
        // est attendu et ne doit pas devenir l'erreur globale de la vue.
        if (parseHermesError(error).code !== 'HERMES_NOT_FOUND') {
          recordError(error)
        }
      }
    } finally {
      if (token === auxToken) {
        auxLoading.value = false
      }
    }
  }

  /** Suit la préparation aux jusqu'à la fin (poll 3 s, budget 45 min). */
  async function pollAuxPreparation(
    datasetId: string,
    token: number,
  ): Promise<boolean> {
    const deadline = Date.now() + AUX_BUDGET_MS
    while (Date.now() < deadline && token === auxPrepareToken) {
      await delay(AUX_POLL_INTERVAL_MS)
      if (token !== auxPrepareToken) return false

      try {
        const status = await invoke<HermesAuxStatus>('hermes_aux_status', {
          datasetId,
        })
        if (activeDatasetId.value === datasetId) {
          activeAux.value = status
        }
        if (status.preparing) continue

        const allReady =
          auxFileReady(status.communes) &&
          auxFileReady(status.tunnels) &&
          auxFileReady(status.mm)
        if (allReady && !status.lastError) {
          // Le serveur ne passe `state` à `ready`/`degraded*` qu'après les aux :
          // on relit le manifest pour mettre à jour le tag sans changer de vue.
          await refreshDatasetState(datasetId)
          ui.showSuccess(`Artefacts aux prêts pour ${datasetId}.`)
          return true
        }

        await refreshDatasetState(datasetId)
        if (status.lastError) {
          ui.showError(
            `Préparation aux de ${datasetId} échouée : ${status.lastError}`,
          )
          return false
        }
        ui.showWarning(`Artefacts aux incomplets pour ${datasetId}.`)
        return false
      } catch {
        // Erreur transitoire : la préparation continue côté serveur.
      }
    }

    if (token === auxPrepareToken) {
      await refreshDatasetState(datasetId)
      ui.showWarning(
        `Préparation aux de ${datasetId} toujours en cours après 45 min.`,
      )
    }
    return false
  }

  /**
   * Lance la préparation des artefacts aux d'un dataset et suit son avancement.
   *
   * `force` régénère les fichiers existants. Un 409 signifie qu'une
   * préparation (ou un build) est déjà en cours : on se contente de suivre le
   * statut. Un seul suivi à la fois, le serveur ne lançant qu'une préparation.
   */
  async function prepareAux(
    datasetId: string,
    force = false,
  ): Promise<boolean> {
    if (auxPreparing.value) return false

    auxPreparing.value = true
    auxPreparingDatasetId.value = datasetId
    const token = ++auxPrepareToken
    void beginActivity()

    try {
      try {
        await invoke<HermesAuxPrepareResponse>('hermes_prepare_aux', {
          datasetId,
          force,
        })
      } catch (error) {
        const parsed = parseHermesError(error)
        if (parsed.code !== 'HERMES_CONFLICT') {
          recordError(error)
          ui.showError(parsed.message)
          return false
        }
      }
      return await pollAuxPreparation(datasetId, token)
    } finally {
      endActivity()
      if (token === auxPrepareToken) {
        auxPreparing.value = false
        auxPreparingDatasetId.value = null
      }
    }
  }

  /**
   * Vérifie que le serveur, le dataset servi et les artefacts requis sont
   * disponibles **avant** une détection. Utilisé par les stores Communes et
   * Tunnels (étapes 2 et 3).
   */
  async function ensureReady(
    kind: 'communes' | 'tunnels',
  ): Promise<HermesReadiness> {
    if (apiOnline.value !== true) {
      const online = await ping()
      if (!online) {
        return {
          ok: false,
          code: 'HERMES_OFFLINE',
          message:
            server.value === 'online'
              ? "Service hermes arrêté : l'API ne répond pas."
              : 'Serveur hermes injoignable (Tailscale ou veille).',
        }
      }
    }

    // Le statut d'un build peut avoir changé depuis le dernier chargement :
    // on relit la liste avant de conclure à un dataset non prêt.
    await loadDatasets()
    if (!activeDatasetId.value) {
      await loadServing()
    }
    if (!activeDatasetId.value) {
      return {
        ok: false,
        code: 'HERMES_NO_DATASET',
        message:
          'Aucun dataset servi. Sélectionnez-en un dans la liste des jeux de données de la vue Hermes.',
      }
    }

    // Un arrêt volontaire conserve la sélection : ORS n'est pas disponible
    // pour autant, la détection doit inviter à cliquer sur « Lancer ».
    if (
      serving.value?.state === 'stopped' ||
      serving.value?.state === 'failed'
    ) {
      return {
        ok: false,
        code: 'HERMES_NO_DATASET',
        message:
          'Service ORS indisponible. Cliquez sur « Lancer » dans la carte Serveur de la vue Hermes.',
      }
    }

    const dataset = activeDataset.value
    if (!dataset) {
      return {
        ok: false,
        code: 'HERMES_NO_DATASET',
        message: `Dataset servi introuvable (${activeDatasetId.value}).`,
      }
    }
    if (!isDatasetGraphReady(dataset)) {
      const label = dataset.stateLabel ?? dataset.state ?? dataset.status
      return {
        ok: false,
        code: 'HERMES_CONFLICT',
        message: `Dataset ${dataset.id} non prêt (${label}).`,
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
   * Charge les données nécessaires au calcul de la couverture du dataset servi
   * sur la carte de l'Accueil : serving, datasets, régions et polygones.
   *
   * Sans dataset servi, aucun chargement de géométries n'est fait. Les erreurs
   * sont absorbées par les chargements unitaires : la carte reste simplement
   * sans couverture si le serveur est indisponible.
   */
  async function ensureCoverageData(): Promise<void> {
    await loadServing()
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

  // ─── Return ────────────────────────────────────────────────────────

  return {
    // State
    server,
    apiOnline,
    baseUrl,
    lastPingAt,
    systemPingAt,
    systemAlertOpen,
    serviceAlertOpen,
    lastError,
    lastErrorCode,
    datasets,
    regions,
    polygons,
    serving,
    activeDatasetId,
    activeAux,
    loading,
    datasetsLoading,
    regionsLoading,
    polygonsLoading,
    auxLoading,
    wakeProgress,
    pollingDatasetId,
    auxPreparing,
    auxPreparingDatasetId,
    keepAliveActive,
    servingPolling,
    servingBusy,
    rebuildingDatasetId,
    // Getters
    activeDataset,
    readyDatasets,
    constructionInProgress,
    statusDisplay,
    isReady,
    // Actions
    refreshServer,
    ping,
    wake,
    startHealthMonitor,
    stopHealthMonitor,
    dismissSystemAlert,
    dismissServiceAlert,
    loadDatasets,
    loadServing,
    loadRegions,
    loadPolygons,
    loadAux,
    prepareAux,
    ensureCoverageData,
    createDataset,
    deleteDataset,
    rebuildDataset,
    pollDataset,
    cancelPolling,
    selectServingDataset,
    stopServing,
    startServedDataset,
    stopServedDataset,
    pollServing,
    cancelServingPolling,
    createPolygon,
    updatePolygon,
    deletePolygon,
    ensureReady,
    beginActivity,
    endActivity,
  }
})
